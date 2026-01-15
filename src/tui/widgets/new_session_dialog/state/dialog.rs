//! NewSessionDialog complete state management

use super::{
    dart_defines::{DartDefine, DartDefinesModalState},
    fuzzy_modal::{FuzzyModalState, FuzzyModalType},
    types::{DialogPane, LaunchContextField, TargetTab},
};
use crate::config::{FlutterMode, LoadedConfigs};
use crate::core::BootableDevice;
use crate::daemon::Device;

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
            // Reset selection to first selectable device (skip headers)
            self.selected_target_index = self.first_selectable_target_index();
            // Note: Handler is responsible for setting loading flags
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

    /// Get the first selectable device index in the current tab
    /// This ensures we don't select a header when switching tabs.
    ///
    /// The device lists are stored flat in state, but rendering groups them with headers.
    /// When devices are grouped by platform, the first item (index 0) is always a header.
    /// The first selectable device is at index 1 (after the first header).
    fn first_selectable_target_index(&self) -> usize {
        match self.target_tab {
            TargetTab::Connected => {
                // If we have devices, they'll be grouped with headers during rendering
                // First header at 0, first device at 1
                if !self.connected_devices.is_empty() {
                    1
                } else {
                    0
                }
            }
            TargetTab::Bootable => {
                // Same logic for bootable devices
                if !self.bootable_devices.is_empty() {
                    1
                } else {
                    0
                }
            }
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
        self.error = None; // Clear error on successful load

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
    // Editability Checks
    // ─────────────────────────────────────────────────────────

    /// Check if mode is editable based on config source
    pub fn is_mode_editable(&self) -> bool {
        use crate::config::ConfigSource;

        if let Some(idx) = self.selected_config {
            if let Some(config) = self.configs.configs.get(idx) {
                return config.source != ConfigSource::VSCode;
            }
        }
        // No config selected or invalid index = editable
        true
    }

    /// Check if flavor is editable based on config source
    pub fn is_flavor_editable(&self) -> bool {
        use crate::config::ConfigSource;

        if let Some(idx) = self.selected_config {
            if let Some(config) = self.configs.configs.get(idx) {
                return config.source != ConfigSource::VSCode;
            }
        }
        // No config selected or invalid index = editable
        true
    }

    /// Check if dart defines are editable based on config source
    pub fn are_dart_defines_editable(&self) -> bool {
        use crate::config::ConfigSource;

        if let Some(idx) = self.selected_config {
            if let Some(config) = self.configs.configs.get(idx) {
                return config.source != ConfigSource::VSCode;
            }
        }
        // No config selected or invalid index = editable
        true
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
