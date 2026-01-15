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
