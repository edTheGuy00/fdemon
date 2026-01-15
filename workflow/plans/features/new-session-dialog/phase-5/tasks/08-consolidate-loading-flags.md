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
