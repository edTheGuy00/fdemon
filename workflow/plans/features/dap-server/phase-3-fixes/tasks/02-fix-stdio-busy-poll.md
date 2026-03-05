## Task: Fix Busy-Poll in Stdio Broadcast Receiver

**Objective**: Eliminate the CPU hot-spin caused by a dead broadcast channel in the `run_on` select loop. Currently, the stdio transport creates a `broadcast::channel(1)` whose sender is immediately dropped, causing `recv()` to return `Err(RecvError::Closed)` on every poll — spinning the CPU at 100%.

**Depends on**: None

**Estimated Time**: 1–2 hours

**Severity**: CRITICAL — causes high CPU usage for all stdio DAP sessions.

### Scope

- `crates/fdemon-dap/src/server/session.rs`: Fix the `run_on` select loop to handle `Closed` properly
- `crates/fdemon-dap/src/transport/stdio.rs`: Fix the dummy broadcast channel creation

### Details

#### Root Cause

In `transport/stdio.rs:95`:
```rust
let (_, log_event_rx) = tokio::sync::broadcast::channel(1);
//   ^ sender dropped immediately — channel is dead
```

In `session.rs:425-446`, the `run_on` select loop:
```rust
log_event_result = log_event_rx.recv() => {
    match log_event_result {
        // ...
        Err(broadcast::error::RecvError::Closed) => {
            tracing::debug!("DAP log event broadcast channel closed");
            // No break, no disable — loop continues, recv() fires again instantly
        }
    }
}
```

`broadcast::Receiver::recv()` with a dead sender returns `Poll::Ready(Err(Closed))` immediately and forever. The `tokio::select!` macro picks this branch repeatedly since it's always ready.

#### Recommended Fix (two changes)

**Change 1 — `session.rs`: Make `log_event_rx` an `Option` and disable on `Closed`**

This is the robust fix that handles any scenario where the broadcast sender is dropped mid-session:

```rust
// Change the parameter type or wrap in Option immediately:
let mut log_event_rx: Option<broadcast::Receiver<DebugEvent>> = Some(log_event_rx);

// In the select loop, replace the log_event arm:
log_event_result = async {
    match &mut log_event_rx {
        Some(rx) => Some(rx.recv().await),
        None => std::future::pending::<Option<_>>().await,
    }
}, if log_event_rx.is_some() => {
    if let Some(result) = log_event_result {
        match result {
            Ok(debug_event) => { /* existing handler */ }
            Err(broadcast::error::RecvError::Lagged(n)) => { /* existing handler */ }
            Err(broadcast::error::RecvError::Closed) => {
                tracing::debug!("DAP log event broadcast channel closed, disabling");
                log_event_rx = None; // Permanently disable this branch
            }
        }
    }
}
```

**Change 2 — `stdio.rs`: Keep the sender alive (belt-and-suspenders)**

Even with the session.rs fix, the stdio transport should not create a dead channel:

```rust
// Before (sender dropped immediately):
let (_, log_event_rx) = tokio::sync::broadcast::channel(1);

// After (keep sender alive for session lifetime):
let (_log_event_tx, log_event_rx) = tokio::sync::broadcast::channel(1);
// _log_event_tx stays alive until run_on returns, keeping the channel open
```

This is a one-character fix (`_` → `_log_event_tx`) that prevents `recv()` from returning `Closed`.

#### Also fix in tests

The same pattern appears in test helpers at `stdio.rs:141` and `stdio.rs:484`. Apply the same sender-lifetime fix.

### Acceptance Criteria

1. No CPU spin when a stdio DAP session is idle (verified by checking CPU usage or adding a test assertion)
2. The `"DAP log event broadcast channel closed"` debug log appears at most once per session, not continuously
3. The select loop continues to function after the broadcast channel closes — read_message and shutdown branches still work
4. All existing session and stdio tests pass
5. New test: create a `run_on` session with a dead broadcast channel, verify no busy-poll (channel closed is handled gracefully)

### Testing

```rust
#[tokio::test]
async fn test_run_on_handles_closed_broadcast_channel() {
    // Create a dead broadcast channel (sender dropped)
    let (_, log_event_rx) = broadcast::channel::<DebugEvent>(1);

    // Create a session with controlled reader/writer
    // Send an initialize request, verify it's processed
    // Verify CPU doesn't spin (the select loop responds to other branches)
    // Send disconnect, verify clean shutdown
}
```

### Notes

- Both changes should be applied: the session.rs fix handles the general case, and the stdio.rs fix prevents the specific pathological input.
- The same `Option` pattern should be applied to `run_on_with_backend` if it also has a broadcast receiver (it uses `mpsc` which doesn't have this issue, but verify).
- This fix will slightly change the select loop structure, which task 04 (consolidate loops) will later refactor further.
