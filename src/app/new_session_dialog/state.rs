//! NewSessionDialog complete state management

use super::types::{
    DartDefine, DialogPane, FuzzyModalType, LaunchContextField, LaunchParams, TargetTab,
};
use crate::config::{ConfigSource, FlutterMode, LoadedConfigs};
use crate::daemon::Device;
use std::path::PathBuf;

// ─────────────────────────────────────────────────────────────────────────────
// FuzzyModalState
// ─────────────────────────────────────────────────────────────────────────────

/// State for the fuzzy search modal
#[derive(Debug, Clone)]
pub struct FuzzyModalState {
    /// Type of modal (determines title and behavior)
    pub modal_type: FuzzyModalType,

    /// User's search query
    pub query: String,

    /// All available items (original order)
    pub items: Vec<String>,

    /// Indices of items matching the query (into `items`)
    pub filtered_indices: Vec<usize>,

    /// Currently highlighted index (into `filtered_indices`)
    pub selected_index: usize,

    /// Scroll offset for long lists
    pub scroll_offset: usize,
}

impl FuzzyModalState {
    /// Create a new fuzzy modal state
    pub fn new(modal_type: FuzzyModalType, items: Vec<String>) -> Self {
        let filtered_indices: Vec<usize> = (0..items.len()).collect();
        Self {
            modal_type,
            query: String::new(),
            items,
            filtered_indices,
            selected_index: 0,
            scroll_offset: 0,
        }
    }

    /// Get the currently selected item, or the query if no match
    pub fn selected_value(&self) -> Option<String> {
        if let Some(&idx) = self.filtered_indices.get(self.selected_index) {
            // Use safe access to prevent panic if index is somehow invalid
            self.items.get(idx).cloned()
        } else if self.modal_type.allows_custom() && !self.query.is_empty() {
            Some(self.query.clone())
        } else {
            None
        }
    }

    /// Check if there are any filtered results
    pub fn has_results(&self) -> bool {
        !self.filtered_indices.is_empty()
    }

    /// Get the number of filtered results
    pub fn result_count(&self) -> usize {
        self.filtered_indices.len()
    }

    /// Navigate up in the list
    pub fn navigate_up(&mut self) {
        if !self.filtered_indices.is_empty() {
            self.selected_index = if self.selected_index == 0 {
                self.filtered_indices.len() - 1
            } else {
                self.selected_index - 1
            };
            self.adjust_scroll();
        }
    }

    /// Navigate down in the list
    pub fn navigate_down(&mut self) {
        if !self.filtered_indices.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.filtered_indices.len();
            self.adjust_scroll();
        }
    }

    /// Adjust scroll offset to keep selection visible
    fn adjust_scroll(&mut self) {
        const VISIBLE_ITEMS: usize = 7; // Number of items visible in modal

        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        } else if self.selected_index >= self.scroll_offset + VISIBLE_ITEMS {
            self.scroll_offset = self.selected_index - VISIBLE_ITEMS + 1;
        }
    }

    /// Add a character to the query
    pub fn input_char(&mut self, c: char) {
        self.query.push(c);
        self.update_filter_placeholder();
    }

    /// Remove the last character from the query
    pub fn backspace(&mut self) {
        self.query.pop();
        self.update_filter_placeholder();
    }

    /// Clear the query
    pub fn clear_query(&mut self) {
        self.query.clear();
        self.update_filter_placeholder();
    }

    /// Update filtered indices based on current query
    /// Note: This calls TUI layer filtering function but stays in App layer state
    fn update_filter_placeholder(&mut self) {
        // Reset selection when filter changes
        self.selected_index = 0;
        self.scroll_offset = 0;

        // Actual filtering is deferred - handler will call update_filter() after this
    }

    /// Update filtered indices externally (called after filtering)
    pub fn update_filter(&mut self, filtered: Vec<usize>) {
        self.filtered_indices = filtered;
        self.selected_index = 0;
        self.scroll_offset = 0;
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// DartDefinesModalState
// ─────────────────────────────────────────────────────────────────────────────

/// Which pane is focused in the dart defines modal
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DartDefinesPane {
    #[default]
    List,
    Edit,
}

/// Which field is focused in the edit pane
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DartDefinesEditField {
    #[default]
    Key,
    Value,
    Save,
    Delete,
}

impl DartDefinesEditField {
    /// Get next field in tab order
    pub fn next(self) -> Self {
        match self {
            Self::Key => Self::Value,
            Self::Value => Self::Save,
            Self::Save => Self::Delete,
            Self::Delete => Self::Key,
        }
    }

    /// Get previous field in tab order
    pub fn prev(self) -> Self {
        match self {
            Self::Key => Self::Delete,
            Self::Value => Self::Key,
            Self::Save => Self::Value,
            Self::Delete => Self::Save,
        }
    }
}

/// State for the dart defines modal
#[derive(Debug, Clone)]
pub struct DartDefinesModalState {
    /// All dart defines (working copy)
    pub defines: Vec<DartDefine>,

    /// Currently selected index in the list (includes "[+] Add New" at end)
    pub selected_index: usize,

    /// Scroll offset for long lists
    pub scroll_offset: usize,

    /// Which pane is currently focused
    pub active_pane: DartDefinesPane,

    /// Which field is focused in the edit pane
    pub edit_field: DartDefinesEditField,

    /// Current value in the Key input field
    pub editing_key: String,

    /// Current value in the Value input field
    pub editing_value: String,

    /// Whether we're editing a new define (vs existing)
    pub is_new: bool,
}

impl DartDefinesModalState {
    /// Create a new dart defines modal state from existing defines
    pub fn new(defines: Vec<DartDefine>) -> Self {
        Self {
            defines,
            selected_index: 0,
            scroll_offset: 0,
            active_pane: DartDefinesPane::List,
            edit_field: DartDefinesEditField::Key,
            editing_key: String::new(),
            editing_value: String::new(),
            is_new: false,
        }
    }

    /// Check if the "[+] Add New" option is selected
    pub fn is_add_new_selected(&self) -> bool {
        self.selected_index >= self.defines.len()
    }

    /// Get the currently selected define (if any)
    pub fn selected_define(&self) -> Option<&DartDefine> {
        self.defines.get(self.selected_index)
    }

    /// Get the total number of items in list (defines + Add New)
    pub fn list_item_count(&self) -> usize {
        self.defines.len() + 1 // +1 for "[+] Add New"
    }

    /// Navigate up in the list
    pub fn navigate_up(&mut self) {
        if self.list_item_count() > 0 {
            self.selected_index = if self.selected_index == 0 {
                self.list_item_count() - 1
            } else {
                self.selected_index - 1
            };
            self.adjust_scroll();
        }
    }

    /// Navigate down in the list
    pub fn navigate_down(&mut self) {
        if self.list_item_count() > 0 {
            self.selected_index = (self.selected_index + 1) % self.list_item_count();
            self.adjust_scroll();
        }
    }

    /// Adjust scroll offset to keep selection visible
    fn adjust_scroll(&mut self) {
        const VISIBLE_ITEMS: usize = 10;

        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        } else if self.selected_index >= self.scroll_offset + VISIBLE_ITEMS {
            self.scroll_offset = self.selected_index - VISIBLE_ITEMS + 1;
        }
    }

    /// Switch to the other pane
    pub fn switch_pane(&mut self) {
        self.active_pane = match self.active_pane {
            DartDefinesPane::List => DartDefinesPane::Edit,
            DartDefinesPane::Edit => DartDefinesPane::List,
        };
    }

    /// Move to next field in edit pane
    pub fn next_field(&mut self) {
        self.edit_field = self.edit_field.next();
    }

    /// Move to previous field in edit pane
    pub fn prev_field(&mut self) {
        self.edit_field = self.edit_field.prev();
    }

    /// Load the selected define into the edit form
    pub fn load_selected_into_edit(&mut self) {
        // Clone the selected define to avoid borrow checker issues
        let selected = self.defines.get(self.selected_index).cloned();

        if let Some(define) = selected {
            self.editing_key = define.key;
            self.editing_value = define.value;
            self.is_new = false;
        } else {
            // "[+] Add New" selected
            self.editing_key.clear();
            self.editing_value.clear();
            self.is_new = true;
        }
        self.active_pane = DartDefinesPane::Edit;
        self.edit_field = DartDefinesEditField::Key;
    }

    /// Save the current edit form to the defines list
    /// Returns true if save was successful
    pub fn save_edit(&mut self) -> bool {
        // Validate: key cannot be empty
        if self.editing_key.trim().is_empty() {
            return false;
        }

        let define = DartDefine::new(
            self.editing_key.trim().to_string(),
            self.editing_value.clone(),
        );

        if self.is_new {
            // Add new define
            self.defines.push(define);
            self.selected_index = self.defines.len() - 1;
            self.is_new = false;
        } else {
            // Update existing
            if let Some(existing) = self.defines.get_mut(self.selected_index) {
                *existing = define;
            }
        }

        true
    }

    /// Delete the currently selected define
    /// Returns true if delete was performed
    pub fn delete_selected(&mut self) -> bool {
        if self.is_add_new_selected() {
            return false; // Can't delete "[+] Add New"
        }

        if self.selected_index < self.defines.len() {
            self.defines.remove(self.selected_index);

            // Adjust selection: clamp to valid range after removal
            // Note: saturating_sub(1) on 0 returns 0, so this handles empty list correctly
            // (index 0 will point to "[+] Add New")
            if self.selected_index >= self.defines.len() {
                self.selected_index = self.defines.len().saturating_sub(1);
            }

            // Clear edit form
            self.editing_key.clear();
            self.editing_value.clear();

            // Return to list
            self.active_pane = DartDefinesPane::List;

            return true;
        }

        false
    }

    /// Input a character to the currently focused text field
    pub fn input_char(&mut self, c: char) {
        match self.edit_field {
            DartDefinesEditField::Key => self.editing_key.push(c),
            DartDefinesEditField::Value => self.editing_value.push(c),
            _ => {}
        }
    }

    /// Backspace in the currently focused text field
    pub fn backspace(&mut self) {
        match self.edit_field {
            DartDefinesEditField::Key => {
                self.editing_key.pop();
            }
            DartDefinesEditField::Value => {
                self.editing_value.pop();
            }
            _ => {}
        }
    }

    /// Check if there are unsaved changes in the edit form
    pub fn has_unsaved_changes(&self) -> bool {
        if self.is_new {
            !self.editing_key.is_empty() || !self.editing_value.is_empty()
        } else if let Some(define) = self.selected_define() {
            self.editing_key != define.key || self.editing_value != define.value
        } else {
            false
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// LaunchContextState
// ─────────────────────────────────────────────────────────────────────────────

/// State for the Launch Context pane
#[derive(Debug, Clone)]
pub struct LaunchContextState {
    /// Available configurations
    pub configs: LoadedConfigs,

    /// Index of selected configuration (None = no config, use defaults)
    pub selected_config_index: Option<usize>,

    /// Selected Flutter mode
    pub mode: FlutterMode,

    /// Flavor (from config or user override)
    pub flavor: Option<String>,

    /// Entry point (from config or user override)
    pub entry_point: Option<PathBuf>,

    /// Available entry points discovered from project
    pub available_entry_points: Vec<PathBuf>,

    /// True while discovering entry points (Phase 3, Task 09)
    pub entry_points_loading: bool,

    /// Dart defines (from config or user override)
    pub dart_defines: Vec<DartDefine>,

    /// Currently focused field
    pub focused_field: LaunchContextField,
}

impl LaunchContextState {
    pub fn new(configs: LoadedConfigs) -> Self {
        Self {
            configs,
            selected_config_index: None,
            mode: FlutterMode::Debug,
            flavor: None,
            entry_point: None,
            available_entry_points: Vec::new(),
            entry_points_loading: false,
            dart_defines: Vec::new(),
            focused_field: LaunchContextField::Config,
        }
    }

    /// Get the currently selected config
    pub fn selected_config(&self) -> Option<&crate::config::SourcedConfig> {
        self.selected_config_index
            .and_then(|i| self.configs.configs.get(i))
    }

    /// Get the source of the selected config
    pub fn selected_config_source(&self) -> Option<ConfigSource> {
        self.selected_config().map(|c| c.source)
    }

    /// Check if a field is editable based on config source
    pub fn is_field_editable(&self, field: LaunchContextField) -> bool {
        match field {
            // Config is always selectable
            LaunchContextField::Config => true,
            // Launch button is always enabled
            LaunchContextField::Launch => true,
            // Other fields depend on config source
            _ => {
                match self.selected_config_source() {
                    // VSCode configs: all fields read-only
                    Some(ConfigSource::VSCode) => false,
                    // FDemon configs: all fields editable
                    Some(ConfigSource::FDemon) => true,
                    // No config: all fields editable (transient)
                    None => true,
                    // CommandLine and Default configs: editable
                    Some(ConfigSource::CommandLine) | Some(ConfigSource::Default) => true,
                }
            }
        }
    }

    /// Check if mode is editable
    pub fn is_mode_editable(&self) -> bool {
        self.is_field_editable(LaunchContextField::Mode)
    }

    /// Check if flavor is editable
    pub fn is_flavor_editable(&self) -> bool {
        self.is_field_editable(LaunchContextField::Flavor)
    }

    /// Check if dart defines are editable
    pub fn are_dart_defines_editable(&self) -> bool {
        self.is_field_editable(LaunchContextField::DartDefines)
    }

    /// Select a configuration by index
    pub fn select_config(&mut self, index: Option<usize>) {
        self.selected_config_index = index;

        // Apply config values
        // Clone the config to avoid borrow checker issues
        if let Some(config) = self.selected_config().cloned() {
            self.mode = config.config.mode;

            if let Some(ref flavor) = config.config.flavor {
                self.flavor = Some(flavor.clone());
            }

            // Apply entry_point from config
            if let Some(ref entry_point) = config.config.entry_point {
                self.entry_point = Some(entry_point.clone());
            }

            if !config.config.dart_defines.is_empty() {
                self.dart_defines = config
                    .config
                    .dart_defines
                    .iter()
                    .map(|(k, v)| DartDefine::new(k, v))
                    .collect();
            }
        }
    }

    /// Select a configuration by name
    pub fn select_config_by_name(&mut self, name: &str) {
        let index = self
            .configs
            .configs
            .iter()
            .position(|c| c.display_name == name);
        self.select_config(index);
    }

    /// Set flavor
    pub fn set_flavor(&mut self, flavor: Option<String>) {
        if self.is_flavor_editable() {
            self.flavor = flavor;
        }
    }

    /// Set dart defines
    pub fn set_dart_defines(&mut self, defines: Vec<DartDefine>) {
        if self.are_dart_defines_editable() {
            self.dart_defines = defines;
        }
    }

    /// Get flavor display string
    pub fn flavor_display(&self) -> String {
        self.flavor.clone().unwrap_or_else(|| "(none)".to_string())
    }

    /// Get dart defines display string
    pub fn dart_defines_display(&self) -> String {
        let count = self.dart_defines.len();
        if count == 0 {
            "(none)".to_string()
        } else if count == 1 {
            "1 item".to_string()
        } else {
            format!("{} items", count)
        }
    }

    /// Get config display string
    pub fn config_display(&self) -> String {
        self.selected_config()
            .map(|c| c.display_name.clone())
            .unwrap_or_else(|| "(none)".to_string())
    }

    /// Get entry point display string
    pub fn entry_point_display(&self) -> String {
        self.entry_point
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "(default)".to_string())
    }

    /// Check if entry point is editable
    pub fn is_entry_point_editable(&self) -> bool {
        self.is_field_editable(LaunchContextField::EntryPoint)
    }

    /// Set entry point
    pub fn set_entry_point(&mut self, entry_point: Option<PathBuf>) {
        if self.is_entry_point_editable() {
            self.entry_point = entry_point;
        }
    }

    /// Set available entry points (typically from discovery)
    pub fn set_available_entry_points(&mut self, entry_points: Vec<PathBuf>) {
        self.available_entry_points = entry_points;
    }

    /// Get entry point items for fuzzy modal
    ///
    /// Returns a list of strings for the fuzzy modal, with "(default)" as first option.
    pub fn entry_point_modal_items(&self) -> Vec<String> {
        let mut items = vec!["(default)".to_string()];
        items.extend(
            self.available_entry_points
                .iter()
                .map(|p| p.display().to_string()),
        );
        items
    }

    /// Creates a new default config, adds it to the config list, and selects it.
    /// Returns the index of the newly created config.
    ///
    /// This is used when the user sets flavor or dart-defines without having
    /// a config selected - we auto-create a config to persist their choices.
    pub fn create_and_select_default_config(&mut self) -> usize {
        use crate::config::launch::create_default_launch_config;
        use crate::config::priority::SourcedConfig;
        use crate::config::types::ConfigSource;

        // Create a new default config with current mode
        let mut new_config = create_default_launch_config();
        new_config.mode = self.mode;

        // Generate unique name if "Default" already exists
        let existing_names: Vec<&str> = self
            .configs
            .configs
            .iter()
            .map(|c| c.config.name.as_str())
            .collect();

        let unique_name = generate_unique_name("Default", &existing_names);
        new_config.name = unique_name;

        // Wrap in SourcedConfig (FDemon source so it's editable and saveable)
        let config_with_source = SourcedConfig {
            display_name: new_config.name.clone(),
            config: new_config,
            source: ConfigSource::FDemon,
        };

        // Add to configs list
        self.configs.configs.push(config_with_source);

        // Select the new config
        let new_index = self.configs.configs.len() - 1;
        self.selected_config_index = Some(new_index);

        new_index
    }

    /// Returns the current configs as LaunchConfig for saving.
    /// Only includes FDemon configs (VSCode configs are read-only).
    pub fn get_fdemon_configs_for_save(&self) -> Vec<crate::config::LaunchConfig> {
        self.configs
            .configs
            .iter()
            .filter(|c| c.source == ConfigSource::FDemon)
            .map(|c| c.config.clone())
            .collect()
    }
}

/// Generate a unique name by appending numbers if needed.
/// "Default" -> "Default", "Default 2", "Default 3", etc.
/// Falls back to timestamp if counter exceeds limit.
fn generate_unique_name(base_name: &str, existing_names: &[&str]) -> String {
    if !existing_names.contains(&base_name) {
        return base_name.to_string();
    }

    // Bounded loop with reasonable limit
    const MAX_COUNTER: u32 = 1000;
    for counter in 2..=MAX_COUNTER {
        let candidate = format!("{} {}", base_name, counter);
        if !existing_names.contains(&candidate.as_str()) {
            return candidate;
        }
    }

    // Fallback to timestamp if all numbered names are taken
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("{} {}", base_name, timestamp)
}

// ─────────────────────────────────────────────────────────────────────────────
// TargetSelectorState
// ─────────────────────────────────────────────────────────────────────────────

// Re-export TargetSelectorState from TUI layer (it has widget-specific caching)
// The TUI widget version is the canonical one since it needs caching for performance
pub use crate::tui::widgets::new_session_dialog::target_selector::TargetSelectorState;

// ─────────────────────────────────────────────────────────────────────────────
// NewSessionDialogState (Main State)
// ─────────────────────────────────────────────────────────────────────────────

/// Complete state for the NewSessionDialog
#[derive(Debug, Clone)]
pub struct NewSessionDialogState {
    /// Target Selector (left pane) state
    pub target_selector: TargetSelectorState,

    /// Launch Context (right pane) state
    pub launch_context: LaunchContextState,

    /// Currently focused pane
    pub focused_pane: DialogPane,

    /// Active fuzzy search modal (if any)
    pub fuzzy_modal: Option<FuzzyModalState>,

    /// Active dart defines modal (if any)
    pub dart_defines_modal: Option<DartDefinesModalState>,

    /// Whether the dialog is visible
    pub visible: bool,
}

impl NewSessionDialogState {
    /// Create a new dialog state with loaded configs
    pub fn new(configs: LoadedConfigs) -> Self {
        Self {
            target_selector: TargetSelectorState::default(),
            launch_context: LaunchContextState::new(configs),
            focused_pane: DialogPane::TargetSelector,
            fuzzy_modal: None,
            dart_defines_modal: None,
            visible: true,
        }
    }

    /// Create with initial devices
    pub fn with_devices(configs: LoadedConfigs, devices: Vec<Device>) -> Self {
        let mut state = Self::new(configs);
        state.target_selector.set_connected_devices(devices);
        state
    }

    // ─────────────────────────────────────────────────────────
    // Pane Focus Methods
    // ─────────────────────────────────────────────────────────

    /// Toggle focus between panes
    pub fn toggle_pane_focus(&mut self) {
        // Don't toggle if modal is open
        if self.has_modal_open() {
            return;
        }
        self.focused_pane = self.focused_pane.toggle();
    }

    /// Set focus to specific pane
    pub fn set_pane_focus(&mut self, pane: DialogPane) {
        if !self.has_modal_open() {
            self.focused_pane = pane;
        }
    }

    /// Check if Target Selector is focused
    pub fn is_target_selector_focused(&self) -> bool {
        self.focused_pane == DialogPane::TargetSelector && !self.has_modal_open()
    }

    /// Check if Launch Context is focused
    pub fn is_launch_context_focused(&self) -> bool {
        self.focused_pane == DialogPane::LaunchContext && !self.has_modal_open()
    }

    // ─────────────────────────────────────────────────────────
    // Modal State Methods
    // ─────────────────────────────────────────────────────────

    /// Check if any modal is open
    pub fn has_modal_open(&self) -> bool {
        self.fuzzy_modal.is_some() || self.dart_defines_modal.is_some()
    }

    /// Check if fuzzy modal is open
    pub fn is_fuzzy_modal_open(&self) -> bool {
        self.fuzzy_modal.is_some()
    }

    /// Check if dart defines modal is open
    pub fn is_dart_defines_modal_open(&self) -> bool {
        self.dart_defines_modal.is_some()
    }

    /// Open fuzzy modal for config selection
    pub fn open_config_modal(&mut self) {
        debug_assert!(
            !self.has_modal_open(),
            "Cannot open config modal: another modal is already open"
        );

        let items: Vec<String> = self
            .launch_context
            .configs
            .configs
            .iter()
            .map(|c| c.display_name.clone())
            .collect();

        self.fuzzy_modal = Some(FuzzyModalState::new(FuzzyModalType::Config, items));
    }

    /// Open fuzzy modal for flavor selection
    pub fn open_flavor_modal(&mut self, known_flavors: Vec<String>) {
        debug_assert!(
            !self.has_modal_open(),
            "Cannot open flavor modal: another modal is already open"
        );

        self.fuzzy_modal = Some(FuzzyModalState::new(FuzzyModalType::Flavor, known_flavors));
    }

    /// Open dart defines modal
    pub fn open_dart_defines_modal(&mut self) {
        debug_assert!(
            !self.has_modal_open(),
            "Cannot open dart defines modal: another modal is already open"
        );

        let defines = self.launch_context.dart_defines.clone();
        self.dart_defines_modal = Some(DartDefinesModalState::new(defines));
    }

    /// Close any open modal
    pub fn close_modal(&mut self) {
        self.fuzzy_modal = None;
        self.dart_defines_modal = None;
    }

    /// Close fuzzy modal and apply selection
    pub fn close_fuzzy_modal_with_selection(&mut self) {
        if let Some(ref modal) = self.fuzzy_modal {
            let selected = modal.selected_value();
            match modal.modal_type {
                FuzzyModalType::Config => {
                    if let Some(name) = selected {
                        self.launch_context.select_config_by_name(&name);
                    }
                }
                FuzzyModalType::Flavor => {
                    self.launch_context.set_flavor(selected);
                }
                FuzzyModalType::EntryPoint => {
                    // Convert "(default)" to None, otherwise parse as PathBuf
                    let entry_point = selected.and_then(|s| {
                        if s == "(default)" {
                            None
                        } else {
                            Some(PathBuf::from(s))
                        }
                    });
                    self.launch_context.set_entry_point(entry_point);
                }
            }
        }
        self.fuzzy_modal = None;
    }

    /// Close dart defines modal and apply changes
    pub fn close_dart_defines_modal_with_changes(&mut self) {
        if let Some(ref modal) = self.dart_defines_modal {
            let defines = modal.defines.clone();
            self.launch_context.set_dart_defines(defines);
        }
        self.dart_defines_modal = None;
    }

    // ─────────────────────────────────────────────────────────
    // Launch Readiness
    // ─────────────────────────────────────────────────────────

    /// Check if ready to launch (device selected)
    pub fn is_ready_to_launch(&self) -> bool {
        self.selected_device().is_some()
    }

    /// Get selected device for launch
    pub fn selected_device(&self) -> Option<&Device> {
        self.target_selector.selected_connected_device()
    }

    /// Build launch parameters
    pub fn build_launch_params(&self) -> Option<LaunchParams> {
        let device = self.selected_device()?;

        Some(LaunchParams {
            device_id: device.id.clone(),
            mode: self.launch_context.mode,
            flavor: self.launch_context.flavor.clone(),
            dart_defines: self
                .launch_context
                .dart_defines
                .iter()
                .map(|d| d.to_arg())
                .collect(),
            config_name: self
                .launch_context
                .selected_config()
                .map(|c| c.display_name.clone()),
            entry_point: self.launch_context.entry_point.clone(),
        })
    }

    // ─────────────────────────────────────────────────────────
    // Dialog Visibility
    // ─────────────────────────────────────────────────────────

    /// Show the dialog
    pub fn show(&mut self) {
        self.visible = true;
        self.focused_pane = DialogPane::TargetSelector;
        self.close_modal();
    }

    /// Hide the dialog
    pub fn hide(&mut self) {
        self.visible = false;
        self.close_modal();
    }

    /// Reset dialog to initial state
    pub fn reset(&mut self) {
        self.focused_pane = DialogPane::TargetSelector;
        self.close_modal();
        self.target_selector.set_tab(TargetTab::Connected);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::priority::SourcedConfig;
    use crate::config::types::{ConfigSource, FlutterMode, LaunchConfig};

    #[test]
    fn test_create_and_select_default_config_empty_list() {
        let mut state = LaunchContextState::new(LoadedConfigs::default());
        state.mode = FlutterMode::Profile;

        let index = state.create_and_select_default_config();

        assert_eq!(index, 0);
        assert_eq!(state.configs.configs.len(), 1);
        assert_eq!(state.selected_config_index, Some(0));
        assert_eq!(state.configs.configs[0].config.name, "Default");
        assert_eq!(state.configs.configs[0].config.mode, FlutterMode::Profile);
        assert_eq!(state.configs.configs[0].source, ConfigSource::FDemon);
    }

    #[test]
    fn test_create_and_select_default_config_unique_naming() {
        let mut state = LaunchContextState::new(LoadedConfigs::default());

        // Add existing "Default" config
        state.configs.configs.push(SourcedConfig {
            config: LaunchConfig {
                name: "Default".to_string(),
                ..Default::default()
            },
            source: ConfigSource::FDemon,
            display_name: "Default".to_string(),
        });

        let index = state.create_and_select_default_config();

        assert_eq!(state.configs.configs.len(), 2);
        assert_eq!(state.configs.configs[index].config.name, "Default 2");
    }

    #[test]
    fn test_create_and_select_default_config_multiple_defaults() {
        let mut state = LaunchContextState::new(LoadedConfigs::default());

        // Add existing "Default" and "Default 2" configs
        state.configs.configs.push(SourcedConfig {
            config: LaunchConfig {
                name: "Default".to_string(),
                ..Default::default()
            },
            source: ConfigSource::FDemon,
            display_name: "Default".to_string(),
        });
        state.configs.configs.push(SourcedConfig {
            config: LaunchConfig {
                name: "Default 2".to_string(),
                ..Default::default()
            },
            source: ConfigSource::FDemon,
            display_name: "Default 2".to_string(),
        });

        let index = state.create_and_select_default_config();

        assert_eq!(state.configs.configs[index].config.name, "Default 3");
    }

    #[test]
    fn test_generate_unique_name_basic() {
        let existing: Vec<&str> = vec![];
        assert_eq!(generate_unique_name("Default", &existing), "Default");
    }

    #[test]
    fn test_generate_unique_name_increments() {
        let existing = vec!["Default", "Default 2"];
        assert_eq!(generate_unique_name("Default", &existing), "Default 3");
    }

    #[test]
    fn test_generate_unique_name_with_other_names() {
        assert_eq!(generate_unique_name("Default", &["Other"]), "Default");
    }

    #[test]
    fn test_generate_unique_name_with_default() {
        assert_eq!(generate_unique_name("Default", &["Default"]), "Default 2");
    }

    #[test]
    fn test_generate_unique_name_fallback() {
        // Create many existing names to trigger fallback
        let existing: Vec<String> = (2..=1000).map(|i| format!("Default {}", i)).collect();
        let existing_refs: Vec<&str> = std::iter::once("Default")
            .chain(existing.iter().map(|s| s.as_str()))
            .collect();

        let result = generate_unique_name("Default", &existing_refs);

        // Should use timestamp fallback, not panic or hang
        assert!(result.starts_with("Default "));
        assert!(!existing_refs.contains(&result.as_str()));
    }

    #[test]
    fn test_get_fdemon_configs_for_save() {
        let mut state = LaunchContextState::new(LoadedConfigs::default());

        state.configs.configs.push(SourcedConfig {
            config: LaunchConfig {
                name: "FDemon Config".to_string(),
                ..Default::default()
            },
            source: ConfigSource::FDemon,
            display_name: "FDemon Config".to_string(),
        });
        state.configs.configs.push(SourcedConfig {
            config: LaunchConfig {
                name: "VSCode Config".to_string(),
                ..Default::default()
            },
            source: ConfigSource::VSCode,
            display_name: "VSCode Config (VSCode)".to_string(),
        });

        let fdemon_configs = state.get_fdemon_configs_for_save();

        assert_eq!(fdemon_configs.len(), 1);
        assert_eq!(fdemon_configs[0].name, "FDemon Config");
    }

    #[test]
    fn test_launch_context_state_entry_point_default() {
        let state = LaunchContextState::new(LoadedConfigs::default());
        assert_eq!(state.entry_point, None);
        assert_eq!(state.entry_point_display(), "(default)");
    }

    #[test]
    fn test_launch_context_state_entry_point_set() {
        let mut state = LaunchContextState::new(LoadedConfigs::default());
        state.entry_point = Some(PathBuf::from("lib/main_dev.dart"));
        assert_eq!(state.entry_point_display(), "lib/main_dev.dart");
    }

    #[test]
    fn test_entry_point_editable_no_config() {
        let state = LaunchContextState::new(LoadedConfigs::default());
        // No config selected = editable
        assert!(state.is_entry_point_editable());
    }

    #[test]
    fn test_entry_point_editable_fdemon_config() {
        let mut state = LaunchContextState::new(LoadedConfigs::default());

        // Add FDemon config
        state.configs.configs.push(SourcedConfig {
            config: LaunchConfig {
                name: "FDemon Config".to_string(),
                ..Default::default()
            },
            source: ConfigSource::FDemon,
            display_name: "FDemon Config".to_string(),
        });
        state.selected_config_index = Some(0);

        // FDemon configs are editable
        assert!(state.is_entry_point_editable());
    }

    #[test]
    fn test_entry_point_not_editable_vscode_config() {
        let mut state = LaunchContextState::new(LoadedConfigs::default());

        // Add VSCode config
        state.configs.configs.push(SourcedConfig {
            config: LaunchConfig {
                name: "VSCode Config".to_string(),
                ..Default::default()
            },
            source: ConfigSource::VSCode,
            display_name: "VSCode Config (VSCode)".to_string(),
        });
        state.selected_config_index = Some(0);

        // VSCode configs are read-only
        assert!(!state.is_entry_point_editable());
    }

    #[test]
    fn test_set_entry_point_when_editable() {
        let mut state = LaunchContextState::new(LoadedConfigs::default());

        // No config selected = editable
        assert!(state.is_entry_point_editable());

        state.set_entry_point(Some(PathBuf::from("lib/main_prod.dart")));
        assert_eq!(state.entry_point, Some(PathBuf::from("lib/main_prod.dart")));
        assert_eq!(state.entry_point_display(), "lib/main_prod.dart");
    }

    #[test]
    fn test_set_entry_point_when_not_editable() {
        let mut state = LaunchContextState::new(LoadedConfigs::default());

        // Add VSCode config (read-only)
        state.configs.configs.push(SourcedConfig {
            config: LaunchConfig {
                name: "VSCode Config".to_string(),
                ..Default::default()
            },
            source: ConfigSource::VSCode,
            display_name: "VSCode Config (VSCode)".to_string(),
        });
        state.selected_config_index = Some(0);

        // Not editable
        assert!(!state.is_entry_point_editable());

        // Try to set entry point - should be ignored
        state.set_entry_point(Some(PathBuf::from("lib/main_prod.dart")));
        assert_eq!(state.entry_point, None);
        assert_eq!(state.entry_point_display(), "(default)");
    }

    #[test]
    fn test_select_config_applies_entry_point() {
        let mut configs = LoadedConfigs::default();
        configs.configs.push(SourcedConfig {
            config: LaunchConfig {
                name: "Dev".to_string(),
                entry_point: Some(PathBuf::from("lib/main_dev.dart")),
                ..Default::default()
            },
            source: ConfigSource::VSCode,
            display_name: "Dev".to_string(),
        });

        let mut state = LaunchContextState::new(configs);
        assert_eq!(state.entry_point, None);

        state.select_config(Some(0));
        assert_eq!(state.entry_point, Some(PathBuf::from("lib/main_dev.dart")));
    }

    #[test]
    fn test_select_config_without_entry_point_preserves_existing() {
        let mut configs = LoadedConfigs::default();
        configs.configs.push(SourcedConfig {
            config: LaunchConfig {
                name: "Basic".to_string(),
                entry_point: None, // No entry point
                ..Default::default()
            },
            source: ConfigSource::FDemon,
            display_name: "Basic".to_string(),
        });

        let mut state = LaunchContextState::new(configs);
        state.entry_point = Some(PathBuf::from("lib/existing.dart"));

        state.select_config(Some(0));
        // Entry point should be preserved since config doesn't specify one
        assert_eq!(state.entry_point, Some(PathBuf::from("lib/existing.dart")));
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // Phase 3 Task 03: Entry Point State Helper Methods Tests
    // ─────────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_entry_point_display_none() {
        let state = LaunchContextState::new(LoadedConfigs::default());
        assert_eq!(state.entry_point_display(), "(default)");
    }

    #[test]
    fn test_entry_point_display_some() {
        let mut state = LaunchContextState::new(LoadedConfigs::default());
        state.set_entry_point(Some(PathBuf::from("lib/main_dev.dart")));
        assert_eq!(state.entry_point_display(), "lib/main_dev.dart");
    }

    #[test]
    fn test_is_entry_point_editable_no_config() {
        let state = LaunchContextState::new(LoadedConfigs::default());
        // No config selected = editable
        assert!(state.is_entry_point_editable());
    }

    #[test]
    fn test_is_entry_point_editable_vscode_config() {
        let mut configs = LoadedConfigs::default();
        configs.configs.push(SourcedConfig {
            config: LaunchConfig::default(),
            source: ConfigSource::VSCode,
            display_name: "VSCode".to_string(),
        });

        let mut state = LaunchContextState::new(configs);
        state.selected_config_index = Some(0);

        // VSCode config = NOT editable
        assert!(!state.is_entry_point_editable());
    }

    #[test]
    fn test_is_entry_point_editable_fdemon_config() {
        let mut configs = LoadedConfigs::default();
        configs.configs.push(SourcedConfig {
            config: LaunchConfig::default(),
            source: ConfigSource::FDemon,
            display_name: "FDemon".to_string(),
        });

        let mut state = LaunchContextState::new(configs);
        state.selected_config_index = Some(0);

        // FDemon config = editable
        assert!(state.is_entry_point_editable());
    }

    #[test]
    fn test_entry_point_modal_items() {
        let mut state = LaunchContextState::new(LoadedConfigs::default());
        state.set_available_entry_points(vec![
            PathBuf::from("lib/main.dart"),
            PathBuf::from("lib/main_dev.dart"),
        ]);

        let items = state.entry_point_modal_items();

        assert_eq!(items.len(), 3);
        assert_eq!(items[0], "(default)");
        assert_eq!(items[1], "lib/main.dart");
        assert_eq!(items[2], "lib/main_dev.dart");
    }

    #[test]
    fn test_entry_point_modal_items_empty() {
        let state = LaunchContextState::new(LoadedConfigs::default());
        let items = state.entry_point_modal_items();

        assert_eq!(items.len(), 1);
        assert_eq!(items[0], "(default)");
    }
}
