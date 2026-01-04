## Task 4c: Remove Legacy Fallback Paths in Handlers

**Objective**: Remove all fallback code paths that use `state.current_app_id` when no session is selected. After tasks 4a and 4b, the only way to interact with Flutter is through sessions.

**Depends on**: Task 4b (Message::Daemon removal must be complete)

---

### Background

Currently, the control message handlers (HotReload, HotRestart, StopApp, AutoReloadTriggered) have a two-tier approach:

1. **Primary path**: Try to use the selected session's `app_id` and `cmd_sender`
2. **Fallback path**: If no session, use global `state.current_app_id` with `session_id: 0`

The fallback path was for backward compatibility during the transition to multi-session. With legacy code removed, we should:
- Remove fallback paths entirely
- Show a clear error if no session is available
- Use session_id from the actual session (never 0)

---

### Scope

#### `src/app/handler/update.rs`

**Message::HotReload (lines 95-140)**

Remove fallback block (lines 120-137):
```rust
// REMOVE THIS BLOCK:
// Fall back to legacy global app_id (uses global state)
if state.is_busy() {
    return UpdateResult::none();
}
if let Some(app_id) = state.current_app_id.clone() {
    // Use session_id 0 for legacy mode (will use global cmd_sender)
    state.start_reload();
    state.log_info(LogSource::App, "Reloading (legacy mode)...");
    UpdateResult::action(UpdateAction::SpawnTask(Task::Reload {
        session_id: 0,
        app_id,
    }))
} else {
    state.log_error(LogSource::App, "No app running to reload");
    UpdateResult::none()
}
```

**New behavior for HotReload:**
```rust
Message::HotReload => {
    // Try to get session info from selected session
    if let Some(handle) = state.session_manager.selected_mut() {
        if handle.session.is_busy() {
            return UpdateResult::none();
        }
        if let Some(app_id) = handle.session.app_id.clone() {
            if handle.cmd_sender.is_some() {
                let session_id = handle.session.id;
                handle.session.start_reload();
                handle.session.add_log(LogEntry::info(
                    LogSource::App,
                    "Reloading...".to_string(),
                ));
                return UpdateResult::action(UpdateAction::SpawnTask(Task::Reload {
                    session_id,
                    app_id,
                }));
            }
        }
    }
    
    // No session or app running - show error to selected session if possible
    if let Some(handle) = state.session_manager.selected_mut() {
        handle.session.add_log(LogEntry::error(
            LogSource::App,
            "No app running to reload".to_string(),
        ));
    }
    UpdateResult::none()
}
```

---

**Message::HotRestart (lines 142-180)**

Remove fallback block (lines 162-178):
```rust
// REMOVE THIS BLOCK:
// Fall back to legacy global app_id (uses global state)
if state.is_busy() {
    return UpdateResult::none();
}
if let Some(app_id) = state.current_app_id.clone() {
    state.start_reload();
    state.log_info(LogSource::App, "Restarting (legacy mode)...");
    UpdateResult::action(UpdateAction::SpawnTask(Task::Restart {
        session_id: 0,
        app_id,
    }))
} else {
    state.log_error(LogSource::App, "No app running to restart");
    UpdateResult::none()
}
```

**New behavior**: Same pattern as HotReload - error to session if no app running.

---

**Message::StopApp (lines 182-214)**

Remove fallback block (lines 198-210):
```rust
// REMOVE THIS BLOCK:
// Fall back to legacy global app_id
if let Some(app_id) = state.current_app_id.clone() {
    state.log_info(LogSource::App, "Stopping app (legacy mode)...");
    UpdateResult::action(UpdateAction::SpawnTask(Task::Stop {
        session_id: 0,
        app_id,
    }))
} else {
    state.log_error(LogSource::App, "No app running to stop");
    UpdateResult::none()
}
```

**New behavior**: Same pattern - error to session if no app running.

---

**Message::AutoReloadTriggered (lines 304-356)**

Remove fallback block (lines 337-349):
```rust
// REMOVE THIS BLOCK:
// Fall back to legacy global app_id (for backward compatibility)
if !state.is_busy() {
    if let Some(app_id) = state.current_app_id.clone() {
        state.log_info(LogSource::Watcher, "File change detected, reloading...");
        state.start_reload();
        return UpdateResult::action(UpdateAction::SpawnTask(Task::Reload {
            session_id: 0,
            app_id,
        }));
    }
}
```

**Note**: For AutoReloadTriggered, the "no running sessions" case is already handled by the reloadable_sessions check. We just remove the fallback, the rest stays.

---

### Implementation Steps

1. **Update Message::HotReload handler**
   - Remove fallback block
   - Log error to session instead of global state
   - Keep early return for busy session

2. **Update Message::HotRestart handler**
   - Same changes as HotReload

3. **Update Message::StopApp handler**
   - Same changes as HotReload

4. **Update Message::AutoReloadTriggered handler**
   - Remove fallback block
   - Keep existing reloadable_sessions logic

5. **Compile and verify**
   - Should have no references to `current_app_id` in update.rs
   - Should have no `session_id: 0` task spawns

---

### Code Patterns After Removal

All control handlers will follow this pattern:

```rust
Message::SomeControlAction => {
    // Try selected session first
    if let Some(handle) = state.session_manager.selected_mut() {
        // Check busy state
        if handle.session.is_busy() {
            return UpdateResult::none();
        }
        // Check for running app
        if let Some(app_id) = handle.session.app_id.clone() {
            if handle.cmd_sender.is_some() {
                // Execute action on session
                return UpdateResult::action(...);
            }
        }
    }
    
    // No session/app - error or no-op
    if let Some(handle) = state.session_manager.selected_mut() {
        handle.session.add_log(LogEntry::error(...));
    }
    UpdateResult::none()
}
```

---

### Files Changed Summary

| File | Lines Removed | Lines Changed |
|------|---------------|---------------|
| `update.rs` | ~50 | ~15 (error logging) |

**Total: ~50 lines removed, ~15 lines modified**

---

### Acceptance Criteria

1. ✅ No fallback to `state.current_app_id` in HotReload handler
2. ✅ No fallback to `state.current_app_id` in HotRestart handler
3. ✅ No fallback to `state.current_app_id` in StopApp handler
4. ✅ No fallback to `state.current_app_id` in AutoReloadTriggered handler
5. ✅ No `session_id: 0` in any Task spawn
6. ✅ Errors logged to session, not global state
7. ✅ `cargo check` passes
8. ✅ `cargo clippy` shows no warnings
9. ✅ Control actions still work on selected session

---

### Testing

#### Compile-Time Verification
- `cargo check` passes
- No references to `current_app_id` in update.rs (grep verification)
- No `session_id: 0` patterns in update.rs

#### Unit Tests
**Tests to update in Task 4g:**
- `test_hot_reload_message_starts_reload` - must use session
- `test_hot_reload_without_app_id_shows_error` - error goes to session
- `test_hot_reload_ignored_when_busy` - use session's is_busy
- `test_reload_ignored_when_already_reloading` - use session
- `test_restart_ignored_when_already_reloading` - use session
- `test_stop_ignored_when_already_reloading` - use session
- `test_reload_no_app_running_shows_error` - error to session
- `test_restart_no_app_running_shows_error` - error to session
- `test_stop_no_app_running_shows_error` - error to session
- `test_auto_reload_triggered_when_app_running` - use session
- `test_auto_reload_skipped_when_no_app` - no session case
- `test_auto_reload_skipped_when_busy` - session busy
- `test_reload_elapsed_tracking` - session tracking
- `test_reload_uses_session_when_no_cmd_sender` - update
- `test_auto_reload_falls_back_to_legacy` - REMOVE

#### Runtime Testing
1. Start session, verify hot reload works (r key)
2. Start session, verify hot restart works (R key)
3. Start session, verify stop works (s key)
4. Save file, verify auto-reload works
5. With no session, verify no crash on r/R/s keys

---

### Edge Cases

1. **No session selected**
   - Control keys should do nothing (no crash)
   - Auto-reload should skip (no sessions to reload)

2. **Session selected but no app running (building)**
   - Should show "No app running" error in session log
   - Should not crash or use legacy path

3. **Session selected but no cmd_sender (process starting)**
   - Should skip action silently (will work once process ready)

---

### Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Breaking control actions | Verify each action still works after change |
| Missing edge cases | Test with various session states |
| Error messages lost | Ensure errors go to session log |

---

### Estimated Effort

**1 hour**

- 0.5 hours: Remove fallback blocks
- 0.25 hours: Update error logging
- 0.25 hours: Compile and test