//! Application state (Model in TEA pattern)

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Instant;

use rand::Rng;

use crate::config::{LoadedConfigs, Settings, SettingsTab, UserPreferences};
use crate::confirm_dialog::ConfirmDialogState;
use crate::new_session_dialog::NewSessionDialogState;
use crate::new_session_dialog::{DartDefinesModalState, FuzzyModalState};
use fdemon_core::{AppPhase, DiagnosticsNode, LayoutInfo};
use fdemon_daemon::{AndroidAvd, Device, IosSimulator, ToolAvailability};

use super::session_manager::SessionManager;

/// Current UI mode/screen
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UiMode {
    /// Startup state - show NewSessionDialog (no sessions yet)
    #[default]
    Startup,

    /// Normal TUI with log view and status bar
    Normal,

    /// New session dialog - unified device and configuration selection
    /// Used both at startup (Startup mode) and when adding sessions (Normal mode)
    NewSessionDialog,

    /// Emulator selector (after choosing "Launch Android Emulator")
    EmulatorSelector,

    /// Confirmation dialog (e.g., quit confirmation)
    ConfirmDialog,

    /// Initial loading screen (discovering devices)
    Loading,

    /// Search input mode - capturing text for log search
    SearchInput,

    /// Link highlight mode - showing clickable file references
    /// User can press 1-9 or a-z to open a file in their editor
    LinkHighlight,

    /// Settings panel - full-screen settings UI
    Settings,

    /// DevTools panel mode - replaces log view with Inspector/Performance panels
    DevTools,
}

// ─────────────────────────────────────────────────────────────────────────────
// DevTools State (Phase 4)
// ─────────────────────────────────────────────────────────────────────────────

/// VM Service connection status for display in DevTools UI.
///
/// Extends the binary `vm_connected: bool` flag on `Session` with richer
/// reconnection/timeout state that can be surfaced in the TUI.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum VmConnectionStatus {
    /// WebSocket connection established and VM Service is responding.
    #[default]
    Connected,

    /// No active connection (startup or after a clean disconnect).
    Disconnected,

    /// Connection was lost and the client is retrying.
    ///
    /// `attempt` is 1-based (first retry = 1).
    /// `max_attempts` is the total number of retries before giving up.
    Reconnecting {
        /// Current attempt number (1-based).
        attempt: u32,
        /// Maximum number of retry attempts.
        max_attempts: u32,
    },

    /// A specific VM RPC call timed out (e.g., FetchWidgetTree, FetchLayoutData).
    ///
    /// The connection itself may still be live; this indicates that a single
    /// on-demand request did not complete within the configurable deadline.
    TimedOut,
}

impl VmConnectionStatus {
    /// Short human-readable label used in the DevTools tab bar indicator.
    ///
    /// Examples:
    /// - `"Connected"`
    /// - `"Reconnecting (2/10)"`
    /// - `"Disconnected"`
    /// - `"Timed Out"`
    pub fn label(&self) -> String {
        match self {
            VmConnectionStatus::Connected => "Connected".to_string(),
            VmConnectionStatus::Disconnected => "Disconnected".to_string(),
            VmConnectionStatus::Reconnecting {
                attempt,
                max_attempts,
            } => {
                format!("Reconnecting ({attempt}/{max_attempts})")
            }
            VmConnectionStatus::TimedOut => "Timed Out".to_string(),
        }
    }

    /// Returns `true` when the status indicates some form of connectivity
    /// loss (disconnected, reconnecting, or timed-out).
    pub fn is_degraded(&self) -> bool {
        !matches!(self, VmConnectionStatus::Connected)
    }
}

/// Active sub-panel within DevTools mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DevToolsPanel {
    /// Widget tree inspector with expand/collapse navigation.
    #[default]
    Inspector,

    /// FPS, memory usage, and frame timing display.
    Performance,

    /// HTTP/WebSocket network request monitor.
    Network,
}

/// A user-friendly error with an actionable hint for DevTools panels.
///
/// Created by [`crate::handler::devtools::map_rpc_error`] which maps raw RPC
/// error strings to concise messages the TUI can display in a centred error box.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DevToolsError {
    /// Short, human-readable description of the problem (≤ 60 chars recommended).
    pub message: String,
    /// Actionable guidance shown below the message (key hints, mode suggestion, etc.).
    pub hint: String,
}

impl DevToolsError {
    /// Create a new `DevToolsError`.
    pub fn new(message: impl Into<String>, hint: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            hint: hint.into(),
        }
    }
}

/// State for the widget inspector tree view.
///
/// Also holds layout data for the currently selected widget (merged into this struct
/// in Phase 2). Layout fields use a `layout_` prefix to avoid conflicts with inspector fields.
#[derive(Debug, Clone, Default)]
pub struct InspectorState {
    /// The root widget tree node (fetched on-demand via VM Service RPC).
    pub root: Option<DiagnosticsNode>,

    /// Set of expanded node IDs (value_id). Collapsed by default.
    pub expanded: HashSet<String>,

    /// Index of the currently selected visible node (0-based flat list position).
    pub selected_index: usize,

    /// Whether a tree fetch is currently in progress.
    pub loading: bool,

    /// User-friendly error from the last failed fetch attempt.
    ///
    /// `None` when no error has occurred or after a successful fetch.
    /// Populated by [`crate::handler::devtools::map_rpc_error`] so the TUI
    /// always shows a clear message + hint instead of a raw RPC error string.
    pub error: Option<DevToolsError>,

    /// Whether the `"fdemon-inspector-1"` VM object group exists on the Flutter VM.
    ///
    /// Set to `true` after a successful widget tree fetch, `false` after disposal
    /// or reset. Used to skip unnecessary `disposeGroup` RPC calls when no group
    /// has been created yet.
    pub has_object_group: bool,

    /// Timestamp of the last successful widget tree fetch.
    ///
    /// Used to enforce a 2-second cooldown on rapid refresh requests (`r` key).
    /// A new fetch is only dispatched when all of the following hold:
    /// - `loading == false` (no fetch in flight), AND
    /// - either `last_fetch_time` is `None`, OR at least 2 seconds have elapsed.
    ///
    /// This prevents RPC spam when the user holds down the refresh key.
    pub last_fetch_time: Option<Instant>,

    // ── Layout fields ──────────────────────────────────────────────────────────
    /// Layout info for the currently selected widget.
    pub layout: Option<LayoutInfo>,

    /// Whether a layout fetch is in progress.
    pub layout_loading: bool,

    /// User-friendly error from the last failed layout fetch.
    ///
    /// `None` when no error has occurred or after a successful fetch.
    /// Populated by [`crate::handler::devtools::map_rpc_error`].
    pub layout_error: Option<DevToolsError>,

    /// Whether the `"devtools-layout"` VM object group exists on the Flutter VM.
    ///
    /// Set to `true` after a successful layout fetch, `false` after disposal
    /// or reset. Used to skip unnecessary `disposeGroup` RPC calls when no group
    /// has been created yet.
    pub has_layout_object_group: bool,

    /// The `value_id` of the inspector node for which layout data was last fetched.
    ///
    /// Compared against the currently selected inspector node when the user
    /// switches to the Layout panel. If the selected node has not changed,
    /// the layout fetch is skipped to avoid redundant RPC calls.
    ///
    /// Reset to `None` when the state is reset (e.g., session switch).
    pub last_fetched_node_id: Option<String>,

    /// The `value_id` of the inspector node for which a fetch is currently in flight.
    ///
    /// Set when a `FetchLayoutData` action is dispatched and consumed in
    /// `handle_layout_data_fetched` to populate `last_fetched_node_id` on
    /// success. Reset to `None` on failure or reset.
    pub pending_node_id: Option<String>,

    /// Timestamp of the last layout data fetch dispatch.
    ///
    /// Used to enforce a 500ms cooldown on auto-fetch requests during tree
    /// navigation (Up/Down keys). A new fetch is skipped when either:
    /// - `layout_loading == true` (fetch already in flight), OR
    /// - `layout_last_fetch_time` is `Some(t)` and `t.elapsed() < 500ms`.
    ///
    /// This prevents RPC spam during rapid scrolling through the widget tree.
    pub layout_last_fetch_time: Option<Instant>,
}

impl InspectorState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Toggle expand/collapse for the node at the given value_id.
    pub fn toggle_expanded(&mut self, value_id: &str) {
        if !self.expanded.remove(value_id) {
            self.expanded.insert(value_id.to_string());
        }
    }

    /// Check if a node is expanded.
    pub fn is_expanded(&self, value_id: &str) -> bool {
        self.expanded.contains(value_id)
    }

    /// Reset state (e.g., on session change or refresh).
    pub fn reset(&mut self) {
        self.root = None;
        self.expanded.clear();
        self.selected_index = 0;
        self.loading = false;
        self.error = None;
        self.has_object_group = false;
        self.last_fetch_time = None;
        // Layout fields
        self.layout = None;
        self.layout_loading = false;
        self.layout_error = None;
        self.has_layout_object_group = false;
        self.last_fetched_node_id = None;
        self.pending_node_id = None;
        self.layout_last_fetch_time = None;
    }

    /// Returns `true` if a tree refresh request should be suppressed.
    ///
    /// A request is suppressed when either:
    /// - A fetch is already in flight (`loading == true`), OR
    /// - The last successful fetch occurred within the 2-second cooldown window.
    pub fn is_fetch_debounced(&self) -> bool {
        const COOLDOWN: std::time::Duration = std::time::Duration::from_secs(2);
        if self.loading {
            return true;
        }
        self.last_fetch_time
            .map(|t| t.elapsed() < COOLDOWN)
            .unwrap_or(false)
    }

    /// Returns `true` if a layout fetch should be skipped (debounced).
    ///
    /// A layout fetch is debounced when either:
    /// - A fetch is already in flight (`layout_loading == true`), OR
    /// - The last layout fetch was dispatched within the 500ms cooldown window.
    ///
    /// This shorter cooldown (vs the 2s tree cooldown) allows reasonable
    /// responsiveness during tree navigation without spamming VM Service RPC calls.
    pub fn is_layout_fetch_debounced(&self) -> bool {
        if self.layout_loading {
            return true;
        }
        match self.layout_last_fetch_time {
            Some(t) => t.elapsed() < std::time::Duration::from_millis(500),
            None => false,
        }
    }

    /// Record that a fetch was just initiated.
    ///
    /// Sets `loading = true` and updates `last_fetch_time` to `Instant::now()`
    /// so that the next request within 2 seconds is suppressed by
    /// [`Self::is_fetch_debounced`].
    pub fn record_fetch_start(&mut self) {
        self.loading = true;
        self.last_fetch_time = Some(Instant::now());
    }

    /// Build a flat list of visible nodes based on expand/collapse state.
    /// Returns (node_ref, depth) pairs for rendering.
    pub fn visible_nodes(&self) -> Vec<(&DiagnosticsNode, usize)> {
        let Some(root) = &self.root else {
            return vec![];
        };
        let mut result = Vec::new();
        self.collect_visible(root, 0, &mut result);
        result
    }

    fn collect_visible<'a>(
        &self,
        node: &'a DiagnosticsNode,
        depth: usize,
        result: &mut Vec<(&'a DiagnosticsNode, usize)>,
    ) {
        // Skip hidden nodes
        if !node.is_visible() {
            return;
        }
        result.push((node, depth));
        if let Some(value_id) = &node.value_id {
            if self.is_expanded(value_id) {
                for child in &node.children {
                    self.collect_visible(child, depth + 1, result);
                }
            }
        }
    }

    /// Return the description of the currently selected visible node.
    ///
    /// Traverses the tree in pre-order (same order as [`Self::visible_nodes`])
    /// and returns the `description` of the node at [`Self::selected_index`].
    /// Returns `None` when no tree is loaded, or when `selected_index` is out
    /// of bounds.
    ///
    /// Unlike [`Self::visible_nodes`], this method does **not** allocate a
    /// `Vec`. It is O(n) in the number of visible nodes but avoids the
    /// allocation cost, making it suitable for the render path where only a
    /// single description is needed.
    pub fn selected_node_description(&self) -> Option<String> {
        let root = self.root.as_ref()?;
        let mut remaining = self.selected_index;
        self.find_nth_description(root, &mut remaining)
            .map(|s| s.to_string())
    }

    /// Recursive pre-order traversal that counts down `remaining` and returns
    /// the description when `remaining` hits zero.
    fn find_nth_description<'a>(
        &self,
        node: &'a DiagnosticsNode,
        remaining: &mut usize,
    ) -> Option<&'a str> {
        if !node.is_visible() {
            return None;
        }
        if *remaining == 0 {
            return Some(&node.description);
        }
        *remaining -= 1;

        if let Some(value_id) = &node.value_id {
            if self.is_expanded(value_id) {
                for child in &node.children {
                    if let Some(found) = self.find_nth_description(child, remaining) {
                        return Some(found);
                    }
                }
            }
        }
        None
    }
}

/// Complete state for the DevTools mode UI.
#[derive(Debug, Clone, Default)]
pub struct DevToolsViewState {
    /// Currently active sub-panel.
    pub active_panel: DevToolsPanel,

    /// Widget inspector tree state (also contains layout explorer data).
    pub inspector: InspectorState,

    /// Current debug overlay states (synced from VM Service).
    pub overlay_repaint_rainbow: bool,
    pub overlay_debug_paint: bool,
    pub overlay_performance: bool,

    /// Last VM Service connection error message, if any.
    /// Set on `VmServiceConnectionFailed`, cleared on `VmServiceConnected`.
    /// Displayed in DevTools panels so users see actionable errors instead of
    /// the generic "VM Service not connected" message.
    pub vm_connection_error: Option<String>,

    /// Rich VM Service connection status (Phase 5, Task 02).
    ///
    /// Tracks connected / disconnected / reconnecting / timed-out states so
    /// the TUI can display colour-coded indicators in the DevTools tab bar
    /// and show appropriate messages in each panel.
    ///
    /// Updated by the handler in response to VM Service lifecycle messages:
    /// - `VmServiceConnected`    → `Connected`
    /// - `VmServiceDisconnected` → `Disconnected`
    /// - `VmServiceReconnecting` → `Reconnecting { attempt, max_attempts }`
    /// - `WidgetTreeFetchTimeout` / `LayoutDataFetchTimeout` → `TimedOut`
    pub connection_status: VmConnectionStatus,

    /// Timestamp of the last debug overlay toggle.
    ///
    /// Used to debounce rapid key presses: overlay toggle RPCs are suppressed
    /// if the last toggle occurred within 500 ms. This prevents multiple
    /// in-flight RPC calls when the user holds down the toggle key.
    pub last_overlay_toggle: Option<Instant>,
}

impl DevToolsViewState {
    /// Reset all session-specific DevTools state.
    ///
    /// Called when the user switches between sessions so that stale data
    /// from the previous session is not displayed for the new session.
    ///
    /// NOTE: `active_panel` is intentionally preserved — the user's panel
    /// choice (Inspector / Performance) persists across session switches
    /// as it is a UI preference, not session data.
    pub fn reset(&mut self) {
        self.inspector.reset();
        self.overlay_repaint_rainbow = false;
        self.overlay_debug_paint = false;
        self.overlay_performance = false;
        self.vm_connection_error = None;
        self.connection_status = VmConnectionStatus::Disconnected;
        self.last_overlay_toggle = None;
    }

    /// Returns `true` if the overlay toggle debounce cooldown (500 ms) has
    /// not yet elapsed since the last toggle.
    ///
    /// When this returns `true` the caller should suppress the RPC and not
    /// update `last_overlay_toggle`.
    pub fn is_overlay_toggle_debounced(&self) -> bool {
        const DEBOUNCE: std::time::Duration = std::time::Duration::from_millis(500);
        self.last_overlay_toggle
            .map(|t| t.elapsed() < DEBOUNCE)
            .unwrap_or(false)
    }

    /// Record that an overlay toggle was just dispatched.
    ///
    /// Updates `last_overlay_toggle` to `Instant::now()` so that the next
    /// call within 500 ms will be suppressed by [`Self::is_overlay_toggle_debounced`].
    pub fn record_overlay_toggle(&mut self) {
        self.last_overlay_toggle = Some(Instant::now());
    }
}

/// State for the settings panel view
#[derive(Debug, Clone)]
pub struct SettingsViewState {
    /// Currently active tab
    pub active_tab: SettingsTab,

    /// Currently selected item index within the active tab
    pub selected_index: usize,

    /// Whether we're in edit mode for the current item
    pub editing: bool,

    /// Text buffer for string editing
    pub edit_buffer: String,

    /// Dirty flag - have settings been modified?
    pub dirty: bool,

    /// Loaded user preferences (for User tab)
    pub user_prefs: UserPreferences,

    /// Error message to display (if any)
    pub error: Option<String>,

    /// Active dart defines modal overlay (if any).
    ///
    /// Set when the user opens the dart defines editor for a launch config.
    pub dart_defines_modal: Option<DartDefinesModalState>,

    /// The 0-based index of the launch config currently being edited in the
    /// dart defines modal. Set on open, cleared on close.
    pub editing_config_idx: Option<usize>,

    /// Active extra args fuzzy modal overlay (if any).
    ///
    /// Set when the user opens the extra args picker for a launch config.
    pub extra_args_modal: Option<FuzzyModalState>,
}

impl Default for SettingsViewState {
    fn default() -> Self {
        Self {
            active_tab: SettingsTab::Project,
            selected_index: 0,
            editing: false,
            edit_buffer: String::new(),
            dirty: false,
            user_prefs: UserPreferences::default(),
            error: None,
            dart_defines_modal: None,
            editing_config_idx: None,
            extra_args_modal: None,
        }
    }
}

impl SettingsViewState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns `true` if any modal overlay is currently open.
    ///
    /// Used by the settings panel key handler to route input to the active modal
    /// instead of the underlying settings list.
    pub fn has_modal_open(&self) -> bool {
        self.dart_defines_modal.is_some() || self.extra_args_modal.is_some()
    }

    /// Load user preferences from disk
    pub fn load_user_prefs(&mut self, project_path: &std::path::Path) {
        if let Some(prefs) = crate::config::load_user_preferences(project_path) {
            self.user_prefs = prefs;
        }
    }

    /// Switch to next tab
    pub fn next_tab(&mut self) {
        self.active_tab = self.active_tab.next();
        self.selected_index = 0;
        self.editing = false;
        self.edit_buffer.clear();
    }

    /// Switch to previous tab
    pub fn prev_tab(&mut self) {
        self.active_tab = self.active_tab.prev();
        self.selected_index = 0;
        self.editing = false;
        self.edit_buffer.clear();
    }

    /// Jump to specific tab
    pub fn goto_tab(&mut self, tab: SettingsTab) {
        self.active_tab = tab;
        self.selected_index = 0;
        self.editing = false;
        self.edit_buffer.clear();
    }

    /// Select next item
    pub fn select_next(&mut self, item_count: usize) {
        if item_count > 0 {
            self.selected_index = (self.selected_index + 1) % item_count;
        }
    }

    /// Select previous item
    pub fn select_previous(&mut self, item_count: usize) {
        if item_count > 0 {
            self.selected_index = if self.selected_index == 0 {
                item_count - 1
            } else {
                self.selected_index - 1
            };
        }
    }

    /// Enter edit mode
    pub fn start_editing(&mut self, initial_value: &str) {
        self.editing = true;
        self.edit_buffer = initial_value.to_string();
    }

    /// Exit edit mode
    pub fn stop_editing(&mut self) {
        self.editing = false;
        self.edit_buffer.clear();
    }

    /// Mark settings as modified
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Clear dirty flag (after save)
    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Loading State (Phase 5 Task 08d)
// ─────────────────────────────────────────────────────────────────────────────

/// Loading messages to cycle through during device discovery (Claude Code style gerunds)
const LOADING_MESSAGES: &[&str] = &[
    "Detecting devices...",
    "Scanning for emulators...",
    "Initializing flutter daemon...",
    "Querying device connections...",
    "Waking up simulators...",
    "Consulting the device oracle...",
    "Rummaging through USB ports...",
    "Befriending nearby devices...",
    "Summoning Android spirits...",
    "Polishing iOS artifacts...",
    "Resolving adb identity crisis...",
    "Jiggling the USB cable...",
    "Bribing the operating system...",
    "Waking up the GPU hamsters...",
    "Filtering logcat noise...",
    "Paging Dr. Flutter...",
    "Ignoring deprecated warnings...",
    "Linking binary libraries...",
    "Writing an App Store appeal email...",
    "Demonizing Flutter daemon...",
    "Possesing the terminal...",
    "Negotiating with local ghosts..",
    "Calibrating flux capacitors...",
    "Flushing the socket buffers...",
    "Asking the hub for directions...",
    "Convincing the emulator it's a real phone...",
    "Interrogating system processes...",
    "Consulting the runes...",
    "Tuning the JVM...",
    "Refactoring AndroidManifest.xml...",
    "Warming up the JIT compiler...",
    "Waiting for Xcode to finish 'Indexing'...",
    "Calculating safe area insets...",
    "Convincing the simulator it has a notch...",
    "Archiving... Validating... Distributing...",
    "Awaiting the Future...",
    "Consulting Guideline 4.2...",
    "Fighting Provisioning Profiles...",
    "Calculating the 30% cut...",
    "Searching for the dSYM...",
    "Asking Siri for help...",
    "Checking IAP entitlements...",
    "Polishing the launch screen...",
    "Generating technical debt...",
    "Blaming the firewall...",
    "Sacrificing RAM to Chrome...",
    "Waiting for Internet Explorer...",
    "Loading... (fingers crossed)...",
    "Reticulating splines...",
    "Downloading Maven Central...",
    "Feeding the Gradle Daemon...",
    "Conversing with the build cache...",
    "Fumigating node_modules folder...",
    "Herding NPM packages...",
    "Orchestrating a race condition...",
    "Debugging the debugger...",
    "Demystifying the provisioning profile...",
    "Exorcising the stale cache...",
    "Arbitrating state management conflicts...",
    "Liquidating memory leaks...",
    "Gambling with hot reload...",
    "Cannibalizing system RAM...",
    "Negotiating with the garbage collector...",
    "Obfuscating spaghetti logic...",
    "Rehydrating the widget tree...",
    "Monkey-patching the framework...",
    "Consulting the dart gods...",
    "Polymorphing into a widget...",
    "Hiding Android artifacts...",
    "Hiding iOS artifacts...",
    "Optimizing the crash loop...",
    "Backporting the bugs...",
    "Injecting hot-reload magic...",
    "Overengineering 'Hello World'...",
    "Demystifying the stack trace...",
    "Siphoning user's data (allegedly)...",
    "Distributing bugs evenly...",
    "Distributing the tech debt...",
    "Distributing spaghetti code globally...",
    "Quantifying 'TODO' comments...",
    "Resolving merge conflicts with a coin toss...",
    "Git cloning node_modules...",
    "Hammering the build button...",
    "Hammering core #2...",
];

/// Loading state for startup initialization
#[derive(Debug, Clone)]
pub struct LoadingState {
    /// Current loading message
    pub message: String,
    /// Animation frame counter for spinner
    pub animation_frame: u64,
    /// Current index into LOADING_MESSAGES for cycling
    message_index: usize,
}

impl LoadingState {
    pub fn new(_message: &str) -> Self {
        // Start at a random index for variety
        let start_index = rand::thread_rng().gen_range(0..LOADING_MESSAGES.len());

        Self {
            message: LOADING_MESSAGES[start_index].to_string(),
            animation_frame: 0,
            message_index: start_index,
        }
    }

    /// Tick animation frame and optionally cycle message
    ///
    /// `cycle_messages`: If true, cycle through messages every ~15 ticks (1.5 sec at 100ms)
    pub fn tick(&mut self, cycle_messages: bool) {
        self.animation_frame = self.animation_frame.wrapping_add(1);

        if cycle_messages {
            // Cycle message every 15 frames (~1.5 seconds at 100ms tick rate)
            if self.animation_frame.is_multiple_of(15) {
                self.message_index = (self.message_index + 1) % LOADING_MESSAGES.len();
                self.message = LOADING_MESSAGES[self.message_index].to_string();
            }
        }
    }

    /// Update message
    pub fn set_message(&mut self, message: &str) {
        self.message = message.to_string();
    }
}

// ─────────────────────────────────────────────────────────────────────────────
/// Complete application state (the Model in TEA)
#[derive(Debug)]
pub struct AppState {
    /// Current UI mode/screen
    pub ui_mode: UiMode,

    /// Session manager for multi-instance support
    pub session_manager: SessionManager,

    /// Application settings from config file
    pub settings: Settings,

    /// Confirmation dialog state
    pub confirm_dialog_state: Option<ConfirmDialogState>,

    /// Project path
    pub project_path: PathBuf,

    /// Project name from pubspec.yaml (cached at startup)
    pub project_name: Option<String>,

    /// Current application phase (used for app-level quitting state)
    pub phase: AppPhase,

    /// Settings view state (for Settings UI mode)
    pub settings_view_state: SettingsViewState,

    /// New session dialog state (unified dialog)
    pub new_session_dialog_state: NewSessionDialogState,

    /// Loading state (for initial startup loading screen)
    pub loading_state: Option<LoadingState>,

    /// Global device cache (used by NewSessionDialog)
    /// Task 08e - Device Cache Sharing
    pub device_cache: Option<Vec<Device>>,

    /// When devices were last discovered (for cache invalidation)
    /// Task 08e - Device Cache Sharing
    pub devices_last_updated: Option<std::time::Instant>,

    /// Bootable device cache - iOS simulators (Bug Fix: Task 03)
    pub ios_simulators_cache: Option<Vec<IosSimulator>>,

    /// Bootable device cache - Android AVDs (Bug Fix: Task 03)
    pub android_avds_cache: Option<Vec<AndroidAvd>>,

    /// When bootable devices were last discovered (for cache invalidation)
    /// Bug Fix: Task 03 - Bootable Device Caching
    pub bootable_last_updated: Option<std::time::Instant>,

    /// Cached tool availability (checked at startup)
    /// Phase 4, Task 05 - Discovery Integration
    pub tool_availability: ToolAvailability,

    /// DevTools mode view state (Phase 4 DevTools Integration)
    pub devtools_view_state: DevToolsViewState,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    /// Create a new AppState with default settings (for backward compatibility)
    pub fn new() -> Self {
        Self::with_settings(PathBuf::new(), Settings::default())
    }

    /// Create a new AppState with project path and settings
    pub fn with_settings(project_path: PathBuf, settings: Settings) -> Self {
        // Parse project name from pubspec.yaml
        let project_name = fdemon_core::get_project_name(&project_path);

        Self {
            ui_mode: UiMode::Normal,
            session_manager: SessionManager::new(),
            settings,
            confirm_dialog_state: None,
            project_path,
            project_name,
            phase: AppPhase::Initializing,
            settings_view_state: SettingsViewState::new(),
            new_session_dialog_state: NewSessionDialogState::new(LoadedConfigs::default()),
            loading_state: None,
            device_cache: None,
            devices_last_updated: None,
            ios_simulators_cache: None,
            android_avds_cache: None,
            bootable_last_updated: None,
            tool_availability: ToolAvailability::default(),
            devtools_view_state: DevToolsViewState::default(),
        }
    }

    // ─────────────────────────────────────────────────────────
    // UI Mode Helpers
    // ─────────────────────────────────────────────────────────

    /// Show settings panel
    pub fn show_settings(&mut self) {
        self.settings_view_state = SettingsViewState::new();
        self.settings_view_state.load_user_prefs(&self.project_path);
        self.ui_mode = UiMode::Settings;
    }

    /// Hide settings panel
    pub fn hide_settings(&mut self) {
        self.ui_mode = UiMode::Normal;
    }

    // ─────────────────────────────────────────────────────────
    // DevTools Mode Helpers (Phase 4)
    // ─────────────────────────────────────────────────────────

    /// Enter DevTools mode with the default panel.
    pub fn enter_devtools_mode(&mut self) {
        self.ui_mode = UiMode::DevTools;
    }

    /// Exit DevTools mode, return to Normal.
    pub fn exit_devtools_mode(&mut self) {
        self.ui_mode = UiMode::Normal;
    }

    /// Switch the active DevTools sub-panel.
    pub fn switch_devtools_panel(&mut self, panel: DevToolsPanel) {
        self.devtools_view_state.active_panel = panel;
    }

    /// Show the new session dialog
    pub fn show_new_session_dialog(&mut self, configs: LoadedConfigs) {
        self.new_session_dialog_state = NewSessionDialogState::new(configs);
        self.ui_mode = UiMode::NewSessionDialog;
    }

    /// Hide the new session dialog
    pub fn hide_new_session_dialog(&mut self) {
        self.ui_mode = UiMode::Normal;
    }

    /// Check if new session dialog is visible
    /// Both UiMode::Startup and UiMode::NewSessionDialog show the new session dialog
    pub fn is_new_session_dialog_visible(&self) -> bool {
        self.ui_mode == UiMode::NewSessionDialog || self.ui_mode == UiMode::Startup
    }

    /// Check if any session should prevent immediate quit
    pub fn has_running_sessions(&self) -> bool {
        self.session_manager.has_running_sessions()
    }

    /// Request application quit
    pub fn request_quit(&mut self) {
        if self.has_running_sessions() && self.settings.behavior.confirm_quit {
            // Create dialog state with session count
            let session_count = self.session_manager.running_sessions().len();
            self.confirm_dialog_state = Some(ConfirmDialogState::quit_confirmation(session_count));
            self.ui_mode = UiMode::ConfirmDialog;
        } else {
            self.phase = AppPhase::Quitting;
        }
    }

    /// Force quit without confirmation
    pub fn force_quit(&mut self) {
        self.phase = AppPhase::Quitting;
    }

    /// Confirm quit (from confirmation dialog)
    pub fn confirm_quit(&mut self) {
        self.phase = AppPhase::Quitting;
    }

    /// Cancel quit (from confirmation dialog)
    pub fn cancel_quit(&mut self) {
        self.confirm_dialog_state = None;
        self.ui_mode = UiMode::Normal;
    }

    /// Check if the app should quit
    pub fn should_quit(&self) -> bool {
        self.phase == AppPhase::Quitting
    }

    // ─────────────────────────────────────────────────────────
    // Loading State Helpers (Task 08d)
    // ─────────────────────────────────────────────────────────

    /// Set loading phase with message
    pub fn set_loading_phase(&mut self, message: &str) {
        self.loading_state = Some(LoadingState::new(message));
        self.ui_mode = UiMode::Loading;
    }

    /// Update loading message
    pub fn update_loading_message(&mut self, message: &str) {
        if let Some(ref mut loading) = self.loading_state {
            loading.set_message(message);
        }
    }

    /// Clear loading state
    pub fn clear_loading(&mut self) {
        self.loading_state = None;
        if self.ui_mode == UiMode::Loading {
            self.ui_mode = UiMode::Normal;
        }
    }

    /// Tick loading animation with optional message cycling
    ///
    /// `cycle_messages`: If true, cycle through messages every ~15 ticks (1.5 sec at 100ms)
    pub fn tick_loading_animation_with_cycling(&mut self, cycle_messages: bool) {
        if let Some(ref mut loading) = self.loading_state {
            loading.tick(cycle_messages);
        }
    }

    /// Tick loading animation (no message cycling - backward compat)
    pub fn tick_loading_animation(&mut self) {
        self.tick_loading_animation_with_cycling(false);
    }

    // ─────────────────────────────────────────────────────────
    // Device Cache Helpers (Task 08e)
    // ─────────────────────────────────────────────────────────

    /// Get cached devices if fresh enough (within TTL)
    ///
    /// Cache is considered valid for 30 seconds to balance freshness with responsiveness.
    /// Device list changes are rare (device connects/disconnects) so this is a safe tradeoff.
    pub fn get_cached_devices(&self) -> Option<&Vec<Device>> {
        // Cache TTL of 30 seconds
        const CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(30);

        if let (Some(ref devices), Some(updated)) = (&self.device_cache, self.devices_last_updated)
        {
            if updated.elapsed() < CACHE_TTL {
                return Some(devices);
            }
        }
        None
    }

    /// Update device cache with fresh devices
    ///
    /// Called after successful device discovery to cache results globally.
    /// Both DeviceSelector and StartupDialog use this shared cache.
    pub fn set_device_cache(&mut self, devices: Vec<Device>) {
        self.device_cache = Some(devices);
        self.devices_last_updated = Some(std::time::Instant::now());
    }

    // ─────────────────────────────────────────────────────────
    // Bootable Device Cache Helpers (Bug Fix: Task 03)
    // ─────────────────────────────────────────────────────────

    /// Get cached bootable devices if fresh enough (within TTL)
    ///
    /// Returns both iOS simulators and Android AVDs from cache if valid.
    /// Cache is considered valid for 30 seconds to balance freshness with responsiveness.
    /// Bootable device changes are rare (simulator/AVD creation/deletion) so this is a safe tradeoff.
    pub fn get_cached_bootable_devices(&self) -> Option<(Vec<IosSimulator>, Vec<AndroidAvd>)> {
        // Cache TTL of 30 seconds (same as connected devices)
        const CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(30);

        if let (Some(ref simulators), Some(ref avds), Some(updated)) = (
            &self.ios_simulators_cache,
            &self.android_avds_cache,
            self.bootable_last_updated,
        ) {
            if updated.elapsed() < CACHE_TTL {
                return Some((simulators.clone(), avds.clone()));
            }
        }
        None
    }

    /// Update the bootable device cache with fresh results
    ///
    /// Called after successful bootable device discovery to cache results globally.
    /// The NewSessionDialog uses this shared cache to show bootable devices instantly.
    pub fn set_bootable_cache(&mut self, simulators: Vec<IosSimulator>, avds: Vec<AndroidAvd>) {
        self.ios_simulators_cache = Some(simulators);
        self.android_avds_cache = Some(avds);
        self.bootable_last_updated = Some(std::time::Instant::now());
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ─────────────────────────────────────────────────────────
    // DevTools State Tests (Phase 4, Task 01)
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_enter_exit_devtools_mode() {
        let mut state = AppState::new();
        state.ui_mode = UiMode::Normal;
        state.enter_devtools_mode();
        assert_eq!(state.ui_mode, UiMode::DevTools);
        state.exit_devtools_mode();
        assert_eq!(state.ui_mode, UiMode::Normal);
    }

    #[test]
    fn test_switch_devtools_panel() {
        let mut state = AppState::new();
        assert_eq!(
            state.devtools_view_state.active_panel,
            DevToolsPanel::Inspector
        );
        state.switch_devtools_panel(DevToolsPanel::Performance);
        assert_eq!(
            state.devtools_view_state.active_panel,
            DevToolsPanel::Performance
        );
        state.switch_devtools_panel(DevToolsPanel::Inspector);
        assert_eq!(
            state.devtools_view_state.active_panel,
            DevToolsPanel::Inspector
        );
    }

    #[test]
    fn test_inspector_state_toggle_expanded() {
        let mut inspector = InspectorState::new();
        assert!(!inspector.is_expanded("widget-1"));
        inspector.toggle_expanded("widget-1");
        assert!(inspector.is_expanded("widget-1"));
        inspector.toggle_expanded("widget-1");
        assert!(!inspector.is_expanded("widget-1"));
    }

    #[test]
    fn test_inspector_state_reset() {
        let mut inspector = InspectorState::new();
        inspector.selected_index = 5;
        inspector.expanded.insert("widget-1".to_string());
        inspector.loading = true;
        inspector.reset();
        assert_eq!(inspector.selected_index, 0);
        assert!(inspector.expanded.is_empty());
        assert!(!inspector.loading);
        assert!(inspector.root.is_none());
    }

    #[test]
    fn test_devtools_panel_default_is_inspector() {
        assert_eq!(DevToolsPanel::default(), DevToolsPanel::Inspector);
    }

    #[test]
    fn test_devtools_view_state_default() {
        let state = DevToolsViewState::default();
        assert_eq!(state.active_panel, DevToolsPanel::Inspector);
        assert!(!state.overlay_repaint_rainbow);
        assert!(!state.overlay_debug_paint);
        assert!(!state.overlay_performance);
        assert!(state.last_overlay_toggle.is_none());
    }

    #[test]
    fn test_overlay_toggle_debounce_initially_false() {
        let state = DevToolsViewState::default();
        assert!(
            !state.is_overlay_toggle_debounced(),
            "Debounce should be false when no toggle has occurred"
        );
    }

    #[test]
    fn test_overlay_toggle_debounce_active_after_record() {
        let mut state = DevToolsViewState::default();
        state.record_overlay_toggle();
        assert!(
            state.is_overlay_toggle_debounced(),
            "Debounce should be active immediately after recording a toggle"
        );
    }

    #[test]
    fn test_overlay_toggle_debounce_cleared_on_reset() {
        let mut state = DevToolsViewState::default();
        state.record_overlay_toggle();
        assert!(state.is_overlay_toggle_debounced());

        state.reset();
        assert!(
            state.last_overlay_toggle.is_none(),
            "reset() should clear last_overlay_toggle"
        );
        assert!(
            !state.is_overlay_toggle_debounced(),
            "Debounce should be inactive after reset"
        );
    }

    // ─────────────────────────────────────────────────────────
    // selected_node_description Tests (Task 06)
    // ─────────────────────────────────────────────────────────

    /// Build a three-node tree: root → child-1 → child-2.
    /// The root is auto-expanded so that all three nodes are visible.
    fn make_tree_with_three_nodes() -> DiagnosticsNode {
        DiagnosticsNode {
            description: "RootNode".to_string(),
            value_id: Some("root-id".to_string()),
            children: vec![DiagnosticsNode {
                description: "SecondNode".to_string(),
                value_id: Some("child-1-id".to_string()),
                children: vec![DiagnosticsNode {
                    description: "ThirdNode".to_string(),
                    value_id: Some("child-2-id".to_string()),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    fn make_single_node() -> DiagnosticsNode {
        DiagnosticsNode {
            description: "SingleNode".to_string(),
            value_id: Some("single-id".to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn test_selected_node_description_empty_tree() {
        let inspector = InspectorState::default();
        assert!(inspector.selected_node_description().is_none());
    }

    #[test]
    fn test_selected_node_description_returns_root_when_index_zero() {
        let mut inspector = InspectorState::default();
        inspector.root = Some(make_tree_with_three_nodes());

        let desc = inspector.selected_node_description();
        assert_eq!(desc.as_deref(), Some("RootNode"));
    }

    #[test]
    fn test_selected_node_description_returns_correct_node() {
        let mut inspector = InspectorState::default();
        inspector.root = Some(make_tree_with_three_nodes());
        // Expand root and first child so that all three nodes are visible.
        inspector.expanded.insert("root-id".to_string());
        inspector.expanded.insert("child-1-id".to_string());
        inspector.selected_index = 1;

        let desc = inspector.selected_node_description();
        assert_eq!(desc.as_deref(), Some("SecondNode"));
    }

    #[test]
    fn test_selected_node_description_third_node() {
        let mut inspector = InspectorState::default();
        inspector.root = Some(make_tree_with_three_nodes());
        inspector.expanded.insert("root-id".to_string());
        inspector.expanded.insert("child-1-id".to_string());
        inspector.selected_index = 2;

        let desc = inspector.selected_node_description();
        assert_eq!(desc.as_deref(), Some("ThirdNode"));
    }

    #[test]
    fn test_selected_node_description_index_out_of_bounds() {
        let mut inspector = InspectorState::default();
        inspector.root = Some(make_single_node());
        inspector.selected_index = 99;
        assert!(inspector.selected_node_description().is_none());
    }

    #[test]
    fn test_selected_node_description_collapsed_children_not_counted() {
        let mut inspector = InspectorState::default();
        inspector.root = Some(make_tree_with_three_nodes());
        // Root is NOT expanded — children are hidden, so only root is visible.
        inspector.selected_index = 1; // index 1 is out of range

        // Only root visible (index 0), index 1 should return None.
        assert!(inspector.selected_node_description().is_none());
    }

    #[test]
    fn test_selected_node_description_no_allocation_path_matches_visible_nodes() {
        // Verify that selected_node_description agrees with visible_nodes().
        let mut inspector = InspectorState::default();
        inspector.root = Some(make_tree_with_three_nodes());
        inspector.expanded.insert("root-id".to_string());
        inspector.expanded.insert("child-1-id".to_string());

        // Collect descriptions from visible_nodes() first to drop the borrow
        // before we mutate selected_index.
        let descriptions: Vec<String> = inspector
            .visible_nodes()
            .into_iter()
            .map(|(node, _)| node.description.clone())
            .collect();

        for (i, expected) in descriptions.iter().enumerate() {
            inspector.selected_index = i;
            let desc = inspector.selected_node_description();
            assert_eq!(
                desc.as_deref(),
                Some(expected.as_str()),
                "Mismatch at index {i}"
            );
        }
    }

    // Helper to create a test device
    fn test_device(id: &str, name: &str) -> Device {
        Device {
            id: id.to_string(),
            name: name.to_string(),
            platform: "ios".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        }
    }

    // Old StartupDialog and DialogSection tests removed - replaced by NewSessionDialog

    // ─────────────────────────────────────────────────────────
    // Loading State Tests (Task 08d)
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_loading_state_creation() {
        let loading = LoadingState::new("Test message");
        // Should start with one of the LOADING_MESSAGES, not the passed message
        assert!(LOADING_MESSAGES.contains(&loading.message.as_str()));
        assert_eq!(loading.animation_frame, 0);
    }

    #[test]
    fn test_loading_state_tick() {
        let mut loading = LoadingState::new("Test");
        loading.tick(false);
        assert_eq!(loading.animation_frame, 1);
        loading.tick(false);
        assert_eq!(loading.animation_frame, 2);
    }

    #[test]
    fn test_loading_state_tick_wraps() {
        let mut loading = LoadingState::new("Test");
        loading.animation_frame = u64::MAX;
        loading.tick(false);
        assert_eq!(loading.animation_frame, 0);
    }

    #[test]
    fn test_loading_state_random_start() {
        // Run multiple times to verify randomness (statistically)
        let mut seen_indices: std::collections::HashSet<String> = std::collections::HashSet::new();

        for _ in 0..20 {
            let loading = LoadingState::new("ignored");
            seen_indices.insert(loading.message.clone());
        }

        // With 10 messages and 20 trials, we should see multiple different starting messages
        assert!(
            seen_indices.len() > 1,
            "Should have random starting messages, saw {} unique messages",
            seen_indices.len()
        );
    }

    #[test]
    fn test_loading_state_message_cycling() {
        let mut loading = LoadingState::new("ignored");
        let initial_message = loading.message.clone();

        // First 14 ticks - no change (cycle at 15)
        for _ in 0..14 {
            loading.tick(true);
        }
        assert_eq!(loading.message, initial_message);

        // 12th tick - first cycle
        loading.tick(true);
        assert_ne!(
            loading.message, initial_message,
            "Message should change after 15 ticks"
        );

        // After 30 total ticks - should be on third message
        let second_message = loading.message.clone();
        for _ in 0..15 {
            loading.tick(true);
        }
        // Message should have changed again
        assert_ne!(loading.message, second_message);
    }

    #[test]
    fn test_loading_state_wraps_around() {
        let mut loading = LoadingState::new("ignored");
        let start_message = loading.message.clone();

        // Cycle through all 84 messages (84 * 15 = 1260 ticks)
        for _ in 0..1260 {
            loading.tick(true);
        }

        // Should have wrapped back to starting message
        assert_eq!(loading.message, start_message);
    }

    #[test]
    fn test_loading_spinner_speed() {
        let mut loading = LoadingState::new("Test");
        let frame0 = loading.animation_frame;
        loading.tick(false);
        assert_eq!(loading.animation_frame, frame0 + 1);
    }

    #[test]
    fn test_loading_no_cycle_when_disabled() {
        let mut loading = LoadingState::new("ignored");
        let initial_message = loading.message.clone();

        // Tick without cycling
        for _ in 0..50 {
            loading.tick(false);
        }

        assert_eq!(
            loading.message, initial_message,
            "Message should not change when cycling disabled"
        );
    }

    #[test]
    fn test_loading_state_set_message() {
        let mut loading = LoadingState::new("Initial");
        loading.set_message("Updated");
        assert_eq!(loading.message, "Updated");
    }

    #[test]
    fn test_app_state_set_loading_phase() {
        let mut state = AppState::new();
        state.set_loading_phase("Loading...");

        assert_eq!(state.ui_mode, UiMode::Loading);
        assert!(state.loading_state.is_some());
        // Message will be one of LOADING_MESSAGES (random start), not the passed message
        assert!(LOADING_MESSAGES.contains(&state.loading_state.as_ref().unwrap().message.as_str()));
    }

    #[test]
    fn test_app_state_update_loading_message() {
        let mut state = AppState::new();
        state.set_loading_phase("Initial");
        state.update_loading_message("Updated");

        assert!(state.loading_state.is_some());
        assert_eq!(state.loading_state.as_ref().unwrap().message, "Updated");
    }

    #[test]
    fn test_app_state_clear_loading() {
        let mut state = AppState::new();
        state.set_loading_phase("Loading...");

        state.clear_loading();

        assert!(state.loading_state.is_none());
        assert_eq!(state.ui_mode, UiMode::Normal);
    }

    #[test]
    fn test_app_state_tick_loading_animation() {
        let mut state = AppState::new();
        state.set_loading_phase("Loading...");

        state.tick_loading_animation();

        assert_eq!(state.loading_state.as_ref().unwrap().animation_frame, 1);
    }

    #[test]
    fn test_app_state_tick_loading_no_state() {
        let mut state = AppState::new();
        // Should not panic when there's no loading state
        state.tick_loading_animation();
        assert!(state.loading_state.is_none());
    }

    #[test]
    fn test_app_state_update_loading_no_state() {
        let mut state = AppState::new();
        // Should not panic when there's no loading state
        state.update_loading_message("Test");
        assert!(state.loading_state.is_none());
    }

    // ─────────────────────────────────────────────────────────
    // Device Cache Tests (Task 08e)
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_device_cache_shared() {
        let mut state = AppState::new();

        // Simulate initial discovery
        let devices = vec![test_device("dev1", "Device 1")];
        state.set_device_cache(devices.clone());

        // Device cache is now available for use
        assert!(state.get_cached_devices().is_some());
        assert_eq!(state.get_cached_devices().unwrap().len(), 1);
    }

    #[test]
    fn test_device_cache_fresh() {
        let mut state = AppState::new();
        state.set_device_cache(vec![test_device("dev1", "Device 1")]);

        // Fresh cache should be available
        assert!(state.get_cached_devices().is_some());
        assert_eq!(state.get_cached_devices().unwrap().len(), 1);
    }

    #[test]
    fn test_device_cache_expires() {
        let mut state = AppState::new();
        state.set_device_cache(vec![test_device("dev1", "Device 1")]);

        // Fresh cache
        assert!(state.get_cached_devices().is_some());

        // Expired cache (mock time travel by manually setting timestamp)
        state.devices_last_updated =
            Some(std::time::Instant::now() - std::time::Duration::from_secs(60));
        assert!(state.get_cached_devices().is_none());
    }

    #[test]
    fn test_device_cache_none_initially() {
        let state = AppState::new();
        assert!(state.get_cached_devices().is_none());
        assert!(state.device_cache.is_none());
        assert!(state.devices_last_updated.is_none());
    }

    #[test]
    fn test_device_cache_updates_timestamp() {
        let mut state = AppState::new();

        let before = std::time::Instant::now();
        state.set_device_cache(vec![test_device("dev1", "Device 1")]);
        let after = std::time::Instant::now();

        assert!(state.devices_last_updated.is_some());
        let timestamp = state.devices_last_updated.unwrap();

        // Timestamp should be between before and after
        assert!(timestamp >= before);
        assert!(timestamp <= after);
    }

    #[test]
    fn test_device_cache_replaces_old() {
        let mut state = AppState::new();

        // Initial cache
        state.set_device_cache(vec![test_device("dev1", "Device 1")]);
        assert_eq!(state.device_cache.as_ref().unwrap().len(), 1);

        // Update with new devices
        state.set_device_cache(vec![
            test_device("dev1", "Device 1"),
            test_device("dev2", "Device 2"),
        ]);
        assert_eq!(state.device_cache.as_ref().unwrap().len(), 2);
    }

    // Old StartupDialogState tests removed - replaced by NewSessionDialog tests

    // ─────────────────────────────────────────────────────────
    // NewSessionDialog Tests (Task 05)
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_new_session_dialog_visibility() {
        let mut state = AppState::new();
        assert!(!state.is_new_session_dialog_visible());

        state.show_new_session_dialog(LoadedConfigs::default());
        assert!(state.is_new_session_dialog_visible());
        assert_eq!(state.ui_mode, UiMode::NewSessionDialog);

        state.hide_new_session_dialog();
        assert!(!state.is_new_session_dialog_visible());
        assert_eq!(state.ui_mode, UiMode::Normal);
    }

    #[test]
    fn test_startup_mode_is_dialog_visible() {
        // UiMode::Startup also shows the new session dialog
        let mut state = AppState::new();
        state.ui_mode = UiMode::Startup;
        assert!(state.is_new_session_dialog_visible());
    }

    // ─────────────────────────────────────────────────────────
    // Cache Preload Tests (Moved to handler tests - Task 01)
    // These tests have been moved to app/handler/new_session/navigation.rs
    // because cache checking is now done in the handler, not in show_new_session_dialog().
    // This follows TEA principles where state methods are pure and handlers contain logic.
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_show_new_session_dialog_does_not_populate_cache() {
        let mut state = AppState::new();
        let configs = LoadedConfigs::default();

        // Simulate cached devices
        let devices = vec![
            test_device("device1", "Test Device 1"),
            test_device("device2", "Test Device 2"),
        ];
        state.set_device_cache(devices.clone());

        // Open dialog - should NOT populate from cache (handler does this)
        state.show_new_session_dialog(configs);

        // Verify devices are NOT pre-populated (handler responsibility)
        assert_eq!(
            state
                .new_session_dialog_state
                .target_selector
                .connected_devices
                .len(),
            0
        );
    }

    #[test]
    fn test_show_new_session_dialog_sets_ui_mode() {
        let mut state = AppState::new();
        let configs = LoadedConfigs::default();

        // Open dialog
        state.show_new_session_dialog(configs);

        // Verify UI mode is set
        assert_eq!(state.ui_mode, UiMode::NewSessionDialog);
    }

    // These cache tests have been moved to handler tests because
    // cache population is now done in handle_open_new_session_dialog(),
    // not in show_new_session_dialog(). This follows TEA principles.

    // ─────────────────────────────────────────────────────────
    // Bootable Device Cache Tests (Bug Fix: Task 03)
    // ─────────────────────────────────────────────────────────

    // Helper to create a test iOS simulator
    fn test_ios_simulator(udid: &str, name: &str) -> IosSimulator {
        IosSimulator {
            udid: udid.to_string(),
            name: name.to_string(),
            runtime: "iOS 17.2".to_string(),
            state: fdemon_daemon::SimulatorState::Shutdown,
            device_type: "iPhone 15".to_string(),
        }
    }

    // Helper to create a test Android AVD
    fn test_android_avd(name: &str) -> AndroidAvd {
        AndroidAvd {
            name: name.to_string(),
            display_name: format!("{} Display", name),
            api_level: Some(33),
            target: Some("android-33".to_string()),
        }
    }

    #[test]
    fn test_set_bootable_cache() {
        let mut state = AppState::default();
        let simulators = vec![test_ios_simulator("test-udid", "iPhone 15")];
        let avds = vec![test_android_avd("Pixel_7")];

        state.set_bootable_cache(simulators.clone(), avds.clone());

        assert!(state.ios_simulators_cache.is_some());
        assert!(state.android_avds_cache.is_some());
        assert!(state.bootable_last_updated.is_some());
        assert_eq!(state.ios_simulators_cache.as_ref().unwrap().len(), 1);
        assert_eq!(state.android_avds_cache.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn test_get_cached_bootable_devices_valid() {
        let mut state = AppState::default();
        let simulators = vec![test_ios_simulator("test-udid", "iPhone 15")];
        let avds = vec![test_android_avd("Pixel_7")];
        state.set_bootable_cache(simulators.clone(), avds.clone());

        let cached = state.get_cached_bootable_devices();
        assert!(cached.is_some());
        let (s, a) = cached.unwrap();
        assert_eq!(s.len(), 1);
        assert_eq!(a.len(), 1);
        assert_eq!(s[0].name, "iPhone 15");
        assert_eq!(a[0].name, "Pixel_7");
    }

    #[test]
    fn test_get_cached_bootable_devices_empty_when_not_set() {
        let state = AppState::default();
        let cached = state.get_cached_bootable_devices();
        assert!(cached.is_none());
    }

    // Bootable cache tests have been moved to handler tests because
    // cache population is now done in handle_open_new_session_dialog(),
    // not in show_new_session_dialog(). This follows TEA principles.

    // ─────────────────────────────────────────────────────────
    // SettingsViewState modal tests (v1-refinements Phase 2, Task 02)
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_settings_view_state_has_modal_open() {
        use crate::new_session_dialog::{FuzzyModalState, FuzzyModalType};

        let mut state = SettingsViewState::new();
        assert!(!state.has_modal_open());

        state.extra_args_modal = Some(FuzzyModalState::new(
            FuzzyModalType::ExtraArgs,
            vec!["--verbose".to_string()],
        ));
        assert!(state.has_modal_open());
    }

    #[test]
    fn test_settings_view_state_has_modal_open_dart_defines() {
        use crate::new_session_dialog::{DartDefine, DartDefinesModalState};

        let mut state = SettingsViewState::new();
        assert!(!state.has_modal_open());

        state.dart_defines_modal = Some(DartDefinesModalState::new(vec![DartDefine::new(
            "ENV", "dev",
        )]));
        assert!(state.has_modal_open());
    }

    #[test]
    fn test_settings_view_state_both_modals_none_by_default() {
        let state = SettingsViewState::new();
        assert!(state.dart_defines_modal.is_none());
        assert!(state.extra_args_modal.is_none());
        assert!(!state.has_modal_open());
    }
}
