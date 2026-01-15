//! NewSessionDialog complete state management

use super::types::{
    DartDefine, DialogPane, FuzzyModalType, LaunchContextField, LaunchParams, TargetTab,
};
use crate::config::{ConfigSource, FlutterMode, LoadedConfigs};
use crate::daemon::Device;

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
