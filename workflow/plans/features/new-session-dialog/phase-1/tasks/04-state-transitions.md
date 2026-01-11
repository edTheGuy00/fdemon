## Task: Implement State Transition Methods

**Objective**: Add methods to `NewSessionDialogState` for handling navigation and state changes.

**Depends on**: Task 03 (Message types)

**Estimated Time**: 30 minutes

### Background

State transition methods encapsulate the logic for responding to user interactions. These methods will be called by the message handler in `update.rs`.

### Scope

- `src/tui/widgets/new_session_dialog/state.rs`: Add transition methods

### Changes Required

**Add methods to `NewSessionDialogState`:**

```rust
impl NewSessionDialogState {
    // ... existing methods ...

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
            self.selected_target_index = 0;  // Reset selection

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
                self.dart_defines = config.config.dart_defines
                    .iter()
                    .map(|(k, v)| DartDefine { key: k.clone(), value: v.clone() })
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
                Some(_) => self.select_config(Some(count - 1)),  // Wrap to end
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
                Some(_) => self.select_config(Some(0)),  // Wrap to start
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

    /// Open fuzzy modal
    pub fn open_fuzzy_modal(&mut self, modal_type: FuzzyModalType) {
        let items = match modal_type {
            FuzzyModalType::Config => {
                self.configs.configs.iter()
                    .map(|c| c.display_name.clone())
                    .collect()
            }
            FuzzyModalType::Flavor => {
                // TODO: Get flavors from project analysis
                Vec::new()
            }
        };

        let indices: Vec<usize> = (0..items.len()).collect();

        self.fuzzy_modal = Some(FuzzyModalState {
            modal_type: Some(modal_type),
            query: String::new(),
            items,
            filtered_indices: indices,
            selected_index: 0,
        });
    }

    /// Close fuzzy modal
    pub fn close_fuzzy_modal(&mut self) {
        self.fuzzy_modal = None;
    }

    /// Open dart defines modal
    pub fn open_dart_defines_modal(&mut self) {
        self.dart_defines_modal = Some(DartDefinesModalState {
            defines: self.dart_defines.clone(),
            selected_index: 0,
            editing_key: String::new(),
            editing_value: String::new(),
            editing_field: DartDefineField::List,
        });
    }

    /// Close dart defines modal (saving changes)
    pub fn close_dart_defines_modal(&mut self) {
        if let Some(ref modal) = self.dart_defines_modal {
            self.dart_defines = modal.defines.clone();
        }
        self.dart_defines_modal = None;
    }
}
```

### Acceptance Criteria

1. Pane navigation: `switch_pane()`, `switch_tab()`, `toggle_tab()`
2. Target selector: `target_up()`, `target_down()`, `selected_*_device()`
3. Launch context: `context_up()`, `context_down()`, `cycle_mode()`
4. Data updates: `set_connected_devices()`, `set_bootable_devices()`, `mark_device_booting()`
5. Config selection: `select_config()`, `config_up()`, `config_down()`
6. Error handling: `set_error()`, `clear_error()`
7. Modal management: `open_fuzzy_modal()`, `close_fuzzy_modal()`, `open_dart_defines_modal()`, `close_dart_defines_modal()`
8. All methods properly documented
9. `cargo check` passes
10. `cargo clippy -- -D warnings` passes

### Testing

Add tests for navigation methods:

```rust
#[cfg(test)]
mod tests {
    // ... existing tests ...

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
        let mut state = NewSessionDialogState::new();
        state.connected_devices = vec![
            Device { id: "d1".into(), name: "Device 1".into(), ..Default::default() },
            Device { id: "d2".into(), name: "Device 2".into(), ..Default::default() },
        ];
        state.loading_connected = false;

        assert_eq!(state.selected_target_index, 0);
        state.target_down();
        assert_eq!(state.selected_target_index, 1);
        state.target_down();  // Wrap
        assert_eq!(state.selected_target_index, 0);
        state.target_up();  // Wrap back
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
}
```

### Notes

- These methods are called by handlers in `update.rs`
- Tab switching triggers loading flag for bootable devices
- Config selection populates mode/flavor/dart_defines from config
- Modal open/close manages Option state

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/tui/widgets/new_session_dialog/state.rs` | Added 28 state transition methods covering pane navigation, target selection, launch context management, device data updates, config selection, error handling, and modal state management. Added 10 comprehensive test cases. |

### Notable Decisions/Tradeoffs

1. **Device Selection Reset on Tab Switch**: When switching between Connected and Bootable tabs, the selected device index is reset to 0. This ensures users don't see an out-of-bounds selection when switching between tabs with different device counts.

2. **Lazy Loading for Bootable Devices**: The `switch_tab()` method only triggers bootable device loading when switching to the Bootable tab and the list is empty. This avoids unnecessary native tool calls (simctl/emulator) on startup.

3. **Config Selection Populates Fields**: The `select_config()` method automatically populates mode, flavor, and dart-defines from the selected configuration. This provides a seamless UX where selecting a config pre-fills the launch context.

4. **Modal Save Behavior**: The `close_dart_defines_modal()` method saves changes from the modal back to the main state, while `close_fuzzy_modal()` simply dismisses (selections are handled by confirmation in the modal).

5. **Wrapping Navigation**: Target and config navigation use modulo arithmetic for wrapping, providing intuitive circular navigation through lists.

### Testing Performed

- `cargo fmt` - Passed
- `cargo check` - Passed (1.19s)
- `cargo test --lib new_session_dialog` - Passed (12 tests)
- `cargo test --lib` - Passed (1361 tests, 0 failed)
- `cargo clippy -- -D warnings` - Passed (no warnings)

### Risks/Limitations

1. **No Validation on Device State Changes**: The `mark_device_booting()` method directly mutates device state without validation. Future implementation should ensure only Shutdown devices can transition to Booting.

2. **TODO for Flavor Discovery**: The `open_fuzzy_modal()` method has a placeholder for flavor discovery. This will need to be implemented in a future task using project analysis.

3. **Bounds Checking Edge Cases**: While selection reset logic handles out-of-bounds cases, rapid device list changes (e.g., device disconnects during navigation) could theoretically cause race conditions. This is mitigated by the single-threaded nature of the TUI event loop.
