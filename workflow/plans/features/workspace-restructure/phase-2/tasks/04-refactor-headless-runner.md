## Task: Refactor Headless Runner to Use Engine

**Objective**: Refactor `headless/runner.rs` to use the `Engine` struct, eliminating ~250 lines of duplicated orchestration code. The biggest win is removing `spawn_headless_session()` (~160 lines) which duplicates `app/actions::spawn_session()`, and the duplicate signal handler (~35 lines). The headless runner should be a thin wrapper: create Engine, add stdin reader, add NDJSON event hooks, run async event loop.

**Depends on**: Task 03 (TUI runner refactored first to validate Engine pattern)

**Estimated Time**: 4-6 hours

### Scope

- `src/headless/runner.rs`: Major refactor -- replace manual setup with Engine, eliminate duplicated spawn
- `src/headless/mod.rs`: May need minor updates to HeadlessEvent
- `src/app/actions.rs`: May need a lifecycle observer hook for headless event emission during spawn

### Details

#### Current Duplication Analysis

| Code Block | Lines | Action |
|---|---|---|
| Config init + settings load + AppState creation | ~15 lines | **Remove** -- Engine::new() does this |
| Message channel creation | ~3 lines | **Remove** -- Engine::new() does this |
| SessionTaskMap creation | ~3 lines | **Remove** -- Engine::new() does this |
| Shutdown channel creation | ~3 lines | **Remove** -- Engine::new() does this |
| `spawn_signal_handler()` (own copy) | ~35 lines | **Remove** -- Engine::new() uses app::signals |
| File watcher setup + bridge | ~17 lines | **Remove** -- Engine::new() does this |
| `spawn_headless_session()` | ~160 lines | **Remove** -- use app/actions::spawn_session() |
| `handle_headless_action()` | ~28 lines | **Remove** -- Engine processes actions via process_message() |
| `headless_auto_start()` | ~55 lines | **Keep but simplify** -- still needs headless-specific HeadlessEvent emission |
| `headless_event_loop()` | ~40 lines | **Simplify** -- use engine.process_message() |
| `process_headless_message()` | ~25 lines | **Keep** -- headless-specific event emission wrapper |
| `emit_pre/post_message_events()` | ~30 lines | **Keep** -- headless-specific |
| `spawn_stdin_reader_blocking()` | ~40 lines | **Keep** -- headless-specific input source |

**Net reduction: ~250+ lines removed**, ~190 lines kept/simplified.

#### Target Headless Runner Structure

```rust
pub async fn run_headless(project_path: &Path) -> Result<()> {
    // Initialize error handling and logging
    color_eyre::install().map_err(|e| Error::terminal(e.to_string()))?;
    crate::common::logging::init()?;

    info!("Flutter Demon starting in HEADLESS mode");

    // Create engine (all shared init happens here)
    let mut engine = Engine::new(project_path.to_path_buf());

    // Spawn headless-specific stdin reader
    let stdin_tx = engine.msg_sender();
    std::thread::spawn(move || {
        spawn_stdin_reader_blocking(stdin_tx);
    });

    // Auto-start: discover devices and send SpawnSession message
    headless_auto_start(&mut engine).await;

    // Main event loop
    let result = headless_event_loop(&mut engine).await;

    // Shutdown
    engine.shutdown().await;

    info!("Flutter Demon headless mode exiting");
    result
}
```

#### Eliminating spawn_headless_session()

The biggest duplication is `spawn_headless_session()` (lines 431-589 in headless/runner.rs) which nearly duplicates `app/actions.rs::spawn_session()` (lines ~158-331). The only difference is that the headless version emits `HeadlessEvent` at certain points:

| Point | HeadlessEvent emitted |
|---|---|
| After process spawns | `daemon_connected` |
| On DaemonEvent::Exited | `app_stopped` |
| On DaemonMessage parse | `emit_daemon_message_event()` |
| On spawn failure | `error` |

**Solution**: Remove `spawn_headless_session()` entirely. Instead, the headless runner processes `SpawnSession` actions through the normal `Engine.process_message()` -> `handle_action()` -> `spawn_session()` path. The headless event emission happens in `emit_post_message_events()` based on state changes rather than inline in the spawn function.

This approach works because:
1. `headless_auto_start()` creates a session and sends a message that results in `UpdateAction::SpawnSession`
2. The engine processes this via `process_message()` which calls `handle_action()` -> `spawn_session()`
3. `HeadlessEvent` emission happens by observing state changes after processing

#### Simplified headless_auto_start()

```rust
async fn headless_auto_start(engine: &mut Engine) {
    info!("Discovering devices for headless auto-start...");

    match devices::discover_devices().await {
        Ok(result) => {
            info!("Found {} device(s)", result.devices.len());

            for device in &result.devices {
                HeadlessEvent::device_detected(&device.id, &device.name, &device.platform).emit();
            }

            // Cache devices in state
            engine.state.set_device_cache(result.devices.clone());

            // Pick first device
            if let Some(device) = result.devices.first() {
                info!("Auto-starting with device: {} ({})", device.name, device.id);

                match engine.state.session_manager.create_session(device) {
                    Ok(session_id) => {
                        HeadlessEvent::session_created(&session_id.to_string(), &device.name).emit();

                        // Send SpawnSession as a message -- Engine will handle it
                        // via process_message -> handle_action -> spawn_session
                        let _ = engine.msg_tx.send(Message::SpawnSessionRequested {
                            session_id,
                            device: device.clone(),
                            config: None,
                        }).await;
                    }
                    Err(e) => {
                        error!("Failed to create session: {}", e);
                        HeadlessEvent::error(format!("Failed to create session: {}", e), true).emit();
                    }
                }
            } else {
                error!("No devices found");
                HeadlessEvent::error("No devices found".to_string(), true).emit();
            }
        }
        Err(e) => {
            error!("Device discovery failed: {}", e);
            HeadlessEvent::error(format!("Device discovery failed: {}", e), true).emit();
        }
    }
}
```

Note: If a `SpawnSessionRequested` message doesn't exist yet, the auto-start can directly call `handle_action()` from the engine's action dispatch instead. The key point is eliminating the duplicate spawn code.

#### Simplified headless_event_loop()

```rust
async fn headless_event_loop(engine: &mut Engine) -> Result<()> {
    loop {
        if engine.should_quit() {
            info!("Quit requested");
            break;
        }

        match engine.msg_rx.recv().await {
            Some(msg) => {
                // Emit pre-processing events
                emit_pre_message_events(&engine.state, &msg);

                // Process through engine
                engine.process_message(msg);
                engine.flush_pending_logs();

                // Emit post-processing events
                emit_post_message_events(&engine.state);
            }
            None => {
                warn!("Message channel closed");
                break;
            }
        }
    }

    Ok(())
}
```

#### Signal Handler Consolidation

The headless runner has its own `spawn_signal_handler()` that duplicates `app::signals::spawn_signal_handler()` but adds `HeadlessEvent::error()` emission. After this refactor, the engine uses `app::signals::spawn_signal_handler()` (which just sends `Message::Quit`). The headless runner detects the quit in the event loop and can emit its HeadlessEvent there.

### Step-by-Step Implementation

1. **Update `run_headless()`**: Replace manual init with `Engine::new()`. Remove duplicate signal handler spawn.

2. **Simplify `headless_auto_start()`**: Change to take `&mut Engine` instead of `(&mut AppState, &Path, Sender)`. Use engine's msg_tx to send spawn action.

3. **Remove `handle_headless_action()`**: No longer needed -- engine processes actions internally.

4. **Remove `spawn_headless_session()`**: The entire 160-line function is eliminated. Session spawning goes through `spawn_session()` in `app/actions.rs`.

5. **Simplify `headless_event_loop()`**: Use `engine.msg_rx.recv().await` and `engine.process_message()`.

6. **Update `process_headless_message()`**: Simplify to use engine methods.

7. **Keep `emit_pre/post_message_events()`**: These are headless-specific and stay.

8. **Keep `spawn_stdin_reader_blocking()`**: This is headless-specific input and stays.

9. **Keep `emit_daemon_message_event()`**: This is headless-specific and stays, but may be simplified once EngineEvent broadcasting is wired in Task 06.

10. **Update shutdown**: Replace manual shutdown with `engine.shutdown().await`.

### Acceptance Criteria

1. `headless/runner.rs` creates an `Engine` via `Engine::new()`
2. `spawn_headless_session()` is completely removed (~160 lines eliminated)
3. `handle_headless_action()` is completely removed
4. Headless `spawn_signal_handler()` is removed (uses Engine's signal handler)
5. Manual channel/state/watcher setup is removed
6. Session spawning goes through `app/actions::spawn_session()` (shared with TUI)
7. NDJSON event emission still works via `HeadlessEvent` (pre/post hooks)
8. Stdin reader still works
9. `cargo build` succeeds
10. `cargo test` passes
11. `cargo clippy` is clean
12. E2E tests pass: `cargo test --test e2e` (or `cargo nextest run --test e2e` with retry)

### Testing

Existing headless/E2E tests should pass. If the project has E2E tests that test headless mode, run them:

```bash
cargo test headless
cargo test e2e
```

Manual testing:
- `cargo run -- --headless /path/to/flutter/project`
- Verify NDJSON events appear on stdout
- Verify `r` + Enter triggers hot reload
- Verify `q` + Enter quits
- Verify Ctrl+C quits

### Notes

- **The `msg_rx` field on Engine is public** so the headless runner can `engine.msg_rx.recv().await` directly. This is intentional -- the headless runner uses async recv (blocking), while TUI uses `drain_pending_messages()` (non-blocking). Both patterns are supported.
- **HeadlessEvent emission during spawn**: The current headless spawn emits events inline (daemon_connected, app_started, etc.). After this refactor, these events are detected from state changes in `emit_post_message_events()`. This may require tracking "last known phase" to detect transitions. If this proves too complex, an alternative is to add a callback/hook to `app/actions::spawn_session()` that the headless runner can provide.
- **The net reduction should be ~250 lines** from headless/runner.rs, making it a thin ~150-line wrapper around Engine.

---

## Completion Summary

**Status:** Not Started
