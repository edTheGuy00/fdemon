## Task: Create Session in Manager on Device Select

**Objective**: Modify the `DeviceSelected` message handler to create a session in `SessionManager` BEFORE returning the `SpawnSession` action, so the spawned process can be properly associated with a managed session.

**Depends on**: Task 01 (SessionId in SpawnSession)

---

### Scope

- `src/app/handler.rs`: Update `Message::DeviceSelected` handler
- `src/app/state.rs`: Ensure session_manager is accessible

---

### Current State

```rust
// In src/app/handler.rs - Message::DeviceSelected handler
Message::DeviceSelected { device } => {
    state.log_info(
        LogSource::App,
        format!("Device selected: {} ({})", device.name, device.id),
    );

    // Hide selector and switch to normal mode
    state.device_selector.hide();
    state.ui_mode = UiMode::Normal;

    // Return action to spawn session - BUT NO SESSION CREATED IN MANAGER!
    UpdateResult::action(UpdateAction::SpawnSession {
        device,
        config: None,
    })
}
```

**Problem:** The session is never created in `SessionManager`, so:
- No session appears in tabs
- No session-specific logging
- Spawned process has no association

---

### Implementation Details

#### 1. Update DeviceSelected Handler

```rust
Message::DeviceSelected { device } => {
    // Check if device already has a running session
    if let Some(_existing_id) = state.session_manager.find_by_device_id(&device.id) {
        state.log_error(
            LogSource::App,
            format!("Device '{}' already has an active session", device.name),
        );
        // Stay in device selector to pick another device
        return UpdateResult::none();
    }
    
    // Create session in manager FIRST
    match state.session_manager.create_session(&device) {
        Ok(session_id) => {
            state.log_info(
                LogSource::App,
                format!("Session created for {} (id: {})", device.name, session_id),
            );

            // Hide selector and switch to normal mode
            state.device_selector.hide();
            state.ui_mode = UiMode::Normal;

            // Return action to spawn session WITH the session_id
            UpdateResult::action(UpdateAction::SpawnSession {
                session_id,
                device,
                config: None,
            })
        }
        Err(e) => {
            // Max sessions reached or other error
            state.log_error(LogSource::App, format!("Failed to create session: {}", e));
            UpdateResult::none()
        }
    }
}
```

#### 2. Handle Device-with-Config Case (if exists)

If there are any other places that create SpawnSession (e.g., auto-start), update them similarly:

```rust
// Example for auto-start config path
let session_id = state.session_manager.create_session(&device)?;
UpdateAction::SpawnSession {
    session_id,
    device,
    config: Some(Box::new(config)),
}
```

#### 3. Session Manager Already Limits Sessions

The `SessionManager::create_session` already checks `MAX_SESSIONS` (9) and returns an error if exceeded. This provides natural rate limiting.

---

### Session Creation Flow After This Task

```
User selects device
         │
         ▼
DeviceSelected { device }
         │
         ▼
Check: device already has session? ──Yes──► Log error, stay in selector
         │
         No
         ▼
session_manager.create_session(&device)
         │
    ┌────┴────┐
    │         │
   Err       Ok(session_id)
    │         │
    ▼         ▼
Log error   Hide selector
Return none Set UiMode::Normal
            Return SpawnSession { session_id, device, config }
```

---

### Acceptance Criteria

1. [ ] `DeviceSelected` handler creates session before spawning
2. [ ] Duplicate device selection is prevented with error message
3. [ ] Session appears in `session_manager` immediately after selection
4. [ ] `SpawnSession` action includes valid `session_id`
5. [ ] Max sessions (9) limit is enforced
6. [ ] Session tabs appear in UI when multiple sessions exist

---

### Testing

```rust
#[test]
fn test_device_selected_creates_session() {
    let mut state = AppState::new();
    state.ui_mode = UiMode::DeviceSelector;
    
    let device = Device {
        id: "device-1".to_string(),
        name: "Test Device".to_string(),
        platform: "ios".to_string(),
        emulator: false,
        category: None,
        platform_type: None,
        ephemeral: false,
        emulator_id: None,
    };
    
    let result = update(&mut state, Message::DeviceSelected { device: device.clone() });
    
    // Session should be created
    assert_eq!(state.session_manager.len(), 1);
    
    // Should return SpawnSession action
    assert!(matches!(result.action, Some(UpdateAction::SpawnSession { .. })));
    
    // UI mode should be Normal
    assert_eq!(state.ui_mode, UiMode::Normal);
}

#[test]
fn test_device_selected_prevents_duplicate() {
    let mut state = AppState::new();
    state.ui_mode = UiMode::DeviceSelector;
    
    let device = Device {
        id: "device-1".to_string(),
        name: "Test Device".to_string(),
        platform: "ios".to_string(),
        emulator: false,
        category: None,
        platform_type: None,
        ephemeral: false,
        emulator_id: None,
    };
    
    // First selection succeeds
    let _ = update(&mut state, Message::DeviceSelected { device: device.clone() });
    assert_eq!(state.session_manager.len(), 1);
    
    // Show device selector again
    state.ui_mode = UiMode::DeviceSelector;
    
    // Second selection of same device should fail
    let result = update(&mut state, Message::DeviceSelected { device });
    
    // Should NOT create another session
    assert_eq!(state.session_manager.len(), 1);
    
    // Should return no action
    assert!(result.action.is_none());
}

#[test]
fn test_max_sessions_enforced() {
    let mut state = AppState::new();
    
    // Create MAX_SESSIONS (9) sessions
    for i in 0..9 {
        let device = Device {
            id: format!("device-{}", i),
            name: format!("Device {}", i),
            platform: "ios".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        };
        state.ui_mode = UiMode::DeviceSelector;
        let _ = update(&mut state, Message::DeviceSelected { device });
    }
    
    assert_eq!(state.session_manager.len(), 9);
    
    // 10th should fail
    let device = Device {
        id: "device-10".to_string(),
        name: "Device 10".to_string(),
        platform: "ios".to_string(),
        emulator: false,
        category: None,
        platform_type: None,
        ephemeral: false,
        emulator_id: None,
    };
    state.ui_mode = UiMode::DeviceSelector;
    let result = update(&mut state, Message::DeviceSelected { device });
    
    // Should NOT create another session
    assert_eq!(state.session_manager.len(), 9);
    assert!(result.action.is_none());
}
```

---

### Notes

- After this task, sessions will be created in the manager, but the spawned process won't yet be attached to the session (Task 04)
- The session will start in `Initializing` phase
- Logs will still go to global state until Task 05 (event routing)
- Session tabs should start appearing in the UI if multiple devices are selected

---

## Completion Summary

**Status:** ✅ Done

**Files Modified:**
- `src/app/handler.rs`:
  - Updated `Message::DeviceSelected` handler (lines 278-317) to:
    - Check for duplicate device sessions via `find_by_device_id()`
    - Create session in `SessionManager` before spawning
    - Pass real `session_id` to `SpawnSession` action
    - Handle errors from session creation (max sessions reached)
  - Added 4 new unit tests for Task 02:
    - `test_device_selected_creates_session`
    - `test_device_selected_prevents_duplicate`
    - `test_device_selected_max_sessions_enforced`
    - `test_device_selected_session_id_in_spawn_action`

**Notable Decisions/Tradeoffs:**
- Duplicate device check uses `find_by_device_id()` - a device can only have one active session
- Error handling logs to global state and keeps device selector open for retry
- Session is created in `Initializing` phase - will transition to `Running` when process starts (Task 06)

**Testing Performed:**
- `cargo check` - Passed (no compilation errors)
- `cargo test` - All 395 tests passed (4 new tests)
- `cargo fmt` - Code formatted
- `cargo clippy` - Only pre-existing warning (unrelated)

**Risks/Limitations:**
- Session is created but process not yet attached (Task 04)
- Logs still go to global state (Task 05 will route to sessions)
- Session tabs appear but are in `Initializing` phase until Task 06 updates state

**Acceptance Criteria Status:**
1. [x] `DeviceSelected` handler creates session before spawning
2. [x] Duplicate device selection is prevented with error message
3. [x] Session appears in `session_manager` immediately after selection
4. [x] `SpawnSession` action includes valid `session_id`
5. [x] Max sessions (9) limit is enforced
6. [x] Session tabs appear in UI when multiple sessions exist (via existing tab rendering)