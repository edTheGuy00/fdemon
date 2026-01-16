# Task 08: Update Tests

## Objective

Update and add tests to cover all changes from Tasks 01-07, ensuring comprehensive test coverage.

## Priority

**Low** - Final cleanup task after all implementation is complete

## Depends On

- All previous tasks (01-07)

## Problem

After implementing the four fixes:
1. Boot platform type changed from String to enum
2. Scroll state added to TargetSelectorState
3. Device cache usage logic added
4. Responsive layout modes added

Existing tests may fail or need updating, and new functionality needs test coverage.

## Solution

### Step 1: Update Boot Platform Tests

**File:** `src/app/handler/new_session/target_selector.rs` (tests section)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Platform;

    #[test]
    fn test_boot_ios_simulator_uses_platform_enum() {
        let mut state = test_app_state_with_bootable_devices();
        state.new_session_dialog_state.target_selector.set_tab(TargetTab::Bootable);
        state.new_session_dialog_state.target_selector.selected_index = 0; // iOS simulator

        let result = handle_boot_device(&mut state);

        if let Some(UpdateAction::BootDevice { device_id: _, platform }) = result.action {
            assert_eq!(platform, Platform::IOS);
        } else {
            panic!("Expected BootDevice action");
        }
    }

    #[test]
    fn test_boot_android_avd_uses_platform_enum() {
        let mut state = test_app_state_with_bootable_devices();
        state.new_session_dialog_state.target_selector.set_tab(TargetTab::Bootable);
        // Select Android AVD (after iOS simulators)
        state.new_session_dialog_state.target_selector.selected_index = 3;

        let result = handle_boot_device(&mut state);

        if let Some(UpdateAction::BootDevice { device_id: _, platform }) = result.action {
            assert_eq!(platform, Platform::Android);
        } else {
            panic!("Expected BootDevice action");
        }
    }
}
```

### Step 2: Add Scroll State Tests

**File:** `src/tui/widgets/new_session_dialog/target_selector.rs` (tests section)

```rust
#[test]
fn test_scroll_offset_initializes_to_zero() {
    let state = TargetSelectorState::default();
    assert_eq!(state.scroll_offset, 0);
}

#[test]
fn test_adjust_scroll_when_selection_below_visible() {
    let mut state = TargetSelectorState::default();
    state.connected_devices = (0..20).map(|i| test_device(i)).collect();
    state.selected_index = 15;
    state.scroll_offset = 0;

    state.adjust_scroll(10);

    // Selection at 15 with 10 visible items needs offset >= 6
    assert!(state.scroll_offset >= 6);
}

#[test]
fn test_adjust_scroll_when_selection_above_visible() {
    let mut state = TargetSelectorState::default();
    state.connected_devices = (0..20).map(|i| test_device(i)).collect();
    state.selected_index = 3;
    state.scroll_offset = 10;

    state.adjust_scroll(10);

    // Selection at 3 with offset 10 needs to scroll up
    assert!(state.scroll_offset <= 3);
}

#[test]
fn test_scroll_resets_on_tab_change() {
    let mut state = TargetSelectorState::default();
    state.scroll_offset = 5;
    state.set_tab(TargetTab::Bootable);
    assert_eq!(state.scroll_offset, 0);
}

#[test]
fn test_scroll_resets_on_device_update() {
    let mut state = TargetSelectorState::default();
    state.scroll_offset = 5;
    state.set_connected_devices(vec![test_device(1)]);
    assert_eq!(state.scroll_offset, 0);
}
```

### Step 3: Add Device Cache Tests

**File:** `src/app/handler/new_session/navigation.rs` (tests section)

```rust
#[test]
fn test_open_dialog_uses_cached_devices() {
    let mut state = test_app_state();

    // Pre-populate cache
    let devices = vec![test_device(1), test_device(2)];
    state.set_device_cache(devices.clone());

    let result = handle_open_new_session_dialog(&mut state);

    // Should have devices immediately
    assert_eq!(
        state.new_session_dialog_state.target_selector.connected_devices.len(),
        2
    );

    // Should NOT show loading
    assert!(!state.new_session_dialog_state.target_selector.loading);

    // Should trigger background refresh
    assert!(matches!(result.action, Some(UpdateAction::RefreshDevicesBackground)));
}

#[test]
fn test_open_dialog_cache_miss_shows_loading() {
    let mut state = test_app_state();
    // No cache set

    let result = handle_open_new_session_dialog(&mut state);

    // Should show loading
    assert!(state.new_session_dialog_state.target_selector.loading);

    // Should trigger foreground discovery
    assert!(matches!(result.action, Some(UpdateAction::DiscoverDevices)));
}

#[test]
fn test_open_dialog_expired_cache_shows_loading() {
    let mut state = test_app_state();

    // Set cache with old timestamp
    state.device_cache = Some(vec![test_device(1)]);
    state.devices_last_updated = Some(std::time::Instant::now() - std::time::Duration::from_secs(60));

    let result = handle_open_new_session_dialog(&mut state);

    // Cache expired - should show loading
    assert!(state.new_session_dialog_state.target_selector.loading);
    assert!(matches!(result.action, Some(UpdateAction::DiscoverDevices)));
}
```

### Step 4: Add Layout Mode Tests

**File:** `src/tui/widgets/new_session_dialog/mod.rs` (tests section)

```rust
#[test]
fn test_layout_mode_horizontal_large_terminal() {
    let area = Rect::new(0, 0, 120, 50);
    assert_eq!(NewSessionDialog::layout_mode(area), LayoutMode::Horizontal);
}

#[test]
fn test_layout_mode_horizontal_boundary() {
    let area = Rect::new(0, 0, 70, 20);
    assert_eq!(NewSessionDialog::layout_mode(area), LayoutMode::Horizontal);
}

#[test]
fn test_layout_mode_vertical_narrow() {
    let area = Rect::new(0, 0, 50, 30);
    assert_eq!(NewSessionDialog::layout_mode(area), LayoutMode::Vertical);
}

#[test]
fn test_layout_mode_vertical_boundary() {
    let area = Rect::new(0, 0, 69, 25);
    assert_eq!(NewSessionDialog::layout_mode(area), LayoutMode::Vertical);
}

#[test]
fn test_layout_mode_too_small_width() {
    let area = Rect::new(0, 0, 35, 25);
    assert_eq!(NewSessionDialog::layout_mode(area), LayoutMode::TooSmall);
}

#[test]
fn test_layout_mode_too_small_height() {
    let area = Rect::new(0, 0, 50, 15);
    assert_eq!(NewSessionDialog::layout_mode(area), LayoutMode::TooSmall);
}

#[test]
fn test_fits_in_area_horizontal() {
    let area = Rect::new(0, 0, 100, 40);
    assert!(NewSessionDialog::fits_in_area(area));
}

#[test]
fn test_fits_in_area_vertical() {
    let area = Rect::new(0, 0, 50, 25);
    assert!(NewSessionDialog::fits_in_area(area));
}

#[test]
fn test_fits_in_area_too_small() {
    let area = Rect::new(0, 0, 30, 15);
    assert!(!NewSessionDialog::fits_in_area(area));
}
```

### Step 5: Add Truncation Utility Tests

```rust
#[test]
fn test_truncate_with_ellipsis_short_text() {
    assert_eq!(truncate_with_ellipsis("hello", 10), "hello");
}

#[test]
fn test_truncate_with_ellipsis_exact_fit() {
    assert_eq!(truncate_with_ellipsis("hello", 5), "hello");
}

#[test]
fn test_truncate_with_ellipsis_needs_truncation() {
    assert_eq!(truncate_with_ellipsis("hello world", 8), "hello...");
}

#[test]
fn test_truncate_with_ellipsis_very_short() {
    assert_eq!(truncate_with_ellipsis("hello", 3), "...");
}

#[test]
fn test_truncate_middle() {
    assert_eq!(truncate_middle("hello_world_test", 12), "hell...test");
}
```

### Step 6: Update Snapshot Tests (if applicable)

**File:** `src/tui/render/tests.rs`

If there are snapshot tests for the NewSessionDialog:

```rust
#[test]
fn test_new_session_dialog_horizontal_snapshot() {
    let mut state = test_app_state();
    state.ui_mode = UiMode::NewSessionDialog;
    // ... setup ...

    let mut terminal = TestTerminal::new(100, 40);
    render_frame(&mut terminal, &state);

    insta::assert_snapshot!(terminal.to_string());
}

#[test]
fn test_new_session_dialog_vertical_snapshot() {
    let mut state = test_app_state();
    state.ui_mode = UiMode::NewSessionDialog;
    // ... setup ...

    let mut terminal = TestTerminal::new(50, 30);
    render_frame(&mut terminal, &state);

    insta::assert_snapshot!(terminal.to_string());
}
```

### Step 7: Run Full Test Suite and Fix Failures

```bash
# Run all tests
cargo test

# Check for any failures related to our changes
cargo test boot
cargo test scroll
cargo test cache
cargo test layout
cargo test new_session

# Run with verbose output for debugging
cargo test -- --nocapture
```

## Files to Modify

| File | Changes |
|------|---------|
| `src/app/handler/new_session/target_selector.rs` | Add/update boot platform tests |
| `src/tui/widgets/new_session_dialog/target_selector.rs` | Add scroll state tests |
| `src/app/handler/new_session/navigation.rs` | Add cache usage tests |
| `src/tui/widgets/new_session_dialog/mod.rs` | Add layout mode tests, truncation tests |
| `src/tui/render/tests.rs` | Update/add snapshot tests |

## Acceptance Criteria

1. All existing tests pass
2. New tests cover:
   - Boot platform enum usage
   - Scroll state initialization and adjustment
   - Device cache usage on dialog open
   - Layout mode detection
   - Text truncation utilities
3. `cargo test` passes with no failures
4. `cargo clippy` passes
5. Test coverage meaningful (not just placeholders)

## Testing

```bash
cargo test
cargo test --lib new_session
cargo clippy -- -D warnings
```

## Notes

- Run tests frequently during implementation to catch regressions early
- Consider adding integration tests for the full dialog flow
- Snapshot tests may need updating if visual output changes

---

## Completion Summary

**Status:** Not Started
