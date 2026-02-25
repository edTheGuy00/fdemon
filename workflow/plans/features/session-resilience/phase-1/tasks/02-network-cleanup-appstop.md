## Task: Add network cleanup to AppStop handler

**Objective**: Stop zombie network polling tasks when a Flutter app stops (daemon `app.stop` event) by mirroring the existing performance cleanup pattern for network monitoring fields.

**Depends on**: None

### Scope

- `crates/fdemon-app/src/handler/session.rs`: Add network cleanup block to the `AppStop` branch in `handle_session_message_state`

### Details

**The bug:** When the Flutter daemon sends an `AppStop` event (triggered by `flutter stop`, hot restart, or explicit stop), `handle_session_message_state` (line 161) cleans up:
- VM Service forwarding task (`vm_shutdown_tx`) — lines 174-177
- Performance polling task (`perf_task_handle`, `perf_shutdown_tx`) — lines 180-187

But it does **not** clean up the network polling task. If a hot restart triggers `AppStop` → `AppStart`, the old network polling task continues alongside whatever new monitoring starts, creating a duplicate polling zombie.

**The fix:** Add a network cleanup block immediately after the performance cleanup block (after line 187, before the closing `}` braces). Mirror the exact perf cleanup pattern:

```rust
// Abort and signal the network monitoring polling task to stop.
if let Some(h) = handle.network_task_handle.take() {
    h.abort();
}
if let Some(tx) = handle.network_shutdown_tx.take() {
    let _ = tx.send(true);
    tracing::info!("Sent network shutdown signal for session {}", session_id);
}
```

**Reference pattern** (existing perf cleanup at lines 180-187):
```rust
// Abort and signal the performance polling task to stop.
if let Some(h) = handle.perf_task_handle.take() {
    h.abort();
}
if let Some(tx) = handle.perf_shutdown_tx.take() {
    let _ = tx.send(true);
    tracing::info!("Sent perf shutdown signal for session {}", session_id);
}
handle.session.performance.monitoring_active = false;
```

**Note:** Same as task 01 — `NetworkState` has no `monitoring_active` flag to reset. The `NetworkState::recording` field is a UI preference, not a task lifecycle flag.

### Acceptance Criteria

1. The `AppStop` branch in `handle_session_message_state` calls `handle.network_task_handle.take().abort()`
2. The `AppStop` branch calls `handle.network_shutdown_tx.take().send(true)`
3. Both fields are `None` after the handler returns
4. Log message at `info!` level matches the existing perf cleanup style
5. `cargo check -p fdemon-app` passes
6. All existing tests pass (`cargo test -p fdemon-app`)

### Testing

Covered by task 03. No test changes in this task.

### Notes

- This is a 4-line addition mirroring an existing pattern — minimal risk
- The `AppStop` handler is inside a nested `if let` that checks `app_id` matches. The network cleanup goes inside that same guard, just like the perf cleanup
- The `CloseCurrentSession` path (in `session_lifecycle.rs`) already does network cleanup correctly — this fix closes the `AppStop` gap so all daemon-initiated stops are covered
- Real-world scenario: user does `flutter stop` from another terminal → daemon sends `app.stop` → this handler fires → network polling should stop

---

## Completion Summary

**Status:** Not started
