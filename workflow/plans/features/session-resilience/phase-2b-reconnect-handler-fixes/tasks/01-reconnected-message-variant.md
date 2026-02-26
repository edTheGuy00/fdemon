## Task: Add VmServiceReconnected Message Variant

**Objective**: Introduce a `Message::VmServiceReconnected` variant that distinguishes WebSocket reconnection from initial connection. Map `VmClientEvent::Reconnected` to this new variant instead of `VmServiceConnected`. Create a handler that preserves accumulated `PerformanceState` and emits a distinct log message.

**Depends on**: None

**Review Reference**: Phase-2 Review Issue #2

### Scope

- `crates/fdemon-app/src/message.rs`: Add `VmServiceReconnected { session_id: SessionId }` variant
- `crates/fdemon-app/src/actions.rs`: Change `VmClientEvent::Reconnected` mapping from `VmServiceConnected` to `VmServiceReconnected`
- `crates/fdemon-app/src/handler/update.rs`: Add `VmServiceReconnected` match arm

### Details

#### Problem

`VmClientEvent::Reconnected` (a brief WebSocket reconnect after backoff) is mapped to `Message::VmServiceConnected` at `actions.rs:1015-1018`. The `VmServiceConnected` handler unconditionally resets `PerformanceState` at `update.rs:1194-1198`, wiping all accumulated telemetry:

| Lost Data | Type | Impact |
|-----------|------|--------|
| `memory_history` | `RingBuffer<MemoryUsage>` | ~2 minutes of rolling memory snapshots |
| `gc_history` | `RingBuffer<GcEvent>` | Up to 50 GC events |
| `frame_history` | `RingBuffer<FrameTiming>` | Up to 300 frame timings |
| `stats` | `PerformanceStats` | Aggregated FPS, jank count, percentiles |
| `memory_samples` | `RingBuffer<MemorySample>` | 60 seconds of time-series memory data |
| `allocation_profile` | `Option<AllocationProfile>` | Most recent class allocation snapshot |

The handler also logs "VM Service connected — enhanced logging active" with no indication it was a reconnection.

#### Fix

**1. New Message variant** (`message.rs`):
```rust
/// VM Service WebSocket successfully reconnected after a brief disconnect.
/// Unlike VmServiceConnected, this preserves accumulated performance state.
VmServiceReconnected { session_id: SessionId },
```

**2. Update mapping** (`actions.rs:1015-1018`):
```rust
Some(VmClientEvent::Reconnected) => {
    let _ = msg_tx
        .send(Message::VmServiceReconnected { session_id })
        .await;
}
```

**3. New handler** (`update.rs`, new match arm):
```rust
Message::VmServiceReconnected { session_id } => {
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        handle.session.vm_connected = true;
        handle.session.add_log(fdemon_core::LogEntry::info(
            LogSource::App,
            "VM Service reconnected — resuming monitoring",
        ));
        // DO NOT reset PerformanceState — preserve accumulated telemetry
    }

    // Update connection status (guarded — see task 03)
    let active_id = state.session_manager.selected().map(|h| h.session.id);
    if active_id == Some(session_id) {
        state.devtools_view_state.vm_connection_error = None;
        state.devtools_view_state.connection_status =
            crate::state::VmConnectionStatus::Connected;
    }

    // Re-subscribe to VM streams and restart performance monitoring
    // (same follow-up actions as VmServiceConnected)
    UpdateResult::with_action(UpdateAction::StartPerformanceMonitoring { session_id })
}
```

#### Key design decision

The new handler still dispatches `StartPerformanceMonitoring` because the old WebSocket connection's stream subscriptions are gone — the VM Service requires re-subscription after reconnection. But it does NOT reset the ring buffers, so historical data is preserved and new samples append to the existing history.

### Acceptance Criteria

1. `Message::VmServiceReconnected` variant exists in `message.rs`
2. `VmClientEvent::Reconnected` maps to `VmServiceReconnected` (not `VmServiceConnected`)
3. `VmServiceReconnected` handler does NOT reset `PerformanceState`
4. Log message says "reconnected" (not "connected")
5. Handler updates `connection_status` to `Connected` (with active-session guard)
6. Handler re-dispatches stream resubscription and performance monitoring
7. `VmServiceConnected` handler continues to work for initial connections
8. `cargo check --workspace` passes
9. `cargo clippy --workspace -- -D warnings` clean

### Notes

- The `VmServiceConnected` handler's comment at `update.rs:1194` says "reset on hot-restart" — that behavior should remain for initial connections. Only the reconnect path changes.
- The handler still needs to dispatch `StartPerformanceMonitoring` because VM stream subscriptions are lost on WebSocket disconnect. Task 02 handles cleaning up the old polling task before this dispatch.
- Consider whether `ResubscribeStreams` should also be dispatched (check if `VmServiceConnected` does this).
