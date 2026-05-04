# Task 10: Backfill `SessionManager::remove_session` tests for selected-index branches

**Status:** Not Started
**Estimated Hours:** 0.5h
**Depends On:** —
**Crate / Area:** `fdemon-app`

## Goal

Discharge review item 12: Phase 3 Task 02 changed `SessionManager::remove_session`'s `selected_index` logic to a three-branch decision (empty → 0; pos < selected → decrement; selected_index >= len → clamp; otherwise unchanged). This was correct in isolation, but only one existing test (`test_remove_session`) exercises the new code. Three call sites rely on the new semantics — `close_session_internal` (Phase 3 Task 02), `evict_oldest_stopped` (existing), and `handle_session_spawn_failed` (existing) — and silent regressions in any of them would cause "wrong-session-selected" UX bugs after eviction or spawn-failure.

Backfill three targeted tests covering the under-exercised branches.

## Files Modified (Write)

- `crates/fdemon-app/src/session_manager.rs`

## Files Read

- `crates/fdemon-app/src/session_manager.rs` — read existing test patterns (`test_remove_session`, etc.) to match style
- `crates/fdemon-app/src/handler/session_lifecycle.rs` — read `close_session_internal` for context on which branches it exercises
- (read other call sites only if needed to understand the failure modes — no edits to those files in this task)

## Implementation Steps

Add three new tests to the existing `#[cfg(test)] mod tests { ... }` block in `session_manager.rs`. Match the surrounding test style (descriptive names, comment headers).

### Test 1: Remove a non-selected, lower-index session preserves selected-session identity

```rust
#[test]
fn test_remove_session_pre_selected_preserves_identity() {
    let mut manager = SessionManager::new();
    let id1 = manager.create_session(/* …test fixture… */);
    let id2 = manager.create_session(/* … */);
    let id3 = manager.create_session(/* … */);

    // Select id2 (index 1)
    assert!(manager.select_by_index(1));
    assert_eq!(manager.selected_id(), Some(id2));

    // Remove id1 (index 0) — *before* the selection
    manager.remove_session(id1);

    // selected_index should have decremented from 1 → 0, but the *identity*
    // (id2) of the selected session should be preserved.
    assert_eq!(manager.session_order(), &[id2, id3]);
    assert_eq!(manager.selected_index(), 0);
    assert_eq!(manager.selected_id(), Some(id2),
        "removing a session before the selection must preserve the selected session's identity");
}
```

### Test 2: Remove a non-selected, higher-index session leaves selection unchanged

```rust
#[test]
fn test_remove_session_post_selected_leaves_selection_unchanged() {
    let mut manager = SessionManager::new();
    let id1 = manager.create_session(/* … */);
    let id2 = manager.create_session(/* … */);
    let id3 = manager.create_session(/* … */);

    // Select id2 (index 1)
    assert!(manager.select_by_index(1));

    // Remove id3 (index 2) — *after* the selection
    manager.remove_session(id3);

    assert_eq!(manager.session_order(), &[id1, id2]);
    assert_eq!(manager.selected_index(), 1);
    assert_eq!(manager.selected_id(), Some(id2),
        "removing a session after the selection must leave selected_index untouched");
}
```

### Test 3: Removing the selected session at the end clamps to last (existing behavior, made explicit)

```rust
#[test]
fn test_remove_selected_session_at_end_clamps_to_last() {
    let mut manager = SessionManager::new();
    let id1 = manager.create_session(/* … */);
    let id2 = manager.create_session(/* … */);
    let id3 = manager.create_session(/* … */);

    // Select id3 (last, index 2)
    assert!(manager.select_by_index(2));
    assert_eq!(manager.selected_id(), Some(id3));

    // Remove id3 (the selected session at the last index)
    manager.remove_session(id3);

    // selected_index should clamp to len - 1 = 1, pointing to id2 by identity
    assert_eq!(manager.session_order(), &[id1, id2]);
    assert_eq!(manager.selected_index(), 1);
    assert_eq!(manager.selected_id(), Some(id2),
        "removing the selected session at the end clamps selected_index to the new last");
}
```

## Acceptance Criteria

- [ ] Three new tests added, with names matching `test_remove_session_*` for grep-ability
- [ ] All three tests pass on first run (no regressions surface)
- [ ] The existing `test_remove_session` continues to pass unchanged
- [ ] Total test count for `fdemon-app` increases by exactly 3
- [ ] `cargo test -p fdemon-app session_manager` passes
- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes

## Notes

- **Use the same fixture style as the existing `test_remove_session`.** Look at how it constructs `Session` instances via test helpers (likely `Session::test_default(...)` or similar) and reuse the pattern — do not invent a new fixture.
- **Do not test the call sites' behavior directly.** This task validates `SessionManager::remove_session` semantics; testing `close_session_internal` / `evict_oldest_stopped` / `handle_session_spawn_failed` end-to-end is a separate concern. The three tests above transitively cover those callers because they all funnel through `remove_session`.
- **The test names use `_pre_selected_` / `_post_selected_` / `_at_end_clamps_` to make the branch they exercise grep-friendly.** Pick names that survive a future maintainer's grep for "selected_index" or "remove_session".
- If the existing test infrastructure for creating sessions requires async / tokio runtime setup, mirror what `test_remove_session` already does — don't introduce a new pattern.
- Do not modify the production `remove_session` implementation. The semantics are correct as of Phase 3 Task 02; this task only adds coverage.
