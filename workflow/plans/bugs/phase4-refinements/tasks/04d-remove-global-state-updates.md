## Task 4d: Remove Legacy Global State Updates

**Objective**: Stop updating global `AppState` fields when session events occur. These updates were for backward compatibility and are no longer needed after tasks 4a-4c.

**Depends on**: Task 4c (fallback paths must be removed first)

---

### Background

Currently, when session events occur, the handlers update both:
1. The session's own state (correct)
2. Global `AppState` fields (legacy compatibility)

These global updates served no purpose after multi-session was implemented, except to support the legacy fallback paths that Task 4c removed.

---

### Scope

#### `src/app/handler/session.rs`

**Remove global state updates in `handle_session_message_state` (lines 99-128)**

Current code:
```rust
pub fn handle_session_message_state(
    state: &mut AppState,
    session_id: SessionId,
    msg: &DaemonMessage,
) {
    // Handle app.start event - capture app_id in session
    if let DaemonMessage::AppStart(app_start) = msg {
        if let Some(handle) = state.session_manager.get_mut(session_id) {
            handle.session.mark_started(app_start.app_id.clone());
            tracing::info!(
                "Session {} app started: app_id={}",
                session_id,
                app_start.app_id
            );
        }
        // Also update global state for legacy compatibility  <-- REMOVE
        state.current_app_id = Some(app_start.app_id.clone()); // <-- REMOVE
    }

    // Handle app.stop event
    if let DaemonMessage::AppStop(app_stop) = msg {
        if let Some(handle) = state.session_manager.get_mut(session_id) {
            if handle.session.app_id.as_ref() == Some(&app_stop.app_id) {
                handle.session.app_id = None;
                handle.session.phase = AppPhase::Initializing;
                tracing::info!(
                    "Session {} app stopped: app_id={}",
                    session_id,
                    app_stop.app_id
                );
            }
        }
        // Also update global state for legacy compatibility  <-- REMOVE
        if state.current_app_id.as_ref() == Some(&app_stop.app_id) { // <-- REMOVE
            state.current_app_id = None;                               // <-- REMOVE
        }                                                              // <-- REMOVE
    }
}
```

**After removal:**
```rust
pub fn handle_session_message_state(
    state: &mut AppState,
    session_id: SessionId,
    msg: &DaemonMessage,
) {
    // Handle app.start event - capture app_id in session
    if let DaemonMessage::AppStart(app_start) = msg {
        if let Some(handle) = state.session_manager.get_mut(session_id) {
            handle.session.mark_started(app_start.app_id.clone());
            tracing::info!(
                "Session {} app started: app_id={}",
                session_id,
                app_start.app_id
            );
        }
    }

    // Handle app.stop event
    if let DaemonMessage::AppStop(app_stop) = msg {
        if let Some(handle) = state.session_manager.get_mut(session_id) {
            if handle.session.app_id.as_ref() == Some(&app_stop.app_id) {
                handle.session.app_id = None;
                handle.session.phase = AppPhase::Initializing;
                tracing::info!(
                    "Session {} app stopped: app_id={}",
                    session_id,
                    app_stop.app_id
                );
            }
        }
    }
}
```

---

#### `src/app/handler/update.rs`

**Remove global state updates in `Message::SessionStarted` handler (around lines 590-605)**

Current code:
```rust
Message::SessionStarted {
    session_id,
    device_id,
    device_name,
    platform,
    pid,
} => {
    // Update session state
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        handle.session.phase = AppPhase::Running;
        // ... session updates ...
    }

    // Also update legacy global state for backward compatibility <-- REMOVE
    state.device_name = Some(device_name.clone());                 // <-- REMOVE
    state.platform = Some(platform.clone());                       // <-- REMOVE

    UpdateResult::none()
}
```

**After removal:**
```rust
Message::SessionStarted {
    session_id,
    device_id: _,  // Now unused, prefix with _
    device_name,
    platform,
    pid,
} => {
    // Update session state
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        handle.session.phase = AppPhase::Running;
        handle.session.add_log(LogEntry::info(
            LogSource::App,
            format!(
                "Flutter started on {} ({}) - PID: {:?}",
                device_name, platform, pid
            ),
        ));
    }

    UpdateResult::none()
}
```

---

### Implementation Steps

1. **Update session.rs**
   - Remove `state.current_app_id = Some(...)` line in AppStart handler
   - Remove `state.current_app_id = None` block in AppStop handler
   - Remove the associated comments mentioning "legacy compatibility"

2. **Update update.rs**
   - Remove `state.device_name = Some(...)` line
   - Remove `state.platform = Some(...)` line
   - Prefix unused parameters with `_` if needed

3. **Compile and verify**
   - `cargo check` should pass
   - May get warnings about unused fields in AppState (expected, will be removed in 4e)

---

### Files Changed Summary

| File | Lines Removed | Lines Changed |
|------|---------------|---------------|
| `session.rs` | 6 | 0 |
| `update.rs` | 4 | 1 (parameter prefix) |

**Total: ~10 lines removed**

---

### Acceptance Criteria

1. ✅ No `state.current_app_id = ` assignments in session.rs
2. ✅ No `state.device_name = ` assignments in update.rs
3. ✅ No `state.platform = ` assignments in update.rs
4. ✅ No comments mentioning "legacy compatibility" in these handlers
5. ✅ Session-level state still updated correctly
6. ✅ `cargo check` passes
7. ✅ Clippy may warn about unused fields (expected for Task 4e)

---

### Testing

#### Compile-Time Verification
- `cargo check` passes
- May see warnings about unused `current_app_id`, `device_name`, `platform` fields
- These warnings are expected and will be resolved in Task 4e

#### Unit Tests
**Tests to update in Task 4g:**
- `test_session_started_updates_legacy_global_state` - REMOVE entirely

#### Runtime Testing
1. Start a session
2. Verify session's app_id is set correctly (session state)
3. Verify session's phase transitions correctly
4. Verify logs show device info correctly
5. Verify hot reload still works (uses session app_id, not global)

---

### Edge Cases

1. **Multiple sessions with different app_ids**
   - Each session has its own app_id
   - No global app_id to conflict
   - All operations use session-specific state

2. **Session closes while others running**
   - Only that session's state changes
   - Other sessions unaffected
   - No global state pollution

---

### Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Missing some global update | Grep for `state.current_app_id` after removal |
| Breaking session functionality | Verify session state still updates correctly |
| Unused field warnings | Expected, addressed in Task 4e |

---

### Verification Commands

After making changes, run:

```bash
# Verify no global state updates remain
grep -n "state\.current_app_id\s*=" src/app/handler/*.rs
grep -n "state\.device_name\s*=" src/app/handler/*.rs  
grep -n "state\.platform\s*=" src/app/handler/*.rs

# Should return no matches
```

---

### Estimated Effort

**30 minutes**

- 15 minutes: Remove the lines
- 15 minutes: Compile and verify