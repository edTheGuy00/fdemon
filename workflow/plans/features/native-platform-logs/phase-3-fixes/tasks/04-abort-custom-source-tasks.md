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
