## Task: Event Routing to Sessions

**Objective**: Route incoming daemon events (logs, status updates, app start/stop) to the correct session based on `app_id` or `device_id`, ensuring each session has independent logging and state.

**Depends on**: Task 04 (Session CommandSender storage)

---

### Scope

- `src/tui/mod.rs`: Update event forwarding in SpawnSession handler
- `src/app/handler.rs`: Update daemon event handling to route to correct session
- `src/app/message.rs`: Possibly add session_id to daemon-related messages

---

### Current State

```rust
// In handle_action - SpawnSession, the event forwarding loop:
loop {
    tokio::select! {
        event = daemon_rx.recv() => {
            match event {
                Some(event) => {
                    // Sends to global message channel without session context
                    if msg_tx_clone.send(Message::Daemon(event)).await.is_err() {
                        break;
                    }
                }
                None => break,
            }
        }
        // ...
    }
}

// In handler.rs - handle_daemon_event
fn handle_daemon_event(state: &mut AppState, event: DaemonEvent) {
    match event {
        DaemonEvent::Stdout(line) => {
            // Logs go to GLOBAL state.logs, not session-specific
            state.add_log(LogEntry::new(...));
        }
        // ...
    }
}
```

**Problem:** All events go to global state, so:
- All sessions' logs are mixed together
- Can't tell which session produced which log
- Status updates affect global state, not session state

---

### Implementation Details

#### 1. Add Session Context to Daemon Message

```rust
// In src/app/message.rs
pub enum Message {
    // ... existing variants ...
    
    /// Daemon event with session context
    SessionDaemon {
        session_id: SessionId,
        event: DaemonEvent,
    },
    
    // Keep legacy Daemon for backward compatibility (single-session mode)
    Daemon(DaemonEvent),
}
```

#### 2. Update SpawnSession Event Forwarding

```rust
// In handle_action - SpawnSession
let handle = tokio::spawn(async move {
    // ... spawn process ...
    
    match spawn_result {
        Ok(mut process) => {
            // ... setup ...
            
            loop {
                tokio::select! {
                    event = daemon_rx.recv() => {
                        match event {
                            Some(event) => {
                                // Send WITH session context
                                if msg_tx_clone
                                    .send(Message::SessionDaemon {
                                        session_id,
                                        event,
                                    })
                                    .await
                                    .is_err()
                                {
                                    break;
                                }
                            }
                            None => break,
                        }
                    }
                    // ...
                }
            }
        }
        Err(e) => { /* ... */ }
    }
});
```

#### 3. Handle SessionDaemon in Update Function

```rust
// In src/app/handler.rs
Message::SessionDaemon { session_id, event } => {
    handle_session_daemon_event(state, session_id, event);
    UpdateResult::none()
}
```

#### 4. Create Session-Aware Daemon Event Handler

```rust
// In src/app/handler.rs
fn handle_session_daemon_event(state: &mut AppState, session_id: SessionId, event: DaemonEvent) {
    // Get the session, if it still exists
    let Some(handle) = state.session_manager.get_mut(session_id) else {
        // Session was closed, discard event
        tracing::debug!("Discarding event for closed session {}", session_id);
        return;
    };
    
    match event {
        DaemonEvent::Stdout(line) => {
            handle_session_stdout(handle, &line, state);
        }
        DaemonEvent::Stderr(line) => {
            handle.session.add_log(LogEntry::new(
                LogLevel::Error,
                LogSource::FlutterError,
                line,
            ));
        }
        DaemonEvent::Exited { code } => {
            handle_session_exited(state, session_id, code);
        }
    }
}

fn handle_session_stdout(handle: &mut SessionHandle, line: &str, state: &mut AppState) {
    // Try to parse as daemon message
    if let Some(json) = protocol::strip_brackets(line) {
        if let Some(msg) = DaemonMessage::parse(json) {
            // Handle responses separately
            if matches!(msg, DaemonMessage::Response { .. }) {
                tracing::debug!("Response for session: {}", msg.summary());
                return;
            }
            
            // Handle app.start to capture app_id
            if let DaemonMessage::AppStart(app_start) = &msg {
                handle.session.mark_started(app_start.app_id.clone());
                // Also update global state for legacy compatibility
                state.current_app_id = Some(app_start.app_id.clone());
            }
            
            // Convert to log entry if applicable
            if let Some(entry_info) = msg.to_log_entry() {
                handle.session.add_log(LogEntry::new(
                    entry_info.level,
                    entry_info.source,
                    entry_info.message,
                ));
                
                // Add stack trace as separate entries
                if let Some(trace) = entry_info.stack_trace {
                    for line in trace.lines().take(10) {
                        handle.session.add_log(LogEntry::new(
                            LogLevel::Debug,
                            LogSource::FlutterError,
                            line.to_string(),
                        ));
                    }
                }
            }
        }
    } else {
        // Non-JSON output - treat as raw Flutter output
        handle.session.add_log(LogEntry::new(
            LogLevel::Info,
            LogSource::Flutter,
            line.to_string(),
        ));
    }
}

fn handle_session_exited(state: &mut AppState, session_id: SessionId, code: Option<i32>) {
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        let level = if code == Some(0) { LogLevel::Info } else { LogLevel::Warning };
        let message = match code {
            Some(0) => "Flutter process exited normally".to_string(),
            Some(c) => format!("Flutter process exited with code {}", c),
            None => "Flutter process exited (no exit code)".to_string(),
        };
        
        handle.session.add_log(LogEntry::new(level, LogSource::App, message));
        handle.session.phase = AppPhase::Stopped;
        
        // Don't auto-quit - let user decide what to do
    }
}
```

#### 5. Update Session Logging Methods

Ensure `Session` has proper logging methods:

```rust
// In src/app/session.rs - already exists
impl Session {
    pub fn add_log(&mut self, entry: LogEntry) {
        self.logs.push(entry);
        if self.logs.len() > self.max_logs {
            let drain_count = self.logs.len() - self.max_logs;
            self.logs.drain(0..drain_count);
            self.log_view_state.offset = self.log_view_state.offset.saturating_sub(drain_count);
        }
    }
    
    pub fn log_info(&mut self, source: LogSource, message: impl Into<String>) {
        self.add_log(LogEntry::info(source, message));
    }
    
    pub fn log_error(&mut self, source: LogSource, message: impl Into<String>) {
        self.add_log(LogEntry::error(source, message));
    }
}
```

---

### Event Routing Flow After This Task

```
Flutter Process (Session 1)           Flutter Process (Session 2)
         │                                      │
         ▼                                      ▼
    daemon_rx.recv()                       daemon_rx.recv()
         │                                      │
         ▼                                      ▼
Message::SessionDaemon {               Message::SessionDaemon {
    session_id: 1,                         session_id: 2,
    event                                  event
}                                      }
         │                                      │
         └──────────────┬───────────────────────┘
                        │
                        ▼
              Main Message Channel
                        │
                        ▼
              handle_session_daemon_event()
                        │
         ┌──────────────┴───────────────────────┐
         │                                      │
         ▼                                      ▼
session_manager.get_mut(1)         session_manager.get_mut(2)
         │                                      │
         ▼                                      ▼
    session.logs                          session.logs
    (independent)                         (independent)
```

---

### Acceptance Criteria

1. [ ] `Message::SessionDaemon` variant added with session_id
2. [ ] SpawnSession forwards events with session context
3. [ ] Events route to correct session's logs
4. [ ] Session 1's logs don't appear in Session 2's log view
5. [ ] App start events update session-specific app_id
6. [ ] Process exit updates session-specific phase
7. [ ] Legacy `Message::Daemon` still works for single-session mode

---

### Testing

```rust
#[test]
fn test_session_daemon_event_routes_to_correct_session() {
    let mut state = AppState::new();
    
    // Create two sessions
    let d1 = test_device("d1", "Device 1");
    let d2 = test_device("d2", "Device 2");
    let id1 = state.session_manager.create_session(&d1).unwrap();
    let id2 = state.session_manager.create_session(&d2).unwrap();
    
    // Send event to session 1
    update(&mut state, Message::SessionDaemon {
        session_id: id1,
        event: DaemonEvent::Stdout("[{\"event\":\"app.log\",\"params\":{\"log\":\"Test log 1\"}}]".into()),
    });
    
    // Send event to session 2
    update(&mut state, Message::SessionDaemon {
        session_id: id2,
        event: DaemonEvent::Stdout("[{\"event\":\"app.log\",\"params\":{\"log\":\"Test log 2\"}}]".into()),
    });
    
    // Check logs are in correct sessions
    let logs1 = &state.session_manager.get(id1).unwrap().session.logs;
    let logs2 = &state.session_manager.get(id2).unwrap().session.logs;
    
    assert!(logs1.iter().any(|l| l.message.contains("Test log 1")));
    assert!(!logs1.iter().any(|l| l.message.contains("Test log 2")));
    
    assert!(logs2.iter().any(|l| l.message.contains("Test log 2")));
    assert!(!logs2.iter().any(|l| l.message.contains("Test log 1")));
}

#[test]
fn test_session_app_start_updates_session_state() {
    let mut state = AppState::new();
    
    let device = test_device("d1", "Device 1");
    let session_id = state.session_manager.create_session(&device).unwrap();
    
    // Simulate app.start event
    let app_start_json = r#"[{"event":"app.start","params":{"appId":"app-123","deviceId":"d1"}}]"#;
    
    update(&mut state, Message::SessionDaemon {
        session_id,
        event: DaemonEvent::Stdout(app_start_json.into()),
    });
    
    let session = &state.session_manager.get(session_id).unwrap().session;
    assert_eq!(session.app_id, Some("app-123".to_string()));
    assert_eq!(session.phase, AppPhase::Running);
}

#[test]
fn test_event_for_closed_session_is_discarded() {
    let mut state = AppState::new();
    
    let device = test_device("d1", "Device 1");
    let session_id = state.session_manager.create_session(&device).unwrap();
    
    // Remove the session
    state.session_manager.remove_session(session_id);
    
    // Send event to removed session - should not panic
    let result = update(&mut state, Message::SessionDaemon {
        session_id,
        event: DaemonEvent::Stdout("test".into()),
    });
    
    // Should complete without error
    assert!(result.action.is_none());
}

#[test]
fn test_process_exit_updates_session_phase() {
    let mut state = AppState::new();
    
    let device = test_device("d1", "Device 1");
    let session_id = state.session_manager.create_session(&device).unwrap();
    state.session_manager.get_mut(session_id).unwrap().session.mark_started("app-1".into());
    
    // Session should be running
    assert_eq!(
        state.session_manager.get(session_id).unwrap().session.phase,
        AppPhase::Running
    );
    
    // Simulate process exit
    update(&mut state, Message::SessionDaemon {
        session_id,
        event: DaemonEvent::Exited { code: Some(0) },
    });
    
    // Session should now be stopped
    assert_eq!(
        state.session_manager.get(session_id).unwrap().session.phase,
        AppPhase::Stopped
    );
}
```

---

### Notes

- Keep legacy `Message::Daemon` handler for backward compatibility
- Events for removed/closed sessions should be silently discarded
- The render function already uses `session_manager.selected_mut()` for log display
- Auto-scroll should work per-session via `session.log_view_state`
- Consider adding session_id to reload completion messages for proper routing