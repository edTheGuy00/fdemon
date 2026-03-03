## Task: Add warn! Logging to Lifecycle Event try_send Failures

**Objective**: Add `warn!` logging when lifecycle event `try_send` calls fail, matching the existing pattern used for stream events. This is an observability regression — `PermanentlyDisconnected` in particular is a terminal event that, if dropped silently, leaves the UI stuck on "Reconnecting" indefinitely.

**Depends on**: None

**Review Reference**: Phase-2 Review Issue #1

### Scope

- `crates/fdemon-daemon/src/vm_service/client.rs`: Lines 612, 620, 647

### Details

Three `let _ = event_tx.try_send(VmClientEvent::...)` calls silently discard errors, while the existing stream event handler at line 815 logs a `warn!` on send failure.

**Current code (lines 612, 620, 647):**
```rust
let _ = event_tx.try_send(VmClientEvent::PermanentlyDisconnected);
let _ = event_tx.try_send(VmClientEvent::Reconnecting { attempt, max_attempts: MAX_RECONNECT_ATTEMPTS });
let _ = event_tx.try_send(VmClientEvent::Reconnected);
```

**Target pattern (matching line 815):**
```rust
if let Err(e) = event_tx.try_send(VmClientEvent::PermanentlyDisconnected) {
    warn!("VM Service: failed to deliver PermanentlyDisconnected event: {}", e);
}
```

Apply the same `if let Err(e)` + `warn!` pattern to all three lifecycle event sends.

### Acceptance Criteria

1. All three lifecycle `try_send` calls log a `warn!` on failure
2. Log messages include the event variant name for debugging
3. Matches the existing stream event precedent at line 815
4. `cargo clippy --workspace -- -D warnings` clean

### Notes

- This is a quick fix — only changes `let _ =` to `if let Err(e) =` with a `warn!` macro call
- The `PermanentlyDisconnected` case is the most important — if dropped, the UI stays stuck on "Reconnecting"
