## Task: Update Keybinding Unit Tests

**Objective**: Update all unit tests that verify 'n' key behavior to test '+' key for new session functionality.

**Depends on**: 01-replace-n-with-plus

### Scope

- `src/app/handler/keys.rs`: Update tests in `device_selector_key_tests` module (lines 791-865)
- `src/app/handler/tests.rs`: Update any related handler tests

### Details

**Tests to update in `keys.rs` (device_selector_key_tests module):**

1. `test_n_key_with_running_sessions_no_search` (line 820) → `test_plus_key_with_running_sessions`
2. `test_n_key_without_sessions` (line 837) → `test_plus_key_without_sessions`
3. `test_n_key_with_search_query` (line 847) - Keep but update expectations

**Renamed/modified tests:**

```rust
#[test]
fn test_plus_key_with_running_sessions() {
    use crate::core::AppPhase;

    let mut state = AppState::new();
    let device = test_device();
    let session_id = state.session_manager.create_session(&device).unwrap();
    // Mark session as running
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        handle.session.phase = AppPhase::Running;
    }

    let msg = handle_key_normal(&state, key(KeyCode::Char('+')));

    assert!(matches!(msg, Some(Message::ShowDeviceSelector)));
}

#[test]
fn test_plus_key_without_sessions() {
    let state = AppState::new();
    // No running sessions

    let msg = handle_key_normal(&state, key(KeyCode::Char('+')));

    assert!(matches!(msg, Some(Message::ShowStartupDialog)));
}

#[test]
fn test_n_key_without_search_does_nothing() {
    use crate::core::AppPhase;

    let mut state = AppState::new();
    let device = test_device();
    let session_id = state.session_manager.create_session(&device).unwrap();
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        handle.session.phase = AppPhase::Running;
    }

    // No search query active
    let msg = handle_key_normal(&state, key(KeyCode::Char('n')));

    // Should return None, not ShowDeviceSelector
    assert!(msg.is_none(), "n key should do nothing without active search");
}

#[test]
fn test_n_key_with_search_query() {
    let mut state = AppState::new();
    let device = test_device();
    let session_id = state.session_manager.create_session(&device).unwrap();

    // Set search query
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        handle.session.search_state.query = "test query".to_string();
    }

    // Select the session
    state.session_manager.select_by_id(session_id);

    let msg = handle_key_normal(&state, key(KeyCode::Char('n')));

    // Should trigger next search match
    assert!(matches!(msg, Some(Message::NextSearchMatch)));
}
```

**Also check `handler/tests.rs`** for any tests that reference 'n' key for session management.

### Acceptance Criteria

1. Old tests renamed/updated to test '+' key
2. New test added: `test_n_key_without_search_does_nothing`
3. All tests in `device_selector_key_tests` module pass
4. All tests in `handler/tests.rs` pass
5. No references to 'n' key for session management remain in test names/comments

### Testing

```bash
cargo test device_selector_key_tests -- --nocapture
cargo test handler -- --nocapture
cargo test --lib
```

### Notes

- Check for any E2E tests in `tests/e2e/` that may reference 'n' key (will be handled in Phase 3)
- The old test for 'n' without sessions should be replaced, not just commented out
- Add test documentation comments explaining the new keybinding behavior

---

## Completion Summary

**Status:** Not Started

**Files Modified:**
- (To be filled after implementation)

**Implementation Details:**
(To be filled after implementation)

**Testing Performed:**
- `cargo fmt` - Pending
- `cargo check` - Pending
- `cargo clippy` - Pending
- `cargo test` - Pending
