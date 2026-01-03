//! Message types for the application (TEA pattern)

use crate::core::DaemonEvent;
use crate::daemon::{Device, Emulator, EmulatorLaunchResult};
use crossterm::event::KeyEvent;

/// All possible messages/actions in the application
#[derive(Debug, Clone)]
pub enum Message {
    /// Keyboard event from terminal
    Key(KeyEvent),

    /// Event from Flutter daemon
    Daemon(DaemonEvent),

    /// Tick event for periodic updates
    Tick,

    /// Request to quit the application
    Quit,

    // ─────────────────────────────────────────────────────────
    // Scroll Messages
    // ─────────────────────────────────────────────────────────
    /// Scroll log view up one line
    ScrollUp,
    /// Scroll log view down one line
    ScrollDown,
    /// Scroll to top of log view
    ScrollToTop,
    /// Scroll to bottom of log view
    ScrollToBottom,
    /// Page up in log view
    PageUp,
    /// Page down in log view
    PageDown,

    // ─────────────────────────────────────────────────────────
    // Control Messages
    // ─────────────────────────────────────────────────────────
    /// Request hot reload
    HotReload,
    /// Request hot restart
    HotRestart,
    /// Stop the running app
    StopApp,

    // ─────────────────────────────────────────────────────────
    // Internal State Updates
    // ─────────────────────────────────────────────────────────
    /// Reload started
    ReloadStarted,
    /// Reload completed successfully
    ReloadCompleted { time_ms: u64 },
    /// Reload failed
    ReloadFailed { reason: String },
    /// Restart started
    RestartStarted,
    /// Restart completed
    RestartCompleted,
    /// Restart failed
    RestartFailed { reason: String },

    // ─────────────────────────────────────────────────────────
    // File Watcher Messages
    // ─────────────────────────────────────────────────────────
    /// Multiple files changed (debounced batch)
    FilesChanged { count: usize },
    /// Auto-reload triggered by file watcher
    AutoReloadTriggered,
    /// Watcher error occurred
    WatcherError { message: String },

    // ─────────────────────────────────────────────────────────
    // Device Selector Messages
    // ─────────────────────────────────────────────────────────
    /// Show the device selector modal
    ShowDeviceSelector,
    /// Hide the device selector modal
    HideDeviceSelector,
    /// Navigate device selector up
    DeviceSelectorUp,
    /// Navigate device selector down
    DeviceSelectorDown,
    /// Device selected from selector
    DeviceSelected { device: Device },
    /// Launch Android emulator requested
    LaunchAndroidEmulator,
    /// Launch iOS simulator requested
    LaunchIOSSimulator,
    /// Device discovery completed
    DevicesDiscovered { devices: Vec<Device> },
    /// Device discovery failed
    DeviceDiscoveryFailed { error: String },
    /// Refresh device list
    RefreshDevices,

    // ─────────────────────────────────────────────────────────
    // Emulator Messages
    // ─────────────────────────────────────────────────────────
    /// Discover available emulators
    DiscoverEmulators,
    /// Emulators discovered
    EmulatorsDiscovered { emulators: Vec<Emulator> },
    /// Emulator discovery failed
    EmulatorDiscoveryFailed { error: String },
    /// Launch a specific emulator by ID
    LaunchEmulator { emulator_id: String },
    /// Emulator launch completed
    EmulatorLaunched { result: EmulatorLaunchResult },

    // ─────────────────────────────────────────────────────────
    // Session Messages
    // ─────────────────────────────────────────────────────────
    /// Session started successfully
    SessionStarted {
        device_id: String,
        device_name: String,
        platform: String,
        pid: Option<u32>,
    },
    /// Session failed to spawn
    SessionSpawnFailed { device_id: String, error: String },
}
