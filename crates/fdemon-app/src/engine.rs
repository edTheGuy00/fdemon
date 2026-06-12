//! Engine - shared orchestration state for TUI and headless runners
//!
//! The Engine encapsulates all shared state and initialization logic currently
//! duplicated between the TUI and headless runners. It owns the message channel,
//! session tasks, shutdown signal, file watcher, and settings.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use tokio::sync::{broadcast, mpsc, watch};
use tracing::{info, warn};

use crate::plugin::EnginePlugin;

use crate::actions::SessionTaskMap;
use crate::config::{self, Settings};
use crate::engine_event::EngineEvent;
use crate::handler::UpdateAction;
use crate::message::Message;
use crate::process;
use crate::services::{
    CommandSenderController, DevToolsSessionSnapshot, FlutterProjectService,
    LocalFlutterController, ProjectInfo, SessionSnapshot, SharedDevToolsService, SharedLogService,
    SharedSessionService, SharedState, SharedStateService, SharedVmExtensionService,
    WidgetTreeSnapshot, DEVTOOLS_SNAPSHOT_MAX_FRAMES, DEVTOOLS_SNAPSHOT_MAX_MEMORY_SAMPLES,
    DEVTOOLS_SNAPSHOT_MAX_NETWORK_ENTRIES,
};
use crate::session::SessionId;
use crate::signals;
use crate::state::{AppState, DapStatus};
use crate::watcher::{FileWatcher, WatcherConfig, WatcherEvent};
use fdemon_core::{AppPhase, LogLevel};
use fdemon_daemon::flutter_sdk;
use fdemon_dap::{adapter::DebugEvent as DapDebugEvent, DapServerHandle, DapService};

/// Lightweight snapshot of state for change detection.
///
/// Captured before message processing, compared after to detect
/// what changed and emit appropriate EngineEvents.
#[derive(Debug, Clone)]
struct StateSnapshot {
    phase: AppPhase,
    selected_session_id: Option<SessionId>,
    log_count: usize,
    /// Per-session `(id, phase)` pairs for all sessions (max 9), used to
    /// detect session removal and per-session transitions into `Stopped`.
    session_phases: Vec<(SessionId, AppPhase)>,
    _reload_count: u32,
}

impl StateSnapshot {
    fn capture(state: &AppState) -> Self {
        let (phase, log_count, reload_count) = state
            .session_manager
            .selected()
            .map(|s| {
                (
                    s.session.phase,
                    s.session.logs.len(),
                    s.session.reload_count,
                )
            })
            .unwrap_or((AppPhase::Initializing, 0, 0));

        Self {
            phase,
            selected_session_id: state.session_manager.selected().map(|s| s.session.id),
            log_count,
            session_phases: state
                .session_manager
                .iter()
                .map(|h| (h.session.id, h.session.phase))
                .collect(),
            _reload_count: reload_count,
        }
    }

    /// Phase of `session_id` at snapshot time, if the session existed.
    fn phase_of(&self, session_id: SessionId) -> Option<AppPhase> {
        self.session_phases
            .iter()
            .find(|(id, _)| *id == session_id)
            .map(|(_, phase)| *phase)
    }
}

/// Orchestration engine for Flutter Demon.
///
/// Encapsulates all shared state between TUI and headless runners:
/// - TEA state management
/// - Message channel
/// - Session task tracking
/// - Shutdown signaling
/// - File watcher
/// - Settings
/// - Shared state for service layer
/// - Event broadcasting for external consumers
pub struct Engine {
    /// TEA application state (the Model).
    ///
    /// Read access is public for rendering. State mutations should go through
    /// `process_message()` to maintain Engine invariants (event emission,
    /// SharedState sync). Direct `&mut` access is provided for TUI startup
    /// only -- do not mutate outside of the TEA cycle in normal operation.
    pub state: AppState,

    /// Sender half of the unified message channel.
    /// Clone this to give to input sources (signal handler, watcher, daemon tasks).
    pub(crate) msg_tx: mpsc::Sender<Message>,

    /// Receiver half of the unified message channel.
    /// The frontend event loop drains messages from here.
    pub(crate) msg_rx: mpsc::Receiver<Message>,

    /// Map of session IDs to their background task handles.
    pub(crate) session_tasks: SessionTaskMap,

    /// Sender for the shutdown signal. Send `true` to initiate shutdown.
    pub(crate) shutdown_tx: watch::Sender<bool>,

    /// Receiver for the shutdown signal. Clone for background tasks.
    pub(crate) shutdown_rx: watch::Receiver<bool>,

    /// File watcher for auto-reload. None if watcher failed to start.
    file_watcher: Option<FileWatcher>,

    /// Loaded settings (cached from config)
    pub settings: Settings,

    /// Path to the Flutter project
    pub project_path: PathBuf,

    /// Shared state for service layer consumers.
    /// Synchronized from AppState after message processing.
    shared_state: Arc<SharedState>,

    /// Event broadcaster for external consumers.
    /// Subscribers receive EngineEvents after each message processing cycle.
    event_tx: broadcast::Sender<EngineEvent>,

    /// Registered plugins
    plugins: Vec<Box<dyn EnginePlugin>>,

    /// Handle for the running DAP server, if any.
    ///
    /// Wrapped in `Arc<Mutex<Option<>>>` so it can be passed to
    /// `actions::handle_action` (which runs on the Tokio thread pool and needs
    /// shared, mutable access to deposit or withdraw the handle).
    ///
    /// The Engine is the sole owner of this slot; `handle_action` only writes
    /// (on `SpawnDapServer`) or reads-and-clears (on `StopDapServer`).
    pub(crate) dap_server_handle: Arc<Mutex<Option<DapServerHandle>>>,

    /// Broadcast sender for forwarding [`DapDebugEvent`]s to connected DAP sessions.
    ///
    /// Set when the DAP server starts (via `set_dap_log_sender`) and cleared when
    /// the server stops. The Engine uses this to push `LogOutput` events derived
    /// from Flutter app stdout/stderr to every connected IDE debug console.
    ///
    /// `None` when no DAP server is running (avoids unnecessary work in the log
    /// forwarding path and correctly satisfies acceptance criterion: no output
    /// events are sent when no DAP session is active).
    dap_log_event_tx: Option<tokio::sync::broadcast::Sender<DapDebugEvent>>,

    /// Shared VM handle slot for the DAP backend factory.
    ///
    /// The [`VmBackendFactory`] captures this `Arc` so it can supply the
    /// active session's [`VmRequestHandle`] to each new DAP client connection
    /// without knowing about `SessionManager` or the TEA update cycle.
    ///
    /// Updated in [`Engine::process_message`] after each TEA cycle via
    /// [`Engine::sync_vm_handle_for_dap`].  Set to `Some` when the selected
    /// session's VM Service is connected; `None` when disconnected or no
    /// session is active.
    pub(crate) vm_handle_for_dap: Arc<Mutex<Option<fdemon_daemon::vm_service::VmRequestHandle>>>,

    /// Per-DAP-client debug event senders.
    ///
    /// Each `mpsc::Sender<DebugEvent>` in this list corresponds to one active
    /// DAP client session. When the TEA handler receives a VM Service debug
    /// event (`PauseBreakpoint`, `Resume`, `IsolateStart`, etc.) it iterates
    /// this list and forwards the translated [`DapDebugEvent`] to all connected
    /// adapters using `try_send`. Stale entries (where the receiver has been
    /// dropped because the client disconnected) are pruned automatically via
    /// the `retain` pattern — `try_send` returns `Err` for a closed channel.
    ///
    /// [`VmBackendFactory::create`] registers a new sender here each time a
    /// DAP client connects and the VM Service is available.
    pub(crate) dap_debug_senders: Arc<Mutex<Vec<tokio::sync::mpsc::Sender<DapDebugEvent>>>>,
}

impl Engine {
    /// Create a new Engine for a Flutter project.
    ///
    /// Performs all shared initialization:
    /// - Initializes .fdemon directory
    /// - Loads settings from config files
    /// - Creates AppState with settings
    /// - Creates message channel (capacity 256)
    /// - Creates shutdown signal channel
    /// - Creates session task map
    /// - Spawns signal handler
    /// - Creates and starts file watcher with message bridge
    /// - Creates shared state for services layer
    pub fn new(project_path: PathBuf) -> Self {
        // 1. Init .fdemon directory (non-fatal if fails)
        if let Err(e) = config::init_fdemon_directory(&project_path) {
            warn!("Failed to initialize .fdemon directory: {}", e);
        }

        // 2. Load settings
        let settings = config::load_settings(&project_path);

        // 2.5. Resolve Flutter SDK (synchronous filesystem detection chain)
        // SDK resolution failure is NOT fatal: fdemon starts without an SDK
        // but cannot spawn sessions or discover devices until one is configured.
        let resolved_sdk = match flutter_sdk::find_flutter_sdk(
            &project_path,
            settings.flutter.sdk_path.as_deref(),
        ) {
            Ok(sdk) => Some(sdk),
            Err(e) => {
                warn!(
                    "Flutter SDK not found: {}. SDK-dependent features will be unavailable.",
                    e
                );
                None
            }
        };

        // 3. Create state
        let mut state = AppState::with_settings(project_path.clone(), settings.clone());

        // Populate resolved SDK and ToolAvailability flutter fields from detection result.
        state.tool_availability.flutter_sdk = resolved_sdk.is_some();
        state.tool_availability.flutter_sdk_source =
            resolved_sdk.as_ref().map(|s| s.source.to_string());
        state.resolved_sdk = resolved_sdk;

        // 4. Create message channel
        let (msg_tx, msg_rx) = mpsc::channel::<Message>(256);

        // 5. Create shutdown channel
        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        // 6. Create session task map
        let session_tasks: SessionTaskMap = Arc::new(std::sync::Mutex::new(HashMap::new()));

        // 7. Spawn signal handler
        signals::spawn_signal_handler(msg_tx.clone());

        // 8. Create and start file watcher
        let file_watcher = Self::start_file_watcher(&project_path, &settings, msg_tx.clone());

        // 9. Create shared state for services layer
        let shared_state = Arc::new(SharedState::new(10_000));

        // 10. Create broadcast channel for engine events (capacity 256)
        let (event_tx, _) = broadcast::channel(256);

        // 11. Create the shared DAP debug sender registry.
        //
        // Engine is the sole owner. `handle_action` (which runs on the Tokio
        // thread pool) receives a clone of this Arc and uses it to forward VM
        // debug events to connected DAP adapters via `ForwardDapDebugEvents`.
        // `VmBackendFactory::create` also receives a clone so it can register
        // per-client senders when a new DAP connection is established.
        let dap_debug_senders: Arc<Mutex<Vec<tokio::sync::mpsc::Sender<DapDebugEvent>>>> =
            Arc::new(Mutex::new(Vec::new()));

        Self {
            state,
            msg_tx,
            msg_rx,
            session_tasks,
            shutdown_tx,
            shutdown_rx,
            file_watcher,
            settings,
            project_path,
            shared_state,
            event_tx,
            plugins: Vec::new(),
            dap_server_handle: Arc::new(Mutex::new(None)),
            dap_log_event_tx: None,
            vm_handle_for_dap: Arc::new(Mutex::new(None)),
            dap_debug_senders,
        }
    }

    /// Subscribe to engine events.
    ///
    /// Returns a receiver that gets EngineEvents after each message
    /// processing cycle. Multiple subscribers are supported.
    ///
    /// If the subscriber falls behind (buffer full), older events are
    /// dropped. Use `broadcast::error::RecvError::Lagged` to detect this.
    pub fn subscribe(&self) -> broadcast::Receiver<EngineEvent> {
        self.event_tx.subscribe()
    }

    /// Register a plugin with the Engine.
    ///
    /// Plugins receive lifecycle callbacks (on_start, on_message, on_event, on_shutdown).
    /// Multiple plugins can be registered. They are called in registration order.
    pub fn register_plugin(&mut self, plugin: Box<dyn EnginePlugin>) {
        info!("Registering plugin: {}", plugin.name());
        self.plugins.push(plugin);
    }

    /// Get the number of registered plugins.
    pub fn plugin_count(&self) -> usize {
        self.plugins.len()
    }

    /// Notify all plugins that the Engine has started.
    ///
    /// This is called by runners after registering plugins and before
    /// entering the event loop.
    pub fn notify_plugins_start(&self) {
        for plugin in &self.plugins {
            if let Err(e) = plugin.on_start(&self.state) {
                warn!("Plugin '{}' on_start error: {}", plugin.name(), e);
            }
        }
    }

    /// Process a single message through the TEA update cycle.
    ///
    /// Delegates to `process::process_message()` which runs handler::update()
    /// and dispatches any resulting UpdateActions. Emits EngineEvents based
    /// on state changes detected by comparing before/after snapshots.
    pub fn process_message(&mut self, msg: Message) {
        // Snapshot state before processing
        let pre = StateSnapshot::capture(&self.state);

        // Clone message for plugin notification only if plugins are registered.
        // This avoids unnecessary cloning on the hot path when no plugins are active.
        let msg_for_plugins = if self.plugins.is_empty() {
            None
        } else {
            Some(msg.clone())
        };

        process::process_message(
            &mut self.state,
            msg,
            &self.msg_tx,
            &self.session_tasks,
            &self.shutdown_rx,
            &self.project_path,
            self.dap_server_handle.clone(),
            self.vm_handle_for_dap.clone(),
            self.dap_debug_senders.clone(),
        );

        // Snapshot state after processing
        let post = StateSnapshot::capture(&self.state);

        // Sync the DAP log event sender from the server handle.
        //
        // The sender lives in `DapServerHandle` (deposited by the async action
        // handler when the TCP server starts). We keep a copy here so that
        // `emit_events` can broadcast log events without acquiring the mutex
        // on every log line. The sync is cheap (just cloning a sender) and
        // runs once per TEA cycle.
        self.sync_dap_log_sender();

        // Keep the VM handle slot in sync with the selected session.
        self.sync_vm_handle_for_dap();

        // Drain events queued by handlers during update() (events whose
        // payloads only exist at the handler layer), then emit them together
        // with snapshot-diff events.
        let queued = std::mem::take(&mut self.state.pending_engine_events);

        // Emit events for any state changes
        self.emit_events(&pre, &post, queued);

        // Notify plugins after processing and event emission (only if registered)
        if let Some(ref m) = msg_for_plugins {
            self.notify_plugins_message(m);
        }
    }

    /// Drain and process all pending messages from the channel.
    ///
    /// Returns the number of messages processed. Used by the TUI runner
    /// which needs to drain all pending messages before rendering.
    /// Events are emitted after each message is processed.
    pub fn drain_pending_messages(&mut self) -> usize {
        let mut count = 0;
        while let Ok(msg) = self.msg_rx.try_recv() {
            self.process_message(msg);
            count += 1;
        }
        count
    }

    /// Drain runner-side-effect actions queued since the last call.
    ///
    /// Returns all `UpdateAction::SetMouseCapture` and
    /// `UpdateAction::WriteClipboard` entries that were intercepted by
    /// `process.rs` during the preceding `process_message()` / `drain_pending_messages()`
    /// calls. The internal queue is cleared on return.
    ///
    /// **Caller contract:** the TUI runner MUST call this after each call to
    /// `process_message()` or `drain_pending_messages()`, then handle every
    /// returned action synchronously before the next render cycle. Leaving
    /// actions unconsumed is a bug — `SetMouseCapture` will be silently
    /// dropped and `WriteClipboard` text will be lost.
    pub fn drain_runner_actions(&mut self) -> Vec<crate::handler::UpdateAction> {
        std::mem::take(&mut self.state.pending_runner_actions)
    }

    /// Flush pending batched logs across all sessions.
    ///
    /// Call after processing messages and before rendering/emitting events.
    /// Also synchronizes AppState to SharedState.
    pub fn flush_pending_logs(&mut self) {
        self.state.session_manager.flush_all_pending_logs();
        self.sync_shared_state_nonblocking();
    }

    /// Synchronize AppState changes to SharedState (non-blocking).
    ///
    /// Called after processing messages. One-way: AppState is the source of truth.
    /// Uses try_write() to avoid blocking - if lock is held by a service consumer,
    /// skip this sync cycle (eventual consistency).
    fn sync_shared_state_nonblocking(&self) {
        if let Some(session_handle) = self.state.session_manager.selected() {
            let session = &session_handle.session;

            // Sync app run state from selected session
            if let Ok(mut app_state) = self.shared_state.app_state.try_write() {
                app_state.phase = session.phase;
                app_state.app_id = session.app_id.clone();
                app_state.device_id = Some(session.device_id.clone());
                app_state.device_name = Some(session.device_name.clone());
                app_state.platform = Some(session.platform.clone());
                app_state.devtools_uri = session.ws_uri.clone();
                app_state.started_at = session.started_at;
                app_state.last_reload_at = session.last_reload_time;
            }

            // Sync logs from selected session (convert VecDeque to Vec)
            if let Ok(mut logs) = self.shared_state.logs.try_write() {
                // Replace with current session's logs
                // Note: This is a snapshot, not a stream -- optimize later if needed
                *logs = session.logs.iter().cloned().collect();
            }
        }

        // Sync snapshots of ALL sessions (not just the selected one) for the
        // SessionService consumers. Runs even with zero sessions so removals
        // are reflected.
        if let Ok(mut sessions) = self.shared_state.sessions.try_write() {
            *sessions = self
                .state
                .session_manager
                .iter()
                .map(|handle| {
                    let session = &handle.session;
                    SessionSnapshot {
                        session_id: session.id,
                        name: session.name.clone(),
                        device_id: session.device_id.clone(),
                        device_name: session.device_name.clone(),
                        platform: session.platform.clone(),
                        phase: session.phase,
                        app_id: session.app_id.clone(),
                        devtools_url: session
                            .devtools_endpoint
                            .as_ref()
                            .zip(session.ws_uri.as_ref())
                            .map(|(endpoint, ws_uri)| endpoint.url(ws_uri)),
                    }
                })
                .collect();
        }

        // Sync per-session DevTools telemetry snapshots for the
        // DevToolsService consumers. Telemetry vectors are tail-capped by the
        // DEVTOOLS_SNAPSHOT_MAX_* constants to bound the per-cycle clone cost.
        if let Ok(mut devtools) = self.shared_state.devtools.try_write() {
            *devtools = self
                .state
                .session_manager
                .iter()
                .map(|handle| {
                    let session = &handle.session;
                    let frames = &session.performance.frame_history;
                    let samples = &session.memory.memory_samples;
                    let requests = &session.network.entries;
                    DevToolsSessionSnapshot {
                        session_id: session.id,
                        vm_connected: session.vm_connected,
                        perf_monitoring_active: session.performance.monitoring_active,
                        network_monitoring_active: handle.network_shutdown_tx.is_some(),
                        network_extensions_available: session.network.extensions_available,
                        stats: session.performance.stats.clone(),
                        recent_frames: frames
                            .iter()
                            .skip(frames.len().saturating_sub(DEVTOOLS_SNAPSHOT_MAX_FRAMES))
                            .cloned()
                            .collect(),
                        memory_samples: samples
                            .iter()
                            .skip(
                                samples
                                    .len()
                                    .saturating_sub(DEVTOOLS_SNAPSHOT_MAX_MEMORY_SAMPLES),
                            )
                            .cloned()
                            .collect(),
                        network_requests: requests
                            .iter()
                            .skip(
                                requests
                                    .len()
                                    .saturating_sub(DEVTOOLS_SNAPSHOT_MAX_NETWORK_ENTRIES),
                            )
                            .cloned()
                            .collect(),
                    }
                })
                .collect();
        }

        // Sync the cached widget tree for the selected session. The tree can
        // be large, so it is deep-cloned into an Arc only when the inspector
        // fetch state changed (fetch started/completed or session switched);
        // in the steady state this is a cheap field comparison.
        if let Ok(mut slot) = self.shared_state.widget_tree.try_write() {
            match self.state.session_manager.selected() {
                Some(handle) => {
                    let inspector = &self.state.devtools_view_state.inspector;
                    let session_id = handle.session.id;
                    let changed = match slot.as_ref() {
                        Some(s) => {
                            s.session_id != session_id
                                || s.fetched_at != inspector.last_fetch_time
                                || s.loading != inspector.loading
                        }
                        None => true,
                    };
                    if changed {
                        *slot = Some(WidgetTreeSnapshot {
                            session_id,
                            fetched_at: inspector.last_fetch_time,
                            loading: inspector.loading,
                            error: inspector.error.as_ref().map(|e| e.message.clone()),
                            root: inspector.root.as_ref().map(|r| Arc::new(r.clone())),
                        });
                    }
                }
                None => {
                    if slot.is_some() {
                        *slot = None;
                    }
                }
            }
        }

        // Sync per-session VM request handles for VmExtensionService
        // consumers (same precedent as `sync_vm_handle_for_dap`, but for all
        // sessions). Handles are cheap to clone (channel sender + Arcs).
        if let Ok(mut vm_handles) = self.shared_state.vm_handles.try_write() {
            *vm_handles = self
                .state
                .session_manager
                .iter()
                .filter_map(|handle| {
                    handle
                        .vm_request_handle
                        .clone()
                        .map(|vm| (handle.session.id, vm))
                })
                .collect();
        }

        // Sync the device cache so StateService::get_devices works for
        // service consumers.
        if let Some(devices) = self.state.get_cached_devices() {
            if let Ok(mut shared_devices) = self.shared_state.devices.try_write() {
                *shared_devices = devices
                    .iter()
                    .map(|d| fdemon_core::DeviceInfo {
                        id: d.id.clone(),
                        name: d.name.clone(),
                        platform: d.platform.clone(),
                        emulator: d.emulator,
                        category: d.category.clone(),
                        platform_type: d.platform_type.clone(),
                        ephemeral: d.ephemeral,
                    })
                    .collect();
            }
        }
    }

    /// Get a clone of the message sender for spawning input sources.
    pub fn msg_sender(&self) -> mpsc::Sender<Message> {
        self.msg_tx.clone()
    }

    /// Receive the next message from the channel.
    ///
    /// Returns None if the channel is closed.
    pub async fn recv_message(&mut self) -> Option<Message> {
        self.msg_rx.recv().await
    }

    /// Get a clone of the shutdown receiver for background tasks.
    pub fn shutdown_receiver(&self) -> watch::Receiver<bool> {
        self.shutdown_rx.clone()
    }

    /// Dispatches a spawn-session action to start a new Flutter process.
    ///
    /// This is the external API for session creation. For full action dispatch
    /// (reload, restart, device discovery), use `process_message()` instead.
    ///
    /// Returns `false` if no Flutter SDK is available (session cannot be spawned).
    pub fn dispatch_spawn_session(
        &self,
        session_id: SessionId,
        device: fdemon_daemon::Device,
        config: Option<Box<crate::config::LaunchConfig>>,
    ) -> bool {
        let flutter = match &self.state.resolved_sdk {
            Some(sdk) => sdk.executable.clone(),
            None => {
                warn!(
                    "dispatch_spawn_session: no Flutter SDK resolved — cannot spawn session {}",
                    session_id
                );
                return false;
            }
        };

        crate::actions::handle_action(
            UpdateAction::SpawnSession {
                session_id,
                device,
                config,
                flutter,
            },
            self.msg_tx.clone(),
            None,
            Vec::new(),
            self.session_tasks.clone(),
            self.shutdown_rx.clone(),
            &self.project_path,
            Default::default(),
            self.dap_server_handle.clone(),
            self.vm_handle_for_dap.clone(),
            self.dap_debug_senders.clone(),
        );
        true
    }

    /// Returns a clone of the shared DAP debug sender registry.
    ///
    /// The registry is an `Arc<Mutex<Vec<mpsc::Sender<DebugEvent>>>>`. The
    /// [`VmBackendFactory`] uses this to register per-session event senders
    /// when a new DAP client connects. The TEA handler reads the same `Arc`
    /// when forwarding VM debug events to connected DAP adapters.
    pub fn dap_debug_senders(&self) -> Arc<Mutex<Vec<tokio::sync::mpsc::Sender<DapDebugEvent>>>> {
        self.dap_debug_senders.clone()
    }

    /// Apply a CLI `--dap-port` override.
    ///
    /// Sets the DAP port and forces `enabled = true` in both the cached
    /// settings and the embedded AppState settings, keeping them in sync.
    pub fn apply_cli_dap_override(&mut self, port: u16) {
        self.settings.dap.port = port;
        self.settings.dap.enabled = true;
        self.state.settings.dap.port = port;
        self.state.settings.dap.enabled = true;
        tracing::info!("DAP server port overridden by --dap-port: {}", port);
    }

    /// Apply a CLI-provided IDE config override (`--dap-config <ide>`).
    ///
    /// Stores the override on `AppState` so that `handle_started()` can
    /// pass it as `ide_override: Some(ide)` to `GenerateIdeConfig`, bypassing
    /// environment-based IDE detection.
    pub fn apply_cli_dap_config_override(&mut self, ide: crate::config::ParentIde) {
        self.state.cli_dap_config_override = Some(ide);
        tracing::info!("DAP IDE config overridden by --dap-config: {:?}", ide);
    }

    /// Check if the application should quit.
    pub fn should_quit(&self) -> bool {
        self.state.should_quit()
    }

    /// Get a FlutterController for the currently selected session.
    ///
    /// Returns None if no session is selected or no command sender is available.
    pub fn flutter_controller(&self) -> Option<impl LocalFlutterController + '_> {
        let session = self.state.session_manager.selected()?;
        let cmd_sender = session.cmd_sender.as_ref()?;
        Some(CommandSenderController::new(
            cmd_sender.clone(),
            self.shared_state.clone(),
        ))
    }

    /// Get an owned, `Send + 'static` FlutterController for the currently
    /// selected session.
    ///
    /// Unlike [`Engine::flutter_controller`], the returned controller does not
    /// borrow the Engine and implements the `Send` trait variant
    /// ([`crate::services::FlutterController`]), so remote consumers (MCP
    /// server) can move it into spawned tokio tasks and call
    /// `reload()`/`restart()` from there.
    ///
    /// Returns None if no session is selected or no command sender is available.
    pub fn flutter_controller_owned(&self) -> Option<CommandSenderController> {
        let session = self.state.session_manager.selected()?;
        let cmd_sender = session.cmd_sender.as_ref()?;
        Some(CommandSenderController::new(
            cmd_sender.clone(),
            self.shared_state.clone(),
        ))
    }

    /// Get a session control service (list/start/stop sessions, DevTools URLs).
    ///
    /// `Send + 'static`: reads come from SharedState snapshots, control
    /// operations are dispatched through the Engine's message channel.
    pub fn session_service(&self) -> SharedSessionService {
        SharedSessionService::new(self.shared_state.clone(), self.msg_tx.clone())
    }

    /// Get a DevTools telemetry service (frames, memory, network, widget tree,
    /// headless monitoring control).
    ///
    /// `Send + 'static`: reads come from SharedState snapshots, control
    /// operations are dispatched through the Engine's message channel.
    pub fn devtools_service(&self) -> SharedDevToolsService {
        SharedDevToolsService::new(self.shared_state.clone(), self.msg_tx.clone())
    }

    /// Get a generic VM service-extension pass-through service (invoke and
    /// discover registered `ext.*` methods per session).
    ///
    /// `Send + 'static`: calls go directly over the per-session VM handles
    /// the Engine syncs into [`SharedState`] after each TEA cycle. No
    /// allowlist is enforced — callers are responsible for what they invoke
    /// (debug-mode VM seam; the TUI never calls this service).
    pub fn vm_extension_service(&self) -> SharedVmExtensionService {
        SharedVmExtensionService::new(self.shared_state.clone())
    }

    /// Get a project operations service (`flutter pub get` / `flutter clean`).
    ///
    /// Returns None when no Flutter SDK has been resolved.
    pub fn project_service(&self) -> Option<FlutterProjectService> {
        self.state.resolved_sdk.as_ref().map(|sdk| {
            FlutterProjectService::new(sdk.executable.clone(), self.project_path.clone())
        })
    }

    /// Get access to the shared log service.
    pub fn log_service(&self) -> SharedLogService {
        SharedLogService::new(self.shared_state.logs.clone(), self.shared_state.max_logs)
    }

    /// Get access to the shared state service.
    pub fn state_service(&self) -> SharedStateService {
        let project_name = self
            .project_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        let project_info = ProjectInfo::new(project_name, self.project_path.clone());
        SharedStateService::new(self.shared_state.clone(), project_info)
    }

    /// Get a reference to the shared state (for custom consumers).
    pub fn shared_state(&self) -> &Arc<SharedState> {
        &self.shared_state
    }

    /// Initiate shutdown: stop DAP server, watcher, signal background tasks, cleanup sessions.
    pub async fn shutdown(&mut self) {
        // Notify plugins first
        self.notify_plugins_shutdown();

        // Emit shutdown event
        self.emit(EngineEvent::Shutdown);

        // Stop DAP server if running
        let dap_handle = match self.dap_server_handle.lock() {
            Ok(mut guard) => guard.take(),
            Err(e) => {
                warn!("DAP handle lock poisoned during shutdown: {}", e);
                None
            }
        };
        if let Some(handle) = dap_handle {
            info!("Stopping DAP server...");
            DapService::stop(handle).await;
            self.state.dap_status = DapStatus::Off;
        }

        // Stop file watcher
        if let Some(ref mut watcher) = self.file_watcher {
            watcher.stop();
        }

        // Gracefully shut down native logs and custom sources for all sessions.
        // This sends the shutdown signal and aborts tasks so child processes
        // receive SIGKILL via kill_on_drop before the tokio runtime winds down.
        for handle in self.state.session_manager.iter_mut() {
            handle.shutdown_native_logs();
        }

        // Shut down shared custom sources (project-level, not per-session).
        // Order matters: per-session sources first, then shared sources (a shared
        // source might be serving multiple sessions), then the global shutdown signal.
        self.state.shutdown_shared_sources();

        // Signal all background tasks to stop
        let _ = self.shutdown_tx.send(true);

        // Drain remaining session tasks with timeout
        let tasks: Vec<_> = {
            match self.session_tasks.lock() {
                Ok(mut map) => map.drain().collect(),
                Err(e) => {
                    warn!(
                        "Failed to acquire session tasks lock during shutdown (poisoned): {}",
                        e
                    );
                    Vec::new()
                }
            }
        };

        for (session_id, handle) in tasks {
            match tokio::time::timeout(std::time::Duration::from_secs(2), handle).await {
                Ok(Ok(())) => info!("Session {} cleaned up", session_id),
                Ok(Err(e)) => warn!("Session {} panicked: {}", session_id, e),
                Err(_) => warn!("Session {} cleanup timed out", session_id),
            }
        }
    }

    /// Synchronize the cached DAP log event sender from the server handle.
    ///
    /// Called once per TEA cycle in [`process_message`]. Acquires the DAP
    /// handle slot (non-blocking, using `try_lock`) and clones the log event
    /// sender if a handle is present. Clears the cached sender when the handle
    /// is absent (server stopped).
    ///
    /// This keeps `dap_log_event_tx` in sync without holding the mutex lock
    /// during the hot log-forwarding path in `emit_events`.
    fn sync_dap_log_sender(&mut self) {
        match self.dap_server_handle.try_lock() {
            Ok(guard) => {
                self.dap_log_event_tx = guard.as_ref().map(|handle| handle.log_event_sender());
            }
            Err(_) => {
                // Lock held by the action handler — skip this cycle, retry next.
            }
        }
    }

    /// Sync `vm_handle_for_dap` from the selected session's `vm_request_handle`.
    ///
    /// Called once per TEA cycle after message processing. The shared slot is
    /// updated to match the selected session's current VM handle so that the
    /// [`VmBackendFactory`] always produces a fresh clone for new DAP clients.
    ///
    /// - If the selected session has a connected VM Service, the slot is `Some`.
    /// - If the session has no VM handle (not yet connected, or disconnected),
    ///   the slot is set to `None`.
    /// - If no session is selected, the slot is set to `None`.
    fn sync_vm_handle_for_dap(&self) {
        let new_handle = self
            .state
            .session_manager
            .selected()
            .and_then(|sh| sh.vm_request_handle.clone());

        match self.vm_handle_for_dap.try_lock() {
            Ok(mut guard) => {
                *guard = new_handle;
            }
            Err(_) => {
                // Lock held by the factory — skip this cycle, retry next.
            }
        }
    }

    /// Emit EngineEvents based on state changes after processing.
    ///
    /// Called after process_message() and flush_pending_logs().
    /// Emits handler-queued events first (session lifecycle, reload/restart
    /// outcomes, device discovery, file changes — see
    /// `AppState::pending_engine_events`), then events derived by comparing
    /// the pre/post snapshots.
    fn emit_events(&self, pre: &StateSnapshot, post: &StateSnapshot, queued: Vec<EngineEvent>) {
        // A restart drives the same Reloading phase transition as a reload;
        // when this cycle queued RestartStarted for a session, suppress the
        // snapshot-derived ReloadStarted so subscribers see only the truthful
        // restart event.
        let restart_started: Vec<SessionId> = queued
            .iter()
            .filter_map(|event| match event {
                EngineEvent::RestartStarted { session_id } => Some(*session_id),
                _ => None,
            })
            .collect();

        for event in queued {
            self.emit(event);
        }

        // Sessions that transitioned into Stopped (any session, not only the
        // selected one — the stop may come from a background process exit).
        for (session_id, post_phase) in &post.session_phases {
            if *post_phase != AppPhase::Stopped {
                continue;
            }
            if matches!(pre.phase_of(*session_id), Some(pre_phase) if pre_phase != AppPhase::Stopped)
            {
                self.emit(EngineEvent::SessionStopped {
                    session_id: *session_id,
                    // The snapshot diff carries no exit reason.
                    reason: None,
                });
            }
        }

        // Sessions removed from the session manager.
        for (session_id, _) in &pre.session_phases {
            if post.phase_of(*session_id).is_none() {
                self.emit(EngineEvent::SessionRemoved {
                    session_id: *session_id,
                });
            }
        }

        // Phase changes
        if pre.phase != post.phase {
            if let Some(session_id) = post.selected_session_id {
                self.emit(EngineEvent::PhaseChanged {
                    session_id,
                    old_phase: pre.phase,
                    new_phase: post.phase,
                });
            }
        }

        // Reload detection - transition from non-Reloading to Reloading.
        // Restart triggers are excluded (they queued RestartStarted above).
        if pre.phase != AppPhase::Reloading && post.phase == AppPhase::Reloading {
            if let Some(session_id) = post.selected_session_id {
                if !restart_started.contains(&session_id) {
                    self.emit(EngineEvent::ReloadStarted { session_id });
                }
            }
        }

        // Reload completion (ReloadCompleted with the daemon-measured time_ms),
        // reload failure, and restart completion are handler-queued events —
        // the snapshot diff cannot distinguish them (all three are a
        // Reloading -> Running transition) nor measure the reload time.

        // New logs detected
        if post.log_count > pre.log_count {
            if let Some(session_id) = post.selected_session_id {
                // Get new log entries
                if let Some(session_handle) = self.state.session_manager.selected() {
                    let new_count = post.log_count - pre.log_count;
                    let logs: Vec<_> = session_handle
                        .session
                        .logs
                        .iter()
                        .rev()
                        .take(new_count)
                        .rev()
                        .cloned()
                        .collect();

                    // Forward new log entries to DAP sessions (if any are connected).
                    // Only forward when DAP is running with at least one client.
                    if let Some(dap_tx) = &self.dap_log_event_tx {
                        if self.state.dap_status.client_count() > 0 {
                            for log in &logs {
                                let level = match log.level {
                                    LogLevel::Error => "error",
                                    LogLevel::Info => "info",
                                    LogLevel::Warning => "warning",
                                    LogLevel::Debug => "debug",
                                }
                                .to_string();
                                let dap_event = DapDebugEvent::LogOutput {
                                    message: log.message.clone(),
                                    level,
                                    source_uri: None,
                                    line: None,
                                };
                                // Ignore send errors — no subscribers means no clients.
                                let _ = dap_tx.send(dap_event);
                            }
                        }
                    }

                    // Use batch emission for multiple logs (more efficient)
                    if logs.len() > 1 {
                        self.emit(EngineEvent::LogBatch {
                            session_id,
                            entries: logs,
                        });
                    } else if let Some(entry) = logs.first() {
                        self.emit(EngineEvent::LogEntry {
                            session_id,
                            entry: entry.clone(),
                        });
                    }
                }
            }
        }
    }

    /// Emit a single EngineEvent to all subscribers.
    ///
    /// send() returns Err only if there are no receivers -- that's fine,
    /// we don't want to panic or log errors for having no subscribers.
    fn emit(&self, event: EngineEvent) {
        // Broadcast to channel subscribers
        let _ = self.event_tx.send(event.clone());

        // Notify plugins
        for plugin in &self.plugins {
            if let Err(e) = plugin.on_event(&event) {
                warn!("Plugin '{}' on_event error: {}", plugin.name(), e);
            }
        }
    }

    /// Notify all plugins that a message was processed.
    fn notify_plugins_message(&self, msg: &Message) {
        for plugin in &self.plugins {
            if let Err(e) = plugin.on_message(msg, &self.state) {
                warn!("Plugin '{}' on_message error: {}", plugin.name(), e);
            }
        }
    }

    /// Notify all plugins about shutdown.
    fn notify_plugins_shutdown(&self) {
        for plugin in &self.plugins {
            if let Err(e) = plugin.on_shutdown() {
                warn!("Plugin '{}' on_shutdown error: {}", plugin.name(), e);
            }
        }
    }

    /// Create and start the file watcher, bridging events to messages.
    fn start_file_watcher(
        project_path: &Path,
        settings: &Settings,
        msg_tx: mpsc::Sender<Message>,
    ) -> Option<FileWatcher> {
        let mut watcher = FileWatcher::new(
            project_path.to_path_buf(),
            WatcherConfig::new()
                .with_paths(settings.watcher.paths.iter().map(PathBuf::from).collect())
                .with_extensions(settings.watcher.extensions.clone())
                .with_debounce_ms(settings.watcher.debounce_ms)
                .with_auto_reload(settings.watcher.auto_reload),
        );

        let (watcher_tx, mut watcher_rx) = mpsc::channel::<WatcherEvent>(32);

        if let Err(e) = watcher.start(watcher_tx) {
            warn!("Failed to start file watcher: {}", e);
            return None;
        }

        // Bridge watcher events to app messages
        tokio::spawn(async move {
            while let Some(event) = watcher_rx.recv().await {
                let msg = match event {
                    WatcherEvent::AutoReloadTriggered => Message::AutoReloadTriggered,
                    WatcherEvent::FilesChanged { count } => Message::FilesChanged { count },
                    WatcherEvent::Error { message } => Message::WatcherError { message },
                };
                let _ = msg_tx.send(msg).await;
            }
        });

        Some(watcher)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fdemon_core::AppPhase;

    #[tokio::test]
    async fn test_engine_new_creates_valid_state() {
        // Engine::new() requires a project path but doesn't require Flutter
        // Use a temp directory to test construction
        let dir = tempfile::tempdir().unwrap();
        let engine = Engine::new(dir.path().to_path_buf());

        assert!(!engine.should_quit());
        assert_eq!(engine.project_path, dir.path());
    }

    #[tokio::test]
    async fn test_engine_drain_empty_channel() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(dir.path().to_path_buf());

        // No messages pending
        assert_eq!(engine.drain_pending_messages(), 0);
    }

    #[tokio::test]
    async fn test_engine_process_quit_message() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(dir.path().to_path_buf());

        engine.process_message(Message::Quit);
        assert!(engine.should_quit());
    }

    #[tokio::test]
    async fn test_engine_shutdown() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(dir.path().to_path_buf());

        // Should not panic on empty engine
        engine.shutdown().await;
    }

    #[tokio::test]
    async fn test_shared_state_initialized() {
        let dir = tempfile::tempdir().unwrap();
        let engine = Engine::new(dir.path().to_path_buf());

        let state = engine.shared_state().app_state.read().await;
        assert_eq!(state.phase, AppPhase::Initializing);
    }

    #[tokio::test]
    async fn test_shared_state_sync_after_flush() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(dir.path().to_path_buf());

        // Initially no sessions, so sync should be a no-op
        engine.flush_pending_logs();

        // SharedState should still be in default state
        let state = engine.shared_state().app_state.read().await;
        assert_eq!(state.phase, AppPhase::Initializing);
        assert!(state.app_id.is_none());
    }

    #[tokio::test]
    async fn test_log_service_accessor() {
        let dir = tempfile::tempdir().unwrap();
        let engine = Engine::new(dir.path().to_path_buf());

        let _log_service = engine.log_service();
        // Should not panic
    }

    #[tokio::test]
    async fn test_state_service_accessor() {
        let dir = tempfile::tempdir().unwrap();
        let engine = Engine::new(dir.path().to_path_buf());

        let _state_service = engine.state_service();
        // Should not panic
    }

    #[tokio::test]
    async fn test_flutter_controller_none_without_session() {
        let dir = tempfile::tempdir().unwrap();
        let engine = Engine::new(dir.path().to_path_buf());

        // No session selected, should return None
        assert!(engine.flutter_controller().is_none());
    }

    #[tokio::test]
    async fn test_flutter_controller_owned_none_without_session() {
        let dir = tempfile::tempdir().unwrap();
        let engine = Engine::new(dir.path().to_path_buf());

        assert!(engine.flutter_controller_owned().is_none());
    }

    #[tokio::test]
    async fn test_flutter_controller_owned_is_send_and_static() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(dir.path().to_path_buf());
        let session_id = engine
            .state
            .session_manager
            .create_session(&test_device("dev-1", "Pixel 6"))
            .unwrap();
        engine
            .state
            .session_manager
            .get_mut(session_id)
            .unwrap()
            .cmd_sender = Some(fdemon_daemon::CommandSender::new_for_test());

        let controller = engine
            .flutter_controller_owned()
            .expect("session with cmd_sender should yield a controller");

        // Moving the controller into a spawned task requires Send + 'static.
        let handle = tokio::spawn(async move {
            crate::services::FlutterController::is_running(&controller).await
        });
        assert!(!handle.await.unwrap());
    }

    #[tokio::test]
    async fn test_session_service_accessor() {
        let dir = tempfile::tempdir().unwrap();
        let engine = Engine::new(dir.path().to_path_buf());

        let _session_service = engine.session_service();
        // Should not panic
    }

    #[tokio::test]
    async fn test_project_service_none_without_sdk() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(dir.path().to_path_buf());
        engine.state.resolved_sdk = None;

        assert!(engine.project_service().is_none());
    }

    #[tokio::test]
    async fn test_project_service_some_with_resolved_sdk() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(dir.path().to_path_buf());
        engine.state.resolved_sdk = Some(fdemon_daemon::test_utils::fake_flutter_sdk());

        assert!(engine.project_service().is_some());
    }

    #[tokio::test]
    async fn test_session_snapshots_synced_to_shared_state() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(dir.path().to_path_buf());
        let session_id = engine
            .state
            .session_manager
            .create_session(&test_device("dev-1", "Pixel 6"))
            .unwrap();
        {
            let handle = engine.state.session_manager.get_mut(session_id).unwrap();
            handle.session.app_id = Some("app-1".to_string());
            handle.session.ws_uri = Some("ws://127.0.0.1:1234/abc=/ws".to_string());
            handle.session.devtools_endpoint = Some(crate::session::DevToolsEndpoint {
                base_url: "http://127.0.0.1:9100".to_string(),
            });
        }

        engine.flush_pending_logs();

        let sessions = engine.shared_state().sessions.read().await;
        assert_eq!(sessions.len(), 1);
        let snapshot = &sessions[0];
        assert_eq!(snapshot.session_id, session_id);
        assert_eq!(snapshot.device_id, "dev-1");
        assert_eq!(snapshot.device_name, "Pixel 6");
        assert_eq!(snapshot.app_id, Some("app-1".to_string()));
        let devtools_url = snapshot.devtools_url.as_deref().unwrap();
        assert!(devtools_url.starts_with("http://127.0.0.1:9100?uri="));
    }

    #[tokio::test]
    async fn test_vm_extension_service_accessor() {
        let dir = tempfile::tempdir().unwrap();
        let engine = Engine::new(dir.path().to_path_buf());

        let _vm_extension_service = engine.vm_extension_service();
        // Should not panic
    }

    #[tokio::test]
    async fn test_vm_handles_synced_to_shared_state() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(dir.path().to_path_buf());
        let session_id = engine
            .state
            .session_manager
            .create_session(&test_device("dev-1", "Pixel 6"))
            .unwrap();

        // No VM handle yet — sync leaves the map empty.
        engine.flush_pending_logs();
        assert!(engine.shared_state().vm_handles.read().await.is_empty());

        // Attach a VM handle — sync publishes it under the session id.
        engine
            .state
            .session_manager
            .get_mut(session_id)
            .unwrap()
            .vm_request_handle = Some(fdemon_daemon::vm_service::VmRequestHandle::new_for_test(
            None,
        ));
        engine.flush_pending_logs();
        assert!(engine
            .shared_state()
            .vm_handles
            .read()
            .await
            .contains_key(&session_id));

        // Handle cleared (VM disconnected) — sync removes the entry.
        engine
            .state
            .session_manager
            .get_mut(session_id)
            .unwrap()
            .vm_request_handle = None;
        engine.flush_pending_logs();
        assert!(engine.shared_state().vm_handles.read().await.is_empty());
    }

    #[tokio::test]
    async fn test_session_snapshot_devtools_url_none_without_endpoint() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(dir.path().to_path_buf());
        engine
            .state
            .session_manager
            .create_session(&test_device("dev-1", "Pixel 6"))
            .unwrap();

        engine.flush_pending_logs();

        let sessions = engine.shared_state().sessions.read().await;
        assert_eq!(sessions.len(), 1);
        assert!(sessions[0].devtools_url.is_none());
    }

    #[tokio::test]
    async fn test_session_snapshots_cleared_after_removal() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(dir.path().to_path_buf());
        let session_id = engine
            .state
            .session_manager
            .create_session(&test_device("dev-1", "Pixel 6"))
            .unwrap();
        engine.flush_pending_logs();
        assert_eq!(engine.shared_state().sessions.read().await.len(), 1);

        engine.state.session_manager.remove_session(session_id);
        engine.flush_pending_logs();

        assert!(engine.shared_state().sessions.read().await.is_empty());
    }

    #[tokio::test]
    async fn test_device_cache_synced_to_shared_state() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(dir.path().to_path_buf());
        engine
            .state
            .set_device_cache(vec![test_device("dev-1", "Pixel 6")]);

        engine.flush_pending_logs();

        let devices = engine.shared_state().devices.read().await;
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].id, "dev-1");
        assert_eq!(devices[0].name, "Pixel 6");
        assert_eq!(devices[0].platform, "android");
    }

    #[tokio::test]
    async fn test_shared_state_reference() {
        let dir = tempfile::tempdir().unwrap();
        let engine = Engine::new(dir.path().to_path_buf());

        let shared_state = engine.shared_state();
        assert_eq!(shared_state.max_logs, 10_000);
    }

    // ─────────────────────────────────────────────────────────
    // Event Broadcasting Tests (Task 06)
    // ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_subscribe_receives_shutdown_event() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(dir.path().to_path_buf());

        let mut rx = engine.subscribe();

        // Shutdown should emit event
        engine.shutdown().await;

        // Should receive shutdown event
        match tokio::time::timeout(std::time::Duration::from_millis(100), rx.recv()).await {
            Ok(Ok(event)) => {
                assert!(matches!(event, EngineEvent::Shutdown));
            }
            _ => panic!("Should have received shutdown event"),
        }
    }

    #[tokio::test]
    async fn test_no_subscribers_no_error() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(dir.path().to_path_buf());

        // No subscribers -- should not error
        engine.process_message(Message::Quit);
        // No panic
    }

    #[tokio::test]
    async fn test_multiple_subscribers() {
        let dir = tempfile::tempdir().unwrap();
        let engine = Engine::new(dir.path().to_path_buf());

        let _rx1 = engine.subscribe();
        let _rx2 = engine.subscribe();
        let _rx3 = engine.subscribe();

        // All three should be valid receivers
    }

    #[test]
    fn test_state_snapshot_capture() {
        let state = AppState::new();
        let snapshot = StateSnapshot::capture(&state);

        assert_eq!(snapshot.phase, AppPhase::Initializing);
        assert_eq!(snapshot.log_count, 0);
        assert!(snapshot.session_phases.is_empty());
    }

    #[tokio::test]
    async fn test_subscribe_channel_capacity() {
        let dir = tempfile::tempdir().unwrap();
        let engine = Engine::new(dir.path().to_path_buf());

        let mut rx = engine.subscribe();

        // Generate many events to test buffer size (256 capacity)
        for _ in 0..100 {
            engine.emit(EngineEvent::Shutdown);
        }

        // Should be able to receive at least some events
        let mut count = 0;
        while rx.try_recv().is_ok() {
            count += 1;
        }

        assert!(count > 0, "Should have received some events");
        assert!(count <= 256, "Should not exceed buffer capacity");
    }

    #[tokio::test]
    async fn test_phase_change_event() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(dir.path().to_path_buf());

        let mut rx = engine.subscribe();

        // Process quit message which changes phase to Quitting
        engine.process_message(Message::Quit);

        // Should receive PhaseChanged event
        match tokio::time::timeout(std::time::Duration::from_millis(100), rx.recv()).await {
            Ok(Ok(event)) => match event {
                EngineEvent::PhaseChanged {
                    old_phase,
                    new_phase,
                    ..
                } => {
                    assert_eq!(old_phase, AppPhase::Initializing);
                    assert_eq!(new_phase, AppPhase::Quitting);
                }
                _ => panic!("Expected PhaseChanged event, got {:?}", event),
            },
            _ => {
                // No session selected, so no event expected - this is OK
            }
        }
    }

    #[tokio::test]
    async fn test_event_type_label() {
        let event = EngineEvent::Shutdown;
        assert_eq!(event.event_type(), "shutdown");
    }

    // ─────────────────────────────────────────────────────────
    // Event Emission Tests (engine-events)
    // ─────────────────────────────────────────────────────────

    fn test_device(id: &str, name: &str) -> fdemon_daemon::Device {
        fdemon_daemon::Device {
            id: id.to_string(),
            name: name.to_string(),
            platform: "android".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
            is_supported: true,
            capabilities: None,
        }
    }

    /// Collect all events already broadcast (emission is synchronous within
    /// `process_message`, so no waiting is needed).
    fn drain_events(rx: &mut broadcast::Receiver<EngineEvent>) -> Vec<EngineEvent> {
        let mut events = Vec::new();
        while let Ok(event) = rx.try_recv() {
            events.push(event);
        }
        events
    }

    #[tokio::test]
    async fn test_session_created_event_emitted_on_auto_launch() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(dir.path().to_path_buf());
        // Prevent a real `flutter run` spawn on machines with a global SDK —
        // session creation (and the event) happens before the spawn dispatch.
        engine.state.resolved_sdk = None;

        let mut rx = engine.subscribe();
        let device = test_device("emulator-5554", "Pixel 6");
        engine.process_message(Message::AutoLaunchResult {
            result: Ok(crate::message::AutoLaunchSuccess {
                device,
                config: None,
            }),
        });

        let events = drain_events(&mut rx);
        let created = events
            .iter()
            .find(|e| matches!(e, EngineEvent::SessionCreated { .. }))
            .expect("SessionCreated should be emitted when auto-launch creates a session");
        match created {
            EngineEvent::SessionCreated { session_id, device } => {
                assert_eq!(
                    Some(*session_id),
                    engine.state.session_manager.selected_id()
                );
                assert_eq!(device.id, "emulator-5554");
                assert_eq!(device.name, "Pixel 6");
            }
            _ => unreachable!(),
        }
    }

    #[tokio::test]
    async fn test_session_started_event_emitted_with_session_payload() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(dir.path().to_path_buf());
        let session_id = engine
            .state
            .session_manager
            .create_session(&test_device("dev-1", "Pixel 6"))
            .unwrap();

        let mut rx = engine.subscribe();
        engine.process_message(Message::SessionStarted {
            session_id,
            device_id: "dev-1".to_string(),
            device_name: "Pixel 6".to_string(),
            platform: "android".to_string(),
            pid: Some(4242),
        });

        let events = drain_events(&mut rx);
        let started = events
            .iter()
            .find(|e| matches!(e, EngineEvent::SessionStarted { .. }))
            .expect("SessionStarted should be emitted when the Flutter process starts");
        match started {
            EngineEvent::SessionStarted {
                session_id: sid,
                device_id,
                device_name,
                platform,
                pid,
            } => {
                assert_eq!(*sid, session_id);
                assert_eq!(device_id, "dev-1");
                assert_eq!(device_name, "Pixel 6");
                assert_eq!(platform, "android");
                assert_eq!(*pid, Some(4242));
            }
            _ => unreachable!(),
        }
    }

    #[tokio::test]
    async fn test_session_stopped_event_emitted_on_process_exit() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(dir.path().to_path_buf());
        let session_id = engine
            .state
            .session_manager
            .create_session(&test_device("dev-1", "Pixel 6"))
            .unwrap();

        let mut rx = engine.subscribe();
        engine.process_message(Message::SessionDaemon {
            session_id,
            event: fdemon_core::DaemonEvent::Exited { code: Some(0) },
        });

        let events = drain_events(&mut rx);
        assert!(
            events.iter().any(|e| matches!(
                e,
                EngineEvent::SessionStopped { session_id: sid, reason: None } if *sid == session_id
            )),
            "SessionStopped should be emitted on process exit, got {:?}",
            events
        );
    }

    #[tokio::test]
    async fn test_session_removed_event_emitted_on_close() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(dir.path().to_path_buf());
        let first = engine
            .state
            .session_manager
            .create_session(&test_device("dev-1", "Pixel 6"))
            .unwrap();
        let _second = engine
            .state
            .session_manager
            .create_session(&test_device("dev-2", "iPhone 15"))
            .unwrap();
        assert_eq!(engine.state.session_manager.selected_id(), Some(first));

        let mut rx = engine.subscribe();
        engine.process_message(Message::CloseCurrentSession);

        let events = drain_events(&mut rx);
        assert!(
            events.iter().any(|e| matches!(
                e,
                EngineEvent::SessionRemoved { session_id } if *session_id == first
            )),
            "SessionRemoved should be emitted when a session is closed, got {:?}",
            events
        );
    }

    #[tokio::test]
    async fn test_reload_completed_event_carries_daemon_measured_time() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(dir.path().to_path_buf());
        let session_id = engine
            .state
            .session_manager
            .create_session(&test_device("dev-1", "Pixel 6"))
            .unwrap();
        {
            let handle = engine.state.session_manager.get_mut(session_id).unwrap();
            handle.session.mark_started("app-1".to_string());
            handle.session.mark_running();
            handle.session.start_reload();
        }

        let mut rx = engine.subscribe();
        engine.process_message(Message::SessionReloadCompleted {
            session_id,
            time_ms: 123,
        });

        let events = drain_events(&mut rx);
        let completed = events
            .iter()
            .find(|e| matches!(e, EngineEvent::ReloadCompleted { .. }))
            .expect("ReloadCompleted should be emitted when a reload finishes");
        match completed {
            EngineEvent::ReloadCompleted {
                session_id: sid,
                time_ms,
            } => {
                assert_eq!(*sid, session_id);
                assert_eq!(*time_ms, 123, "time_ms must be the daemon-measured value");
                assert!(*time_ms > 0, "time_ms must no longer be the hardcoded 0");
            }
            _ => unreachable!(),
        }
    }

    #[tokio::test]
    async fn test_stale_reload_completed_does_not_emit_event() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(dir.path().to_path_buf());
        let session_id = engine
            .state
            .session_manager
            .create_session(&test_device("dev-1", "Pixel 6"))
            .unwrap();
        // Session is not Reloading — the completion is stale.

        let mut rx = engine.subscribe();
        engine.process_message(Message::SessionReloadCompleted {
            session_id,
            time_ms: 50,
        });

        let events = drain_events(&mut rx);
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, EngineEvent::ReloadCompleted { .. })),
            "a stale completion must not emit ReloadCompleted, got {:?}",
            events
        );
    }

    #[tokio::test]
    async fn test_reload_failed_event_emitted_with_reason() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(dir.path().to_path_buf());
        let session_id = engine
            .state
            .session_manager
            .create_session(&test_device("dev-1", "Pixel 6"))
            .unwrap();
        {
            let handle = engine.state.session_manager.get_mut(session_id).unwrap();
            handle.session.mark_started("app-1".to_string());
            handle.session.mark_running();
            handle.session.start_reload();
        }

        let mut rx = engine.subscribe();
        engine.process_message(Message::SessionReloadFailed {
            session_id,
            reason: "compile error".to_string(),
        });

        let events = drain_events(&mut rx);
        assert!(
            events.iter().any(|e| matches!(
                e,
                EngineEvent::ReloadFailed { session_id: sid, reason }
                    if *sid == session_id && reason == "compile error"
            )),
            "ReloadFailed should be emitted with the failure reason, got {:?}",
            events
        );
    }

    #[tokio::test]
    async fn test_restart_started_event_emitted_instead_of_reload_started() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(dir.path().to_path_buf());
        let session_id = engine
            .state
            .session_manager
            .create_session(&test_device("dev-1", "Pixel 6"))
            .unwrap();
        {
            let handle = engine.state.session_manager.get_mut(session_id).unwrap();
            handle.session.mark_started("app-1".to_string());
            handle.session.mark_running();
            handle.cmd_sender = Some(fdemon_daemon::CommandSender::new_for_test());
        }

        let mut rx = engine.subscribe();
        engine.process_message(Message::HotRestart);

        let events = drain_events(&mut rx);
        assert!(
            events.iter().any(|e| matches!(
                e,
                EngineEvent::RestartStarted { session_id: sid } if *sid == session_id
            )),
            "RestartStarted should be emitted when a hot restart begins, got {:?}",
            events
        );
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, EngineEvent::ReloadStarted { .. })),
            "a restart must not emit ReloadStarted, got {:?}",
            events
        );
    }

    #[tokio::test]
    async fn test_restart_completed_event_emitted() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(dir.path().to_path_buf());
        let session_id = engine
            .state
            .session_manager
            .create_session(&test_device("dev-1", "Pixel 6"))
            .unwrap();
        {
            let handle = engine.state.session_manager.get_mut(session_id).unwrap();
            handle.session.mark_started("app-1".to_string());
            handle.session.mark_running();
            handle.session.start_reload();
        }

        let mut rx = engine.subscribe();
        engine.process_message(Message::SessionRestartCompleted { session_id });

        let events = drain_events(&mut rx);
        assert!(
            events.iter().any(|e| matches!(
                e,
                EngineEvent::RestartCompleted { session_id: sid } if *sid == session_id
            )),
            "RestartCompleted should be emitted when a restart finishes, got {:?}",
            events
        );
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, EngineEvent::ReloadCompleted { .. })),
            "a restart completion must not emit ReloadCompleted, got {:?}",
            events
        );
    }

    #[tokio::test]
    async fn test_devices_discovered_event_emitted() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(dir.path().to_path_buf());

        let mut rx = engine.subscribe();
        engine.process_message(Message::DevicesDiscovered {
            devices: vec![test_device("dev-1", "Pixel 6")],
        });

        let events = drain_events(&mut rx);
        assert!(
            events.iter().any(|e| matches!(
                e,
                EngineEvent::DevicesDiscovered { devices }
                    if devices.len() == 1 && devices[0].id == "dev-1"
            )),
            "DevicesDiscovered should be emitted with the device list, got {:?}",
            events
        );
    }

    #[tokio::test]
    async fn test_files_changed_event_emitted_without_auto_reload() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(dir.path().to_path_buf());

        let mut rx = engine.subscribe();
        engine.process_message(Message::FilesChanged { count: 3 });

        let events = drain_events(&mut rx);
        assert!(
            events.iter().any(|e| matches!(
                e,
                EngineEvent::FilesChanged {
                    count: 3,
                    auto_reload_triggered: false
                }
            )),
            "FilesChanged should be emitted with the watcher's count, got {:?}",
            events
        );
    }

    #[tokio::test]
    async fn test_files_changed_event_emitted_on_auto_reload_trigger() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(dir.path().to_path_buf());

        let mut rx = engine.subscribe();
        engine.process_message(Message::AutoReloadTriggered);

        let events = drain_events(&mut rx);
        assert!(
            events.iter().any(|e| matches!(
                e,
                EngineEvent::FilesChanged {
                    auto_reload_triggered: true,
                    ..
                }
            )),
            "FilesChanged should be emitted when the watcher triggers auto-reload, got {:?}",
            events
        );
    }

    // ─────────────────────────────────────────────────────────
    // Watcher settings pass-through tests (Task 02)
    // ─────────────────────────────────────────────────────────

    /// `Engine::new()` uses default settings when no config file is present.
    /// Default watcher paths should be `["lib"]` and extensions `["dart"]`.
    #[tokio::test]
    async fn test_engine_default_watcher_settings() {
        let dir = tempfile::tempdir().unwrap();
        let engine = Engine::new(dir.path().to_path_buf());

        assert_eq!(engine.settings.watcher.paths, vec!["lib".to_string()]);
        assert_eq!(engine.settings.watcher.extensions, vec!["dart".to_string()]);
        assert!(engine.settings.watcher.auto_reload);
    }

    /// `WatcherConfig` constructed from settings correctly maps custom paths.
    /// Mirrors the logic in `start_file_watcher` so we can verify it without
    /// accessing the private `file_watcher` field.
    #[test]
    fn test_watcher_config_from_settings_custom_paths() {
        use crate::config::Settings;
        use crate::watcher::WatcherConfig;

        let mut settings = Settings::default();
        settings.watcher.paths = vec!["lib".to_string(), "../shared/lib".to_string()];

        let config = WatcherConfig::new()
            .with_paths(settings.watcher.paths.iter().map(PathBuf::from).collect())
            .with_extensions(settings.watcher.extensions.clone())
            .with_debounce_ms(settings.watcher.debounce_ms)
            .with_auto_reload(settings.watcher.auto_reload);

        assert_eq!(
            config.paths,
            vec![PathBuf::from("lib"), PathBuf::from("../shared/lib")]
        );
    }

    /// `WatcherConfig` constructed from settings correctly maps custom extensions.
    #[test]
    fn test_watcher_config_from_settings_custom_extensions() {
        use crate::config::Settings;
        use crate::watcher::WatcherConfig;

        let mut settings = Settings::default();
        settings.watcher.extensions = vec!["dart".to_string(), "yaml".to_string()];

        let config = WatcherConfig::new()
            .with_paths(settings.watcher.paths.iter().map(PathBuf::from).collect())
            .with_extensions(settings.watcher.extensions.clone())
            .with_debounce_ms(settings.watcher.debounce_ms)
            .with_auto_reload(settings.watcher.auto_reload);

        assert_eq!(
            config.extensions,
            vec!["dart".to_string(), "yaml".to_string()]
        );
    }

    /// Default `Settings` values produce a `WatcherConfig` with default paths
    /// and extensions (i.e. no custom config.toml present).
    #[test]
    fn test_watcher_config_from_default_settings() {
        use crate::config::Settings;
        use crate::watcher::WatcherConfig;

        let settings = Settings::default();

        let config = WatcherConfig::new()
            .with_paths(settings.watcher.paths.iter().map(PathBuf::from).collect())
            .with_extensions(settings.watcher.extensions.clone())
            .with_debounce_ms(settings.watcher.debounce_ms)
            .with_auto_reload(settings.watcher.auto_reload);

        // Defaults: paths=["lib"], extensions=["dart"]
        assert_eq!(config.paths, vec![PathBuf::from("lib")]);
        assert_eq!(config.extensions, vec!["dart".to_string()]);
        assert!(config.auto_reload);
    }

    // ── DevTools telemetry sync (DevToolsService) ─────────────────────────────

    fn devtools_test_device() -> fdemon_daemon::Device {
        fdemon_daemon::Device {
            id: "emulator-5554".to_string(),
            name: "Pixel 6".to_string(),
            platform: "android".to_string(),
            emulator: true,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
            is_supported: true,
            capabilities: None,
        }
    }

    #[tokio::test]
    async fn test_sync_populates_devtools_snapshots() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(dir.path().to_path_buf());

        let session_id = engine
            .state
            .session_manager
            .create_session(&devtools_test_device())
            .unwrap();
        {
            let handle = engine.state.session_manager.get_mut(session_id).unwrap();
            handle.session.vm_connected = true;
            handle.session.performance.monitoring_active = true;
            for i in 1..=5u64 {
                handle.session.performance.frame_history.push(
                    fdemon_core::performance::FrameTiming {
                        number: i,
                        build_micros: 5_000,
                        raster_micros: 5_000,
                        elapsed_micros: 10_000,
                        timestamp: chrono::Local::now(),
                        phases: None,
                        shader_compilation: false,
                    },
                );
            }
        }

        // flush_pending_logs runs the non-blocking SharedState sync.
        engine.flush_pending_logs();

        let snapshots = engine.shared_state().devtools.read().await.clone();
        assert_eq!(snapshots.len(), 1);
        let snap = &snapshots[0];
        assert_eq!(snap.session_id, session_id);
        assert!(snap.vm_connected);
        assert!(snap.perf_monitoring_active);
        assert!(!snap.network_monitoring_active);
        assert_eq!(snap.recent_frames.len(), 5);
        assert_eq!(snap.recent_frames[0].number, 1);
        assert!(snap.memory_samples.is_empty());
        assert!(snap.network_requests.is_empty());

        // The service accessor reads the same data.
        use crate::services::DevToolsService;
        let service = engine.devtools_service();
        let perf = service.performance_frames(session_id).await.unwrap();
        assert_eq!(perf.frames.len(), 5);
    }

    #[tokio::test]
    async fn test_sync_widget_tree_only_clones_on_change() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(dir.path().to_path_buf());

        let session_id = engine
            .state
            .session_manager
            .create_session(&devtools_test_device())
            .unwrap();

        // Simulate a completed inspector fetch for the selected session.
        engine.state.devtools_view_state.inspector.root = Some(fdemon_core::DiagnosticsNode {
            description: "RootWidget".to_string(),
            ..Default::default()
        });
        engine.state.devtools_view_state.inspector.loading = false;
        engine.state.devtools_view_state.inspector.last_fetch_time =
            Some(std::time::Instant::now());

        engine.flush_pending_logs();

        let slot = engine.shared_state().widget_tree.read().await.clone();
        let snap = slot.expect("widget tree must be synced for the selected session");
        assert_eq!(snap.session_id, session_id);
        let first_root = snap.root.clone().expect("root must be present");
        assert_eq!(first_root.description, "RootWidget");

        // A second sync with unchanged fetch state must reuse the same Arc
        // (no deep clone in the steady state).
        engine.flush_pending_logs();
        let slot2 = engine.shared_state().widget_tree.read().await.clone();
        let second_root = slot2.unwrap().root.unwrap();
        assert!(
            std::sync::Arc::ptr_eq(&first_root, &second_root),
            "unchanged inspector state must not re-clone the tree"
        );
    }

    #[tokio::test]
    async fn test_sync_widget_tree_cleared_without_sessions() {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(dir.path().to_path_buf());

        engine.flush_pending_logs();
        assert!(engine.shared_state().widget_tree.read().await.is_none());
    }
}
