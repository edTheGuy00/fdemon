# Task: Dialog State

## Summary

Create the main `NewSessionDialogState` that combines Target Selector, Launch Context, and modal states into a unified dialog state.

## Files

| File | Action |
|------|--------|
| `src/tui/widgets/new_session_dialog/state.rs` | Modify (add main state) |

## Implementation

### 1. Define dialog pane enum

```rust
// src/tui/widgets/new_session_dialog/state.rs

/// Which pane is currently focused
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DialogPane {
    #[default]
    TargetSelector,
    LaunchContext,
}

impl DialogPane {
    pub fn toggle(self) -> Self {
        match self {
            DialogPane::TargetSelector => DialogPane::LaunchContext,
            DialogPane::LaunchContext => DialogPane::TargetSelector,
        }
    }
}
```

### 2. Main dialog state

```rust
use super::target_selector::TargetSelectorState;
use super::fuzzy_modal::FuzzyModalState;
use super::dart_defines_modal::DartDefinesModalState;
use crate::config::LoadedConfigs;

/// State for the NewSessionDialog
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
}
```

### 3. Pane focus methods

```rust
impl NewSessionDialogState {
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
}
```

### 4. Modal state methods

```rust
impl NewSessionDialogState {
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
        let items: Vec<String> = self.launch_context.configs.configs
            .iter()
            .map(|c| c.display_name.clone())
            .collect();

        self.fuzzy_modal = Some(FuzzyModalState::new(
            FuzzyModalType::Config,
            items,
            false, // No custom input
        ));
    }

    /// Open fuzzy modal for flavor selection
    pub fn open_flavor_modal(&mut self, known_flavors: Vec<String>) {
        self.fuzzy_modal = Some(FuzzyModalState::new(
            FuzzyModalType::Flavor,
            known_flavors,
            true, // Allow custom input
        ));
    }

    /// Open dart defines modal
    pub fn open_dart_defines_modal(&mut self) {
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
            let selected = modal.selected_item();
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
            let defines = modal.get_defines();
            self.launch_context.set_dart_defines(defines);
        }
        self.dart_defines_modal = None;
    }
}
```

### 5. Launch readiness

```rust
impl NewSessionDialogState {
    /// Check if ready to launch (device selected)
    pub fn is_ready_to_launch(&self) -> bool {
        self.target_selector.selected_connected_device().is_some()
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
            dart_defines: self.launch_context.dart_defines
                .iter()
                .map(|d| d.to_arg())
                .collect(),
            config_name: self.launch_context.selected_config()
                .map(|c| c.display_name.clone()),
        })
    }
}

/// Parameters for launching a Flutter session
#[derive(Debug, Clone)]
pub struct LaunchParams {
    pub device_id: String,
    pub mode: FlutterMode,
    pub flavor: Option<String>,
    pub dart_defines: Vec<String>,
    pub config_name: Option<String>,
}
```

### 6. Dialog visibility

```rust
impl NewSessionDialogState {
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
```

## Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_dialog_state() {
        let state = NewSessionDialogState::new(LoadedConfigs::default());

        assert!(state.visible);
        assert_eq!(state.focused_pane, DialogPane::TargetSelector);
        assert!(!state.has_modal_open());
    }

    #[test]
    fn test_toggle_pane_focus() {
        let mut state = NewSessionDialogState::new(LoadedConfigs::default());

        state.toggle_pane_focus();
        assert_eq!(state.focused_pane, DialogPane::LaunchContext);

        state.toggle_pane_focus();
        assert_eq!(state.focused_pane, DialogPane::TargetSelector);
    }

    #[test]
    fn test_modal_blocks_pane_toggle() {
        let mut state = NewSessionDialogState::new(LoadedConfigs::default());
        state.open_config_modal();

        state.toggle_pane_focus();

        // Pane focus should not change when modal is open
        assert_eq!(state.focused_pane, DialogPane::TargetSelector);
    }

    #[test]
    fn test_is_ready_to_launch() {
        let mut state = NewSessionDialogState::new(LoadedConfigs::default());
        assert!(!state.is_ready_to_launch());

        state.target_selector.set_connected_devices(vec![
            test_device_full("1", "iPhone", "ios", false),
        ]);
        assert!(state.is_ready_to_launch());
    }

    #[test]
    fn test_close_fuzzy_modal_applies_selection() {
        let mut state = NewSessionDialogState::new(LoadedConfigs::default());
        state.open_flavor_modal(vec!["dev".to_string(), "prod".to_string()]);

        // Simulate selection
        state.fuzzy_modal.as_mut().unwrap().select_next();

        state.close_fuzzy_modal_with_selection();

        assert!(state.fuzzy_modal.is_none());
        // Flavor should be set (either "dev" or "prod" depending on selection)
    }
}
```

## Verification

```bash
cargo fmt && cargo check && cargo test dialog_state && cargo clippy -- -D warnings
```

## Notes

- Dialog state is the single source of truth for the entire dialog
- Modal state prevents pane focus changes
- Launch requires a device to be selected
- Reset clears modals and returns to Connected tab
