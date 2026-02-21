## Task: Fix Session Close Network Task Leak

**Objective**: Add network monitoring cleanup to `handle_close_current_session` to prevent leaked polling tasks when a session is closed while network monitoring is active.

**Depends on**: None
**Severity**: HIGH
**Review ref**: REVIEW.md Issue #3

### Scope

- `crates/fdemon-app/src/handler/session_lifecycle.rs`: Add network task cleanup
- `crates/fdemon-app/src/handler/tests.rs`: Add test for network cleanup on session close

### Root Cause

`handle_close_current_session` (line ~130-152) cleans up `vm_shutdown_tx`, `perf_task_handle`, and `perf_shutdown_tx`, but does NOT clean up `network_task_handle` or `network_shutdown_tx`. The orphaned polling task continues running until application exit.

The `VmServiceDisconnected` handler (update.rs ~line 1283-1290) correctly cleans up both performance and network tasks — this is the reference pattern.

### Fix

In `crates/fdemon-app/src/handler/session_lifecycle.rs`, after the performance cleanup block (line ~151, after `handle.session.performance.monitoring_active = false;`), add:

```rust
// Abort and signal the network monitoring polling task to stop.
if let Some(h) = handle.network_task_handle.take() {
    h.abort();
}
if let Some(tx) = handle.network_shutdown_tx.take() {
    let _ = tx.send(true);
    tracing::info!(
        "Sent network shutdown signal on session close for session {}",
        current_session_id
    );
}
```

This mirrors the existing performance cleanup pattern directly above it, and matches the `VmServiceDisconnected` handler's network cleanup.

### Tests

Add a test that:
1. Creates a session with `network_shutdown_tx = Some(...)` and `network_task_handle = Some(...)`
2. Sends `CloseCurrentSession`
3. Asserts `network_shutdown_tx` is `None` after close
4. Asserts `network_task_handle` is `None` after close

### Verification

```bash
cargo test -p fdemon-app -- close_current_session
cargo test -p fdemon-app -- session_lifecycle
cargo clippy -p fdemon-app
```
