## Task: File Watcher Multi-Session Hot Reload

**Objective**: Make the file watcher hot reload ALL running sessions on file saves, not just the selected session. Keyboard shortcuts `r` and `R` remain per-session (selected session only) for granular control.

**Depends on**: None (independent of Tasks 01-04)

---

### Background

Currently, when the file watcher detects changes:
1. Sends `Message::AutoReloadTriggered`
2. Handler in `update.rs` only reloads the **selected session**
3. Other running sessions are NOT reloaded

This is problematic for multi-device development where you want to see changes on all devices simultaneously.

**Desired behavior:**
- File watcher auto-reload → reload ALL running sessions
- `r` key → reload selected session only (unchanged)
- `R` key → restart selected session only (unchanged)

---

### Scope

#### `src/app/handler/update.rs`
- Modify `Message::AutoReloadTriggered` handler to reload ALL running sessions
- Return multiple `SpawnTask(Reload)` actions, or introduce a new action type for multi-session reload
- Keep legacy fallback for now (will be removed in Task 04)

#### `src/app/handler/mod.rs`
- Potentially add new `UpdateAction::ReloadAllSessions` variant
- Or change `UpdateAction::SpawnTask` to support batch tasks

#### `src/tui/actions.rs`
- Handle new action type for reloading all sessions
- Spawn reload tasks for each running session

#### `src/app/session_manager.rs`
- Add helper method `running_sessions_with_app_id()` that returns sessions with active app_ids

---

### Implementation Details

#### Option A: New UpdateAction variant (Recommended)

Add new action type:

```rust
pub enum UpdateAction {
    // ... existing variants ...
    
    /// Reload all running sessions (file watcher auto-reload)
    ReloadAllSessions,
}
```

Handler in `update.rs`:

```rust
Message::AutoReloadTriggered => {
    // Check if any session is busy (reloading)
    let any_busy = state.session_manager.iter()
        .any(|h| h.session.is_busy());
    
    if any_busy {
        tracing::debug!("Auto-reload skipped: some session(s) already reloading");
        return UpdateResult::none();
    }
    
    // Get all running sessions with app_id
    let running_sessions: Vec<_> = state.session_manager.iter()
        .filter(|h| h.session.app_id.is_some() && h.cmd_sender.is_some())
        .collect();
    
    if running_sessions.is_empty() {
        tracing::debug!("Auto-reload skipped: no running sessions");
        return UpdateResult::none();
    }
    
    state.log_info(
        LogSource::Watcher, 
        format!("File change detected, reloading {} session(s)...", running_sessions.len())
    );
    
    UpdateResult::action(UpdateAction::ReloadAllSessions)
}
```

Action handler in `actions.rs`:

```rust
UpdateAction::ReloadAllSessions => {
    // This needs access to session_manager, so we handle it differently
    // Either pass session info, or handle in process.rs
    spawn_all_sessions_reload(msg_tx, session_tasks, ...);
}
```

#### Option B: Return multiple actions

Change `UpdateResult` to support multiple actions:

```rust
pub struct UpdateResult {
    pub message: Option<Message>,
    pub actions: Vec<UpdateAction>,  // Changed from Option<UpdateAction>
}
```

This is more invasive but cleaner long-term.

#### Option C: Loop and send multiple messages (Simplest)

Keep existing action, but have the handler loop through sessions and send multiple reload messages:

```rust
Message::AutoReloadTriggered => {
    let mut reload_count = 0;
    
    for handle in state.session_manager.iter() {
        if handle.session.is_busy() {
            continue;
        }
        if let (Some(app_id), Some(_)) = (&handle.session.app_id, &handle.cmd_sender) {
            let session_id = handle.session.id;
            // Can't return multiple actions, so we need a different approach
            // ...
        }
    }
}
```

This doesn't work well with the current single-action return.

---

### Recommended Approach: Option A

1. Add `UpdateAction::ReloadAllSessions` variant
2. Pass session reload info through the action or fetch from state
3. Handle in `actions.rs` by spawning reload tasks for each running session

#### Session Manager Helper

```rust
impl SessionManager {
    /// Get all sessions that can be reloaded (have app_id and cmd_sender)
    pub fn reloadable_sessions(&self) -> Vec<(SessionId, String, CommandSender)> {
        self.handles.values()
            .filter_map(|h| {
                let app_id = h.session.app_id.clone()?;
                let sender = h.cmd_sender.clone()?;
                Some((h.session.id, app_id, sender))
            })
            .collect()
    }
}
```

#### Action Handler

```rust
UpdateAction::ReloadAllSessions => {
    // Get reloadable sessions from state (need to pass this info somehow)
    // For each session, spawn a reload task
    for (session_id, app_id, sender) in reloadable_sessions {
        let task = Task::Reload { session_id, app_id };
        tokio::spawn(async move {
            execute_task(task, msg_tx.clone(), Some(sender)).await;
        });
    }
}
```

**Challenge**: The action handler in `actions.rs` doesn't have access to `AppState`. 

**Solution**: Include session info in the action:

```rust
UpdateAction::ReloadAllSessions {
    sessions: Vec<(SessionId, String)>,  // (session_id, app_id)
}
```

Then in `process.rs`, collect the cmd_senders before dispatching:

```rust
UpdateAction::ReloadAllSessions { sessions } => {
    for (session_id, app_id) in sessions {
        // Get cmd_sender from session_manager (we have access in process.rs)
        if let Some(handle) = state.session_manager.get(session_id) {
            if let Some(sender) = handle.cmd_sender.clone() {
                let task = Task::Reload { session_id, app_id };
                // Spawn reload task with this session's sender
            }
        }
    }
}
```

---

### Acceptance Criteria

1. ✅ File save triggers hot reload on ALL running sessions
2. ✅ `r` key still reloads only the selected session
3. ✅ `R` key still restarts only the selected session
4. ✅ Sessions that are already reloading are skipped
5. ✅ Sessions without app_id (not yet started) are skipped
6. ✅ Log message shows count of sessions being reloaded
7. ✅ Each session's log shows its own reload message
8. ✅ All existing tests pass
9. ✅ New tests cover multi-session reload behavior

---

### Testing

#### Unit Tests

```rust
#[test]
fn test_auto_reload_triggers_all_sessions() {
    let mut state = AppState::new();
    
    // Create two running sessions
    let d1 = test_device("d1", "iPhone");
    let d2 = test_device("d2", "Pixel");
    let id1 = state.session_manager.create_session(&d1).unwrap();
    let id2 = state.session_manager.create_session(&d2).unwrap();
    
    // Mark both as running with app_ids
    state.session_manager.get_mut(id1).unwrap().session.mark_started("app1".into());
    state.session_manager.get_mut(id2).unwrap().session.mark_started("app2".into());
    
    // Trigger auto-reload
    let result = update(&mut state, Message::AutoReloadTriggered);
    
    // Should return ReloadAllSessions action
    assert!(matches!(result.action, Some(UpdateAction::ReloadAllSessions { .. })));
}

#[test]
fn test_auto_reload_skips_busy_sessions() {
    let mut state = AppState::new();
    
    // Create two sessions, one busy
    let d1 = test_device("d1", "iPhone");
    let d2 = test_device("d2", "Pixel");
    let id1 = state.session_manager.create_session(&d1).unwrap();
    let id2 = state.session_manager.create_session(&d2).unwrap();
    
    state.session_manager.get_mut(id1).unwrap().session.mark_started("app1".into());
    state.session_manager.get_mut(id2).unwrap().session.mark_started("app2".into());
    state.session_manager.get_mut(id1).unwrap().session.start_reload(); // Busy
    
    let result = update(&mut state, Message::AutoReloadTriggered);
    
    // Should skip all if any is busy (or only reload non-busy ones - TBD)
}

#[test]
fn test_manual_reload_only_selected_session() {
    let mut state = AppState::new();
    
    // Create two sessions
    let d1 = test_device("d1", "iPhone");
    let d2 = test_device("d2", "Pixel");
    let id1 = state.session_manager.create_session(&d1).unwrap();
    let id2 = state.session_manager.create_session(&d2).unwrap();
    
    state.session_manager.get_mut(id1).unwrap().session.mark_started("app1".into());
    state.session_manager.get_mut(id2).unwrap().session.mark_started("app2".into());
    state.session_manager.select(id1);
    
    // Manual reload (r key)
    let result = update(&mut state, Message::HotReload);
    
    // Should only reload session 1
    if let Some(UpdateAction::SpawnTask(Task::Reload { session_id, .. })) = result.action {
        assert_eq!(session_id, id1);
    } else {
        panic!("Expected single session reload");
    }
}
```

#### Manual Testing

1. Start fdemon with two devices
2. Make a file change in lib/
3. Verify BOTH devices hot reload
4. Press `r` → verify only selected device reloads
5. Press `R` → verify only selected device restarts
6. Make file change while one device is reloading → verify behavior (skip all vs skip busy)

---

### Design Decisions

**Q: Should we skip ALL sessions if ANY is busy, or only skip the busy ones?**

Option 1: Skip all if any busy (simpler, prevents race conditions)
Option 2: Reload non-busy sessions only (more responsive, but may cause sync issues)

Recommendation: **Option 1 - Skip all if any busy**. This ensures all devices stay in sync.

**Q: Should we add a status indicator showing "Reloading X sessions"?**

Nice to have, but not required for this task. Can be added later.

---

### Notes

- The file watcher itself doesn't need changes - it already sends `AutoReloadTriggered`
- Only the handler needs to change behavior for this message
- This task is independent of the legacy code removal (Task 04)
- Consider adding a setting to toggle between "reload all" vs "reload selected" for file watcher

---

### Files Changed Summary

| File | Change |
|------|--------|
| `src/app/handler/mod.rs` | Add `UpdateAction::ReloadAllSessions` variant |
| `src/app/handler/update.rs` | Change `AutoReloadTriggered` handler to reload all sessions |
| `src/app/session_manager.rs` | Add `reloadable_sessions()` helper method |
| `src/tui/actions.rs` | Handle new `ReloadAllSessions` action |
| `src/tui/process.rs` | Dispatch `ReloadAllSessions` with session info |
| `src/app/handler/tests.rs` | Add tests for multi-session reload |