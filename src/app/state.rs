//! Application state (Model in TEA pattern)

use std::path::PathBuf;

use crate::config::{
    FlutterMode, LoadedConfigs, Settings, SettingsTab, SourcedConfig, UserPreferences,
};
use crate::core::AppPhase;
use crate::daemon::Device;
use crate::tui::widgets::{ConfirmDialogState, DeviceSelectorState};

use super::session_manager::SessionManager;

/// Current UI mode/screen
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UiMode {
    /// Normal TUI with log view and status bar
    #[default]
    Normal,

    /// Device selector modal is active
    DeviceSelector,

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

    /// Startup dialog - comprehensive session launch UI
    /// Shows config selection, mode, flavor, dart-defines, and device list
    StartupDialog,
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

/// Loading state for startup initialization
#[derive(Debug, Clone)]
pub struct LoadingState {
    /// Current loading message
    pub message: String,
    /// Animation frame counter for spinner
    pub animation_frame: u64,
}

impl LoadingState {
    pub fn new(message: &str) -> Self {
        Self {
            message: message.to_string(),
            animation_frame: 0,
        }
    }

    /// Tick animation frame
    pub fn tick(&mut self) {
        self.animation_frame = self.animation_frame.wrapping_add(1);
    }

    /// Update message
    pub fn set_message(&mut self, message: &str) {
        self.message = message.to_string();
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Startup Dialog State (Phase 5)
// ─────────────────────────────────────────────────────────────────────────────

/// Which section of the startup dialog is focused
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DialogSection {
    #[default]
    Configs, // Launch config selection
    Mode,        // Debug/Profile/Release
    Flavor,      // Flavor text input
    DartDefines, // Dart-define text input
    Devices,     // Device selection
}

impl DialogSection {
    pub fn next(&self) -> Self {
        match self {
            Self::Configs => Self::Mode,
            Self::Mode => Self::Flavor,
            Self::Flavor => Self::DartDefines,
            Self::DartDefines => Self::Devices,
            Self::Devices => Self::Configs,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            Self::Configs => Self::Devices,
            Self::Mode => Self::Configs,
            Self::Flavor => Self::Mode,
            Self::DartDefines => Self::Flavor,
            Self::Devices => Self::DartDefines,
        }
    }
}

/// State for the startup dialog
#[derive(Debug, Clone)]
pub struct StartupDialogState {
    /// Loaded configurations (launch.toml + launch.json)
    pub configs: LoadedConfigs,

    /// Available devices
    pub devices: Vec<Device>,

    /// Currently selected config index (None = no config, bare flutter run)
    pub selected_config: Option<usize>,

    /// Currently selected device index
    pub selected_device: Option<usize>,

    /// Selected build mode
    pub mode: FlutterMode,

    /// Flavor input (optional)
    pub flavor: String,

    /// Dart-define input (optional, format: KEY=VALUE,KEY2=VALUE2)
    pub dart_defines: String,

    /// Currently focused section
    pub active_section: DialogSection,

    /// Whether currently editing flavor/dart-defines
    pub editing: bool,

    /// Loading state (discovering devices)
    pub loading: bool,

    /// Refreshing devices in background
    pub refreshing: bool,

    /// Error message (if any)
    pub error: Option<String>,

    /// Animation frame for loading indicator
    pub animation_frame: u64,
}

impl Default for StartupDialogState {
    fn default() -> Self {
        Self {
            configs: LoadedConfigs::default(),
            devices: Vec::new(),
            selected_config: None,
            selected_device: None,
            mode: FlutterMode::Debug,
            flavor: String::new(),
            dart_defines: String::new(),
            active_section: DialogSection::Configs,
            editing: false,
            loading: true,
            refreshing: false,
            error: None,
            animation_frame: 0,
        }
    }
}

impl StartupDialogState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Initialize with configs
    pub fn with_configs(configs: LoadedConfigs) -> Self {
        let selected_config = if configs.configs.is_empty() {
            None
        } else {
            Some(0) // Select first config by default
        };

        Self {
            configs,
            selected_config,
            ..Self::default()
        }
    }

    /// Set devices after discovery
    pub fn set_devices(&mut self, devices: Vec<Device>) {
        self.devices = devices;
        self.loading = false;
        self.refreshing = false;
        self.error = None;

        // Auto-select first device if none selected
        if self.selected_device.is_none() && !self.devices.is_empty() {
            self.selected_device = Some(0);
        }
    }

    /// Set error state
    pub fn set_error(&mut self, error: String) {
        self.error = Some(error);
        self.loading = false;
        self.refreshing = false;
    }

    /// Get selected config
    pub fn selected_config(&self) -> Option<&SourcedConfig> {
        self.selected_config
            .and_then(|idx| self.configs.configs.get(idx))
    }

    /// Get selected device
    pub fn selected_device(&self) -> Option<&Device> {
        self.selected_device.and_then(|idx| self.devices.get(idx))
    }

    /// Can launch? (need device, config optional)
    pub fn can_launch(&self) -> bool {
        self.selected_device.is_some()
    }

    /// Navigate up in current section
    pub fn navigate_up(&mut self) {
        match self.active_section {
            DialogSection::Configs => {
                if let Some(idx) = self.selected_config {
                    if idx > 0 {
                        self.selected_config = Some(idx - 1);
                    } else {
                        // Wrap to end or set to None (no config)
                        self.selected_config = Some(self.configs.configs.len().saturating_sub(1));
                    }
                }
            }
            DialogSection::Mode => {
                self.mode = match self.mode {
                    FlutterMode::Debug => FlutterMode::Release,
                    FlutterMode::Profile => FlutterMode::Debug,
                    FlutterMode::Release => FlutterMode::Profile,
                };
            }
            DialogSection::Devices => {
                if let Some(idx) = self.selected_device {
                    if idx > 0 {
                        self.selected_device = Some(idx - 1);
                    } else if !self.devices.is_empty() {
                        self.selected_device = Some(self.devices.len() - 1);
                    }
                }
            }
            _ => {} // Flavor/DartDefines are text inputs
        }
    }

    /// Navigate down in current section
    pub fn navigate_down(&mut self) {
        match self.active_section {
            DialogSection::Configs => {
                if !self.configs.configs.is_empty() {
                    let max = self.configs.configs.len() - 1;
                    let current = self.selected_config.unwrap_or(0);
                    self.selected_config = Some(if current >= max { 0 } else { current + 1 });
                }
            }
            DialogSection::Mode => {
                self.mode = match self.mode {
                    FlutterMode::Debug => FlutterMode::Profile,
                    FlutterMode::Profile => FlutterMode::Release,
                    FlutterMode::Release => FlutterMode::Debug,
                };
            }
            DialogSection::Devices => {
                if !self.devices.is_empty() {
                    let max = self.devices.len() - 1;
                    let current = self.selected_device.unwrap_or(0);
                    self.selected_device = Some(if current >= max { 0 } else { current + 1 });
                }
            }
            _ => {} // Flavor/DartDefines are text inputs
        }
    }

    /// Move to next section
    pub fn next_section(&mut self) {
        self.editing = false;
        self.active_section = self.active_section.next();
    }

    /// Move to previous section
    pub fn prev_section(&mut self) {
        self.editing = false;
        self.active_section = self.active_section.prev();
    }

    /// Tick animation frame
    pub fn tick(&mut self) {
        self.animation_frame = self.animation_frame.wrapping_add(1);
    }

    /// Jump directly to a section
    pub fn jump_to_section(&mut self, section: DialogSection) {
        self.editing = false; // Exit any edit mode
        self.active_section = section;
    }

    /// Check if current section is editable (text input)
    pub fn is_text_section(&self) -> bool {
        matches!(
            self.active_section,
            DialogSection::Flavor | DialogSection::DartDefines
        )
    }

    /// Enter edit mode (only for text sections)
    pub fn enter_edit(&mut self) {
        if self.is_text_section() {
            self.editing = true;
        }
    }

    /// Exit edit mode
    pub fn exit_edit(&mut self) {
        self.editing = false;
    }

    /// Apply config defaults when config is selected
    pub fn apply_config_defaults(&mut self) {
        if let Some(config) = self.selected_config() {
            // Clone the values we need before mutating self
            let mode = config.config.mode;
            let flavor = config.config.flavor.clone();
            let dart_defines = if !config.config.dart_defines.is_empty() {
                config
                    .config
                    .dart_defines
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect::<Vec<_>>()
                    .join(",")
            } else {
                String::new()
            };

            // Now update self
            self.mode = mode;
            if let Some(flavor) = flavor {
                self.flavor = flavor;
            }
            if !dart_defines.is_empty() {
                self.dart_defines = dart_defines;
            }
        }
    }
}

/// Complete application state (the Model in TEA)
#[derive(Debug)]
pub struct AppState {
    /// Current UI mode/screen
    pub ui_mode: UiMode,

    /// Session manager for multi-instance support
    pub session_manager: SessionManager,

    /// Device selector state
    pub device_selector: DeviceSelectorState,

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

    /// Startup dialog state
    pub startup_dialog_state: StartupDialogState,

    /// Loading state (for initial startup loading screen)
    pub loading_state: Option<LoadingState>,

    /// Global device cache (shared between DeviceSelector and StartupDialog)
    /// Task 08e - Device Cache Sharing
    pub device_cache: Option<Vec<Device>>,

    /// When devices were last discovered (for cache invalidation)
    /// Task 08e - Device Cache Sharing
    pub devices_last_updated: Option<std::time::Instant>,
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
        let project_name = crate::core::get_project_name(&project_path);

        Self {
            ui_mode: UiMode::Normal,
            session_manager: SessionManager::new(),
            device_selector: DeviceSelectorState::new(),
            settings,
            confirm_dialog_state: None,
            project_path,
            project_name,
            phase: AppPhase::Initializing,
            settings_view_state: SettingsViewState::new(),
            startup_dialog_state: StartupDialogState::new(),
            loading_state: None,
            device_cache: None,
            devices_last_updated: None,
        }
    }

    // ─────────────────────────────────────────────────────────
    // UI Mode Helpers
    // ─────────────────────────────────────────────────────────

    /// Show device selector modal
    pub fn show_device_selector(&mut self) {
        self.ui_mode = UiMode::DeviceSelector;
        self.device_selector.show_loading();
    }

    /// Hide device selector modal
    pub fn hide_device_selector(&mut self) {
        self.device_selector.hide();
        self.ui_mode = UiMode::Normal;
    }

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

    /// Show startup dialog
    pub fn show_startup_dialog(&mut self, configs: LoadedConfigs) {
        self.startup_dialog_state = StartupDialogState::with_configs(configs);

        // Pre-populate with cached devices if available (Task 08e)
        // Clone the cached devices first to avoid borrow checker issues
        let cached_devices = self.get_cached_devices().cloned();
        if let Some(cached) = cached_devices {
            let is_empty = cached.is_empty();
            self.startup_dialog_state.devices = cached;
            self.startup_dialog_state.loading = false;
            self.startup_dialog_state.refreshing = true;
            if !is_empty {
                self.startup_dialog_state.selected_device = Some(0);
            }
        }

        self.ui_mode = UiMode::StartupDialog;
    }

    /// Hide startup dialog
    pub fn hide_startup_dialog(&mut self) {
        self.ui_mode = UiMode::Normal;
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

    /// Tick loading animation
    pub fn tick_loading_animation(&mut self) {
        if let Some(ref mut loading) = self.loading_state {
            loading.tick();
        }
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
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn test_dialog_section_navigation() {
        assert_eq!(DialogSection::Configs.next(), DialogSection::Mode);
        assert_eq!(DialogSection::Mode.next(), DialogSection::Flavor);
        assert_eq!(DialogSection::Flavor.next(), DialogSection::DartDefines);
        assert_eq!(DialogSection::DartDefines.next(), DialogSection::Devices);
        assert_eq!(DialogSection::Devices.next(), DialogSection::Configs);

        assert_eq!(DialogSection::Configs.prev(), DialogSection::Devices);
        assert_eq!(DialogSection::Devices.prev(), DialogSection::DartDefines);
        assert_eq!(DialogSection::DartDefines.prev(), DialogSection::Flavor);
        assert_eq!(DialogSection::Flavor.prev(), DialogSection::Mode);
        assert_eq!(DialogSection::Mode.prev(), DialogSection::Configs);
    }

    #[test]
    fn test_startup_dialog_state_defaults() {
        let state = StartupDialogState::new();

        assert!(state.loading);
        assert!(state.devices.is_empty());
        assert!(state.selected_config.is_none());
        assert_eq!(state.mode, FlutterMode::Debug);
        assert!(state.flavor.is_empty());
        assert_eq!(state.active_section, DialogSection::Configs);
    }

    #[test]
    fn test_can_launch_requires_device() {
        let mut state = StartupDialogState::new();
        assert!(!state.can_launch());

        state.set_devices(vec![test_device("test", "Test Device")]);

        assert!(state.can_launch());
    }

    #[test]
    fn test_mode_cycling() {
        let mut state = StartupDialogState::new();
        state.active_section = DialogSection::Mode;

        state.navigate_down();
        assert_eq!(state.mode, FlutterMode::Profile);

        state.navigate_down();
        assert_eq!(state.mode, FlutterMode::Release);

        state.navigate_down();
        assert_eq!(state.mode, FlutterMode::Debug);
    }

    #[test]
    fn test_mode_cycling_up() {
        let mut state = StartupDialogState::new();
        state.active_section = DialogSection::Mode;

        state.navigate_up();
        assert_eq!(state.mode, FlutterMode::Release);

        state.navigate_up();
        assert_eq!(state.mode, FlutterMode::Profile);

        state.navigate_up();
        assert_eq!(state.mode, FlutterMode::Debug);
    }

    #[test]
    fn test_set_devices_clears_loading() {
        let mut state = StartupDialogState::new();
        assert!(state.loading);

        state.set_devices(vec![test_device("test", "Test")]);

        assert!(!state.loading);
        assert!(!state.refreshing);
        assert!(state.error.is_none());
    }

    #[test]
    fn test_set_devices_auto_selects_first() {
        let mut state = StartupDialogState::new();
        assert!(state.selected_device.is_none());

        state.set_devices(vec![
            test_device("dev1", "Device 1"),
            test_device("dev2", "Device 2"),
        ]);

        assert_eq!(state.selected_device, Some(0));
        assert_eq!(state.selected_device().unwrap().name, "Device 1");
    }

    #[test]
    fn test_set_error_clears_loading() {
        let mut state = StartupDialogState::new();
        assert!(state.loading);

        state.set_error("Test error".to_string());

        assert!(!state.loading);
        assert!(!state.refreshing);
        assert_eq!(state.error, Some("Test error".to_string()));
    }

    #[test]
    fn test_next_section_clears_editing() {
        let mut state = StartupDialogState::new();
        state.editing = true;
        state.active_section = DialogSection::Flavor;

        state.next_section();

        assert!(!state.editing);
        assert_eq!(state.active_section, DialogSection::DartDefines);
    }

    #[test]
    fn test_prev_section_clears_editing() {
        let mut state = StartupDialogState::new();
        state.editing = true;
        state.active_section = DialogSection::Flavor;

        state.prev_section();

        assert!(!state.editing);
        assert_eq!(state.active_section, DialogSection::Mode);
    }

    #[test]
    fn test_tick_increments_animation_frame() {
        let mut state = StartupDialogState::new();
        assert_eq!(state.animation_frame, 0);

        state.tick();
        assert_eq!(state.animation_frame, 1);

        state.tick();
        assert_eq!(state.animation_frame, 2);
    }

    #[test]
    fn test_tick_wraps_around() {
        let mut state = StartupDialogState::new();
        state.animation_frame = u64::MAX;

        state.tick();
        assert_eq!(state.animation_frame, 0);
    }

    #[test]
    fn test_device_navigation_wraps() {
        let mut state = StartupDialogState::new();
        state.active_section = DialogSection::Devices;
        state.set_devices(vec![
            test_device("dev1", "Device 1"),
            test_device("dev2", "Device 2"),
            test_device("dev3", "Device 3"),
        ]);

        assert_eq!(state.selected_device, Some(0));

        // Navigate down wraps to start
        state.selected_device = Some(2);
        state.navigate_down();
        assert_eq!(state.selected_device, Some(0));

        // Navigate up wraps to end
        state.selected_device = Some(0);
        state.navigate_up();
        assert_eq!(state.selected_device, Some(2));
    }

    #[test]
    fn test_with_configs_selects_first() {
        let mut configs = LoadedConfigs::default();
        configs.configs.push(SourcedConfig {
            config: crate::config::LaunchConfig {
                name: "Config 1".to_string(),
                ..Default::default()
            },
            source: crate::config::ConfigSource::FDemon,
            display_name: "Config 1".to_string(),
        });

        let state = StartupDialogState::with_configs(configs);

        assert_eq!(state.selected_config, Some(0));
        assert_eq!(state.selected_config().unwrap().config.name, "Config 1");
    }

    #[test]
    fn test_with_configs_empty() {
        let configs = LoadedConfigs::default();
        let state = StartupDialogState::with_configs(configs);

        assert!(state.selected_config.is_none());
    }

    #[test]
    fn test_app_state_show_startup_dialog() {
        let mut state = AppState::new();
        assert_eq!(state.ui_mode, UiMode::Normal);

        let configs = LoadedConfigs::default();
        state.show_startup_dialog(configs);

        assert_eq!(state.ui_mode, UiMode::StartupDialog);
    }

    #[test]
    fn test_app_state_hide_startup_dialog() {
        let mut state = AppState::new();
        state.ui_mode = UiMode::StartupDialog;

        state.hide_startup_dialog();

        assert_eq!(state.ui_mode, UiMode::Normal);
    }

    // ─────────────────────────────────────────────────────────
    // Loading State Tests (Task 08d)
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_loading_state_creation() {
        let loading = LoadingState::new("Test message");
        assert_eq!(loading.message, "Test message");
        assert_eq!(loading.animation_frame, 0);
    }

    #[test]
    fn test_loading_state_tick() {
        let mut loading = LoadingState::new("Test");
        loading.tick();
        assert_eq!(loading.animation_frame, 1);
        loading.tick();
        assert_eq!(loading.animation_frame, 2);
    }

    #[test]
    fn test_loading_state_tick_wraps() {
        let mut loading = LoadingState::new("Test");
        loading.animation_frame = u64::MAX;
        loading.tick();
        assert_eq!(loading.animation_frame, 0);
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
        assert_eq!(state.loading_state.as_ref().unwrap().message, "Loading...");
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

        // Show startup dialog should use cache
        let configs = LoadedConfigs::default();
        state.show_startup_dialog(configs);

        assert!(!state.startup_dialog_state.loading);
        assert!(state.startup_dialog_state.refreshing);
        assert_eq!(state.startup_dialog_state.devices.len(), 1);
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

    #[test]
    fn test_show_startup_dialog_with_cache() {
        let mut state = AppState::new();

        // Setup cache
        let devices = vec![
            test_device("dev1", "Device 1"),
            test_device("dev2", "Device 2"),
        ];
        state.set_device_cache(devices);

        // Show startup dialog
        let configs = LoadedConfigs::default();
        state.show_startup_dialog(configs);

        // Should have cached devices, not loading, but refreshing
        assert_eq!(state.startup_dialog_state.devices.len(), 2);
        assert!(!state.startup_dialog_state.loading);
        assert!(state.startup_dialog_state.refreshing);
        assert_eq!(state.startup_dialog_state.selected_device, Some(0));
    }

    #[test]
    fn test_show_startup_dialog_without_cache() {
        let mut state = AppState::new();

        // Show startup dialog without cache
        let configs = LoadedConfigs::default();
        state.show_startup_dialog(configs);

        // Should be loading with no devices
        assert!(state.startup_dialog_state.devices.is_empty());
        assert!(state.startup_dialog_state.loading);
        assert!(!state.startup_dialog_state.refreshing);
        assert_eq!(state.startup_dialog_state.selected_device, None);
    }
}
