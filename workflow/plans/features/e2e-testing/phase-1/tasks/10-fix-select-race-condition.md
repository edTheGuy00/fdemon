## Task: Fix tokio::select! Race Condition

**Objective**: Fix the `else => break` branch in `tokio::select!` that may cause premature exit when the event queue is temporarily empty.

**Depends on**: None (can be done independently)

**Priority**: Critical (logic issue identified in code review)

**Source**: [REVIEW.md](../../../REVIEW.md) - Logic Reasoning Review, Critical Issue #1

### Scope

- `tests/e2e/mock_daemon.rs`: `run()` method, lines ~131-159

### Details

The current `tokio::select!` implementation has a potential race condition:

```rust
// Current implementation
loop {
    tokio::select! {
        Some(cmd) = self.cmd_rx.recv() => { ... }
        Some(ctrl) = self.control_rx.recv() => { ... }
        _ = tokio::time::sleep(Duration::from_millis(10)), if !self.event_queue.is_empty() => {
            let event = self.event_queue.remove(0);
            let _ = self.event_tx.send(event).await;
        }
        else => break,  // Problem: exits when all channels return None
    }
}
```

**Problem:** The `else` branch triggers when:
1. `cmd_rx.recv()` returns `None` (sender dropped), AND
2. `control_rx.recv()` returns `None` (sender dropped), AND
3. The sleep branch is disabled (queue empty)

This can cause premature exit if the event queue is temporarily empty but the test hasn't finished sending commands yet.

**Fix: Add explicit channel-closed tracking**

```rust
pub async fn run(mut self) {
    self.send_daemon_connected().await;

    let mut cmd_closed = false;
    let mut ctrl_closed = false;

    loop {
        tokio::select! {
            biased;  // Prioritize command handling

            cmd = self.cmd_rx.recv(), if !cmd_closed => {
                match cmd {
                    Some(cmd) => {
                        if !self.handle_command(&cmd).await {
                            break;
                        }
                    }
                    None => cmd_closed = true,
                }
            }

            ctrl = self.control_rx.recv(), if !ctrl_closed => {
                match ctrl {
                    Some(ctrl) => {
                        match ctrl {
                            MockControl::SetResponse { method, response } => {
                                self.responses.insert(method, response);
                            }
                            MockControl::QueueEvent(event) => {
                                self.event_queue.push_back(event);
                            }
                            MockControl::Shutdown => break,
                        }
                    }
                    None => ctrl_closed = true,
                }
            }

            _ = tokio::time::sleep(Duration::from_millis(10)), if !self.event_queue.is_empty() => {
                if let Some(event) = self.event_queue.pop_front() {
                    if self.event_tx.send(event).await.is_err() {
                        break;
                    }
                }
            }

            // Only exit when BOTH channels are closed AND queue is empty
            else => {
                if cmd_closed && ctrl_closed && self.event_queue.is_empty() {
                    break;
                }
                // Otherwise, yield and continue
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        }
    }
}
```

### Alternative: Simpler approach

If the full fix is too invasive, a simpler approach:

```rust
else => {
    // Don't break immediately - wait briefly for more work
    tokio::time::sleep(Duration::from_millis(50)).await;
    // Only break if channels are truly closed
    if self.cmd_rx.is_closed() && self.control_rx.is_closed() {
        break;
    }
}
```

### Acceptance Criteria

1. Event loop doesn't exit prematurely when queue is temporarily empty
2. Event loop exits cleanly when all channels are closed
3. No busy-waiting (must have sleep/yield)
4. All existing tests pass
5. `cargo clippy --test e2e` passes

### Testing

```bash
cargo test --test e2e
cargo clippy --test e2e
```

Consider adding a test that sends multiple commands with delays between them to verify the race condition is fixed.

### Notes

- This is the most complex fix of the three critical issues
- The `biased` keyword ensures deterministic branch selection
- Testing race conditions is inherently difficult
- May need to run tests multiple times to verify fix
