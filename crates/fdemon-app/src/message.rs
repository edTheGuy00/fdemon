//! Message types for the application (TEA pattern)

use crate::config::{FlutterMode, LaunchConfig, LoadedConfigs};
use crate::input_key::InputKey;
use crate::new_session_dialog::{DartDefine, FuzzyModalType, TargetTab};
use crate::session::SessionId;
use fdemon_core::{BootableDevice, DaemonEvent};
use fdemon_daemon::{
    AndroidAvd, CommandSender, Device, Emulator, EmulatorLaunchResult, IosSimulator,
    ToolAvailability,
};

/// Type of device discovery (Connected or Bootable)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscoveryType {
    /// Connected/running devices (from flutter devices)
    Connected,
    /// Bootable/offline devices (simulators, AVDs)
    Bootable,
}

/// Successful auto-launch discovery result
#[derive(Debug, Clone)]
pub struct AutoLaunchSuccess {
    /// Device to launch on
    pub device: Device,
    /// Optional launch config (None = bare flutter run)
    pub config: Option<LaunchConfig>,
}

/// All possible messages/actions in the application
#[derive(Debug, Clone)]
pub enum Message {
    /// Keyboard event from terminal
    Key(InputKey),

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
    /// Launch iOS simulator requested
    LaunchIOSSimulator,
    /// Device discovery completed
    DevicesDiscovered { devices: Vec<Device> },
    /// Device discovery failed
    DeviceDiscoveryFailed { error: String, is_background: bool },

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
        matches: Vec<fdemon_core::SearchMatch>,
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
    // Auto-Launch Messages (Startup Flow Consistency)
    // ─────────────────────────────────────────────────────────────
    /// Trigger auto-launch flow from Normal mode
    /// Sent by runner after first render when auto_start=true
    StartAutoLaunch {
        /// Pre-loaded configs to avoid re-loading in handler
        configs: LoadedConfigs,
    },

    /// Update loading screen message during auto-launch
    /// Sent by auto-launch task during device discovery
    AutoLaunchProgress {
        /// Message to display on loading screen
        message: String,
    },

    /// Report auto-launch result (success or failure)
    /// Sent by auto-launch task when device discovery completes
    AutoLaunchResult {
        /// Ok: device and optional config to launch with
        /// Err: error message to display in StartupDialog
        result: Result<AutoLaunchSuccess, String>,
    },

    // ─────────────────────────────────────────────────────────
    // NewSessionDialog Messages
    // ─────────────────────────────────────────────────────────
    /// Show the new session dialog
    ShowNewSessionDialog,

    /// Hide the new session dialog (cancel)
    HideNewSessionDialog,

    /// Open the new session dialog
    OpenNewSessionDialog,

    /// Close the new session dialog
    CloseNewSessionDialog,

    /// Switch focus between left (Target) and right (Launch) panes
    NewSessionDialogSwitchPane,

    /// Cancel current modal or close dialog (context-aware Escape)
    NewSessionDialogEscape,

    /// Switch between Connected and Bootable tabs (left pane)
    NewSessionDialogSwitchTab(TargetTab),

    /// Toggle between Connected and Bootable tabs
    NewSessionDialogToggleTab,

    /// Navigate up in current list/field
    NewSessionDialogUp,

    /// Navigate down in current list/field
    NewSessionDialogDown,

    /// Navigate up in device list (Target Selector)
    NewSessionDialogDeviceUp,

    /// Navigate down in device list (Target Selector)
    NewSessionDialogDeviceDown,

    /// Select current item / confirm action
    /// - On Connected device: launch session
    /// - On Bootable device: boot the device
    /// - On Config/Flavor field: open fuzzy modal
    /// - On DartDefines field: open dart defines modal
    /// - On Launch button: launch session
    NewSessionDialogConfirm,

    /// Select current device or boot device (Target Selector specific)
    NewSessionDialogDeviceSelect,

    /// Refresh device list for current tab
    NewSessionDialogRefreshDevices,

    /// Boot a specific bootable device
    NewSessionDialogBootDevice { device_id: String },

    /// Device boot started
    NewSessionDialogBootStarted { device_id: String },

    /// Device boot completed - refresh connected list
    NewSessionDialogBootCompleted { device_id: String },

    /// Device boot failed
    NewSessionDialogBootFailed { device_id: String, error: String },

    /// Device boot completed (deprecated - use NewSessionDialogBootCompleted)
    NewSessionDialogDeviceBooted { device_id: String },

    /// Set connected devices (from flutter devices discovery)
    NewSessionDialogSetConnectedDevices { devices: Vec<Device> },

    /// Connected devices received (from discovery)
    NewSessionDialogConnectedDevicesReceived(Vec<Device>),

    /// Set bootable devices (from native discovery)
    NewSessionDialogSetBootableDevices { devices: Vec<BootableDevice> },

    /// Bootable devices received (from discovery)
    NewSessionDialogBootableDevicesReceived {
        ios_simulators: Vec<IosSimulator>,
        android_avds: Vec<AndroidAvd>,
    },

    /// Device discovery failed
    NewSessionDialogDeviceDiscoveryFailed {
        error: String,
        discovery_type: DiscoveryType,
    },

    /// Set error message
    NewSessionDialogSetError { error: String },

    /// Clear error message
    NewSessionDialogClearError,

    // ─────────────────────────────────────────────────────────
    // Launch Context Messages
    // ─────────────────────────────────────────────────────────
    /// Select a configuration by index
    NewSessionDialogSelectConfig { index: Option<usize> },

    /// Set the build mode
    NewSessionDialogSetMode { mode: FlutterMode },

    /// Set the flavor string
    NewSessionDialogSetFlavor { flavor: String },

    /// Set dart defines
    NewSessionDialogSetDartDefines { defines: Vec<DartDefine> },

    // ─────────────────────────────────────────────────────────
    // Launch Context Field Navigation Messages (Phase 6, Task 05)
    // ─────────────────────────────────────────────────────────
    /// Move focus to next field in Launch Context
    NewSessionDialogFieldNext,

    /// Move focus to previous field in Launch Context
    NewSessionDialogFieldPrev,

    /// Activate current field (Enter key - opens modals or triggers launch)
    NewSessionDialogFieldActivate,

    /// Change mode to next (right arrow on mode field)
    NewSessionDialogModeNext,

    /// Change mode to previous (left arrow on mode field)
    NewSessionDialogModePrev,

    /// Config selected from fuzzy modal
    NewSessionDialogConfigSelected { config_name: String },

    /// Flavor selected from fuzzy modal
    NewSessionDialogFlavorSelected { flavor: Option<String> },

    /// Entry point selected from fuzzy modal
    NewSessionDialogEntryPointSelected { entry_point: Option<String> },

    /// Dart defines updated from modal
    NewSessionDialogDartDefinesUpdated { defines: Vec<DartDefine> },

    /// Trigger launch action
    NewSessionDialogLaunch,

    /// Config auto-save completed
    NewSessionDialogConfigSaved,

    /// Config auto-save failed
    NewSessionDialogConfigSaveFailed { error: String },

    // ─────────────────────────────────────────────────────────
    // Fuzzy Modal Messages
    // ─────────────────────────────────────────────────────────
    /// Open fuzzy search modal
    NewSessionDialogOpenFuzzyModal { modal_type: FuzzyModalType },

    /// Close fuzzy search modal (cancel)
    NewSessionDialogCloseFuzzyModal,

    /// Fuzzy modal: input character
    NewSessionDialogFuzzyInput { c: char },

    /// Fuzzy modal: backspace
    NewSessionDialogFuzzyBackspace,

    /// Fuzzy modal: navigate up
    NewSessionDialogFuzzyUp,

    /// Fuzzy modal: navigate down
    NewSessionDialogFuzzyDown,

    /// Fuzzy modal: select current item
    NewSessionDialogFuzzyConfirm,

    /// Fuzzy modal: clear query
    NewSessionDialogFuzzyClear,

    // ─────────────────────────────────────────────────────────
    // Dart Defines Modal Messages
    // ─────────────────────────────────────────────────────────
    /// Open dart defines modal
    NewSessionDialogOpenDartDefinesModal,

    /// Close dart defines modal (saves changes)
    NewSessionDialogCloseDartDefinesModal,

    /// Switch between list and edit panes
    NewSessionDialogDartDefinesSwitchPane,

    /// Navigate up in list
    NewSessionDialogDartDefinesUp,

    /// Navigate down in list
    NewSessionDialogDartDefinesDown,

    /// Confirm selection (edit item) or activate button
    NewSessionDialogDartDefinesConfirm,

    /// Move to next field in edit form
    NewSessionDialogDartDefinesNextField,

    /// Input character in active text field
    NewSessionDialogDartDefinesInput { c: char },

    /// Backspace in active text field
    NewSessionDialogDartDefinesBackspace,

    /// Save current edit
    NewSessionDialogDartDefinesSave,

    /// Delete current item
    NewSessionDialogDartDefinesDelete,

    // ─────────────────────────────────────────────────────────
    // Tool Availability & Device Discovery Messages (Phase 4, Task 05)
    // ─────────────────────────────────────────────────────────
    /// Tool availability check completed
    ToolAvailabilityChecked { availability: ToolAvailability },

    /// Request to discover bootable devices (iOS simulators + Android AVDs)
    DiscoverBootableDevices,

    /// Bootable devices discovered
    BootableDevicesDiscovered {
        ios_simulators: Vec<IosSimulator>,
        android_avds: Vec<AndroidAvd>,
    },

    /// Boot a device (simulator or AVD)
    BootDevice {
        device_id: String,
        platform: fdemon_core::Platform,
    },

    /// Device boot completed
    DeviceBootCompleted { device_id: String },

    /// Device boot failed
    DeviceBootFailed { device_id: String, error: String },

    // ─────────────────────────────────────────────────────────
    // Entry Point Discovery Messages (Phase 3, Task 09)
    // ─────────────────────────────────────────────────────────
    /// Entry point discovery completed
    EntryPointsDiscovered {
        entry_points: Vec<std::path::PathBuf>,
    },
}
