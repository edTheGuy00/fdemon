## Task: Keep extra args modal open when confirming with no selection

**Objective**: Change `handle_settings_extra_args_confirm()` to return early (keeping the modal open) when `selected_value()` returns `None`, instead of silently closing the modal.

**Depends on**: None

**Estimated Time**: 0.5-1 hour

**Review Issues**: Major #6

### Scope

- `crates/fdemon-app/src/handler/settings_extra_args.rs`: Modify `handle_settings_extra_args_confirm()`

### Details

Currently, `handle_settings_extra_args_confirm()` (lines 114-136) unconditionally closes the modal after the confirm attempt:

```rust
pub fn handle_settings_extra_args_confirm(state: &mut AppState) -> UpdateResult {
    if let Some(ref modal) = state.settings_view_state.extra_args_modal {
        if let Some(selected) = modal.selected_value() {
            // ... save logic ...
        }
        // No else — falls through
    }
    // Always closes modal:
    state.settings_view_state.extra_args_modal = None;
    state.settings_view_state.editing_config_idx = None;
    UpdateResult::none()
}
```

When `selected_value()` is `None` (empty filter, no typed input), Enter closes the modal silently — the user loses their context without any feedback.

**Fix**: Return early when there is no selection, keeping the modal open:

```rust
pub fn handle_settings_extra_args_confirm(state: &mut AppState) -> UpdateResult {
    let selected = {
        let modal = match state.settings_view_state.extra_args_modal.as_ref() {
            Some(m) => m,
            None => return UpdateResult::none(),
        };
        match modal.selected_value() {
            Some(v) => v,
            None => return UpdateResult::none(), // Keep modal open
        }
    };

    // ... save logic using `selected` ...

    // Only close after successful save
    state.settings_view_state.extra_args_modal = None;
    state.settings_view_state.editing_config_idx = None;
    UpdateResult::none()
}
```

The key behavioral change: the cleanup block (setting modal and idx to `None`) only runs when a selection was successfully processed.

### Acceptance Criteria

1. Pressing Enter with no selection keeps the extra args modal open
2. Pressing Enter with a valid selection saves and closes the modal (unchanged behavior)
3. Pressing Esc still discards and closes the modal (unchanged behavior)

### Testing

```rust
#[test]
fn test_confirm_with_no_selection_keeps_modal_open() {
    // Open extra args modal, set query to something that filters out everything,
    // send Confirm, verify modal is still Some
}

#[test]
fn test_confirm_with_selection_closes_modal() {
    // Open extra args modal, select an item, send Confirm, verify modal is None
}
```

### Notes

- The borrow checker requires extracting `selected_value()` before mutating `state` for the save logic, since `modal` borrows `state` immutably. The code structure above uses a temporary scope to drop the borrow.
- An alternative approach would be to clone the selected value: `let selected = modal.selected_value().cloned()` then match outside the borrow.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/settings_extra_args.rs` | Restructured `handle_settings_extra_args_confirm()` to extract `selected_value()` in a temporary scope, returning early with `UpdateResult::none()` when it is `None` (keeping modal open). The cleanup block (setting modal and idx to `None`) now only runs after a successful selection is processed. |

### Notable Decisions/Tradeoffs

1. **Temporary scope pattern**: Used a temporary scope (`let selected = { ... }`) to extract the owned `String` from `selected_value()` before mutating `state`, satisfying the borrow checker without cloning.
2. **Save error still closes modal**: If `save_launch_configs` fails, the error is captured but the modal still closes. This is pre-existing behavior and not changed by this task.

### Testing Performed

- `cargo check --workspace` - Passed
- `cargo test -p fdemon-app` - Passed (1122 tests)
- `cargo clippy --workspace -- -D warnings` - Passed
- 2 confirm tests: `test_extra_args_confirm_with_no_selection_keeps_modal_open`, `test_confirm_adds_selected_arg_to_config` (pre-existing)
