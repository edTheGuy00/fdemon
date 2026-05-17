## Task: Bind Enter / Shift+H / Tab / Shift+Tab to the new Inspector messages

**Objective**: Add the Phase 1 key bindings in `handler/keys.rs`. `Enter` opens details, `Shift+H` toggles hide-implementation, `Tab` / `Shift+Tab` cycle Details tabs. Existing vim-style `h` collapse remains.

**Depends on**: 02-state-inspector-extensions, 04-message-variants

**Estimated Time**: 2–3 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/keys.rs`

**Files Read (Dependencies):**
- `crates/fdemon-app/src/message.rs` (new variants from task 04).
- `crates/fdemon-app/src/state.rs` (read `details_open` for context-sensitive bindings).

### Details

The Inspector key handling lives around `crates/fdemon-app/src/handler/keys.rs:628–642` (see existing `in_inspector` guard pattern). The new bindings must respect the same guard so they don't fire from other DevTools tabs or other UI modes.

#### 1. `Enter` → `DevToolsInspectorOpenDetails` (tree mode only)

Currently `Enter` is handled by `KeyCode::Enter` somewhere in the inspector keymap; verify the existing arm. The current Inspector entry at line 635 binds `KeyCode::Enter` alongside `Right` to mean "Expand":

```rust
InputKey::Enter if in_inspector => {
    Some(Message::DevToolsInspectorNavigate(InspectorNav::Expand))
}
```

(Check the precise current shape with `grep -n "InspectorNav::Expand" crates/fdemon-app/src/handler/keys.rs`.)

Split this arm: `Right` keeps the Expand binding, but `Enter` becomes "open details":

```rust
InputKey::Enter if in_inspector && !details_open => {
    Some(Message::DevToolsInspectorOpenDetails)
}
InputKey::Right if in_inspector && !details_open => {
    Some(Message::DevToolsInspectorNavigate(InspectorNav::Expand))
}
InputKey::Right if in_inspector && details_open => {
    Some(Message::DevToolsInspectorCycleTab { forward: true })
}
InputKey::Left if in_inspector && details_open => {
    Some(Message::DevToolsInspectorCycleTab { forward: false })
}
InputKey::Left | InputKey::Char('h') if in_inspector && !details_open => {
    Some(Message::DevToolsInspectorNavigate(InspectorNav::Collapse))
}
```

`details_open` is read off `state.devtools_view_state.inspector.details_open`. If the existing match block doesn't have access to `state`, lift the binding to a helper function that takes `&AppState` and returns `Option<Message>`. Most likely `handle_key` already has access (line 320 / line 553 / line 602 patterns reference `state` indirectly via `in_*` flags).

#### 2. `Shift+H` → `DevToolsInspectorToggleHideImplementation`

Add a NEW arm. Lowercase `h` is bound elsewhere (vim collapse — see #1 above). Uppercase `H` is what crossterm reports when the user presses `Shift+h`. Verify how the project's `InputKey` enum represents this:

- If `InputKey::Char('H')` is the convention (case-sensitive), match `Char('H')`.
- If crossterm modifier flags are normalized into the char, this is straightforward. Otherwise the existing key-event handling will yield a `KeyModifiers::SHIFT + Char('h')` pair — bind on that combination.

```rust
InputKey::Char('H') if in_inspector => {
    Some(Message::DevToolsInspectorToggleHideImplementation)
}
```

Pattern-check existing uppercase bindings if any exist (search `Char('[A-Z]')` in keys.rs to confirm the project's convention).

#### 3. `Tab` / `Shift+Tab` → `DevToolsInspectorCycleTab`

Only fire when Details is open — otherwise Tab is unbound in Inspector tab.

```rust
InputKey::Tab if in_inspector && details_open => {
    Some(Message::DevToolsInspectorCycleTab { forward: true })
}
InputKey::BackTab if in_inspector && details_open => {
    Some(Message::DevToolsInspectorCycleTab { forward: false })
}
```

(`BackTab` is crossterm's convention for `Shift+Tab`. Confirm by grepping `InputKey::` definitions.)

#### 4. Esc handling

Esc routing is in task 05's `handler/devtools/mod.rs` change (the tiered close). Task 06 should leave the existing Esc binding alone — the binding still emits the same `Esc` message; the handler chooses whether to close details or exit.

#### 5. Tests

In the existing `#[cfg(test)] mod tests` block at the bottom of keys.rs (search for `fn test_handle_key`):

- `test_enter_in_inspector_tree_mode_emits_open_details`.
- `test_enter_in_inspector_details_mode_is_unbound` (returns `None` or falls through).
- `test_uppercase_h_in_inspector_emits_toggle_hide_implementation`.
- `test_lowercase_h_in_inspector_still_emits_collapse` (regression guard for the existing vim binding).
- `test_tab_in_inspector_details_mode_emits_cycle_tab_forward`.
- `test_back_tab_in_inspector_details_mode_emits_cycle_tab_backward`.
- `test_tab_in_inspector_tree_mode_is_unbound`.
- `test_left_in_inspector_details_mode_emits_cycle_tab_backward`.
- `test_right_in_inspector_details_mode_emits_cycle_tab_forward`.

### Acceptance Criteria

1. `cargo test -p fdemon-app` passes with all new key-binding tests.
2. Existing inspector key tests at `keys.rs:2174+` continue to pass.
3. Pressing `H` (uppercase) in Inspector tab emits `DevToolsInspectorToggleHideImplementation`.
4. Pressing `h` (lowercase) in Inspector tab still emits `DevToolsInspectorNavigate(Collapse)`.
5. `Tab` in Inspector tree mode is unbound; in Details mode it cycles tabs forward.
6. `Enter` in tree mode opens details; in details mode it is unbound.
7. `cargo clippy -p fdemon-app --all-targets -- -D warnings` passes.

### Testing

```rust
#[test]
fn test_enter_in_inspector_tree_mode_emits_open_details() {
    let state = make_state_in_inspector_tab(/* details_open = */ false);
    let msg = handle_key(&state, InputKey::Enter);
    assert_eq!(msg, Some(Message::DevToolsInspectorOpenDetails));
}

#[test]
fn test_uppercase_h_in_inspector_emits_toggle_hide_implementation() {
    let state = make_state_in_inspector_tab(false);
    let msg = handle_key(&state, InputKey::Char('H'));
    assert_eq!(msg, Some(Message::DevToolsInspectorToggleHideImplementation));
}
```

### Notes

- If the project's `InputKey` does not distinguish uppercase from lowercase chars, add `KeyModifiers` checking via the underlying crossterm event. Confirm by reading the `InputKey` definition (search `pub enum InputKey` in the codebase).
- Mouse clicks on tree rows currently call `DevToolsInspectorSelectRow`. Task 05 makes that handler also early-return when `details_open == true`; this task does not need to change the mouse pipeline.
- Resist adding a Phase-2 binding (e.g., `getProperties` refresh) here. Keep the diff scope to Phase 1 messages only.

---

## Completion Summary

**Status:** Not Started
**Branch:** feat/devtools-inspector-parity

### Files Modified

| File | Changes |
|------|---------|

### Notable Decisions/Tradeoffs

### Testing Performed

### Risks/Limitations
