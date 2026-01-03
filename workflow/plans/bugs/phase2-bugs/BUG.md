# Phase 2 Bug Fixes - Comprehensive Plan

## TL;DR

Four critical bugs were discovered after completing Phase 2 of Flutter Demon development:

1. **Selector UI** - Layout uses raw stdout instead of Ratatui, causing poor visual presentation
2. **Reload Timeout** - Hot reload commands complete but Flutter Demon times out (daemon responses not routed to RequestTracker)
3. **Quit Doesn't Kill App** - Pressing 'q' closes Flutter Demon but leaves Flutter app running
4. **Process Exit Not Handled** - When Flutter app closes externally, Flutter Demon stays in Loading state

Bug #2 is the **critical root cause** blocking proper command handling; it must be fixed first as it also affects Bug #3.

---

## Bug 1: Selector UI Layout Issues

### Symptoms
- Project selector text appears on plain new lines
- No visual border or centering
- Doesn't follow Ratatui layout best practices

### Root Cause
`src/tui/selector.rs` uses raw crossterm `Print()` commands with manual newlines:

```rust
stdout.queue(Print("\n  "))?;
stdout.queue(SetForegroundColor(Color::Cyan))?;
stdout.queue(Print("Flutter Demon".bold()))?;
```

This bypasses Ratatui's layout system entirely.

### Affected Modules
- `src/tui/selector.rs`

### Proposed Fix
1. Initialize a temporary Ratatui terminal for the selector
2. Create a centered modal widget with proper border
3. Use `List` widget for project items with highlight state
4. Add arrow key navigation (currently only number keys work)
5. Follow the layout patterns in `src/tui/layout.rs`

### Priority
Low - Cosmetic issue; current implementation is functional

---

## Bug 2: Reload Timeout (CRITICAL)

### Symptoms
```
│20:30:48 • [app] Reloading...
│20:30:48 • [flutter] Reloaded 0 libraries in 74ms (compile: 7 ms, reload: 0 ms, reassemble: 29 ms).
│20:31:18 ✗ [app] Reload failed: Flutter process error: Command 'hot reload' timed out after 30s
```

- Flutter successfully reloads (visible in logs and app updates)
- Flutter Demon UI stuck in "Reloading" state
- Times out after 30 seconds with error
- Reload/restart commands unavailable during timeout period
- Auto-reload from file watcher also affected

### Root Cause
**Response routing is broken.** The daemon sends JSON-RPC responses, but they never reach the `RequestTracker`.

In `src/app/handler.rs` lines 233-236:
```rust
// Handle responses separately (they don't create log entries)
if matches!(msg, DaemonMessage::Response { .. }) {
    tracing::debug!("Response received: {}", msg.summary());
    return;  // <-- BUG: Response is discarded, never forwarded!
}
```

The response is parsed correctly but then **immediately discarded**. The `RequestTracker::handle_response()` method exists but is **never called** in the actual application code.

### Command Flow Analysis
```
User presses 'r'
    ↓
Message::HotReload
    ↓
handler::update() spawns Task::Reload { app_id }
    ↓
execute_task() calls sender.send(DaemonCommand::Reload)
    ↓
CommandSender::send_with_timeout():
    1. tracker.register() → creates oneshot channel, returns rx
    2. Sends JSON to Flutter daemon stdin
    3. Awaits response_rx with 30s timeout
    ↓
Flutter daemon processes reload, sends response to stdout
    ↓
FlutterProcess::stdout_reader() → DaemonEvent::Stdout(line)
    ↓
run_loop() → process_message() → handler::update()
    ↓
handle_daemon_event() parses Response
    ↓
Response is logged at debug level and DISCARDED  ← BUG
    ↓
response_rx never receives data
    ↓
30 second timeout triggers
```

### Affected Modules
- `src/tui/mod.rs` - Needs to route responses to tracker
- `src/app/handler.rs` - Currently discards responses
- `src/daemon/protocol.rs` - Response parsing (works correctly)
- `src/daemon/commands.rs` - RequestTracker (works correctly)

### Proposed Fix

**Option A: Route responses in run_loop() (Recommended)**

Intercept stdout events in `run_loop()` before passing to handler:

```rust
// In run_loop(), when processing daemon_rx:
while let Ok(event) = daemon_rx.try_recv() {
    // Pre-process stdout for responses
    if let DaemonEvent::Stdout(ref line) = event {
        if let Some(json) = protocol::strip_brackets(line) {
            if let Some(DaemonMessage::Response { id, result, error }) = 
                DaemonMessage::parse(json) 
            {
                if let (Some(sender), Some(id_num)) = (&cmd_sender, id.as_u64()) {
                    sender.tracker().handle_response(id_num, result, error).await;
                }
            }
        }
    }
    // Still pass to handler for logging/other processing
    process_message(state, Message::Daemon(event), &msg_tx, &cmd_sender);
}
```

**Challenge:** `run_loop()` is currently sync, but `handle_response()` is async.

**Solution:** Make `run_loop()` async or use `tokio::spawn` for response routing.

**Option B: Pass tracker reference through Message**

Add tracker to the update context - more invasive refactor.

### Priority
**Critical** - This bug makes reload functionality unusable and blocks Bug #3 fix

---

## Bug 3: Quit Doesn't Terminate Flutter App

### Symptoms
```
│20:37:48 • [app] Shutting down Flutter process...
```

Flutter Demon closes, but the Flutter app continues running in the simulator/device.

### Root Cause

Two issues:

1. **`daemon.shutdown` doesn't stop the app** - This command shuts down the daemon protocol layer, not the running Flutter app. The app continues running.

2. **Shutdown doesn't wait for response** - Due to Bug #2, even if we sent `app.stop`, we'd never get confirmation.

In `src/daemon/process.rs`:
```rust
pub async fn shutdown(&mut self) -> Result<()> {
    // Try graceful shutdown first
    let shutdown_cmd = r#"{"method":"daemon.shutdown","id":9999}"#;
    let _ = self.send_json(shutdown_cmd).await;

    // Wait up to 5 seconds for graceful exit
    match timeout(Duration::from_secs(5), self.child.wait()).await {
        // ...
    }
}
```

The `daemon.shutdown` method tells the daemon protocol to disconnect, but the Flutter app itself keeps running. We need to send `app.stop` first.

### Affected Modules
- `src/daemon/process.rs` - shutdown() method
- `src/tui/mod.rs` - cleanup sequence

### Proposed Fix

1. **Fix Bug #2 first** (response routing)

2. **Modify shutdown sequence:**
```rust
pub async fn shutdown(&mut self, app_id: Option<&str>, cmd_sender: &CommandSender) -> Result<()> {
    // Step 1: Stop the app if we have an app_id
    if let Some(id) = app_id {
        info!("Stopping Flutter app: {}", id);
        let _ = cmd_sender.send_with_timeout(
            DaemonCommand::Stop { app_id: id.to_string() },
            Duration::from_secs(5)
        ).await;
    }

    // Step 2: Shutdown daemon protocol
    let shutdown_cmd = r#"{"method":"daemon.shutdown","id":9999}"#;
    let _ = self.send_json(shutdown_cmd).await;

    // Step 3: Wait or force kill
    // ... existing code ...
}
```

3. **Pass app_id to shutdown in run_with_project():**
```rust
if let Some(mut p) = flutter {
    let app_id = state.current_app_id.clone();
    if let Err(e) = p.shutdown(app_id.as_deref(), &cmd_sender).await {
        // ...
    }
}
```

### Priority
High - Core functionality broken

---

## Bug 4: App Exit Doesn't Quit Flutter Demon

### Symptoms
```
│20:36:32 ⚠ [app] App stopped
│20:36:32 ⚠ [app] Flutter process exited
```

When the Flutter app is closed externally (e.g., stopping simulator, closing app window), Flutter Demon stays in a "Loading" state indefinitely instead of exiting.

### Root Cause

In `src/app/handler.rs` line 297-299:
```rust
DaemonEvent::Exited { code } => {
    // ... logging ...
    state.phase = AppPhase::Initializing;  // Should be Quitting
}
```

When the Flutter process exits, the phase is set to `Initializing` instead of `Quitting`. The `should_quit()` method only returns `true` for `AppPhase::Quitting`.

### Affected Modules
- `src/app/handler.rs` - Exited event handling

### Proposed Fix

Change behavior when Flutter process exits:

```rust
DaemonEvent::Exited { code } => {
    let (level, message) = match code {
        Some(0) => (LogLevel::Info, "Flutter process exited normally".to_string()),
        Some(c) => (LogLevel::Warning, format!("Flutter process exited with code {}", c)),
        None => (LogLevel::Warning, "Flutter process exited".to_string()),
    };
    state.add_log(LogEntry::new(level, LogSource::App, message));
    
    // Exit Flutter Demon when Flutter process exits
    state.phase = AppPhase::Quitting;  // Changed from Initializing
}
```

**Alternative (Future Enhancement):** Show a prompt offering to restart the app or quit. This would require additional UI work.

### Priority
Medium - Annoying UX issue, not blocking

---

## Implementation Order

### Phase 1: Fix Response Routing (Bug #2)
**Duration:** 1-2 hours
**Complexity:** Medium

1. Modify `run_loop()` to intercept Response messages
2. Route responses to `RequestTracker::handle_response()`
3. Handle async nature (may need to make run_loop async or spawn task)
4. Test: Press 'r', verify reload completes without timeout

### Phase 2: Fix Shutdown Sequence (Bug #3)
**Duration:** 30 min
**Complexity:** Low

1. Modify `FlutterProcess::shutdown()` to accept app_id
2. Send `app.stop` before `daemon.shutdown`
3. Update `run_with_project()` to pass app_id
4. Test: Press 'q', verify both Flutter Demon and Flutter app close

### Phase 3: Fix Process Exit Handling (Bug #4)
**Duration:** 15 min
**Complexity:** Low

1. Change `AppPhase::Initializing` to `AppPhase::Quitting` in Exited handler
2. Test: Close Flutter app externally, verify Flutter Demon exits

### Phase 4: Fix Selector UI (Bug #1)
**Duration:** 2-3 hours
**Complexity:** Medium-High

1. Refactor `selector.rs` to use Ratatui
2. Create centered modal layout
3. Use List widget with highlight
4. Add arrow key navigation
5. Test: Have multiple Flutter projects, verify nice UI

---

## Testing Checklist

### After Bug #2 Fix
- [ ] Press 'r' → reload completes, no timeout
- [ ] Press 'R' → restart completes, no timeout
- [ ] Modify .dart file → auto-reload works, no timeout
- [ ] Multiple reloads in succession work

### After Bug #3 Fix
- [ ] Press 'q' → Flutter Demon closes AND Flutter app stops
- [ ] Ctrl+C → Same behavior
- [ ] No orphan Flutter processes after quit

### After Bug #4 Fix
- [ ] Stop iOS/Android simulator → Flutter Demon exits
- [ ] Close app from device → Flutter Demon exits

### After Bug #1 Fix
- [ ] Multiple projects show in centered modal
- [ ] Border and styling match Ratatui patterns
- [ ] Arrow keys navigate (optional)
- [ ] Number keys still work

---

## Edge Cases & Risks

### Response Routing
- **Risk:** Race condition between response parsing and tracker cleanup
- **Mitigation:** Ensure tracker cleanup only happens on timeout, not preemptively

### Shutdown Sequence
- **Risk:** `app.stop` might fail if app already crashed
- **Mitigation:** Ignore errors from app.stop, proceed to daemon.shutdown

### Process Exit
- **Risk:** User might want to restart, not quit
- **Mitigation:** Phase 5 enhancement can add restart prompt

### Selector UI
- **Risk:** Terminal size too small for modal
- **Mitigation:** Graceful fallback to simple list if terminal < 40 cols