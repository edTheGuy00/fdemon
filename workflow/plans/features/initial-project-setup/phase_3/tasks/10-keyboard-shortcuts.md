## Task: Keyboard Shortcuts for Tab Navigation and App Control

**Objective**: Implement comprehensive keyboard shortcuts for tab navigation between sessions, stopping apps, and other control operations. This enables efficient multi-session management without leaving the keyboard.

**Depends on**: [09-refined-layout](09-refined-layout.md)

---

### Scope

- `src/tui/event.rs`: Add new key mappings for tab navigation and control
- `src/app/handler.rs`: Handle new keyboard messages
- `src/app/message.rs`: Add new message types for shortcuts

---

### Implementation Details

#### Keyboard Shortcut Reference

| Key | Action | Context |
|-----|--------|---------|
| `1-9` | Switch to session N | Normal mode, multiple sessions |
| `Tab` | Next session | Normal mode, multiple sessions |
| `Shift+Tab` | Previous session | Normal mode, multiple sessions |
| `n` | New session (show device selector) | Normal mode |
| `s` | Stop current app | Normal mode, session running |
| `x` / `Ctrl+W` | Close current session | Normal mode |
| `r` | Hot reload | Normal mode, session running |
| `R` | Hot restart | Normal mode, session running |
| `d` | Open DevTools | Normal mode, session running |
| `c` | Clear logs | Normal mode |
| `q` / `Esc` | Quit (or cancel modal) | Any mode |
| `↑` / `k` | Scroll up / Navigate up | Log view / Modals |
| `↓` / `j` | Scroll down / Navigate down | Log view / Modals |
| `Enter` | Confirm selection | Modals |
| `?` | Show help overlay | Normal mode |

#### Event Handling (`src/tui/event.rs`)

```rust
//! Terminal event handling

use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};

use crate::app::message::Message;
use crate::app::state::UiMode;

/// Poll for terminal events with timeout
/// 
/// Returns None if no event is available within the timeout.
pub fn poll() -> std::io::Result<Option<Message>> {
    poll_with_timeout(Duration::from_millis(16)) // ~60fps
}

/// Poll for terminal events with custom timeout
pub fn poll_with_timeout(timeout: Duration) -> std::io::Result<Option<Message>> {
    if !event::poll(timeout)? {
        return Ok(None);
    }
    
    match event::read()? {
        Event::Key(key) => Ok(Some(key_to_message(key))),
        Event::Resize(_, _) => Ok(Some(Message::Resize)),
        _ => Ok(None),
    }
}

/// Convert a key event to a message based on current UI mode
fn key_to_message(key: KeyEvent) -> Message {
    // Note: UI mode context is handled in the handler, not here
    // This function just translates raw key events to messages
    
    match (key.code, key.modifiers) {
        // Quit / Cancel
        (KeyCode::Char('q'), KeyModifiers::NONE) => Message::Quit,
        (KeyCode::Esc, _) => Message::Cancel,
        
        // Tab navigation with number keys
        (KeyCode::Char('1'), KeyModifiers::NONE) => Message::SelectSessionByIndex(0),
        (KeyCode::Char('2'), KeyModifiers::NONE) => Message::SelectSessionByIndex(1),
        (KeyCode::Char('3'), KeyModifiers::NONE) => Message::SelectSessionByIndex(2),
        (KeyCode::Char('4'), KeyModifiers::NONE) => Message::SelectSessionByIndex(3),
        (KeyCode::Char('5'), KeyModifiers::NONE) => Message::SelectSessionByIndex(4),
        (KeyCode::Char('6'), KeyModifiers::NONE) => Message::SelectSessionByIndex(5),
        (KeyCode::Char('7'), KeyModifiers::NONE) => Message::SelectSessionByIndex(6),
        (KeyCode::Char('8'), KeyModifiers::NONE) => Message::SelectSessionByIndex(7),
        (KeyCode::Char('9'), KeyModifiers::NONE) => Message::SelectSessionByIndex(8),
        
        // Tab navigation with Tab key
        (KeyCode::Tab, KeyModifiers::NONE) => Message::NextSession,
        (KeyCode::BackTab, _) => Message::PreviousSession, // Shift+Tab
        (KeyCode::Tab, KeyModifiers::SHIFT) => Message::PreviousSession,
        
        // Session control
        (KeyCode::Char('n'), KeyModifiers::NONE) => Message::ShowDeviceSelector,
        (KeyCode::Char('s'), KeyModifiers::NONE) => Message::StopCurrentApp,
        (KeyCode::Char('x'), KeyModifiers::NONE) => Message::CloseCurrentSession,
        (KeyCode::Char('w'), KeyModifiers::CONTROL) => Message::CloseCurrentSession,
        
        // App control
        (KeyCode::Char('r'), KeyModifiers::NONE) => Message::Reload,
        (KeyCode::Char('R'), KeyModifiers::NONE) => Message::Restart,
        (KeyCode::Char('R'), KeyModifiers::SHIFT) => Message::Restart,
        (KeyCode::Char('d'), KeyModifiers::NONE) => Message::OpenDevTools,
        
        // Log view
        (KeyCode::Char('c'), KeyModifiers::NONE) => Message::ClearLogs,
        (KeyCode::Char('g'), KeyModifiers::NONE) => Message::ScrollToTop,
        (KeyCode::Char('G'), KeyModifiers::NONE) => Message::ScrollToBottom,
        (KeyCode::Char('G'), KeyModifiers::SHIFT) => Message::ScrollToBottom,
        
        // Navigation
        (KeyCode::Up, _) => Message::ScrollUp,
        (KeyCode::Down, _) => Message::ScrollDown,
        (KeyCode::Char('k'), KeyModifiers::NONE) => Message::ScrollUp,
        (KeyCode::Char('j'), KeyModifiers::NONE) => Message::ScrollDown,
        (KeyCode::PageUp, _) => Message::PageUp,
        (KeyCode::PageDown, _) => Message::PageDown,
        
        // Confirm / Select
        (KeyCode::Enter, _) => Message::Confirm,
        
        // Help
        (KeyCode::Char('?'), _) => Message::ShowHelp,
        
        // Refresh (for device selector)
        (KeyCode::Char('r'), KeyModifiers::NONE) => Message::Reload, // Context-dependent
        
        // Unhandled
        _ => Message::Noop,
    }
}

/// Convert key event to message with UI mode context
pub fn key_to_message_with_context(key: KeyEvent, ui_mode: UiMode) -> Message {
    match ui_mode {
        UiMode::DeviceSelector => key_to_device_selector_message(key),
        UiMode::EmulatorSelector => key_to_emulator_selector_message(key),
        UiMode::ConfirmDialog => key_to_confirm_dialog_message(key),
        UiMode::Loading => key_to_loading_message(key),
        UiMode::Normal => key_to_message(key),
    }
}

/// Key handling for device selector modal
fn key_to_device_selector_message(key: KeyEvent) -> Message {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => Message::DeviceSelectorUp,
        KeyCode::Down | KeyCode::Char('j') => Message::DeviceSelectorDown,
        KeyCode::Enter => Message::DeviceSelectorConfirm,
        KeyCode::Esc | KeyCode::Char('q') => Message::HideDeviceSelector,
        KeyCode::Char('r') => Message::RefreshDevices,
        _ => Message::Noop,
    }
}

/// Key handling for emulator selector modal
fn key_to_emulator_selector_message(key: KeyEvent) -> Message {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => Message::EmulatorSelectorUp,
        KeyCode::Down | KeyCode::Char('j') => Message::EmulatorSelectorDown,
        KeyCode::Enter => Message::EmulatorSelectorConfirm,
        KeyCode::Esc | KeyCode::Char('q') => Message::HideEmulatorSelector,
        _ => Message::Noop,
    }
}

/// Key handling for confirmation dialog
fn key_to_confirm_dialog_message(key: KeyEvent) -> Message {
    match key.code {
        KeyCode::Char('y') | KeyCode::Enter => Message::ConfirmYes,
        KeyCode::Char('n') | KeyCode::Esc => Message::ConfirmNo,
        _ => Message::Noop,
    }
}

/// Key handling during loading state
fn key_to_loading_message(key: KeyEvent) -> Message {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => Message::Quit,
        _ => Message::Noop,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }
    
    fn key_with_mod(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }
    
    #[test]
    fn test_number_keys_select_session() {
        assert!(matches!(key_to_message(key(KeyCode::Char('1'))), Message::SelectSessionByIndex(0)));
        assert!(matches!(key_to_message(key(KeyCode::Char('5'))), Message::SelectSessionByIndex(4)));
        assert!(matches!(key_to_message(key(KeyCode::Char('9'))), Message::SelectSessionByIndex(8)));
    }
    
    #[test]
    fn test_tab_navigation() {
        assert!(matches!(key_to_message(key(KeyCode::Tab)), Message::NextSession));
        assert!(matches!(key_to_message(key(KeyCode::BackTab)), Message::PreviousSession));
    }
    
    #[test]
    fn test_session_control_keys() {
        assert!(matches!(key_to_message(key(KeyCode::Char('n'))), Message::ShowDeviceSelector));
        assert!(matches!(key_to_message(key(KeyCode::Char('s'))), Message::StopCurrentApp));
        assert!(matches!(key_to_message(key(KeyCode::Char('x'))), Message::CloseCurrentSession));
    }
    
    #[test]
    fn test_app_control_keys() {
        assert!(matches!(key_to_message(key(KeyCode::Char('r'))), Message::Reload));
        assert!(matches!(key_to_message(key(KeyCode::Char('R'))), Message::Restart));
        assert!(matches!(key_to_message(key(KeyCode::Char('d'))), Message::OpenDevTools));
    }
    
    #[test]
    fn test_vim_style_navigation() {
        assert!(matches!(key_to_message(key(KeyCode::Char('j'))), Message::ScrollDown));
        assert!(matches!(key_to_message(key(KeyCode::Char('k'))), Message::ScrollUp));
        assert!(matches!(key_to_message(key(KeyCode::Char('g'))), Message::ScrollToTop));
        assert!(matches!(key_to_message(key(KeyCode::Char('G'))), Message::ScrollToBottom));
    }
    
    #[test]
    fn test_device_selector_context() {
        let up = key_to_message_with_context(key(KeyCode::Up), UiMode::DeviceSelector);
        assert!(matches!(up, Message::DeviceSelectorUp));
        
        let enter = key_to_message_with_context(key(KeyCode::Enter), UiMode::DeviceSelector);
        assert!(matches!(enter, Message::DeviceSelectorConfirm));
        
        let esc = key_to_message_with_context(key(KeyCode::Esc), UiMode::DeviceSelector);
        assert!(matches!(esc, Message::HideDeviceSelector));
    }
    
    #[test]
    fn test_confirm_dialog_context() {
        let y = key_to_message_with_context(key(KeyCode::Char('y')), UiMode::ConfirmDialog);
        assert!(matches!(y, Message::ConfirmYes));
        
        let n = key_to_message_with_context(key(KeyCode::Char('n')), UiMode::ConfirmDialog);
        assert!(matches!(n, Message::ConfirmNo));
    }
    
    #[test]
    fn test_ctrl_w_closes_session() {
        let ctrl_w = key_with_mod(KeyCode::Char('w'), KeyModifiers::CONTROL);
        assert!(matches!(key_to_message(ctrl_w), Message::CloseCurrentSession));
    }
}
```

#### Message Types (`src/app/message.rs`)

```rust
// Additions to existing Message enum

pub enum Message {
    // ... existing variants ...
    
    // ─────────────────────────────────────────────────────────
    // Session Navigation
    // ─────────────────────────────────────────────────────────
    
    /// Select session by index (0-based)
    SelectSessionByIndex(usize),
    
    /// Switch to next session
    NextSession,
    
    /// Switch to previous session
    PreviousSession,
    
    // ─────────────────────────────────────────────────────────
    // Session Control
    // ─────────────────────────────────────────────────────────
    
    /// Stop the current app (but keep session)
    StopCurrentApp,
    
    /// Close the current session entirely
    CloseCurrentSession,
    
    // ─────────────────────────────────────────────────────────
    // Modal Navigation
    // ─────────────────────────────────────────────────────────
    
    /// Emulator selector navigation
    EmulatorSelectorUp,
    EmulatorSelectorDown,
    EmulatorSelectorConfirm,
    HideEmulatorSelector,
    
    /// Device selector confirm (different from generic Confirm)
    DeviceSelectorConfirm,
    
    // ─────────────────────────────────────────────────────────
    // Confirmation Dialog
    // ─────────────────────────────────────────────────────────
    
    /// User confirmed (yes)
    ConfirmYes,
    
    /// User declined (no)
    ConfirmNo,
    
    // ─────────────────────────────────────────────────────────
    // General
    // ─────────────────────────────────────────────────────────
    
    /// Generic confirm action
    Confirm,
    
    /// Cancel current action / close modal
    Cancel,
    
    /// Show help overlay
    ShowHelp,
    
    /// Hide help overlay
    HideHelp,
    
    /// Terminal resized
    Resize,
    
    /// No operation (ignore key)
    Noop,
    
    /// Clear logs for current session
    ClearLogs,
    
    /// Scroll to top of logs
    ScrollToTop,
    
    /// Scroll to bottom of logs
    ScrollToBottom,
    
    /// Page up in logs
    PageUp,
    
    /// Page down in logs
    PageDown,
}
```

#### Handler Updates (`src/app/handler.rs`)

```rust
// Additions to handler

/// Handle session navigation by index
fn handle_select_session_by_index(state: &mut AppState, index: usize) -> UpdateResult {
    if state.session_manager.select_by_index(index) {
        UpdateResult::none()
    } else {
        // Index out of range - ignore silently
        UpdateResult::none()
    }
}

/// Handle next session
fn handle_next_session(state: &mut AppState) -> UpdateResult {
    state.session_manager.select_next();
    UpdateResult::none()
}

/// Handle previous session
fn handle_previous_session(state: &mut AppState) -> UpdateResult {
    state.session_manager.select_previous();
    UpdateResult::none()
}

/// Handle stop current app
fn handle_stop_current_app(state: &mut AppState) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        if let Some(ref app_id) = handle.session.app_id {
            let app_id = app_id.clone();
            return UpdateResult::with_action(UpdateAction::SpawnTask(Task::Stop { app_id }));
        }
    }
    UpdateResult::none()
}

/// Handle close current session
fn handle_close_current_session(state: &mut AppState) -> UpdateResult {
    if let Some(session_id) = state.session_manager.selected_id() {
        // Check if session has running app
        let has_running_app = state.session_manager
            .get(session_id)
            .map(|h| h.session.is_running())
            .unwrap_or(false);
        
        if has_running_app {
            // Need to stop app first
            let app_id = state.session_manager
                .get(session_id)
                .and_then(|h| h.session.app_id.clone());
            
            if let Some(app_id) = app_id {
                // Stop app, then close session
                return UpdateResult::with_action(UpdateAction::StopAndCloseSession {
                    session_id,
                    app_id,
                });
            }
        }
        
        // Remove session
        state.session_manager.remove_session(session_id);
        
        // If no sessions left, show device selector
        if state.session_manager.is_empty() {
            state.ui_mode = UiMode::DeviceSelector;
            state.device_selector.show_loading();
            return UpdateResult::with_action(UpdateAction::DiscoverDevices);
        }
    }
    UpdateResult::none()
}

/// Handle clear logs
fn handle_clear_logs(state: &mut AppState) -> UpdateResult {
    if let Some(session) = state.current_session_mut() {
        session.clear_logs();
    }
    UpdateResult::none()
}

/// Handle scroll to top
fn handle_scroll_to_top(state: &mut AppState) -> UpdateResult {
    if let Some(session) = state.current_session_mut() {
        session.log_view_state.offset = 0;
    }
    UpdateResult::none()
}

/// Handle scroll to bottom
fn handle_scroll_to_bottom(state: &mut AppState) -> UpdateResult {
    if let Some(session) = state.current_session_mut() {
        if !session.logs.is_empty() {
            session.log_view_state.offset = session.logs.len().saturating_sub(1);
        }
    }
    UpdateResult::none()
}

/// Handle page up
fn handle_page_up(state: &mut AppState, page_size: usize) -> UpdateResult {
    if let Some(session) = state.current_session_mut() {
        session.log_view_state.offset = session.log_view_state.offset.saturating_sub(page_size);
    }
    UpdateResult::none()
}

/// Handle page down
fn handle_page_down(state: &mut AppState, page_size: usize) -> UpdateResult {
    if let Some(session) = state.current_session_mut() {
        let max_offset = session.logs.len().saturating_sub(1);
        session.log_view_state.offset = (session.log_view_state.offset + page_size).min(max_offset);
    }
    UpdateResult::none()
}

/// Handle cancel action
fn handle_cancel(state: &mut AppState) -> UpdateResult {
    match state.ui_mode {
        UiMode::DeviceSelector => {
            // Only hide if there are sessions
            if !state.session_manager.is_empty() {
                state.device_selector.hide();
                state.ui_mode = UiMode::Normal;
            }
            UpdateResult::none()
        }
        UiMode::EmulatorSelector => {
            state.ui_mode = UiMode::DeviceSelector;
            UpdateResult::none()
        }
        UiMode::ConfirmDialog => {
            state.ui_mode = UiMode::Normal;
            UpdateResult::none()
        }
        _ => UpdateResult::none(),
    }
}

/// Handle confirm yes
fn handle_confirm_yes(state: &mut AppState) -> UpdateResult {
    match state.ui_mode {
        UiMode::ConfirmDialog => {
            // Currently only used for quit confirmation
            state.force_quit();
            UpdateResult::none()
        }
        _ => UpdateResult::none(),
    }
}

/// Handle confirm no
fn handle_confirm_no(state: &mut AppState) -> UpdateResult {
    state.ui_mode = UiMode::Normal;
    UpdateResult::none()
}

/// Main update function with new message handling
pub fn update(state: &mut AppState, message: Message) -> UpdateResult {
    match message {
        // Session navigation
        Message::SelectSessionByIndex(index) => handle_select_session_by_index(state, index),
        Message::NextSession => handle_next_session(state),
        Message::PreviousSession => handle_previous_session(state),
        
        // Session control
        Message::StopCurrentApp => handle_stop_current_app(state),
        Message::CloseCurrentSession => handle_close_current_session(state),
        
        // Log control
        Message::ClearLogs => handle_clear_logs(state),
        Message::ScrollToTop => handle_scroll_to_top(state),
        Message::ScrollToBottom => handle_scroll_to_bottom(state),
        Message::PageUp => handle_page_up(state, 10),
        Message::PageDown => handle_page_down(state, 10),
        
        // Modal/dialog handling
        Message::Cancel => handle_cancel(state),
        Message::ConfirmYes => handle_confirm_yes(state),
        Message::ConfirmNo => handle_confirm_no(state),
        
        // Noop
        Message::Noop => UpdateResult::none(),
        Message::Resize => UpdateResult::none(), // Terminal handles resize
        
        // ... existing message handling ...
        _ => handle_existing_message(state, message),
    }
}
```

---

### Acceptance Criteria

1. [ ] Number keys `1-9` switch to corresponding session
2. [ ] `Tab` cycles to next session, `Shift+Tab` to previous
3. [ ] `n` opens device selector to add new session
4. [ ] `s` stops the current app (sends stop command)
5. [ ] `x` and `Ctrl+W` close the current session
6. [ ] `c` clears logs for current session
7. [ ] `g` scrolls to top, `G` scrolls to bottom
8. [ ] `PageUp`/`PageDown` scroll by page
9. [ ] `?` shows help overlay (basic implementation)
10. [ ] `Esc` cancels current modal or action
11. [ ] Confirmation dialog accepts `y`/`n`/`Enter`/`Esc`
12. [ ] Keys are context-aware (different behavior in modals)
13. [ ] Invalid session index (e.g., `5` with only 3 sessions) is ignored gracefully
14. [ ] All new code has unit tests
15. [ ] `cargo test` passes
16. [ ] `cargo clippy` has no warnings

---

### Testing

```rust
#[cfg(test)]
mod handler_tests {
    use super::*;
    
    fn test_state() -> AppState {
        let temp = tempfile::tempdir().unwrap();
        AppState::new(temp.path().to_path_buf(), Settings::default())
    }
    
    fn test_device(id: &str, name: &str) -> Device {
        Device {
            id: id.to_string(),
            name: name.to_string(),
            platform: "ios".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            sdk: None,
            is_supported: true,
        }
    }
    
    #[test]
    fn test_session_navigation() {
        let mut state = test_state();
        
        state.session_manager.create_session(&test_device("d1", "D1")).unwrap();
        state.session_manager.create_session(&test_device("d2", "D2")).unwrap();
        state.session_manager.create_session(&test_device("d3", "D3")).unwrap();
        
        assert_eq!(state.session_manager.selected_index(), 0);
        
        handle_select_session_by_index(&mut state, 2);
        assert_eq!(state.session_manager.selected_index(), 2);
        
        handle_next_session(&mut state);
        assert_eq!(state.session_manager.selected_index(), 0); // Wraps
        
        handle_previous_session(&mut state);
        assert_eq!(state.session_manager.selected_index(), 2); // Wraps back
    }
    
    #[test]
    fn test_invalid_session_index() {
        let mut state = test_state();
        state.session_manager.create_session(&test_device("d1", "D1")).unwrap();
        
        // Should not panic or change selection
        handle_select_session_by_index(&mut state, 5);
        assert_eq!(state.session_manager.selected_index(), 0);
    }
    
    #[test]
    fn test_clear_logs() {
        let mut state = test_state();
        let id = state.session_manager.create_session(&test_device("d1", "D1")).unwrap();
        
        state.session_manager.get_mut(id).unwrap().session.log_info(LogSource::App, "Test");
        assert_eq!(state.session_manager.get(id).unwrap().session.logs.len(), 1);
        
        state.session_manager.select_by_id(id);
        handle_clear_logs(&mut state);
        
        assert_eq!(state.session_manager.get(id).unwrap().session.logs.len(), 0);
    }
    
    #[test]
    fn test_cancel_in_device_selector_with_sessions() {
        let mut state = test_state();
        state.session_manager.create_session(&test_device("d1", "D1")).unwrap();
        state.ui_mode = UiMode::DeviceSelector;
        state.device_selector.visible = true;
        
        handle_cancel(&mut state);
        
        assert_eq!(state.ui_mode, UiMode::Normal);
        assert!(!state.device_selector.visible);
    }
    
    #[test]
    fn test_cancel_in_device_selector_without_sessions() {
        let mut state = test_state();
        state.ui_mode = UiMode::DeviceSelector;
        state.device_selector.visible = true;
        
        handle_cancel(&mut state);
        
        // Should NOT hide - no sessions to fall back to
        assert_eq!(state.ui_mode, UiMode::DeviceSelector);
    }
    
    #[test]
    fn test_close_last_session_shows_device_selector() {
        let mut state = test_state();
        let id = state.session_manager.create_session(&test_device("d1", "D1")).unwrap();
        state.ui_mode = UiMode::Normal;
        
        handle_close_current_session(&mut state);
        
        assert!(state.session_manager.is_empty());
        assert_eq!(state.ui_mode, UiMode::DeviceSelector);
    }
}
```

---

### Notes

- Vim-style navigation (`j`/`k`/`g`/`G`) provides familiarity for many developers
- `Ctrl+W` mirrors browser/editor tab closing behavior
- The `?` help overlay can be a simple implementation initially; enhance in future
- Context-aware key handling prevents accidental actions in modals
- Number key shortcuts `1-9` match the maximum session limit
- Consider adding visual feedback (brief highlight) when switching tabs
- Future: Add `/` for log search/filter

---

### Files to Create/Modify

| File | Action |
|------|--------|
| `src/tui/event.rs` | Major update with context-aware key handling |
| `src/app/message.rs` | Add new message variants for navigation and control |
| `src/app/handler.rs` | Add handlers for all new messages |