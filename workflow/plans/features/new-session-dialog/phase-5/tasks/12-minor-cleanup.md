## Task: Minor Cleanup and Documentation

**Objective**: Address minor issues identified in review: unused field, missing docs, and navigation edge case.

**Depends on**: 05-target-selector-messages

**Priority**: Minor

**Source**: Review Issues #7, #8, #9

### Scope

- `src/tui/widgets/new_session_dialog/target_selector.rs`: Remove or document `scroll_offset`
- `src/tui/widgets/new_session_dialog/target_selector.rs`: Add docs to `TargetSelectorState::new()`
- `src/tui/widgets/new_session_dialog/device_list.rs`: Add docs to `calculate_scroll_offset()`, `DeviceListStyles`
- `src/tui/widgets/new_session_dialog/device_groups.rs:215-241`: Add defensive navigation check

### Issue #7: Unused scroll_offset Field

**Problem:** `TargetSelectorState.scroll_offset` exists but is never used.

**Options:**
- Remove the field entirely (preferred if no planned use)
- Add `#[allow(dead_code)]` with TODO comment if needed for future work

```rust
// OPTION A: Remove
pub struct TargetSelectorState {
    // Remove: scroll_offset: usize,
}

// OPTION B: Document for future
pub struct TargetSelectorState {
    /// Scroll offset for viewport. TODO: Implement in Phase 6 for long device lists.
    #[allow(dead_code)]
    scroll_offset: usize,
}
```

### Issue #8: Missing Documentation

Add doc comments to public items:

```rust
// target_selector.rs
impl TargetSelectorState {
    /// Creates a new TargetSelectorState with default settings.
    ///
    /// Starts on the Connected tab with no devices loaded.
    /// Selection is initially at index 0 (will be adjusted when devices load).
    pub fn new() -> Self {
        // ...
    }
}

// device_list.rs
/// Calculates the scroll offset needed to keep the selected item visible.
///
/// # Arguments
/// * `selected_index` - The currently selected item index
/// * `viewport_height` - Number of visible rows
/// * `current_offset` - Current scroll position
///
/// # Returns
/// New scroll offset that ensures selected item is visible
pub fn calculate_scroll_offset(
    selected_index: usize,
    viewport_height: usize,
    current_offset: usize,
) -> usize {
    // ...
}

/// Styling configuration for device list rendering.
///
/// Defines colors and styles for headers, devices, selection indicators,
/// and various device states (connected, disconnected, booting).
pub struct DeviceListStyles {
    // ...
}
```

### Issue #9: Defensive Navigation Check

**Problem:** If `selected_index` points to a header (invalid state), `next_selectable()` behavior is unpredictable.

**Fix:** Add validation that finds nearest selectable index:

```rust
// device_groups.rs
pub fn next_selectable(&self, current: usize, direction: Direction) -> usize {
    // Defensive check: if current is a header, find nearest selectable first
    let start = if self.is_header(current) {
        self.nearest_selectable(current)
    } else {
        current
    };

    // Existing navigation logic from valid start position
    // ...
}

fn nearest_selectable(&self, index: usize) -> usize {
    // Try forward first, then backward
    for i in index..self.len() {
        if !self.is_header(i) {
            return i;
        }
    }
    for i in (0..index).rev() {
        if !self.is_header(i) {
            return i;
        }
    }
    0 // Fallback (shouldn't happen with non-empty list)
}
```

### Acceptance Criteria

1. No dead code warnings for `scroll_offset` (removed or documented)
2. `cargo doc` builds with no "missing documentation" warnings for public items
3. Navigation is predictable even if selection state becomes corrupted
4. All existing tests pass
5. `cargo clippy -- -D warnings` passes

### Testing

```rust
#[test]
fn test_navigation_from_header_position() {
    let groups = create_groups_with_headers();
    // Simulate corrupted state: selection on header
    let result = groups.next_selectable(0, Direction::Down); // 0 is header
    // Should return first device, not stay on header
    assert!(!groups.is_header(result));
}
```

### Notes

- These are minor polish items, can be done quickly
- Prefer removing dead code over documenting for "future use"
- Doc comments should be concise but informative

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/tui/widgets/new_session_dialog/target_selector.rs` | Removed unused `scroll_offset` field and all references to it. Added comprehensive doc comment to `TargetSelectorState::new()` method. |
| `src/tui/widgets/new_session_dialog/device_list.rs` | Added doc comment to `DeviceListStyles` struct explaining its purpose and usage. `calculate_scroll_offset()` already had documentation. |
| `src/tui/widgets/new_session_dialog/device_groups.rs` | Added defensive navigation checks to `next_selectable()` and `prev_selectable()`. Implemented `is_header()` and `nearest_selectable()` helper functions. Added 11 new tests for header navigation edge cases. |

### Notable Decisions/Tradeoffs

1. **Removed scroll_offset entirely**: Chose to remove the unused field rather than document it for future use, following code standards preference for removing dead code over documenting speculative features.

2. **Defensive navigation implementation**: When navigation functions receive a header index, they now find the nearest selectable device first, then perform the navigation. This prevents navigation from getting stuck or producing unpredictable results if the state becomes corrupted.

3. **Test coverage**: Added comprehensive tests for navigation from header positions, testing both forward and backward navigation, as well as edge cases like empty lists and lists with only headers.

### Testing Performed

- `cargo fmt` - Passed
- `cargo check` - Passed
- `cargo test --lib` - Passed (1552 tests)
- `cargo clippy -- -D warnings` - Passed (no warnings)

All new tests pass:
- `test_navigation_from_header_position_next` - Verifies next navigation from header
- `test_navigation_from_header_position_prev` - Verifies previous navigation from header
- `test_nearest_selectable_forward` - Tests finding nearest device forward
- `test_nearest_selectable_backward` - Tests finding nearest device backward
- `test_nearest_selectable_already_selectable` - Tests when already on device
- `test_nearest_selectable_empty` - Tests empty list edge case
- `test_nearest_selectable_no_devices` - Tests list with only headers
- `test_is_header_true/false/out_of_bounds` - Tests header detection

### Risks/Limitations

1. **Navigation behavior change**: The navigation functions now perform an extra step when starting from a header, finding the nearest device before navigating. This could theoretically change behavior if any code was relying on the previous behavior, but since starting from a header was an invalid/corrupted state, this is an improvement.

2. **No actual scrolling implementation**: While `calculate_scroll_offset()` exists and is documented, scrolling is not yet implemented in the target selector widget. This will need to be addressed in a future phase when device lists grow long enough to require scrolling.
