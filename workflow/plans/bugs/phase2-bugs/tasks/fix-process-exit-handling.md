## Task: Fix Process Exit Handling

**Objective**: When the Flutter process exits (externally closed, crashed, etc.), Flutter Demon should exit gracefully instead of staying in a "Loading" state indefinitely.

**Depends on**: None (independent fix)

### Scope

- `src/app/handler.rs`: Change `DaemonEvent::Exited` handling to set `AppPhase::Quitting`

### Problem

Currently in `handle_daemon_event()` (lines 289-301):

```rust
DaemonEvent::Exited { code } => {
    let (level, message) = match code {
        Some(0) => (LogLevel::Info, "Flutter process exited normally".to_string()),
        Some(c) => (LogLevel::Warning, format!("Flutter process exited with code {}", c)),
        None => (LogLevel::Warning, "Flutter process exited".to_string()),
    };
    state.add_log(LogEntry::new(level, LogSource::App, message));
    state.phase = AppPhase::Initializing;  // <-- BUG: Should be Quitting
}
```

When the Flutter process exits, the phase is set to `Initializing`, which leaves Flutter Demon running with nothing to control. The `should_quit()` method only returns `true` for `AppPhase::Quitting`.

### Implementation

1. Change line 299 from:
   ```rust
   state.phase = AppPhase::Initializing;
   ```
   To:
   ```rust
   state.phase = AppPhase::Quitting;
   ```

2. Optionally, add an info log indicating Flutter Demon is exiting:
   ```rust
   state.add_log(LogEntry::info(LogSource::App, "Exiting Flutter Demon..."));
   ```

### Acceptance Criteria

1. When Flutter app is closed externally (e.g., stop simulator, close app window), Flutter Demon exits
2. When Flutter process crashes, Flutter Demon exits with appropriate log message
3. Exit code from Flutter process is logged correctly (0 = normal, non-zero = warning)
4. No orphan Flutter Demon processes after app closes

### Testing

**Manual Testing:**
1. Start Flutter Demon with a Flutter project
2. Wait for app to launch on simulator/device
3. Stop the simulator OR close the app from the device
4. Verify Flutter Demon exits gracefully (returns to shell)
5. Verify log shows "Flutter process exited" message

**Unit Testing:**
- Update existing test `test_daemon_exited_event_logs_message` to verify phase is `Quitting`
- Add test: `test_daemon_exited_sets_quitting_phase`

```rust
#[test]
fn test_daemon_exited_sets_quitting_phase() {
    let mut state = AppState::new();
    state.phase = AppPhase::Running;
    
    let event = DaemonEvent::Exited { code: Some(0) };
    handle_daemon_event(&mut state, event);
    
    assert_eq!(state.phase, AppPhase::Quitting);
}
```

### Estimated Time

15-30 minutes

### Notes

- This is the simplest fix of all 4 bugs
- Future enhancement: Instead of auto-quit, show a prompt offering to restart the app
- Consider: Should we differentiate between normal exit (code 0) and crash (non-zero)? For now, exit in both cases.

---

## Completion Summary

**Status:** ✅ Done

**Files Modified:**
- `src/app/handler.rs` - Fixed `DaemonEvent::Exited` handler and added tests

**Implementation Details:**
- Changed `state.phase = AppPhase::Initializing;` to `state.phase = AppPhase::Quitting;` (line 300)
- Added info log "Exiting Flutter Demon..." before setting phase to Quitting
- Added 2 new unit tests:
  - `test_daemon_exited_sets_quitting_phase` - verifies phase is Quitting and `should_quit()` returns true
  - `test_daemon_exited_with_error_code_sets_quitting` - verifies non-zero exit codes are logged and still trigger quit

**Testing Performed:**
- `cargo check` - PASS
- `cargo test` - PASS (220 tests, +2 new)
- `cargo clippy` - PASS (no warnings)

**Risks/Limitations:**
- Manual testing recommended to verify end-to-end behavior
- Future enhancement could offer restart prompt instead of auto-exit