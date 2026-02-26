## Task: Reset Heartbeat Failure Counter on Reconnection Events

**Objective**: Reset `consecutive_failures` to 0 when `VmClientEvent::Reconnected` or `VmClientEvent::Reconnecting` is received, preventing stale failure counts from causing premature disconnection after a successful reconnection.

**Depends on**: None

**Review Reference**: Phase-3 Review Issues #1, #6

### Scope

- `crates/fdemon-app/src/actions.rs`: Modify `forward_vm_events` (~lines 1051-1064)

### Details

#### Problem

`consecutive_failures` is declared at line 992 and only reset to 0 on a successful heartbeat probe (`Ok(Ok(_))` at line 1092). During a WebSocket reconnection, the heartbeat `tokio::time::interval` continues to fire every 30 seconds. Each heartbeat during reconnection fails with `Error::ChannelClosed`, incrementing `consecutive_failures`.

When `VmClientEvent::Reconnected` arrives, the code forwards the event to the TEA handler but does **not** reset `consecutive_failures`. If 2 failures accumulated during reconnection and the first post-reconnect heartbeat encounters any transient issue, `consecutive_failures` hits 3 and the connection is terminated — even though it just reconnected successfully.

**Failure scenario timeline:**
```
t=0s    VM Service disconnects, reconnection begins
t=30s   heartbeat fires → ChannelClosed → consecutive_failures = 1
t=60s   heartbeat fires → ChannelClosed → consecutive_failures = 2
t=75s   VmClientEvent::Reconnected → consecutive_failures stays at 2
t=90s   heartbeat fires → transient timeout → consecutive_failures = 3 → DISCONNECT
```

The connection is killed 15 seconds after a successful reconnection.

#### Fix

Add `consecutive_failures = 0;` in two places:

**Fix 1 — `Reconnecting` arm** (line 1051):
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

Resetting on `Reconnecting` (the first event in a reconnection cycle) immediately clears any pre-disconnect failures, giving the reconnection window a clean slate.

**Fix 2 — `Reconnected` arm** (line 1060):
```rust
Some(VmClientEvent::Reconnected) => {
    consecutive_failures = 0;  // ← ADD: clean slate after successful reconnect
    let _ = msg_tx
        .send(Message::VmServiceReconnected { session_id })
        .await;
}
```

Resetting on `Reconnected` is the definitive reset — after a successful reconnect, any prior failures are irrelevant.

Both resets are needed: `Reconnecting` prevents accumulation during the backoff window; `Reconnected` handles the case where heartbeats raced before `Reconnecting` arrived.

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
3. After a reconnection cycle, the failure counter is provably 0 (verify via test or log inspection)
4. `cargo check --workspace` passes
5. `cargo clippy --workspace -- -D warnings` clean
6. `cargo test -p fdemon-app` passes

### Testing

No new test is strictly required — the heartbeat is an async runtime behavior inside `forward_vm_events` that cannot be unit tested without an integration harness. However, the fix can be verified by:

1. Reading the code to confirm both arms set `consecutive_failures = 0`
2. The existing `test_heartbeat_constants_are_reasonable` (line 2058) continues to pass
3. If desired, add a comment-only test documenting the expected behaviour:

```rust
#[test]
fn test_heartbeat_counter_reset_documented() {
    // The consecutive_failures counter in forward_vm_events is reset to 0 in:
    // 1. The Reconnecting arm (prevents accumulation during backoff)
    // 2. The Reconnected arm (clean slate after successful reconnect)
    // 3. The Ok(Ok(_)) heartbeat success arm (normal operation)
    //
    // This is an async integration concern that cannot be unit tested here.
    // Verified by code review and the phase-3b review checklist.
}
```

### Notes

- The `heartbeat_handle` (obtained at line 989 via `client.request_handle()`) survives reconnection because `VmRequestHandle` communicates through the `VmServiceClient`'s internal command channel, which is re-established during reconnection
- The heartbeat `tokio::time::interval` continues to tick during reconnection — this is intentional (it acts as a liveness check), but the counter must not carry over from the disconnected state
- Consider whether heartbeat probes should be skipped entirely during reconnection (e.g., using a `reconnecting: bool` flag). This is a future enhancement — for now, resetting the counter is sufficient
