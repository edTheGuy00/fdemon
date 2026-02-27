## Task: Emit lifecycle events from run_client_task reconnection loop

**Objective**: Send `VmClientEvent::Reconnecting`, `VmClientEvent::Reconnected`, and `VmClientEvent::PermanentlyDisconnected` from the VM Service client's background reconnection task at the correct state transitions.

**Depends on**: 04-vm-client-event-type

### Scope

- `crates/fdemon-daemon/src/vm_service/client.rs`: Add event emissions in `run_client_task`

### Details

**Current state:** `run_client_task` handles reconnection internally — it transitions `ConnectionState` through `Connecting → Connected → Reconnecting { attempt } → Connected/Disconnected` — but these transitions are only written to a shared `Arc<RwLock<ConnectionState>>`. No events are sent through the event channel to inform consumers.

**Change:** At each state transition in the reconnection loop, send the corresponding `VmClientEvent` through `event_tx`. This lets `forward_vm_events` (in `fdemon-app`) translate them into `Message` variants for the TEA pipeline.

#### Reconnection loop code flow (client.rs lines 582-674)

The reconnection loop in `run_client_task` has this structure (simplified):

```
fn run_client_task(..., event_tx, state, ...) {
    // Initial connection — run_io_loop returns true if reconnect needed
    let reconnect = run_io_loop(ws_stream, ...).await;
    if !reconnect {
        *state.write() = Disconnected;        // ← EMIT PermanentlyDisconnected here (clean shutdown)
        return;
    }

    for attempt in 1..=MAX_RECONNECT_ATTEMPTS {
        *state.write() = Reconnecting { attempt };  // ← EMIT Reconnecting here
        sleep(compute_backoff(attempt)).await;

        if cmd_rx.is_closed() {
            *state.write() = Disconnected;           // ← EMIT PermanentlyDisconnected here
            break;
        }

        match connect_ws(&ws_uri).await {
            Ok(ws_stream) => {
                *state.write() = Connected;          // ← EMIT Reconnected here
                // Re-enter IO loop
                let reconnect = run_io_loop(ws_stream, ..., resubscribe=true).await;
                if !reconnect {
                    *state.write() = Disconnected;   // ← EMIT PermanentlyDisconnected here
                    return;
                }
                // Connection lost again — loop continues
            }
            Err(_) => {
                // Attempt failed — loop continues to next attempt
            }
        }
    }
    // All attempts exhausted
    *state.write() = Disconnected;                   // ← EMIT PermanentlyDisconnected here
}
```

#### Emission points (5 sites in run_client_task)

**1. Reconnecting — at each retry attempt (inside the for loop, after state transition)**

Find the line where `*state.write().unwrap() = ConnectionState::Reconnecting { attempt }` is set (approximately line 617). Add immediately after:

```rust
let _ = event_tx.try_send(VmClientEvent::Reconnecting {
    attempt,
    max_attempts: MAX_RECONNECT_ATTEMPTS,
});
```

**2. Reconnected — after successful reconnection (inside the Ok(ws_stream) branch)**

Find the line where `*state.write().unwrap() = ConnectionState::Connected` is set after a successful reconnect (approximately line 641). Add immediately after:

```rust
let _ = event_tx.try_send(VmClientEvent::Reconnected);
```

**3-5. PermanentlyDisconnected — at all terminal disconnect points**

There are three places where the function transitions to `Disconnected` and exits/breaks:

a. **Clean shutdown after initial IO loop** (~line 598): `run_io_loop` returned `false` (client requested disconnect). This is NOT a reconnection failure — it's a clean shutdown. Do NOT emit `PermanentlyDisconnected` here. The event channel closing (dropping `event_tx`) is sufficient to signal the consumer.

b. **Command channel closed during backoff** (~line 611): `cmd_rx.is_closed()` detected — the `VmServiceClient` was dropped. Also a clean shutdown — no emission needed.

c. **All attempts exhausted** (~line 657, after the for loop ends): This IS a reconnection failure. Emit:
```rust
let _ = event_tx.try_send(VmClientEvent::PermanentlyDisconnected);
```

d. **Clean shutdown after reconnected IO loop** (~line 631): `run_io_loop` returned `false` after a reconnection — client requested disconnect. Clean shutdown — no emission.

**Revised emission strategy:** Only emit `PermanentlyDisconnected` when all reconnection attempts are exhausted (site c). Clean shutdowns (disconnect requested) are signaled by the channel closing naturally (`event_tx` dropped → `event_rx.recv()` returns `None`).

This means only 3 active emission sites:
1. `Reconnecting` — on each retry attempt
2. `Reconnected` — on successful reconnect
3. `PermanentlyDisconnected` — only when max attempts exhausted

#### Using try_send (not send)

Use `try_send` (non-blocking) for all lifecycle emissions, matching the existing stream event pattern at line 808-814. The `let _ =` discard is acceptable because:
- The channel has capacity 256 and lifecycle events are rare (~10 max)
- If the consumer has dropped, we're shutting down anyway
- Blocking on `send().await` could delay the reconnection loop

### Acceptance Criteria

1. `VmClientEvent::Reconnecting { attempt, max_attempts }` is sent at each reconnection attempt (before the backoff sleep)
2. `VmClientEvent::Reconnected` is sent after a successful reconnection (after `ConnectionState::Connected` is set)
3. `VmClientEvent::PermanentlyDisconnected` is sent when all reconnection attempts are exhausted
4. Clean shutdowns (disconnect requested) do NOT emit `PermanentlyDisconnected` — the channel close is sufficient
5. All emissions use `try_send` (non-blocking), matching the existing stream event pattern
6. `cargo check -p fdemon-daemon` passes
7. `cargo test -p fdemon-daemon` passes

### Testing

Covered by Task 07. No test changes in this task.

Testing lifecycle emissions requires an integration test with a mock WebSocket server that can simulate disconnection and reconnection. The existing daemon test suite does not have this infrastructure. Task 07 will add handler-level tests that verify the downstream effects.

### Notes

- The `Reconnecting` event should be sent AFTER setting `ConnectionState` but BEFORE the backoff sleep. This way the consumer sees the event immediately when the reconnection attempt starts, not after the delay.
- `attempt` is 1-based (first retry = 1), matching `ConnectionState::Reconnecting { attempt }`.
- `max_attempts` comes from `MAX_RECONNECT_ATTEMPTS` (now `pub const` from Task 04).
- If the IO loop fails again after a successful reconnection (connection drops a second time), the for loop continues with the next attempt number. The `Reconnecting` event is sent again with the new attempt number.
- The `info!` log messages at each state transition (already present) remain unchanged — the events supplement, not replace, the logs.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/vm_service/client.rs` | Added 3 `try_send` event emissions in `run_client_task`: `Reconnecting` after state transition (before backoff sleep), `Reconnected` after successful reconnect, `PermanentlyDisconnected` only when max attempts exhausted |

### Notable Decisions/Tradeoffs

1. **Emission ordering for `Reconnecting`**: The event is sent after the `ConnectionState::Reconnecting { attempt }` write but before the `tokio::time::sleep(backoff)` call, matching the task spec. This ensures consumers see the event immediately at the start of each attempt rather than after the delay.

2. **`PermanentlyDisconnected` only at exhaustion**: The event is emitted only in the `if attempt > MAX_RECONNECT_ATTEMPTS { ... break }` branch. The two clean-shutdown `break` paths (`cmd_rx.is_closed()` and `run_io_loop` returning `false` after reconnection) do not emit this event — the channel closing naturally is sufficient for consumers to detect clean shutdowns.

3. **`try_send` with `let _ =`**: Matches the existing pattern for stream event forwarding at the bottom of the file. Non-blocking and discards send errors, appropriate since the channel has capacity 256 and lifecycle events are rare.

### Testing Performed

- `cargo check -p fdemon-daemon` - Passed
- `cargo test -p fdemon-daemon` - Passed (375 passed, 3 ignored)
- `cargo fmt --all --check` - Passed
- `cargo clippy -p fdemon-daemon -- -D warnings` - Passed

### Risks/Limitations

1. **No dedicated tests for emission logic**: As specified in the task, lifecycle emission tests are deferred to Task 07, which will add handler-level integration tests. The existing unit tests cover the surrounding code paths but not the channel sends directly.
