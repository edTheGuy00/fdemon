## Task: Map X Key to Close Current Session

**Objective**: Map the 'x' key and Ctrl+W to `Message::CloseCurrentSession` in normal mode, allowing users to close the currently selected session without quitting the entire application.

**Depends on**: None (can be implemented independently)

---

### Scope

- `src/app/handler.rs`: Add key mappings in `handle_key_normal` function
- `src/app/message.rs`: Verify `CloseCurrentSession` message exists

---

### Current State

```rust
// In src/app/handler.rs - handle_key_normal
fn handle_key_normal(state: &AppState, key: KeyEvent, is_busy: bool) -> Option<Message> {
    match (key.code, key.modifiers) {
        // Quit - always allowed
        (KeyCode::Char('q'), KeyModifiers::NONE) => Some(Message::Quit),
        (KeyCode::Esc, _) => Some(Message::Quit),
        // ... other key mappings ...
        
        // NO 'x' key mapping exists!
        
        _ => None,
    }
}

// CloseCurrentSession message exists and is handled:
Message::CloseCurrentSession => {
    // ... removes session from manager ...
}
```

**Problem:** There's no way for users to close just the current session - they can only quit the entire application.

---

### Implementation Details

#### 1. Add 'x' and Ctrl+W Key Mappings

```rust
// In src/app/handler.rs - handle_key_normal
fn handle_key_normal(state: &AppState, key: KeyEvent, is_busy: bool) -> Option<Message> {
    match (key.code, key.modifiers) {
        // Quit - always allowed
        (KeyCode::Char('q'), KeyModifiers::NONE) => Some(Message::Quit),
        (KeyCode::Esc, _) => Some(Message::Quit),
        (KeyCode::Char('c'), m) if m.contains(KeyModifiers::CONTROL) => Some(Message::Quit),

        // ─────────────────────────────────────────────────────────
        // Session Management
        // ─────────────────────────────────────────────────────────
        // Close current session with 'x' or Ctrl+W
        (KeyCode::Char('x'), KeyModifiers::NONE) => Some(Message::CloseCurrentSession),
        (KeyCode::Char('w'), m) if m.contains(KeyModifiers::CONTROL) => {
            Some(Message::CloseCurrentSession)
        }

        // ... rest of existing key mappings ...
        
        _ => None,
    }
}
```

#### 2. Verify CloseCurrentSession Handler Works Correctly

The existing handler should already:
- Remove the session from SessionManager
- Switch to another session if available
- Show device selector if no sessions remain

```rust
// Existing handler in handler.rs
Message::CloseCurrentSession => {
    if let Some(session_id) = state.session_manager.selected_id() {
        let app_id = state
            .session_manager
            .get(session_id)
            .and_then(|h| h.session.app_id.clone());

        if let Some(app_id) = app_id {
            state.log_info(LogSource::App, "Stopping app before closing session...");
            state.session_manager.remove_session(session_id);

            if state.session_manager.is_empty() {
                state.ui_mode = UiMode::DeviceSelector;
                state.device_selector.show_loading();
                return UpdateResult::action(UpdateAction::DiscoverDevices);
            }

            return UpdateResult::action(UpdateAction::SpawnTask(Task::Stop { app_id }));
        }

        state.session_manager.remove_session(session_id);

        if state.session_manager.is_empty() {
            state.ui_mode = UiMode::DeviceSelector;
            state.device_selector.show_loading();
            return UpdateResult::action(UpdateAction::DiscoverDevices);
        }
    }
    UpdateResult::none()
}
```

#### 3. Update Header Keybinding Hints

The header should show 'x' as an available action:

```rust
// In widgets/header.rs or similar
// Add [x] to the keybinding hints shown in the header
// "[r] Reload  [R] Restart  [x] Close  [q] Quit"
```

---

### Expected Behavior After Implementation

| Scenario | 'x' Key Action |
|----------|----------------|
| Single session running | Close session, show device selector |
| Multiple sessions, session 2 selected | Close session 2, switch to session 1 |
| Multiple sessions, session 1 selected | Close session 1, switch to session 2 |
| No sessions (device selector shown) | No effect (key not active in this mode) |
| Session with running app | Stop app, then close session |

---

### Acceptance Criteria

1. [ ] 'x' key in normal mode triggers `Message::CloseCurrentSession`
2. [ ] Ctrl+W in normal mode triggers `Message::CloseCurrentSession`
3. [ ] Closing session with running app sends stop command first
4. [ ] Closing last session shows device selector
5. [ ] Closing one of multiple sessions switches to another
6. [ ] 'x' key doesn't work in device selector mode (no effect)
7. [ ] Header shows '[x]' in keybinding hints

---

### Testing

```rust
#[test]
fn test_x_key_closes_session() {
    let mut state = AppState::new();
    state.ui_mode = UiMode::Normal;
    
    let device = test_device("d1", "iPhone 15");
    state.session_manager.create_session(&device).unwrap();
    
    let key = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE);
    let result = handle_key(&state, key);
    
    assert!(matches!(result, Some(Message::CloseCurrentSession)));
}

#[test]
fn test_ctrl_w_closes_session() {
    let state = AppState::new();
    
    let key = KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL);
    let result = handle_key(&state, key);
    
    assert!(matches!(result, Some(Message::CloseCurrentSession)));
}

#[test]
fn test_x_key_in_device_selector_no_effect() {
    let mut state = AppState::new();
    state.ui_mode = UiMode::DeviceSelector;
    
    let key = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE);
    let result = handle_key(&state, key);
    
    // Should not produce CloseCurrentSession in device selector mode
    assert!(!matches!(result, Some(Message::CloseCurrentSession)));
}

#[test]
fn test_close_session_stops_app_first() {
    let mut state = AppState::new();
    
    let device = test_device("d1", "iPhone 15");
    let session_id = state.session_manager.create_session(&device).unwrap();
    
    // Mark session as running with app_id
    state.session_manager.get_mut(session_id).unwrap().session.mark_started("app-123".into());
    
    let result = update(&mut state, Message::CloseCurrentSession);
    
    // Should return Stop task action
    assert!(matches!(
        result.action,
        Some(UpdateAction::SpawnTask(Task::Stop { app_id })) if app_id == "app-123"
    ));
}

#[test]
fn test_close_last_session_shows_device_selector() {
    let mut state = AppState::new();
    state.ui_mode = UiMode::Normal;
    
    let device = test_device("d1", "iPhone 15");
    state.session_manager.create_session(&device).unwrap();
    
    assert_eq!(state.session_manager.len(), 1);
    
    update(&mut state, Message::CloseCurrentSession);
    
    assert!(state.session_manager.is_empty());
    assert_eq!(state.ui_mode, UiMode::DeviceSelector);
}

#[test]
fn test_close_one_of_multiple_sessions() {
    let mut state = AppState::new();
    state.ui_mode = UiMode::Normal;
    
    let d1 = test_device("d1", "iPhone 15");
    let d2 = test_device("d2", "Pixel 8");
    state.session_manager.create_session(&d1).unwrap();
    let id2 = state.session_manager.create_session(&d2).unwrap();
    
    // Select session 2
    state.session_manager.select_by_id(id2);
    
    update(&mut state, Message::CloseCurrentSession);
    
    // Should have 1 session remaining
    assert_eq!(state.session_manager.len(), 1);
    
    // Should still be in normal mode
    assert_eq!(state.ui_mode, UiMode::Normal);
}
```

---

### Notes

- The 'x' key is intuitive for "close" (like closing a tab)
- Ctrl+W follows standard terminal/browser conventions for closing tabs
- The handler already exists and works; this task just adds the key binding
- Consider adding a brief "Session closed" log message
- The stop command is sent but we don't wait for confirmation before removing the session

---

## Completion Summary

**Status:** ✅ Done

**Files Modified:**
- `src/tui/widgets/header.rs` - Added `[x]` to keybinding hints in header (changed from `[r] [R] [d] [q]` to `[r] [R] [x] [d] [q]`)

**Key Findings:**
The 'x' and Ctrl+W key bindings already existed in `handle_key_normal` from earlier implementation:
- `src/app/handler.rs:1159` - 'x' key maps to `CloseCurrentSession`
- `src/app/handler.rs:1161` - Ctrl+W maps to `CloseCurrentSession`

The `CloseCurrentSession` handler already correctly:
- Sends stop command for running apps before closing
- Removes session from SessionManager
- Shows device selector when no sessions remain
- Mode-specific key handling ensures 'x' doesn't work in device selector

**Testing Performed:**
- `cargo check` - compilation successful
- `cargo test` - all tests pass
- `cargo fmt` - code formatted

**Acceptance Criteria Met:**
- [x] 'x' key in normal mode triggers `Message::CloseCurrentSession` (pre-existing)
- [x] Ctrl+W in normal mode triggers `Message::CloseCurrentSession` (pre-existing)
- [x] Closing session with running app sends stop command first (pre-existing)
- [x] Closing last session shows device selector (pre-existing)
- [x] Closing one of multiple sessions switches to another (pre-existing)
- [x] 'x' key doesn't work in device selector mode (separate key handler)
- [x] Header shows '[x]' in keybinding hints (added)