## Task: Add network cleanup to `handle_session_exited`

**Objective**: Stop zombie network polling tasks when a Flutter process exits by mirroring the existing performance cleanup pattern for network monitoring fields.

**Depends on**: None

### Scope

- `crates/fdemon-app/src/handler/session.rs`: Add network cleanup block to `handle_session_exited`

### Details

**The bug:** When a Flutter process exits (normal exit, crash, or SIGKILL), `handle_session_exited` (line 95) cleans up:
- VM Service forwarding task (`vm_shutdown_tx`) — lines 116-119
- Performance polling task (`perf_task_handle`, `perf_shutdown_tx`) — lines 125-135

But it does **not** clean up the network polling task (`network_task_handle`, `network_shutdown_tx`). The network polling loop in `actions.rs:1734-1777` continues running as a zombie, sending `VmServiceHttpProfileReceived` messages for a dead session.

**The fix:** Add a network cleanup block immediately after the performance cleanup block (after line 135, before the closing `}` of the `if let Some(handle)` block). Mirror the exact perf cleanup pattern:

```rust
// Abort and signal the network monitoring polling task to stop.
if let Some(h) = handle.network_task_handle.take() {
    h.abort();
}
if let Some(tx) = handle.network_shutdown_tx.take() {
    let _ = tx.send(true);
    tracing::info!(
        "Sent network shutdown signal on process exit for session {}",
        session_id
    );
}
```

**Reference pattern** (existing perf cleanup at lines 125-135):
```rust
// Abort and signal the performance polling task to stop.
if let Some(h) = handle.perf_task_handle.take() {
    h.abort();
}
if let Some(tx) = handle.perf_shutdown_tx.take() {
    let _ = tx.send(true);
    tracing::info!(
        "Sent perf shutdown signal on process exit for session {}",
        session_id
    );
}
handle.session.performance.monitoring_active = false;
```

**Note:** `NetworkState` has no `monitoring_active` flag (unlike `PerformanceState`), so there is no equivalent flag to reset. The `NetworkState::recording` field controls UI recording preference, not task lifecycle — leave it unchanged.

### Acceptance Criteria

1. `handle_session_exited` calls `handle.network_task_handle.take().abort()` to kill the task
2. `handle_session_exited` calls `handle.network_shutdown_tx.take().send(true)` for cooperative shutdown
3. Both fields are `None` after the function returns
4. Log message at `info!` level matches the existing perf cleanup style
5. `cargo check -p fdemon-app` passes
6. All existing tests pass (`cargo test -p fdemon-app`)

### Testing

Covered by task 03. No test changes in this task.

### Notes

- This is a 4-line addition mirroring an existing pattern — minimal risk
- The `.take()` pattern atomically extracts and clears the `Option`, preventing double-cleanup
- Aborting the `JoinHandle` is the hard kill; sending on `shutdown_tx` is the cooperative stop. Both are needed for reliability: abort handles the case where the task is blocked on an RPC call, and the shutdown signal handles the case where the task is between poll ticks
- The network polling loop's error tolerance (`actions.rs:1755-1764`) is not modified — we only add explicit cleanup, not behavioral changes

---

## Completion Summary

**Status:** Not started
