## Task: Update SessionStarted Handler for Multi-Session

**Objective**: Update the `SessionStarted` message handler to modify session-specific state in `SessionManager` instead of only updating legacy global state fields, ensuring each session tracks its own status independently.

**Depends on**: Task 05 (Event routing to sessions)

---

### Scope

- `src/app/handler.rs`: Update `Message::SessionStarted` handler
- `src/app/message.rs`: Add session_id to SessionStarted message
- `src/tui/mod.rs`: Update SessionStarted message construction

---

### Current State

```rust
// In src/app/handler.rs
Message::SessionStarted {
    device_id: _,
    device_name,
    platform,
    pid,
} => {
    // Update legacy single-session state for now
    state.device_name = Some(device_name.clone());
    state.platform = Some(platform);
    state.phase = AppPhase::Running;
    state.session_start = Some(chrono::Local::now());

    state.log_info(
        LogSource::App,
        format!(
            "Flutter session started on {} (PID: {})",
            device_name,
            pid.map_or("unknown".to_string(), |p| p.to_string())
        ),
    );
    UpdateResult::none()
}
```

**Problem:** 
- Only updates global `state.device_name`, `state.platform`, `state.phase`
- Doesn't update the session in `SessionManager`
- Second session would overwrite first session's info in global state

---

### Implementation Details

#### 1. Add session_id to SessionStarted Message

```rust
// In src/app/message.rs
pub enum Message {
    // ... existing variants ...
    
    /// Session started successfully
    SessionStarted {
        session_id: SessionId,  // ADD THIS
        device_id: String,
        device_name: String,
        platform: String,
        pid: Option<u32>,
    },
}
```

#### 2. Update Message Construction in SpawnSession

```rust
// In src/tui/mod.rs - handle_action SpawnSession
let _ = msg_tx_clone
    .send(Message::SessionStarted {
        session_id,  // Now available in closure
        device_id: device_id.clone(),
        device_name: device_name.clone(),
        platform: device_platform.clone(),
        pid: process.id(),
    })
    .await;
```

#### 3. Update SessionStarted Handler

```rust
// In src/app/handler.rs
Message::SessionStarted {
    session_id,
    device_id: _,
    device_name,
    platform,
    pid,
} => {
    // Update session-specific state
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        // Note: mark_started sets phase to Running and stores app_id
        // But at this point we don't have app_id yet (comes from app.start event)
        // So just log and update phase
        handle.session.phase = AppPhase::Running;
        handle.session.start_time = Some(chrono::Local::now());
        
        handle.session.log_info(
            LogSource::App,
            format!(
                "Flutter session started (PID: {})",
                pid.map_or("unknown".to_string(), |p| p.to_string())
            ),
        );
    }
    
    // Also update legacy global state for backward compatibility
    state.device_name = Some(device_name.clone());
    state.platform = Some(platform);
    state.phase = AppPhase::Running;
    state.session_start = Some(chrono::Local::now());

    state.log_info(
        LogSource::App,
        format!(
            "Flutter session started on {} (PID: {})",
            device_name,
            pid.map_or("unknown".to_string(), |p| p.to_string())
        ),
    );
    
    UpdateResult::none()
}
```

#### 4. Ensure Session Has start_time Field

```rust
// In src/app/session.rs - Session struct
pub struct Session {
    // ... existing fields ...
    
    /// When this session started
    pub start_time: Option<DateTime<Local>>,
}

impl Session {
    pub fn new(...) -> Self {
        Self {
            // ... existing init ...
            start_time: None,
        }
    }
    
    /// Calculate session duration
    pub fn duration(&self) -> Option<chrono::Duration> {
        self.start_time.map(|start| Local::now() - start)
    }
    
    /// Format session duration as HH:MM:SS
    pub fn duration_display(&self) -> Option<String> {
        self.duration().map(|d| {
            let total_secs = d.num_seconds().max(0);
            let hours = total_secs / 3600;
            let minutes = (total_secs % 3600) / 60;
            let seconds = total_secs % 60;
            format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
        })
    }
}
```

#### 5. Update SessionSpawnFailed Similarly

```rust
// In src/app/message.rs - add session_id
Message::SessionSpawnFailed {
    session_id: SessionId,  // ADD THIS
    device_id: String,
    error: String,
}

// In src/app/handler.rs
Message::SessionSpawnFailed {
    session_id,
    device_id: _,
    error,
} => {
    // Update session-specific state
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        handle.session.phase = AppPhase::Stopped;
        handle.session.log_error(
            LogSource::App,
            format!("Failed to start session: {}", error),
        );
    }
    
    // Remove the failed session from manager
    state.session_manager.remove_session(session_id);
    
    state.log_error(
        LogSource::App,
        format!("Failed to start session: {}", error),
    );
    
    // Show device selector again so user can retry
    state.ui_mode = UiMode::DeviceSelector;
    UpdateResult::none()
}
```

---

### Session State Flow After This Task

```
DeviceSelected
      │
      ▼
create_session() → Session created with phase=Initializing
      │
      ▼
SpawnSession action
      │
      ▼
tokio::spawn() starts Flutter process
      │
      ├── Success ─────────────────────────────┐
      │                                        │
      ▼                                        ▼
Message::SessionStarted              Message::SessionSpawnFailed
      │                                        │
      ▼                                        ▼
session.phase = Running              session.phase = Stopped
session.start_time = now()           Remove from SessionManager
      │                                        │
      ▼                                        ▼
Later: app.start event               Show DeviceSelector
      │
      ▼
session.app_id = "..."
```

---

### Acceptance Criteria

1. [ ] `SessionStarted` message includes `session_id`
2. [ ] Handler updates session-specific `phase` and `start_time`
3. [ ] Handler logs to session-specific logs
4. [ ] `SessionSpawnFailed` includes `session_id`
5. [ ] Failed sessions are removed from manager
6. [ ] Legacy global state still updated for backward compatibility
7. [ ] Session duration can be calculated per-session

---

### Testing

```rust
#[test]
fn test_session_started_updates_session_state() {
    let mut state = AppState::new();
    
    let device = test_device("d1", "iPhone 15");
    let session_id = state.session_manager.create_session(&device).unwrap();
    
    // Initially Initializing
    assert_eq!(
        state.session_manager.get(session_id).unwrap().session.phase,
        AppPhase::Initializing
    );
    
    // Simulate SessionStarted
    update(&mut state, Message::SessionStarted {
        session_id,
        device_id: "d1".into(),
        device_name: "iPhone 15".into(),
        platform: "ios".into(),
        pid: Some(12345),
    });
    
    let session = &state.session_manager.get(session_id).unwrap().session;
    
    // Phase should be Running
    assert_eq!(session.phase, AppPhase::Running);
    
    // start_time should be set
    assert!(session.start_time.is_some());
    
    // Should have a log entry
    assert!(!session.logs.is_empty());
    assert!(session.logs.iter().any(|l| l.message.contains("PID: 12345")));
}

#[test]
fn test_session_spawn_failed_removes_session() {
    let mut state = AppState::new();
    
    let device = test_device("d1", "iPhone 15");
    let session_id = state.session_manager.create_session(&device).unwrap();
    
    assert_eq!(state.session_manager.len(), 1);
    
    // Simulate spawn failure
    update(&mut state, Message::SessionSpawnFailed {
        session_id,
        device_id: "d1".into(),
        error: "Connection refused".into(),
    });
    
    // Session should be removed
    assert_eq!(state.session_manager.len(), 0);
    
    // Should show device selector
    assert_eq!(state.ui_mode, UiMode::DeviceSelector);
}

#[test]
fn test_multiple_sessions_have_independent_state() {
    let mut state = AppState::new();
    
    let d1 = test_device("d1", "iPhone 15");
    let d2 = test_device("d2", "Pixel 8");
    let id1 = state.session_manager.create_session(&d1).unwrap();
    let id2 = state.session_manager.create_session(&d2).unwrap();
    
    // Start session 1
    update(&mut state, Message::SessionStarted {
        session_id: id1,
        device_id: "d1".into(),
        device_name: "iPhone 15".into(),
        platform: "ios".into(),
        pid: Some(1000),
    });
    
    // Session 1 should be Running, Session 2 still Initializing
    assert_eq!(
        state.session_manager.get(id1).unwrap().session.phase,
        AppPhase::Running
    );
    assert_eq!(
        state.session_manager.get(id2).unwrap().session.phase,
        AppPhase::Initializing
    );
    
    // Start session 2
    update(&mut state, Message::SessionStarted {
        session_id: id2,
        device_id: "d2".into(),
        device_name: "Pixel 8".into(),
        platform: "android".into(),
        pid: Some(2000),
    });
    
    // Both should now be Running
    assert_eq!(
        state.session_manager.get(id1).unwrap().session.phase,
        AppPhase::Running
    );
    assert_eq!(
        state.session_manager.get(id2).unwrap().session.phase,
        AppPhase::Running
    );
    
    // Each should have their own logs
    let logs1 = &state.session_manager.get(id1).unwrap().session.logs;
    let logs2 = &state.session_manager.get(id2).unwrap().session.logs;
    
    assert!(logs1.iter().any(|l| l.message.contains("1000")));
    assert!(logs2.iter().any(|l| l.message.contains("2000")));
}

#[test]
fn test_session_duration_calculation() {
    let mut state = AppState::new();
    
    let device = test_device("d1", "iPhone 15");
    let session_id = state.session_manager.create_session(&device).unwrap();
    
    // Start session
    update(&mut state, Message::SessionStarted {
        session_id,
        device_id: "d1".into(),
        device_name: "iPhone 15".into(),
        platform: "ios".into(),
        pid: Some(12345),
    });
    
    let session = &state.session_manager.get(session_id).unwrap().session;
    
    // Duration should be calculable
    assert!(session.duration().is_some());
    assert!(session.duration_display().is_some());
    
    // Duration should be very small (just started)
    let duration = session.duration().unwrap();
    assert!(duration.num_seconds() < 1);
}
```

---

### Notes

- Keep legacy global state updates for backward compatibility during migration
- The `app_id` is set later via the `app.start` daemon event (Task 05)
- Session logs will show in the UI because render.rs already uses `session_manager.selected_mut()`
- Failed sessions are immediately removed to avoid ghost entries in tabs
- Consider adding a "Starting..." indicator in session tabs for `Initializing` phase