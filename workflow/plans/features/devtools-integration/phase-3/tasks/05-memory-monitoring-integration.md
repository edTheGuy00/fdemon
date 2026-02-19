## Task: Memory Monitoring Integration

**Objective**: Integrate memory usage polling and GC event handling into the TEA architecture. Add periodic polling via a background task, new Message variants for memory data, session-level performance state, and TEA handlers to process incoming data. This is the core integration task that connects the daemon-layer RPCs (Task 03) to the app-layer state management.

**Depends on**: 01-performance-data-models, 02-vm-request-handle, 03-memory-gc-rpcs

**Estimated Time**: 4-6 hours

### Scope

- `crates/fdemon-app/src/session.rs`: Add `PerformanceState` to `Session`
- `crates/fdemon-app/src/message.rs`: Add memory/GC Message variants
- `crates/fdemon-app/src/handler/mod.rs`: Add `UpdateAction::StartPerformanceMonitoring`
- `crates/fdemon-app/src/handler/update.rs`: Add handlers for new Messages
- `crates/fdemon-app/src/actions.rs`: Spawn periodic polling task, handle GC events in forwarding loop

### Details

#### 1. PerformanceState on Session

Add performance monitoring state to `Session`:

```rust
use fdemon_core::performance::{MemoryUsage, GcEvent, FrameTiming, PerformanceStats, RingBuffer};

/// Performance monitoring state for a session.
#[derive(Debug, Clone)]
pub struct PerformanceState {
    /// Rolling history of memory snapshots.
    pub memory_history: RingBuffer<MemoryUsage>,
    /// Rolling history of GC events.
    pub gc_history: RingBuffer<GcEvent>,
    /// Rolling history of frame timings (populated by Task 06).
    pub frame_history: RingBuffer<FrameTiming>,
    /// Aggregated performance statistics (updated periodically).
    pub stats: PerformanceStats,
    /// Whether performance monitoring is active.
    pub monitoring_active: bool,
}

impl Default for PerformanceState {
    fn default() -> Self {
        Self {
            memory_history: RingBuffer::new(DEFAULT_MEMORY_HISTORY_SIZE),
            gc_history: RingBuffer::new(DEFAULT_GC_HISTORY_SIZE),
            frame_history: RingBuffer::new(DEFAULT_FRAME_HISTORY_SIZE),
            stats: PerformanceStats::default(),
            monitoring_active: false,
        }
    }
}

/// Default number of memory snapshots to keep (at 2s interval = 2 minutes).
const DEFAULT_MEMORY_HISTORY_SIZE: usize = 60;
/// Default number of GC events to keep.
const DEFAULT_GC_HISTORY_SIZE: usize = 100;
/// Default number of frame timings to keep.
const DEFAULT_FRAME_HISTORY_SIZE: usize = 300;
```

Add `performance` field to `Session`:

```rust
pub struct Session {
    // ... existing fields ...

    /// Performance monitoring state (memory, GC, frames).
    pub performance: PerformanceState,
}
```

Initialize in `Session::new()`:

```rust
performance: PerformanceState::default(),
```

#### 2. New Message Variants

Add to `Message` enum in `message.rs`:

```rust
// ─────────────────────────────────────────────────────────
// VM Service Performance Messages (Phase 3)
// ─────────────────────────────────────────────────────────

/// Memory usage snapshot received from periodic polling.
VmServiceMemorySnapshot {
    session_id: SessionId,
    memory: fdemon_core::performance::MemoryUsage,
},

/// GC event received from the GC stream.
VmServiceGcEvent {
    session_id: SessionId,
    gc_event: fdemon_core::performance::GcEvent,
},

/// Performance monitoring task started for a session.
VmServicePerformanceMonitoringStarted {
    session_id: SessionId,
    /// Shutdown sender for the polling task.
    perf_shutdown_tx: std::sync::Arc<tokio::sync::watch::Sender<bool>>,
},
```

Note: `MemoryUsage` and `GcEvent` must implement `Clone` (they do from Task 01).

#### 3. UpdateAction: StartPerformanceMonitoring

Add a new `UpdateAction` variant:

```rust
/// Start periodic performance monitoring for a session.
/// Spawns a background task that polls memory usage at a configured interval.
StartPerformanceMonitoring {
    session_id: SessionId,
},
```

#### 4. Trigger Monitoring After VM Service Connects

In the handler for `VmServiceConnected`, trigger performance monitoring:

```rust
Message::VmServiceConnected { session_id } => {
    // ... existing logic (set vm_connected = true) ...

    // Start performance monitoring
    UpdateResult {
        action: Some(UpdateAction::StartPerformanceMonitoring { session_id }),
        ..Default::default()
    }
}
```

#### 5. Periodic Polling Task (actions.rs)

Handle `UpdateAction::StartPerformanceMonitoring`:

```rust
UpdateAction::StartPerformanceMonitoring { session_id } => {
    // Get the request handle from the session
    if let Some(session_handle) = self.state.session_manager.get(&session_id) {
        if let Some(ref handle) = session_handle.vm_request_handle {
            let handle = handle.clone();
            let msg_tx = self.msg_tx.clone();

            let task_handle = spawn_performance_polling(
                session_id,
                handle,
                msg_tx,
            );
            // Store the task handle for cleanup
            // (Use a separate map or attach to session)
        }
    }
}
```

The polling task:

```rust
use fdemon_daemon::vm_service::performance::get_memory_usage;

/// Default polling interval for memory usage (2 seconds).
const PERF_POLL_INTERVAL: Duration = Duration::from_secs(2);

fn spawn_performance_polling(
    session_id: SessionId,
    handle: VmRequestHandle,
    msg_tx: mpsc::Sender<Message>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        // Create a shutdown channel
        let (perf_shutdown_tx, mut perf_shutdown_rx) = tokio::sync::watch::channel(false);
        let perf_shutdown_tx = std::sync::Arc::new(perf_shutdown_tx);

        // Notify TEA that monitoring has started (sends shutdown handle)
        let _ = msg_tx.send(Message::VmServicePerformanceMonitoringStarted {
            session_id,
            perf_shutdown_tx,
        }).await;

        let mut interval = tokio::time::interval(PERF_POLL_INTERVAL);

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    // Fetch memory usage
                    match handle.main_isolate_id().await {
                        Ok(isolate_id) => {
                            match get_memory_usage(&handle, &isolate_id).await {
                                Ok(memory) => {
                                    let _ = msg_tx.send(Message::VmServiceMemorySnapshot {
                                        session_id,
                                        memory,
                                    }).await;
                                }
                                Err(e) => {
                                    tracing::debug!(
                                        "Memory poll failed for session {}: {}",
                                        session_id, e
                                    );
                                    // Don't break — transient errors are expected
                                    // (e.g., during hot reload)
                                }
                            }
                        }
                        Err(e) => {
                            tracing::debug!(
                                "Could not get isolate ID for perf polling: {}",
                                e
                            );
                        }
                    }
                }
                _ = perf_shutdown_rx.changed() => {
                    if *perf_shutdown_rx.borrow() {
                        tracing::info!(
                            "Performance monitoring stopped for session {}",
                            session_id
                        );
                        break;
                    }
                }
            }
        }
    })
}
```

#### 6. GC Event Handling in forward_vm_events

Extend the `forward_vm_events` function in `actions.rs` to parse GC events. Add after the existing Flutter.Error and LogRecord handling:

```rust
// In forward_vm_events, inside the event match:

// Try parsing as a GC event
if let Some(gc_event) = fdemon_daemon::vm_service::performance::parse_gc_event(
    &event.params.event
) {
    let _ = msg_tx
        .send(Message::VmServiceGcEvent {
            session_id,
            gc_event,
        })
        .await;
    continue;
}
```

#### 7. TEA Handlers for New Messages

In `handler/update.rs`:

```rust
Message::VmServiceMemorySnapshot { session_id, memory } => {
    if let Some(handle) = state.session_manager.get_mut(&session_id) {
        handle.session.performance.memory_history.push(memory);
        handle.session.performance.monitoring_active = true;
    }
    UpdateResult::default()
}

Message::VmServiceGcEvent { session_id, gc_event } => {
    if let Some(handle) = state.session_manager.get_mut(&session_id) {
        handle.session.performance.gc_history.push(gc_event);
    }
    UpdateResult::default()
}

Message::VmServicePerformanceMonitoringStarted { session_id, perf_shutdown_tx } => {
    if let Some(handle) = state.session_manager.get_mut(&session_id) {
        handle.perf_shutdown_tx = Some(perf_shutdown_tx);
    }
    UpdateResult::default()
}
```

#### 8. Shutdown Coordination

Add `perf_shutdown_tx` to `SessionHandle`:

```rust
pub struct SessionHandle {
    // ... existing fields ...

    /// Shutdown sender for the performance monitoring task.
    pub perf_shutdown_tx: Option<std::sync::Arc<tokio::sync::watch::Sender<bool>>>,
}
```

Signal shutdown when the session stops or VM disconnects:

```rust
// In handler for VmServiceDisconnected or AppStop:
if let Some(ref tx) = session_handle.perf_shutdown_tx {
    let _ = tx.send(true);
}
session_handle.perf_shutdown_tx = None;
session_handle.vm_request_handle = None;
session_handle.session.performance.monitoring_active = false;
```

#### 9. Reset Performance State on Reconnection

When the VM Service reconnects (after a hot restart, for example), clear stale performance data:

```rust
// In handler for VmServiceConnected:
if let Some(handle) = state.session_manager.get_mut(&session_id) {
    handle.session.performance = PerformanceState::default();
}
```

### Acceptance Criteria

1. `PerformanceState` is added to `Session` with ring buffers for memory, GC, and frames
2. `VmServiceMemorySnapshot` message pushes data into `memory_history`
3. `VmServiceGcEvent` message pushes data into `gc_history`
4. Performance monitoring starts automatically when VM Service connects
5. Polling task runs at configurable interval (default 2s)
6. Polling handles transient errors without crashing (debug log + continue)
7. GC events from the forwarding loop are parsed and forwarded as messages
8. Performance monitoring stops when session stops or VM disconnects
9. `perf_shutdown_tx` is stored in `SessionHandle` and used for clean shutdown
10. Performance state is reset on VM Service reconnection
11. Existing VM Service behavior (errors, logs, connection lifecycle) is unchanged

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_performance_state_default() {
        let state = PerformanceState::default();
        assert!(!state.monitoring_active);
        assert!(state.memory_history.is_empty());
        assert!(state.gc_history.is_empty());
        assert!(state.frame_history.is_empty());
    }

    #[test]
    fn test_memory_snapshot_handler() {
        let mut state = make_test_state_with_session();
        let session_id = active_session_id(&state);
        let memory = MemoryUsage {
            heap_usage: 50_000_000,
            heap_capacity: 100_000_000,
            external_usage: 10_000_000,
            timestamp: chrono::Local::now(),
        };

        let msg = Message::VmServiceMemorySnapshot { session_id, memory: memory.clone() };
        let result = update(&mut state, msg);
        assert!(result.action.is_none());

        let perf = &state.session_manager.get(&session_id).unwrap()
            .session.performance;
        assert_eq!(perf.memory_history.len(), 1);
        assert_eq!(perf.memory_history.latest().unwrap().heap_usage, 50_000_000);
    }

    #[test]
    fn test_gc_event_handler() {
        let mut state = make_test_state_with_session();
        let session_id = active_session_id(&state);
        let gc = GcEvent {
            gc_type: "Scavenge".into(),
            reason: Some("allocation".into()),
            isolate_id: None,
            timestamp: chrono::Local::now(),
        };

        let msg = Message::VmServiceGcEvent { session_id, gc_event: gc };
        let result = update(&mut state, msg);
        assert!(result.action.is_none());

        let perf = &state.session_manager.get(&session_id).unwrap()
            .session.performance;
        assert_eq!(perf.gc_history.len(), 1);
    }

    #[test]
    fn test_vm_connected_starts_monitoring() {
        let mut state = make_test_state_with_session();
        let session_id = active_session_id(&state);

        let msg = Message::VmServiceConnected { session_id };
        let result = update(&mut state, msg);

        // Should trigger StartPerformanceMonitoring action
        assert!(matches!(
            result.action,
            Some(UpdateAction::StartPerformanceMonitoring { .. })
        ));
    }

    #[test]
    fn test_vm_disconnected_stops_monitoring() {
        let mut state = make_test_state_with_session();
        let session_id = active_session_id(&state);

        // Set up monitoring state
        {
            let handle = state.session_manager.get_mut(&session_id).unwrap();
            handle.session.performance.monitoring_active = true;
        }

        let msg = Message::VmServiceDisconnected { session_id };
        update(&mut state, msg);

        let handle = state.session_manager.get(&session_id).unwrap();
        assert!(!handle.session.performance.monitoring_active);
    }
}
```

### Notes

- **Polling interval of 2 seconds** is a balance between responsiveness and overhead. `getMemoryUsage` is cheap (no heap walk), so 2s is conservative. Could be configurable via `config.toml`'s `[devtools]` section.
- **`getAllocationProfile` is NOT polled automatically** — it's expensive (forces heap walk). It should be triggered on user request (Phase 4 UI) or at very long intervals. This task only polls `getMemoryUsage`.
- **GC events are high-frequency** — Dart's young-generation scavenger runs frequently. The ring buffer (100 entries) prevents unbounded growth. The handler should not do expensive processing per GC event.
- **The `VmServiceConnected` handler is modified** to return `UpdateAction::StartPerformanceMonitoring`. This means the existing handler's return value changes. If it already returns an action, chain them via follow-up message.
- **Shutdown coordination** follows the same pattern as the VM Service forwarding task (`watch::channel(false)` + `Arc` wrapper for Message clone). The `perf_shutdown_tx` is stored in `SessionHandle` alongside the existing `vm_shutdown_tx`.
- **Transient polling errors** (e.g., during hot reload when the isolate is paused) should be logged at debug level and silently skipped. The polling loop continues and the next tick will succeed.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/session.rs` | Added `PerformanceState` struct with ring buffers and constants; added `performance` field to `Session`; added `perf_shutdown_tx` to `SessionHandle`; updated `Session::new()` and `SessionHandle::new()`; added 3 session-level tests |
| `crates/fdemon-app/src/message.rs` | Added `VmServiceMemorySnapshot`, `VmServiceGcEvent`, `VmServicePerformanceMonitoringStarted` message variants |
| `crates/fdemon-app/src/handler/mod.rs` | Added `StartPerformanceMonitoring { session_id, handle: Option<VmRequestHandle> }` to `UpdateAction` enum |
| `crates/fdemon-app/src/handler/update.rs` | Modified `VmServiceConnected` handler to reset performance state and return `StartPerformanceMonitoring` action; modified `VmServiceDisconnected` to signal perf shutdown and clear state; added handlers for 3 new message variants |
| `crates/fdemon-app/src/actions.rs` | Added `PERF_POLL_INTERVAL` constant; imported `parse_gc_event`, `VmRequestHandle`; added `spawn_performance_polling()` function; handled `StartPerformanceMonitoring` in `handle_action`; updated `forward_vm_events` to parse and forward GC events |
| `crates/fdemon-app/src/process.rs` | Added `hydrate_start_performance_monitoring()` to extract `vm_request_handle` from session and populate the `handle` field before dispatch; wrapped `handle_action` call with `Option` handling |
| `crates/fdemon-app/src/handler/tests.rs` | Updated `test_vm_service_connected_sets_flag` and `test_vm_service_connected_ignores_unknown_session` to expect new `StartPerformanceMonitoring` action; added 8 new performance handler tests |

### Notable Decisions/Tradeoffs

1. **Two-phase `UpdateAction` hydration**: The `handler::update()` function cannot access `SessionHandle.vm_request_handle` because it only has `&mut AppState`. Rather than restructuring the TEA architecture, `StartPerformanceMonitoring.handle` is `Option<VmRequestHandle>` — `None` when returned from the handler, populated by `process.rs` before dispatch. This keeps the TEA handler pure and avoids leaking infrastructure concerns into the update function.

2. **`VmServiceConnected` now returns an action**: The existing tests asserted `action.is_none()`. These were updated to expect `StartPerformanceMonitoring`. The action is discarded by `process.rs` if `vm_request_handle` is not yet available (race condition guard).

3. **GC events in `forward_vm_events` use `continue`**: The existing `parse_log_record` branch didn't use `continue`, but to properly short-circuit the GC branch we added `continue` to the `parse_log_record` branch as well. This is consistent and avoids double-processing of events.

4. **`spawn_performance_polling` returns `JoinHandle`**: Unlike the task spec (which returns `()`) we return the handle for future cleanup integration, matching the pattern of `spawn_vm_service_connection`. The handle is currently discarded at the call site.

### Testing Performed

- `cargo fmt --all` — Passed
- `cargo check --workspace` — Passed (0 errors)
- `cargo test --lib --workspace` — Passed (783 fdemon-app, 446 fdemon-tui, 334 fdemon-daemon, 314 fdemon-core — all pass)
- `cargo clippy --workspace -- -D warnings` — Passed (0 warnings)

New tests added:
- `session::tests::test_performance_state_default`
- `session::tests::test_session_has_performance_field`
- `session::tests::test_performance_state_memory_ring_buffer_capacity`
- `handler::tests::test_memory_snapshot_handler`
- `handler::tests::test_gc_event_handler`
- `handler::tests::test_vm_connected_starts_monitoring`
- `handler::tests::test_vm_connected_resets_performance_state`
- `handler::tests::test_vm_disconnected_stops_monitoring`
- `handler::tests::test_performance_monitoring_started_stores_shutdown_tx`
- `handler::tests::test_memory_snapshot_ignored_for_unknown_session`
- `handler::tests::test_gc_event_ignored_for_unknown_session`

### Risks/Limitations

1. **Polling task not tracked in `session_tasks`**: The `spawn_performance_polling` task handle is discarded. If the engine shuts down abruptly the polling loop may run briefly past shutdown. Mitigation: the loop exits when `msg_tx` closes (detects engine shutdown), and the `perf_shutdown_tx` signal is sent by `VmServiceDisconnected` handler. A future improvement would track the handle in `session_tasks`.

2. **GC stream subscription**: GC events are only received if the VM Service `GC` stream is subscribed. Current `subscribe_flutter_streams()` subscribes to Extension and Logging streams. The GC stream subscription is handled in Task 03; this task only adds the parsing in the forwarding loop for when it IS subscribed.
