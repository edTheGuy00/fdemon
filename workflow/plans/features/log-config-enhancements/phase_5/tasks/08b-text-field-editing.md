# Task: Text Field Editing (Flavor/DartDefines)

**Objective**: Enable text editing for Flavor and Dart Defines fields in the startup dialog.

**Depends on**: Task 04 (Dialog Key Handler)

## Problem

When selecting Flavor or Dart Defines sections, users cannot enter text. The issues are:

1. **No way to enter edit mode**: Enter key only checks `can_launch()` and doesn't trigger edit mode
2. **Text input handler exists but is unreachable**: `handle_key_startup_dialog_text_input` handles input, but `dialog.editing` is never set to `true`
3. **No dedicated message for entering edit mode**

## Root Cause (from `keys.rs`)

```rust
// handle_key_startup_dialog: Enter only checks can_launch()
(KeyCode::Enter, KeyModifiers::NONE) => {
    if dialog.can_launch() {
        Some(Message::StartupDialogConfirm)
    } else {
        None  // <-- Does nothing on Flavor/DartDefines
    }
}
```

## Scope

- `src/app/message.rs` - Add enter/exit edit mode messages
- `src/app/handler/keys.rs` - Update Enter key handling
- `src/app/handler/update.rs` - Add message handlers
- `src/app/state.rs` - Add edit mode methods
- `src/tui/widgets/startup_dialog/mod.rs` - Visual feedback for edit mode

## Implementation

### 1. Add Message Types (`src/app/message.rs`)

```rust
// In StartupDialog Messages section:

/// Enter edit mode for current text field (Flavor or DartDefines)
StartupDialogEnterEdit,

/// Exit edit mode without changing section
StartupDialogExitEdit,
```

### 2. Add State Methods (`src/app/state.rs`)

```rust
impl StartupDialogState {
    /// Check if current section is editable (text input)
    pub fn is_text_section(&self) -> bool {
        matches!(self.active_section, DialogSection::Flavor | DialogSection::DartDefines)
    }

    /// Enter edit mode (only for text sections)
    pub fn enter_edit(&mut self) {
        if self.is_text_section() {
            self.editing = true;
        }
    }

    /// Exit edit mode
    pub fn exit_edit(&mut self) {
        self.editing = false;
    }
}
```

### 3. Update Key Handler (`src/app/handler/keys.rs`)

```rust
// In handle_key_startup_dialog, replace Enter handling:

(KeyCode::Enter, KeyModifiers::NONE) => {
    // Context-sensitive Enter:
    // - On Flavor/DartDefines: enter edit mode (or confirm if already editing)
    // - On other sections with device selected: launch
    match dialog.active_section {
        DialogSection::Flavor | DialogSection::DartDefines => {
            if dialog.editing {
                // Already editing, exit edit mode
                Some(Message::StartupDialogExitEdit)
            } else {
                // Enter edit mode
                Some(Message::StartupDialogEnterEdit)
            }
        }
        _ => {
            // Other sections: launch if device selected
            if dialog.can_launch() {
                Some(Message::StartupDialogConfirm)
            } else {
                None
            }
        }
    }
}

// Also add Space key to toggle edit:
(KeyCode::Char(' '), KeyModifiers::NONE) => {
    if dialog.is_text_section() && !dialog.editing {
        Some(Message::StartupDialogEnterEdit)
    } else {
        None
    }
}
```

### 4. Update Text Input Handler (`src/app/handler/keys.rs`)

```rust
// In handle_key_startup_dialog_text_input, fix Esc/Enter handling:

fn handle_key_startup_dialog_text_input(key: KeyEvent) -> Option<Message> {
    match (key.code, key.modifiers) {
        // Exit edit mode (but stay in dialog)
        (KeyCode::Esc, _) => Some(Message::StartupDialogExitEdit),

        // Confirm edit and exit edit mode
        (KeyCode::Enter, _) => Some(Message::StartupDialogExitEdit),

        // ... rest unchanged
    }
}
```

### 5. Add Message Handlers (`src/app/handler/update.rs`)

```rust
Message::StartupDialogEnterEdit => {
    state.startup_dialog_state.enter_edit();
    UpdateResult::none()
}

Message::StartupDialogExitEdit => {
    state.startup_dialog_state.exit_edit();
    UpdateResult::none()
}
```

### 6. Visual Feedback (`src/tui/widgets/startup_dialog/mod.rs`)

Update `render_input_field` to show cursor when editing:

```rust
fn render_input_field(/* ... */) {
    let is_active = self.state.active_section == section;
    let is_editing = is_active && self.state.editing;

    let display_value = if is_editing {
        // Show cursor at end when editing
        format!("{}|", value)
    } else if value.is_empty() {
        "(optional)".to_string()
    } else {
        value.to_string()
    };

    // Highlight background when editing
    let value_style = if is_editing {
        Style::default().fg(VALUE_COLOR).bg(Color::DarkGray)
    } else if is_active {
        Style::default().fg(VALUE_COLOR)
    } else {
        Style::default().fg(PLACEHOLDER_COLOR)
    };

    // ... render
}
```

## Acceptance Criteria

1. Enter key on Flavor section enters edit mode
2. Enter key on DartDefines section enters edit mode
3. While editing, character keys append to field
4. Backspace removes last character
5. Enter/Esc exits edit mode (stays in dialog)
6. Tab/Shift+Tab move to next/prev section and exit edit
7. Visual cursor indicator when editing
8. Unit tests for edit mode transitions

## Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enter_on_flavor_enters_edit_mode() {
        let mut state = AppState::new();
        state.ui_mode = UiMode::StartupDialog;
        state.startup_dialog_state.active_section = DialogSection::Flavor;
        state.startup_dialog_state.editing = false;

        let msg = handle_key_startup_dialog(&state, key(KeyCode::Enter));
        assert!(matches!(msg, Some(Message::StartupDialogEnterEdit)));
    }

    #[test]
    fn test_enter_on_flavor_while_editing_exits() {
        let mut state = AppState::new();
        state.ui_mode = UiMode::StartupDialog;
        state.startup_dialog_state.active_section = DialogSection::Flavor;
        state.startup_dialog_state.editing = true;

        let msg = handle_key_startup_dialog_text_input(key(KeyCode::Enter));
        assert!(matches!(msg, Some(Message::StartupDialogExitEdit)));
    }

    #[test]
    fn test_esc_in_edit_mode_exits_edit_not_dialog() {
        let msg = handle_key_startup_dialog_text_input(key(KeyCode::Esc));
        assert!(matches!(msg, Some(Message::StartupDialogExitEdit)));
    }

    #[test]
    fn test_char_input_while_editing() {
        let msg = handle_key_startup_dialog_text_input(key(KeyCode::Char('a')));
        assert!(matches!(msg, Some(Message::StartupDialogCharInput('a'))));
    }

    #[test]
    fn test_is_text_section() {
        let mut state = StartupDialogState::new();

        state.active_section = DialogSection::Flavor;
        assert!(state.is_text_section());

        state.active_section = DialogSection::DartDefines;
        assert!(state.is_text_section());

        state.active_section = DialogSection::Configs;
        assert!(!state.is_text_section());

        state.active_section = DialogSection::Mode;
        assert!(!state.is_text_section());

        state.active_section = DialogSection::Devices;
        assert!(!state.is_text_section());
    }
}
```

## Notes

- This follows the pattern from settings panel editing
- Dart defines format: `KEY=VALUE,KEY2=VALUE2` - parsing already exists in update.rs
- Future enhancement: inline validation feedback

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/app/message.rs` | Added `StartupDialogEnterEdit` and `StartupDialogExitEdit` messages |
| `src/app/state.rs` | Added `is_text_section()`, `enter_edit()`, `exit_edit()` methods to `StartupDialogState` |
| `src/app/handler/keys.rs` | Updated Enter key handling to be context-sensitive (enter/exit edit mode on text sections); Added Space key support; Updated text input handler to exit edit mode on Esc/Enter; Added 14 unit tests |
| `src/app/handler/update.rs` | Updated message handlers for `StartupDialogEnterEdit` and `StartupDialogExitEdit` to use new state methods |
| `src/tui/widgets/startup_dialog/mod.rs` | Enhanced `render_input_field()` to show cursor indicator (`|`) and dark gray background when editing |

### Notable Decisions/Tradeoffs

1. **Context-sensitive Enter key**: Enter key now behaves differently based on the active section:
   - On Flavor/DartDefines: enters/exits edit mode
   - On other sections: launches session if device selected
   This provides intuitive behavior without needing additional keys.

2. **Space key as alternative**: Added Space key to enter edit mode on text sections, following common UI patterns where Space and Enter are interchangeable for activation.

3. **Visual feedback**: Cursor indicator (`|`) at end of text and dark gray background provide clear visual feedback that the field is in edit mode, similar to settings panel editing pattern.

4. **Exit edit stays in dialog**: Esc while editing exits edit mode but stays in the dialog, rather than closing the entire dialog. This prevents accidental cancellation and follows the principle of least surprise.

### Testing Performed

- `cargo fmt` - Passed
- `cargo check` - Passed
- `cargo test startup_dialog_edit_tests` - Passed (14 tests)
- `cargo test` - Passed (1167 tests total)
- `cargo clippy` - Passed (no warnings)

### Tests Added

14 comprehensive unit tests covering:
- Entering edit mode on Flavor and DartDefines sections
- Exiting edit mode with Enter and Esc
- Space key behavior on text and non-text sections
- Character input and backspace while editing
- Tab/Shift+Tab section navigation during edit
- `is_text_section()` validation for all section types
- `enter_edit()` and `exit_edit()` state methods

### Risks/Limitations

1. **Cursor position**: The cursor is always shown at the end of the text. This is acceptable for short text fields but could be enhanced in the future with cursor positioning within the text.

2. **No inline validation**: The implementation does not validate dart-defines format during editing. Validation occurs at launch time. Future enhancement could add real-time format validation feedback.

3. **Overflow handling**: Long text in Flavor or DartDefines fields may overflow the visible area. The current implementation does not handle horizontal scrolling within the input field.
