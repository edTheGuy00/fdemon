//! Message types for the application (TEA pattern)

use crate::config::{FlutterMode, LaunchConfig, LoadedConfigs};
use crate::input_key::InputKey;
use crate::new_session_dialog::{DartDefine, FuzzyModalType, TargetTab};
use crate::session::{NetworkDetailTab, SessionId};
use crate::state::DevToolsPanel;
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

    /// Exit DevTools mode (return to Normal mode via Esc).
    ExitDevToolsMode,

    /// Switch to a specific DevTools sub-panel.
    SwitchDevToolsPanel(DevToolsPanel),

    /// Open Flutter DevTools in the system browser.
    OpenBrowserDevTools,

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

    // ── Performance Panel UI Messages ─────────────────────────────────────────
    /// Toggle the allocation table sort column (Size ↔ Instances).
    ToggleAllocationSort,

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
}
