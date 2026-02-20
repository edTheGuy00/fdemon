//! Handler module - TEA update function and event handlers
//!
//! Organized into submodules:
//! - `update`: Main update() function and message dispatch
//! - `daemon`: Multi-session daemon event handling
//! - `session`: Session state helpers
//! - `session_lifecycle`: Session lifecycle handlers
//! - `keys`: Key event handlers for UI modes
//! - `helpers`: Utility functions
//! - `new_session`: NewSessionDialog handlers
//! - `settings`: Settings helpers
//! - `settings_handlers`: Settings page handlers
//! - `scroll`: Scroll handlers
//! - `log_view`: Log view operation handlers

pub(crate) mod daemon;
pub(crate) mod devtools;
pub(crate) mod helpers;
pub(crate) mod keys;
pub(crate) mod log_view;
pub(crate) mod new_session;
pub(crate) mod scroll;
pub(crate) mod session;
pub(crate) mod session_lifecycle;
pub(crate) mod settings;
pub(crate) mod settings_handlers;
pub(crate) mod update;

#[cfg(test)]
mod tests;

use crate::config::{LaunchConfig, LoadedConfigs};
use crate::message::Message;
use crate::session::SessionId;
use fdemon_daemon::Device;

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
    DiscoverDevices,

    /// Refresh devices in background (no loading spinner)
    /// Used when cache is fresh but we want to update in background
    RefreshDevicesBackground,

    /// Discover devices and auto-launch a session
    /// Used when auto_start=true to run device discovery in background
    /// and automatically launch with the best available config/device
    DiscoverDevicesAndAutoLaunch {
        /// Pre-loaded configs for selection logic
        configs: LoadedConfigs,
    },

    /// Discover available emulators
    DiscoverEmulators,

    /// Launch an emulator by ID
    LaunchEmulator { emulator_id: String },

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
}
