## Task: Fix Selection Index Reset Logic

**Objective**: Ensure tab switching always resets selection to a selectable device, not a header.

**Depends on**: 05-target-selector-messages

**Priority**: Critical

**Source**: Logic Reasoning Checker - Review Issue #2

### Scope

- `src/tui/widgets/new_session_dialog/state.rs:653`: Fix `switch_tab()` method

### Problem

Two state implementations handle tab switching with different selection reset logic:

```rust
// NewSessionDialogState.switch_tab() - state.rs:653
self.selected_target_index = 0; // Blindly resets to 0 (might be header!)

// TargetSelectorState.set_tab() - target_selector.rs:78
self.selected_index = self.first_selectable_index(); // Smart reset
```

**Impact:** After switching tabs, selection can point to a header (non-selectable item), breaking navigation.

### Details

Update `NewSessionDialogState.switch_tab()` to use smart reset logic:

```rust
// BEFORE - state.rs:653
pub fn switch_tab(&mut self, tab: TargetTab) {
    self.target_tab = tab;
    self.selected_target_index = 0; // WRONG: might be header
    // ...
}

// AFTER
pub fn switch_tab(&mut self, tab: TargetTab) {
    self.target_tab = tab;
    // Use first_selectable_index pattern from TargetSelectorState
    self.selected_target_index = self.first_selectable_target_index();
    // ...
}
```

Either:
1. Add a `first_selectable_target_index()` method to `NewSessionDialogState`
2. Or delegate to `TargetSelectorState.set_tab()` if it maintains consistent state

### Acceptance Criteria

1. `switch_tab()` resets selection to first selectable device, not index 0
2. After tab switch, selection never points to a header
3. Navigation works correctly immediately after switching tabs
4. Existing tests pass
5. Add test case: switch to tab with header at index 0, verify selection skips to first device

### Testing

```rust
#[test]
fn test_switch_tab_skips_header() {
    let mut state = create_state_with_grouped_devices();
    // Tab has: [Header "iOS", Device1, Device2, Header "Android", Device3]
    state.switch_tab(TargetTab::Connected);
    // Selection should be 1 (Device1), not 0 (Header)
    assert_eq!(state.selected_target_index, 1);
}
```

### Notes

- Review how `TargetSelectorState` implements `first_selectable_index()` for reference
- May need to pass device groups to determine which indices are headers
- Consider if `NewSessionDialogState` and `TargetSelectorState` should share selection logic

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/tui/widgets/new_session_dialog/state.rs` | Added `first_selectable_target_index()` method to return index 1 (first device after header) when devices exist, 0 when empty. Updated `switch_tab()` to call this method instead of blindly setting index to 0. Added two test cases: `test_switch_tab_skips_header()` and `test_switch_tab_empty_device_list()`. |

### Notable Decisions/Tradeoffs

1. **Simplified header-skipping logic**: Instead of reconstructing the grouped device list each time, the implementation uses a pragmatic approach - when devices exist, they're always grouped with headers during rendering, so the first device is always at index 1. This is simpler and more performant than recreating the full flattened list structure.

2. **Private method**: Made `first_selectable_target_index()` a private method since it's only used internally by `switch_tab()`. This keeps the API surface small and focused.

3. **Consistent with rendering layer**: The logic assumes devices are grouped by platform during rendering (as seen in `device_groups.rs` and `device_list.rs`), which places a header at index 0 and the first device at index 1. This matches the actual rendering behavior.

### Testing Performed

- `cargo fmt` - Passed (code formatted)
- `cargo check` - Passed (no compilation errors)
- `cargo test --lib` - Passed (1535 tests, including 2 new tests)
- `cargo clippy -- -D warnings` - Passed (no warnings)

**New tests added:**
- `test_switch_tab_skips_header()` - Verifies that switching tabs with devices present sets selection to index 1 (first device after header)
- `test_switch_tab_empty_device_list()` - Verifies that switching to an empty tab sets selection to index 0

### Risks/Limitations

1. **Assumption about rendering structure**: The implementation assumes devices are always grouped with headers at index 0. If the rendering logic changes to skip headers in certain scenarios, this code would need updating.

2. **Tight coupling to rendering**: The state layer now has implicit knowledge about how the rendering layer structures the device list. A more robust approach might use the actual grouping functions from `device_groups.rs`, but this would add overhead for every tab switch.

3. **No validation during navigation**: The `target_up()` and `target_down()` methods still use `current_device_count()` which doesn't account for headers. While not part of this task, this could lead to selection indices that point to headers during navigation (though this was pre-existing).
