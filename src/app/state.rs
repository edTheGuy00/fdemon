//! Application state (Model in TEA pattern)

use std::path::PathBuf;

use rand::Rng;

use crate::config::{
    FlutterMode, LoadedConfigs, Settings, SettingsTab, SourcedConfig, UserPreferences,
};
use crate::core::AppPhase;
use crate::daemon::{Device, ToolAvailability};
use crate::tui::widgets::{ConfirmDialogState, DeviceSelectorState, NewSessionDialogState};

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

    /// New session dialog - unified device and configuration selection
    /// Replaces DeviceSelector and StartupDialog with a modern two-pane UI
    NewSessionDialog,
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

    // ─────────────────────────────────────────────────────────
    // Auto-save State (Task 10c)
    // ─────────────────────────────────────────────────────────
    /// Name of the currently selected fdemon config (for saving edits)
    pub editing_config_name: Option<String>,

    /// Whether there are unsaved changes
    pub dirty: bool,

    /// Timestamp of last edit (for debouncing)
    pub last_edit_time: Option<std::time::Instant>,

    // ─────────────────────────────────────────────────────────
    // No-Config Auto-save State (Task 10d)
    // ─────────────────────────────────────────────────────────
    /// True if we're creating a new config from scratch (no selection)
    pub creating_new_config: bool,

    /// Name for the new config being created
    pub new_config_name: String,

    // ─────────────────────────────────────────────────────────
    // "New Config" Option (Task 10e)
    // ─────────────────────────────────────────────────────────
    /// Whether the "+ New config" option is selected
    pub new_config_selected: bool,
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
            editing_config_name: None,
            dirty: false,
            last_edit_time: None,
            creating_new_config: false,
            new_config_name: "Default".to_string(),
            new_config_selected: false,
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

    // ─────────────────────────────────────────────────────────
    // "New Config" Option Helpers (Task 10e)
    // ─────────────────────────────────────────────────────────

    /// Should show "+ New config" option?
    /// Show when there are any configs (VSCode or FDemon)
    pub fn should_show_new_config_option(&self) -> bool {
        !self.configs.configs.is_empty()
    }

    /// Total items in config list (including "+ New config" if applicable)
    pub fn config_list_len(&self) -> usize {
        let base = self.configs.configs.len();
        if self.should_show_new_config_option() {
            base + 1 // +1 for "+ New config"
        } else {
            base
        }
    }

    /// Handle navigation down in config list (with "+ New config" support)
    pub fn navigate_config_down(&mut self) {
        if self.new_config_selected {
            // From "+ New config" -> wrap to first config
            self.new_config_selected = false;
            self.selected_config = if self.configs.configs.is_empty() {
                None
            } else {
                Some(0)
            };
        } else if let Some(idx) = self.selected_config {
            if idx >= self.configs.configs.len().saturating_sub(1) {
                // At last real config -> go to "+ New config" if available
                if self.should_show_new_config_option() {
                    self.new_config_selected = true;
                    self.selected_config = None;
                } else {
                    // Wrap to first
                    self.selected_config = Some(0);
                }
            } else {
                self.selected_config = Some(idx + 1);
            }
        } else {
            // No selection -> select first
            self.selected_config = Some(0);
        }

        self.on_selection_changed();
    }

    /// Handle navigation up in config list (with "+ New config" support)
    pub fn navigate_config_up(&mut self) {
        if self.new_config_selected {
            // From "+ New config" -> go to last real config
            self.new_config_selected = false;
            if !self.configs.configs.is_empty() {
                self.selected_config = Some(self.configs.configs.len() - 1);
            }
        } else if let Some(idx) = self.selected_config {
            if idx == 0 {
                // At first config -> go to "+ New config" or wrap
                if self.should_show_new_config_option() {
                    self.new_config_selected = true;
                    self.selected_config = None;
                } else {
                    // Wrap to last
                    self.selected_config = Some(self.configs.configs.len().saturating_sub(1));
                }
            } else {
                self.selected_config = Some(idx - 1);
            }
        }

        self.on_selection_changed();
    }

    /// Handle selection change (used by navigate_config_down/up)
    fn on_selection_changed(&mut self) {
        if self.new_config_selected {
            // Enable editing mode for new config
            self.creating_new_config = true;
            self.editing_config_name = None;
            // Keep existing field values (user might switch back and forth)
        } else if let Some(idx) = self.selected_config {
            self.on_config_selected(Some(idx));
        }
    }

    /// Navigate up in current section
    pub fn navigate_up(&mut self) {
        match self.active_section {
            DialogSection::Configs => {
                // Task 10e: Use new navigate_config_up for proper "+ New config" handling
                self.navigate_config_up();
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
                // Task 10e: Use new navigate_config_down for proper "+ New config" handling
                self.navigate_config_down();
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

    // ─────────────────────────────────────────────────────────
    // VSCode Config Field Disabling (Task 10b)
    // ─────────────────────────────────────────────────────────

    /// Whether flavor/dart_defines fields are editable
    /// VSCode configs have these fields disabled (read-only)
    /// Task 10e: "+ New config" option makes fields editable
    pub fn flavor_editable(&self) -> bool {
        // Editable if "+ New config" is selected
        if self.new_config_selected {
            return true;
        }

        match self.selected_config {
            Some(idx) => self
                .configs
                .configs
                .get(idx)
                .map(|c| c.source != crate::config::ConfigSource::VSCode)
                .unwrap_or(true),
            None => true, // No config selected = editable
        }
    }

    /// Get display value for flavor (from config if VSCode, else manual input)
    pub fn flavor_display(&self) -> &str {
        if let Some(idx) = self.selected_config {
            if let Some(config) = self.configs.configs.get(idx) {
                if config.source == crate::config::ConfigSource::VSCode {
                    // Show config value (read-only)
                    return config.config.flavor.as_deref().unwrap_or("");
                }
            }
        }
        &self.flavor
    }

    /// Get display value for dart_defines (from config if VSCode, else manual input)
    pub fn dart_defines_display(&self) -> String {
        if let Some(idx) = self.selected_config {
            if let Some(config) = self.configs.configs.get(idx) {
                if config.source == crate::config::ConfigSource::VSCode {
                    // Show config value (read-only)
                    return format_dart_defines(&config.config.dart_defines);
                }
            }
        }
        self.dart_defines.clone()
    }

    /// Update field values when config selection changes
    pub fn on_config_selected(&mut self, idx: Option<usize>) {
        self.selected_config = idx;

        // Clear dirty state when changing selection
        self.dirty = false;
        self.last_edit_time = None;
        self.editing_config_name = None;
        self.creating_new_config = false; // Task 10d: Reset new config flag

        // If VSCode config, populate fields with config values (read-only display)
        // If FDemon config, auto-fill fields and enable auto-save
        // If no config, keep current manual values
        if let Some(i) = idx {
            if let Some(config) = self.configs.configs.get(i) {
                if config.source == crate::config::ConfigSource::VSCode {
                    // Show config values in fields (read-only)
                    self.flavor = config.config.flavor.clone().unwrap_or_default();
                    self.dart_defines = format_dart_defines(&config.config.dart_defines);
                    self.mode = config.config.mode;
                } else if config.source == crate::config::ConfigSource::FDemon {
                    // Task 10c: Auto-fill fields from FDemon config
                    self.mode = config.config.mode;
                    self.flavor = config.config.flavor.clone().unwrap_or_default();
                    self.dart_defines = format_dart_defines(&config.config.dart_defines);

                    // Track config name for auto-save
                    self.editing_config_name = Some(config.config.name.clone());
                }
            }
        }
    }

    /// Check if we should create a new config on save (Task 10d)
    pub fn should_create_new_config(&self) -> bool {
        // No config selected AND user has entered something
        self.selected_config.is_none() && (!self.flavor.is_empty() || !self.dart_defines.is_empty())
    }

    /// Mark as dirty and record edit time (Task 10c)
    /// Also flags for new config creation if no config selected (Task 10d)
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
        self.last_edit_time = Some(std::time::Instant::now());

        // If no config selected and user is entering data, flag for new config (Task 10d)
        if self.selected_config.is_none()
            && !self.creating_new_config
            && (!self.flavor.is_empty() || !self.dart_defines.is_empty())
        {
            self.creating_new_config = true;
        }
    }

    /// Check if debounce period has passed (500ms since last edit) (Task 10c)
    pub fn should_save(&self) -> bool {
        if !self.dirty {
            return false;
        }
        match self.last_edit_time {
            Some(t) => t.elapsed() >= std::time::Duration::from_millis(500),
            None => false,
        }
    }

    /// Clear dirty state after save (Task 10c)
    pub fn mark_saved(&mut self) {
        self.dirty = false;
        self.last_edit_time = None;
    }
}

/// Helper function to format dart-defines HashMap into comma-separated string
fn format_dart_defines(defines: &std::collections::HashMap<String, String>) -> String {
    defines
        .iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join(",")
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

    /// New session dialog state (unified dialog)
    pub new_session_dialog_state: NewSessionDialogState,

    /// Loading state (for initial startup loading screen)
    pub loading_state: Option<LoadingState>,

    /// Global device cache (shared between DeviceSelector and StartupDialog)
    /// Task 08e - Device Cache Sharing
    pub device_cache: Option<Vec<Device>>,

    /// When devices were last discovered (for cache invalidation)
    /// Task 08e - Device Cache Sharing
    pub devices_last_updated: Option<std::time::Instant>,

    /// Cached tool availability (checked at startup)
    /// Phase 4, Task 05 - Discovery Integration
    pub tool_availability: ToolAvailability,
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
            new_session_dialog_state: NewSessionDialogState::new(),
            loading_state: None,
            device_cache: None,
            devices_last_updated: None,
            tool_availability: ToolAvailability::default(),
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

    /// Show the new session dialog
    pub fn show_new_session_dialog(&mut self, configs: LoadedConfigs) {
        self.new_session_dialog_state = NewSessionDialogState::with_configs(configs);
        self.ui_mode = UiMode::NewSessionDialog;
    }

    /// Hide the new session dialog
    pub fn hide_new_session_dialog(&mut self) {
        self.ui_mode = UiMode::Normal;
    }

    /// Check if new session dialog is visible
    pub fn is_new_session_dialog_visible(&self) -> bool {
        self.ui_mode == UiMode::NewSessionDialog
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

    // ─────────────────────────────────────────────────────────
    // VSCode Config Field Disabling Tests (Task 10b)
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_flavor_editable_vscode() {
        use crate::config::{ConfigSource, LaunchConfig};

        let mut configs = LoadedConfigs::default();
        configs.configs.push(SourcedConfig {
            config: LaunchConfig::default(),
            source: ConfigSource::VSCode,
            display_name: "VSCode".to_string(),
        });

        let mut state = StartupDialogState::with_configs(configs);
        state.selected_config = Some(0);

        assert!(!state.flavor_editable());
    }

    #[test]
    fn test_flavor_editable_fdemon() {
        use crate::config::{ConfigSource, LaunchConfig};

        let mut configs = LoadedConfigs::default();
        configs.configs.push(SourcedConfig {
            config: LaunchConfig::default(),
            source: ConfigSource::FDemon,
            display_name: "FDemon".to_string(),
        });

        let mut state = StartupDialogState::with_configs(configs);
        state.selected_config = Some(0);

        assert!(state.flavor_editable());
    }

    #[test]
    fn test_flavor_editable_no_config() {
        let state = StartupDialogState::new();
        assert!(state.flavor_editable());
    }

    #[test]
    fn test_flavor_display_vscode() {
        use crate::config::{ConfigSource, LaunchConfig};

        let mut configs = LoadedConfigs::default();
        let mut config = LaunchConfig::default();
        config.flavor = Some("production".to_string());
        configs.configs.push(SourcedConfig {
            config,
            source: ConfigSource::VSCode,
            display_name: "VSCode".to_string(),
        });

        let mut state = StartupDialogState::with_configs(configs);
        state.selected_config = Some(0);
        state.flavor = "manual".to_string(); // Should be ignored

        assert_eq!(state.flavor_display(), "production");
    }

    #[test]
    fn test_flavor_display_fdemon() {
        use crate::config::{ConfigSource, LaunchConfig};

        let mut configs = LoadedConfigs::default();
        configs.configs.push(SourcedConfig {
            config: LaunchConfig::default(),
            source: ConfigSource::FDemon,
            display_name: "FDemon".to_string(),
        });

        let mut state = StartupDialogState::with_configs(configs);
        state.selected_config = Some(0);
        state.flavor = "manual".to_string();

        assert_eq!(state.flavor_display(), "manual");
    }

    #[test]
    fn test_dart_defines_display_vscode() {
        use crate::config::{ConfigSource, LaunchConfig};
        use std::collections::HashMap;

        let mut configs = LoadedConfigs::default();
        let mut config = LaunchConfig::default();
        let mut defines = HashMap::new();
        defines.insert("API_URL".to_string(), "https://api.example.com".to_string());
        defines.insert("DEBUG".to_string(), "true".to_string());
        config.dart_defines = defines;
        configs.configs.push(SourcedConfig {
            config,
            source: ConfigSource::VSCode,
            display_name: "VSCode".to_string(),
        });

        let mut state = StartupDialogState::with_configs(configs);
        state.selected_config = Some(0);
        state.dart_defines = "manual=value".to_string(); // Should be ignored

        let display = state.dart_defines_display();
        // Note: HashMap iteration order is not guaranteed, so check both are present
        assert!(display.contains("API_URL=https://api.example.com"));
        assert!(display.contains("DEBUG=true"));
    }

    #[test]
    fn test_on_config_selected_vscode() {
        use crate::config::{ConfigSource, FlutterMode, LaunchConfig};
        use std::collections::HashMap;

        let mut configs = LoadedConfigs::default();
        let mut config = LaunchConfig::default();
        config.flavor = Some("production".to_string());
        config.mode = FlutterMode::Release;
        let mut defines = HashMap::new();
        defines.insert("KEY".to_string(), "value".to_string());
        config.dart_defines = defines;
        configs.configs.push(SourcedConfig {
            config,
            source: ConfigSource::VSCode,
            display_name: "VSCode".to_string(),
        });

        let mut state = StartupDialogState::with_configs(configs);
        state.flavor = "initial".to_string();
        state.mode = FlutterMode::Debug;

        state.on_config_selected(Some(0));

        assert_eq!(state.flavor, "production");
        assert_eq!(state.mode, FlutterMode::Release);
        assert!(state.dart_defines.contains("KEY=value"));
    }

    #[test]
    fn test_on_config_selected_fdemon() {
        use crate::config::{ConfigSource, FlutterMode, LaunchConfig};

        let mut configs = LoadedConfigs::default();
        let mut config = LaunchConfig::default();
        config.flavor = Some("dev".to_string());
        config.mode = FlutterMode::Profile;
        configs.configs.push(SourcedConfig {
            config,
            source: ConfigSource::FDemon,
            display_name: "FDemon".to_string(),
        });

        let mut state = StartupDialogState::with_configs(configs);
        state.flavor = "manual".to_string();
        state.mode = FlutterMode::Debug;

        state.on_config_selected(Some(0));

        // Task 10c: FDemon configs now auto-populate fields (for auto-save)
        assert_eq!(state.flavor, "dev");
        assert_eq!(state.mode, FlutterMode::Profile);
    }

    #[test]
    fn test_on_config_selected_none() {
        let mut state = StartupDialogState::new();
        state.flavor = "test".to_string();
        state.on_config_selected(None);

        // Should not change anything
        assert_eq!(state.flavor, "test");
        assert!(state.selected_config.is_none());
    }

    // ─────────────────────────────────────────────────────────
    // FDemon Config Auto-fill and Auto-save Tests (Task 10c)
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_on_config_selected_fills_fields_fdemon() {
        use crate::config::{ConfigSource, LaunchConfig};
        use std::collections::HashMap;

        let mut configs = LoadedConfigs::default();
        let mut dart_defines = HashMap::new();
        dart_defines.insert("API_URL".to_string(), "https://dev.com".to_string());
        configs.configs.push(SourcedConfig {
            config: LaunchConfig {
                name: "Dev".to_string(),
                flavor: Some("development".to_string()),
                mode: FlutterMode::Profile,
                dart_defines,
                ..Default::default()
            },
            source: ConfigSource::FDemon,
            display_name: "Dev".to_string(),
        });

        let mut state = StartupDialogState::with_configs(configs);
        state.on_config_selected(Some(0));

        assert_eq!(state.flavor, "development");
        assert_eq!(state.mode, FlutterMode::Profile);
        assert!(state.dart_defines.contains("API_URL=https://dev.com"));
        assert_eq!(state.editing_config_name, Some("Dev".to_string()));
    }

    #[test]
    fn test_mark_dirty_and_should_save() {
        let mut state = StartupDialogState::new();

        assert!(!state.should_save());

        state.mark_dirty();
        assert!(state.dirty);
        assert!(state.last_edit_time.is_some());
        assert!(!state.should_save()); // Not enough time passed

        std::thread::sleep(std::time::Duration::from_millis(600));
        assert!(state.should_save()); // Now should save

        state.mark_saved();
        assert!(!state.dirty);
        assert!(state.last_edit_time.is_none());
        assert!(!state.should_save());
    }

    #[test]
    fn test_mark_dirty_updates_timestamp() {
        let mut state = StartupDialogState::new();

        state.mark_dirty();
        let first_time = state.last_edit_time;
        assert!(first_time.is_some());

        std::thread::sleep(std::time::Duration::from_millis(10));
        state.mark_dirty();
        let second_time = state.last_edit_time;

        assert!(second_time > first_time);
    }

    #[test]
    fn test_should_save_not_dirty() {
        let mut state = StartupDialogState::new();
        state.last_edit_time = Some(std::time::Instant::now());

        assert!(!state.should_save()); // Not dirty
    }

    #[test]
    fn test_should_save_no_timestamp() {
        let mut state = StartupDialogState::new();
        state.dirty = true;
        state.last_edit_time = None;

        assert!(!state.should_save()); // No timestamp
    }

    #[test]
    fn test_on_config_selected_clears_dirty() {
        use crate::config::{ConfigSource, LaunchConfig};

        let mut configs = LoadedConfigs::default();
        configs.configs.push(SourcedConfig {
            config: LaunchConfig {
                name: "Dev".to_string(),
                ..Default::default()
            },
            source: ConfigSource::FDemon,
            display_name: "Dev".to_string(),
        });

        let mut state = StartupDialogState::with_configs(configs);
        state.dirty = true;
        state.last_edit_time = Some(std::time::Instant::now());
        state.editing_config_name = Some("Old".to_string());

        state.on_config_selected(Some(0));

        assert!(!state.dirty);
        assert!(state.last_edit_time.is_none());
        assert_eq!(state.editing_config_name, Some("Dev".to_string()));
    }

    #[test]
    fn test_on_config_selected_fdemon_tracks_config_name() {
        use crate::config::{ConfigSource, LaunchConfig};

        let mut configs = LoadedConfigs::default();
        configs.configs.push(SourcedConfig {
            config: LaunchConfig {
                name: "MyConfig".to_string(),
                ..Default::default()
            },
            source: ConfigSource::FDemon,
            display_name: "MyConfig".to_string(),
        });

        let mut state = StartupDialogState::with_configs(configs);
        state.on_config_selected(Some(0));

        assert_eq!(state.editing_config_name, Some("MyConfig".to_string()));
    }

    #[test]
    fn test_on_config_selected_vscode_does_not_track() {
        use crate::config::{ConfigSource, LaunchConfig};

        let mut configs = LoadedConfigs::default();
        configs.configs.push(SourcedConfig {
            config: LaunchConfig {
                name: "VSCode".to_string(),
                ..Default::default()
            },
            source: ConfigSource::VSCode,
            display_name: "VSCode".to_string(),
        });

        let mut state = StartupDialogState::with_configs(configs);
        state.on_config_selected(Some(0));

        assert!(state.editing_config_name.is_none());
    }

    // ─────────────────────────────────────────────────────────
    // No-Config Auto-save Tests (Task 10d)
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_should_create_new_config() {
        let mut state = StartupDialogState::new();
        state.selected_config = None;

        // Initially false - no data entered
        assert!(!state.should_create_new_config());

        // After entering flavor
        state.flavor = "test".to_string();
        assert!(state.should_create_new_config());

        // After entering dart_defines
        state.flavor = String::new();
        state.dart_defines = "KEY=value".to_string();
        assert!(state.should_create_new_config());

        // Both empty again - false
        state.dart_defines = String::new();
        assert!(!state.should_create_new_config());
    }

    #[test]
    fn test_should_create_new_config_with_selected() {
        let mut state = StartupDialogState::new();
        state.selected_config = Some(0);
        state.flavor = "test".to_string();

        // Should not create when config is selected
        assert!(!state.should_create_new_config());
    }

    #[test]
    fn test_mark_dirty_sets_creating_flag() {
        let mut state = StartupDialogState::new();
        state.selected_config = None;
        state.flavor = "test".to_string();

        state.mark_dirty();

        assert!(state.creating_new_config);
        assert!(state.dirty);
        assert!(state.last_edit_time.is_some());
    }

    #[test]
    fn test_mark_dirty_no_create_if_empty() {
        let mut state = StartupDialogState::new();
        state.selected_config = None;
        state.flavor = String::new();
        state.dart_defines = String::new();

        state.mark_dirty();

        assert!(!state.creating_new_config);
    }

    #[test]
    fn test_mark_dirty_no_create_if_already_creating() {
        let mut state = StartupDialogState::new();
        state.selected_config = None;
        state.flavor = "test".to_string();

        state.mark_dirty();
        assert!(state.creating_new_config);

        // Shouldn't toggle off
        state.flavor = "test2".to_string();
        state.mark_dirty();
        assert!(state.creating_new_config);
    }

    #[test]
    fn test_on_config_selected_clears_creating_flag() {
        use crate::config::{ConfigSource, LaunchConfig};

        let mut configs = LoadedConfigs::default();
        configs.configs.push(SourcedConfig {
            config: LaunchConfig {
                name: "Dev".to_string(),
                ..Default::default()
            },
            source: ConfigSource::FDemon,
            display_name: "Dev".to_string(),
        });

        let mut state = StartupDialogState::with_configs(configs);
        state.creating_new_config = true;
        state.dirty = true;

        state.on_config_selected(Some(0));

        assert!(!state.creating_new_config);
        assert!(!state.dirty);
    }

    #[test]
    fn test_default_new_config_name() {
        let state = StartupDialogState::new();
        assert_eq!(state.new_config_name, "Default");
    }

    // ─────────────────────────────────────────────────────────
    // "New Config" Option Tests (Task 10e)
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_should_show_new_config_option() {
        use crate::config::{ConfigSource, LaunchConfig};

        let mut configs = LoadedConfigs::default();
        configs.configs.push(SourcedConfig {
            config: LaunchConfig::default(),
            source: ConfigSource::VSCode,
            display_name: "VSCode".to_string(),
        });

        let state = StartupDialogState::with_configs(configs);
        assert!(state.should_show_new_config_option());
    }

    #[test]
    fn test_should_show_new_config_option_empty() {
        let state = StartupDialogState::new();
        assert!(!state.should_show_new_config_option());
    }

    #[test]
    fn test_config_list_len_includes_new_config() {
        use crate::config::{ConfigSource, LaunchConfig};

        let mut configs = LoadedConfigs::default();
        configs.configs.push(SourcedConfig {
            config: LaunchConfig::default(),
            source: ConfigSource::VSCode,
            display_name: "VSCode".to_string(),
        });

        let state = StartupDialogState::with_configs(configs);
        assert_eq!(state.config_list_len(), 2); // 1 config + 1 "New config"
    }

    #[test]
    fn test_config_list_len_without_new_config() {
        let state = StartupDialogState::new();
        assert_eq!(state.config_list_len(), 0);
    }

    #[test]
    fn test_navigate_config_down_to_new_config() {
        use crate::config::{ConfigSource, LaunchConfig};

        let mut configs = LoadedConfigs::default();
        configs.configs.push(SourcedConfig {
            config: LaunchConfig::default(),
            source: ConfigSource::VSCode,
            display_name: "VSCode".to_string(),
        });

        let mut state = StartupDialogState::with_configs(configs);
        state.selected_config = Some(0);

        state.navigate_config_down();

        assert!(state.new_config_selected);
        assert!(state.selected_config.is_none());
    }

    #[test]
    fn test_navigate_config_up_from_new_config() {
        use crate::config::{ConfigSource, LaunchConfig};

        let mut configs = LoadedConfigs::default();
        configs.configs.push(SourcedConfig {
            config: LaunchConfig::default(),
            source: ConfigSource::VSCode,
            display_name: "VSCode".to_string(),
        });

        let mut state = StartupDialogState::with_configs(configs);
        state.new_config_selected = true;
        state.selected_config = None;

        state.navigate_config_up();

        assert!(!state.new_config_selected);
        assert_eq!(state.selected_config, Some(0));
    }

    #[test]
    fn test_navigate_config_down_wraps_from_new_config() {
        use crate::config::{ConfigSource, LaunchConfig};

        let mut configs = LoadedConfigs::default();
        configs.configs.push(SourcedConfig {
            config: LaunchConfig::default(),
            source: ConfigSource::VSCode,
            display_name: "VSCode".to_string(),
        });

        let mut state = StartupDialogState::with_configs(configs);
        state.new_config_selected = true;
        state.selected_config = None;

        state.navigate_config_down();

        assert!(!state.new_config_selected);
        assert_eq!(state.selected_config, Some(0));
    }

    #[test]
    fn test_navigate_config_up_wraps_from_first_to_new_config() {
        use crate::config::{ConfigSource, LaunchConfig};

        let mut configs = LoadedConfigs::default();
        configs.configs.push(SourcedConfig {
            config: LaunchConfig::default(),
            source: ConfigSource::VSCode,
            display_name: "VSCode".to_string(),
        });

        let mut state = StartupDialogState::with_configs(configs);
        state.selected_config = Some(0);

        state.navigate_config_up();

        assert!(state.new_config_selected);
        assert!(state.selected_config.is_none());
    }

    #[test]
    fn test_flavor_editable_new_config_selected() {
        use crate::config::{ConfigSource, LaunchConfig};

        let mut configs = LoadedConfigs::default();
        configs.configs.push(SourcedConfig {
            config: LaunchConfig::default(),
            source: ConfigSource::VSCode,
            display_name: "VSCode".to_string(),
        });

        let mut state = StartupDialogState::with_configs(configs);
        state.new_config_selected = true;

        assert!(state.flavor_editable());
    }

    #[test]
    fn test_on_selection_changed_sets_creating_new_config() {
        use crate::config::{ConfigSource, LaunchConfig};

        let mut configs = LoadedConfigs::default();
        configs.configs.push(SourcedConfig {
            config: LaunchConfig::default(),
            source: ConfigSource::VSCode,
            display_name: "VSCode".to_string(),
        });

        let mut state = StartupDialogState::with_configs(configs);
        state.creating_new_config = false;
        state.new_config_selected = true;

        state.on_selection_changed();

        assert!(state.creating_new_config);
        assert!(state.editing_config_name.is_none());
    }

    #[test]
    fn test_navigate_with_multiple_configs() {
        use crate::config::{ConfigSource, LaunchConfig};

        let mut configs = LoadedConfigs::default();
        configs.configs.push(SourcedConfig {
            config: LaunchConfig {
                name: "Config1".to_string(),
                ..Default::default()
            },
            source: ConfigSource::FDemon,
            display_name: "Config1".to_string(),
        });
        configs.configs.push(SourcedConfig {
            config: LaunchConfig {
                name: "Config2".to_string(),
                ..Default::default()
            },
            source: ConfigSource::VSCode,
            display_name: "Config2".to_string(),
        });

        let mut state = StartupDialogState::with_configs(configs);
        state.selected_config = Some(0);

        // Navigate down to second config
        state.navigate_config_down();
        assert_eq!(state.selected_config, Some(1));
        assert!(!state.new_config_selected);

        // Navigate down to "+ New config"
        state.navigate_config_down();
        assert!(state.new_config_selected);
        assert!(state.selected_config.is_none());

        // Navigate down wraps to first
        state.navigate_config_down();
        assert_eq!(state.selected_config, Some(0));
        assert!(!state.new_config_selected);
    }

    #[test]
    fn test_default_new_config_selected_false() {
        let state = StartupDialogState::new();
        assert!(!state.new_config_selected);
    }

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
}
