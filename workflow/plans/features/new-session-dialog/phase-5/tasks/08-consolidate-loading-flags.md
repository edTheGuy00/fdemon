## Task: Consolidate Loading Flag Management

**Objective**: Eliminate race condition by having a single source of truth for loading flag management.

**Depends on**: 05-target-selector-messages

**Priority**: Major

**Source**: Logic Reasoning Checker - Review Issue #3

### Scope

- `src/app/handler/update.rs:1738-1751`: Handler loading flag logic
- `src/tui/widgets/new_session_dialog/state.rs:656`: State method loading flag logic

### Problem

Both handler and state method set `loading_bootable` flag, creating potential race conditions:

```rust
// Handler - update.rs:1738-1751
if !state.new_session_dialog_state.loading_bootable {
    state.new_session_dialog_state.loading_bootable = true;
    // trigger discovery
}

// State method - state.rs:656
pub fn switch_tab(&mut self, tab: TargetTab) {
    // ...
    self.loading_bootable = true; // Also sets flag!
}
```

**Impact:** Rapid tab switching can create scenarios where:
- Flag is `true` but no discovery action was dispatched
- Flag state becomes inconsistent with actual discovery status

### Details

Choose **Option A: Handler manages flags** (preferred for TEA purity):

```rust
// state.rs - REMOVE loading flag manipulation from switch_tab()
pub fn switch_tab(&mut self, tab: TargetTab) {
    self.target_tab = tab;
    self.selected_target_index = self.first_selectable_target_index();
    // DO NOT set loading_bootable here - handler is responsible
}

// update.rs - Handler remains sole owner of loading flags
Message::NewSessionDialogSwitchTab(tab) => {
    state.new_session_dialog_state.switch_tab(tab);

    if tab == TargetTab::Bootable && !state.new_session_dialog_state.loading_bootable {
        state.new_session_dialog_state.loading_bootable = true;
        return UpdateAction::DiscoverBootableDevices;
    }
    None
}
```

### Acceptance Criteria

1. Only ONE location sets/clears loading flags (handler preferred)
2. State methods do not mutate loading flags
3. Rapid tab switching does not cause inconsistent flag state
4. Discovery is triggered correctly when switching to Bootable tab
5. All existing tests pass

### Testing

```rust
#[test]
fn test_rapid_tab_switching_no_race() {
    let mut state = create_state();

    // Rapid switch: Connected -> Bootable -> Connected -> Bootable
    state.switch_tab(TargetTab::Connected);
    assert!(!state.loading_bootable);

    state.switch_tab(TargetTab::Bootable);
    // Handler should set flag, not switch_tab
    assert!(!state.loading_bootable); // State method doesn't set it

    // Simulate handler setting flag
    state.loading_bootable = true;

    state.switch_tab(TargetTab::Connected);
    // Switching away shouldn't clear the flag (discovery still running)
}
```

### Notes

- TEA pattern: State transitions happen in handler, not in state methods
- State methods should be pure transformations without side effects on flags
- Consider adding comments documenting which component owns flag management

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/tui/widgets/new_session_dialog/state.rs` | Removed `loading_bootable = true` from `switch_tab()` method (line 658), added comment documenting handler responsibility, updated `test_tab_switching` test to assert flag is NOT set by state method, added `test_rapid_tab_switching_no_race` test |
| `src/app/handler/update.rs` | Added `state.new_session_dialog_state.loading_bootable = true` before returning `UpdateAction::DiscoverBootableDevices` in `NewSessionDialogSwitchTab` handler (line 1748) |

### Notable Decisions/Tradeoffs

1. **Handler as Single Source of Truth**: Implemented Option A from the task specification. The handler now exclusively manages loading flags, ensuring TEA pattern purity. State methods remain pure transformations without side effects.
2. **Flag Setting Location**: The handler sets the loading flag AFTER calling `switch_tab()` but BEFORE dispatching the discovery action. This ensures the flag accurately reflects that discovery has been initiated.
3. **Comment Added**: Added inline comment in `switch_tab()` to document that handler is responsible for loading flags, preventing future regressions.

### Testing Performed

- `cargo fmt` - Passed
- `cargo check` - Passed
- `cargo test --lib` - Passed (1536 tests)
- `cargo clippy -- -D warnings` - Passed

### Test Changes

1. **Updated existing test**: Modified `test_tab_switching` to assert that `loading_bootable` is FALSE after tab switch, confirming state method doesn't set the flag
2. **Added new test**: Implemented `test_rapid_tab_switching_no_race` as specified in task requirements, verifying that rapid tab switching doesn't cause inconsistent flag state

### Risks/Limitations

None identified. The change eliminates the race condition by ensuring loading flags are only set/cleared by the handler when actual discovery actions are dispatched.
