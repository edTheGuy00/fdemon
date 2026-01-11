## Task: Wire Up Dart Defines Modal Messages and Handlers

**Objective**: Add message types and handlers for dart defines modal interactions.

**Depends on**: Task 04 (Modal Widget)

**Estimated Time**: 15 minutes

### Background

The dart defines modal needs messages for opening, closing, navigation, pane switching, text input, and button actions. Handlers update the `DartDefinesModalState` within `NewSessionDialogState`.

### Scope

- `src/app/message.rs`: Add dart defines modal messages
- `src/app/handler/update.rs`: Add handlers

### Changes Required

**Add to `message.rs`:**

```rust
// ─────────────────────────────────────────────────────────────────
// NewSessionDialog - Dart Defines Modal Messages
// ─────────────────────────────────────────────────────────────────

/// Open dart defines modal
NewSessionDialogOpenDartDefinesModal,

/// Close dart defines modal (saves changes)
NewSessionDialogCloseDartDefinesModal,

/// Switch between list and edit panes
NewSessionDialogDartDefinesSwitchPane,

/// Navigate up in list
NewSessionDialogDartDefinesUp,

/// Navigate down in list
NewSessionDialogDartDefinesDown,

/// Confirm selection (edit item) or activate button
NewSessionDialogDartDefinesConfirm,

/// Move to next field in edit form
NewSessionDialogDartDefinesNextField,

/// Input character in active text field
NewSessionDialogDartDefinesInput { c: char },

/// Backspace in active text field
NewSessionDialogDartDefinesBackspace,

/// Save current edit
NewSessionDialogDartDefinesSave,

/// Delete current item
NewSessionDialogDartDefinesDelete,
```

**Add handlers in `update.rs`:**

```rust
use crate::tui::widgets::new_session_dialog::state::{
    DartDefine, DartDefinesModalState, DartDefinesPane, DartDefinesEditField,
};

// ─────────────────────────────────────────────────────────────────
// NewSessionDialog - Dart Defines Modal Handlers
// ─────────────────────────────────────────────────────────────────

Message::NewSessionDialogOpenDartDefinesModal => {
    // Copy current dart defines into modal state
    let defines = state.new_session_dialog_state.dart_defines.clone();
    state.new_session_dialog_state.dart_defines_modal = Some(DartDefinesModalState::new(defines));
    UpdateResult::none()
}

Message::NewSessionDialogCloseDartDefinesModal => {
    // Save changes back to main state
    if let Some(modal) = state.new_session_dialog_state.dart_defines_modal.take() {
        state.new_session_dialog_state.dart_defines = modal.defines;
    }
    UpdateResult::none()
}

Message::NewSessionDialogDartDefinesSwitchPane => {
    if let Some(ref mut modal) = state.new_session_dialog_state.dart_defines_modal {
        modal.switch_pane();
    }
    UpdateResult::none()
}

Message::NewSessionDialogDartDefinesUp => {
    if let Some(ref mut modal) = state.new_session_dialog_state.dart_defines_modal {
        if modal.active_pane == DartDefinesPane::List {
            modal.navigate_up();
        }
    }
    UpdateResult::none()
}

Message::NewSessionDialogDartDefinesDown => {
    if let Some(ref mut modal) = state.new_session_dialog_state.dart_defines_modal {
        if modal.active_pane == DartDefinesPane::List {
            modal.navigate_down();
        }
    }
    UpdateResult::none()
}

Message::NewSessionDialogDartDefinesConfirm => {
    if let Some(ref mut modal) = state.new_session_dialog_state.dart_defines_modal {
        match modal.active_pane {
            DartDefinesPane::List => {
                // Load selected item into edit form
                modal.load_selected_into_edit();
            }
            DartDefinesPane::Edit => {
                // Activate current button or confirm field
                match modal.edit_field {
                    DartDefinesEditField::Key | DartDefinesEditField::Value => {
                        // Move to next field
                        modal.next_field();
                    }
                    DartDefinesEditField::Save => {
                        modal.save_edit();
                    }
                    DartDefinesEditField::Delete => {
                        modal.delete_selected();
                    }
                }
            }
        }
    }
    UpdateResult::none()
}

Message::NewSessionDialogDartDefinesNextField => {
    if let Some(ref mut modal) = state.new_session_dialog_state.dart_defines_modal {
        if modal.active_pane == DartDefinesPane::Edit {
            modal.next_field();
        }
    }
    UpdateResult::none()
}

Message::NewSessionDialogDartDefinesInput { c } => {
    if let Some(ref mut modal) = state.new_session_dialog_state.dart_defines_modal {
        if modal.active_pane == DartDefinesPane::Edit {
            modal.input_char(c);
        }
    }
    UpdateResult::none()
}

Message::NewSessionDialogDartDefinesBackspace => {
    if let Some(ref mut modal) = state.new_session_dialog_state.dart_defines_modal {
        if modal.active_pane == DartDefinesPane::Edit {
            modal.backspace();
        }
    }
    UpdateResult::none()
}

Message::NewSessionDialogDartDefinesSave => {
    if let Some(ref mut modal) = state.new_session_dialog_state.dart_defines_modal {
        modal.save_edit();
    }
    UpdateResult::none()
}

Message::NewSessionDialogDartDefinesDelete => {
    if let Some(ref mut modal) = state.new_session_dialog_state.dart_defines_modal {
        modal.delete_selected();
    }
    UpdateResult::none()
}
```

**Update state.rs helper methods:**

```rust
impl NewSessionDialogState {
    /// Open dart defines modal with current defines
    pub fn open_dart_defines_modal(&mut self) {
        let defines = self.dart_defines.clone();
        self.dart_defines_modal = Some(DartDefinesModalState::new(defines));
    }

    /// Close dart defines modal and apply changes
    pub fn close_dart_defines_modal(&mut self) {
        if let Some(modal) = self.dart_defines_modal.take() {
            self.dart_defines = modal.defines;
        }
    }

    /// Check if dart defines modal is open
    pub fn is_dart_defines_modal_open(&self) -> bool {
        self.dart_defines_modal.is_some()
    }
}
```

### Acceptance Criteria

1. All dart defines modal message variants added to `Message` enum
2. Handler for opening modal copies defines into modal state
3. Handler for closing modal saves changes back
4. Navigation handlers respect active pane
5. Confirm handler behavior varies by pane and field
6. Text input only works in edit pane text fields
7. Save/Delete handlers trigger state methods
8. Helper methods on `NewSessionDialogState` for modal lifecycle
9. `cargo check` passes
10. `cargo clippy -- -D warnings` passes

### Testing

Handler tests:

```rust
#[cfg(test)]
mod dart_defines_handler_tests {
    use super::*;

    #[test]
    fn test_open_dart_defines_modal() {
        let mut state = AppState::new();
        state.new_session_dialog_state.dart_defines = vec![
            DartDefine::new("KEY1", "value1"),
        ];

        let _ = update(&mut state, Message::NewSessionDialogOpenDartDefinesModal);

        assert!(state.new_session_dialog_state.dart_defines_modal.is_some());
        let modal = state.new_session_dialog_state.dart_defines_modal.as_ref().unwrap();
        assert_eq!(modal.defines.len(), 1);
        assert_eq!(modal.defines[0].key, "KEY1");
    }

    #[test]
    fn test_close_dart_defines_modal_saves_changes() {
        let mut state = AppState::new();
        state.new_session_dialog_state.dart_defines = vec![];

        // Open modal
        let _ = update(&mut state, Message::NewSessionDialogOpenDartDefinesModal);

        // Add a define via modal
        if let Some(ref mut modal) = state.new_session_dialog_state.dart_defines_modal {
            modal.editing_key = "NEW_KEY".into();
            modal.editing_value = "new_value".into();
            modal.is_new = true;
            modal.save_edit();
        }

        // Close modal
        let _ = update(&mut state, Message::NewSessionDialogCloseDartDefinesModal);

        // Changes should be saved to main state
        assert!(state.new_session_dialog_state.dart_defines_modal.is_none());
        assert_eq!(state.new_session_dialog_state.dart_defines.len(), 1);
        assert_eq!(state.new_session_dialog_state.dart_defines[0].key, "NEW_KEY");
    }

    #[test]
    fn test_navigation_only_in_list_pane() {
        let mut state = AppState::new();
        state.new_session_dialog_state.dart_defines = vec![
            DartDefine::new("A", "1"),
            DartDefine::new("B", "2"),
        ];

        let _ = update(&mut state, Message::NewSessionDialogOpenDartDefinesModal);

        // Navigate in list pane
        let _ = update(&mut state, Message::NewSessionDialogDartDefinesDown);
        {
            let modal = state.new_session_dialog_state.dart_defines_modal.as_ref().unwrap();
            assert_eq!(modal.selected_index, 1);
        }

        // Switch to edit pane
        let _ = update(&mut state, Message::NewSessionDialogDartDefinesSwitchPane);

        // Navigation should not change selection
        let _ = update(&mut state, Message::NewSessionDialogDartDefinesDown);
        {
            let modal = state.new_session_dialog_state.dart_defines_modal.as_ref().unwrap();
            assert_eq!(modal.selected_index, 1);  // Unchanged
        }
    }

    #[test]
    fn test_confirm_in_list_loads_edit() {
        let mut state = AppState::new();
        state.new_session_dialog_state.dart_defines = vec![
            DartDefine::new("KEY", "value"),
        ];

        let _ = update(&mut state, Message::NewSessionDialogOpenDartDefinesModal);
        let _ = update(&mut state, Message::NewSessionDialogDartDefinesConfirm);

        let modal = state.new_session_dialog_state.dart_defines_modal.as_ref().unwrap();
        assert_eq!(modal.active_pane, DartDefinesPane::Edit);
        assert_eq!(modal.editing_key, "KEY");
        assert_eq!(modal.editing_value, "value");
    }

    #[test]
    fn test_input_only_in_edit_text_fields() {
        let mut state = AppState::new();

        let _ = update(&mut state, Message::NewSessionDialogOpenDartDefinesModal);

        // Input in list pane should be ignored
        let _ = update(&mut state, Message::NewSessionDialogDartDefinesInput { c: 'x' });
        {
            let modal = state.new_session_dialog_state.dart_defines_modal.as_ref().unwrap();
            assert_eq!(modal.editing_key, "");
        }

        // Switch to edit and input
        let _ = update(&mut state, Message::NewSessionDialogDartDefinesSwitchPane);
        let _ = update(&mut state, Message::NewSessionDialogDartDefinesInput { c: 'x' });
        {
            let modal = state.new_session_dialog_state.dart_defines_modal.as_ref().unwrap();
            assert_eq!(modal.editing_key, "x");
        }
    }
}
```

### Notes

- Opening modal creates a working copy of defines
- Closing modal commits the working copy back
- Navigation (Up/Down) only works in List pane
- Text input only works in Edit pane on Key/Value fields
- Confirm behavior is context-sensitive
- Full key binding wiring happens in Phase 7 (Integration)
