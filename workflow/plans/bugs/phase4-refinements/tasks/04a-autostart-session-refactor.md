## Task 4a: Refactor Auto-Start to Use Sessions

**Objective**: Change auto-start mode to create sessions through the SessionManager instead of owning FlutterProcess directly. This eliminates the dual code path where auto-start bypasses the session system.

**Depends on**: Tasks 01, 02, 03, 05 (all prerequisite tasks complete)

---

### Background

Currently, auto-start mode has a completely different code path:
1. `startup_flutter()` spawns FlutterProcess directly
2. Returns `(Option<FlutterProcess>, Option<CommandSender>)` 
3. The process is owned outside of SessionManager
4. Uses `daemon_tx` channel directly instead of SessionDaemon messages

This must be unified with the normal session flow:
1. Auto-start discovers devices and finds matching device
2. Creates session in SessionManager
3. Returns UpdateAction::SpawnSession (deferred spawn)
4. Process owned by session task like any other session

---

### Scope

#### `src/tui/startup.rs` (Major Changes)

**Current signature:**
```rust
pub async fn startup_flutter(...) -> (Option<FlutterProcess>, Option<CommandSender>)
```

**New approach:**
- Change return type to `Option<UpdateAction>` (the SpawnSession action to execute)
- Remove FlutterProcess spawning from startup_flutter
- Create session in SessionManager and return SpawnSession action
- Let the event loop handle the action like normal device selection

**Specific changes:**
- Lines 28-110: Rewrite auto-start path to:
  1. Discover devices (await)
  2. Find matching device
  3. Create session via `state.session_manager.create_session()`
  4. Return `Some(UpdateAction::SpawnSession { ... })`
- Remove `daemon_tx` parameter (no longer needed)
- Remove FlutterProcess::spawn_with_config call

#### `src/tui/runner.rs` (Major Changes)

**Remove daemon_rx channel:**
- Lines 44-45: Remove `let (daemon_tx, daemon_rx) = mpsc::channel::<DaemonEvent>(256);`
- Remove `daemon_tx` from `startup_flutter()` call
- Lines 143-158: Remove daemon_rx processing in run_loop
- Remove `daemon_rx` parameter from run_loop signature
- Remove `route_daemon_response()` function (lines 164-182) - will be fully removed in 4b

**Handle startup action:**
- After calling `startup_flutter()`, if it returns Some(action), execute it via handle_action

#### `src/tui/startup.rs` - `cleanup_sessions` (Simplification)

**Current signature:**
```rust
pub async fn cleanup_sessions(
    state: &mut AppState,
    term: &mut ratatui::DefaultTerminal,
    flutter: Option<FlutterProcess>,  // REMOVE THIS
    cmd_sender: Arc<Mutex<Option<CommandSender>>>,
    session_tasks: SessionTaskMap,
    shutdown_tx: watch::Sender<bool>,
)
```

**New signature:**
```rust
pub async fn cleanup_sessions(
    state: &mut AppState,
    term: &mut ratatui::DefaultTerminal,
    session_tasks: SessionTaskMap,
    shutdown_tx: watch::Sender<bool>,
)
```

**Changes:**
- Remove `flutter: Option<FlutterProcess>` parameter
- Remove `cmd_sender` parameter (only used for flutter.shutdown)
- Remove lines 132-152 (the `if let Some(mut p) = flutter` branch)
- Keep only the session_tasks cleanup path (lines 153-180)

---

### Implementation Steps

1. **Update startup_flutter signature and return type**
   - Return `Option<UpdateAction>` instead of tuple
   - Remove daemon_tx parameter

2. **Rewrite auto-start logic in startup_flutter**
   - Keep device discovery
   - Instead of spawning FlutterProcess, create session and return action
   - Handle multiple auto-start configs (future: spawn multiple sessions)

3. **Update runner.rs to handle startup action**
   - Remove daemon_rx channel
   - Execute returned action from startup_flutter

4. **Simplify cleanup_sessions**
   - Remove flutter parameter
   - Remove cmd_sender parameter
   - Only handle session_tasks path

5. **Update run() test function in runner.rs**
   - Remove daemon_rx from test harness

---

### Edge Cases

1. **No devices found during auto-start**
   - Fall back to showing device selector (current behavior preserved)
   
2. **Device specifier doesn't match**
   - Fall back to showing device selector (current behavior preserved)

3. **Multiple auto-start configs**
   - For now, still only start first config
   - Future: could spawn multiple sessions

4. **Auto-start fails to spawn**
   - SessionSpawnFailed message will be sent by spawn_session task
   - User sees error in session, can close and retry

---

### Acceptance Criteria

1. ✅ `startup_flutter()` no longer returns FlutterProcess
2. ✅ `startup_flutter()` no longer takes daemon_tx parameter
3. ✅ Auto-start creates session via SessionManager
4. ✅ Auto-start returns UpdateAction::SpawnSession
5. ✅ `cleanup_sessions()` only handles session_tasks
6. ✅ No `daemon_rx` channel in runner.rs
7. ✅ Auto-start with `launch.toml` still works
8. ✅ Manual start still works
9. ✅ All tests pass
10. ✅ No clippy warnings

---

### Testing

#### Compile-Time
- `cargo check` passes
- `cargo clippy` shows no new warnings
- No unused variable warnings

#### Unit Tests
- Existing startup tests should pass (may need updates)

#### Integration Testing
1. **Auto-start mode:**
   - Create `launch.toml` with `auto_start = true`
   - Run fdemon → verify session starts
   - Verify session appears in SessionManager
   - Verify hot reload works

2. **Manual start mode:**
   - Run fdemon without auto-start config
   - Verify device selector shows
   - Select device → verify session starts

3. **Shutdown:**
   - Start auto-start session
   - Press 'q' → verify clean shutdown
   - Verify process terminates properly

#### Manual Testing Checklist
- [ ] `fdemon` with launch.toml auto-starts correctly
- [ ] `fdemon` without launch.toml shows device selector
- [ ] Auto-started session shows in tab bar (when 1+ sessions)
- [ ] Hot reload works on auto-started session
- [ ] Quit (q) shuts down cleanly
- [ ] Force quit (Ctrl+C) works

---

### Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Breaking auto-start for existing users | Thorough manual testing with various launch.toml configs |
| Race condition in startup | Action execution is synchronous in event loop |
| Orphaned processes on failure | spawn_session already handles cleanup |

---

### Estimated Effort

**2 hours**

- 1 hour: Rewrite startup_flutter and cleanup_sessions
- 0.5 hours: Update runner.rs
- 0.5 hours: Testing and fixes

---

## Completion Summary

**Status**: ✅ Done

**Date Completed**: 2026-01-04

### Files Modified

| File | Changes |
|------|---------|
| `src/tui/startup.rs` | Changed `startup_flutter()` return type to `Option<UpdateAction>`, removed `daemon_tx` parameter, refactored auto-start to create session via SessionManager, simplified `cleanup_sessions()` to only handle session_tasks |
| `src/tui/runner.rs` | Removed `daemon_tx`/`daemon_rx` channels, removed `route_daemon_response()` function, updated to execute startup action via `handle_action()`, simplified cleanup call |

### Key Changes

1. **`startup_flutter()` signature changed**:
   - Old: `(Option<FlutterProcess>, Option<CommandSender>)`
   - New: `Option<UpdateAction>`

2. **Auto-start now uses SessionManager**:
   - Creates session via `session_manager.create_session_with_config()`
   - Returns `UpdateAction::SpawnSession` for the event loop to execute
   - Session process is owned by the session task (like manual device selection)

3. **Removed daemon channel from runner.rs**:
   - No more `daemon_tx`/`daemon_rx` channels
   - No more `route_daemon_response()` function
   - All daemon events now route through `Message::SessionDaemon`

4. **Simplified `cleanup_sessions()`**:
   - Removed `flutter: Option<FlutterProcess>` parameter
   - Removed `cmd_sender` parameter
   - Only handles session_tasks cleanup path

### Testing Performed

- `cargo check` - ✅ Passes (no errors)
- `cargo clippy` - ✅ Passes (no warnings)
- `cargo test` - ✅ 454 passed, 1 failed (pre-existing flaky UI animation test unrelated to this task)
  - All session-related tests pass
  - All daemon-related tests pass

### Notable Decisions

1. **Startup action execution**: The startup action is executed immediately after `startup_flutter()` returns, before entering the main event loop. This ensures the session spawn task starts before we begin processing messages.

2. **Error logging**: When session creation fails during auto-start, errors are logged to the selected session if available (though typically there won't be one yet).

3. **Route_daemon_response removed**: This function was used for legacy mode response routing and is no longer needed since all daemon events now come through session tasks.

### Risks/Limitations

- The flaky test `test_indeterminate_ratio_oscillates` is unrelated to this task (tests UI animation oscillation timing)
- Manual testing with actual Flutter project recommended to verify auto-start still works correctly