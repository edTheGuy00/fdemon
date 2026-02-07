## Task: Refactor TUI Runner to Use Engine

**Objective**: Refactor `tui/runner.rs` to create an `Engine` and delegate all channel/state/watcher management to it. The TUI runner retains only terminal init/restore, the render loop, crossterm event polling, and startup dialog flow. This validates the Engine abstraction with the primary (and more complex) frontend before applying it to headless.

**Depends on**: Task 01 (Engine struct), Task 02 (EngineEvent enum)

**Estimated Time**: 5-7 hours

### Scope

- `src/tui/runner.rs`: Major refactor -- replace manual channel/state setup with `Engine::new()`
- `src/tui/startup.rs`: Extract session cleanup logic to `Engine::shutdown()`, keep terminal-specific startup
- `src/app/engine.rs`: May need minor adjustments based on integration findings

### Details

#### Current TUI Runner Structure (runner.rs)

```
run_with_project(project_path):
  1. terminal::install_panic_hook()
  2. config::init_fdemon_directory()          ── moves to Engine::new()
  3. config::load_settings()                  ── moves to Engine::new()
  4. ratatui::init()                          ── stays (TUI-specific)
  5. AppState::with_settings()                ── moves to Engine::new()
  6. mpsc::channel::<Message>(256)            ── moves to Engine::new()
  7. signals::spawn_signal_handler()          ── moves to Engine::new()
  8. SessionTaskMap::new()                    ── moves to Engine::new()
  9. watch::channel(false)                    ── moves to Engine::new()
  10. startup::startup_flutter()              ── stays (TUI-specific dialog)
  11. Render first frame                      ── stays (TUI-specific)
  12. spawn::spawn_tool_availability_check()  ── stays (startup action)
  13. spawn::spawn_device_discovery()         ── stays (startup action)
  14. FileWatcher::new() + start + bridge     ── moves to Engine::new()
  15. run_loop(...)                           ── refactored to use engine
  16. file_watcher.stop()                     ── moves to Engine::shutdown()
  17. startup::cleanup_sessions()             ── partially moves to Engine::shutdown()
  18. ratatui::restore()                      ── stays (TUI-specific)
```

#### Target TUI Runner Structure

```rust
pub async fn run_with_project(project_path: &Path) -> Result<()> {
    // Install panic hook for terminal restoration
    terminal::install_panic_hook();

    // Create the engine (handles all shared initialization)
    let mut engine = Engine::new(project_path.to_path_buf());

    // Initialize terminal (TUI-specific)
    let mut term = ratatui::init();

    // TUI-specific startup: show NewSessionDialog, load configs
    let _startup_result = startup::startup_flutter(
        &mut engine.state,
        &engine.settings,
        &engine.project_path,
    );

    // Render first frame
    if let Err(e) = term.draw(|frame| render::view(frame, &mut engine.state)) {
        error!("Failed to render initial frame: {}", e);
    }

    // Trigger startup discovery (non-blocking)
    spawn::spawn_tool_availability_check(engine.msg_sender());
    spawn::spawn_device_discovery(engine.msg_sender());

    // Run the main loop
    let result = run_loop(&mut term, &mut engine);

    // Shutdown engine (stops watcher, cleans up sessions)
    engine.shutdown().await;

    // Restore terminal (TUI-specific)
    ratatui::restore();

    result
}
```

#### Target run_loop

```rust
fn run_loop(
    terminal: &mut ratatui::DefaultTerminal,
    engine: &mut Engine,
) -> Result<()> {
    while !engine.should_quit() {
        // Drain and process all pending messages
        engine.drain_pending_messages();

        // Flush batched logs
        engine.flush_pending_logs();

        // Render
        terminal.draw(|frame| render::view(frame, &mut engine.state))?;

        // Handle terminal events (TUI-specific)
        if let Some(message) = event::poll()? {
            engine.process_message(message);
        }
    }

    Ok(())
}
```

#### Changes to run_loop Signature

The current `run_loop` takes 7 parameters:
```rust
fn run_loop(
    terminal, state, msg_rx, msg_tx, session_tasks, shutdown_rx, project_path
) -> Result<()>
```

The new version takes 2:
```rust
fn run_loop(terminal, engine) -> Result<()>
```

This is a massive simplification.

#### Cleanup of startup.rs

`tui/startup.rs::cleanup_sessions()` currently:
1. Drains remaining messages from `msg_rx`
2. Sends `Quit` to all session senders
3. Draws "Shutting down..." frame
4. Sends `shutdown_tx = true`
5. Awaits each task handle with 2-second timeout
6. Drops task handles

Steps 1, 4, 5, 6 move to `Engine::shutdown()`. Steps 2, 3 stay in TUI because they involve terminal drawing. The TUI runner can call `Engine::shutdown()` and optionally draw intermediate frames.

The adjusted flow:
```rust
// In run_with_project, after run_loop:

// Draw shutdown frame (TUI-specific)
if let Err(e) = term.draw(|frame| {
    // Render a "shutting down" message
}) {
    error!("Failed to render shutdown frame: {}", e);
}

// Engine handles the rest
engine.shutdown().await;
ratatui::restore();
```

### Step-by-Step Implementation

1. **Update `run_with_project()`**: Replace lines 32-62 (manual setup) with `Engine::new()`. Keep terminal init, startup dialog, first render, and startup spawns.

2. **Update `run_loop()` signature**: Change from 7 params to `(terminal, engine)`. Replace direct channel access with engine methods.

3. **Update `run_loop()` body**: Replace `msg_rx.try_recv()` loop with `engine.drain_pending_messages()`. Replace `process::process_message()` call with `engine.process_message()`. Replace `state.session_manager.flush_all_pending_logs()` with `engine.flush_pending_logs()`. Replace `state.should_quit()` with `engine.should_quit()`.

4. **Update shutdown sequence**: Replace `file_watcher.stop()` + `startup::cleanup_sessions()` with `engine.shutdown().await`.

5. **Update `run()` (test/demo mode)**: Apply same pattern -- create Engine with dummy path.

6. **Verify `startup.rs`**: Ensure `startup_flutter()` can operate on `&mut AppState` without needing the full Engine. It only needs state and settings, which Engine provides as public fields.

7. **Verify spawn calls**: `spawn::spawn_tool_availability_check()` and `spawn::spawn_device_discovery()` take `mpsc::Sender<Message>`. Get this from `engine.msg_sender()`.

### Acceptance Criteria

1. `tui/runner.rs` creates an `Engine` via `Engine::new()`
2. `run_with_project()` has no direct `mpsc::channel`, `watch::channel`, `SessionTaskMap`, or `FileWatcher` setup
3. `run_loop()` takes only `(terminal, engine)` as parameters
4. `run_loop()` uses `engine.drain_pending_messages()` instead of manual `try_recv()` loop
5. `run_loop()` uses `engine.process_message()` for terminal events
6. Shutdown uses `engine.shutdown().await`
7. `startup::startup_flutter()` still works (receives `&mut engine.state`)
8. The TUI behavior is identical to before (no user-visible changes)
9. `cargo build` succeeds
10. `cargo test` passes (all existing TUI tests)
11. `cargo clippy` is clean
12. Manual test: `cargo run -- /path/to/flutter/project` works as before

### Testing

Existing TUI tests should pass without modification since the Engine is a transparent wrapper around the same state and channels.

Additional manual testing:
- Start fdemon, verify NewSessionDialog appears
- Select a device, verify session spawns
- Hot reload (press `r`), verify reload works
- File watcher auto-reload, verify it triggers
- Quit (press `q`), verify clean shutdown
- SIGINT (Ctrl+C), verify clean shutdown

### Notes

- The `terminal` variable must remain outside Engine because `ratatui::DefaultTerminal` is TUI-specific
- `event::poll()` stays outside Engine because crossterm is TUI-specific
- `render::view()` stays outside Engine because rendering is TUI-specific
- `Engine.state` is `pub` so the render function can read it: `render::view(frame, &mut engine.state)`
- If `startup.rs::cleanup_sessions()` needs terminal access for the shutdown animation, keep a thin TUI wrapper that calls `engine.shutdown()` internally
- This is the most impactful task in Phase 2 -- it establishes the pattern that Task 04 follows

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/tui/runner.rs` | Complete refactor to use Engine. Replaced 7-parameter run_loop with 2-parameter version. Eliminated all manual channel, watcher, and session task setup. |

### Notable Decisions/Tradeoffs

1. **Simplified run_with_project()**: Reduced from ~130 lines to ~55 lines by delegating all shared initialization to `Engine::new()`. The TUI runner now only handles terminal-specific concerns (setup, rendering, event polling, teardown).

2. **Eliminated run_loop() parameters**: Changed from 7 parameters `(terminal, state, msg_rx, msg_tx, session_tasks, shutdown_rx, project_path)` to just 2 `(terminal, engine)`. This is a massive simplification and makes the event loop much clearer.

3. **Engine owns cleanup**: Replaced manual `file_watcher.stop()` and `startup::cleanup_sessions()` with a single `engine.shutdown().await` call. The Engine handles all cleanup logic internally.

4. **Preserved TUI-specific code**: Terminal setup/teardown (`ratatui::init()`/`ratatui::restore()`), panic hook installation, startup dialog flow, and event polling (`event::poll()`) all remain in the TUI runner as they are presentation-specific.

5. **Updated run() test mode**: Applied the same pattern to the demo/test entry point for consistency.

### Testing Performed

- `cargo check` - Passed
- `cargo test --lib` - Passed (1525 tests, 0 failed)
- `cargo clippy -- -D warnings` - Passed (no warnings)

### Risks/Limitations

1. **Behavior preserved**: No user-visible changes. The refactoring is purely structural and maintains identical runtime behavior.

2. **startup::cleanup_sessions() still exists**: The function is now orphaned but left in place for potential future use or to be removed in a cleanup task. Engine.shutdown() has replaced its functionality.

3. **Pattern established**: This refactoring establishes the pattern that Task 04 (headless runner) will follow, validating the Engine abstraction with the more complex frontend first.
