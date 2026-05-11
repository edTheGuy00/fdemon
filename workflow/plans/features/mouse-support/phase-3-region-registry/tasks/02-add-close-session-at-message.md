## Task: Add `Message::CloseSessionAt(usize)` and Handler

**Objective**: Introduce a new TEA message that closes the session at a given index, regardless of which session is currently selected. Refactor `handle_close_current_session` to share its cmd-sender + remove logic with the new `handle_close_session_at` handler.

**Depends on**: None

**Estimated Time**: 1 hour

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/message.rs`: Add `CloseSessionAt(usize)` variant next to the existing `CloseCurrentSession` variant in the "Session Navigation" group (around line 232).
- `crates/fdemon-app/src/handler/session_lifecycle.rs`: Add `handle_close_session_at(state, index)`; refactor `handle_close_current_session` to delegate via a shared private helper that takes a `SessionId`.
- `crates/fdemon-app/src/handler/update.rs`: Add the `Message::CloseSessionAt(idx) => session_lifecycle::handle_close_session_at(state, idx)` arm next to `Message::CloseCurrentSession`.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/session_manager.rs`: `SessionManager::session_id_at(index)` (may not exist — see Details), `remove_session`, `selected_index`, `select_by_index`.
- `crates/fdemon-app/src/handler/keys.rs`: Reference for the existing `CloseCurrentSession` keybinding (`x` and `Ctrl+W`) — no changes here.

### Details

#### Why a new variant

Phase 3 Task 07 wires middle-click on a session tab to close *that* session, which may differ from the currently selected session. The existing `Message::CloseCurrentSession` only closes `state.session_manager.selected_id()` — it would close the wrong session if the user middle-clicked a non-selected tab.

#### Message variant

In `message.rs`, in the "Session Navigation (Task 10)" comment block:

```rust
// Close the current session (x / Ctrl+W)
CloseCurrentSession,

/// Close the session at a specific index (middle-click on a tab).
///
/// Differs from [`Message::CloseCurrentSession`] in that it operates on an
/// arbitrary index rather than `state.session_manager.selected_id()`.
/// Out-of-range indices are silently ignored.
CloseSessionAt(usize),
```

#### Helper extraction

Refactor `session_lifecycle::handle_close_current_session` (currently around lines 188-281) into a thin wrapper around a new private helper `close_session_internal(state, session_id)`. The helper does the existing work of:

1. Looking up `app_id` and `cmd_sender` for the session.
2. Sending VM Service / performance / network shutdown signals.
3. Aborting native log capture.
4. Sending the daemon `Stop` command.
5. Calling `state.session_manager.remove_session(session_id)`.
6. If no sessions remain, opening the New Session dialog.

```rust
/// Shared logic for closing a session by id. Used by both
/// [`handle_close_current_session`] (current-session shortcut) and
/// [`handle_close_session_at`] (middle-click on an arbitrary tab).
fn close_session_internal(state: &mut AppState, session_id: SessionId) -> UpdateResult {
    // Move the body of the existing handle_close_current_session here,
    // substituting `current_session_id` -> `session_id` and skipping the
    // `state.session_manager.len() <= 1` quit-on-last-session check (the
    // caller decides whether to convert "close last session" into "quit").
    //
    // Important: do NOT call state.session_manager.selected_id() here —
    // the caller passes the id explicitly.
    //
    // ...existing body...
}
```

`handle_close_current_session` becomes:

```rust
pub fn handle_close_current_session(state: &mut AppState) -> UpdateResult {
    // Preserve the existing behavior: if there's only one session (or none),
    // treat 'x' as a quit request.
    if state.session_manager.len() <= 1 {
        state.request_quit();
        return UpdateResult::none();
    }

    let Some(current_session_id) = state.session_manager.selected_id() else {
        return UpdateResult::none();
    };

    close_session_internal(state, current_session_id)
}
```

`handle_close_session_at` is new:

```rust
/// Close the session at `index` (0-based, in tab order). Out-of-range
/// indices are silently ignored. If `index` happens to be the currently
/// selected session AND it is the last remaining session, converts the
/// action into a quit request (mirroring `handle_close_current_session`).
pub fn handle_close_session_at(state: &mut AppState, index: usize) -> UpdateResult {
    // Resolve index -> session_id. SessionManager already exposes ordered
    // iteration; if there is no `session_id_at(index)` accessor, add one
    // (one-line lookup against `session_order[index]`).
    let Some(session_id) = state.session_manager.session_id_at(index) else {
        return UpdateResult::none(); // out of range — silently ignore
    };

    // Mirror the "last session = quit" semantics of the keyboard shortcut.
    if state.session_manager.len() <= 1 {
        state.request_quit();
        return UpdateResult::none();
    }

    close_session_internal(state, session_id)
}
```

If `SessionManager::session_id_at(index)` does not already exist, add a small accessor in `session_manager.rs` (mark this in the "Files Modified" list when filing this task — this is a one-line addition):

```rust
/// Return the `SessionId` at `index` in tab order, or `None` if out of range.
pub fn session_id_at(&self, index: usize) -> Option<SessionId> {
    self.session_order.get(index).copied()
}
```

**Implementor note**: confirm via `grep -n "session_order\|SessionId" crates/fdemon-app/src/session_manager.rs` whether such an accessor exists before adding it.

#### Dispatcher arm

In `handler/update.rs`, find the existing `Message::CloseCurrentSession` arm (line 544) and add:

```rust
Message::CloseCurrentSession => session_lifecycle::handle_close_current_session(state),
Message::CloseSessionAt(idx) => session_lifecycle::handle_close_session_at(state, idx),
```

### Acceptance Criteria

1. `Message::CloseSessionAt(usize)` exists with the doc-comment above.
2. `session_lifecycle::handle_close_session_at(state, index)` is `pub`, mirrors the cleanup logic of `handle_close_current_session`, and shares the implementation via the new private `close_session_internal`.
3. Out-of-range indices return `UpdateResult::none()` without modifying state.
4. Closing the last session via `CloseSessionAt(0)` triggers the same quit path as `CloseCurrentSession`.
5. Closing a non-selected session preserves the currently selected session's identity (the index of the *currently selected* session may change due to renumbering after removal).
6. `update.rs` dispatches `Message::CloseSessionAt(idx)` to the new handler.
7. No regression in any existing `CloseCurrentSession` test (`handler/tests.rs`).
8. No clippy warnings, no fmt diff.

### Testing

Add tests to `handler/tests.rs` (next to the existing close-session tests around lines 530-580):

```rust
#[test]
fn test_close_session_at_specific_index_removes_only_that_session() {
    let mut state = AppState::new();
    let manager = &mut state.session_manager;
    let id1 = manager.create_session(&test_device("d1", "iPhone")).unwrap();
    let id2 = manager.create_session(&test_device("d2", "Pixel")).unwrap();
    let id3 = manager.create_session(&test_device("d3", "Web")).unwrap();
    manager.select_by_id(id2); // select the middle one

    update(&mut state, Message::CloseSessionAt(0)); // close iPhone

    assert_eq!(state.session_manager.len(), 2);
    assert!(state.session_manager.get(id1).is_none(), "session 0 was closed");
    assert!(state.session_manager.get(id2).is_some(), "Pixel preserved");
    assert!(state.session_manager.get(id3).is_some(), "Web preserved");
    assert_eq!(
        state.session_manager.selected_id(),
        Some(id2),
        "selection follows the live session, not the index"
    );
}

#[test]
fn test_close_session_at_out_of_range_is_noop() {
    let mut state = AppState::new();
    state
        .session_manager
        .create_session(&test_device("d1", "iPhone"))
        .unwrap();
    let count_before = state.session_manager.len();

    update(&mut state, Message::CloseSessionAt(99));

    assert_eq!(state.session_manager.len(), count_before);
}

#[test]
fn test_close_session_at_last_session_triggers_quit() {
    let mut state = AppState::new();
    state
        .session_manager
        .create_session(&test_device("d1", "iPhone"))
        .unwrap();
    state.settings.behavior.confirm_quit = false; // bypass dialog

    update(&mut state, Message::CloseSessionAt(0));

    assert!(state.should_quit(), "closing the only session should quit");
}

#[test]
fn test_close_session_at_zero_when_selected_is_zero_picks_next() {
    // Sanity check that closing the selected session at index 0 leaves
    // selection on a sensible session (delegates to existing
    // SessionManager::remove_session post-removal selection logic).
    let mut state = AppState::new();
    let manager = &mut state.session_manager;
    let id1 = manager.create_session(&test_device("d1", "iPhone")).unwrap();
    let id2 = manager.create_session(&test_device("d2", "Pixel")).unwrap();
    manager.select_by_index(0); // select id1
    let _ = id1;

    update(&mut state, Message::CloseSessionAt(0));

    assert_eq!(state.session_manager.len(), 1);
    assert_eq!(state.session_manager.selected_id(), Some(id2));
}
```

### Notes

- This task is **independent of the rest of Phase 3** (no dependency on Task 01) — it can be picked up in parallel as a Wave-1 task.
- Do NOT touch `handler/keys.rs` — the keyboard shortcut continues to use `CloseCurrentSession`.
- The existing `handle_close_current_session` returns `UpdateResult::action(UpdateAction::DiscoverDevices { flutter })` when no sessions remain after removal AND a Flutter SDK is available. Preserve this path in `close_session_internal`. Only the "len <= 1 → request_quit" early-return is caller-specific.
- The async `cmd_sender.send(Stop { ... })` task is spawned via `tokio::spawn` inside the helper. Both callers must hit this path identically.
- If you discover that `SessionManager` needs more than just `session_id_at`, keep additions minimal — the goal is index → SessionId resolution, nothing else.

---

## Completion Summary

**Status:** Done
**Branch:** feat/mouse-support

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/message.rs` | Added `CloseSessionAt(usize)` variant with doc comment next to `CloseCurrentSession` |
| `crates/fdemon-app/src/session_manager.rs` | Added `session_id_at(index)` accessor; fixed `remove_session` to preserve selected session identity when removing a session before the selection |
| `crates/fdemon-app/src/handler/session_lifecycle.rs` | Extracted `close_session_internal` private helper; refactored `handle_close_current_session` to delegate via it; added new `handle_close_session_at` pub function |
| `crates/fdemon-app/src/handler/update.rs` | Added `Message::CloseSessionAt(idx)` dispatch arm next to `CloseCurrentSession` |
| `crates/fdemon-app/src/handler/tests.rs` | Added 4 tests for `CloseSessionAt` covering specific index removal, out-of-range noop, last-session quit, and post-removal selection |

### Notable Decisions/Tradeoffs

1. **`remove_session` selection identity fix**: The acceptance criterion requires that closing a non-selected session preserves the currently selected session's identity. The existing `remove_session` only clamped the index on overflow but did not decrement when removing before the selection. This caused the test to fail (selected index stayed at 1, now pointing to `id3` instead of `id2`). The fix adds a `pos < selected_index` decrement branch, which is correct and backward-compatible — existing tests still pass because all pre-existing test scenarios for `remove_session` either remove the selected session or remove one after it.

2. **`handle_close_current_session` simplification**: The original used a nested `if let Some(...) { ... }` pattern. The refactor converts it to an early return with `let Some(...) else { return ... }`, which is cleaner and makes the delegation to `close_session_internal` explicit.

3. **`close_session_internal` always runs discovery check**: The helper checks `session_manager.is_empty()` after removal and triggers `DiscoverDevices` if a Flutter SDK is available. Both callers (`handle_close_current_session` and `handle_close_session_at`) hit this path identically, as required by the task notes.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo test --workspace` - Passed (2002 tests in fdemon-app; 0 failures)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed (0 warnings)

### Risks/Limitations

1. **`remove_session` behavior change**: The decrement-before-selection fix is a behavior change to an existing public method. All 2002 existing tests pass, and the new logic is semantically correct (preserving session identity), but any external consumer relying on index-preserving behavior (rather than identity-preserving) would see a difference. In practice, all callers in this codebase care about which session is selected, not its index.
