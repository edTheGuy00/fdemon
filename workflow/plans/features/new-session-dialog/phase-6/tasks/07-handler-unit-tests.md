# Task: Add Unit Tests for New Handlers

## Summary

Add unit tests for the 394 new lines of handler code in `update.rs` that currently have zero test coverage.

## Files

| File | Action |
|------|--------|
| `src/app/handler/tests.rs` | Modify (add new tests) |

## Background

The code review identified that the new message handlers added in task 05 have no test coverage. This violates the project's code standards requiring tests for new public functions.

## Implementation

### 1. Create test helper for dialog state

```rust
// Add to tests.rs

fn create_test_dialog_state() -> NewSessionDialogState {
    let configs = LaunchConfigs::new(vec![
        LaunchConfig {
            name: "fdemon-config".to_string(),
            display_name: "Development".to_string(),
            source: ConfigSource::FDemon,
            mode: Some(FlutterMode::Debug),
            flavor: None,
            dart_defines: vec![],
        },
        LaunchConfig {
            name: "vscode-config".to_string(),
            display_name: "VSCode Debug".to_string(),
            source: ConfigSource::VSCode,
            mode: Some(FlutterMode::Debug),
            flavor: Some("dev".to_string()),
            dart_defines: vec![],
        },
    ]);

    NewSessionDialogState::new(configs, vec![], vec![])
}

fn create_test_state_with_dialog() -> AppState {
    let mut state = create_test_state();
    state.new_session_dialog_state = Some(create_test_dialog_state());
    state
}
```

### 2. Test field navigation

```rust
#[test]
fn test_field_next_moves_to_next_field() {
    let mut state = create_test_state_with_dialog();
    state.new_session_dialog_state.as_mut().unwrap().active_pane = DialogPane::LaunchContext;

    let action = update(&mut state, Message::NewSessionDialogFieldNext);

    let dialog = state.new_session_dialog_state.as_ref().unwrap();
    assert_eq!(dialog.launch_context_state.focused_field, LaunchContextField::Mode);
    assert!(action.is_none());
}

#[test]
fn test_field_prev_moves_to_previous_field() {
    let mut state = create_test_state_with_dialog();
    {
        let dialog = state.new_session_dialog_state.as_mut().unwrap();
        dialog.active_pane = DialogPane::LaunchContext;
        dialog.launch_context_state.focused_field = LaunchContextField::Mode;
    }

    let action = update(&mut state, Message::NewSessionDialogFieldPrev);

    let dialog = state.new_session_dialog_state.as_ref().unwrap();
    assert_eq!(dialog.launch_context_state.focused_field, LaunchContextField::Config);
}

#[test]
fn test_field_navigation_ignored_when_target_pane_active() {
    let mut state = create_test_state_with_dialog();
    state.new_session_dialog_state.as_mut().unwrap().active_pane = DialogPane::TargetSelector;

    update(&mut state, Message::NewSessionDialogFieldNext);

    let dialog = state.new_session_dialog_state.as_ref().unwrap();
    // Should remain on Config (default) since TargetSelector is active
    assert_eq!(dialog.launch_context_state.focused_field, LaunchContextField::Config);
}
```

### 3. Test mode cycling with editability checks

```rust
#[test]
fn test_mode_next_cycles_mode() {
    let mut state = create_test_state_with_dialog();
    {
        let dialog = state.new_session_dialog_state.as_mut().unwrap();
        dialog.active_pane = DialogPane::LaunchContext;
        dialog.launch_context_state.focused_field = LaunchContextField::Mode;
        dialog.launch_context_state.selected_config_index = Some(0); // FDemon config
    }

    let action = update(&mut state, Message::NewSessionDialogModeNext);

    let dialog = state.new_session_dialog_state.as_ref().unwrap();
    assert_eq!(dialog.launch_context_state.mode, FlutterMode::Profile);
    // Should trigger auto-save for FDemon config
    assert!(matches!(action, Some(UpdateAction::AutoSaveConfig { .. })));
}

#[test]
fn test_mode_change_blocked_for_vscode_config() {
    let mut state = create_test_state_with_dialog();
    {
        let dialog = state.new_session_dialog_state.as_mut().unwrap();
        dialog.active_pane = DialogPane::LaunchContext;
        dialog.launch_context_state.focused_field = LaunchContextField::Mode;
        dialog.launch_context_state.selected_config_index = Some(1); // VSCode config
    }

    let original_mode = state.new_session_dialog_state.as_ref().unwrap()
        .launch_context_state.mode;

    update(&mut state, Message::NewSessionDialogModeNext);

    let dialog = state.new_session_dialog_state.as_ref().unwrap();
    // Mode should not change for VSCode config (read-only)
    assert_eq!(dialog.launch_context_state.mode, original_mode);
}
```

### 4. Test launch requires device selection

```rust
#[test]
fn test_launch_requires_device_selection() {
    let mut state = create_test_state_with_dialog();
    state.new_session_dialog_state.as_mut().unwrap().active_pane = DialogPane::LaunchContext;

    let action = update(&mut state, Message::NewSessionDialogLaunch);

    // Should not trigger launch without device selected
    assert!(action.is_none() || !matches!(action, Some(UpdateAction::LaunchFlutterSession { .. })));
}

#[test]
fn test_launch_with_device_selected() {
    let mut state = create_test_state_with_dialog();
    {
        let dialog = state.new_session_dialog_state.as_mut().unwrap();
        dialog.active_pane = DialogPane::LaunchContext;
        // Add a connected device and select it
        // ... setup code for device selection ...
    }

    let action = update(&mut state, Message::NewSessionDialogLaunch);

    assert!(matches!(action, Some(UpdateAction::LaunchFlutterSession { .. })));
}
```

### 5. Test auto-save triggering

```rust
#[test]
fn test_auto_save_triggers_for_fdemon_config() {
    let mut state = create_test_state_with_dialog();
    {
        let dialog = state.new_session_dialog_state.as_mut().unwrap();
        dialog.active_pane = DialogPane::LaunchContext;
        dialog.launch_context_state.selected_config_index = Some(0); // FDemon config
    }

    // Simulate flavor change
    let action = update(&mut state, Message::NewSessionDialogFlavorSelected {
        flavor: Some("prod".to_string())
    });

    assert!(matches!(action, Some(UpdateAction::AutoSaveConfig { .. })));
}

#[test]
fn test_no_auto_save_for_vscode_config() {
    let mut state = create_test_state_with_dialog();
    {
        let dialog = state.new_session_dialog_state.as_mut().unwrap();
        dialog.active_pane = DialogPane::LaunchContext;
        dialog.launch_context_state.selected_config_index = Some(1); // VSCode config
    }

    // Even if we somehow trigger a flavor change, no auto-save for VSCode
    let action = update(&mut state, Message::NewSessionDialogFlavorSelected {
        flavor: Some("prod".to_string())
    });

    assert!(action.is_none() || !matches!(action, Some(UpdateAction::AutoSaveConfig { .. })));
}
```

## Acceptance Criteria

1. Test helper functions created for dialog state setup
2. Field navigation tests added (next/prev, respects active_pane)
3. Mode cycling tests added (respects VSCode read-only)
4. Launch action tests added (requires device selection)
5. Auto-save triggering tests added (only for FDemon configs)
6. All new tests pass: `cargo test new_session_dialog`

## Verification

```bash
cargo fmt && cargo check && cargo test handler && cargo clippy -- -D warnings
```

## Notes

- Focus on testing the happy paths and edge cases identified in the review
- Use existing test infrastructure patterns from `tests.rs`
- Tests should be fast and not require actual file I/O
