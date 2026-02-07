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

// Handler submodule functions are dispatched through update() but the compiler
// cannot trace pub(crate) cross-module usage, resulting in false dead_code warnings.
#[allow(dead_code)]
pub(crate) mod daemon;
#[allow(dead_code)]
pub(crate) mod helpers;
#[allow(dead_code)]
pub(crate) mod keys;
#[allow(dead_code)]
pub(crate) mod log_view;
#[allow(dead_code)]
pub(crate) mod new_session;
#[allow(dead_code)]
pub(crate) mod scroll;
#[allow(dead_code)]
pub(crate) mod session;
#[allow(dead_code)]
pub(crate) mod session_lifecycle;
#[allow(dead_code)]
pub(crate) mod settings;
#[allow(dead_code)]
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
#[allow(unused_imports)]
pub(crate) use helpers::detect_raw_line_level;
#[allow(unused_imports)]
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
