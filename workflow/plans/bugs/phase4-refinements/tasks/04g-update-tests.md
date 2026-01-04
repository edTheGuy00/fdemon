## Task 4g: Update and Remove Obsolete Tests

**Objective**: Fix all tests that rely on legacy single-session behavior. Remove tests that test removed functionality and update tests to use session-based approach.

**Depends on**: Tasks 4a-4f (all code changes must be complete)

---

### Background

After removing legacy code in tasks 4a-4f, many tests in `src/app/handler/tests.rs` will fail because they:

1. Use `Message::Daemon` variant (removed in 4b)
2. Set `state.current_app_id` directly (field removed in 4e)
3. Use `state.logs` or `state.log_view_state` (fields removed in 4e)
4. Call legacy methods like `state.start_reload()` (removed in 4e)
5. Test legacy fallback behavior (removed in 4c)
6. Test legacy global state updates (removed in 4d)

---

### Scope

#### `src/app/handler/tests.rs`

**Tests to REMOVE entirely:**

| Test Name | Reason |
|-----------|--------|
| `test_daemon_exited_event_logs_message` | Uses `Message::Daemon` |
| `test_daemon_exited_sets_quitting_phase` | Uses `Message::Daemon` |
| `test_daemon_exited_with_error_code_sets_quitting` | Uses `Message::Daemon` |
| `test_session_started_updates_legacy_global_state` | Tests legacy global state updates |
| `test_auto_reload_falls_back_to_legacy` | Tests legacy fallback behavior |

---

**Tests to UPDATE to use sessions:**

| Test Name | Required Changes |
|-----------|------------------|
| `test_hot_reload_message_starts_reload` | Create session with app_id and cmd_sender |
| `test_hot_reload_without_app_id_shows_error` | Check session logs, not global logs |
| `test_hot_reload_ignored_when_busy` | Use session's is_busy, not global |
| `test_reload_ignored_when_already_reloading` | Use session phase |
| `test_restart_ignored_when_already_reloading` | Use session phase |
| `test_stop_ignored_when_already_reloading` | Use session phase |
| `test_reload_no_app_running_shows_error` | Check session logs |
| `test_restart_no_app_running_shows_error` | Check session logs |
| `test_stop_no_app_running_shows_error` | Check session logs |
| `test_auto_reload_triggered_when_app_running` | Use session with app_id |
| `test_auto_reload_skipped_when_no_app` | No sessions, verify no action |
| `test_auto_reload_skipped_when_busy` | Session is busy |
| `test_reload_elapsed_tracking` | Use session's reload_start_time |
| `test_reload_uses_session_when_no_cmd_sender` | Update assertion |
| `test_stop_app_spawns_task` | Use session, not global state |
| `test_stop_app_without_app_id_shows_error` | Check session logs |

---

### Implementation Patterns

#### Pattern 1: Create session with app_id for reload/restart tests

```rust
// Before (legacy):
fn test_hot_reload_message_starts_reload() {
    let mut state = AppState::new();
    state.current_app_id = Some("test-app-id".to_string());

    let result = update(&mut state, Message::HotReload);

    assert!(state.is_busy());
    // ...
}

// After (session-based):
fn test_hot_reload_message_starts_reload() {
    let mut state = AppState::new();
    
    // Create session with device
    let device = test_device("device-1", "Test Device");
    let session_id = state.session_manager.create_session(&device).unwrap();
    state.session_manager.select_by_id(session_id);
    
    // Set up session with app_id (simulates app.start event received)
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        handle.session.app_id = Some("test-app-id".to_string());
        handle.session.phase = AppPhase::Running;
        // Note: cmd_sender would be needed for actual action execution
        // For update() testing, we just check the action is returned
    }

    let result = update(&mut state, Message::HotReload);

    // Check session is busy, not global state
    if let Some(handle) = state.session_manager.get(session_id) {
        assert!(handle.session.is_busy());
    }
    
    assert!(matches!(
        result.action,
        Some(UpdateAction::SpawnTask(Task::Reload { session_id: sid, .. })) if sid == session_id
    ));
}
```

---

#### Pattern 2: Check session logs instead of global logs

```rust
// Before (legacy):
fn test_reload_no_app_running_shows_error() {
    let mut state = AppState::new();
    state.current_app_id = None;

    update(&mut state, Message::HotReload);

    assert!(state.logs.iter().any(|e| e.message.contains("No app running")));
}

// After (session-based):
fn test_reload_no_app_running_shows_error() {
    let mut state = AppState::new();
    
    // Create session without app_id
    let device = test_device("device-1", "Test Device");
    let session_id = state.session_manager.create_session(&device).unwrap();
    state.session_manager.select_by_id(session_id);

    update(&mut state, Message::HotReload);

    // Check session logs
    if let Some(handle) = state.session_manager.get(session_id) {
        assert!(handle.session.logs.iter().any(|e| e.message.contains("No app running")));
    }
}
```

---

#### Pattern 3: Use session phase for busy checks

```rust
// Before (legacy):
fn test_hot_reload_ignored_when_busy() {
    let mut state = AppState::new();
    state.current_app_id = Some("test-app-id".to_string());
    state.phase = AppPhase::Reloading;

    let result = update(&mut state, Message::HotReload);

    assert!(result.action.is_none());
}

// After (session-based):
fn test_hot_reload_ignored_when_busy() {
    let mut state = AppState::new();
    
    let device = test_device("device-1", "Test Device");
    let session_id = state.session_manager.create_session(&device).unwrap();
    state.session_manager.select_by_id(session_id);
    
    // Set session as busy
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        handle.session.app_id = Some("test-app-id".to_string());
        handle.session.phase = AppPhase::Reloading;
    }

    let result = update(&mut state, Message::HotReload);

    assert!(result.action.is_none());
}
```

---

#### Pattern 4: Test no action when no sessions

```rust
// Before (legacy):
fn test_auto_reload_skipped_when_no_app() {
    let mut state = AppState::new();
    state.current_app_id = None;

    let result = update(&mut state, Message::AutoReloadTriggered);

    assert!(result.action.is_none());
}

// After (session-based):
fn test_auto_reload_skipped_when_no_sessions() {
    let mut state = AppState::new();
    // No sessions created

    let result = update(&mut state, Message::AutoReloadTriggered);

    assert!(result.action.is_none());
}
```

---

### Helper Function Updates

#### Update `test_device` helper if not already present:

```rust
fn test_device(id: &str, name: &str) -> Device {
    Device {
        id: id.to_string(),
        name: name.to_string(),
        platform: "android".to_string(),
        emulator: false,
        sdk: None,
        category: Some("mobile".to_string()),
        platform_type: None,
        ephemeral: None,
    }
}
```

#### Add helper to create session with full setup:

```rust
fn create_test_session_with_app(state: &mut AppState, app_id: &str) -> SessionId {
    let device = test_device("test-device", "Test Device");
    let session_id = state.session_manager.create_session(&device).unwrap();
    state.session_manager.select_by_id(session_id);
    
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        handle.session.app_id = Some(app_id.to_string());
        handle.session.phase = AppPhase::Running;
    }
    
    session_id
}
```

---

### Implementation Steps

1. **Identify all failing tests**
   ```bash
   cargo test 2>&1 | grep "FAILED"
   ```

2. **Remove tests for deleted functionality**
   - Delete the 5 tests listed in "Tests to REMOVE"

3. **Update test helper functions**
   - Ensure `test_device()` exists
   - Add `create_test_session_with_app()` helper

4. **Update each failing test**
   - Follow patterns above
   - Use sessions instead of global state
   - Check session logs instead of global logs

5. **Run tests incrementally**
   ```bash
   cargo test test_hot_reload
   cargo test test_reload
   cargo test test_restart
   cargo test test_stop
   cargo test test_auto_reload
   ```

6. **Run full test suite**
   ```bash
   cargo test
   ```

7. **Fix any clippy warnings in tests**
   ```bash
   cargo clippy --tests
   ```

---

### Files Changed Summary

| File | Tests Removed | Tests Updated |
|------|---------------|---------------|
| `tests.rs` | 5 | ~15+ |

**Estimated lines changed: ~200-300 lines**

---

### Acceptance Criteria

1. ✅ All tests using `Message::Daemon` removed or updated
2. ✅ All tests using `state.current_app_id` updated to use sessions
3. ✅ All tests using `state.logs` updated to use session logs
4. ✅ All tests using `state.phase` updated (except should_quit tests)
5. ✅ All legacy fallback tests removed
6. ✅ All legacy global state update tests removed
7. ✅ `cargo test` passes with no failures
8. ✅ `cargo clippy --tests` shows no warnings
9. ✅ Test coverage maintained for session-based functionality

---

### Testing

#### Verification Commands

```bash
# Run all tests
cargo test

# Run with verbose output
cargo test -- --nocapture

# Run specific test group
cargo test test_hot_reload

# Check for clippy warnings in tests
cargo clippy --tests

# Ensure no legacy patterns remain in tests
grep -n "state\.current_app_id" src/app/handler/tests.rs
grep -n "state\.logs" src/app/handler/tests.rs
grep -n "Message::Daemon" src/app/handler/tests.rs
# All should return no matches
```

---

### Edge Cases

1. **Tests that need cmd_sender**
   - Some tests may need a mock CommandSender
   - For update() tests, we typically just check the action returned
   - Actual command execution tested in integration tests

2. **Tests with multiple sessions**
   - Ensure multi-session tests still work
   - Verify correct session is targeted by actions

3. **Session not found**
   - Test graceful handling when session_id is invalid

---

### Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Missing test coverage | Review each removed test - is functionality still tested? |
| Breaking existing tests | Update incrementally, run after each change |
| New bugs from test changes | Ensure test assertions are correct |

---

### Test Count Summary

Before removal:
- ~50+ tests in tests.rs

After update:
- 5 tests removed
- ~15+ tests updated
- ~45+ tests remain

---

### Estimated Effort

**1.5 hours**

- 0.5 hours: Remove obsolete tests
- 0.75 hours: Update remaining tests
- 0.25 hours: Verify all pass and fix issues