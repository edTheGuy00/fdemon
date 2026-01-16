## Task: Fix Handler Tests

**Objective**: Update handler tests that reference deleted types (`UiMode::StartupDialog`, `startup_dialog_state`) and deprecated messages.

**Depends on**: 08-remove-deprecated-handlers

**Estimated Time**: 25 minutes

**Priority**: 🟠 Major

**Source**: Architecture Enforcer

### Scope

- `src/app/handler/tests.rs`: Fix tests at lines 555, 1803-1807
- `src/app/handler/keys.rs`: Fix key handler tests at lines 698, 765, 775, 795

### Issues to Fix

**handler/tests.rs:**

1. **Line 555**: `test_close_session_shows_device_selector_when_multiple()`
   - References deleted `DeviceSelector` behavior
   - Either remove test or update to use `NewSessionDialog`

2. **Lines 1803-1807**: Assertions using deleted types
   ```rust
   assert_eq!(state.ui_mode, UiMode::StartupDialog);  // Deleted!
   assert!(state.startup_dialog_state.error.is_some());  // Deleted!
   ```
   - Update to use `UiMode::NewSessionDialog` and `new_session_dialog_state`

**handler/keys.rs tests:**

3. **Line 698**: Test expecting `Message::ShowStartupDialog`
   - Update to expect `Message::OpenNewSessionDialog`

4. **Line 765**: Test expecting `Message::ShowDeviceSelector`
   - Update to expect `Message::OpenNewSessionDialog`

5. **Line 775**: Test expecting `Message::ShowStartupDialog`
   - Update to expect `Message::OpenNewSessionDialog`

6. **Line 795**: Test expecting `Message::ShowDeviceSelector`
   - Update to expect `Message::OpenNewSessionDialog`

### Process

1. Run `cargo test --lib` to identify all compilation errors
2. For each failing test:
   - If test logic is still valid, update assertions to use new types
   - If test behavior is obsolete (DeviceSelector-specific), remove test
3. Run `cargo test --lib` until all tests pass

### Acceptance Criteria

1. `cargo test --lib` compiles without errors
2. All handler tests pass
3. No references to deleted `UiMode::StartupDialog` or `UiMode::DeviceSelector`
4. No references to deleted `startup_dialog_state` or `device_selector` fields
5. Key handler tests verify `OpenNewSessionDialog` behavior

### Testing

```bash
cargo test --lib handler
cargo test --lib keys
```

### Notes

- Some tests may need complete rewriting if their assertions are no longer meaningful
- Consider adding new tests for unified `OpenNewSessionDialog` behavior if coverage gaps exist
- Tests should verify both "with sessions" and "without sessions" paths now produce same result

---

## Completion Summary

**Status:** Not Started
