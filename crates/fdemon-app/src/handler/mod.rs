//! Handler module - TEA update function and event handlers
//!
//! Organized into submodules:
//! - `update`: Main update() function and message dispatch
//! - `daemon`: Multi-session daemon event handling
//! - `dap`: DAP server lifecycle message handler
//! - `session`: Session state helpers
//! - `session_lifecycle`: Session lifecycle handlers
//! - `keys`: Key event handlers for UI modes
//! - `helpers`: Utility functions
//! - `new_session`: NewSessionDialog handlers
//! - `settings`: Settings helpers
//! - `settings_handlers`: Settings page handlers
//! - `settings_dart_defines`: Dart defines modal handlers for the settings panel
//! - `settings_extra_args`: Extra args fuzzy modal handlers for the settings panel
//! - `scroll`: Scroll handlers
//! - `log_view`: Log view operation handlers
//! - `flutter_version`: Flutter Version panel handlers

pub(crate) mod daemon;
pub(crate) mod dap;
pub(crate) mod dap_backend;
pub(crate) mod devtools;
pub(crate) mod flutter_version;
pub(crate) mod helpers;
pub(crate) mod keys;
pub(crate) mod log_view;
pub(crate) mod new_session;
pub(crate) mod scroll;
pub(crate) mod session;
pub(crate) mod session_lifecycle;
pub(crate) mod settings;
pub(crate) mod settings_dart_defines;
pub(crate) mod settings_extra_args;
pub(crate) mod settings_handlers;
pub(crate) mod update;

#[cfg(test)]
mod tests;

use crate::config::{LaunchConfig, LoadedConfigs};
use crate::message::Message;
use crate::session::SessionId;
use fdemon_daemon::{Device, FlutterExecutable};

// Re-export main entry point
pub use update::update;

// Re-export functions used by internal tests
#[cfg(test)]
pub(crate) use helpers::detect_raw_line_level;
#[cfg(test)]
pub(crate) use keys::handle_key;

/// Actions that the event loop should perform after update
#[derive(Debug, Clone)]
pub enum UpdateAction {
    /// Spawn a background task
    SpawnTask(Task),

    /// Discover available devices
    DiscoverDevices {
        /// Flutter executable to use for device discovery.
        flutter: FlutterExecutable,
    },

    /// Refresh devices in background (no loading spinner)
    /// Used when cache is fresh but we want to update in background
    RefreshDevicesBackground {
        /// Flutter executable to use for device discovery.
        flutter: FlutterExecutable,
    },

    /// Discover devices and auto-launch a session
    /// Used when auto_start=true to run device discovery in background
    /// and automatically launch with the best available config/device
    DiscoverDevicesAndAutoLaunch {
        /// Pre-loaded configs for selection logic
        configs: LoadedConfigs,
        /// Flutter executable to use for device discovery.
        flutter: FlutterExecutable,
    },

    /// Discover available emulators
    DiscoverEmulators {
        /// Flutter executable to use for emulator discovery.
        flutter: FlutterExecutable,
    },

    /// Launch an emulator by ID
    LaunchEmulator {
        emulator_id: String,
        /// Flutter executable to use for emulator launch.
        flutter: FlutterExecutable,
    },

    /// Launch iOS Simulator (macOS shortcut)
    LaunchIOSSimulator,

    /// Spawn a new session for a device
    SpawnSession {
        /// The session ID in SessionManager (already created)
        session_id: SessionId,
        /// The device to run on
        device: Device,
        /// Optional launch configuration
        config: Option<Box<LaunchConfig>>,
        /// Flutter executable to use for spawning the process.
        flutter: FlutterExecutable,
    },

    /// Reload all running sessions (file watcher auto-reload)
    /// Contains list of (session_id, app_id) pairs to reload
    ReloadAllSessions { sessions: Vec<(SessionId, String)> },

    /// Check tool availability (runs at startup)
    CheckToolAvailability,

    /// Discover bootable devices (iOS simulators + Android AVDs)
    DiscoverBootableDevices,

    /// Boot a specific device
    BootDevice {
        device_id: String,
        platform: fdemon_core::Platform,
    },

    /// Auto-save FDemon config after field changes (Phase 6, Task 05)
    AutoSaveConfig { configs: LoadedConfigs },

    /// Launch a new Flutter session from NewSessionDialog (Phase 6, Task 05)
    LaunchFlutterSession {
        device: Device,
        mode: crate::config::FlutterMode,
        flavor: Option<String>,
        dart_defines: Vec<String>,
        config_name: Option<String>,
    },

    /// Discover entry points in background (Phase 3, Task 09)
    DiscoverEntryPoints { project_path: std::path::PathBuf },

    /// Connect to the VM Service WebSocket for a session
    ConnectVmService {
        session_id: SessionId,
        ws_uri: String,
    },

    /// Start periodic performance monitoring for a session.
    ///
    /// Spawns a background polling task that fetches memory usage at a
    /// configured interval (default 2 seconds) and sends
    /// `VmServiceMemorySnapshot` and `VmServiceMemorySample` messages to
    /// the TEA loop. Also periodically calls `getAllocationProfile` at a
    /// lower frequency and sends `VmServiceAllocationProfileReceived`.
    ///
    /// The `handle` field is `None` when returned by `handler::update()` and
    /// hydrated by `process.rs` with the `VmRequestHandle` from the session
    /// before the action is dispatched to `handle_action`. If the session has
    /// no active VM connection at dispatch time the action is discarded.
    StartPerformanceMonitoring {
        session_id: SessionId,
        /// VM Service request handle used by the polling task.
        /// `None` until hydrated by `process.rs` from the session's
        /// `vm_request_handle`. `handle_action` can safely `.unwrap()` this
        /// because `process.rs` discards actions where it remains `None`.
        handle: Option<fdemon_daemon::vm_service::VmRequestHandle>,
        /// Memory polling interval in milliseconds (from `settings.devtools.performance_refresh_ms`).
        /// Clamped to a minimum of 500ms to prevent excessive polling.
        performance_refresh_ms: u64,
        /// Allocation profile polling interval in milliseconds (from `settings.devtools.allocation_profile_interval_ms`).
        /// Clamped to a minimum of 1000ms. `getAllocationProfile` is expensive
        /// (walks the entire Dart heap), so a lower frequency than memory polling is used.
        allocation_profile_interval_ms: u64,
    },

    /// Fetch the widget tree from the VM Service for the Inspector panel.
    ///
    /// `vm_handle` is `None` until hydrated by `process.rs` from the session's
    /// `vm_request_handle`. `handle_action` silently skips the action when it
    /// remains `None` (VM not yet connected).
    FetchWidgetTree {
        session_id: SessionId,
        /// VM Service request handle used for the RPC call.
        vm_handle: Option<fdemon_daemon::vm_service::VmRequestHandle>,
        /// Max depth for widget tree fetch (0 = unlimited).
        /// From `settings.devtools.tree_max_depth`.
        tree_max_depth: u32,
        /// Overall timeout for the fetch operation including readiness polling
        /// and retries. From `settings.devtools.inspector_fetch_timeout_secs`.
        fetch_timeout_secs: u64,
    },

    /// Fetch layout data for a specific widget node.
    ///
    /// `vm_handle` is `None` until hydrated by `process.rs` from the session's
    /// `vm_request_handle`. `handle_action` silently skips the action when it
    /// remains `None` (VM not yet connected).
    FetchLayoutData {
        session_id: SessionId,
        node_id: String,
        /// VM Service request handle used for the RPC call.
        vm_handle: Option<fdemon_daemon::vm_service::VmRequestHandle>,
    },

    /// Toggle a debug overlay via VM Service extension call.
    ///
    /// `vm_handle` is `None` until hydrated by `process.rs` from the session's
    /// `vm_request_handle`. `handle_action` silently skips the action when it
    /// remains `None` (VM not yet connected).
    ToggleOverlay {
        session_id: SessionId,
        extension: crate::message::DebugOverlayKind,
        /// VM Service request handle used for the RPC call.
        vm_handle: Option<fdemon_daemon::vm_service::VmRequestHandle>,
    },

    /// Open the Flutter DevTools URL in the system browser.
    ///
    /// Fire-and-forget OS call — no VM Service handle needed.
    /// If `browser` is empty, the platform default opener is used.
    OpenBrowserDevTools { url: String, browser: String },

    /// Start the network monitoring polling task.
    ///
    /// Spawns a background task that polls `ext.dart.io.getHttpProfile` at
    /// the given interval and sends `VmServiceHttpProfileReceived` messages.
    ///
    /// `handle` is `None` until hydrated by `process.rs` from the session's
    /// `vm_request_handle`. `handle_action` discards the action when `handle`
    /// remains `None` (VM not yet connected).
    StartNetworkMonitoring {
        session_id: SessionId,
        /// VM Service request handle used by the polling task.
        /// `None` until hydrated by `process.rs`.
        handle: Option<fdemon_daemon::vm_service::VmRequestHandle>,
        /// Polling interval in milliseconds.
        poll_interval_ms: u64,
    },

    /// Fetch full detail for a specific HTTP request.
    ///
    /// Issues a `ext.dart.io.getHttpProfileRequest` call and sends
    /// `VmServiceHttpRequestDetailReceived` or `VmServiceHttpRequestDetailFailed`.
    ///
    /// `vm_handle` is `None` until hydrated by `process.rs`.
    FetchHttpRequestDetail {
        session_id: SessionId,
        /// The unique ID of the HTTP request to fetch detail for.
        request_id: String,
        /// VM Service request handle used for the RPC call.
        /// `None` until hydrated by `process.rs`.
        vm_handle: Option<fdemon_daemon::vm_service::VmRequestHandle>,
    },

    /// Clear the HTTP profile on the VM.
    ///
    /// Issues a `ext.dart.io.clearHttpProfile` call to reset the VM's
    /// request history. The local `NetworkState` is cleared immediately
    /// by the handler; this action clears the VM side.
    ///
    /// `vm_handle` is `None` until hydrated by `process.rs`.
    ClearHttpProfile {
        session_id: SessionId,
        /// VM Service request handle used for the RPC call.
        /// `None` until hydrated by `process.rs`.
        vm_handle: Option<fdemon_daemon::vm_service::VmRequestHandle>,
    },

    /// Dispose both DevTools VM object groups when exiting DevTools mode.
    ///
    /// Disposes `"fdemon-inspector-1"` and `"devtools-layout"` groups to
    /// release VM references held by the Flutter inspector. This prevents
    /// memory accumulation on the Flutter VM side during long debug sessions.
    ///
    /// `vm_handle` is `None` until hydrated by `process.rs` from the session's
    /// `vm_request_handle`. `handle_action` silently skips the action when it
    /// remains `None` (VM not connected).
    ///
    /// Disposal failures are logged at debug level and do not block the exit.
    DisposeDevToolsGroups {
        session_id: SessionId,
        /// VM Service request handle used for the RPC calls.
        /// `None` until hydrated by `process.rs`.
        vm_handle: Option<fdemon_daemon::vm_service::VmRequestHandle>,
    },

    // --- Debug Actions (DAP Server Phase 1, Task 05) ---
    /// Pause an isolate in the VM.
    ///
    /// `vm_handle` is `None` until hydrated by `process.rs` from the session's
    /// `vm_request_handle`. `handle_action` silently skips the action when it
    /// remains `None` (VM not connected).
    PauseIsolate {
        session_id: SessionId,
        /// VM Service request handle used for the RPC call.
        /// `None` until hydrated by `process.rs`.
        vm_handle: Option<fdemon_daemon::vm_service::VmRequestHandle>,
        /// The isolate ID to pause (e.g. `"isolates/1234"`).
        isolate_id: String,
    },

    /// Resume an isolate, optionally with a step action.
    ///
    /// `vm_handle` is `None` until hydrated by `process.rs` from the session's
    /// `vm_request_handle`. `handle_action` silently skips the action when it
    /// remains `None` (VM not connected).
    ResumeIsolate {
        session_id: SessionId,
        /// VM Service request handle used for the RPC call.
        /// `None` until hydrated by `process.rs`.
        vm_handle: Option<fdemon_daemon::vm_service::VmRequestHandle>,
        /// The isolate ID to resume (e.g. `"isolates/1234"`).
        isolate_id: String,
        /// Optional step action. `None` resumes normally; `Some` performs a step.
        step: Option<fdemon_daemon::vm_service::debugger_types::StepOption>,
    },

    /// Set a breakpoint via URI and line number.
    ///
    /// `vm_handle` is `None` until hydrated by `process.rs` from the session's
    /// `vm_request_handle`. `handle_action` silently skips the action when it
    /// remains `None` (VM not connected).
    AddBreakpoint {
        session_id: SessionId,
        /// VM Service request handle used for the RPC call.
        /// `None` until hydrated by `process.rs`.
        vm_handle: Option<fdemon_daemon::vm_service::VmRequestHandle>,
        /// The isolate ID the breakpoint belongs to.
        isolate_id: String,
        /// The script URI (e.g. `"package:app/main.dart"`).
        script_uri: String,
        /// 1-based line number in the source file.
        line: i32,
        /// Optional 1-based column number.
        column: Option<i32>,
    },

    /// Remove a breakpoint by VM Service ID.
    ///
    /// `vm_handle` is `None` until hydrated by `process.rs` from the session's
    /// `vm_request_handle`. `handle_action` silently skips the action when it
    /// remains `None` (VM not connected).
    RemoveBreakpoint {
        session_id: SessionId,
        /// VM Service request handle used for the RPC call.
        /// `None` until hydrated by `process.rs`.
        vm_handle: Option<fdemon_daemon::vm_service::VmRequestHandle>,
        /// The isolate ID the breakpoint belongs to.
        isolate_id: String,
        /// The VM Service breakpoint ID to remove (e.g. `"breakpoints/1"`).
        breakpoint_id: String,
    },

    /// Set the exception pause mode for an isolate.
    ///
    /// `vm_handle` is `None` until hydrated by `process.rs` from the session's
    /// `vm_request_handle`. `handle_action` silently skips the action when it
    /// remains `None` (VM not connected).
    SetIsolatePauseMode {
        session_id: SessionId,
        /// VM Service request handle used for the RPC call.
        /// `None` until hydrated by `process.rs`.
        vm_handle: Option<fdemon_daemon::vm_service::VmRequestHandle>,
        /// The isolate ID to configure.
        isolate_id: String,
        /// The new exception pause mode.
        mode: fdemon_daemon::vm_service::debugger_types::ExceptionPauseMode,
    },

    // --- DAP Server Actions (DAP Server Phase 2, Task 03) ---
    /// Spawn the DAP TCP server as a background task.
    ///
    /// Handled by the TUI/headless runner event loops (not by `actions/mod.rs`),
    /// because the DAP server is an Engine-level service (like the file watcher),
    /// not a session-scoped action.
    SpawnDapServer {
        /// The TCP port to bind the DAP server on.
        port: u16,
        /// The bind address (e.g. `"127.0.0.1"` or `"0.0.0.0"`).
        bind_addr: String,
    },

    /// Stop the running DAP server and disconnect all clients.
    ///
    /// Handled by the TUI/headless runner event loops (not by `actions/mod.rs`).
    StopDapServer,

    /// Forward translated VM debug events to all connected DAP adapters.
    ///
    /// Produced by `handle_debug_event` and `handle_isolate_event` after
    /// updating per-session `DebugState`. The actual `try_send` calls happen
    /// in `actions::handle_action`, outside the synchronous TEA update cycle,
    /// which preserves TEA purity (no blocking mutex / channel ops in `update()`).
    ///
    /// Stale senders (where the DAP client has disconnected) are pruned
    /// automatically inside `handle_action` via the `retain` + `try_send` pattern.
    ForwardDapDebugEvents(Vec<fdemon_dap::adapter::DebugEvent>),

    /// Generate IDE-specific DAP config file (Phase 5, Task 03).
    ///
    /// Triggers the IDE config generation task that inspects the detected
    /// `ParentIde` and writes the appropriate config (launch.json,
    /// languages.toml, .emacs.d/fdemon-dap.el, etc.).
    ///
    /// `ide_override` allows the `--dap-config <IDE>` CLI flag to bypass
    /// auto-detection and target a specific IDE explicitly (Phase 5, Task 10).
    ///
    /// Handled by the TUI/headless runner event loops.
    GenerateIdeConfig {
        port: u16,
        /// Optional IDE override from `--dap-config` CLI flag.
        /// When `None`, the IDE is auto-detected from the environment.
        ide_override: Option<crate::config::ParentIde>,
    },

    /// Start native platform log capture for a session (after AppStarted).
    ///
    /// Dispatched by the TEA handler when a `DaemonMessage::AppStart` is
    /// received for an Android, macOS, or iOS session. The `platform` field
    /// determines which capture backend is used. Linux/Windows/Web sessions
    /// are silently ignored by the action dispatcher.
    StartNativeLogCapture {
        /// The session to capture logs for.
        session_id: SessionId,
        /// Platform string (e.g., `"android"`, `"macos"`, `"ios"`).
        platform: String,
        /// ADB device serial (Android) or device UDID (iOS).
        device_id: String,
        /// Human-readable device name (e.g., `"iPhone 15 Simulator"`, `"Ed's iPhone"`).
        /// Used for iOS simulator detection: simulator device names contain "Simulator".
        device_name: String,
        /// Flutter app ID (package name / bundle ID) from the `app.start` event.
        app_id: Option<String>,
        /// Native log settings snapshot (captured at action creation time so
        /// the action dispatcher does not need access to AppState).
        settings: crate::config::NativeLogsSettings,
        /// Flutter project directory — used as the default working directory
        /// for custom log source processes when `working_dir` is not specified.
        project_path: std::path::PathBuf,
        /// Names of custom sources that are already running for this session.
        ///
        /// Captured at action creation time from `SessionHandle::custom_source_handles`.
        /// Passed to `spawn_custom_sources()` so it can skip sources that were
        /// already started as pre-app sources, preventing double-spawning.
        running_source_names: Vec<String>,
        /// Names of shared custom sources that are already running globally.
        ///
        /// Captured at action creation time from `state.running_shared_source_names()`.
        /// Passed to `spawn_custom_sources()` so it skips shared sources that were
        /// already spawned by a previous session's pre-app phase, preventing
        /// duplicate shared-source processes.
        running_shared_names: Vec<String>,
    },

    /// Spawn pre-app custom sources and run their readiness checks before
    /// launching the Flutter session.
    ///
    /// Dispatched by `handle_launch()` when the config has custom sources with
    /// `start_before_app = true`. On completion (all sources ready or timed out),
    /// sends `Message::PreAppSourcesReady` which triggers `SpawnSession`.
    ///
    /// (pre-app-custom-sources Phase 1, Task 03)
    SpawnPreAppSources {
        /// Session ID in `SessionManager` (already created before this action is dispatched).
        session_id: SessionId,
        /// The device to run on (passed through to `PreAppSourcesReady` → `SpawnSession`).
        device: Device,
        /// Optional launch configuration (passed through to `PreAppSourcesReady` → `SpawnSession`).
        config: Option<Box<LaunchConfig>>,
        /// Native log settings snapshot — provides `custom_sources` (filtered for
        /// `start_before_app`), `exclude_tags`, and `include_tags`.
        settings: crate::config::NativeLogsSettings,
        /// Flutter project directory — used as the default `working_dir` when
        /// constructing daemon-layer configs for each custom source.
        project_path: std::path::PathBuf,
        /// Names of shared custom sources that are already running on `AppState`.
        ///
        /// Snapshot of shared custom source names already running at the time
        /// this action was constructed, taken from `state.running_shared_source_names()`.
        /// Sources in this list are skipped by `spawn_pre_app_sources` so a shared
        /// source is never spawned twice.
        running_shared_names: Vec<String>,
    },

    // ── Flutter Version Panel ─────────────────────────────────────────────────
    /// Scan the FVM cache for installed SDK versions.
    /// Triggered when the Flutter Version panel opens.
    ScanInstalledSdks {
        /// Root path of the currently active SDK (for `is_active` marking)
        active_sdk_root: Option<std::path::PathBuf>,
    },

    /// Switch the active Flutter SDK version.
    /// Writes `.fvmrc` in the project root and re-resolves the SDK.
    SwitchFlutterVersion {
        /// Version string to switch to (e.g., "3.19.0", "stable")
        version: String,
        /// Path to the selected SDK in the FVM cache
        sdk_path: std::path::PathBuf,
        /// Project root where `.fvmrc` will be written
        project_path: std::path::PathBuf,
        /// Explicit SDK path from settings (passed to re-resolution)
        explicit_sdk_path: Option<std::path::PathBuf>,
    },

    /// Remove an installed SDK version from the FVM cache.
    RemoveFlutterVersion {
        /// Version string being removed
        version: String,
        /// Path to the SDK directory to delete
        path: std::path::PathBuf,
        /// Root of the currently active SDK (to re-scan after removal)
        active_sdk_root: Option<std::path::PathBuf>,
    },
}

/// Background tasks to spawn
#[derive(Debug, Clone)]
pub enum Task {
    /// Hot reload (with session context for cmd_sender lookup)
    Reload {
        session_id: SessionId,
        app_id: String,
    },
    /// Hot restart (with session context for cmd_sender lookup)
    Restart {
        session_id: SessionId,
        app_id: String,
    },
    /// Stop the app (with session context for cmd_sender lookup)
    Stop {
        session_id: SessionId,
        app_id: String,
    },
}

/// Result of processing a message
#[derive(Debug, Default)]
pub struct UpdateResult {
    /// Optional follow-up message to process
    pub message: Option<Message>,
    /// Optional action for the event loop to perform
    pub action: Option<UpdateAction>,
}

impl UpdateResult {
    pub fn none() -> Self {
        Self::default()
    }

    pub fn message(msg: Message) -> Self {
        Self {
            message: Some(msg),
            action: None,
        }
    }

    pub fn action(action: UpdateAction) -> Self {
        Self {
            message: None,
            action: Some(action),
        }
    }

    /// Carry both a follow-up message and a side-effect action.
    ///
    /// Used when an event simultaneously triggers a state-gate message
    /// (e.g. `SuspendFileWatcher` on a DAP pause) **and** must forward
    /// translated debug events to connected DAP adapters via
    /// `ForwardDapDebugEvents`.
    pub fn message_and_action(msg: Message, action: UpdateAction) -> Self {
        Self {
            message: Some(msg),
            action: Some(action),
        }
    }
}
