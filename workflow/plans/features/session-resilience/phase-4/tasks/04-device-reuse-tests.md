## Task: Add device reuse tests for handle_launch

**Objective**: Add tests that verify the fixed behavior — stopped sessions allow device reuse, active sessions still block it. Also replace the dead `test_device_selected_prevents_duplicate` stub.

**Depends on**: 03-update-launch-guard

### Scope

- `crates/fdemon-app/src/handler/new_session/launch_context.rs`: Add tests to existing `#[cfg(test)] mod tests` block (after line ~1077)
- `crates/fdemon-app/src/handler/tests.rs`: Remove or update the dead `test_device_selected_prevents_duplicate` stub (line 335)

### Details

#### New tests in `launch_context.rs`

These tests need a helper to set up an `AppState` with:
1. A pre-existing session for a specific device
2. A `new_session_dialog_state` with the same device selected

```rust
#[test]
fn test_handle_launch_allows_device_reuse_when_session_stopped() {
    // Setup: Create state with a stopped session for "macos" device
    let mut state = AppState::default();
    let id = state.session_manager
        .create_session("macos", "macOS", "macos", false)
        .unwrap();
    state.session_manager.get_mut(id).unwrap().session.phase = AppPhase::Stopped;

    // Configure new session dialog to select the same device
    // ... (set up target_selector with connected device "macos")

    let result = handle_launch(&mut state);

    // Should succeed — returns SpawnSession action, not none()
    assert!(result.action.is_some());
}

#[test]
fn test_handle_launch_blocks_device_with_running_session() {
    // Setup: Create state with a running session for "macos" device
    let mut state = AppState::default();
    let id = state.session_manager
        .create_session("macos", "macOS", "macos", false)
        .unwrap();
    state.session_manager.get_mut(id).unwrap().session.phase = AppPhase::Running;

    // Configure new session dialog to select the same device
    // ... (set up target_selector with connected device "macos")

    let result = handle_launch(&mut state);

    // Should fail — returns none() with error set
    assert!(result.action.is_none());
    assert!(state.new_session_dialog_state.target_selector.error_message
        .as_ref()
        .unwrap()
        .contains("already has an active session"));
}

#[test]
fn test_handle_launch_blocks_device_with_initializing_session() {
    // Setup: Create state with an initializing session (default phase)
    let mut state = AppState::default();
    state.session_manager
        .create_session("macos", "macOS", "macos", false)
        .unwrap();

    // Configure new session dialog to select the same device

    let result = handle_launch(&mut state);

    // Should fail — initializing sessions occupy the device
    assert!(result.action.is_none());
}
```

#### Dead stub cleanup in `handler/tests.rs`

The `test_device_selected_prevents_duplicate` stub at line 335 is `#[ignore]`'d and empty. Either:
- Remove it entirely (the `DeviceSelected` message is deprecated)
- Or replace the comment with a pointer to the new tests

### Acceptance Criteria

1. Test verifies stopped session allows device reuse (returns `SpawnSession` action)
2. Test verifies running session blocks device reuse (returns `none()` with error)
3. Test verifies initializing session blocks device reuse
4. Dead test stub cleaned up
5. All tests pass: `cargo test -p fdemon-app`

### Testing

```bash
cargo test -p fdemon-app -- test_handle_launch_allows_device_reuse
cargo test -p fdemon-app -- test_handle_launch_blocks_device
cargo test -p fdemon-app -- --test-threads=1  # full suite
```

### Notes

- Follow the existing test patterns in `launch_context.rs` (lines 889–1077) for `AppState` setup.
- The test setup for `new_session_dialog_state` requires populating `target_selector.connected_devices` and calling `build_launch_params()` — look at existing tests for the pattern.
- The `error_message` field name on `TargetSelectorState` may be named differently (e.g., `error`) — verify against the struct definition.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/new_session/launch_context.rs` | Added 3 new tests in the `#[cfg(test)] mod tests` block: `test_handle_launch_allows_device_reuse_when_session_stopped`, `test_handle_launch_blocks_device_with_running_session`, `test_handle_launch_blocks_device_with_initializing_session` |
| `crates/fdemon-app/src/handler/tests.rs` | Replaced the dead `test_device_selected_prevents_duplicate` stub with a comment pointing to the 3 new tests |

### Notable Decisions/Tradeoffs

1. **`error` not `error_message`**: The task description mentioned `target_selector.error_message` but the struct field is named `error`. The tests assert against `state.new_session_dialog_state.target_selector.error` per the actual struct definition in `target_selector_state.rs`.
2. **Used existing `test_device()` helper**: The task pseudocode used `create_session("macos", ...)` but the actual signature is `create_session(&Device)`. All three tests reuse the existing `test_device()` helper (device id `"emulator-5554"`) and populate the dialog's `connected_devices` with the same device.
3. **Dead stub replaced with comment**: Rather than keeping an empty `#[ignore]` test, the stub was replaced with a doc comment pointing to the three new tests that cover this scenario.

### Testing Performed

- `cargo test -p fdemon-app -- test_handle_launch_allows_device_reuse` - Passed (1 test)
- `cargo test -p fdemon-app -- test_handle_launch_blocks_device` - Passed (2 tests)
- `cargo test -p fdemon-app -- --test-threads=1` - Passed (1152 tests, 4 ignored, 0 failed)

### Risks/Limitations

1. **None**: All acceptance criteria met. The three tests exercise the `find_active_by_device_id` guard introduced in task 03 and confirm it correctly allows reuse for stopped sessions while blocking initializing and running sessions.
