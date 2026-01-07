## Task: Fix Channel Error Handling

**Objective**: Handle `event_tx.send()` failures properly instead of silently ignoring them, preventing tests from hanging or passing incorrectly.

**Depends on**: None (can be done independently)

**Priority**: Critical (logic issue identified in code review)

**Source**: [REVIEW.md](../../../REVIEW.md) - Logic Reasoning Review, Critical Issue #2

### Scope

- `tests/e2e/mock_daemon.rs`: Multiple locations with `let _ = self.event_tx.send(...)`

### Details

The current implementation silently ignores channel send failures:

```rust
// Current - silently ignores failures
let _ = self.event_tx.send(event).await;  // Line 155
let _ = self.event_tx.send(DaemonEvent::Stdout(...)).await;  // Lines 165-168, 280-283, 293-296
```

**Problem:** If the receiver is dropped (test ended or crashed), events are lost silently. This can cause:
- Tests passing incorrectly (events never received)
- Tests hanging indefinitely (waiting for events that were dropped)

**Fix Options:**

**Option A: Break on send failure (Recommended)**
```rust
// In run() event queue handling
if self.event_tx.send(event).await.is_err() {
    break;  // Receiver dropped, exit loop
}

// In helper methods, return Result or bool
async fn send_daemon_connected(&self) -> bool {
    let json = r#"{"event":"daemon.connected"...}"#;
    self.event_tx
        .send(DaemonEvent::Stdout(format!("[{}]", json)))
        .await
        .is_ok()
}
```

**Option B: Trace log on failure**
```rust
if self.event_tx.send(event).await.is_err() {
    eprintln!("[mock_daemon] Event channel closed, receiver dropped");
    break;
}
```

### Affected Locations

1. `run()` method, line ~155: Event queue send
2. `send_daemon_connected()`, lines ~165-168
3. `send_event()`, lines ~280-283
4. `send_response()`, lines ~293-296

### Acceptance Criteria

1. All `event_tx.send()` calls check for errors
2. Event loop breaks when send fails (receiver dropped)
3. No silent failures - either break or log
4. All existing tests pass
5. `cargo clippy --test e2e` passes with no new warnings

### Testing

```bash
cargo test --test e2e
cargo clippy --test e2e
```

Consider adding a test that verifies the daemon exits cleanly when the handle is dropped.

### Notes

- This is a test-only change, no production code affected
- Breaking on send failure is the safest approach for tests
- Logging can help debug test failures but adds noise
