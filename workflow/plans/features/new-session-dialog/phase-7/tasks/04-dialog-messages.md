# Task: Dialog Messages

## Summary

Add message types and handlers for the main NewSessionDialog, including pane switching, dialog open/close, and key routing.

## Files

| File | Action |
|------|--------|
| `src/app/message.rs` | Modify (add messages) |
| `src/app/handler/new_session/navigation.rs` | Modify (add pane switching handlers) |
| `src/app/handler/new_session/mod.rs` | Modify (add dialog open/close handlers) |
| `src/app/handler/keys.rs` | Modify (add key routing) |

## Implementation

### 1. Add dialog-level messages

```rust
// src/app/message.rs

#[derive(Debug, Clone)]
pub enum Message {
    // ... existing variants ...

    // ─────────────────────────────────────────────────────────
    // NewSessionDialog Top-Level Messages
    // ─────────────────────────────────────────────────────────

    /// Open the new session dialog
    OpenNewSessionDialog,

    /// Close the new session dialog
    CloseNewSessionDialog,

    /// Switch focus between Target Selector and Launch Context
    NewSessionDialogSwitchPane,

    /// Cancel current modal or close dialog
    NewSessionDialogEscape,
}
```

### 2. Handle dialog open/close

```rust
// src/app/handler/new_session/mod.rs

fn handle_open_new_session_dialog(state: &mut AppState) -> Option<UpdateAction> {
    // Create dialog state with loaded configs
    let configs = state.loaded_configs.clone();
    let dialog = NewSessionDialogState::new(configs);

    state.new_session_dialog = Some(dialog);
    state.ui_mode = UiMode::NewSessionDialog;

    // Trigger device discovery
    Some(UpdateAction::DiscoverConnectedDevices)
}

fn handle_close_new_session_dialog(state: &mut AppState) -> Option<UpdateAction> {
    state.new_session_dialog = None;

    // Return to appropriate UI mode
    if state.session_manager.has_sessions() {
        state.ui_mode = UiMode::Normal;
    } else {
        // No sessions, stay in startup mode or show dialog again
        state.ui_mode = UiMode::Startup;
    }

    None
}
```

### 3. Handle pane switching

```rust
// src/app/handler/new_session/navigation.rs

fn handle_new_session_dialog_switch_pane(state: &mut AppState) -> Option<UpdateAction> {
    if let Some(ref mut dialog) = state.new_session_dialog {
        dialog.toggle_pane_focus();
    }
    None
}
```

### 4. Handle Escape (context-aware)

```rust
fn handle_new_session_dialog_escape(state: &mut AppState) -> Option<UpdateAction> {
    if let Some(ref mut dialog) = state.new_session_dialog {
        // Priority 1: Close fuzzy modal
        if dialog.is_fuzzy_modal_open() {
            dialog.fuzzy_modal = None;
            return None;
        }

        // Priority 2: Close dart defines modal (with save)
        if dialog.is_dart_defines_modal_open() {
            dialog.close_dart_defines_modal_with_changes();
            return None;
        }

        // Priority 3: Close dialog (only if sessions exist)
        if state.session_manager.has_sessions() {
            return Some(UpdateAction::CloseNewSessionDialog);
        }

        // No sessions: don't close, nowhere to go
    }
    None
}
```

### 5. Key routing

```rust
// src/app/handler/keys.rs

/// Route keys to the appropriate handler based on dialog state
pub fn handle_new_session_dialog_key(
    key: KeyEvent,
    state: &AppState,
) -> Option<Message> {
    let dialog = state.new_session_dialog.as_ref()?;

    // Check modal state first
    if dialog.is_fuzzy_modal_open() {
        return handle_fuzzy_modal_key(key, dialog);
    }

    if dialog.is_dart_defines_modal_open() {
        return handle_dart_defines_modal_key(key, dialog);
    }

    // No modal open - handle main dialog keys
    match key.code {
        // Pane switching
        KeyCode::Tab => Some(Message::NewSessionDialogSwitchPane),

        // Tab shortcuts (always work)
        KeyCode::Char('1') => Some(Message::NewSessionDialogSwitchTab(TargetTab::Connected)),
        KeyCode::Char('2') => Some(Message::NewSessionDialogSwitchTab(TargetTab::Bootable)),

        // Escape
        KeyCode::Esc => Some(Message::NewSessionDialogEscape),

        // Route to focused pane
        _ => match dialog.focused_pane {
            DialogPane::TargetSelector => handle_target_selector_key(key, dialog),
            DialogPane::LaunchContext => handle_launch_context_key(key, dialog),
        },
    }
}

fn handle_target_selector_key(key: KeyEvent, dialog: &NewSessionDialogState) -> Option<Message> {
    match key.code {
        KeyCode::Up => Some(Message::NewSessionDialogDeviceUp),
        KeyCode::Down => Some(Message::NewSessionDialogDeviceDown),
        KeyCode::Enter => Some(Message::NewSessionDialogDeviceSelect),
        KeyCode::Char('r') => Some(Message::NewSessionDialogRefreshDevices),
        _ => None,
    }
}

fn handle_launch_context_key(key: KeyEvent, dialog: &NewSessionDialogState) -> Option<Message> {
    match key.code {
        KeyCode::Up => Some(Message::NewSessionDialogFieldPrev),
        KeyCode::Down => Some(Message::NewSessionDialogFieldNext),
        KeyCode::Enter => Some(Message::NewSessionDialogFieldActivate),

        // Mode field: left/right changes mode
        KeyCode::Left if dialog.launch_context.focused_field == LaunchContextField::Mode => {
            Some(Message::NewSessionDialogModePrev)
        }
        KeyCode::Right if dialog.launch_context.focused_field == LaunchContextField::Mode => {
            Some(Message::NewSessionDialogModeNext)
        }

        _ => None,
    }
}

fn handle_fuzzy_modal_key(key: KeyEvent, dialog: &NewSessionDialogState) -> Option<Message> {
    match key.code {
        KeyCode::Up => Some(Message::NewSessionDialogFuzzyUp),
        KeyCode::Down => Some(Message::NewSessionDialogFuzzyDown),
        KeyCode::Enter => Some(Message::NewSessionDialogFuzzyConfirm),
        KeyCode::Esc => Some(Message::NewSessionDialogFuzzyCancel),
        KeyCode::Backspace => Some(Message::NewSessionDialogFuzzyBackspace),
        KeyCode::Char(c) => Some(Message::NewSessionDialogFuzzyInput { c }),
        _ => None,
    }
}

fn handle_dart_defines_modal_key(key: KeyEvent, dialog: &NewSessionDialogState) -> Option<Message> {
    match key.code {
        KeyCode::Tab => Some(Message::NewSessionDialogDartDefinesSwitchPane),
        KeyCode::Up => Some(Message::NewSessionDialogDartDefinesUp),
        KeyCode::Down => Some(Message::NewSessionDialogDartDefinesDown),
        KeyCode::Enter => {
            // Context-dependent: select, save, or delete
            Some(Message::NewSessionDialogDartDefinesActivate)
        }
        KeyCode::Esc => Some(Message::NewSessionDialogCloseDartDefinesModal),
        KeyCode::Backspace => Some(Message::NewSessionDialogDartDefinesBackspace),
        KeyCode::Char(c) => Some(Message::NewSessionDialogDartDefinesInput { c }),
        _ => None,
    }
}
```

### 6. UiMode integration

```rust
// src/app/state.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiMode {
    Startup,
    Normal,
    NewSessionDialog,  // New variant
    // ... other modes
}

// src/app/handler/keys.rs

pub fn handle_key_event(key: KeyEvent, state: &AppState) -> Option<Message> {
    match state.ui_mode {
        UiMode::NewSessionDialog => handle_new_session_dialog_key(key, state),
        UiMode::Normal => handle_normal_mode_key(key, state),
        // ... other modes
    }
}
```

### 7. Trigger dialog from 'd' key

```rust
// In normal mode key handler

fn handle_normal_mode_key(key: KeyEvent, state: &AppState) -> Option<Message> {
    match key.code {
        // 'd' opens new session dialog (add device)
        KeyCode::Char('d') => Some(Message::OpenNewSessionDialog),
        // ... other keys
    }
}
```

## Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_new_session_dialog() {
        let mut state = create_test_state();

        let action = handle_open_new_session_dialog(&mut state);

        assert!(state.new_session_dialog.is_some());
        assert_eq!(state.ui_mode, UiMode::NewSessionDialog);
        assert!(matches!(action, Some(UpdateAction::DiscoverConnectedDevices)));
    }

    #[test]
    fn test_close_with_sessions() {
        let mut state = create_test_state_with_sessions();
        state.new_session_dialog = Some(NewSessionDialogState::new(LoadedConfigs::default()));

        handle_close_new_session_dialog(&mut state);

        assert!(state.new_session_dialog.is_none());
        assert_eq!(state.ui_mode, UiMode::Normal);
    }

    #[test]
    fn test_escape_closes_modal_first() {
        let mut state = create_test_state();
        state.new_session_dialog = Some(NewSessionDialogState::new(LoadedConfigs::default()));
        state.new_session_dialog.as_mut().unwrap().open_config_modal();

        handle_new_session_dialog_escape(&mut state);

        // Modal should be closed, dialog still open
        assert!(state.new_session_dialog.is_some());
        assert!(!state.new_session_dialog.as_ref().unwrap().has_modal_open());
    }

    #[test]
    fn test_escape_without_sessions_does_nothing() {
        let mut state = create_test_state();
        state.new_session_dialog = Some(NewSessionDialogState::new(LoadedConfigs::default()));

        let action = handle_new_session_dialog_escape(&mut state);

        // Dialog should stay open (nowhere to go)
        assert!(state.new_session_dialog.is_some());
        assert!(action.is_none());
    }

    #[test]
    fn test_key_routing_to_focused_pane() {
        let dialog = NewSessionDialogState::new(LoadedConfigs::default());

        // Target Selector focused
        let msg = handle_new_session_dialog_key(
            KeyEvent::from(KeyCode::Up),
            &create_state_with_dialog(dialog.clone()),
        );
        assert!(matches!(msg, Some(Message::NewSessionDialogDeviceUp)));

        // Launch Context focused
        let mut dialog = dialog;
        dialog.focused_pane = DialogPane::LaunchContext;
        let msg = handle_new_session_dialog_key(
            KeyEvent::from(KeyCode::Up),
            &create_state_with_dialog(dialog),
        );
        assert!(matches!(msg, Some(Message::NewSessionDialogFieldPrev)));
    }
}
```

## Verification

```bash
cargo fmt && cargo check && cargo test dialog_messages && cargo clippy -- -D warnings
```

## Notes

- Key routing respects modal state (modals get priority)
- Escape has tiered behavior: modal → dialog → nothing
- Tab shortcuts (1/2) always work regardless of focus
- Dialog can only be closed if sessions exist
