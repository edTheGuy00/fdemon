## Task: Add Process Watchdog to spawn_session Event Loop

**Objective**: Detect process death that doesn't produce a stdout EOF (e.g., SIGKILL, OOM kill) by periodically polling `process.has_exited()` from the `spawn_session` event loop.

**Depends on**: None

### Scope

- `crates/fdemon-app/src/actions.rs`: Add a third `tokio::select!` arm to the loop in `spawn_session` (~line 410)

### Details

#### Problem

The only exit signal today is stdout EOF, which emits `DaemonEvent::Exited { code: None }`. If a Flutter process is killed externally (SIGKILL, OOM killer, frozen pipe) without closing stdout cleanly, the `daemon_rx.recv()` arm blocks indefinitely. The session appears "Running" forever.

#### Fix

Add a `tokio::time::interval` arm to the existing `tokio::select!` loop inside `spawn_session` (line 409). On each tick, poll `process.has_exited()`. If the process has died, synthesize an `Exited` event and break the loop.

**Location**: Inside the `tokio::spawn(async move { ... })` block in `spawn_session`, within the `loop { tokio::select! { ... } }` at line 409.

**Current structure** (2 arms):
```rust
loop {
    tokio::select! {
        event = daemon_rx.recv() => { ... }
        _ = shutdown_rx_clone.changed() => { ... }
    }
}
```

**New structure** (3 arms):
```rust
let mut watchdog = tokio::time::interval(Duration::from_secs(5));
watchdog.tick().await; // consume the immediate first tick

loop {
    tokio::select! {
        event = daemon_rx.recv() => { ... }     // existing — unchanged
        _ = shutdown_rx_clone.changed() => { ... }  // existing — unchanged
        _ = watchdog.tick() => {
            if process.has_exited() {
                info!(
                    "Watchdog detected process exit for session {}",
                    session_id
                );
                // Synthesize exit event to the TEA layer
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
    }
}
```

#### Key Design Decisions

1. **5-second interval**: Balances responsiveness (user waits at most 5s to learn of death) vs CPU overhead (one `try_wait()` syscall every 5s is negligible).

2. **Consume first tick**: `tokio::time::interval` fires immediately on creation. The first tick would be a redundant check at process startup. Consuming it before the loop avoids this.

3. **`process.has_exited()` requires `&mut self`**: This is safe because `process` is exclusively owned by this task. No other task holds a reference to it.

4. **Duplicate `Exited` guard**: If stdout EOF races with the watchdog, both may try to emit `Exited`. The existing `process_exited` flag handles this — the first one wins, and the TEA handler at `handle_session_exited` is idempotent for the `AppPhase::Stopped` transition. Additionally, the watchdog `break`s immediately after sending, so no duplicate from this arm.

5. **`code: None` for now**: The watchdog emits `code: None` because `has_exited()` uses `try_wait()` which consumes the status internally via `matches!()` without capturing it. Task 04 (`wait-for-exit-task`) will add proper exit code capture; for now, the watchdog correctly detects death but cannot report the code.

#### Import Addition

Add `use std::time::Duration;` to the top of the `spawn_session` closure (or at file level if not already present). Also ensure `tokio::time` is imported.

### Acceptance Criteria

1. A `tokio::time::interval(5s)` arm exists in the `spawn_session` select loop
2. When the Flutter process is killed externally (no stdout EOF), the watchdog detects it within 5 seconds
3. A `DaemonEvent::Exited { code: None }` message is forwarded to the TEA layer
4. The `process_exited` flag is set to `true`, enabling the fast shutdown path
5. The watchdog does NOT fire during normal operation (process alive = no-op)
6. No duplicate `Exited` messages when stdout EOF and watchdog race
7. `cargo check --workspace` passes
8. `cargo clippy --workspace -- -D warnings` clean

### Testing

Add tests in `crates/fdemon-app/src/handler/tests.rs` or a new `actions_tests.rs`:

```rust
#[test]
fn test_watchdog_interval_constant() {
    // Verify the watchdog uses the expected 5s interval
    // (implementation detail test — optional)
}
```

Note: The watchdog is an async runtime behavior inside a `tokio::spawn`, making it difficult to unit test directly. The primary verification will be:
- `cargo check` confirms compilation
- Manual testing: start fdemon, `kill -9` the Flutter process, verify session shows "exited" within ~5s
- The existing `handle_session_exited` test coverage applies to the downstream handling

### Notes

- The watchdog is a safety net, not the primary exit path. Normal exits still go through stdout EOF -> `DaemonEvent::Exited`
- The `process` variable is already `mut` in the closure (needed for `shutdown()`), so adding `has_exited()` calls is free
- Consider making the interval a named constant: `const PROCESS_WATCHDOG_INTERVAL: Duration = Duration::from_secs(5);`
- Phase 3 task 04 (`wait-for-exit-task`) will improve this to capture actual exit codes

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/actions.rs` | Added `PROCESS_WATCHDOG_INTERVAL` constant; added watchdog `tokio::time::interval` initialisation (with first-tick consumed) before the `spawn_session` event loop; added third `tokio::select!` arm that polls `process.has_exited()` on each tick, synthesises `DaemonEvent::Exited { code: None }`, sets `process_exited = true`, and breaks |

### Notable Decisions/Tradeoffs

1. **`std::time::Duration` already imported**: The file already had `use std::time::Duration;` at line 6, so no new import was needed.
2. **Constant placement**: `PROCESS_WATCHDOG_INTERVAL` was placed directly below `PERF_POLL_MIN_MS` at the module level, keeping timing constants together.
3. **First tick consumed outside the loop**: `watchdog.tick().await` is called once before entering `loop { tokio::select! { ... } }` to avoid a spurious check at process startup, per the task design notes.
4. **No duplicate Exited guard needed in the watchdog arm itself**: The `process_exited = true` + `break` immediately after sending prevents any further watchdog ticks from re-firing. The existing `process_exited` flag in the normal EOF arm handles the race with the watchdog.

### Testing Performed

- `cargo check -p fdemon-app` - Passed
- `cargo clippy -p fdemon-app -- -D warnings` - Passed (0 warnings)
- `cargo test -p fdemon-app` - Passed (1136 unit tests + 1 doc test, 0 failures)

### Risks/Limitations

1. **`code: None` from watchdog**: The watchdog emits `Exited { code: None }` because `has_exited()` uses `try_wait()` internally which does not surface the exit code. Task 04 (`wait-for-exit-task`) will improve this.
2. **5-second worst-case detection latency**: By design. A killed process may appear "Running" for up to 5 seconds before the watchdog detects it. This is an acceptable trade-off per the task design.
