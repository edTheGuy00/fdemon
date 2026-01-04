//! Message types for the application (TEA pattern)

use crate::app::session::SessionId;
use crate::core::DaemonEvent;
use crate::daemon::{CommandSender, Device, Emulator, EmulatorLaunchResult};
use crossterm::event::KeyEvent;

/// All possible messages/actions in the application
#[derive(Debug, Clone)]
pub enum Message {
    /// Keyboard event from terminal
    Key(KeyEvent),

    /// Event from Flutter daemon (legacy single-session mode)
    Daemon(DaemonEvent),

    /// Event from Flutter daemon with session context (multi-session mode)
    SessionDaemon {
        session_id: SessionId,
        event: DaemonEvent,
    },

    /// Tick event for periodic updates
    Tick,

    /// Request to quit (may show confirmation dialog if sessions running)
    RequestQuit,

    /// Force quit without confirmation (Ctrl+C, signal handler)
    Quit,

    /// Confirm quit from confirmation dialog
    ConfirmQuit,

    /// Cancel quit from confirmation dialog
    CancelQuit,

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
    /// Session-specific reload completed (for multi-session auto-reload)
    SessionReloadCompleted { session_id: SessionId, time_ms: u64 },
    /// Session-specific reload failed (for multi-session auto-reload)
    SessionReloadFailed {
        session_id: SessionId,
        reason: String,
    },
    /// Restart started
    RestartStarted,
    /// Restart completed
    RestartCompleted,
    /// Restart failed
    RestartFailed { reason: String },
    /// Session-specific restart completed (for multi-session mode)
    SessionRestartCompleted { session_id: SessionId },
    /// Session-specific restart failed (for multi-session mode)
    SessionRestartFailed {
        session_id: SessionId,
        reason: String,
    },

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
        session_id: SessionId,
        device_id: String,
        device_name: String,
        platform: String,
        pid: Option<u32>,
    },
    /// Session failed to spawn
    SessionSpawnFailed {
        session_id: SessionId,
        device_id: String,
        error: String,
    },
    /// Attach command sender to session (from background task)
    SessionProcessAttached {
        session_id: SessionId,
        cmd_sender: CommandSender,
    },

    // ─────────────────────────────────────────────────────────
    // Session Navigation (Task 10)
    // ─────────────────────────────────────────────────────────
    /// Select session by index (0-based, for keys 1-9)
    SelectSessionByIndex(usize),
    /// Switch to next session (Tab)
    NextSession,
    /// Switch to previous session (Shift+Tab)
    PreviousSession,
    /// Close the current session (x / Ctrl+W)
    CloseCurrentSession,

    // ─────────────────────────────────────────────────────────
    // Log Control (Task 10)
    // ─────────────────────────────────────────────────────────
    /// Clear logs for current session
    ClearLogs,
}
