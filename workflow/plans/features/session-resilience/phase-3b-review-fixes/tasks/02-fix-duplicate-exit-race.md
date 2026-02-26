## Task: Fix Duplicate Exited Event Race Between Watchdog and wait_for_exit

**Objective**: Prevent the watchdog arm from synthesizing a second `DaemonEvent::Exited` when the real exit event from `wait_for_exit` has already been received and forwarded.

**Depends on**: None

**Review Reference**: Phase-3 Review Issue #2

### Scope

- `crates/fdemon-app/src/actions.rs`: Modify `spawn_session` (~lines 476-494, watchdog arm)

### Details

#### Problem

The `spawn_session` event loop has a race condition between the `daemon_rx.recv()` arm and the `watchdog.tick()` arm:

1. `wait_for_exit` sets `exited = true` (AtomicBool, line `process.rs:165`) BEFORE sending `DaemonEvent::Exited { code: Some(N) }` (line `process.rs:169`)
2. `daemon_rx.recv()` arm picks up the real `Exited` event, sets `process_exited = true` (line 436), forwards it to the TEA layer — but does NOT `break` the loop
3. The loop continues to the next iteration. The watchdog may tick before `daemon_rx` returns `None` (channel closure)
4. Watchdog sees `process.has_exited() == true` and synthesizes a SECOND `DaemonEvent::Exited { code: None }` (line 489)
5. TEA handler receives two exit messages. The second one overwrites the real exit code (`Some(N)`) with `None` and adds a duplicate log entry

**Race window:** Between step 2 (forwarding the real `Exited`) and the next `daemon_rx.recv()` returning `None` (channel closure). If the watchdog ticks during this window (up to 5 seconds), the duplicate fires.

#### Current code (watchdog arm, lines 476-494)

```rust
_ = watchdog.tick() => {
    if process.has_exited() {
        info!(
            "Watchdog detected process exit for session {}",
            session_id
        );
        let _ = msg_tx_clone
            .send(Message::SessionDaemon {
                session_id,
                event: DaemonEvent::Exited { code: None },
            })
            .await;
        process_exited = true;
        break;
    }
}
```

#### Fix

Add a `process_exited` guard to the watchdog arm:

```rust
_ = watchdog.tick() => {
    if !process_exited && process.has_exited() {
        info!(
            "Watchdog detected process exit for session {}",
            session_id
        );
        let _ = msg_tx_clone
            .send(Message::SessionDaemon {
                session_id,
                event: DaemonEvent::Exited { code: None },
            })
            .await;
        process_exited = true;
        break;
    }
}
```

The `!process_exited` check ensures the watchdog only synthesizes an `Exited` event if the `daemon_rx` arm hasn't already received and forwarded the real one. Since `process_exited` is set to `true` at line 436 when `DaemonEvent::Exited` arrives via the channel, the watchdog will see `process_exited == true` and skip the synthesis.

#### Why not break immediately on `Exited` in the `daemon_rx` arm?

An alternative fix would be to `break` the loop after forwarding the `Exited` event in the `daemon_rx` arm. This would also prevent the watchdog from firing. However, it would change the loop's exit behaviour:
- Currently, after `Exited`, the loop continues and picks up `daemon_rx` returning `None`, which is a clean channel-closure exit
- Breaking immediately would skip any pending events in the channel between `Exited` and closure

The guard approach is simpler and preserves the existing flow. It's also explicit about the intent: "the watchdog is a backup, not a duplicate."

### Acceptance Criteria

1. The watchdog arm checks `!process_exited` before calling `process.has_exited()`
2. When `wait_for_exit` sends a real `Exited` event AND the watchdog ticks before channel closure, only one `Exited` reaches the TEA handler
3. The real exit code (`Some(N)`) is preserved — the watchdog's `code: None` never overwrites it
4. The watchdog still correctly detects orphaned process death (when no channel `Exited` event arrives)
5. `cargo check --workspace` passes
6. `cargo clippy --workspace -- -D warnings` clean
7. `cargo test -p fdemon-app` passes

### Testing

This is an async race condition inside `tokio::spawn` that is difficult to unit test deterministically. Verification is primarily through code review:

1. Confirm the `!process_exited` guard is present in the watchdog arm
2. Trace the `process_exited` flag through all paths:
   - `daemon_rx` receives `Exited` → `process_exited = true` (line 436) → watchdog guard blocks
   - `daemon_rx` returns `None` → `process_exited = true` + `break` (line 463-464) → loop ends
   - Watchdog detects death first → `process_exited = true` + `break` (line 492-493) → loop ends

Task 03 will add a double-exit idempotency test at the TEA handler level as defense-in-depth.

### Notes

- This was explicitly called out as a phase-3 success criterion: "No duplicate `Exited` events when watchdog and wait task race" — the current code violates this criterion
- The `process_exited` flag already exists at line 418 and is used post-loop (line 500) to decide whether to call `process.shutdown()`. This fix leverages the existing flag rather than introducing new state
- The fix is a single-line addition (`!process_exited &&`) with no structural changes to the loop
