## Task: Clean Up Performance Polling Task on Reconnection

**Objective**: Abort the existing performance polling task before spawning a new one during reconnection (and initial connection), preventing duplicate `VmServiceMemorySample` messages and leaked tokio tasks.

**Depends on**: 01-reconnected-message-variant

**Review Reference**: Phase-2 Review Issue #3

### Scope

- `crates/fdemon-app/src/handler/update.rs`: Add perf task cleanup in `VmServiceConnected` and `VmServiceReconnected` handlers

### Details

#### Problem

When `VmServiceConnected` (or after task 01, `VmServiceReconnected`) fires, the handler dispatches `UpdateAction::StartPerformanceMonitoring` without aborting any existing polling task. The flow:

1. Reconnect occurs → `VmServiceReconnected` dispatched
2. Handler returns `StartPerformanceMonitoring` action
3. `spawn_performance_polling()` creates a new `watch::channel(false)` + `tokio::spawn` polling loop
4. `VmServicePerformanceMonitoringStarted` message stores the new handle, **silently dropping the old `JoinHandle` without aborting it**
5. The old polling task continues running with a still-valid `VmRequestHandle`
6. Both old and new tasks emit `VmServiceMemorySample` — duplicates

The old `JoinHandle` is dropped without `.abort()`, so the task runs forever (leaked) until the session exits.

#### Fix

Add the standard perf cleanup pattern (already used in 4 other sites) to both the `VmServiceConnected` and `VmServiceReconnected` handlers, **before** returning the `StartPerformanceMonitoring` action.

**In `VmServiceConnected` handler** (`update.rs`, inside the `if let Some(handle)` block):
```rust
if let Some(handle) = state.session_manager.get_mut(session_id) {
    // Clean up any existing performance task before spawning a new one.
    // No-op on first connect (both are None), correctly tears down on hot-restart.
    if let Some(h) = handle.perf_task_handle.take() {
        h.abort();
    }
    if let Some(tx) = handle.perf_shutdown_tx.take() {
        let _ = tx.send(true);
    }

    handle.session.vm_connected = true;
    // ... rest of handler ...
}
```

**In `VmServiceReconnected` handler** (same pattern):
```rust
if let Some(handle) = state.session_manager.get_mut(session_id) {
    // Clean up old performance polling before re-subscribing
    if let Some(h) = handle.perf_task_handle.take() {
        h.abort();
    }
    if let Some(tx) = handle.perf_shutdown_tx.take() {
        let _ = tx.send(true);
    }

    handle.session.vm_connected = true;
    // ... rest of handler ...
}
```

#### Reference: Existing cleanup sites

All four of these correctly abort + signal before the handle goes stale:

| Path | File | Pattern |
|------|------|---------|
| `VmServiceDisconnected` | `update.rs:1294-1301` | `perf_task_handle.take().abort()` + `perf_shutdown_tx.take().send(true)` |
| `CloseCurrentSession` | `session_lifecycle.rs:141-150` | Same |
| `SessionExited` | `session.rs:124-134` | Same |
| `AppStop` | `session.rs:192-198` | Same |

### Acceptance Criteria

1. `VmServiceConnected` handler aborts existing perf task before dispatching `StartPerformanceMonitoring`
2. `VmServiceReconnected` handler (from task 01) does the same
3. Cleanup is a no-op on first connection (both fields are `None`)
4. No duplicate `VmServiceMemorySample` messages possible during reconnection
5. No leaked tokio tasks after reconnection
6. `cargo check --workspace` passes
7. `cargo clippy --workspace -- -D warnings` clean

### Notes

- The `perf_task_handle.take()` returns `None` on first connect, so `.abort()` is never called — this is safe
- Network monitoring (`network_task_handle`) likely has the same leak on reconnection, but the review did not flag it. Consider checking and applying the same fix if applicable.
- The `VmServicePerformanceMonitoringStarted` handler at `update.rs:1414-1419` also silently overwrites the old handles — after this fix, the old handles will already be `None` by the time the new ones arrive, so no change needed there.
