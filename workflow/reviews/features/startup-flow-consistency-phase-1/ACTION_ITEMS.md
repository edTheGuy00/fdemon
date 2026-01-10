# Action Items: Startup Flow Consistency - Phase 1

**Review Date:** 2026-01-10
**Verdict:** NEEDS WORK
**Blocking Issues:** 1

## Critical Issues (Must Fix)

### 1. Fix Session Creation Error Handler

- **Source:** Logic Reasoning Checker, Risks/Tradeoffs Analyzer
- **File:** `src/app/handler/update.rs`
- **Lines:** 1701-1711
- **Problem:** When `AutoLaunchResult` succeeds but session creation fails, the handler attempts to log to a session that may not exist, and leaves the UI in an invalid state (Normal mode with no session).
- **Required Action:** Replace the current error handler with logic that shows the StartupDialog with an error message, matching the device-discovery-failure path.

**Current Code:**
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

- **Acceptance:** User sees StartupDialog with error message when session creation fails; no silent failures.

## Major Issues (Should Fix)

### 1. Add AutoLaunchResult Handler Test

- **Source:** Risks/Tradeoffs Analyzer
- **File:** `src/app/handler/tests.rs`
- **Problem:** The `AutoLaunchResult` handler contains complex state transition logic but has no unit tests. This is inconsistent with testing standards for similar handlers.
- **Suggested Action:** Add at least two tests:
  1. Test success path: loading cleared, mode = Normal, SpawnSession action returned
  2. Test error path: StartupDialog shown with error message

**Example Test Skeleton:**
```rust
#[test]
fn test_auto_launch_result_success_creates_session() {
    use crate::daemon::Device;

    let mut state = AppState::new();
    state.set_loading_phase("Testing");

    let device = Device {
        id: "test-device".to_string(),
        name: "Test Device".to_string(),
        platform: "android".to_string(),
        emulator: false,
        emulator_id: None,
        ephemeral: false,
        category: None,
        platform_type: None,
    };

    let success = AutoLaunchSuccess {
        device: device.clone(),
        config: None,
    };

    let result = update(&mut state, Message::AutoLaunchResult { result: Ok(success) });

    assert!(state.loading_state.is_none());
    assert_eq!(state.ui_mode, UiMode::Normal);
    assert!(matches!(result.action, Some(UpdateAction::SpawnSession { .. })));
}

#[test]
fn test_auto_launch_result_error_shows_startup_dialog() {
    let mut state = AppState::new();
    state.set_loading_phase("Testing");

    let result = update(&mut state, Message::AutoLaunchResult {
        result: Err("Test error".to_string()),
    });

    assert!(state.loading_state.is_none());
    assert_eq!(state.ui_mode, UiMode::StartupDialog);
    assert!(state.startup_dialog_state.error.is_some());
}
```

## Minor Issues (Consider Fixing)

### 1. Use `expect()` Instead of `unwrap()` for Better Panic Messages

- **Source:** Code Quality Inspector
- **File:** `src/tui/spawn.rs`
- **Line:** 205
- **Problem:** The `unwrap()` has a safety comment but no descriptive message for debugging.
- **Suggested Action:** Change to `expect()` with descriptive message.

**Current:**
```rust
device: devices.first().unwrap().clone(), // Safe: we checked devices.is_empty() above
```

**Suggested:**
```rust
device: devices.first().expect("devices non-empty; checked at line 137").clone(),
```

### 2. Add Validation Warnings for Index Mismatches

- **Source:** Risks/Tradeoffs Analyzer
- **File:** `src/tui/spawn.rs`
- **Lines:** 174-180
- **Problem:** If saved config index is out of bounds in current configs, it silently falls through to Priority 2.
- **Suggested Action:** Add a warning log when index lookup fails.

### 3. Document Device Fallback Behavior

- **Source:** Risks/Tradeoffs Analyzer
- **File:** `src/tui/spawn.rs`
- **Lines:** 189-200
- **Problem:** If configured device is not found, silently falls back to first device.
- **Suggested Action:** Add warning log when device fallback occurs.

## Re-review Checklist

After addressing issues, the following must pass:

- [ ] Critical issue #1 resolved (session creation error handler fixed)
- [ ] Error path now shows StartupDialog with error message
- [ ] `cargo fmt` passes
- [ ] `cargo check` passes
- [ ] `cargo test --lib` passes
- [ ] `cargo clippy -- -D warnings` passes
- [ ] Manual verification: Simulate session creation failure scenario

## Notes

- This is Phase 1 (infrastructure only) - actual behavior change happens in Phase 2
- The E2E test failures mentioned in task files are pre-existing and unrelated to these changes
- The pre-existing TUI import in `update.rs` (line 6) is noted but not a blocker
