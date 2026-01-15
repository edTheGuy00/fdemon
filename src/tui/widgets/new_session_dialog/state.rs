//! State definitions for NewSessionDialog

use crate::config::{FlutterMode, LoadedConfigs};
use crate::core::BootableDevice;
use crate::daemon::Device;

/// Which pane has focus
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DialogPane {
    #[default]
    Left, // Target Selector
    Right, // Launch Context
}

/// Which tab is active in the Target Selector
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TargetTab {
    #[default]
    Connected, // Running/connected devices
    Bootable, // Offline simulators/AVDs
}

impl TargetTab {
    pub fn label(&self) -> &'static str {
        match self {
            TargetTab::Connected => "1 Connected",
            TargetTab::Bootable => "2 Bootable",
        }
    }

    pub fn shortcut(&self) -> char {
        match self {
            TargetTab::Connected => '1',
            TargetTab::Bootable => '2',
        }
    }

    /// Get the other tab
    pub fn toggle(&self) -> Self {
        match self {
            TargetTab::Connected => TargetTab::Bootable,
            TargetTab::Bootable => TargetTab::Connected,
        }
    }
}

/// Which field is focused in the Launch Context pane
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LaunchContextField {
    #[default]
    Config,
    Mode,
    Flavor,
    DartDefines,
    Launch,
}

impl LaunchContextField {
    pub fn next(self) -> Self {
        match self {
            Self::Config => Self::Mode,
            Self::Mode => Self::Flavor,
            Self::Flavor => Self::DartDefines,
            Self::DartDefines => Self::Launch,
            Self::Launch => Self::Config,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::Config => Self::Launch,
            Self::Mode => Self::Config,
            Self::Flavor => Self::Mode,
            Self::DartDefines => Self::Flavor,
            Self::Launch => Self::DartDefines,
        }
    }
}

/// A single dart define key-value pair
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DartDefine {
    pub key: String,
    pub value: String,
}

impl DartDefine {
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
        }
    }
}

/// Type of fuzzy modal
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FuzzyModalType {
    /// Configuration selection (from LoadedConfigs)
    Config,
    /// Flavor selection (from project + custom)
    Flavor,
}

impl FuzzyModalType {
    /// Get the modal title
    pub fn title(&self) -> &'static str {
        match self {
            Self::Config => "Select Configuration",
            Self::Flavor => "Select Flavor",
        }
    }

    /// Whether custom input is allowed
    pub fn allows_custom(&self) -> bool {
        match self {
            Self::Config => false, // Must select from list
            Self::Flavor => true,  // Can type custom flavor
        }
    }
}

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
        self.update_filter();
    }

    /// Remove the last character from the query
    pub fn backspace(&mut self) {
        self.query.pop();
        self.update_filter();
    }

    /// Clear the query
    pub fn clear_query(&mut self) {
        self.query.clear();
        self.update_filter();
    }

    /// Update filtered indices based on current query
    pub fn update_filter(&mut self) {
        use super::fuzzy_modal::fuzzy_filter;

        // Reset selection when filter changes
        self.selected_index = 0;
        self.scroll_offset = 0;

        self.filtered_indices = fuzzy_filter(&self.query, &self.items);
    }
}

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

/// Complete state for the NewSessionDialog
#[derive(Debug, Clone)]
pub struct NewSessionDialogState {
    // ─────────────────────────────────────────────────────────
    // Pane Focus
    // ─────────────────────────────────────────────────────────
    /// Which pane has focus (Left = Target Selector, Right = Launch Context)
    pub active_pane: DialogPane,

    // ─────────────────────────────────────────────────────────
    // Target Selector (Left Pane)
    // ─────────────────────────────────────────────────────────
    /// Active tab (Connected or Bootable)
    pub target_tab: TargetTab,

    /// Connected/running devices (from flutter devices)
    pub connected_devices: Vec<Device>,

    /// Bootable/offline devices (from xcrun simctl, emulator -list-avds)
    pub bootable_devices: Vec<BootableDevice>,

    /// Selected index in current device list
    pub selected_target_index: usize,

    /// Loading state for connected devices
    pub loading_connected: bool,

    /// Loading state for bootable devices
    pub loading_bootable: bool,

    // ─────────────────────────────────────────────────────────
    // Launch Context (Right Pane)
    // ─────────────────────────────────────────────────────────
    /// Available launch configurations
    pub configs: LoadedConfigs,

    /// Selected config index (None = no config / new config)
    pub selected_config: Option<usize>,

    /// Build mode (Debug/Profile/Release)
    pub mode: FlutterMode,

    /// Flavor string
    pub flavor: String,

    /// Dart define key-value pairs
    pub dart_defines: Vec<DartDefine>,

    /// Active field in launch context
    pub active_field: LaunchContextField,

    // ─────────────────────────────────────────────────────────
    // Modals
    // ─────────────────────────────────────────────────────────
    /// Fuzzy search modal state (None = modal closed)
    pub fuzzy_modal: Option<FuzzyModalState>,

    /// Dart defines modal state (None = modal closed)
    pub dart_defines_modal: Option<DartDefinesModalState>,

    // ─────────────────────────────────────────────────────────
    // Common
    // ─────────────────────────────────────────────────────────
    /// Error message to display
    pub error: Option<String>,

    /// Animation frame counter
    pub animation_frame: u64,
}

impl Default for NewSessionDialogState {
    fn default() -> Self {
        Self::new()
    }
}

impl NewSessionDialogState {
    /// Create a new dialog state
    pub fn new() -> Self {
        Self {
            active_pane: DialogPane::Left,
            target_tab: TargetTab::Connected,
            connected_devices: Vec::new(),
            bootable_devices: Vec::new(),
            selected_target_index: 0,
            loading_connected: true, // Start loading by default
            loading_bootable: false,
            configs: LoadedConfigs::default(),
            selected_config: None,
            mode: FlutterMode::Debug,
            flavor: String::new(),
            dart_defines: Vec::new(),
            active_field: LaunchContextField::Config,
            fuzzy_modal: None,
            dart_defines_modal: None,
            error: None,
            animation_frame: 0,
        }
    }

    /// Create with pre-loaded configs
    pub fn with_configs(configs: LoadedConfigs) -> Self {
        let mut state = Self::new();
        state.configs = configs;
        // Auto-select first config if available
        if !state.configs.configs.is_empty() {
            state.selected_config = Some(0);
        }
        state
    }

    /// Advance animation frame
    pub fn tick(&mut self) {
        self.animation_frame = self.animation_frame.wrapping_add(1);
    }

    /// Check if a modal is open
    pub fn has_modal_open(&self) -> bool {
        self.fuzzy_modal.is_some() || self.dart_defines_modal.is_some()
    }

    /// Get current device list based on active tab
    pub fn current_device_count(&self) -> usize {
        match self.target_tab {
            TargetTab::Connected => self.connected_devices.len(),
            TargetTab::Bootable => self.bootable_devices.len(),
        }
    }

    /// Check if currently loading (for either tab)
    pub fn is_loading(&self) -> bool {
        match self.target_tab {
            TargetTab::Connected => self.loading_connected,
            TargetTab::Bootable => self.loading_bootable,
        }
    }

    // ─────────────────────────────────────────────────────────
    // Pane Navigation
    // ─────────────────────────────────────────────────────────

    /// Switch focus between left and right panes
    pub fn switch_pane(&mut self) {
        self.active_pane = match self.active_pane {
            DialogPane::Left => DialogPane::Right,
            DialogPane::Right => DialogPane::Left,
        };
    }

    /// Switch to a specific tab
    pub fn switch_tab(&mut self, tab: TargetTab) {
        if self.target_tab != tab {
            self.target_tab = tab;
            self.selected_target_index = 0; // Reset selection

            // Trigger loading if switching to bootable and not loaded
            if tab == TargetTab::Bootable && self.bootable_devices.is_empty() {
                self.loading_bootable = true;
            }
        }
    }

    /// Toggle between Connected and Bootable tabs
    pub fn toggle_tab(&mut self) {
        let new_tab = match self.target_tab {
            TargetTab::Connected => TargetTab::Bootable,
            TargetTab::Bootable => TargetTab::Connected,
        };
        self.switch_tab(new_tab);
    }

    // ─────────────────────────────────────────────────────────
    // Target Selector Navigation (Left Pane)
    // ─────────────────────────────────────────────────────────

    /// Navigate up in device list
    pub fn target_up(&mut self) {
        let count = self.current_device_count();
        if count > 0 {
            self.selected_target_index = if self.selected_target_index == 0 {
                count - 1
            } else {
                self.selected_target_index - 1
            };
        }
    }

    /// Navigate down in device list
    pub fn target_down(&mut self) {
        let count = self.current_device_count();
        if count > 0 {
            self.selected_target_index = (self.selected_target_index + 1) % count;
        }
    }

    /// Get currently selected connected device
    pub fn selected_connected_device(&self) -> Option<&Device> {
        if self.target_tab == TargetTab::Connected {
            self.connected_devices.get(self.selected_target_index)
        } else {
            None
        }
    }

    /// Get currently selected bootable device
    pub fn selected_bootable_device(&self) -> Option<&BootableDevice> {
        if self.target_tab == TargetTab::Bootable {
            self.bootable_devices.get(self.selected_target_index)
        } else {
            None
        }
    }

    // ─────────────────────────────────────────────────────────
    // Launch Context Navigation (Right Pane)
    // ─────────────────────────────────────────────────────────

    /// Navigate up in launch context (previous field)
    pub fn context_up(&mut self) {
        self.active_field = self.active_field.prev();
    }

    /// Navigate down in launch context (next field)
    pub fn context_down(&mut self) {
        self.active_field = self.active_field.next();
    }

    /// Cycle mode (Debug -> Profile -> Release -> Debug)
    pub fn cycle_mode(&mut self) {
        self.mode = match self.mode {
            FlutterMode::Debug => FlutterMode::Profile,
            FlutterMode::Profile => FlutterMode::Release,
            FlutterMode::Release => FlutterMode::Debug,
        };
    }

    /// Cycle mode backwards
    pub fn cycle_mode_reverse(&mut self) {
        self.mode = match self.mode {
            FlutterMode::Debug => FlutterMode::Release,
            FlutterMode::Profile => FlutterMode::Debug,
            FlutterMode::Release => FlutterMode::Profile,
        };
    }

    // ─────────────────────────────────────────────────────────
    // Device Data Updates
    // ─────────────────────────────────────────────────────────

    /// Set connected devices from discovery
    pub fn set_connected_devices(&mut self, devices: Vec<Device>) {
        self.connected_devices = devices;
        self.loading_connected = false;

        // Reset selection if out of bounds
        if self.selected_target_index >= self.connected_devices.len() {
            self.selected_target_index = 0;
        }
    }

    /// Set bootable devices from native discovery
    pub fn set_bootable_devices(&mut self, devices: Vec<BootableDevice>) {
        self.bootable_devices = devices;
        self.loading_bootable = false;

        // Reset selection if out of bounds
        if self.target_tab == TargetTab::Bootable
            && self.selected_target_index >= self.bootable_devices.len()
        {
            self.selected_target_index = 0;
        }
    }

    /// Mark a bootable device as booting
    pub fn mark_device_booting(&mut self, device_id: &str) {
        if let Some(device) = self.bootable_devices.iter_mut().find(|d| d.id == device_id) {
            device.state = crate::core::DeviceState::Booting;
        }
    }

    /// Handle device boot completion - switch to Connected tab
    pub fn handle_device_booted(&mut self) {
        // Switch to Connected tab and trigger refresh
        self.target_tab = TargetTab::Connected;
        self.loading_connected = true;
        self.selected_target_index = 0;
    }

    // ─────────────────────────────────────────────────────────
    // Config Selection
    // ─────────────────────────────────────────────────────────

    /// Select a config by index
    pub fn select_config(&mut self, index: Option<usize>) {
        self.selected_config = index;

        // If a config is selected, populate fields from it
        if let Some(idx) = index {
            if let Some(config) = self.configs.configs.get(idx) {
                self.mode = config.config.mode;
                if let Some(ref flavor) = config.config.flavor {
                    self.flavor = flavor.clone();
                }
                // Convert dart_defines HashMap to Vec<DartDefine>
                self.dart_defines = config
                    .config
                    .dart_defines
                    .iter()
                    .map(|(k, v)| DartDefine {
                        key: k.clone(),
                        value: v.clone(),
                    })
                    .collect();
            }
        }
    }

    /// Navigate config up
    pub fn config_up(&mut self) {
        let count = self.configs.configs.len();
        if count > 0 {
            match self.selected_config {
                Some(idx) if idx > 0 => self.select_config(Some(idx - 1)),
                Some(_) => self.select_config(Some(count - 1)), // Wrap to end
                None => self.select_config(Some(count - 1)),
            }
        }
    }

    /// Navigate config down
    pub fn config_down(&mut self) {
        let count = self.configs.configs.len();
        if count > 0 {
            match self.selected_config {
                Some(idx) if idx < count - 1 => self.select_config(Some(idx + 1)),
                Some(_) => self.select_config(Some(0)), // Wrap to start
                None => self.select_config(Some(0)),
            }
        }
    }

    // ─────────────────────────────────────────────────────────
    // Error Handling
    // ─────────────────────────────────────────────────────────

    /// Set an error message
    pub fn set_error(&mut self, error: String) {
        self.error = Some(error);
    }

    /// Clear error message
    pub fn clear_error(&mut self) {
        self.error = None;
    }

    // ─────────────────────────────────────────────────────────
    // Modal State
    // ─────────────────────────────────────────────────────────

    /// Open fuzzy modal with items
    pub fn open_fuzzy_modal(&mut self, modal_type: FuzzyModalType, items: Vec<String>) {
        self.fuzzy_modal = Some(FuzzyModalState::new(modal_type, items));
    }

    /// Close fuzzy modal
    pub fn close_fuzzy_modal(&mut self) {
        self.fuzzy_modal = None;
    }

    /// Open dart defines modal with current defines
    pub fn open_dart_defines_modal(&mut self) {
        let defines = self.dart_defines.clone();
        self.dart_defines_modal = Some(DartDefinesModalState::new(defines));
    }

    /// Close dart defines modal and apply changes
    pub fn close_dart_defines_modal(&mut self) {
        if let Some(modal) = self.dart_defines_modal.take() {
            self.dart_defines = modal.defines;
        }
    }

    /// Check if dart defines modal is open
    pub fn is_dart_defines_modal_open(&self) -> bool {
        self.dart_defines_modal.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_session_dialog_state_default() {
        let state = NewSessionDialogState::new();
        assert_eq!(state.active_pane, DialogPane::Left);
        assert_eq!(state.target_tab, TargetTab::Connected);
        assert!(state.loading_connected);
        assert!(!state.has_modal_open());
    }

    #[test]
    fn test_launch_context_field_navigation() {
        assert_eq!(LaunchContextField::Config.next(), LaunchContextField::Mode);
        assert_eq!(
            LaunchContextField::Launch.next(),
            LaunchContextField::Config
        );
        assert_eq!(
            LaunchContextField::Config.prev(),
            LaunchContextField::Launch
        );
    }

    #[test]
    fn test_pane_navigation() {
        let mut state = NewSessionDialogState::new();
        assert_eq!(state.active_pane, DialogPane::Left);

        state.switch_pane();
        assert_eq!(state.active_pane, DialogPane::Right);

        state.switch_pane();
        assert_eq!(state.active_pane, DialogPane::Left);
    }

    #[test]
    fn test_tab_switching() {
        let mut state = NewSessionDialogState::new();
        assert_eq!(state.target_tab, TargetTab::Connected);

        state.toggle_tab();
        assert_eq!(state.target_tab, TargetTab::Bootable);
        assert!(state.loading_bootable);
    }

    #[test]
    fn test_target_navigation_wrapping() {
        use crate::daemon::Device;

        let mut state = NewSessionDialogState::new();
        state.connected_devices = vec![
            Device {
                id: "d1".into(),
                name: "Device 1".into(),
                platform: "ios".into(),
                emulator: false,
                category: None,
                platform_type: None,
                ephemeral: false,
                emulator_id: None,
            },
            Device {
                id: "d2".into(),
                name: "Device 2".into(),
                platform: "android".into(),
                emulator: false,
                category: None,
                platform_type: None,
                ephemeral: false,
                emulator_id: None,
            },
        ];
        state.loading_connected = false;

        assert_eq!(state.selected_target_index, 0);
        state.target_down();
        assert_eq!(state.selected_target_index, 1);
        state.target_down(); // Wrap
        assert_eq!(state.selected_target_index, 0);
        state.target_up(); // Wrap back
        assert_eq!(state.selected_target_index, 1);
    }

    #[test]
    fn test_mode_cycling() {
        let mut state = NewSessionDialogState::new();
        assert_eq!(state.mode, FlutterMode::Debug);

        state.cycle_mode();
        assert_eq!(state.mode, FlutterMode::Profile);

        state.cycle_mode();
        assert_eq!(state.mode, FlutterMode::Release);

        state.cycle_mode();
        assert_eq!(state.mode, FlutterMode::Debug);
    }

    #[test]
    fn test_mode_cycling_reverse() {
        let mut state = NewSessionDialogState::new();
        assert_eq!(state.mode, FlutterMode::Debug);

        state.cycle_mode_reverse();
        assert_eq!(state.mode, FlutterMode::Release);

        state.cycle_mode_reverse();
        assert_eq!(state.mode, FlutterMode::Profile);

        state.cycle_mode_reverse();
        assert_eq!(state.mode, FlutterMode::Debug);
    }

    #[test]
    fn test_set_connected_devices() {
        use crate::daemon::Device;

        let mut state = NewSessionDialogState::new();
        assert!(state.loading_connected);

        let devices = vec![Device {
            id: "d1".into(),
            name: "Device 1".into(),
            platform: "ios".into(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        }];

        state.set_connected_devices(devices);
        assert!(!state.loading_connected);
        assert_eq!(state.connected_devices.len(), 1);
    }

    #[test]
    fn test_error_handling() {
        let mut state = NewSessionDialogState::new();
        assert!(state.error.is_none());

        state.set_error("Test error".to_string());
        assert_eq!(state.error, Some("Test error".to_string()));

        state.clear_error();
        assert!(state.error.is_none());
    }

    #[test]
    fn test_selected_device_getters() {
        use crate::core::{BootableDevice, DeviceState, Platform};
        use crate::daemon::Device;

        let mut state = NewSessionDialogState::new();
        state.connected_devices = vec![Device {
            id: "d1".into(),
            name: "Device 1".into(),
            platform: "ios".into(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        }];
        state.bootable_devices = vec![BootableDevice {
            id: "sim1".into(),
            name: "Simulator 1".into(),
            platform: Platform::IOS,
            runtime: "iOS 17.2".into(),
            state: DeviceState::Shutdown,
        }];

        // Connected tab returns connected device
        state.target_tab = TargetTab::Connected;
        assert!(state.selected_connected_device().is_some());
        assert!(state.selected_bootable_device().is_none());

        // Bootable tab returns bootable device
        state.target_tab = TargetTab::Bootable;
        assert!(state.selected_connected_device().is_none());
        assert!(state.selected_bootable_device().is_some());
    }

    #[test]
    fn test_modal_management() {
        let mut state = NewSessionDialogState::new();
        assert!(!state.has_modal_open());

        state.open_fuzzy_modal(
            FuzzyModalType::Config,
            vec!["config1".into(), "config2".into()],
        );
        assert!(state.has_modal_open());
        assert!(state.fuzzy_modal.is_some());

        state.close_fuzzy_modal();
        assert!(!state.has_modal_open());
        assert!(state.fuzzy_modal.is_none());
    }

    #[test]
    fn test_dart_defines_modal() {
        let mut state = NewSessionDialogState::new();
        state.dart_defines = vec![DartDefine {
            key: "API_URL".into(),
            value: "https://api.com".into(),
        }];

        state.open_dart_defines_modal();
        assert!(state.dart_defines_modal.is_some());
        let modal = state.dart_defines_modal.as_ref().unwrap();
        assert_eq!(modal.defines.len(), 1);

        // Close saves changes
        state.close_dart_defines_modal();
        assert!(state.dart_defines_modal.is_none());
    }
}

#[cfg(test)]
mod fuzzy_modal_tests {
    use super::*;

    #[test]
    fn test_fuzzy_modal_new() {
        let items = vec!["alpha".into(), "beta".into(), "gamma".into()];
        let state = FuzzyModalState::new(FuzzyModalType::Flavor, items);

        assert_eq!(state.query, "");
        assert_eq!(state.filtered_indices.len(), 3);
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn test_fuzzy_navigation() {
        let items = vec!["a".into(), "b".into(), "c".into()];
        let mut state = FuzzyModalState::new(FuzzyModalType::Config, items);

        assert_eq!(state.selected_index, 0);
        state.navigate_down();
        assert_eq!(state.selected_index, 1);
        state.navigate_down();
        assert_eq!(state.selected_index, 2);
        state.navigate_down(); // Wrap
        assert_eq!(state.selected_index, 0);
        state.navigate_up(); // Wrap back
        assert_eq!(state.selected_index, 2);
    }

    #[test]
    fn test_fuzzy_filter_basic() {
        let items = vec!["dev".into(), "staging".into(), "production".into()];
        let mut state = FuzzyModalState::new(FuzzyModalType::Flavor, items);

        state.input_char('d');
        assert_eq!(state.filtered_indices.len(), 2); // dev, production

        state.input_char('e');
        assert_eq!(state.filtered_indices.len(), 1); // dev only (production doesn't have "de")

        state.input_char('v');
        assert_eq!(state.filtered_indices.len(), 1); // dev only
    }

    #[test]
    fn test_fuzzy_custom_value() {
        let items = vec!["existing".into()];
        let mut state = FuzzyModalState::new(FuzzyModalType::Flavor, items);

        state.input_char('c');
        state.input_char('u');
        state.input_char('s');
        state.input_char('t');
        state.input_char('o');
        state.input_char('m');

        // No matches, but Flavor allows custom
        assert!(!state.has_results());
        assert_eq!(state.selected_value(), Some("custom".into()));
    }

    #[test]
    fn test_config_no_custom() {
        let items = vec!["existing".into()];
        let mut state = FuzzyModalState::new(FuzzyModalType::Config, items);

        state.input_char('z'); // No match - 'z' not in "existing"

        assert!(!state.has_results());
        assert_eq!(state.selected_value(), None); // Config doesn't allow custom
    }
}

#[cfg(test)]
mod dart_defines_modal_tests {
    use super::*;

    #[test]
    fn test_dart_defines_modal_new() {
        let defines = vec![
            DartDefine::new("API_KEY", "secret"),
            DartDefine::new("DEBUG", "true"),
        ];
        let state = DartDefinesModalState::new(defines);

        assert_eq!(state.defines.len(), 2);
        assert_eq!(state.selected_index, 0);
        assert_eq!(state.list_item_count(), 3); // 2 defines + Add New
    }

    #[test]
    fn test_navigation_wraps() {
        let defines = vec![DartDefine::new("A", "1")];
        let mut state = DartDefinesModalState::new(defines);

        assert_eq!(state.selected_index, 0);
        state.navigate_down();
        assert_eq!(state.selected_index, 1); // Add New
        state.navigate_down();
        assert_eq!(state.selected_index, 0); // Wrap to first
        state.navigate_up();
        assert_eq!(state.selected_index, 1); // Wrap to Add New
    }

    #[test]
    fn test_load_existing_into_edit() {
        let defines = vec![DartDefine::new("KEY", "value")];
        let mut state = DartDefinesModalState::new(defines);

        state.load_selected_into_edit();

        assert_eq!(state.editing_key, "KEY");
        assert_eq!(state.editing_value, "value");
        assert!(!state.is_new);
        assert_eq!(state.active_pane, DartDefinesPane::Edit);
    }

    #[test]
    fn test_load_add_new_into_edit() {
        let defines = vec![DartDefine::new("KEY", "value")];
        let mut state = DartDefinesModalState::new(defines);

        state.navigate_down(); // Select Add New
        state.load_selected_into_edit();

        assert_eq!(state.editing_key, "");
        assert_eq!(state.editing_value, "");
        assert!(state.is_new);
    }

    #[test]
    fn test_save_new_define() {
        let mut state = DartDefinesModalState::new(vec![]);

        state.is_new = true;
        state.editing_key = "NEW_KEY".into();
        state.editing_value = "new_value".into();

        assert!(state.save_edit());
        assert_eq!(state.defines.len(), 1);
        assert_eq!(state.defines[0].key, "NEW_KEY");
    }

    #[test]
    fn test_save_empty_key_fails() {
        let mut state = DartDefinesModalState::new(vec![]);

        state.is_new = true;
        state.editing_key = "   ".into(); // Only whitespace

        assert!(!state.save_edit());
        assert!(state.defines.is_empty());
    }

    #[test]
    fn test_delete_define() {
        let defines = vec![DartDefine::new("A", "1"), DartDefine::new("B", "2")];
        let mut state = DartDefinesModalState::new(defines);

        state.selected_index = 0;
        assert!(state.delete_selected());

        assert_eq!(state.defines.len(), 1);
        assert_eq!(state.defines[0].key, "B");
    }

    #[test]
    fn test_cannot_delete_add_new() {
        let defines = vec![DartDefine::new("A", "1")];
        let mut state = DartDefinesModalState::new(defines);

        state.selected_index = 1; // Add New
        assert!(!state.delete_selected());
        assert_eq!(state.defines.len(), 1);
    }

    #[test]
    fn test_edit_field_tab_order() {
        let field = DartDefinesEditField::Key;
        assert_eq!(field.next(), DartDefinesEditField::Value);
        assert_eq!(field.next().next(), DartDefinesEditField::Save);
        assert_eq!(field.next().next().next(), DartDefinesEditField::Delete);
        assert_eq!(field.next().next().next().next(), DartDefinesEditField::Key);
    }

    #[test]
    fn test_delete_middle_item_adjusts_selection() {
        // Test that deleting middle item keeps selection in valid range
        let defines = vec![
            DartDefine::new("A", "1"),
            DartDefine::new("B", "2"),
            DartDefine::new("C", "3"),
        ];
        let mut state = DartDefinesModalState::new(defines);

        // Delete middle item (index 1 = "B")
        state.selected_index = 1;
        assert!(state.delete_selected());

        // After deletion: ["A", "C"], selected_index should be 1 (now "C")
        assert_eq!(state.defines.len(), 2);
        assert_eq!(state.selected_index, 1);
        assert_eq!(state.defines[1].key, "C");
    }

    #[test]
    fn test_delete_last_item_clamps_selection() {
        // Test that deleting last item clamps selection to new last item
        let defines = vec![DartDefine::new("A", "1"), DartDefine::new("B", "2")];
        let mut state = DartDefinesModalState::new(defines);

        // Delete last item (index 1 = "B")
        state.selected_index = 1;
        assert!(state.delete_selected());

        // After deletion: ["A"], selected_index should be 0 (clamped)
        assert_eq!(state.defines.len(), 1);
        assert_eq!(state.selected_index, 0);
        assert_eq!(state.defines[0].key, "A");
    }

    #[test]
    fn test_delete_only_item_points_to_add_new() {
        // Test that deleting the only item leaves selection at 0 (Add New)
        let defines = vec![DartDefine::new("A", "1")];
        let mut state = DartDefinesModalState::new(defines);

        // Delete only item (index 0 = "A")
        state.selected_index = 0;
        assert!(state.delete_selected());

        // After deletion: [], selected_index should be 0 (Add New)
        assert!(state.defines.is_empty());
        assert_eq!(state.selected_index, 0);
        assert!(state.is_add_new_selected());
    }
}
