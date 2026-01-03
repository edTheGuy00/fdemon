## Task: Q Key Request Quit Flow

**Objective**: Change the 'q' key handler to call `state.request_quit()` instead of directly sending `Message::Quit`, enabling the confirmation dialog to be shown when sessions are running.

**Depends on**: None (can be implemented independently)

---

### Scope

- `src/app/handler.rs`: Update 'q' key handling in `handle_key_normal`
- `src/app/message.rs`: Add `Message::RequestQuit` variant
- `src/app/state.rs`: Verify `request_quit()` method exists and works correctly

---

### Current State

```rust
// In src/app/handler.rs - handle_key_normal
fn handle_key_normal(state: &AppState, key: KeyEvent, is_busy: bool) -> Option<Message> {
    match (key.code, key.modifiers) {
        // Quit - always allowed - DIRECTLY QUITS!
        (KeyCode::Char('q'), KeyModifiers::NONE) => Some(Message::Quit),
        (KeyCode::Esc, _) => Some(Message::Quit),
        (KeyCode::Char('c'), m) if m.contains(KeyModifiers::CONTROL) => Some(Message::Quit),
        // ...
    }
}

// Message::Quit handler immediately quits
Message::Quit => {
    state.phase = AppPhase::Quitting;
    UpdateResult::none()
}
```

**Problem:** 
- 'q' immediately quits without confirmation
- Running Flutter processes are not properly stopped
- `state.request_quit()` exists but is never called:

```rust
// In state.rs - request_quit exists but is unused!
pub fn request_quit(&mut self) {
    if self.has_running_sessions() && self.settings.behavior.confirm_quit {
        self.ui_mode = UiMode::ConfirmDialog;
    } else {
        self.phase = AppPhase::Quitting;
    }
}
```

---

### Implementation Details

#### 1. Add Message::RequestQuit Variant

```rust
// In src/app/message.rs
pub enum Message {
    // ... existing variants ...
    
    /// Request application quit (may show confirmation dialog)
    RequestQuit,
    
    /// Force quit without confirmation (Ctrl+C, signal handler)
    Quit,
    
    /// Confirm quit from dialog
    ConfirmQuit,
    
    /// Cancel quit from dialog
    CancelQuit,
}
```

#### 2. Update Key Mappings

```rust
// In src/app/handler.rs - handle_key_normal
fn handle_key_normal(state: &AppState, key: KeyEvent, is_busy: bool) -> Option<Message> {
    match (key.code, key.modifiers) {
        // Request quit (may show confirmation)
        (KeyCode::Char('q'), KeyModifiers::NONE) => Some(Message::RequestQuit),
        (KeyCode::Esc, _) => Some(Message::RequestQuit),
        
        // Force quit (bypass confirmation) - Ctrl+C should still work immediately
        (KeyCode::Char('c'), m) if m.contains(KeyModifiers::CONTROL) => Some(Message::Quit),
        
        // ... rest of key mappings ...
    }
}
```

#### 3. Handle RequestQuit Message

```rust
// In src/app/handler.rs - update function
Message::RequestQuit => {
    state.request_quit();
    UpdateResult::none()
}

Message::ConfirmQuit => {
    state.confirm_quit();
    UpdateResult::none()
}

Message::CancelQuit => {
    state.cancel_quit();
    UpdateResult::none()
}

// Keep Message::Quit for force quit and signal handler
Message::Quit => {
    state.phase = AppPhase::Quitting;
    UpdateResult::none()
}
```

#### 4. Update Confirm Dialog Key Handler

```rust
// In src/app/handler.rs - handle_key_confirm_dialog
fn handle_key_confirm_dialog(key: KeyEvent) -> Option<Message> {
    match key.code {
        // Confirm quit
        KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => Some(Message::ConfirmQuit),
        
        // Cancel
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => Some(Message::CancelQuit),
        
        // Force quit with Ctrl+C even in dialog
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(Message::Quit),
        
        _ => None,
    }
}
```

#### 5. Verify state.rs Methods

```rust
// In src/app/state.rs - these should already exist
impl AppState {
    /// Request application quit - shows confirmation if sessions running
    pub fn request_quit(&mut self) {
        if self.has_running_sessions() && self.settings.behavior.confirm_quit {
            self.ui_mode = UiMode::ConfirmDialog;
        } else {
            self.phase = AppPhase::Quitting;
        }
    }

    /// Force quit without confirmation
    pub fn force_quit(&mut self) {
        self.phase = AppPhase::Quitting;
    }

    /// Confirm quit from confirmation dialog
    pub fn confirm_quit(&mut self) {
        self.phase = AppPhase::Quitting;
    }

    /// Cancel quit from confirmation dialog
    pub fn cancel_quit(&mut self) {
        self.ui_mode = UiMode::Normal;
    }
    
    /// Check if any session is running
    pub fn has_running_sessions(&self) -> bool {
        self.session_manager.has_running_sessions()
    }
}
```

---

### Quit Flow After Implementation

```
User presses 'q'
       │
       ▼
Message::RequestQuit
       │
       ▼
state.request_quit()
       │
       ├── No running sessions OR confirm_quit=false
       │            │
       │            ▼
       │   state.phase = Quitting → Exit app
       │
       └── Has running sessions AND confirm_quit=true
                    │
                    ▼
           state.ui_mode = ConfirmDialog
                    │
                    ▼
         ┌─────────────────────┐
         │  Quit all sessions? │
         │     [y] Yes  [n] No │
         └─────────────────────┘
                    │
        ┌───────────┴───────────┐
        │                       │
        ▼                       ▼
   'y' pressed             'n' pressed
        │                       │
        ▼                       ▼
Message::ConfirmQuit    Message::CancelQuit
        │                       │
        ▼                       ▼
state.confirm_quit()    state.cancel_quit()
        │                       │
        ▼                       ▼
phase = Quitting        ui_mode = Normal
        │                       │
        ▼                       ▼
   Exit app              Continue running
```

---

### Acceptance Criteria

1. [ ] 'q' key triggers `Message::RequestQuit` instead of `Message::Quit`
2. [ ] Esc key triggers `Message::RequestQuit`
3. [ ] Ctrl+C still triggers immediate `Message::Quit` (force quit)
4. [ ] With running sessions and `confirm_quit=true`, dialog is shown
5. [ ] Without running sessions, app quits immediately
6. [ ] With `confirm_quit=false` in settings, app quits immediately
7. [ ] 'y' in dialog triggers `Message::ConfirmQuit`
8. [ ] 'n' in dialog triggers `Message::CancelQuit`
9. [ ] Cancel returns to normal mode

---

### Testing

```rust
#[test]
fn test_q_key_produces_request_quit() {
    let state = AppState::new();
    
    let key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
    let result = handle_key(&state, key);
    
    assert!(matches!(result, Some(Message::RequestQuit)));
}

#[test]
fn test_esc_key_produces_request_quit() {
    let mut state = AppState::new();
    state.ui_mode = UiMode::Normal;
    
    let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    let result = handle_key(&state, key);
    
    assert!(matches!(result, Some(Message::RequestQuit)));
}

#[test]
fn test_ctrl_c_produces_force_quit() {
    let state = AppState::new();
    
    let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    let result = handle_key(&state, key);
    
    assert!(matches!(result, Some(Message::Quit)));
}

#[test]
fn test_request_quit_no_sessions_quits_immediately() {
    let mut state = AppState::new();
    state.settings.behavior.confirm_quit = true;
    
    // No sessions
    assert!(state.session_manager.is_empty());
    
    update(&mut state, Message::RequestQuit);
    
    // Should quit immediately
    assert_eq!(state.phase, AppPhase::Quitting);
}

#[test]
fn test_request_quit_with_sessions_shows_dialog() {
    let mut state = AppState::new();
    state.settings.behavior.confirm_quit = true;
    
    // Create a running session
    let device = test_device("d1", "iPhone 15");
    let id = state.session_manager.create_session(&device).unwrap();
    state.session_manager.get_mut(id).unwrap().session.mark_started("app-1".into());
    
    update(&mut state, Message::RequestQuit);
    
    // Should show dialog, not quit
    assert_ne!(state.phase, AppPhase::Quitting);
    assert_eq!(state.ui_mode, UiMode::ConfirmDialog);
}

#[test]
fn test_request_quit_confirm_quit_disabled_quits_immediately() {
    let mut state = AppState::new();
    state.settings.behavior.confirm_quit = false;
    
    // Create a running session
    let device = test_device("d1", "iPhone 15");
    let id = state.session_manager.create_session(&device).unwrap();
    state.session_manager.get_mut(id).unwrap().session.mark_started("app-1".into());
    
    update(&mut state, Message::RequestQuit);
    
    // Should quit immediately despite running session
    assert_eq!(state.phase, AppPhase::Quitting);
}

#[test]
fn test_confirm_quit_sets_quitting_phase() {
    let mut state = AppState::new();
    state.ui_mode = UiMode::ConfirmDialog;
    
    update(&mut state, Message::ConfirmQuit);
    
    assert_eq!(state.phase, AppPhase::Quitting);
}

#[test]
fn test_cancel_quit_returns_to_normal() {
    let mut state = AppState::new();
    state.ui_mode = UiMode::ConfirmDialog;
    
    update(&mut state, Message::CancelQuit);
    
    assert_eq!(state.ui_mode, UiMode::Normal);
    assert_ne!(state.phase, AppPhase::Quitting);
}

#[test]
fn test_y_key_in_dialog_confirms() {
    let mut state = AppState::new();
    state.ui_mode = UiMode::ConfirmDialog;
    
    let key = KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE);
    let result = handle_key(&state, key);
    
    assert!(matches!(result, Some(Message::ConfirmQuit)));
}

#[test]
fn test_n_key_in_dialog_cancels() {
    let mut state = AppState::new();
    state.ui_mode = UiMode::ConfirmDialog;
    
    let key = KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE);
    let result = handle_key(&state, key);
    
    assert!(matches!(result, Some(Message::CancelQuit)));
}
```

---

### Notes

- Ctrl+C remains a force quit for emergency exit (e.g., frozen UI)
- The `confirm_quit` setting in `.fdemon/config.toml` controls this behavior
- Default for `confirm_quit` should be `true` to prevent accidental quits
- The confirmation dialog UI rendering is handled in Task 09
- Signal handler (SIGINT/SIGTERM) should continue to send `Message::Quit` for proper cleanup

---

## Completion Summary

**Status:** ✅ Done

**Files Modified:**
- `src/app/message.rs` - Added `RequestQuit`, `ConfirmQuit`, `CancelQuit` message variants
- `src/app/handler.rs`:
  - Updated 'q' and Esc key handlers to produce `RequestQuit` instead of `Quit`
  - Added handlers for `RequestQuit`, `ConfirmQuit`, `CancelQuit`
  - Updated `handle_key_confirm_dialog` to use new messages
  - Updated 2 existing tests and added 9 new tests

**Key Changes:**
- `'q'` key → `Message::RequestQuit` (was `Message::Quit`)
- `Esc` key → `Message::RequestQuit` (was `Message::Quit`)
- `Ctrl+C` → `Message::Quit` (unchanged, force quit)
- `'y'`/`'Y'`/Enter in dialog → `Message::ConfirmQuit`
- `'n'`/`'N'`/Esc in dialog → `Message::CancelQuit`
- `Ctrl+C` in dialog → `Message::Quit` (force quit)

**State Methods Used:**
- `state.request_quit()` - shows dialog if sessions running + confirm_quit enabled
- `state.confirm_quit()` - sets phase to Quitting
- `state.cancel_quit()` - returns to Normal mode

**Testing Performed:**
- `cargo check` - compilation successful
- `cargo test` - all 425 tests pass (9 new tests added)
- `cargo fmt` - code formatted

**Acceptance Criteria Met:**
- [x] 'q' key triggers `Message::RequestQuit`
- [x] Esc key triggers `Message::RequestQuit`
- [x] Ctrl+C triggers immediate `Message::Quit` (force quit)
- [x] With running sessions and `confirm_quit=true`, dialog is shown
- [x] Without running sessions, app quits immediately
- [x] With `confirm_quit=false`, app quits immediately
- [x] 'y' in dialog triggers `Message::ConfirmQuit`
- [x] 'n' in dialog triggers `Message::CancelQuit`
- [x] Cancel returns to normal mode

**Note:** The confirmation dialog UI rendering (Task 09) is separate and must be completed for the dialog to be visible to users.