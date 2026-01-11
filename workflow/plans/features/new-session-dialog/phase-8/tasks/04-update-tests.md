# Task: Update Tests

## Summary

Update all test files to use NewSessionDialog instead of the removed DeviceSelector and StartupDialog.

## Files to Update

| File | Changes Needed |
|------|----------------|
| `src/app/handler/tests.rs` | Update handler tests |
| `src/tui/widgets/log_view/tests.rs` | May reference dialog states |
| `tests/integration/*.rs` | Update integration tests |
| Any snapshot tests | Update expected output |

## Implementation

### 1. Handler tests

```rust
// src/app/handler/tests.rs

// Replace StartupDialog tests with NewSessionDialog tests

#[cfg(test)]
mod new_session_dialog_tests {
    use super::*;
    use crate::tui::widgets::new_session_dialog::*;

    fn create_test_dialog_state() -> NewSessionDialogState {
        NewSessionDialogState::new(LoadedConfigs::default())
    }

    fn create_test_state_with_dialog() -> AppState {
        let mut state = AppState::new(PathBuf::from("/test"));
        state.new_session_dialog = Some(create_test_dialog_state());
        state.ui_mode = UiMode::NewSessionDialog;
        state
    }

    #[test]
    fn test_open_dialog() {
        let mut state = AppState::new(PathBuf::from("/test"));

        let action = handle_open_new_session_dialog(&mut state);

        assert!(state.new_session_dialog.is_some());
        assert_eq!(state.ui_mode, UiMode::NewSessionDialog);
        assert!(matches!(action, Some(UpdateAction::DiscoverConnectedDevices)));
    }

    #[test]
    fn test_close_dialog() {
        let mut state = create_test_state_with_dialog();
        state.session_manager.add_mock_session();

        handle_close_new_session_dialog(&mut state);

        assert!(state.new_session_dialog.is_none());
        assert_eq!(state.ui_mode, UiMode::Normal);
    }

    #[test]
    fn test_pane_switching() {
        let mut state = create_test_state_with_dialog();

        handle_new_session_dialog_switch_pane(&mut state);

        let dialog = state.new_session_dialog.as_ref().unwrap();
        assert_eq!(dialog.focused_pane, DialogPane::LaunchContext);
    }

    #[test]
    fn test_tab_switching() {
        let mut state = create_test_state_with_dialog();

        handle_new_session_dialog_switch_tab(&mut state, TargetTab::Bootable);

        let dialog = state.new_session_dialog.as_ref().unwrap();
        assert_eq!(dialog.target_selector.active_tab, TargetTab::Bootable);
    }

    #[test]
    fn test_device_navigation() {
        let mut state = create_test_state_with_dialog();
        state.new_session_dialog.as_mut().unwrap()
            .target_selector.set_connected_devices(vec![
                test_device("1", "Device 1"),
                test_device("2", "Device 2"),
            ]);

        handle_new_session_dialog_device_down(&mut state);

        let dialog = state.new_session_dialog.as_ref().unwrap();
        // Selection should have moved
        assert!(dialog.target_selector.selected_index > 0);
    }

    #[test]
    fn test_field_navigation() {
        let mut state = create_test_state_with_dialog();
        state.new_session_dialog.as_mut().unwrap().focused_pane = DialogPane::LaunchContext;

        handle_new_session_dialog_field_next(&mut state);

        let dialog = state.new_session_dialog.as_ref().unwrap();
        assert_eq!(dialog.launch_context.focused_field, LaunchContextField::Mode);
    }

    #[test]
    fn test_launch_requires_device() {
        let mut state = create_test_state_with_dialog();

        let action = handle_new_session_dialog_launch(&mut state);

        // No device selected, should not launch
        assert!(action.is_none());
    }

    #[test]
    fn test_launch_with_device() {
        let mut state = create_test_state_with_dialog();
        state.new_session_dialog.as_mut().unwrap()
            .target_selector.set_connected_devices(vec![
                test_device("device-1", "iPhone 15"),
            ]);

        let action = handle_new_session_dialog_launch(&mut state);

        assert!(matches!(action, Some(UpdateAction::LaunchFlutterSession(_))));
    }
}
```

### 2. Widget tests

```rust
// src/tui/widgets/new_session_dialog/tests.rs

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};

    #[test]
    fn test_dialog_renders() {
        let state = NewSessionDialogState::new(LoadedConfigs::default());
        let tool_availability = ToolAvailability::default();

        let backend = TestBackend::new(100, 40);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let dialog = NewSessionDialog::new(&state, &tool_availability);
                f.render_widget(dialog, f.area());
            })
            .unwrap();

        let content = buffer_to_string(terminal.backend().buffer());
        assert!(content.contains("New Session"));
        assert!(content.contains("Target Selector"));
        assert!(content.contains("Launch Context"));
    }

    #[test]
    fn test_dialog_with_devices() {
        let mut state = NewSessionDialogState::new(LoadedConfigs::default());
        state.target_selector.set_connected_devices(vec![
            test_device("1", "iPhone 15 Pro"),
            test_device("2", "Pixel 8"),
        ]);

        let tool_availability = ToolAvailability::default();

        let backend = TestBackend::new(100, 40);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let dialog = NewSessionDialog::new(&state, &tool_availability);
                f.render_widget(dialog, f.area());
            })
            .unwrap();

        let content = buffer_to_string(terminal.backend().buffer());
        assert!(content.contains("iPhone 15 Pro"));
        assert!(content.contains("Pixel 8"));
    }

    fn buffer_to_string(buffer: &ratatui::buffer::Buffer) -> String {
        buffer.content().iter().map(|c| c.symbol()).collect()
    }
}
```

### 3. Integration tests

```rust
// tests/integration/dialog_tests.rs

use flutter_demon::app::AppState;
use flutter_demon::tui::widgets::new_session_dialog::*;
use flutter_demon::config::LoadedConfigs;

#[tokio::test]
async fn test_full_dialog_flow() {
    // Create app state
    let mut state = AppState::new(PathBuf::from("/test/project"));

    // Open dialog
    state.new_session_dialog = Some(NewSessionDialogState::new(LoadedConfigs::default()));
    state.ui_mode = UiMode::NewSessionDialog;

    // Simulate device discovery
    let devices = vec![
        Device { id: "device-1".to_string(), name: "Test Device".to_string(), ..Default::default() },
    ];
    state.new_session_dialog.as_mut().unwrap()
        .target_selector.set_connected_devices(devices);

    // Verify device is visible
    assert!(state.new_session_dialog.as_ref().unwrap().is_ready_to_launch());

    // Simulate launch
    let params = state.new_session_dialog.as_ref().unwrap()
        .build_launch_params()
        .unwrap();

    assert_eq!(params.device_id, "device-1");
}

#[tokio::test]
async fn test_config_selection_flow() {
    let mut configs = LoadedConfigs::default();
    configs.configs.push(SourcedConfig {
        config: LaunchConfig {
            name: "Production".to_string(),
            mode: FlutterMode::Release,
            flavor: Some("prod".to_string()),
            ..Default::default()
        },
        source: ConfigSource::FDemon,
        display_name: "Production".to_string(),
    });

    let mut state = NewSessionDialogState::new(configs);

    // Select the config
    state.launch_context.select_config(Some(0));

    // Verify config values applied
    assert_eq!(state.launch_context.mode, FlutterMode::Release);
    assert_eq!(state.launch_context.flavor, Some("prod".to_string()));
}
```

### 4. Remove old tests

```bash
# Find and remove tests that reference old types
rg "StartupDialog" tests/ --files-with-matches
rg "DeviceSelector" tests/ --files-with-matches

# Remove or update each file found
```

### 5. Update snapshot tests (if any)

```rust
// If using insta or similar snapshot testing:
// Update snapshots to reflect new dialog output

#[test]
fn test_dialog_snapshot() {
    let state = NewSessionDialogState::new(LoadedConfigs::default());
    let output = render_to_string(&state);

    insta::assert_snapshot!(output);
}
```

## Verification

```bash
# Run all tests
cargo test

# Run specific test modules
cargo test new_session_dialog
cargo test handler::tests
cargo test integration

# Check for any test failures
cargo test 2>&1 | grep -E "(FAILED|error\[)"

# Update snapshots if using insta
cargo insta review
```

## Common Test Patterns

### Testing state transitions

```rust
#[test]
fn test_state_transition() {
    let mut state = create_initial_state();

    // Apply action
    let action = handle_some_message(&mut state);

    // Assert new state
    assert_eq!(state.expected_field, expected_value);
    assert!(matches!(action, Some(ExpectedAction)));
}
```

### Testing widget rendering

```rust
#[test]
fn test_widget_renders_content() {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal.draw(|f| {
        let widget = MyWidget::new(&state);
        f.render_widget(widget, f.area());
    }).unwrap();

    let content = buffer_to_string(terminal.backend().buffer());
    assert!(content.contains("expected text"));
}
```

## Notes

- Run tests frequently during updates
- Use `cargo test -- --nocapture` for debugging
- Update fixtures to use NewSessionDialogState
- Remove tests for deleted functionality
