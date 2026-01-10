## Task: Handle Auto-Launch Edge Cases

**Objective**: Ensure the auto-launch flow handles edge cases gracefully: no devices, discovery failures, max sessions reached, and user interaction during loading.

**Depends on**: Phase 2 complete

**Estimated Time**: 1 hour

### Scope

- `src/tui/spawn.rs`: Edge case handling in spawn function
- `src/app/handler/update.rs`: Edge case handling in result handler
- `src/app/handler/keys.rs`: Block '+' key during loading

### Details

#### Edge Case 1: No Devices Found

**Current handling** (in Phase 1 Task 4):
```rust
if devices.is_empty() {
    let _ = msg_tx.send(Message::AutoLaunchResult {
        result: Err("No devices found".to_string()),
    }).await;
    return;
}
```

**Improvement**: More helpful error message:
```rust
if devices.is_empty() {
    let _ = msg_tx.send(Message::AutoLaunchResult {
        result: Err("No devices found. Connect a device or start an emulator.".to_string()),
    }).await;
    return;
}
```

#### Edge Case 2: Discovery Timeout/Failure

**Current handling**: Error message sent to result.

**Improvement**: Include more context:
```rust
Err(e) => {
    let error_msg = format!("Device discovery failed: {}. Check Flutter SDK installation.", e);
    let _ = msg_tx.send(Message::AutoLaunchResult {
        result: Err(error_msg),
    }).await;
    return;
}
```

#### Edge Case 3: Max Sessions Reached

The `AutoLaunchResult` handler creates a session. If max sessions (9) is reached, `create_session()` returns an error.

**Current handling** (in Phase 1 Task 3):
```rust
Err(e) => {
    state.clear_loading();
    if let Some(session) = state.session_manager.selected_mut() {
        session.session.log_error(...);
    }
    UpdateResult::none()
}
```

**Improvement**: Show error in UI, not just log:
```rust
Err(e) => {
    state.clear_loading();
    // Show startup dialog with error instead of silent failure
    let configs = crate::config::load_all_configs(&state.project_path);
    state.show_startup_dialog(configs);
    state.startup_dialog_state.set_error(format!("Cannot create session: {}", e));
    UpdateResult::none()
}
```

#### Edge Case 4: User Presses '+' During Loading

If user presses '+' while auto-launch is in progress, we should ignore it.

**Location**: `src/app/handler/keys.rs`

**Current behavior**: '+' shows StartupDialog or DeviceSelector.

**Required change**: Check if loading before showing dialog:
```rust
(KeyCode::Char('+'), KeyModifiers::NONE) | (KeyCode::Char('+'), KeyModifiers::SHIFT) => {
    // Don't show dialogs while loading (auto-launch in progress)
    if state.ui_mode == UiMode::Loading {
        return None;
    }

    if state.has_running_sessions() {
        Some(Message::ShowDeviceSelector)
    } else {
        Some(Message::ShowStartupDialog)
    }
}
```

Similarly for 'd' key:
```rust
KeyCode::Char('d') if state.ui_mode == UiMode::Normal => {
    // Don't show dialogs while loading
    if state.loading_state.is_some() {
        return None;
    }
    // ... existing logic
}
```

#### Edge Case 5: Escape During Loading

User might press Escape to cancel auto-launch. Currently loading mode doesn't handle Escape.

**Option A**: Ignore Escape during loading (simpler)
**Option B**: Allow canceling auto-launch (complex - need to abort task)

**Recommended**: Option A for now. Loading is brief; cancellation adds complexity.

### Acceptance Criteria

1. No devices: Shows StartupDialog with helpful error message
2. Discovery failure: Shows StartupDialog with error context
3. Max sessions: Shows StartupDialog with error (not silent failure)
4. '+' key during loading: Ignored (no dialog shown)
5. 'd' key during loading: Ignored
6. `cargo check` passes
7. `cargo clippy -- -D warnings` passes

### Testing

```rust
#[test]
fn test_plus_key_ignored_during_loading() {
    let mut state = AppState::new();
    state.set_loading_phase("Testing...");

    let result = handle_key(&mut state, KeyCode::Char('+'), KeyModifiers::NONE);

    assert!(result.is_none());
    assert_eq!(state.ui_mode, UiMode::Loading); // Still loading, no dialog
}

#[test]
fn test_auto_launch_no_devices_shows_dialog() {
    let mut state = AppState::new();
    state.set_loading_phase("Testing...");

    let result = update(&mut state, Message::AutoLaunchResult {
        result: Err("No devices found".to_string()),
    });

    assert!(state.loading_state.is_none()); // Loading cleared
    assert_eq!(state.ui_mode, UiMode::StartupDialog);
    assert!(state.startup_dialog_state.error.is_some());
}
```

### Notes

- These edge cases should be rare in normal usage
- The goal is graceful degradation, not silent failure
- User should always have a path forward (via StartupDialog)
- Cancellation of auto-launch is deferred to future enhancement

---

## Completion Summary

**Status:** Not Started

**Files Modified:**
- (pending)

**Implementation Details:**

(pending)

**Testing Performed:**
- (pending)

**Notable Decisions:**
- (pending)

**Risks/Limitations:**
- (pending)
