## Task: Fix Heartbeat Failure Counter Reset on Reconnection Events

**Objective**: Reset `consecutive_failures` to 0 when `VmClientEvent::Reconnecting` or `VmClientEvent::Reconnected` is received in `forward_vm_events`, preventing stale failure counts from causing premature disconnection after a successful reconnect.

**Depends on**: None

### Scope

- `crates/fdemon-app/src/actions.rs`: Modify `forward_vm_events` (lines 1053-1066)

### Details

#### Problem

The Phase 3b task `01-reset-heartbeat-on-reconnect.md` was marked Done but the fix was **never applied to source code**. The completion summary (lines 118-141) falsely claims both arms were modified and a test was added.

`consecutive_failures` is declared at line 994 and only reset to 0 on a successful heartbeat probe (`Ok(Ok(_))` at line 1094). During a WebSocket reconnection, the heartbeat interval continues to fire every 30 seconds, and each probe fails with `ChannelClosed`, incrementing `consecutive_failures`.

When `VmClientEvent::Reconnected` arrives, the code forwards the event but does **not** reset the counter. If 2 failures accumulated during reconnection and the first post-reconnect heartbeat encounters any transient issue, `consecutive_failures` hits 3 and the connection is terminated.

**Failure timeline:**
```
t=0s    VM Service disconnects, reconnection begins
t=30s   heartbeat fires → ChannelClosed → consecutive_failures = 1
t=60s   heartbeat fires → ChannelClosed → consecutive_failures = 2
t=75s   VmClientEvent::Reconnected → consecutive_failures stays at 2
t=90s   heartbeat fires → transient timeout → consecutive_failures = 3 → DISCONNECT
```

#### Fix

Add `consecutive_failures = 0;` as the first statement in both match arms:

**Fix 1 — `Reconnecting` arm** (line 1053):
```rust
Some(VmClientEvent::Reconnecting { attempt, max_attempts }) => {
    consecutive_failures = 0;  // ← ADD: prevent accumulation during backoff
    let _ = msg_tx
        .send(Message::VmServiceReconnecting {
            session_id,
            attempt,
            max_attempts,
        })
        .await;
}
```

**Fix 2 — `Reconnected` arm** (line 1062):
```rust
Some(VmClientEvent::Reconnected) => {
    consecutive_failures = 0;  // ← ADD: clean slate after successful reconnect
    let _ = msg_tx
        .send(Message::VmServiceReconnected { session_id })
        .await;
}
```

#### State machine after fix

```
consecutive_failures state machine:
  ├── heartbeat tick fires
  │     ├── Ok(Ok(_))     → reset to 0
  │     ├── Ok(Err(e))    → +=1, break if >= 3
  │     └── Err(timeout)  → +=1, break if >= 3
  │
  ├── VmClientEvent::Reconnecting  → reset to 0  ← NEW
  ├── VmClientEvent::Reconnected   → reset to 0  ← NEW
  └── VmClientEvent::PermanentlyDisconnected → break
```

### Acceptance Criteria

1. `consecutive_failures = 0` is set in the `VmClientEvent::Reconnecting` arm
2. `consecutive_failures = 0` is set in the `VmClientEvent::Reconnected` arm
3. `cargo check --workspace` passes
4. `cargo clippy --workspace -- -D warnings` clean
5. `cargo test -p fdemon-app` passes

### Testing

Add a documentation test to record the invariant (same as originally specified in Phase 3b):

```rust
#[test]
fn test_heartbeat_counter_reset_on_reconnection() {
    // The consecutive_failures counter in forward_vm_events is reset to 0 in:
    // 1. The Reconnecting arm (prevents accumulation during backoff)
    // 2. The Reconnected arm (clean slate after successful reconnect)
    // 3. The Ok(Ok(_)) heartbeat success arm (normal operation)
    //
    // This is an async integration concern that cannot be unit tested here.
    // Verified by code review.
}
```

### Notes

- This is the same fix specified in Phase 3b `01-reset-heartbeat-on-reconnect.md` — it was never applied
- The fix is applied to the monolithic `actions.rs` before the refactoring begins (tasks 02-06)
- After task 03, this code will live in `actions/vm_service.rs`
