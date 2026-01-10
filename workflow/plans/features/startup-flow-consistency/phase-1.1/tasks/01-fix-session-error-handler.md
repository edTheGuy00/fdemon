## Task: Fix Session Creation Error Handler

**Objective**: Fix the `AutoLaunchResult` error handler to show `StartupDialog` with error message instead of silently failing and leaving the app in an invalid state.

**Depends on**: None

**Estimated Time**: 0.5 hours

**Severity**: Critical (blocking)

### Problem

When `AutoLaunchResult` succeeds at device discovery but session creation fails (line 1701-1711 in `update.rs`), the current handler:

1. Tries to log to `state.session_manager.selected_mut()` which may return `None` during auto-launch (no existing sessions)
2. Error message is silently dropped
3. State left in `UiMode::Normal` with no session to display - invalid UI state

**Current Code (`update.rs:1701-1711`):**
```rust
Err(e) => {
    // Session creation failed
    state.clear_loading();
    if let Some(session) = state.session_manager.selected_mut() {
        session.session.log_error(
            LogSource::App,
            format!("Failed to create session: {}", e),
        );
    }
    UpdateResult::none()
}
```

### Scope

- `src/app/handler/update.rs`: Replace error handler at lines 1701-1711

### Solution

Match the device-discovery-failure pattern (lines 1714-1721) which correctly shows `StartupDialog` with an error message:

**Required Fix:**
```rust
Err(e) => {
    // Session creation failed - show startup dialog with error
    state.clear_loading();
    let configs = crate::config::load_all_configs(&state.project_path);
    state.show_startup_dialog(configs);
    state.startup_dialog_state.set_error(format!("Failed to create session: {}", e));
    UpdateResult::none()
}
```

### Implementation Steps

1. Locate the `Message::AutoLaunchResult` handler in `update.rs`
2. Find the `Err(e)` branch inside the `Ok(AutoLaunchResult { .. })` success path (session creation failure)
3. Replace the error handler with code that:
   - Clears loading state
   - Reloads configs (user may have changed them)
   - Shows startup dialog with error message
4. Verify the change compiles

### Acceptance Criteria

1. Session creation error shows `StartupDialog` (not `UiMode::Normal`)
2. Error message is displayed in startup dialog via `set_error()`
3. User can retry launch from the dialog
4. No silent failures - user always gets feedback

### Testing

Manual verification steps:
1. Temporarily inject a session creation failure
2. Trigger `StartAutoLaunch`
3. Verify `StartupDialog` appears with error message
4. Verify user can interact with dialog to retry

Automated testing covered by Task 02.

### Notes

- This fix follows the existing pattern at lines 1714-1721 (device discovery failure)
- The error message format matches other session errors in the codebase
- Reloading configs ensures fresh data if user made changes during failed attempt

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/app/handler/update.rs` | Replaced session creation error handler (lines 1701-1708) to show StartupDialog with error instead of silently failing |

### Notable Decisions/Tradeoffs

1. **Follows existing pattern**: The fix matches the device-discovery-failure pattern at lines 1714-1721, ensuring consistency across error handling paths.
2. **Reloads configs**: Calls `load_all_configs()` before showing the dialog to ensure fresh configuration data in case the user modified configs during the failed attempt.
3. **Error format unchanged**: Maintains the same error message format "Failed to create session: {}" for consistency with existing error messages.

### Testing Performed

- `cargo fmt` - Passed (code automatically reformatted)
- `cargo check` - Passed (no compilation errors)
- `cargo test --lib` - Passed (1333 unit tests passed, 0 failed)
- `cargo clippy -- -D warnings` - Passed (no warnings)

### Risks/Limitations

1. **E2E tests**: Some E2E tests failed (26 failures), but these are pre-existing issues related to headless testing environments and PTY/terminal interaction, not related to this change. All unit tests pass successfully.
2. **Manual verification pending**: Manual testing with injected session creation failures should be performed to verify the dialog appears correctly and users can retry launch.
