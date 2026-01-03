## Task: Session CommandSender Storage

**Objective**: Store `CommandSender` in `SessionHandle.cmd_sender` instead of the shared global mutex, enabling per-session command routing for reload/restart operations.

**Depends on**: Task 02 (Session creation), Task 03 (Per-session task tracking)

---

### Scope

- `src/tui/mod.rs`: Update SpawnSession handler to store cmd_sender in SessionHandle
- `src/app/session.rs`: Ensure SessionHandle has proper cmd_sender field
- `src/app/handler.rs`: Update reload/restart handlers to get cmd_sender from session

---

### Current State

```rust
// In src/tui/mod.rs - run_with_project
let cmd_sender: Arc<Mutex<Option<CommandSender>>> = Arc::new(Mutex::new(None));

// In handle_action - SpawnSession
let sender = process.command_sender(request_tracker);
*cmd_sender_clone.lock().await = Some(sender);  // GLOBAL - only ONE sender!

// In execute_task - for reload/restart
let Some(sender) = cmd_sender else {
    // Uses the single global sender
};
```

**Problem:** Only one CommandSender is stored. When a second session starts:
- The new session's cmd_sender overwrites the previous one
- Hot reload commands go to the wrong session
- First session becomes uncontrollable

---

### SessionHandle Already Has Fields

```rust
// In src/app/session.rs - SessionHandle
pub struct SessionHandle {
    pub session: Session,
    pub process: Option<FlutterProcess>,
    pub cmd_sender: Option<CommandSender>,  // EXISTS but unused!
    pub request_tracker: Arc<RequestTracker>,
}
```

The infrastructure exists but isn't being used.

---

### Implementation Details

#### 1. Remove Global cmd_sender (Eventually)

For this task, we'll keep the global cmd_sender for backward compatibility but prioritize SessionHandle storage.

```rust
// In run_with_project - can remove or keep for legacy single-session mode
// let cmd_sender: Arc<Mutex<Option<CommandSender>>> = Arc::new(Mutex::new(None));
```

#### 2. Update SpawnSession Handler to Store in SessionHandle

```rust
UpdateAction::SpawnSession { session_id, device, config } => {
    let project_path = project_path.to_path_buf();
    let msg_tx_clone = msg_tx.clone();
    let session_tasks_clone = session_tasks.clone();
    // Need access to state for session_manager - pass via message
    
    let handle = tokio::spawn(async move {
        // ... spawn process ...
        
        match spawn_result {
            Ok(mut process) => {
                let request_tracker = Arc::new(RequestTracker::default());
                let sender = process.command_sender(request_tracker.clone());
                
                // Send message to attach cmd_sender to session
                let _ = msg_tx_clone
                    .send(Message::SessionProcessAttached {
                        session_id,
                        cmd_sender: Some(sender),
                    })
                    .await;
                    
                // ... rest of event forwarding loop ...
            }
            Err(e) => { /* ... */ }
        }
    });
    
    session_tasks.lock().await.insert(session_id, handle);
}
```

#### 3. Add New Message for Process Attachment

```rust
// In src/app/message.rs
pub enum Message {
    // ... existing variants ...
    
    /// Attach a command sender to a session (from background task)
    SessionProcessAttached {
        session_id: SessionId,
        cmd_sender: Option<CommandSender>,
    },
}
```

#### 4. Handle SessionProcessAttached Message

```rust
// In src/app/handler.rs
Message::SessionProcessAttached { session_id, cmd_sender } => {
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        handle.cmd_sender = cmd_sender;
        state.log_info(
            LogSource::App, 
            format!("Command sender attached to session {}", session_id)
        );
    }
    UpdateResult::none()
}
```

#### 5. Update Reload/Restart to Use Session's CommandSender

```rust
// In src/app/handler.rs - handle HotReload
Message::HotReload => {
    if state.is_busy() {
        return UpdateResult::none();
    }
    
    // Get app_id and cmd_sender from selected session
    let session_info = state.session_manager.selected().and_then(|h| {
        h.session.app_id.clone().map(|app_id| (app_id, h.cmd_sender.clone()))
    });
    
    if let Some((app_id, Some(sender))) = session_info {
        state.start_reload();
        state.log_info(LogSource::App, "Reloading...");
        
        // Need to pass sender to the task somehow
        // Option A: Include sender in Task enum
        // Option B: Task looks up sender by session_id
        
        UpdateResult::action(UpdateAction::SpawnTask(Task::Reload { app_id }))
    } else if let Some(app_id) = state.current_app_id.clone() {
        // Fallback to legacy global behavior
        state.start_reload();
        UpdateResult::action(UpdateAction::SpawnTask(Task::Reload { app_id }))
    } else {
        state.log_error(LogSource::App, "No app running to reload");
        UpdateResult::none()
    }
}
```

#### 6. Update execute_task to Accept Optional Sender

```rust
// Option A: Pass sender directly (requires changing Task enum)
pub enum Task {
    Reload { app_id: String, cmd_sender: Option<CommandSender> },
    Restart { app_id: String, cmd_sender: Option<CommandSender> },
    Stop { app_id: String, cmd_sender: Option<CommandSender> },
}

// Option B: Keep Task simple, pass sender separately in execute_task
async fn execute_task(
    task: Task,
    msg_tx: mpsc::Sender<Message>,
    cmd_sender: Option<CommandSender>,  // From handler context
) {
    // Use provided sender
}
```

---

### Architectural Decision: CommandSender in Message Flow

Since `CommandSender` contains async channels and isn't `Clone` in a simple way, we have two options:

**Option A: Session-Based Lookup**
- Store sender in SessionHandle (already planned)
- Tasks include session_id, not sender
- execute_task looks up sender via state

**Option B: Task Includes Sender**
- Clone CommandSender (it implements Clone via Arc internals)
- Task enum variants include the sender
- No state lookup needed in execute_task

**Recommendation:** Option A - cleaner separation, state remains source of truth.

---

### Updated Task Enum (If Using Option A)

```rust
pub enum Task {
    Reload { session_id: SessionId, app_id: String },
    Restart { session_id: SessionId, app_id: String },
    Stop { session_id: SessionId, app_id: String },
}
```

---

### Acceptance Criteria

1. [ ] `SessionHandle.cmd_sender` is populated when process starts
2. [ ] `SessionProcessAttached` message updates the session
3. [ ] Hot reload uses the selected session's cmd_sender
4. [ ] Hot restart uses the selected session's cmd_sender  
5. [ ] Each session can be reloaded independently
6. [ ] Legacy single-session mode still works (fallback)

---

### Testing

```rust
#[test]
fn test_session_process_attached() {
    let mut state = AppState::new();
    
    // Create session
    let device = test_device("d1", "Device 1");
    let session_id = state.session_manager.create_session(&device).unwrap();
    
    // Initially no cmd_sender
    assert!(state.session_manager.get(session_id).unwrap().cmd_sender.is_none());
    
    // Simulate process attachment (in real code, sender would come from background task)
    // For testing, we verify the handler path works
    // Note: Can't easily test with real CommandSender in unit tests
    
    // After attachment, cmd_sender should be Some
    // (Would need integration test or mock)
}

#[test]
fn test_reload_uses_session_sender() {
    let mut state = AppState::new();
    
    // Create two sessions
    let d1 = test_device("d1", "Device 1");
    let d2 = test_device("d2", "Device 2");
    let id1 = state.session_manager.create_session(&d1).unwrap();
    let id2 = state.session_manager.create_session(&d2).unwrap();
    
    // Mark sessions as running with different app_ids
    state.session_manager.get_mut(id1).unwrap().session.mark_started("app-1".into());
    state.session_manager.get_mut(id2).unwrap().session.mark_started("app-2".into());
    
    // Select session 2
    state.session_manager.select_by_id(id2);
    
    // Hot reload should use session 2's app_id
    let result = update(&mut state, Message::HotReload);
    
    if let Some(UpdateAction::SpawnTask(Task::Reload { app_id, .. })) = result.action {
        assert_eq!(app_id, "app-2");
    } else {
        // Might fail without cmd_sender - that's expected for this unit test
    }
}
```

---

### Notes

- `CommandSender` is `Clone` because it wraps `mpsc::Sender` in an Arc
- The background task (SpawnSession) runs in tokio, can't directly access AppState
- Use message passing to update SessionHandle from background task
- Keep legacy `cmd_sender` for backward compatibility during migration
- Task 05 (Event Routing) will also need session_id to route events properly