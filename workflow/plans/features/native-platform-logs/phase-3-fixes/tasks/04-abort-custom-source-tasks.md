## Task: Abort Custom Source Task Handles on Shutdown

**Objective**: Add `.abort()` fallback to custom source cleanup, matching the pattern used for platform capture and DevTools tasks.

**Depends on**: None

**Review Issue**: #4 (MAJOR)

### Scope

- `crates/fdemon-app/src/session/handle.rs`: Add abort to custom source shutdown loop (~lines 197-206)

### Details

The `shutdown_native_logs` method in `handle.rs` sends shutdown signals to custom source tasks but never calls `.abort()` on their `JoinHandle`s. When `clear()` is called, the handles are dropped, which **detaches** the tasks — they continue running in the background with no way to stop them.

**Current code (handle.rs:197-206):**
```rust
for handle in &self.custom_source_handles {
    let _ = handle.shutdown_tx.send(true);
    tracing::debug!(
        "Sent shutdown signal to custom log source '{}' for session {}",
        handle.name,
        self.session.id
    );
}
self.custom_source_handles.clear();
```

**Correct pattern (platform capture, handle.rs:185-195):**
```rust
if let Some(tx) = self.native_log_shutdown_tx.take() {
    let _ = tx.send(true);
}
if let Some(handle) = self.native_log_task_handle.take() {
    handle.abort();  // <-- abort IS called
}
```

DevTools tasks (`perf_task_handle`, `network_task_handle`) also follow the signal-then-abort pattern.

**Fixed code:**
```rust
for mut handle in self.custom_source_handles.drain(..) {
    let _ = handle.shutdown_tx.send(true);
    if let Some(task) = handle.task_handle.take() {
        task.abort();
    }
    tracing::debug!(
        "Shut down custom log source '{}' for session {}",
        handle.name,
        self.session.id
    );
}
```

Using `drain(..)` instead of `&self` + `clear()` consumes the handles, avoids the borrow-then-clear pattern, and allows calling `.take()` on each handle's `task_handle`.

### Acceptance Criteria

1. Custom source task handles are aborted on shutdown (not just signaled)
2. `drain(..)` is used instead of iterate-then-clear
3. Pattern matches platform capture and DevTools cleanup
4. Existing tests pass

### Testing

Verify existing shutdown tests still pass. The abort is a fallback — the graceful shutdown signal should still work. The abort prevents zombie tasks when the signal is ignored.

### Notes

- Check whether `CustomSourceHandle::task_handle` is `Option<JoinHandle<()>>` or `JoinHandle<()>`. If it's not `Option`, wrap it to support `.take()`, or just call `.abort()` directly.
- The `drain(..)` pattern is idiomatic Rust for consuming a Vec while iterating.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/session/handle.rs` | Replaced iterate-by-ref + clear() with drain(..) loop; added task.abort() call on each custom source handle's task_handle |

### Notable Decisions/Tradeoffs

1. **drain(..) consumes the Vec in place**: Using `drain(..)` rather than iterating `&self` then calling `clear()` is idiomatic and ensures each `CustomSourceHandle` is moved out, allowing `handle.task_handle.take()` without a borrow conflict. The Vec is empty after the loop completes — no separate `clear()` call is needed.
2. **`task_handle` was already `Option<JoinHandle<()>>`**: No struct change was required. The `CustomSourceHandle` struct already had `task_handle: Option<tokio::task::JoinHandle<()>>` so `.take()` worked directly.
3. **Pre-existing clippy failure in native_logs.rs**: `cargo clippy -p fdemon-app -- -D warnings` fails with a `too_many_arguments` warning in `actions/native_logs.rs:42`. This failure exists on the branch before this task's change (confirmed by stashing and re-running). It is not introduced by this task.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check -p fdemon-app` - Passed
- `cargo test -p fdemon-app --lib` - Passed (1549 passed, 0 failed, 4 ignored)
- `cargo clippy -p fdemon-app -- -D warnings` - Failed (pre-existing `too_many_arguments` in native_logs.rs, not introduced by this task)

### Risks/Limitations

1. **Pre-existing clippy warning**: The `too_many_arguments` lint in `actions/native_logs.rs` was present before this change. A separate task should add `#[allow(clippy::too_many_arguments)]` or refactor the function to use a params struct.
