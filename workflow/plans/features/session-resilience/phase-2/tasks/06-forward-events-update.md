## Task: Update forward_vm_events to handle VmClientEvent

**Objective**: Update the `forward_vm_events` function in the app layer to handle the new `VmClientEvent` wrapper enum, translating lifecycle events into TEA `Message` variants that flow through the existing handler pipeline.

**Depends on**: 04-vm-client-event-type

### Scope

- `crates/fdemon-app/src/actions.rs`: Update `forward_vm_events` match logic

### Details

**Current state:** `forward_vm_events` (lines 943-1026) receives raw `VmServiceEvent` structs from `client.event_receiver().recv()` and pattern-matches on the event's `params.event` fields to translate them into `Message` variants. After Task 04 changes the channel type to `VmClientEvent`, this code no longer compiles because `event.params.event` doesn't exist on `VmClientEvent`.

**Change:** Wrap the existing event-handling logic in a `VmClientEvent::StreamEvent(event)` match arm, and add new arms for lifecycle events.

#### Current code structure (actions.rs ~lines 950-1025)

```rust
loop {
    tokio::select! {
        event = client.event_receiver().recv() => {
            match event {
                Some(event) => {
                    // event is VmServiceEvent — access event.params.event
                    if let Some(flutter_error) = parse_flutter_error(&event.params.event) {
                        // ...
                        continue;
                    }
                    if let Some(timing) = parse_frame_timing(&event.params.event) {
                        // ...
                        continue;
                    }
                    // ... more parsers ...
                }
                None => break, // channel closed
            }
        }
        Ok(_) = vm_shutdown_rx.changed(), if *vm_shutdown_rx.borrow() => {
            client.disconnect().await;
            break;
        }
    }
}
// After loop: send VmServiceDisconnected
```

#### Updated code structure

```rust
loop {
    tokio::select! {
        event = client.event_receiver().recv() => {
            match event {
                Some(VmClientEvent::StreamEvent(event)) => {
                    // Existing logic unchanged — event is VmServiceEvent
                    if let Some(flutter_error) = parse_flutter_error(&event.params.event) {
                        // ... (unchanged)
                        continue;
                    }
                    if let Some(timing) = parse_frame_timing(&event.params.event) {
                        // ... (unchanged)
                        continue;
                    }
                    // ... more parsers (unchanged) ...
                }
                Some(VmClientEvent::Reconnecting { attempt, max_attempts }) => {
                    let _ = msg_tx.send(Message::VmServiceReconnecting {
                        session_id,
                        attempt,
                        max_attempts,
                    }).await;
                }
                Some(VmClientEvent::Reconnected) => {
                    let _ = msg_tx.send(Message::VmServiceConnected {
                        session_id,
                    }).await;
                }
                Some(VmClientEvent::PermanentlyDisconnected) => {
                    break; // Fall through to VmServiceDisconnected below
                }
                None => break, // Channel closed — same behavior as before
            }
        }
        Ok(_) = vm_shutdown_rx.changed(), if *vm_shutdown_rx.borrow() => {
            client.disconnect().await;
            break;
        }
    }
}
// After loop: send VmServiceDisconnected (unchanged)
let _ = msg_tx.send(Message::VmServiceDisconnected { session_id }).await;
```

#### Import addition

Add `VmClientEvent` to the imports from `fdemon_daemon::vm_service` (line ~18-25):

```rust
use fdemon_daemon::vm_service::{
    ..., VmClientEvent, ...
};
```

#### Key design decisions

**1. `Reconnected` → `Message::VmServiceConnected`**

When the daemon reconnects, we reuse the existing `VmServiceConnected` message. This is correct because the `VmServiceConnected` handler (update.rs:1179-1254):
- Sets `vm_connected = true`
- Resets `DevToolsViewState.connection_status = VmConnectionStatus::Connected`
- Resets `PerformanceState` to fresh
- Triggers `StartPerformanceMonitoring` (re-starts perf polling)
- Triggers inspector widget tree fetch if in DevTools/Inspector mode

All of these are exactly what should happen after a reconnection. The handler already handles being called multiple times for the same session (idempotent).

**However**, there is one important difference: on initial connection, `spawn_vm_service_connection` also sends `VmServiceHandleReady` (with the request handle) and `VmServiceAttached` (with the shutdown sender) before sending `VmServiceConnected`. After a reconnection, the request handle and shutdown sender are still valid (they use the command channel, which persists across reconnects). So we only need to send `VmServiceConnected` — the handle and shutdown sender don't need re-attaching.

**2. `PermanentlyDisconnected` → break (not explicit message)**

When the daemon gives up reconnecting, we break out of the loop. This falls through to the existing `VmServiceDisconnected` send after the loop, which triggers the standard disconnect cleanup (clear handles, abort polling tasks, etc.). This reuses the existing disconnect logic without duplication.

**3. `None` (channel closed) → break (unchanged)**

When `event_tx` is dropped (background task exits), `recv()` returns `None`. This continues to trigger the same `VmServiceDisconnected` flow. In practice, `PermanentlyDisconnected` is sent just before the task exits, so the consumer sees: `PermanentlyDisconnected` → break → `VmServiceDisconnected`. If the channel closes before `PermanentlyDisconnected` is received (race), the `None` → break path catches it.

### Acceptance Criteria

1. `forward_vm_events` matches on `VmClientEvent::StreamEvent(event)` for all existing stream event handling
2. `VmClientEvent::Reconnecting` sends `Message::VmServiceReconnecting { session_id, attempt, max_attempts }`
3. `VmClientEvent::Reconnected` sends `Message::VmServiceConnected { session_id }`
4. `VmClientEvent::PermanentlyDisconnected` breaks out of the loop (triggers `VmServiceDisconnected`)
5. `VmClientEvent` is imported from `fdemon_daemon::vm_service`
6. All existing stream event handling logic is unchanged (just wrapped in `StreamEvent` arm)
7. `cargo check --workspace` passes
8. `cargo test --workspace` passes

### Testing

Covered by Task 07. No test changes in this task.

The changes here are structural (match arm wrapping) plus three new one-liner translations. The translations delegate to existing, tested message handlers — no new logic is introduced.

### Notes

- After this task, the full pipeline is live IF Task 05 is also complete. The daemon emits lifecycle events → `forward_vm_events` translates them → existing handlers update state → existing TUI renders status. No handler or TUI changes needed.
- `msg_tx.send(...).await` (async, blocking until space) is used for lifecycle messages, matching the pattern used for stream event translations in the same function. The `let _ =` discard is acceptable because if `msg_tx` is closed, the engine is shutting down.
- The `Reconnected` → `VmServiceConnected` reuse means performance monitoring and inspector state are automatically re-initialized after reconnection. This is correct because `resubscribe_streams` in the daemon already re-subscribes to Extension/Logging/GC streams after reconnect.
- If the connection drops again after reconnection (second disconnect), the daemon re-enters the reconnection loop and sends `Reconnecting` again. `forward_vm_events` handles this naturally — it stays in the select loop as long as events arrive.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/actions.rs` | Added `VmClientEvent` to `fdemon_daemon::vm_service` imports; wrapped `Some(event)` arm as `Some(VmClientEvent::StreamEvent(event))`; added three new match arms for `Reconnecting`, `Reconnected`, and `PermanentlyDisconnected` |

### Notable Decisions/Tradeoffs

1. **`Reconnected` reuses `Message::VmServiceConnected`**: Matches the design decision in the task — the existing handler already does the right thing (resets DevToolsViewState, restarts perf monitoring, re-fetches inspector tree). No new handler code needed.
2. **`PermanentlyDisconnected` → break (no explicit message)**: Falls through to the `VmServiceDisconnected` send after the loop, reusing the existing disconnect cleanup path without duplication.
3. **`let _ =` discards on send**: Consistent with the existing pattern throughout the function — if `msg_tx` is closed, the engine is shutting down and the send failure is acceptable.

### Testing Performed

- `cargo check --workspace` - Passed
- `cargo test --workspace` - Passed (all crates, no failures)
- `cargo fmt --all --check` - Passed
- `cargo clippy --workspace -- -D warnings` - Passed

### Risks/Limitations

1. **No new unit tests in this task**: Task 07 covers testing. The structural change (match arm wrapping) is minimal and the new arms delegate entirely to existing tested message handlers.
