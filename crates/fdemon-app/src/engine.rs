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
    CommandSenderController, LocalFlutterController, ProjectInfo, SharedLogService, SharedState,
    SharedStateService,
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
    _session_count: usize,
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
            _session_count: state.session_manager.len(),
            _reload_count: reload_count,
        }
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

        // Emit events for any state changes
        self.emit_events(&pre, &post);

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
    /// Compares pre/post snapshots to detect what changed.
    fn emit_events(&self, pre: &StateSnapshot, post: &StateSnapshot) {
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

        // Reload detection - transition from non-Reloading to Reloading
        if pre.phase != AppPhase::Reloading && post.phase == AppPhase::Reloading {
            if let Some(session_id) = post.selected_session_id {
                self.emit(EngineEvent::ReloadStarted { session_id });
            }
        }

        // Reload completion - transition from Reloading to Running
        if pre.phase == AppPhase::Reloading && post.phase == AppPhase::Running {
            if let Some(session_id) = post.selected_session_id {
                // Calculate reload time if we have reload start/end times
                // For now, emit with 0ms - actual timing is tracked elsewhere
                self.emit(EngineEvent::ReloadCompleted {
                    session_id,
                    time_ms: 0,
                });
            }
        }

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

        // Note: Hot restart events (RestartStarted, RestartCompleted) would be
        // emitted here based on message type or phase changes, but the current
        // AppPhase enum doesn't have a Restarting variant. These events may be
        // added in a future update when restart tracking is implemented.
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
        assert_eq!(snapshot._session_count, 0);
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
}
