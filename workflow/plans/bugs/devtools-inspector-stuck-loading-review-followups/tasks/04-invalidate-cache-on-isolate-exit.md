## Task: Invalidate Isolate Cache on `IsolateExit` Event

**Objective**: Fulfill the BUG.md commitment to invalidate the isolate cache when an isolate exits. Without this, a Dart uncaught exception (or DAP `terminate`) leaves a stale isolate id cached, producing confusing "method not found" errors on subsequent fetches.

**Depends on**: None

**Estimated Time**: 0.5-1 hour

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/devtools/debug.rs` — add `invalidate_isolate_cache()` call to the `IsolateEvent::IsolateExit` arm

**Files Read (Dependencies):**
- `crates/fdemon-app/src/handler/update.rs` — pattern reference (`SessionRestartCompleted` already does this at lines 222-238)

### Details

**Current code (`handler/devtools/debug.rs:311-317`):**
```rust
IsolateEvent::IsolateExit { isolate } => {
    handle.session.debug.remove_isolate(&isolate.id);
    // If the paused isolate exited, clear pause state to reflect reality.
    if handle.session.debug.paused_isolate_id.as_deref() == Some(&isolate.id) {
        handle.session.debug.mark_resumed();
    }
}
```

**Target code:**
```rust
IsolateEvent::IsolateExit { isolate } => {
    handle.session.debug.remove_isolate(&isolate.id);
    // If the paused isolate exited, clear pause state to reflect reality.
    if handle.session.debug.paused_isolate_id.as_deref() == Some(&isolate.id) {
        handle.session.debug.mark_resumed();
    }
    // Drop the cached Flutter UI isolate id — the cached value may point to
    // the exiting isolate. Next fetch will re-resolve.
    if let Some(ref vm_handle) = handle.vm_request_handle {
        vm_handle.invalidate_isolate_cache();
    }
}
```

Use the same pattern as `Message::SessionRestartCompleted` in `handler/update.rs:222-238`.

### Acceptance Criteria

1. The `IsolateEvent::IsolateExit` arm in `handle_isolate_event` calls `invalidate_isolate_cache()` on the session's `vm_request_handle` (if present).
2. A unit test simulates an `IsolateExit` event and asserts `cached_isolate_id()` returns `None` afterward.
3. The cache invalidation happens regardless of whether the exiting isolate is the currently-paused one (the cache could hold either the paused or any other isolate id).
4. All existing tests continue to pass.

### Testing

Add a test in `crates/fdemon-app/src/handler/devtools/debug.rs` tests module (or wherever isolate-event handler tests live):

```rust
#[tokio::test]
async fn test_isolate_exit_clears_resolved_isolate_cache() {
    // Build a SessionHandle with a VmRequestHandle whose isolate_id_cache is pre-populated.
    // Dispatch IsolateEvent::IsolateExit.
    // Assert vm_request_handle.cached_isolate_id() returns None.
}
```

### Notes

- Use `invalidate_isolate_cache()` (the canonical name), not the redundant `clear_isolate_cache` alias. Task 10 removes the alias.
- This fulfills the explicit "Edge Cases & Risks" commitment from the original `BUG.md`.
- No change needed in `fdemon-daemon` — only the handler-layer call site is missing.

---

## Completion Summary

**Status:** Done
**Branch:** fix/devtools-improvements

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/devtools/debug.rs` | Added `invalidate_isolate_cache()` call to `IsolateEvent::IsolateExit` arm; added 3 unit tests |

### Notable Decisions/Tradeoffs

1. **Three tests added instead of one**: The task specified one test, but three were added to cover all acceptance criteria cleanly: (1) basic cache invalidation, (2) cache invalidated even when exiting isolate is not the paused one, and (3) no panic when `vm_request_handle` is `None`. This mirrors the pattern used for `SessionRestartCompleted` tests in `handler/tests.rs`.

2. **No import changes needed**: `vm_request_handle` is already accessible via the session handle in this handler. The `invalidate_isolate_cache()` method is on the `VmRequestHandle` type already in scope through `handle.vm_request_handle`.

### Testing Performed

- `cargo check -p fdemon-app` — Passed
- `cargo test -p fdemon-app handler::devtools::debug` — Passed (57 tests, including 3 new)
- `cargo test -p fdemon-app --lib` — Passed (2193 tests, 0 failed)
- `cargo clippy -p fdemon-app` — No warnings in modified file

### Risks/Limitations

1. **Cache invalidated on any IsolateExit**: The cache is invalidated whenever any isolate exits, not only the cached isolate. This is intentional and conservative — isolate IDs can change on exit and re-registration, and re-resolution via `getVM` is cheap. This matches the task's intent (acceptance criterion 3).
