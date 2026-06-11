//! Message types for the application (TEA pattern)

use crate::config::{FlutterMode, LaunchConfig, LoadedConfigs};
use crate::input_key::InputKey;
use crate::input_mouse::MouseInput;
use crate::install_wizard::{WizardOrigin, WizardStepKind};
use crate::new_session_dialog::{DartDefine, FuzzyModalType, TargetTab};
use crate::session::memory::MemorySection;
use crate::session::performance::{PerfSection, SelectionDirection, TimelineEventCursor};
use crate::session::{NetworkDetailTab, SessionId};
use crate::state::{DevToolsPanel, PerfDetailsTab};
use fdemon_core::network::{HttpProfileEntry, HttpProfileEntryDetail};
use fdemon_core::{BootableDevice, DaemonEvent, DiagnosticsNode, LayoutInfo};
use fdemon_daemon::{
    flutter_sdk::InstalledSdk, vm_service::VmRequestHandle, AndroidAvd, CommandSender, Device,
    Emulator, EmulatorLaunchResult, FlutterSdk, FlutterVersionInfo, IosSimulator, NativeLogEvent,
    ToolAvailability,
};

/// Shared, abort-able handle to a background task.
///
/// Used in `Message` variants that transfer ownership of a spawned task to the
/// session state so it can be cancelled on disconnect or session close.
type SharedTaskHandle = std::sync::Arc<std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>>;

/// The three debug overlay types that can be toggled from DevTools mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugOverlayKind {
    RepaintRainbow,
    DebugPaint,
    PerformanceOverlay,
}

/// Navigation commands for the widget inspector tree view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectorNav {
    Up,
    Down,
    Expand,
    Collapse,
}

/// Navigation actions for the network request list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkNav {
    Up,
    Down,
    PageUp,
    PageDown,
}

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

    /// Mouse event from terminal (click, release, drag, scroll).
    ///
    /// Routed to [`crate::handler::mouse::handle_mouse`] which dispatches
    /// per `UiMode` to a concrete `Message`. Mouse events are no-ops in
    /// Phase 1; later phases populate the dispatcher.
    Mouse(MouseInput),

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

    // ── Coordinated Pause / File-Watcher Gate (Phase 4, Task 03) ─────────────
    /// Suspend auto-reload while the debugger is paused.
    ///
    /// Emitted by `handle_debug_event` on any `PauseBreakpoint`, `PauseException`,
    /// `PauseInterrupted`, `PausePostRequest`, or `PauseStart` event when
    /// `settings.dap.suppress_reload_on_pause` is `true`.
    ///
    /// The update handler sets `state.file_watcher_suspended = true`.
    SuspendFileWatcher,

    /// Resume auto-reload after the debugger continues execution.
    ///
    /// Emitted by `handle_debug_event` on `Resume` events and by
    /// `handle_client_disconnected` when a DAP client disconnects while the
    /// watcher was suspended.
    ///
    /// The update handler clears `state.file_watcher_suspended` and triggers
    /// `AutoReloadTriggered` if `pending_file_changes > 0`.
    ResumeFileWatcher,

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

    /// Close the session at a specific index (middle-click on a tab).
    ///
    /// Differs from [`Message::CloseCurrentSession`] in that it operates on an
    /// arbitrary index rather than `state.session_manager.selected_id()`.
    /// Out-of-range indices are silently ignored.
    CloseSessionAt(usize),

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
    // Wrap Mode (v1-refinements Phase 1)
    // ─────────────────────────────────────────────────────────
    /// Toggle line wrap mode on/off
    ToggleWrapMode,

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
    // Background Settings Persistence Handshake
    // (devtools-inspector-parity Phase 1.5, Task 02)
    // ─────────────────────────────────────────────────────────────
    /// Confirmation that a `UpdateAction::PersistSettings` completed successfully.
    SettingsPersisted,

    /// A `UpdateAction::PersistSettings` write failed.
    /// `error` carries the formatted error string for logging/UI surfacing.
    SettingsPersistFailed { error: String },

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
        /// Whether the cached `last_device` selection (Tier 2) is allowed.
        ///
        /// When `false`, `find_auto_launch_target` skips `try_cached_selection`
        /// and falls through to Tier 3 (first config + first device) or
        /// Tier 4 (bare flutter run). Populated from
        /// `settings.behavior.auto_launch` when `StartAutoLaunch` is emitted.
        cache_allowed: bool,
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

    /// Toggle multi-launch selection of the cursor device (Connected tab).
    NewSessionDialogToggleDeviceSelection,

    /// Select all / clear all connected devices for multi-launch.
    NewSessionDialogSelectAllDevices,

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

    /// Close dart defines modal and persist changes to the launch context.
    ///
    /// Reads the current working copy from the modal, applies it to
    /// `launch_context.dart_defines`, triggers auto-save if a FDemon config
    /// is selected, then dismisses the modal.
    NewSessionDialogCloseDartDefinesModal,

    /// Cancel dart defines modal and discard all unsaved edits.
    ///
    /// Closes the modal without applying any changes to the launch context.
    /// No auto-save is triggered. Used when the user presses Esc from the
    /// List pane.
    NewSessionDialogCancelDartDefinesModal,

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
    // Version Check Messages (version-check-banner)
    // ─────────────────────────────────────────────────────────
    /// A newer fdemon release was discovered on GitHub during the startup
    /// background check. Stores the version in `AppState::startup_notice`, which is
    /// rendered either above the New Session Dialog (Startup / NewSessionDialog
    /// modes) or as a standalone top-row banner on all other screens (Normal,
    /// Loading, …). Cleared on the first keypress outside the dialog.
    NewVersionAvailable { latest: String },

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

    // ─────────────────────────────────────────────────────────
    // VM Service Messages (Phase 1 DevTools Integration)
    // ─────────────────────────────────────────────────────────
    /// VM Service task ready — attaches shutdown sender to the session handle.
    ///
    /// Sent by the `spawn_vm_service_connection` background task immediately
    /// after the WebSocket connects, before `VmServiceConnected`.
    /// The TEA update handler stores the sender so that AppStop / process-exit
    /// can signal the forwarding task to stop gracefully.
    VmServiceAttached {
        session_id: SessionId,
        /// Sender half of the `watch::channel(false)` used to signal shutdown.
        /// Wrapped in `Arc` to satisfy `Clone` bound on `Message`.
        /// Sending `true` stops the forwarding task and triggers disconnect.
        vm_shutdown_tx: std::sync::Arc<tokio::sync::watch::Sender<bool>>,
    },

    /// VM Service request handle is ready for on-demand RPC calls.
    ///
    /// Sent by `spawn_vm_service_connection` immediately after the WebSocket
    /// connects and before `VmServiceConnected`. The TEA update handler stores
    /// the handle in the session so that background tasks (memory polling, etc.)
    /// can issue RPC calls through the same connection.
    ///
    /// The handle is `Clone` (wraps an `Arc`-ed channel sender) and `Debug`
    /// (shows connection state without exposing channel internals).
    VmServiceHandleReady {
        session_id: SessionId,
        handle: VmRequestHandle,
    },

    /// VM Service WebSocket connected for a session
    VmServiceConnected { session_id: SessionId },

    /// VM Service WebSocket successfully reconnected after a brief disconnect.
    ///
    /// Unlike `VmServiceConnected`, this variant does **not** reset accumulated
    /// performance telemetry (ring buffers, stats). Stream re-subscriptions and
    /// performance monitoring are restarted because the old WebSocket connection
    /// and its Dart VM stream subscriptions are gone, but historical data is
    /// preserved so the UI shows continuous history across the reconnect.
    VmServiceReconnected { session_id: SessionId },

    /// VM Service connection failed
    VmServiceConnectionFailed {
        session_id: SessionId,
        error: String,
    },

    /// VM Service disconnected (unexpected or graceful)
    VmServiceDisconnected { session_id: SessionId },

    /// VM Service connection lost and is being retried.
    ///
    /// Emitted during the reconnection backoff loop so the TUI can display
    /// a "Reconnecting (attempt/max)" indicator. Sent by the action layer
    /// when it detects a disconnection and begins retry logic.
    VmServiceReconnecting {
        session_id: SessionId,
        /// Current attempt number (1-based).
        attempt: u32,
        /// Maximum number of retry attempts before giving up.
        max_attempts: u32,
    },

    /// VM Service received a Flutter.Error event (crash log)
    VmServiceFlutterError {
        session_id: SessionId,
        log_entry: fdemon_core::LogEntry,
    },

    /// VM Service received a log record from Logging stream
    VmServiceLogRecord {
        session_id: SessionId,
        log_entry: fdemon_core::LogEntry,
    },

    // ─────────────────────────────────────────────────────────
    // VM Service Performance Messages (Phase 3, Task 05)
    // ─────────────────────────────────────────────────────────
    /// Memory usage snapshot received from periodic polling.
    VmServiceMemorySnapshot {
        session_id: SessionId,
        memory: fdemon_core::performance::MemoryUsage,
    },

    /// GC event received from the GC stream.
    VmServiceGcEvent {
        session_id: SessionId,
        gc_event: fdemon_core::performance::GcEvent,
    },

    /// Performance monitoring task started for a session.
    ///
    /// Carries the shutdown sender and the task's JoinHandle so the TEA layer
    /// can store them in the session handle, signal the polling task to stop
    /// when needed, and abort it if signalling is not sufficient.
    VmServicePerformanceMonitoringStarted {
        session_id: SessionId,
        /// Shutdown sender for the performance polling task.
        /// Wrapped in `Arc` to satisfy the `Clone` bound on `Message`.
        /// Sending `true` stops the polling loop cleanly.
        perf_shutdown_tx: std::sync::Arc<tokio::sync::watch::Sender<bool>>,
        /// JoinHandle for the performance polling task.
        /// Wrapped in `Arc<Mutex<Option<>>>` to satisfy the `Clone` bound on
        /// `Message`. The handler takes the handle out of the `Option` when
        /// storing it on `SessionHandle`, leaving `None` for any subsequent
        /// (unexpected) clone.
        perf_task_handle: SharedTaskHandle,
        /// Pause sender for the `getAllocationProfile` polling arm.
        ///
        /// Sending `true` pauses allocation polling (Performance panel not visible).
        /// Sending `false` unpauses it (Performance panel is visible).
        ///
        /// Initial channel value is `true` (paused) — allocation polling starts
        /// paused because performance monitoring begins at VM connect time, often
        /// before the user opens the Performance panel. The handler sends `false`
        /// when the user enters the Performance panel.
        alloc_pause_tx: std::sync::Arc<tokio::sync::watch::Sender<bool>>,
        /// Higher-level pause sender for the entire performance polling loop.
        ///
        /// Sending `true` pauses both memory and allocation polling (user not in
        /// DevTools mode). Sending `false` unpauses both (user entered DevTools).
        ///
        /// Initial channel value is `true` (paused) — monitoring starts at VM
        /// connect time, before the user opens DevTools. This prevents all
        /// `getMemoryUsage` and `getIsolate` RPCs while viewing logs.
        ///
        /// The `alloc_tick` arm checks both `perf_pause_rx` and `alloc_pause_rx`;
        /// the `memory_tick` arm checks only `perf_pause_rx`.
        perf_pause_tx: std::sync::Arc<tokio::sync::watch::Sender<bool>>,
    },

    // ─────────────────────────────────────────────────────────
    // VM Service Frame Timing Messages (Phase 3, Task 06)
    // ─────────────────────────────────────────────────────────
    /// Frame timing data received from a `Flutter.Frame` Extension event.
    ///
    /// Posted by Flutter on the Extension stream (already subscribed) whenever
    /// a frame is rendered. Carries build and raster durations for FPS/jank
    /// calculation. Pushed into `PerformanceState::frame_history`.
    VmServiceFrameTiming {
        session_id: SessionId,
        timing: fdemon_core::performance::FrameTiming,
    },

    // ─────────────────────────────────────────────────────────────────────────
    // VM Service Performance Messages — Phase 3 extensions (Task 02)
    // ─────────────────────────────────────────────────────────────────────────
    /// User selected or deselected a frame in the performance bar chart.
    ///
    /// `index: None` clears the selection (equivalent to pressing Esc in the
    /// frame bar chart). `index: Some(i)` highlights frame `i` in
    /// `PerformanceState::frame_history` and shows the detail panel.
    SelectPerformanceFrame {
        /// Index into `PerformanceState::frame_history`, or `None` to deselect.
        index: Option<usize>,
    },

    /// Rich memory sample received from VM service (for time-series chart).
    ///
    /// Pushed into `PerformanceState::memory_samples` by the handler.
    /// Contains a full per-category breakdown (Dart heap, native, raster cache, RSS)
    /// at 500ms polling resolution — richer than `VmServiceMemorySnapshot`.
    VmServiceMemorySample {
        session_id: SessionId,
        sample: fdemon_core::performance::MemorySample,
    },

    /// Allocation profile snapshot received from VM service.
    ///
    /// Replaces `PerformanceState::allocation_profile` with the new snapshot.
    /// Fetched on-demand or periodically, not streamed. Only the most recent
    /// profile is retained in state.
    VmServiceAllocationProfileReceived {
        session_id: SessionId,
        profile: fdemon_core::performance::AllocationProfile,
    },

    // ── DevTools Mode (Phase 4) ──────────────────────────────────────────────
    /// Enter DevTools mode (from Normal mode via 'd' key).
    EnterDevToolsMode,

    /// Escape key pressed while in DevTools mode. The handler routes this
    /// through [`handle_devtools_escape`]:
    /// - Inspector tab + details open → close details, stay in DevTools.
    /// - Otherwise → exit DevTools back to Logs.
    DevToolsEscape,

    /// Switch to a specific DevTools sub-panel.
    SwitchDevToolsPanel(DevToolsPanel),

    /// Open Flutter DevTools in the system browser.
    OpenBrowserDevTools,

    /// DevTools server is ready for the given session.
    ///
    /// Populated from the `app.devTools` daemon event (primary path) or from a
    /// `devtools.serve` RPC response (fallback). The `base_url` is the raw
    /// DevTools server URL without any `?uri=` query parameter.
    ///
    /// The handler stores a [`crate::session::DevToolsEndpoint`] on the session
    /// and clears `devtools_serve_pending`.
    DevToolsServed {
        session_id: SessionId,
        /// Base DevTools server URL (e.g. `http://127.0.0.1:9100` or
        /// `http://127.0.0.1:59123/<auth-token>/devtools`).
        base_url: String,
    },

    /// DevTools server could not be started for the given session.
    ///
    /// Emitted when the `devtools.serve` RPC returns an error or null host/port,
    /// or when the daemon reports that DevTools is unavailable.
    /// The handler clears `devtools_serve_pending` and may show a toast.
    DevToolsServeFailed {
        session_id: SessionId,
        /// Human-readable reason for the failure (e.g. "Method not supported on
        /// this Flutter SDK — update Flutter to ≥ 1.22 or run `dart devtools`
        /// manually").
        reason: String,
    },

    /// Internal: dispatch the `devtools.serve` fallback RPC if the session
    /// still needs it (idempotent — no-op when an endpoint is already set or
    /// a previous dispatch is in flight).
    ///
    /// Emitted as a follow-up from `VmServiceConnected` so the fallback can
    /// fire alongside `StartPerformanceMonitoring`, which would otherwise
    /// monopolise the single action slot when the user is already in
    /// DevTools mode at VM-connection time. The `continuation` field chains
    /// the original `VmServiceConnected` follow-up (widget-tree fetch,
    /// auto-overlay) so it is not lost.
    TriggerDevToolsServeFallback {
        session_id: SessionId,
        continuation: Option<Box<Message>>,
    },

    /// Request a widget tree refresh from the VM Service.
    RequestWidgetTree { session_id: SessionId },

    /// Widget tree data received from VM Service RPC.
    WidgetTreeFetched {
        session_id: SessionId,
        root: Box<DiagnosticsNode>,
    },

    /// Widget tree fetch failed.
    WidgetTreeFetchFailed {
        session_id: SessionId,
        error: String,
    },

    /// Widget tree fetch timed out (10-second deadline exceeded).
    ///
    /// Sent by `spawn_fetch_widget_tree` when `tokio::time::timeout` fires.
    /// The handler sets `inspector.loading = false` and stores an error message
    /// with a retry hint so the user can press `r` to try again.
    WidgetTreeFetchTimeout { session_id: SessionId },

    /// Request layout data for a specific widget node.
    RequestLayoutData {
        session_id: SessionId,
        node_id: String,
    },

    /// Layout data received from VM Service RPC.
    LayoutDataFetched {
        session_id: SessionId,
        /// The node id that was fetched. Used by the stale-guard in
        /// `handle_layout_data_fetched` to cross-check against
        /// `details_node_id` (Phase 2 follow-up M2).
        node_id: String,
        layout: Box<LayoutInfo>,
    },

    /// Layout data fetch failed.
    LayoutDataFetchFailed {
        session_id: SessionId,
        error: String,
    },

    /// Layout data fetch timed out (10-second deadline exceeded).
    ///
    /// Sent by `spawn_fetch_layout_data` when `tokio::time::timeout` fires.
    /// The handler sets `inspector.layout_loading = false` and stores an error
    /// message with a retry hint.
    LayoutDataFetchTimeout { session_id: SessionId },

    /// `ext.flutter.inspector.getProperties` succeeded.
    ///
    /// `widget_properties` is the partition with `propertyType != "RenderObject"`;
    /// `render_properties` contains the render-object nodes plus (already merged
    /// in by the spawn task) the sub-properties of each render object.
    DevToolsInspectorPropertiesFetched {
        session_id: SessionId,
        node_id: String,
        widget_properties: Vec<DiagnosticsNode>,
        render_properties: Vec<DiagnosticsNode>,
    },

    /// `getProperties` returned an error or the response failed to parse.
    DevToolsInspectorPropertiesFetchFailed {
        session_id: SessionId,
        node_id: String,
        error: String,
    },

    /// `getProperties` exceeded its 10-second timeout.
    DevToolsInspectorPropertiesFetchTimeout {
        session_id: SessionId,
        node_id: String,
    },

    /// Toggle a debug overlay extension (repaint rainbow, debug paint, perf overlay).
    ToggleDebugOverlay { extension: DebugOverlayKind },

    /// Debug overlay toggle result.
    DebugOverlayToggled {
        extension: DebugOverlayKind,
        enabled: bool,
    },

    /// Navigate within the widget inspector tree.
    DevToolsInspectorNavigate(InspectorNav),

    // ─────────────────────────────────────────────────────────────────────────
    // VM Service Debug Messages (DAP Server Phase 1, Task 05)
    // ─────────────────────────────────────────────────────────────────────────
    /// A debug stream event from the VM Service (breakpoints, pause, resume, etc.).
    ///
    /// Sent by the event forwarding loop when a "Debug" stream notification
    /// arrives. The handler updates per-session `DebugState`.
    VmServiceDebugEvent {
        session_id: SessionId,
        event: fdemon_daemon::vm_service::debugger_types::DebugEvent,
    },

    /// An isolate lifecycle event from the VM Service.
    ///
    /// Sent by the event forwarding loop when an "Isolate" stream notification
    /// arrives. The handler tracks known isolates and clears pause state on exit.
    VmServiceIsolateEvent {
        session_id: SessionId,
        event: fdemon_daemon::vm_service::debugger_types::IsolateEvent,
    },

    // ── VM Service Network Messages (Phase 4, Network Monitor) ───────────────
    /// HTTP profile poll results arrived.
    VmServiceHttpProfileReceived {
        session_id: SessionId,
        timestamp: i64,
        entries: Vec<HttpProfileEntry>,
    },

    /// Full detail for a single HTTP request arrived.
    VmServiceHttpRequestDetailReceived {
        session_id: SessionId,
        detail: Box<HttpProfileEntryDetail>,
    },

    /// Detail fetch failed.
    VmServiceHttpRequestDetailFailed {
        session_id: SessionId,
        error: String,
    },

    /// Network monitoring background task started.
    VmServiceNetworkMonitoringStarted {
        session_id: SessionId,
        network_shutdown_tx: std::sync::Arc<tokio::sync::watch::Sender<bool>>,
        network_task_handle: SharedTaskHandle,
        /// Pause sender for the network polling loop.
        ///
        /// `true` = paused (not on Network tab), `false` = active (polling).
        ///
        /// Initial value is `false` (active) — the task starts when the user is
        /// already on the Network tab, so polling should begin immediately.
        network_pause_tx: std::sync::Arc<tokio::sync::watch::Sender<bool>>,
    },

    /// Network extensions not available (e.g., release mode).
    VmServiceNetworkExtensionsUnavailable { session_id: SessionId },

    // ── Network Monitor UI Messages ───────────────────────────────────────────
    /// Navigate the network request list.
    NetworkNavigate(NetworkNav),

    /// Select a specific request by index.
    NetworkSelectRequest { index: Option<usize> },

    /// Switch detail sub-tab.
    NetworkSwitchDetailTab(NetworkDetailTab),

    /// Toggle recording on/off.
    ToggleNetworkRecording,

    /// Clear all recorded network entries.
    ClearNetworkProfile { session_id: SessionId },

    /// Update filter text.
    NetworkFilterChanged(String),

    /// Enter network filter input mode (activates text input).
    NetworkEnterFilterMode,

    /// Exit network filter input mode (cancel, discard buffer).
    NetworkExitFilterMode,

    /// Commit the filter input buffer (apply filter and exit input mode).
    NetworkCommitFilter,

    /// Append a character to the filter input buffer.
    NetworkFilterInput(char),

    /// Delete last character from filter input buffer.
    NetworkFilterBackspace,

    // ── Memory Panel UI Messages ──────────────────────────────────────────────
    /// Cycle focus within the Memory panel sections (Chart ↔ AllocationList).
    MemFocusSection(MemorySection),
    /// Scroll the focused Memory section up by one unit (one row / one sample).
    MemScrollUp,
    /// Scroll the focused Memory section down by one unit.
    MemScrollDown,
    /// Page the focused Memory section up by a viewport-height unit.
    MemPageUp,
    /// Page the focused Memory section down by a viewport-height unit.
    MemPageDown,
    /// Jump to the oldest / first item in the focused Memory section.
    MemJumpToStart,
    /// Jump to the live edge / last item in the focused Memory section.
    MemJumpToEnd,
    /// Select an allocation table row (or deselect with `None`).
    MemSelectAllocRow { index: Option<usize> },
    /// Toggle the allocation table sort column (BySize ↔ ByInstances).
    MemToggleSort,

    // ── Performance Panel UI Messages ─────────────────────────────────────────
    // --- Performance panel interactivity ---
    /// Move keyboard focus to the given sub-section within the Performance panel.
    PerfFocusSection(PerfSection),
    /// Scroll the focused Performance panel section up by one row/bar.
    PerfScrollUp,
    /// Scroll the focused Performance panel section down by one row/bar.
    PerfScrollDown,
    /// Scroll the focused Performance panel section up by one page.
    PerfPageUp,
    /// Scroll the focused Performance panel section down by one page.
    PerfPageDown,
    /// Jump to the first item in the focused Performance panel section.
    PerfJumpToStart,
    /// Jump to the last item in the focused Performance panel section.
    PerfJumpToEnd,

    // --- Performance details pane (Phase 2) ---
    /// Cycle the active tab in the Performance Details pane.
    ///
    /// Emitted by `]` (forward = true) and `[` (forward = false) when
    /// `PerformanceState::focused_section == PerfSection::Details`.
    PerfCycleDetailsTab { forward: bool },

    /// Focus a specific tab in the Performance Details pane.
    ///
    /// Phase 2 only emits this from tests; Phase 3 wires up mouse-click
    /// regions on the tab strip that emit this variant.
    PerfFocusDetailsTab(PerfDetailsTab),

    // --- Performance Phase 3: Rebuild Stats + Timeline ---
    /// A new `Flutter.RebuiltWidgets` extension event arrived.
    ///
    /// Emitted by `forward_vm_events` when it receives a stream event whose
    /// `extensionKind == "Flutter.RebuiltWidgets"`. The payload has already
    /// been parsed by `fdemon_core::rebuild_stats::parse_rebuilt_widgets_event`.
    RebuildStatsEventReceived {
        session_id: SessionId,
        payload: fdemon_core::rebuild_stats::RebuildEventPayload,
    },

    /// The user pressed `R` on the Rebuild Stats tab — toggle the extension.
    ///
    /// Triggers an async `set_profile_widget_builds` RPC and emits
    /// `RebuildStatsExtensionStateChanged` on success.
    ToggleRebuildStats { session_id: SessionId },

    /// The async toggle returned a new state — update `rebuild_stats_enabled`.
    ///
    /// When `enabled` flips to `false`, clears `rebuild_stats_totals` and
    /// `rebuild_stats_frames` and snaps the active details tab if it was
    /// on `RebuildStats`.
    RebuildStatsExtensionStateChanged {
        session_id: SessionId,
        enabled: bool,
    },

    /// The one-shot `widgetLocationIdMap` RPC returned a fresh map.
    ///
    /// Used as a fallback seed for the location map when early
    /// `Flutter.RebuiltWidgets` events were missed (location data arrives
    /// inline in those events, but the RPC covers the case where they were
    /// not observed).
    RebuildStatsLocationMapFetched {
        session_id: SessionId,
        map: fdemon_core::rebuild_stats::LocationMap,
    },

    /// The async toggle of `ext.flutter.profileWidgetBuilds` failed.
    ///
    /// Emitted by the `ToggleProfileWidgetBuilds` action when the RPC call
    /// returns an error (e.g., dying isolate during hot-restart). The handler
    /// appends a `LogEntry` to the session log buffer so the user knows the
    /// toggle did not take effect.
    ///
    /// A companion `RebuildStatsExtensionStateChanged` with the rolled-back
    /// state is also emitted so the UI is consistent with the actual extension
    /// state.
    RebuildStatsToggleFailed {
        session_id: SessionId,
        reason: String,
    },

    /// The 1-Hz timeline poll returned a batch of new events.
    ///
    /// Merged into `PerformanceState::timeline_tracks` and capped at
    /// `settings.devtools.timeline_event_buffer_size` total nodes.
    /// `metadata` carries `ph:"M"` thread-name events extracted from the same
    /// response and used to populate `timeline_thread_name_map`.
    TimelineEventsBatchReceived {
        session_id: SessionId,
        events: Vec<fdemon_core::timeline::TimelineEvent>,
        metadata: Vec<fdemon_core::timeline::ThreadMetadata>,
    },

    /// The user pressed `f` on the Timeline Events tab — cycle the filter.
    ///
    /// Cycles `TimelineFilter::All → Ui → Raster → All` and resets the
    /// scroll offset to the top.
    TimelineEventsCycleFilter { session_id: SessionId },

    // ── Phase 5 T04: Timeline search ─────────────────────────────────────────
    /// Open the timeline search input (user pressed `/` on the TimelineEvents tab).
    ///
    /// Sets `timeline_search_input_active = true` and
    /// `timeline_search_query = Some("")`.
    TimelineSearchOpen { session_id: SessionId },

    /// Append a character to the timeline search query while input is active.
    TimelineSearchInputChar { session_id: SessionId, ch: char },

    /// Delete the last character from the timeline search query while input is active.
    TimelineSearchInputBackspace { session_id: SessionId },

    /// Commit the current search query (Enter while input active).
    ///
    /// Sets `timeline_search_input_active = false`, keeps the query so
    /// `n`/`N` navigation can begin.
    TimelineSearchInputCommit { session_id: SessionId },

    /// Cancel the current search (Esc while input active).
    ///
    /// Sets `timeline_search_input_active = false`, clears the query.
    TimelineSearchInputCancel { session_id: SessionId },

    /// Navigate to the next search match (`n` key, query must be `Some`).
    ///
    /// Advances `timeline_search_match_cursor` modulo the match count, pans
    /// the viewport to center on the match, and updates `timeline_selected_event`.
    TimelineSearchNextMatch { session_id: SessionId },

    /// Navigate to the previous search match (`N` key, query must be `Some`).
    ///
    /// Mirrors `TimelineSearchNextMatch` in the reverse direction.
    TimelineSearchPrevMatch { session_id: SessionId },

    /// The timeline polling task started — carries shutdown/pause/handle refs.
    ///
    /// Modeled on `VmServicePerformanceMonitoringStarted`. The TEA handler
    /// stores the senders and handle on `SessionHandle` so lifecycle events
    /// (session close, VM disconnect, panel switch) can pause/stop the task.
    VmServiceTimelineMonitoringStarted {
        session_id: SessionId,
        /// Shutdown sender — `true` stops the polling loop.
        timeline_shutdown_tx: std::sync::Arc<tokio::sync::watch::Sender<bool>>,
        /// Pause sender — `true` skips poll ticks (Performance panel not active).
        timeline_pause_tx: std::sync::Arc<tokio::sync::watch::Sender<bool>>,
        /// Shared slot containing the task's `JoinHandle` (for abort on close).
        timeline_task_handle: SharedTaskHandle,
    },

    // ─────────────────────────────────────────────────────────────────────────
    // Settings — Dart Defines Modal (v1-refinements Phase 2, Task 02)
    // ─────────────────────────────────────────────────────────────────────────
    /// Open the dart defines editor modal for the launch config at `config_idx`.
    ///
    /// `config_idx` is the 0-based index into the list of launch configs
    /// loaded from `.fdemon/launch.toml`. It is extracted from the
    /// `SettingItem.id` pattern `"launch.{idx}.dart_defines"`.
    SettingsDartDefinesOpen { config_idx: usize },

    /// Close the dart defines modal and persist all changes to disk.
    SettingsDartDefinesClose,

    /// Cancel the dart defines modal, discarding any unsaved changes.
    SettingsDartDefinesCancel,

    /// Switch focus between the list pane and the edit pane.
    SettingsDartDefinesSwitchPane,

    /// Navigate up in the dart defines list.
    SettingsDartDefinesUp,

    /// Navigate down in the dart defines list.
    SettingsDartDefinesDown,

    /// Confirm selection / activate the focused button.
    SettingsDartDefinesConfirm,

    /// Move to the next field in the edit form (Tab).
    SettingsDartDefinesNextField,

    /// Input a character into the currently focused text field.
    SettingsDartDefinesInput { c: char },

    /// Backspace in the currently focused text field.
    SettingsDartDefinesBackspace,

    /// Save the current edit form entry to the defines list.
    SettingsDartDefinesSave,

    /// Delete the currently selected dart define from the list.
    SettingsDartDefinesDelete,

    // ─────────────────────────────────────────────────────────────────────────
    // Settings — Extra Args Fuzzy Modal (v1-refinements Phase 2, Task 02)
    // ─────────────────────────────────────────────────────────────────────────
    /// Open the extra args fuzzy picker for the launch config at `config_idx`.
    ///
    /// `config_idx` is the 0-based index into the list of launch configs.
    SettingsExtraArgsOpen { config_idx: usize },

    /// Close the extra args modal without saving changes.
    SettingsExtraArgsClose,

    /// Input a character into the extra args search field.
    SettingsExtraArgsInput { c: char },

    /// Backspace in the extra args search field.
    SettingsExtraArgsBackspace,

    /// Clear the extra args search query.
    SettingsExtraArgsClear,

    /// Navigate up in the extra args list.
    SettingsExtraArgsUp,

    /// Navigate down in the extra args list.
    SettingsExtraArgsDown,

    /// Confirm the selected extra args value.
    SettingsExtraArgsConfirm,

    // ─────────────────────────────────────────────────────────
    // DAP Server Messages
    // ─────────────────────────────────────────────────────────
    /// Request to start the DAP server on the configured port.
    StartDapServer,

    /// Request to stop the DAP server and disconnect all clients.
    StopDapServer,

    /// Toggle DAP server on/off (keybinding handler).
    ToggleDap,

    /// DAP server successfully started and is listening.
    DapServerStarted { port: u16 },

    /// DAP server has been stopped.
    DapServerStopped,

    /// DAP server failed to start.
    DapServerFailed { reason: String },

    /// A DAP client connected to the server.
    DapClientConnected { client_id: String },

    /// A DAP client disconnected from the server.
    DapClientDisconnected { client_id: String },

    /// IDE DAP config was generated/updated/skipped.
    ///
    /// Sent by the IDE config generation task after writing (or skipping)
    /// the config file. The `action` field is a human-readable description
    /// such as `"Created"`, `"Updated"`, or `"Skipped: <reason>"`.
    DapConfigGenerated {
        /// The IDE the config was generated for (e.g. `"VS Code"`, `"Neovim"`).
        ide_name: String,
        /// The config file path that was written (or would have been written).
        path: std::path::PathBuf,
        /// What happened: `"Created"`, `"Updated"`, or `"Skipped: <reason>"`.
        action: String,
    },

    // ─────────────────────────────────────────────────────────
    // Native Platform Log Messages (Phase 1, Task 07)
    // ─────────────────────────────────────────────────────────
    /// A native platform log line was captured (from adb logcat, log stream, etc.).
    ///
    /// Sent by the native log capture forwarding task for each log event.
    /// The update handler converts this to a `LogEntry` with
    /// `LogSource::Native { tag }` and queues it on the session log buffer.
    NativeLog {
        session_id: SessionId,
        event: NativeLogEvent,
    },

    /// Native log capture process started successfully for a session.
    ///
    /// Sent by `actions::native_logs::spawn_native_log_capture` immediately
    /// after `NativeLogCapture::spawn()` succeeds. The TEA handler stores the
    /// shutdown sender and task handle on the `SessionHandle` so they can be
    /// signalled/aborted on session stop.
    NativeLogCaptureStarted {
        session_id: SessionId,
        /// Shutdown sender — send `true` to signal the capture task to stop.
        /// Stored as `Arc` because `Message` requires `Clone`.
        shutdown_tx: std::sync::Arc<tokio::sync::watch::Sender<bool>>,
        /// JoinHandle for the capture forwarding task.
        /// Wrapped in `Arc<Mutex<Option<>>>` to satisfy the `Clone` bound on
        /// `Message`. The handler takes the handle out of the `Option` when
        /// storing it on `SessionHandle`, leaving `None` for any subsequent
        /// (unexpected) clone.
        task_handle: SharedTaskHandle,
    },

    /// Native log capture process ended (exited or failed to start).
    ///
    /// Sent by the forwarding task when the capture process's event channel
    /// closes (i.e., the capture process exited). The handler clears the
    /// stored handles from `SessionHandle`.
    NativeLogCaptureStopped { session_id: SessionId },

    // ─────────────────────────────────────────────────────────
    // Custom Log Source Lifecycle Messages (Phase 3, Task 04)
    // ─────────────────────────────────────────────────────────
    /// A custom log source process started successfully for a session.
    ///
    /// Sent by `actions::native_logs::spawn_custom_sources` immediately
    /// after `CustomLogCapture::spawn()` succeeds. The TEA handler stores the
    /// shutdown sender and task handle in `SessionHandle::custom_source_handles`
    /// so they can be signalled/aborted on session stop.
    ///
    /// Events from the custom source flow through `Message::NativeLog` — this
    /// variant is only for lifecycle management (storing the handles).
    CustomSourceStarted {
        session_id: SessionId,
        /// Human-readable name for this source (used as log tag).
        name: String,
        /// Shutdown sender — send `true` to signal the capture task to stop.
        /// Stored as `Arc` because `Message` requires `Clone`.
        shutdown_tx: std::sync::Arc<tokio::sync::watch::Sender<bool>>,
        /// JoinHandle for the capture forwarding task.
        /// Wrapped in `Arc<Mutex<Option<>>>` to satisfy the `Clone` bound on
        /// `Message`. The handler takes the handle out of the `Option` when
        /// storing it on `SessionHandle`, leaving `None` for any subsequent
        /// (unexpected) clone.
        task_handle: SharedTaskHandle,
        /// Whether this source was started before the Flutter app.
        ///
        /// Set to `true` by `spawn_pre_app_sources()`, `false` by
        /// `spawn_custom_sources()`. The TEA handler stores this on
        /// `CustomSourceHandle` so that `spawn_custom_sources()` can skip
        /// re-spawning sources that are already running.
        start_before_app: bool,
    },

    /// A custom log source process exited or was stopped.
    ///
    /// Sent by the forwarding task when the custom source's event channel
    /// closes (i.e., the process exited). The handler removes the named
    /// handle from `SessionHandle::custom_source_handles`.
    CustomSourceStopped {
        session_id: SessionId,
        /// Name of the custom source that stopped (matches the name in
        /// `CustomSourceHandle` for lookup and removal).
        name: String,
    },

    // ─────────────────────────────────────────────────────────
    // Pre-App Custom Source Lifecycle Messages
    // (pre-app-custom-sources Phase 1, Task 03)
    // ─────────────────────────────────────────────────────────
    /// All pre-app custom sources are ready (or individually timed out).
    ///
    /// Triggers the Flutter session spawn that was gated on readiness.
    /// Sent by the pre-app source coordinator task when every source with
    /// `start_before_app = true` has either become ready or timed out.
    PreAppSourcesReady {
        session_id: SessionId,
        device: Device,
        config: Option<Box<LaunchConfig>>,
    },

    /// A specific pre-app source's readiness check timed out.
    ///
    /// Informational — logged as a warning. Does not block other sources.
    /// The pre-app coordinator continues and eventually sends
    /// `PreAppSourcesReady` once all sources are settled.
    PreAppSourceTimedOut {
        session_id: SessionId,
        source_name: String,
    },

    /// Progress update during pre-app source startup.
    ///
    /// Displayed in the session's log buffer for user feedback
    /// (e.g., "Starting server 'my-server'...", "Server 'my-server' ready (3.2s)").
    PreAppSourceProgress {
        session_id: SessionId,
        message: String,
    },

    // ─────────────────────────────────────────────────────────
    // Native Tag Filter Messages (Phase 2, Task 07)
    // ─────────────────────────────────────────────────────────
    /// Toggle a specific native log tag's visibility in the active session.
    ///
    /// If the tag is currently visible, it becomes hidden (future log entries
    /// with this tag are not added to the log buffer). If hidden, it becomes
    /// visible (future entries appear in the log).
    ///
    /// The tag must already be in `NativeTagState::discovered_tags` for the
    /// toggle to have an observable effect; toggling an unknown tag is a no-op
    /// on the `hidden_tags` set but will pre-hide the tag when it is first seen.
    ToggleNativeTag { tag: String },

    /// Show all native log tags in the active session.
    ///
    /// Clears the hidden set so every tag becomes visible. Future log entries
    /// from all tags will be added to the log buffer.
    ShowAllNativeTags,

    /// Hide all native log tags in the active session.
    ///
    /// Hides every tag currently in `discovered_tags`. Future entries from
    /// any of these tags will not be added to the log buffer until un-hidden.
    HideAllNativeTags,

    /// Open the native tag filter overlay.
    ///
    /// Switches the UI into tag-filter mode where the user can see the list
    /// of discovered tags and toggle their visibility. Handled by task 09
    /// (per-tag filter UI).
    ShowTagFilter,

    /// Close the native tag filter overlay.
    ///
    /// Returns the UI to normal mode without changing tag visibility state.
    HideTagFilter,

    // ─────────────────────────────────────────────────────────
    // Tag Filter Navigation Messages (Phase 2, Task 09)
    // ─────────────────────────────────────────────────────────
    /// Move the tag filter list selection up by one row.
    TagFilterMoveUp,

    /// Move the tag filter list selection down by one row.
    TagFilterMoveDown,

    /// Toggle the visibility of the currently selected tag in the filter overlay.
    TagFilterToggleSelected,

    // ─────────────────────────────────────────────────────────────────────────
    // Shared Custom Source Messages
    // (pre-app-custom-sources Phase 2, Task 03)
    // ─────────────────────────────────────────────────────────────────────────
    /// Log event from a shared custom source (not bound to a specific session).
    ///
    /// The TEA handler broadcasts this to all active sessions, applying per-session
    /// tag filtering. Contrast with `NativeLog` which targets a single session.
    SharedSourceLog {
        /// The native log event (tag = source name, level, message).
        event: NativeLogEvent,
    },

    /// A shared custom source process has been spawned successfully.
    ///
    /// The TEA handler stores the handle on `AppState.shared_source_handles`
    /// (not per-session). Sent by the forwarding task in `spawn_pre_app_sources`
    /// or `spawn_custom_sources` for sources with `shared = true`.
    SharedSourceStarted {
        /// Source name (matches config `name` field).
        name: String,
        /// Shutdown sender for graceful stop.
        /// Wrapped in `Arc` to satisfy the `Clone` bound on `Message`.
        shutdown_tx: std::sync::Arc<tokio::sync::watch::Sender<bool>>,
        /// Task handle for abort fallback.
        ///
        /// Wrapped in `Arc<Mutex<Option<>>>` so the spawning task can deposit
        /// the handle after `tokio::spawn`. The handler takes it out of the
        /// `Option` when storing it on `AppState`, leaving `None` for any
        /// subsequent (unexpected) clone.
        task_handle: SharedTaskHandle,
        /// Whether this source was started before the Flutter app.
        start_before_app: bool,
    },

    /// A shared custom source process has exited.
    ///
    /// The TEA handler removes the handle from `AppState.shared_source_handles`
    /// and logs a warning to all active sessions.
    SharedSourceStopped {
        /// Source name.
        name: String,
    },

    // ── Flutter SDK ──────────────────────────────────────────────────────────
    /// Flutter SDK resolution completed successfully (e.g., after re-resolution
    /// triggered by a config change or explicit user request in Phase 2).
    ///
    /// Updates `AppState.resolved_sdk` and `tool_availability.flutter_sdk`.
    SdkResolved { sdk: FlutterSdk },

    /// Flutter SDK resolution failed (e.g., after the user reconfigures the
    /// SDK path to an invalid location).
    ///
    /// Clears `AppState.resolved_sdk` and `tool_availability.flutter_sdk`.
    SdkResolutionFailed { reason: String },

    // ── Flutter Version Panel ─────────────────────────────────────────────────
    /// Open the Flutter Version panel (V key in Normal mode)
    ShowFlutterVersion,

    /// Close the Flutter Version panel (Esc key)
    HideFlutterVersion,

    /// Priority-ordered escape: close panel → return to Normal
    FlutterVersionEscape,

    /// Switch pane focus (Tab key)
    FlutterVersionSwitchPane,

    /// Navigate up in the version list (k/Up)
    FlutterVersionUp,

    /// Navigate down in the version list (j/Down)
    FlutterVersionDown,

    /// Cache scan completed — populate version list
    FlutterVersionScanCompleted { versions: Vec<InstalledSdk> },

    /// Cache scan failed
    FlutterVersionScanFailed { reason: String },

    /// Switch to the selected version (Enter key)
    FlutterVersionSwitch,

    /// Version switch completed — SDK re-resolved
    FlutterVersionSwitchCompleted { version: String },

    /// Version switch failed
    FlutterVersionSwitchFailed { reason: String },

    /// Remove the selected version from cache (d key)
    FlutterVersionRemove,

    /// Version removal completed
    FlutterVersionRemoveCompleted { version: String },

    /// Version removal failed
    FlutterVersionRemoveFailed { reason: String },

    /// Install a new version (i key) — stub for Phase 3
    FlutterVersionInstall,

    /// Update the selected version (u key) — stub for Phase 3
    FlutterVersionUpdate,

    // ── Install Wizard ────────────────────────────────────────────────────────
    /// Open the Install Wizard panel.
    ///
    /// `origin` records why the wizard was opened so the post-install handback
    /// can be gated: only `Bootstrap` auto-advances to device discovery;
    /// `UserInvoked` (the `I` key) is an informational view that returns to
    /// `UiMode::Normal` on close.
    ShowInstallWizard { origin: WizardOrigin },

    /// Close the Install Wizard panel
    HideInstallWizard,

    /// Priority-ordered escape: close panel → return to Normal
    InstallWizardEscape,

    /// Switch pane focus (Tab key)
    InstallWizardSwitchPane,

    /// Navigate up in the step list or scroll detail pane up (k/Up)
    InstallWizardUp,

    /// Navigate down in the step list or scroll detail pane down (j/Down)
    InstallWizardDown,

    /// Re-run the toolchain preflight check (r key)
    InstallWizardRerunPreflight,

    /// Copy the selected step's guided command to the clipboard (c key).
    ///
    /// No-op when the currently selected step has no guided command to copy
    /// (e.g. FlutterSdk and PathConfig steps, which are fully automated).
    /// Used for steps like `PlatformAndroid` that may surface a JDK install
    /// command the user should run manually.
    InstallWizardCopyCommand,

    /// Select the previous guided command within the selected step (`[` key).
    ///
    /// No-op when the selected step has 0 or 1 guided commands.
    /// Steps with multiple commands (e.g. macOS Prerequisites: CLT / CocoaPods /
    /// Rosetta) cycle backwards through the list.
    InstallWizardPrevCommand,

    /// Select the next guided command within the selected step (`]` key).
    ///
    /// No-op when the selected step has 0 or 1 guided commands.
    /// Steps with multiple commands (e.g. macOS Prerequisites: CLT / CocoaPods /
    /// Rosetta) cycle forwards through the list.
    InstallWizardNextCommand,

    /// Toggle expand/collapse of the Platforms submenu parent row.
    ///
    /// No-op unless the selected step is the `Platforms` parent. When the
    /// submenu expands, host-gated leaf rows (`PlatformAndroid`, `PlatformWeb`,
    /// etc.) are inserted after the parent in the step list. The cursor stays on
    /// the parent row; the user presses `j` to descend into the leaves.
    InstallWizardToggleExpand,

    /// Expand the Platforms submenu (directional `l`/`Right`). Sets `platforms_expanded = true`.
    /// No-op unless the selected step is the collapsed `Platforms` parent.
    InstallWizardExpand,

    /// Collapse the Platforms submenu (directional `h`/`Left`). Sets `platforms_expanded = false`.
    /// No-op unless the submenu is currently expanded; re-anchors the cursor to the parent.
    InstallWizardCollapse,

    // ── Install Wizard — Version Picker (Phase 6) ────────────────────────────
    /// Open the Flutter version picker overlay.
    ///
    /// Dispatched by the `v` key in `UiMode::InstallWizard`, or by `Enter` on the
    /// `FlutterSdk` step when no version choice exists yet. The handler refuses
    /// while a step is running and no-ops unless the selected step is `FlutterSdk`;
    /// otherwise it opens the picker and, when a manifest fetch is needed, returns
    /// `UpdateAction::FetchFlutterReleaseManifest`.
    InstallWizardOpenVersionPicker,

    /// Close the version picker overlay without confirming (Esc).
    InstallWizardVersionPickerClose,

    /// Move the picker selection up one row (`k` / Up).
    InstallWizardVersionPickerUp,

    /// Move the picker selection down one row (`j` / Down).
    InstallWizardVersionPickerDown,

    /// Cycle to the next channel tab: Stable → Beta → Master (Tab).
    InstallWizardVersionPickerNextTab,

    /// Re-fetch the release manifest while the picker is visible (`r`).
    InstallWizardVersionPickerRefetch,

    /// Confirm the current picker selection (Enter).
    ///
    /// On a stable/beta/master row this dispatches the pinned install through the
    /// shared `FlutterSdk` run path. In the `Failed` fetch state it closes the
    /// picker and dispatches an un-pinned default-channel install (offline path).
    InstallWizardVersionPickerConfirm,

    /// A Flutter release manifest fetch succeeded.
    ///
    /// The handler groups the releases (arch-filtered for the host) and populates
    /// the picker. Applying with the picker already closed is harmless (the rows
    /// are cached for the next open).
    FlutterManifestFetched {
        manifest: fdemon_daemon::toolchain::FlutterReleaseManifest,
    },

    /// A Flutter release manifest fetch failed.
    ///
    /// The handler records the error and transitions the picker to the `Failed`
    /// state so the user can retry with `r` or fall back to a default-channel
    /// install with Enter.
    FlutterManifestFetchFailed { error: String },

    /// Preflight task completed — populate the wizard with the report
    ToolchainPreflightCompleted {
        report: fdemon_daemon::toolchain::ToolchainReport,
    },

    // ── Install Wizard — Step Execution Protocol (Phase 2, Task 05) ──────────
    /// Run (or retry) the currently selected wizard step.
    ///
    /// Emitted by `Enter` in `UiMode::InstallWizard`. The update handler reads
    /// `install_wizard_state.selected_step` to determine which step to run and
    /// returns `UpdateAction::RunWizardStep`. Handling lands in task 09.
    InstallWizardRunSelectedStep,

    /// A wizard step has started executing.
    ///
    /// Transitions the step's status to `StepStatus::Running` (added in task 07)
    /// and clears any previous log lines for the step.
    ///
    /// `run_seq` is the sequence counter assigned at dispatch (mirrors the value
    /// stored on `InstallWizardState::run_seq`). The handler discards any message
    /// whose `run_seq` does not equal the current state `run_seq`, closing the
    /// cross-kind race where a delayed Started from a cancelled run (Run A) can
    /// clobber the live install (Run B) via the `begin_step` defensive fallback.
    WizardStepStarted { kind: WizardStepKind, run_seq: u64 },

    /// Streamed log line from a running wizard step.
    ///
    /// Appended to the step's detail log buffer so the TUI can display
    /// live progress while the executor is running.
    WizardStepLog { kind: WizardStepKind, line: String },

    /// Download progress for a running wizard step.
    ///
    /// `received` is the number of bytes downloaded so far.
    /// `total` is `Some(n)` when the Content-Length is known, or `None`
    /// for chunked/unknown-size transfers.
    WizardDownloadProgress {
        kind: WizardStepKind,
        received: u64,
        total: Option<u64>,
    },

    /// A wizard step finished successfully.
    ///
    /// `summary` is a human-readable description of what was done (e.g. the
    /// resolved SDK path or the rc file written). `sdk_path` is set for the
    /// `FlutterSdk` step and `None` for all other steps.
    WizardStepCompleted {
        kind: WizardStepKind,
        summary: String,
        sdk_path: Option<std::path::PathBuf>,
    },

    /// Phase label update from a running wizard step.
    ///
    /// Sent by the executor when `InstallEvent::Phase(label)` is received
    /// (e.g. `"Cloning"`, `"Downloading"`, `"Verifying"`, `"Extracting"`).
    /// The handler calls `InstallWizardState::set_step_phase` so the
    /// `StepProgress` widget can display the current operation name.
    WizardStepPhase { kind: WizardStepKind, label: String },

    /// A wizard step failed.
    ///
    /// `reason` is a human-readable error description shown in the step's
    /// detail pane so the user can diagnose and retry.
    WizardStepFailed {
        kind: WizardStepKind,
        reason: String,
    },

    /// The install task for a wizard step is ready — carries the join handle
    /// so the TEA can upgrade the already-stored `InstallTaskHandle`.
    ///
    /// Sent by `handle_action(RunWizardStep)` after `tokio::spawn` returns
    /// (so the `JoinHandle` is available). The token is no longer carried here
    /// — it is minted synchronously by `handle_run_selected_step` and stored
    /// on `InstallWizardState::install_task` **before** `RunWizardStep` is
    /// dispatched. This message only upgrades the `join` field.
    ///
    /// `handle_install_task_ready` validates that `kind` and `run_seq` match
    /// the current run before upgrading — stale messages are discarded and
    /// the associated `JoinHandle` is aborted.
    WizardInstallTaskReady {
        /// Which wizard step this ready message belongs to.
        ///
        /// Used by `handle_install_task_ready` to reject stale messages from
        /// a previously cancelled run of the same step kind.
        kind: WizardStepKind,
        /// Sequence counter from `InstallWizardState::run_seq` at the time
        /// the run was started. Used alongside `kind` to distinguish run A
        /// from run B when the same step kind is retried after a cancel.
        run_seq: u64,
        /// JoinHandle for the install task.
        ///
        /// Wrapped in `Arc<Mutex<Option<>>>` to satisfy the `Clone` bound on
        /// `Message`. The handler takes the handle out when upgrading.
        handle: std::sync::Arc<std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>>,
    },

    /// Cancel the currently running wizard step (Esc while a step is running).
    ///
    /// The update handler calls `install_task.cancel.cancel()`, resets the
    /// step to `Idle`, and sets a neutral "Cancelled" `status_message`.
    /// A subsequent `Enter` retries the step.
    ///
    /// This message is dispatched by `Esc` only when `is_step_running()` is
    /// `true`; when no step is running `Esc` dispatches `InstallWizardEscape`
    /// (close the wizard).
    InstallWizardCancelStep,

    /// Auto-configure PATH after a successful managed install.
    ///
    /// Emitted by `handle_step_completed` when `FlutterSdk` or `PlatformAndroid`
    /// completes with a resolved SDK path.  The handler dispatches
    /// `RunWizardStep { kind: PathConfig, .. }` using the freshly-stashed SDK
    /// root so the shell rc file is updated without a manual step.
    ///
    /// - For `FlutterSdk` origin: writes the Flutter `<sdk>/bin` PATH entry
    ///   only (`android_sdk_root: None`), keeping FlutterSdk side-effects scoped
    ///   to what was installed.
    /// - For `PlatformAndroid` origin: writes both the Flutter PATH (if a Flutter
    ///   SDK is known) and the Android `ANDROID_HOME` + PATH entries.
    ///
    /// If no Flutter bin dir can be resolved (unlikely but possible on a fresh
    /// machine with no prior SDK), the handler falls back to
    /// `InstallWizardRerunPreflight` so the step list still refreshes.
    InstallWizardAutoConfigurePath {
        /// Which installer step triggered this auto-config.
        kind: WizardStepKind,
    },

    // ── Mouse Click Messages (Phase 5) ────────────────────────────────────────
    /// Click on a device row inside the NewSessionDialog Connected/Bootable list.
    ///
    /// `index` is the absolute position into the *currently active* tab's device
    /// list at render time (Connected or Bootable, whichever was visible). The
    /// handler at [`crate::handler::new_session::clicks::handle_select_device_at`]
    /// sets `target_selector.selected_index = index` for the active tab and emits
    /// a follow-up [`Message::NewSessionDialogDeviceSelect`] via
    /// [`UpdateResult::message`] so the click is exactly equivalent to "arrow
    /// down N times then Enter".
    NewSessionDialogSelectDeviceAt { index: usize },

    /// Click on a launch-context field row (Configuration / Mode / Flavor /
    /// Entry Point / Dart Defines).
    ///
    /// Sets `launch_context.focused_field = field` and emits a follow-up
    /// [`Message::NewSessionDialogFieldActivate`] via [`UpdateResult::message`]
    /// for fields that activate-on-Enter. The Mode field's left/right cycler is
    /// not exercised by click in v1 — clicking the Mode field activates the
    /// existing keyboard-Enter behaviour (cycle to next mode).
    NewSessionDialogFocusField {
        field: crate::new_session_dialog::LaunchContextField,
    },

    /// Click on a result row inside the NewSessionDialog fuzzy modal
    /// (config picker, flavor picker, entry-point picker).
    ///
    /// Sets `fuzzy_modal.selected_index = index` and emits a follow-up
    /// [`Message::NewSessionDialogFuzzyConfirm`] via [`UpdateResult::message`].
    /// Equivalent to "arrow down N times then Enter" inside the modal.
    NewSessionDialogFuzzySelectAt { index: usize },

    /// Click on a setting row in the Settings panel.
    ///
    /// `index` is the absolute position into the active tab's `SettingItem` list
    /// at render time. The handler at
    /// [`crate::handler::settings_handlers::handle_settings_click_row`] updates
    /// `AppState::last_settings_click` for double-click detection and sets
    /// `settings_view_state.selected_index = index`. When the same row is
    /// clicked twice within 400 ms, a follow-up
    /// [`Message::SettingsToggleEdit`] is emitted via
    /// [`UpdateResult::message`] (mirroring [`Message::ClickLogRow`]).
    SettingsClickRow { index: usize },

    /// Click on a tag row in the tag-filter overlay.
    ///
    /// `index` is the absolute position into the *sorted* tag list at render
    /// time. The inline handler in `update.rs` sets
    /// `tag_filter_ui.selected_index = index` AND toggles the tag's visibility
    /// in a single arm — no follow-up message. Single click both navigates to
    /// and toggles the tag, since there is no useful "select-without-toggle"
    /// state in this overlay (the user wants both).
    TagFilterClickRow { index: usize },

    // ── Mouse Click Messages (Phase 4) ──────────────────────────────────────
    /// Click on a single log-view row.
    ///
    /// Emitted by the per-frame mouse region registry when the user left-clicks
    /// inside the log content area. `entry_id` is the [`LogEntry::id`] of the
    /// clicked entry; `frame_index` is `Some(i)` when the click landed on the
    /// i-th visible stack-frame line under that entry, or `None` for the
    /// message-line click.
    ///
    /// Handler at [`crate::handler::log_view::handle_click_log_row`] updates
    /// `AppState::last_log_click` for double-click detection. When the same
    /// entry is clicked twice within 400 ms, a follow-up
    /// [`Message::ToggleStackTraceForEntry`] is emitted via
    /// [`UpdateResult::message`].
    ClickLogRow {
        entry_id: u64,
        frame_index: Option<usize>,
    },

    /// Toggle stack trace expand / collapse for a *specific* log entry.
    ///
    /// Emitted as a follow-up to [`Message::ClickLogRow`] when a double click is
    /// detected. Distinct from [`Message::ToggleStackTrace`], which operates on
    /// the scroll-focused entry — the click target is rarely the focused entry,
    /// so the click flow needs an absolute-id variant.
    ToggleStackTraceForEntry { entry_id: u64 },

    /// Click on a row in the widget inspector tree.
    ///
    /// `index` is the absolute position into `InspectorState::visible_nodes()`
    /// at render time — the registry stored this index when recording the row's
    /// rect. The handler sets `inspector.selected_index = index` and dispatches
    /// a layout fetch under the same debounce / cache rules as
    /// [`InspectorNav::Up`] / [`InspectorNav::Down`].
    DevToolsInspectorSelectRow { index: usize },

    /// Click on the leading expansion glyph (▶ / ▼ / ●) of a tree row.
    ///
    /// Selects the row first (same as [`Message::DevToolsInspectorSelectRow`])
    /// then toggles the node's `expanded` set if the node has children. No-op
    /// for leaf nodes.
    DevToolsInspectorToggleNode { index: usize },

    /// Opens the Details view for the currently selected widget in the
    /// Inspector tree.
    ///
    /// Snapshots the selected `value_id` into `InspectorState::details_node_id`
    /// and sets `InspectorState::details_open = true`. In Phase 1 this also
    /// fires `FetchLayoutData` for the snapshotted node if it isn't already
    /// cached. Phase 2 will additionally fire `FetchInspectorProperties`.
    ///
    /// Key binding: `Enter` while the Inspector tree is focused (task 06).
    DevToolsInspectorOpenDetails,

    /// Closes the Details view and returns the Inspector tab to tree mode.
    ///
    /// Sets `InspectorState::details_open = false`. Tied to the first `Esc`
    /// press while details is open (tiered Esc — a second `Esc` exits DevTools
    /// mode entirely).
    ///
    /// Key binding: `Esc` while details view is open (task 06).
    DevToolsInspectorCloseDetails,

    /// Cycles the active Details tab forward or backward.
    ///
    /// `forward = true` advances to the next [`crate::state::DetailsTab`]
    /// (wrapping at the end); `forward = false` steps to the previous tab
    /// (wrapping at the start). The cycle order is
    /// `Properties → RenderObject → FlexExplorer → Properties`.
    ///
    /// Key bindings: `Tab` (forward) and `Shift+Tab` (backward) while the
    /// Details view is open (task 06).
    DevToolsInspectorCycleTab { forward: bool },

    /// Toggles `InspectorState::hide_implementation_widgets`.
    ///
    /// The handler reads the current value, flips it, rebuilds the visible-row
    /// list, and persists the new value to `.fdemon/config.toml` (task 03 /
    /// task 05). This variant is parameterless — the toggle is not signed
    /// (cannot force a specific value via the message bus), consistent with
    /// other toggle variants in this enum.
    ///
    /// Key binding: `H` while the Inspector panel is active (task 06).
    DevToolsInspectorToggleHideImplementation,

    // ── Mouse Capture (log-text-selection-broken fix) ─────────────────────────
    /// Copy a specific log entry's rendered text to the system clipboard.
    ///
    /// Emitted by the right-click handler in `handler/mouse.rs` when the user
    /// right-clicks on a log row. The handler resolves `entry_id` to the entry's
    /// rendered text and writes it via the `Clipboard` service; a confirmation
    /// toast is pushed onto `AppState::toasts`.
    ///
    /// Fix for log-text-selection bug — see
    /// `workflow/plans/bugs/log-text-selection-broken/BUG.md`.
    CopyLogEntryToClipboard { entry_id: u64 },

    /// Request a runtime toggle of terminal mouse capture.
    ///
    /// Emitted by the `Alt+m` keybinding. The update handler returns
    /// `UpdateAction::SetMouseCapture(!state.mouse_capture_active)`; the runner
    /// performs the side effect and follows up with `MouseCaptureChanged` once
    /// the terminal mode has changed.
    ToggleMouseCapture,

    /// Reflect a successful runtime change to terminal mouse capture.
    ///
    /// Sent by the runner after `terminal::set_mouse_capture(...)` returns
    /// `Ok(())`. Updates `AppState::mouse_capture_active` so the status-bar
    /// indicator (Task 08) and the click hit-test gates render the correct
    /// state.
    MouseCaptureChanged { active: bool },

    /// Internal trigger: start the version probe.
    ///
    /// Sent as a follow-up message from `handle_show` so that both
    /// `ScanInstalledSdks` (returned as action) and `ProbeFlutterVersion`
    /// (returned as action on this message's turn) can be dispatched in the
    /// same TEA processing cycle. Only fires if `probe_completed == false`.
    FlutterVersionProbeRequested,

    /// Result of the async `flutter --version --machine` probe.
    ///
    /// Sent by the `ProbeFlutterVersion` background task once the subprocess
    /// exits (successfully or with an error).
    FlutterVersionProbeCompleted {
        /// `Ok` carries the parsed metadata; `Err` carries a human-readable
        /// error description. Both variants set `probe_completed = true`.
        result: std::result::Result<FlutterVersionInfo, String>,
    },

    // ── Phase 5: Frame-anchored timeline viewport ─────────────────────────────
    /// Commit the Timeline Events viewport anchor to the given frame number.
    ///
    /// Sent by the 200 ms debounce task spawned on each frame selection change.
    /// Stale messages (where `generation < state.performance.frame_anchor_generation`)
    /// are silently dropped; only the most recent debounce wins.
    ApplyFrameAnchor {
        /// Session the anchor belongs to.
        session_id: SessionId,
        /// Monotonic counter at the time this debounce was created.
        generation: u64,
        /// Frame number to anchor on, or `None` to clear the anchor.
        frame_number: Option<u64>,
    },

    // ── Phase 5: Timeline pan/zoom viewport ───────────────────────────────────
    /// Zoom in on the Timeline Events Gantt (halve the viewport width).
    ///
    /// Sets `timeline_follow_latest = false` and halves `timeline_viewport_width_micros`,
    /// clamped at [`TIMELINE_VIEWPORT_MIN_MICROS`]. The anchor point is the
    /// current viewport center.
    TimelineZoomIn { session_id: SessionId },

    /// Zoom out on the Timeline Events Gantt (double the viewport width).
    ///
    /// Sets `timeline_follow_latest = false` and doubles `timeline_viewport_width_micros`,
    /// clamped at [`TIMELINE_VIEWPORT_MAX_MICROS`].
    TimelineZoomOut { session_id: SessionId },

    /// Pan the Timeline Events Gantt left by 10% of the current viewport width.
    ///
    /// Sets `timeline_follow_latest = false`; saturates at 0.
    TimelinePanLeft { session_id: SessionId },

    /// Pan the Timeline Events Gantt right by 10% of the current viewport width.
    ///
    /// Sets `timeline_follow_latest = false`.
    TimelinePanRight { session_id: SessionId },

    /// Resume follow-latest mode on the Timeline Events Gantt.
    ///
    /// Sets `timeline_follow_latest = true` and resets `viewport_width_micros` to
    /// the default 5 s window. The `committed_frame_anchor` (if any) is preserved —
    /// the next render will return to the frame-anchored viewport (PLAN D2 mode 2)
    /// rather than the live-edge fallback.
    TimelineFollowLatest { session_id: SessionId },

    // ── Phase 5 T03: Timeline event selection ─────────────────────────────────
    /// Select the first visible event in the Timeline Events Gantt.
    ///
    /// Selects the first root event of the first visible thread (in `tid` ascending
    /// order, filter-respected). Emitted by `Enter` when no event is currently selected.
    TimelineSelectFirstVisible { session_id: SessionId },

    /// Move the timeline event selection in the given direction.
    ///
    /// Emitted by `←`/`→` (sibling nav) and `↑`/`↓`/`j`/`k` (depth/thread nav)
    /// when an event is selected.
    TimelineMoveSelection {
        session_id: SessionId,
        dir: SelectionDirection,
    },

    /// Open the event details popup for the currently selected event.
    ///
    /// Emitted by `Enter` when an event is already selected and the popup is
    /// not open. No-op if no event is selected.
    TimelineOpenPopup { session_id: SessionId },

    /// Close the event details popup without clearing the selection.
    ///
    /// Emitted by `Esc` when the popup is open.
    TimelineClosePopup { session_id: SessionId },

    /// Clear the timeline event selection.
    ///
    /// Emitted by `Esc` when the popup is closed but an event is selected.
    TimelineClearSelection { session_id: SessionId },

    /// Select a specific event by cursor (mouse-driven).
    ///
    /// Emitted when the user clicks on a Gantt bar. The handler sets
    /// `timeline_selected_event = Some(cursor)` without opening the popup
    /// (a second click or `Enter` opens the popup).
    TimelineSelectAt {
        session_id: SessionId,
        cursor: TimelineEventCursor,
    },
}
