## Task: Comment ignored `_mods` parameters in `flutter_version.rs` and `new_session.rs`

**Objective**: Add a one-line inline comment to each `handle_scroll` function in `crates/fdemon-app/src/handler/mouse/flutter_version.rs` and `crates/fdemon-app/src/handler/mouse/new_session.rs` explaining why the `_mods` parameter is intentionally unused. A future reader should be able to tell "deliberately ignored" from "accidentally not implemented" at a glance.

**Depends on**: None

**Estimated Time**: 0.25h

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/mouse/flutter_version.rs`: Add a one-line `//` comment immediately after the function signature (or just inside the body before the first `match`) explaining that modifiers are ignored because there is no page-step analogue in this mode.
- `crates/fdemon-app/src/handler/mouse/new_session.rs`: Add a one-line `//` comment in the same place explaining that NewSessionDialog has no Shift+anything keyboard binding so the mouse mirrors that.

**Files Read (Dependencies):**
- None.

### Details

#### `flutter_version.rs`

Today the function reads (lines 12-19):

```rust
pub(super) fn handle_scroll(
    _state: &AppState,
    dir: ScrollDir,
    _mods: KeyModSet,
) -> Option<Message> {
    match dir {
        ScrollDir::Up => Some(Message::FlutterVersionUp),
        ...
    }
}
```

Add a comment after the function signature:

```rust
pub(super) fn handle_scroll(
    _state: &AppState,
    dir: ScrollDir,
    _mods: KeyModSet,
) -> Option<Message> {
    // Modifiers ignored: FlutterVersion has no page-step analogue in the
    // keyboard handler (keys.rs:332-355 binds only j/k and Up/Down).
    match dir {
        ScrollDir::Up => Some(Message::FlutterVersionUp),
        ...
    }
}
```

#### `new_session.rs`

Today the function reads (lines 12-13):

```rust
pub(super) fn handle_scroll(state: &AppState, dir: ScrollDir, _mods: KeyModSet) -> Option<Message> {
    let dialog = &state.new_session_dialog_state;
    ...
}
```

Add a comment after the local binding:

```rust
pub(super) fn handle_scroll(state: &AppState, dir: ScrollDir, _mods: KeyModSet) -> Option<Message> {
    let dialog = &state.new_session_dialog_state;
    // Modifiers ignored: NewSessionDialog's keyboard handlers (keys.rs:793-896)
    // bind no Shift+anything for navigation, so the mouse mirrors that — every
    // wheel direction is single-step regardless of held modifier.
    ...
}
```

The exact wording can vary; what matters is that a reader knows the `_mods` underscore is deliberate, not a half-finished implementation.

### Acceptance Criteria

1. `flutter_version.rs::handle_scroll` carries a one-line comment naming the absence of page-step analogue and citing `keys.rs:332-355` (or equivalent reference).
2. `new_session.rs::handle_scroll` carries a one-line comment naming the absence of Shift+anything binding and citing `keys.rs:793-896` (or equivalent reference).
3. No behavior change — both functions still return the same messages for the same inputs.
4. No new imports, no new tests required.
5. `cargo fmt --all -- --check`, `cargo check --workspace --all-targets`, `cargo test --workspace`, and `cargo clippy --workspace --all-targets -- -D warnings` all pass.

### Testing

```bash
cargo test -p fdemon-app handler::mouse::flutter_version
cargo test -p fdemon-app handler::mouse::new_session
cargo clippy --workspace --all-targets -- -D warnings
```

All existing tests must continue to pass without modification.

### Notes

- **Why both files in one task.** Each file gets one small comment. Splitting into two tasks would over-fragment the orchestrator wave for a 10-minute change. Both files carry the same conceptual fix (explain the `_mods` underscore).
- **DO NOT touch `settings.rs`.** Settings also has an unused `_mods` parameter, but Task 02 owns that file in this phase. The existing comment in `settings.rs` ("Modifier handling. Settings has no PageUp/PageDown analogues...") in the prose body of the task plan was reflected in the test name (`modifier_keys_do_not_change_behavior_in_main_list`), but the production function itself does not have an inline comment — that is acceptable to leave for now. If a future cleanup wave wants to add one, it can do so when next editing `settings.rs`.
- **DO NOT touch `mod.rs`** — Task 04 owns it.
- **DO NOT touch tests** — Task 06 owns `tests.rs`. Existing per-submodule unit tests already cover modifier-ignore behavior; this task adds production-side comments only.
- **Why production-side comment matters.** Test names like `modifier_keys_do_not_change_behavior` document the contract from the test's perspective, but a developer reading `flutter_version.rs::handle_scroll` cold has no signal that the underscore is intentional.

---

## Completion Summary

**Status:** <!-- Done / Blocked / Failed -->
**Branch:** <!-- current branch name -->

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/mouse/flutter_version.rs` | <!-- one-line comment added --> |
| `crates/fdemon-app/src/handler/mouse/new_session.rs` | <!-- one-line comment added --> |

### Notable Decisions/Tradeoffs

1. **<Decision>**: <!-- Rationale -->

### Testing Performed

- `cargo fmt --all -- --check` — Passed/Failed
- `cargo test -p fdemon-app handler::mouse` — Passed/Failed
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed/Failed

### Risks/Limitations

None known.
