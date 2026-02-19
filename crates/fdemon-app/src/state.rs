//! Application state (Model in TEA pattern)

use std::collections::HashSet;
use std::path::PathBuf;

use rand::Rng;

use crate::config::{LoadedConfigs, Settings, SettingsTab, UserPreferences};
use crate::confirm_dialog::ConfirmDialogState;
use crate::new_session_dialog::NewSessionDialogState;
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

    /// DevTools panel mode - replaces log view with Inspector/Layout/Performance panels
    DevTools,
}

// ─────────────────────────────────────────────────────────────────────────────
// DevTools State (Phase 4)
// ─────────────────────────────────────────────────────────────────────────────

/// Active sub-panel within DevTools mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DevToolsPanel {
    /// Widget tree inspector with expand/collapse navigation.
    #[default]
    Inspector,

    /// Flex layout visualization for the selected widget.
    Layout,

    /// FPS, memory usage, and frame timing display.
    Performance,
}

/// State for the widget inspector tree view.
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

    /// Error message from the last failed fetch attempt.
    pub error: Option<String>,
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
}

/// State for the layout explorer panel.
#[derive(Debug, Clone, Default)]
pub struct LayoutExplorerState {
    /// Layout info for the currently selected widget.
    pub layout: Option<LayoutInfo>,

    /// Whether a layout fetch is in progress.
    pub loading: bool,

    /// Error from the last failed fetch.
    pub error: Option<String>,
}

/// Complete state for the DevTools mode UI.
#[derive(Debug, Clone, Default)]
pub struct DevToolsViewState {
    /// Currently active sub-panel.
    pub active_panel: DevToolsPanel,

    /// Widget inspector tree state.
    pub inspector: InspectorState,

    /// Layout explorer state.
    pub layout_explorer: LayoutExplorerState,

    /// Current debug overlay states (synced from VM Service).
    pub overlay_repaint_rainbow: bool,
    pub overlay_debug_paint: bool,
    pub overlay_performance: bool,
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
        }
    }
}

impl SettingsViewState {
    pub fn new() -> Self {
        Self::default()
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
        state.switch_devtools_panel(DevToolsPanel::Layout);
        assert_eq!(
            state.devtools_view_state.active_panel,
            DevToolsPanel::Layout
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
}
