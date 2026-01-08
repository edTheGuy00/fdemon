## Task: Add UI Mode Transition Tests

**Objective**: Test that UI mode changes result in correct screen renders - verifying transitions between modes work correctly.

**Depends on**: 07-header-widget-tests, 08-statusbar-widget-tests, 09-device-selector-tests, 10-confirm-dialog-tests

### Scope

- `src/tui/render/tests.rs`: Add transition tests
- OR `src/app/handler/tests.rs`: Verify handler produces correct mode changes

### Details

#### 1. UI Mode Transitions to Test

```
Normal ──────────────────► DeviceSelector (press 'd')
Normal ──────────────────► ConfirmDialog (press 'q')
Normal ──────────────────► SearchInput (press '/')
DeviceSelector ──────────► Normal (press Escape)
DeviceSelector ──────────► Normal (select device)
ConfirmDialog ─────────► Normal (press 'n' or Escape)
ConfirmDialog ─────────► Quit (press 'y')
```

#### 2. Add Transition Tests

Add to `src/tui/render/tests.rs`:

```rust
// ===========================================================================
// UI Mode Transition Tests
// ===========================================================================

#[test]
fn test_transition_normal_to_device_selector() {
    let mut state = create_base_state();
    state.ui_mode = UiMode::Normal;
    state.phase = AppPhase::Running;

    // Render normal mode
    let before = render_screen(&mut state);
    assert!(!before.contains("Select") || !before.contains("Device"));

    // Transition to device selector
    state.ui_mode = UiMode::DeviceSelector;
    let after = render_screen(&mut state);

    // Device selector should now be visible
    assert!(
        after.contains("Select") || after.contains("Device") || after.contains("device"),
        "Device selector should appear after transition"
    );
}

#[test]
fn test_transition_normal_to_confirm_dialog() {
    let mut state = create_base_state();
    state.ui_mode = UiMode::Normal;

    let before = render_screen(&mut state);
    assert!(!before.contains("Quit?"));

    // Transition to confirm dialog
    state.ui_mode = UiMode::ConfirmDialog;
    state.confirm_dialog_state = Some(ConfirmDialogState::quit());
    let after = render_screen(&mut state);

    assert!(
        after.contains("Quit") || after.contains("quit"),
        "Confirm dialog should appear after transition"
    );
}

#[test]
fn test_transition_device_selector_to_normal() {
    let mut state = create_base_state();
    state.ui_mode = UiMode::DeviceSelector;

    let before = render_screen(&mut state);

    // Transition back to normal (e.g., Escape pressed)
    state.ui_mode = UiMode::Normal;
    let after = render_screen(&mut state);

    // Device selector should be gone
    // Just verify it renders differently
    assert_ne!(before, after, "Screen should change on mode transition");
}

#[test]
fn test_transition_confirm_to_normal_cancel() {
    let mut state = create_base_state();
    state.ui_mode = UiMode::ConfirmDialog;
    state.confirm_dialog_state = Some(ConfirmDialogState::quit());

    let before = render_screen(&mut state);
    assert!(before.contains("Quit") || before.contains("quit"));

    // Cancel - return to normal
    state.ui_mode = UiMode::Normal;
    state.confirm_dialog_state = None;
    let after = render_screen(&mut state);

    assert!(
        !after.contains("Quit?"),
        "Dialog should disappear after cancel"
    );
}

#[test]
fn test_phase_transition_renders_correctly() {
    let mut state = create_base_state();
    state.ui_mode = UiMode::Normal;

    // Initializing
    state.phase = AppPhase::Initializing;
    let init = render_screen(&mut state);

    // Running
    state.phase = AppPhase::Running;
    let running = render_screen(&mut state);

    // Reloading
    state.phase = AppPhase::Reloading;
    let reloading = render_screen(&mut state);

    // All should be different
    assert_ne!(init, running, "Initializing and Running should look different");
    assert_ne!(running, reloading, "Running and Reloading should look different");
}

#[test]
fn test_modal_overlay_preserves_background() {
    let mut state = create_base_state();
    state.ui_mode = UiMode::Normal;
    state.phase = AppPhase::Running;
    state.project_name = Some("my_app".to_string());

    // Get normal background
    let normal = render_screen(&mut state);

    // Show device selector overlay
    state.ui_mode = UiMode::DeviceSelector;
    let with_modal = render_screen(&mut state);

    // Modal should be visible, but check that something from normal mode
    // might still be partially visible (depends on implementation)
    assert!(with_modal.len() > 0, "Modal overlay should render");
}

#[test]
fn test_loading_to_normal_transition() {
    let mut state = create_base_state();

    // Loading state
    state.ui_mode = UiMode::Loading;
    state.loading_state = Some(LoadingState::new("Starting..."));
    let loading = render_screen(&mut state);

    // Transition to normal running
    state.ui_mode = UiMode::Normal;
    state.phase = AppPhase::Running;
    state.loading_state = None;
    let normal = render_screen(&mut state);

    assert_ne!(loading, normal, "Loading and normal modes should differ");
}

#[test]
fn test_rapid_mode_changes() {
    let mut state = create_base_state();

    // Simulate rapid mode changes
    let modes = [
        UiMode::Normal,
        UiMode::DeviceSelector,
        UiMode::Normal,
        UiMode::ConfirmDialog,
        UiMode::Normal,
    ];

    for mode in modes {
        state.ui_mode = mode;
        if mode == UiMode::ConfirmDialog {
            state.confirm_dialog_state = Some(ConfirmDialogState::quit());
        } else {
            state.confirm_dialog_state = None;
        }

        // Should render without panic
        let content = render_screen(&mut state);
        assert!(!content.is_empty(), "Should render in mode {:?}", mode);
    }
}
```

### Test Coverage

| Test Case | Transition |
|-----------|------------|
| `test_transition_normal_to_device_selector` | Normal → DeviceSelector |
| `test_transition_normal_to_confirm_dialog` | Normal → ConfirmDialog |
| `test_transition_device_selector_to_normal` | DeviceSelector → Normal |
| `test_transition_confirm_to_normal_cancel` | ConfirmDialog → Normal |
| `test_phase_transition_renders_correctly` | Phase changes in Normal mode |
| `test_modal_overlay_preserves_background` | Modal rendering |
| `test_loading_to_normal_transition` | Loading → Normal |
| `test_rapid_mode_changes` | Stress test mode switching |

### Acceptance Criteria

1. All mode transitions render correctly
2. Modals appear/disappear on mode change
3. Phase changes update status bar
4. No panics on rapid mode switching
5. Background content handled correctly with overlays

### Testing

```bash
# Run transition tests
cargo test tui::render::tests::transition --lib -- --nocapture

# Run all render tests
cargo test tui::render::tests --lib
```

### Notes

- These tests verify rendering, not key handling
- Key handling tests belong in handler tests
- Focus on visual correctness after state change

---

## Completion Summary

**Status:** Not Started
