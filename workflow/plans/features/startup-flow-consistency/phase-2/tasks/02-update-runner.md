## Task: Update Runner to Send Auto-Start Message After First Render

**Objective**: Modify the runner to use the new sync `startup_flutter()` and send the auto-start message after the first frame renders.

**Depends on**: 01-simplify-startup-flutter

**Estimated Time**: 1 hour

### Scope

- `src/tui/runner.rs`: Update startup flow and first-render logic

### Details

#### Current Runner Flow (lines 60-89)

```rust
// Task 08d: Set initial loading state before async operations (if auto_start)
if settings.behavior.auto_start {
    state.set_loading_phase("Initializing...");
    let _ = term.draw(|frame| render::view(frame, &mut state));
}

// Determine startup behavior based on settings
let startup_action = startup::startup_flutter(
    &mut state,
    &settings,
    project_path,
    msg_tx.clone(),
    &mut term,
)
.await;

// If we have a startup action (auto-start session), execute it
if let Some(action) = startup_action {
    handle_action(...);
}
```

#### New Runner Flow

```rust
// Initialize startup state (always enters Normal mode)
let startup_result = startup::startup_flutter(
    &mut state,
    &settings,
    project_path,
);

// Render first frame (user sees Normal mode briefly)
let _ = term.draw(|frame| render::view(frame, &mut state));

// If auto-start is configured, send the message to trigger it
if let startup::StartupAction::AutoStart { configs } = startup_result {
    // Send auto-start message - this will be processed in the event loop
    let _ = msg_tx.send(Message::StartAutoLaunch { configs }).await;
}
```

### Changes Required

1. **Remove pre-loop loading state setup** (lines 60-65)
   - Delete the `if settings.behavior.auto_start { ... }` block that sets loading phase

2. **Update `startup_flutter()` call**
   - Remove `.await` (no longer async)
   - Remove `msg_tx` and `term` arguments
   - Change return type handling to `StartupAction` enum

3. **Add first-frame render** before auto-start message
   - Ensures user sees Normal mode briefly before Loading

4. **Send auto-start message** instead of executing action directly
   - Use `msg_tx.send(Message::StartAutoLaunch { configs }).await`
   - The handler (from Phase 1) will process this

5. **Remove the direct `handle_action()` call** for startup action
   - The action will be returned by the `StartAutoLaunch` handler instead

### Import Updates

Add to imports:
```rust
use crate::app::message::Message;
use super::startup::StartupAction;
```

Remove (if no longer used elsewhere):
```rust
// msg_tx is still used, but term reference to startup is removed
```

### Full Updated Section

```rust
pub async fn run_with_project(project_path: &Path) -> Result<()> {
    // ... (terminal init, settings load, state creation - unchanged)

    // Create unified message channel
    let (msg_tx, msg_rx) = mpsc::channel::<Message>(256);

    // Spawn signal handler
    signals::spawn_signal_handler(msg_tx.clone());

    // Per-session task handles
    let session_tasks: SessionTaskMap = Arc::new(Mutex::new(std::collections::HashMap::new()));

    // Shutdown signal
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    // Initialize startup state (always enters Normal mode)
    let startup_result = startup::startup_flutter(
        &mut state,
        &settings,
        project_path,
    );

    // Render first frame - user sees Normal mode
    let _ = term.draw(|frame| render::view(frame, &mut state));

    // If auto-start is configured, send message to trigger it
    // This will be processed in the event loop, showing Loading screen
    if let startup::StartupAction::AutoStart { configs } = startup_result {
        let _ = msg_tx.send(Message::StartAutoLaunch { configs }).await;
    }

    // Start file watcher for auto-reload
    // ... (unchanged)

    // Run the main loop
    let result = run_loop(...);

    // ... (cleanup - unchanged)
}
```

### Acceptance Criteria

1. Pre-loop loading state setup is removed
2. `startup_flutter()` call is updated (sync, fewer args)
3. First frame renders before auto-start message is sent
4. Auto-start message is sent via channel (not executed directly)
5. `cargo check` passes
6. `cargo clippy -- -D warnings` passes
7. App compiles and runs

### Testing

Manual testing required:

1. **With `auto_start=true`:**
   - Start app
   - Observe: Normal mode appears briefly → Loading screen → Session starts
   - Loading animation should work correctly

2. **With `auto_start=false`:**
   - Start app
   - Observe: Normal mode appears, stays in "Not Connected" state
   - Press '+' to open StartupDialog

### Notes

- The "brief Normal mode" flash is intentional and acceptable
- If the flash is too noticeable, Phase 4 could add a "Starting..." status
- The `msg_tx.send().await` should not block significantly
- Error from send is ignored (channel should never be full at startup)

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/tui/runner.rs` | Updated imports to include `StartupAction`, removed pre-loop loading state setup, updated `startup_flutter()` call to sync version with new signature, added first-frame render before auto-start message, and send `Message::StartAutoLaunch` via channel instead of executing action directly |

### Notable Decisions/Tradeoffs

1. **First-frame render before auto-start message**: This ensures the user briefly sees Normal mode before the Loading screen appears, which is the desired behavior to show a clean transition. The "flash" of Normal mode is intentional and acceptable per the task specification.

2. **Ignored send error**: The `msg_tx.send().await` error is intentionally ignored with `let _ =` because the channel should never be full at startup, and if it somehow fails, the app will simply not auto-start (graceful degradation).

### Testing Performed

- `cargo check` - Passed
- `cargo test --lib` - Passed (1337 tests passed, 0 failed)
- `cargo clippy -- -D warnings` - Passed (no warnings)
- `cargo fmt -- --check` - Passed (code already properly formatted)

### Risks/Limitations

1. **Brief Normal mode flash**: Users will briefly see Normal mode before Loading screen appears when auto-start is enabled. This is intentional per the task design, but could be noticeable on slower systems. Phase 4 could add a "Starting..." status if needed.

2. **Message send timing**: The auto-start message is sent immediately after first render. If the event loop is somehow blocked, there could be a delay before the message is processed. This is unlikely at startup but worth noting.
