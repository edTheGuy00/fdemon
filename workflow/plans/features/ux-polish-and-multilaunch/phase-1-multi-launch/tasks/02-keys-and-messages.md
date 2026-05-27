## Task: Key bindings & messages for multi-select

**Objective**: Wire `Space` (toggle) and `a` (select-all/clear) in the TargetSelector pane to new messages and handlers that drive the multi-select state.

**Depends on**: 01-multi-select-state

**Estimated Time**: 2–3h

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/message.rs`: add `NewSessionDialogToggleDeviceSelection` and `NewSessionDialogSelectAllDevices` to the NewSessionDialog message group.
- `crates/fdemon-app/src/handler/keys.rs`: in `handle_target_selector_key`, map `Space` and `a` to the new messages.
- `crates/fdemon-app/src/handler/new_session/target_selector.rs`: add `handle_toggle_device_selection` and `handle_select_all_devices` handlers.
- `crates/fdemon-app/src/handler/update.rs`: dispatch the two new messages to the handlers.
- (Trailing, unmanaged doc) `docs/KEYBINDINGS.md`: document `Space` / `a` in the New Session Dialog section.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/new_session_dialog/target_selector_state.rs`: the methods from task 01.

### Details

**Messages** (`message.rs`, near the other `NewSessionDialog*` variants ~line 487–510):

```rust
/// Toggle multi-launch selection of the cursor device (Connected tab).
NewSessionDialogToggleDeviceSelection,
/// Select all / clear all connected devices for multi-launch.
NewSessionDialogSelectAllDevices,
```

**Key routing** (`keys.rs`, `handle_target_selector_key`, ~line 1363):

```rust
fn handle_target_selector_key(key: InputKey) -> Option<Message> {
    match key {
        InputKey::Up => Some(Message::NewSessionDialogDeviceUp),
        InputKey::Down => Some(Message::NewSessionDialogDeviceDown),
        InputKey::Enter => Some(Message::NewSessionDialogDeviceSelect),
        InputKey::Char(' ') => Some(Message::NewSessionDialogToggleDeviceSelection),
        InputKey::Char('a') => Some(Message::NewSessionDialogSelectAllDevices),
        InputKey::Char('r') => Some(Message::NewSessionDialogRefreshDevices),
        _ => None,
    }
}
```

> Note: this pane is a list (no text input), so `Char(' ')` / `Char('a')` are safe here. The text-input suppression logic at the top of `dispatch_key` only applies to the `LaunchContext` pane.

**Handlers** (`handler/new_session/target_selector.rs`):

```rust
pub fn handle_toggle_device_selection(state: &mut AppState) -> UpdateResult {
    state.new_session_dialog_state.target_selector.toggle_checked_cursor();
    UpdateResult::none()
}

pub fn handle_select_all_devices(state: &mut AppState) -> UpdateResult {
    state.new_session_dialog_state.target_selector.toggle_select_all();
    UpdateResult::none()
}
```

**Dispatch** (`handler/update.rs`, alongside the other `NewSessionDialog*` arms):

```rust
Message::NewSessionDialogToggleDeviceSelection =>
    new_session::target_selector::handle_toggle_device_selection(state),
Message::NewSessionDialogSelectAllDevices =>
    new_session::target_selector::handle_select_all_devices(state),
```

### Acceptance Criteria

1. In the TargetSelector pane, `Space` emits `NewSessionDialogToggleDeviceSelection` and `a` emits `NewSessionDialogSelectAllDevices`.
2. The handlers mutate `checked_device_ids` via the task-01 methods and return `UpdateResult::none()`.
3. On the Bootable tab the messages still dispatch but are no-ops (task-01 methods guard the tab).
4. Existing `Up`/`Down`/`Enter`/`r` behavior is unchanged.
5. `KEYBINDINGS.md` lists the new dialog shortcuts.

### Testing

```rust
#[test]
fn space_key_maps_to_toggle_selection() {
    // handle_target_selector_key(InputKey::Char(' ')) == Some(NewSessionDialogToggleDeviceSelection)
}

#[test]
fn a_key_maps_to_select_all() { /* ... */ }

#[test]
fn handle_toggle_device_selection_checks_cursor_device() {
    // seed connected devices, cursor on a device, call handler -> checked_count() == 1
}

#[test]
fn handle_select_all_devices_checks_all() { /* ... */ }
```

(Follow the patterns in the existing `handler/new_session/target_selector.rs` tests, e.g. `test_app_state()`.)

### Notes

- Do not intercept `Space`/`a` outside the TargetSelector pane — leave `LaunchContext` text/field handling untouched.
- Keep handlers thin; all selection logic lives in task 01.
- Optional: a mouse click on a row could also toggle in multi mode (see `handler/new_session/clicks.rs`), but defer unless trivial — keyboard is the acceptance path.

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-adc366c884196df4f

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/message.rs` | Added `NewSessionDialogToggleDeviceSelection` and `NewSessionDialogSelectAllDevices` variants |
| `crates/fdemon-app/src/handler/keys.rs` | Added `Space` and `a` mappings in `handle_target_selector_key`; added `target_selector_multiselect_key_tests` test module |
| `crates/fdemon-app/src/handler/new_session/target_selector.rs` | Added `handle_toggle_device_selection` and `handle_select_all_devices` handlers with full test coverage |
| `crates/fdemon-app/src/handler/update.rs` | Dispatched both new messages to the new handlers |
| `docs/KEYBINDINGS.md` | Documented `Space` and `a` in the Target Selector (Left Pane) section |

### Notable Decisions/Tradeoffs

1. **Re-export path**: Since `target_selector` is a private module with `pub use target_selector::*` in `new_session/mod.rs`, the dispatch in `update.rs` uses `new_session::handle_toggle_device_selection` (flat re-export) rather than `new_session::target_selector::handle_toggle_device_selection`.

2. **Test device setup**: The handler tests use all-android devices to avoid the multi-platform grouping complication (where the flat list interleaves iOS and Android headers). This keeps the flat-list index arithmetic predictable for test assertions.

### Testing Performed

- `cargo test -p fdemon-app --lib handler::new_session::target_selector` - 19 tests, all passed
- `cargo test -p fdemon-app --lib target_selector_multiselect_key_tests` - 3 tests, all passed
- `cargo test --workspace --lib` - 6,034 tests total, 0 failures
- `cargo clippy --workspace` - no warnings or errors

### Risks/Limitations

1. **Mouse clicks deferred**: Mouse click toggling on device rows is noted in the task as optional/deferred. Not implemented in this task as per instructions.
