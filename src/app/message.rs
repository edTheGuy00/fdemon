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
    // Session Reload/Restart Completion (multi-session mode)
    // ─────────────────────────────────────────────────────────
    /// Session-specific reload completed
    SessionReloadCompleted { session_id: SessionId, time_ms: u64 },
    /// Session-specific reload failed
    SessionReloadFailed {
        session_id: SessionId,
        reason: String,
    },
    /// Session-specific restart completed
    SessionRestartCompleted { session_id: SessionId },
    /// Session-specific restart failed
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

    // ─────────────────────────────────────────────────────────
    // Log Filter Messages (Phase 1)
    // ─────────────────────────────────────────────────────────
    /// Cycle to next log level filter
    CycleLevelFilter,
    /// Cycle to next log source filter
    CycleSourceFilter,
    /// Reset all filters to default
    ResetFilters,

    // ─────────────────────────────────────────────────────────
    // Log Search Messages (Phase 1)
    // ─────────────────────────────────────────────────────────
    /// Enter search mode (show search prompt)
    StartSearch,
    /// Cancel search mode (hide prompt, keep query)
    CancelSearch,
    /// Clear search completely (remove query and matches)
    ClearSearch,
    /// Update search query text
    SearchInput { text: String },
    /// Navigate to next search match
    NextSearchMatch,
    /// Navigate to previous search match
    PrevSearchMatch,
    /// Search completed with matches (internal)
    SearchCompleted {
        matches: Vec<crate::core::SearchMatch>,
    },

    // ─────────────────────────────────────────────────────────
    // Error Navigation Messages (Phase 1)
    // ─────────────────────────────────────────────────────────
    /// Jump to next error in log
    NextError,
    /// Jump to previous error in log
    PrevError,

    // ─────────────────────────────────────────────────────────
    // Stack Trace Collapse Messages (Phase 2 Task 6)
    // ─────────────────────────────────────────────────────────
    /// Toggle stack trace expand/collapse for entry at current position
    ToggleStackTrace,

    // ─────────────────────────────────────────────────────────
    // Horizontal Scroll Messages (Phase 2 Task 12)
    // ─────────────────────────────────────────────────────────
    /// Scroll log view left by n columns
    ScrollLeft(usize),
    /// Scroll log view right by n columns
    ScrollRight(usize),
    /// Scroll to start of line (column 0)
    ScrollToLineStart,
    /// Scroll to end of line
    ScrollToLineEnd,

    // ─────────────────────────────────────────────────────────
    // Link Highlight Mode (Phase 3.1)
    // ─────────────────────────────────────────────────────────
    /// Enter link highlight mode - scan viewport for file references
    /// and display shortcut keys (1-9, a-z) for each link
    EnterLinkMode,

    /// Exit link highlight mode - return to normal mode
    ExitLinkMode,

    /// Select a link by its shortcut key ('1'-'9' or 'a'-'z')
    /// The char identifies which link shortcut was pressed
    SelectLink(char),

    // ─────────────────────────────────────────────────────────
    // Settings Messages (Phase 4)
    // ─────────────────────────────────────────────────────────
    /// Open settings panel
    ShowSettings,

    /// Close settings panel
    HideSettings,

    /// Switch to next settings tab
    SettingsNextTab,

    /// Switch to previous settings tab
    SettingsPrevTab,

    /// Jump to specific settings tab (0-3)
    SettingsGotoTab(usize),

    /// Select next setting item
    SettingsNextItem,

    /// Select previous setting item
    SettingsPrevItem,

    /// Toggle or edit the selected setting
    SettingsToggleEdit,

    /// Save settings to disk
    SettingsSave,

    /// Reset current setting to default
    SettingsResetItem,

    // ─────────────────────────────────────────────────────────
    // Settings Editing Messages (Phase 4, Task 10)
    // ─────────────────────────────────────────────────────────
    /// Toggle boolean value
    SettingsToggleBool,

    /// Cycle enum to next value
    SettingsCycleEnumNext,

    /// Cycle enum to previous value
    SettingsCycleEnumPrev,

    /// Increment/decrement number value
    SettingsIncrement(i64),

    /// Character input for string/number editing
    SettingsCharInput(char),

    /// Backspace in edit buffer
    SettingsBackspace,

    /// Clear edit buffer (Delete key)
    SettingsClearBuffer,

    /// Commit current edit
    SettingsCommitEdit,

    /// Cancel current edit (Escape)
    SettingsCancelEdit,

    /// Remove last item from list
    SettingsRemoveListItem,

    // ─────────────────────────────────────────────────────────────
    // Settings Persistence Messages (Phase 4, Task 11)
    // ─────────────────────────────────────────────────────────────
    /// Save settings and close panel
    SettingsSaveAndClose,

    /// Force close settings panel without saving
    ForceHideSettings,

    // ─────────────────────────────────────────────────────────────
    // Startup Dialog Messages (Phase 5)
    // ─────────────────────────────────────────────────────────────
    /// Show startup dialog
    ShowStartupDialog,

    /// Hide startup dialog (cancel)
    HideStartupDialog,

    /// Navigate up in current section
    StartupDialogUp,

    /// Navigate down in current section
    StartupDialogDown,

    /// Move to next section (Tab)
    StartupDialogNextSection,

    /// Move to previous section (Shift+Tab)
    StartupDialogPrevSection,

    /// Move to next section, skipping disabled fields (Task 10b)
    StartupDialogNextSectionSkipDisabled,

    /// Move to previous section, skipping disabled fields (Task 10b)
    StartupDialogPrevSectionSkipDisabled,

    /// Select specific config by index
    StartupDialogSelectConfig(usize),

    /// Select specific device by index
    StartupDialogSelectDevice(usize),

    /// Set build mode
    StartupDialogSetMode(crate::config::FlutterMode),

    /// Character input for flavor/dart-defines
    StartupDialogCharInput(char),

    /// Backspace in input field
    StartupDialogBackspace,

    /// Clear input field
    StartupDialogClearInput,

    /// Confirm and launch session
    StartupDialogConfirm,

    /// Refresh device list
    StartupDialogRefreshDevices,

    /// Jump directly to a dialog section (1=Configs, 2=Mode, 3=Flavor, 4=DartDefines, 5=Devices)
    StartupDialogJumpToSection(crate::app::state::DialogSection),

    /// Enter edit mode for current text field (Flavor or DartDefines)
    StartupDialogEnterEdit,

    /// Exit edit mode without changing section
    StartupDialogExitEdit,

    // ─────────────────────────────────────────────────────────────
    // Launch Config Editing Messages (Phase 5, Task 07)
    // ─────────────────────────────────────────────────────────────
    /// Create a new launch configuration
    LaunchConfigCreate,

    /// Delete launch configuration at index
    LaunchConfigDelete(usize),

    /// Update a field of launch configuration
    LaunchConfigUpdate {
        config_idx: usize,
        field: String,
        value: String,
    },

    // ─────────────────────────────────────────────────────────────
    // FDemon Config Auto-save (Phase 5, Task 10c)
    // ─────────────────────────────────────────────────────────────
    /// Save FDemon config edits (flavor, dart_defines) after debounce
    SaveStartupDialogConfig,
}
