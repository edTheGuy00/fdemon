## Task: Create NewSessionDialogState Structure

**Objective**: Define the complete state structure for the NewSessionDialog with dual-pane and tabbed layout.

**Depends on**: Task 01 (BootableDevice type)

**Estimated Time**: 30 minutes

### Background

The new dialog state must support:
- Left pane: Target Selector with Connected/Bootable tabs
- Right pane: Launch Context with config/mode/flavor/dart-defines
- Modal overlays: Fuzzy search and dart defines editor
- Dual focus tracking (pane and field within pane)

### Scope

- `src/tui/widgets/new_session_dialog/state.rs`: New file with state definitions
- `src/tui/widgets/new_session_dialog/mod.rs`: Module setup
- `src/tui/widgets/mod.rs`: Add module export

### Changes Required

**Create `src/tui/widgets/new_session_dialog/mod.rs`:**

```rust
//! NewSessionDialog - Unified session launch dialog
//!
//! Replaces DeviceSelector and StartupDialog with a single dialog featuring:
//! - Target Selector (left pane): Connected/Bootable device tabs
//! - Launch Context (right pane): Config, mode, flavor, dart-defines
//! - Fuzzy search modals for config/flavor selection
//! - Dart defines master-detail modal

mod state;

pub use state::*;

// Widget implementation comes in Phase 3-4
```

**Create `src/tui/widgets/new_session_dialog/state.rs`:**

```rust
//! State definitions for NewSessionDialog

use crate::config::{FlutterMode, LoadedConfigs};
use crate::core::BootableDevice;
use crate::daemon::Device;

/// Which pane has focus
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DialogPane {
    #[default]
    Left,   // Target Selector
    Right,  // Launch Context
}

/// Which tab is active in the Target Selector
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TargetTab {
    #[default]
    Connected,  // Running/connected devices
    Bootable,   // Offline simulators/AVDs
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

/// Type of fuzzy modal being shown
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FuzzyModalType {
    Config,
    Flavor,
}

/// State for the fuzzy search modal
#[derive(Debug, Clone, Default)]
pub struct FuzzyModalState {
    pub modal_type: Option<FuzzyModalType>,
    pub query: String,
    pub items: Vec<String>,
    pub filtered_indices: Vec<usize>,
    pub selected_index: usize,
}

/// State for the dart defines modal
#[derive(Debug, Clone, Default)]
pub struct DartDefinesModalState {
    pub defines: Vec<DartDefine>,
    pub selected_index: usize,
    pub editing_key: String,
    pub editing_value: String,
    pub editing_field: DartDefineField,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DartDefineField {
    #[default]
    List,
    Key,
    Value,
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
            loading_connected: true,  // Start loading by default
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
}
```

**Update `src/tui/widgets/mod.rs`:**

Add the new module export alongside existing widgets.

### Acceptance Criteria

1. `NewSessionDialogState` struct with all fields documented
2. `DialogPane`, `TargetTab`, `LaunchContextField` enums with navigation methods
3. `FuzzyModalState` and `DartDefinesModalState` for modal overlays
4. `DartDefine` type for key-value pairs
5. Constructor methods: `new()`, `with_configs()`
6. Helper methods: `tick()`, `has_modal_open()`, `current_device_count()`, `is_loading()`
7. Module properly exported from `tui/widgets`
8. `cargo check` passes
9. `cargo clippy -- -D warnings` passes

### Testing

Add inline tests in state.rs:

```rust
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
        assert_eq!(LaunchContextField::Launch.next(), LaunchContextField::Config);
        assert_eq!(LaunchContextField::Config.prev(), LaunchContextField::Launch);
    }
}
```

### Notes

- This state coexists with old `StartupDialogState` until Phase 7
- Modal states are `Option<T>` to indicate open/closed
- Loading states are per-tab to support async discovery
