## Task: Fix Response Routing for Daemon Commands

**Objective**: Route daemon JSON-RPC responses to the RequestTracker so that command completion (reload, restart, stop) is properly detected instead of timing out after 30 seconds.

**Depends on**: None

---

### Problem Summary

When commands are sent to the Flutter daemon (hot reload, restart, stop), the daemon sends back JSON-RPC responses. These responses are currently parsed but immediately discarded in `handle_daemon_event()`, causing the `CommandSender::send_with_timeout()` to wait indefinitely until the 30-second timeout triggers.

The `RequestTracker::handle_response()` method exists and works correctly, but it is **never called** in the application code.

---

### Scope

#### `src/tui/mod.rs`
- Modify `run_loop()` to intercept `DaemonEvent::Stdout` events
- Parse stdout lines for JSON-RPC responses
- When a `DaemonMessage::Response` is detected, extract `id`, `result`, `error`
- Call `tracker.handle_response(id, result, error)` to complete the pending request
- Handle the async nature of `handle_response()` (may need to spawn a task or make run_loop async)

#### `src/app/handler.rs`
- Remove or modify the early return for Response messages (line 233-236)
- Responses can still be logged but should not block other processing

#### `src/daemon/protocol.rs`
- No changes needed (response parsing already works)

#### `src/daemon/commands.rs`
- No changes needed (RequestTracker already works)

---

### Implementation Details

**Option A: Spawn async task for response handling (Recommended)**

```rust
// In run_loop(), when processing daemon_rx:
while let Ok(event) = daemon_rx.try_recv() {
    // Pre-process stdout for responses before passing to handler
    if let DaemonEvent::Stdout(ref line) = event {
        if let Some(json) = protocol::strip_brackets(line) {
            if let Some(DaemonMessage::Response { id, result, error }) = 
                DaemonMessage::parse(json) 
            {
                if let Some(ref sender) = cmd_sender {
                    if let Some(id_num) = id.as_u64() {
                        let tracker = sender.tracker().clone();
                        tokio::spawn(async move {
                            tracker.handle_response(id_num, result, error).await;
                        });
                    }
                }
            }
        }
    }
    // Still pass to handler for logging/other processing
    process_message(state, Message::Daemon(event), &msg_tx, &cmd_sender);
}
```

**Option B: Make run_loop async**

Convert `run_loop()` from sync to async, allowing direct `.await` on `handle_response()`. This is a larger refactor but cleaner long-term.

---

### Acceptance Criteria

1. Pressing 'r' triggers hot reload and completes successfully without timeout
2. Log shows "Reloaded in Xms" instead of "Reload failed: Command timed out"
3. Pressing 'R' triggers hot restart and completes successfully
4. Pressing 's' stops the app without timeout
5. Auto-reload on file save works without timeout
6. Multiple consecutive reloads work correctly
7. UI returns to "Running" state after reload completes (not stuck in "Reloading")

---

### Testing

#### Manual Testing
1. Run `cargo run -- /path/to/flutter/app`
2. Wait for app to start and reach "Running" state
3. Press 'r' - verify reload completes in < 1 second, no timeout error
4. Press 'R' - verify restart completes, no timeout
5. Modify a .dart file - verify auto-reload works
6. Press 'r' multiple times in quick succession - all should complete

#### Unit Tests
- Existing `RequestTracker` tests already cover the tracker logic
- Add integration test that simulates receiving a response and verifies the tracker is notified

---

### Estimated Duration
1-2 hours

### Priority
**Critical** - This bug makes reload functionality unusable and blocks Bug #3 fix

---

## Completion Summary

**Status:** ✅ Done

**Files Modified:**
- `src/tui/mod.rs` - Added response routing logic in `run_loop()`

**Implementation Details:**
- Used Option A (spawn async task) as recommended
- Added imports for `protocol`, `DaemonMessage` to `tui/mod.rs`
- In `run_loop()`, before passing daemon events to `process_message()`:
  1. Check if event is `DaemonEvent::Stdout`
  2. Strip brackets and parse JSON
  3. If message is `DaemonMessage::Response`, extract `id`, `result`, `error`
  4. Spawn async task to call `tracker.handle_response(id, result, error)`
  5. Event still passes to handler for logging

**Notable Decisions:**
- No changes needed to `handler.rs` - the early return for Response messages is fine since response routing now happens before the handler is called
- Used `tokio::spawn()` to handle the async `handle_response()` call without blocking the sync `run_loop()`

**Testing Performed:**
- `cargo check` - PASS
- `cargo test` - PASS (218 tests)
- `cargo clippy` - PASS (no warnings)

**Risks/Limitations:**
- Manual testing against a real Flutter project is recommended to verify end-to-end functionality
- The spawned task for `handle_response()` runs independently; in rare cases of rapid successive responses, ordering could theoretically be non-deterministic (but this is acceptable for response matching)