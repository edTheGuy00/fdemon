## Task: Fix Performance Polling Lifecycle Management

**Objective**: Fix the blocking resource leak where `perf_shutdown_tx` is not signaled on session close, and track the polling task's JoinHandle for clean shutdown.

**Depends on**: None

**Estimated Time**: 2-3 hours

### Scope

- `crates/fdemon-app/src/handler/session_lifecycle.rs`: Signal `perf_shutdown_tx` in `handle_close_current_session`
- `crates/fdemon-app/src/handler/session.rs`: Signal `perf_shutdown_tx` in `handle_session_exited` and `AppStop` branch
- `crates/fdemon-app/src/session.rs`: Add `perf_task_handle: Option<JoinHandle<()>>` to `SessionHandle`
- `crates/fdemon-app/src/actions.rs`: Store JoinHandle from `spawn_performance_polling` via message
- `crates/fdemon-app/src/message.rs`: Add JoinHandle to `VmServicePerformanceMonitoringStarted` message
- `crates/fdemon-app/src/handler/update.rs`: Store JoinHandle when processing the started message; abort on disconnect
- `crates/fdemon-app/src/handler/tests.rs`: Update existing tests, add new tests

### Details

#### Part A: Signal `perf_shutdown_tx` on All Close Paths

There are **three** code paths where a session ends but `perf_shutdown_tx` is not signaled:

**1. Explicit session close** (`session_lifecycle.rs:118-126`)

`handle_close_current_session` already signals `vm_shutdown_tx` but not `perf_shutdown_tx`. Add after the `vm_shutdown_tx` block:

```rust
// Signal performance monitoring shutdown
if let Some(tx) = handle.perf_shutdown_tx.take() {
    let _ = tx.send(true);
}
handle.session.performance.monitoring_active = false;
```

**2. Session process exited** (`handler/session.rs:117-123`)

`handle_session_exited` signals `vm_shutdown_tx` but not `perf_shutdown_tx`. Add the same pattern.

**3. AppStop daemon message** (`handler/session.rs:162-164`)

The `AppStop` branch signals `vm_shutdown_tx` but not `perf_shutdown_tx`. Add the same pattern.

**Reference pattern** — `VmServiceDisconnected` handler at `update.rs:1181-1195` already does this correctly. Mirror that exact sequence.

#### Part B: Track JoinHandle

The current `session_tasks` map (`Arc<Mutex<HashMap<SessionId, JoinHandle<()>>>>`) only holds one handle per session — `ConnectVmService` overwrites `SpawnSession`'s entry. Do NOT add the perf handle to `session_tasks`.

Instead, store it directly on `SessionHandle` as a new field:

```rust
pub struct SessionHandle {
    // ... existing fields ...
    /// JoinHandle for the performance monitoring polling task.
    /// Aborted on session close or VM disconnect.
    pub perf_task_handle: Option<tokio::task::JoinHandle<()>>,
}
```

**Flow:**
1. `spawn_performance_polling` already returns `JoinHandle<()>`
2. Send the handle alongside `perf_shutdown_tx` in `VmServicePerformanceMonitoringStarted`
3. In the message handler at `update.rs:1263-1268`, store both on `SessionHandle`
4. On disconnect/close, abort the handle before signaling shutdown:
   ```rust
   if let Some(h) = handle.perf_task_handle.take() {
       h.abort();
   }
   ```

**Note on `Message` enum**: `Message` requires `Clone`. `JoinHandle` is not `Clone`. Wrap in `Arc<Mutex<Option<JoinHandle<()>>>>` for the message transport, or use a separate oneshot channel to deliver it. The `Arc<Mutex<Option<>>>` approach is simpler — take the handle out of the `Option` when storing on `SessionHandle`.

### Acceptance Criteria

1. `perf_shutdown_tx` is signaled (`tx.send(true)`) on all three close paths
2. `perf_task_handle` is stored on `SessionHandle` and aborted on close/disconnect
3. `monitoring_active` is set to `false` on all close paths
4. Existing tests updated to expect the new message shape
5. New test: verify `perf_shutdown_tx` is signaled when `handle_close_current_session` runs
6. New test: verify `perf_shutdown_tx` is signaled when `handle_session_exited` runs
7. New test: verify `perf_shutdown_tx` is signaled on `AppStop`
8. `cargo test -p fdemon-app` passes
9. `cargo clippy -p fdemon-app -- -D warnings` passes

### Testing

```rust
#[test]
fn test_close_session_signals_perf_shutdown() {
    // Setup: create session with perf_shutdown_tx set
    // Action: process CloseCurrentSession message
    // Assert: perf_shutdown_tx receiver sees true
    // Assert: monitoring_active is false
}

#[test]
fn test_session_exited_signals_perf_shutdown() {
    // Setup: create session with perf_shutdown_tx set
    // Action: process SessionExited message
    // Assert: perf_shutdown_tx receiver sees true
}

#[test]
fn test_app_stop_signals_perf_shutdown() {
    // Setup: create session with perf_shutdown_tx and app_id
    // Action: process Daemon(AppStop) message
    // Assert: perf_shutdown_tx receiver sees true
}
```

### Notes

- The `VmServiceDisconnected` handler at `update.rs:1189-1192` is the canonical reference — replicate its exact 3-step pattern (signal tx, set to None, set monitoring_active = false)
- `handle_close_current_session` is the highest priority since it's the most common user-triggered path
- The `handle_session_exited` and `AppStop` paths may fire when the Flutter process crashes — important for preventing zombie polling tasks

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/session.rs` | Added `perf_task_handle: Option<tokio::task::JoinHandle<()>>` to `SessionHandle`; updated `Debug` impl and `new()` constructor |
| `crates/fdemon-app/src/message.rs` | Added `perf_task_handle: Arc<Mutex<Option<JoinHandle<()>>>>` field to `VmServicePerformanceMonitoringStarted` |
| `crates/fdemon-app/src/actions.rs` | Refactored `spawn_performance_polling` to create the shutdown channel outside the task; added Arc<Mutex<Option<JoinHandle<>>>> slot for rendezvous pattern; changed return type to `()` |
| `crates/fdemon-app/src/handler/update.rs` | Updated `VmServiceDisconnected` handler to abort `perf_task_handle` before signaling shutdown; updated `VmServicePerformanceMonitoringStarted` handler already had `perf_task_handle` storage (pre-existing partial impl) |
| `crates/fdemon-app/src/handler/session_lifecycle.rs` | Added perf task abort + `perf_shutdown_tx` signal + `monitoring_active = false` in `handle_close_current_session` |
| `crates/fdemon-app/src/handler/session.rs` | Added perf task abort + `perf_shutdown_tx` signal + `monitoring_active = false` in `handle_session_exited` and `handle_session_message_state` (AppStop branch) |
| `crates/fdemon-app/src/handler/tests.rs` | Added 3 new tests: `test_close_session_signals_perf_shutdown`, `test_session_exited_signals_perf_shutdown`, `test_app_stop_signals_perf_shutdown`; existing `test_performance_monitoring_started_stores_shutdown_tx` was already updated for new message shape |

### Notable Decisions/Tradeoffs

1. **Arc<Mutex<Option<JoinHandle<>>>> rendezvous**: The `VmServicePerformanceMonitoringStarted` message is sent as the first `.await` inside the spawned task. To include the `JoinHandle` in the message, a shared slot (`Arc<Mutex<Option<>>>`) is pre-created before `tokio::spawn`, filled synchronously after spawn returns (before the runtime schedules the task), and sent via the message. The TEA handler extracts the handle with `.take()`. This matches the task's suggested approach exactly.

2. **`spawn_performance_polling` returns `()`**: The return type was changed from `JoinHandle<()>` to `()` because the handle is now tracked via `SessionHandle.perf_task_handle`. Callers already discarded the return value.

3. **Abort before signal on all paths**: Each close path now calls `.abort()` on the task handle before sending `true` on the shutdown channel. This provides belt-and-suspenders cleanup: the abort is immediate, the signal is graceful. This matches the pattern established in `VmServiceDisconnected`.

4. **`monitoring_active = false` on all paths**: Set explicitly on all three close paths, consistent with the canonical `VmServiceDisconnected` handler.

### Testing Performed

- `cargo check -p fdemon-app` - Passed
- `cargo test -p fdemon-app` - Passed (801 tests: 798 pre-existing + 3 new)
- `cargo clippy -p fdemon-app -- -D warnings` - Passed (no warnings)
- `cargo fmt --all` - Applied (no changes needed)
- `cargo check --workspace` - Passed (all crates)

### Risks/Limitations

1. **Tokio scheduling assumption**: The rendezvous pattern relies on the spawned task not running until the current thread yields to the runtime after `tokio::spawn`. This is guaranteed by Tokio's cooperative scheduling — the task is only polled when the current thread reaches an `.await` point. The slot fill happens synchronously between `tokio::spawn` and the next `.await`, so the guarantee holds.

2. **Double-signal harmless**: If both `.abort()` and `tx.send(true)` fire, the abort wins. The polling loop's `perf_shutdown_rx.changed()` branch may never execute, but that is safe — `abort()` cancels the future at the next `.await` point.
