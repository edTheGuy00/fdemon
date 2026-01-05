## Task: Setting Type Editors

**Objective**: Implement inline editors for different setting value types (boolean, number, string, enum, list).

**Depends on**: 06-project-settings-tab, 07-user-preferences-tab, 08-launch-config-tab

**Estimated Time**: 2-3 hours

### Scope

- `src/tui/widgets/settings_panel.rs`: Add editing logic for each value type
- `src/app/handler/update.rs`: Handle edit messages

### Details

#### 1. Boolean Editor (Toggle)

```rust
impl SettingsPanel<'_> {
    /// Toggle boolean value
    fn toggle_bool(&self, item: &mut SettingItem) {
        if let SettingValue::Bool(ref mut val) = item.value {
            *val = !*val;
        }
    }

    fn render_bool_value(
        &self,
        x: u16,
        y: u16,
        buf: &mut Buffer,
        value: bool,
        is_selected: bool,
    ) {
        let (text, style) = if value {
            ("true", Style::default().fg(Color::Green))
        } else {
            ("false", Style::default().fg(Color::Red))
        };

        let style = if is_selected {
            style.add_modifier(Modifier::BOLD)
        } else {
            style
        };

        buf.set_string(x, y, text, style);

        // Toggle hint
        if is_selected {
            buf.set_string(
                x + text.len() as u16 + 1,
                y,
                "[Space/Enter to toggle]",
                Style::default().fg(Color::DarkGray),
            );
        }
    }
}
```

#### 2. Number Editor (Increment/Decrement + Direct Input)

```rust
impl SettingsPanel<'_> {
    /// Increment number value
    fn increment_number(&self, item: &mut SettingItem, delta: i64) {
        if let SettingValue::Number(ref mut val) = item.value {
            *val = val.saturating_add(delta);
        }
    }

    fn render_number_editor(
        &self,
        x: u16,
        y: u16,
        width: u16,
        buf: &mut Buffer,
        value: i64,
        is_editing: bool,
        edit_buffer: &str,
    ) {
        if is_editing {
            // Show input field with cursor
            let display = format!("{}▌", edit_buffer);
            buf.set_string(
                x,
                y,
                &display,
                Style::default().fg(Color::Yellow).bg(Color::DarkGray),
            );
        } else {
            buf.set_string(
                x,
                y,
                &value.to_string(),
                Style::default().fg(Color::Cyan),
            );
        }
    }

    /// Handle number key input
    fn handle_number_input(&self, state: &mut SettingsViewState, ch: char) {
        if ch.is_ascii_digit() || (ch == '-' && state.edit_buffer.is_empty()) {
            state.edit_buffer.push(ch);
        }
    }

    /// Commit number edit
    fn commit_number_edit(&self, state: &mut SettingsViewState, item: &mut SettingItem) {
        if let Ok(num) = state.edit_buffer.parse::<i64>() {
            item.value = SettingValue::Number(num);
            state.mark_dirty();
        }
        state.stop_editing();
    }
}
```

#### 3. String Editor (Inline Text Input)

```rust
impl SettingsPanel<'_> {
    fn render_string_editor(
        &self,
        x: u16,
        y: u16,
        width: u16,
        buf: &mut Buffer,
        value: &str,
        is_editing: bool,
        edit_buffer: &str,
    ) {
        let max_display = (width as usize).saturating_sub(2);

        if is_editing {
            // Editing mode with cursor
            let display = if edit_buffer.len() > max_display {
                format!("…{}▌", &edit_buffer[edit_buffer.len() - max_display + 2..])
            } else {
                format!("{}▌", edit_buffer)
            };

            buf.set_string(
                x,
                y,
                &display,
                Style::default().fg(Color::Yellow).bg(Color::DarkGray),
            );
        } else {
            // Display mode
            let display = if value.is_empty() {
                "(empty)".to_string()
            } else if value.len() > max_display {
                format!("{}…", &value[..max_display - 1])
            } else {
                value.to_string()
            };

            let style = if value.is_empty() {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default().fg(Color::White)
            };

            buf.set_string(x, y, &display, style);
        }
    }

    /// Handle string key input
    fn handle_string_input(&self, state: &mut SettingsViewState, key: KeyEvent) {
        match key.code {
            KeyCode::Char(ch) => {
                state.edit_buffer.push(ch);
            }
            KeyCode::Backspace => {
                state.edit_buffer.pop();
            }
            KeyCode::Delete => {
                state.edit_buffer.clear();
            }
            _ => {}
        }
    }

    /// Commit string edit
    fn commit_string_edit(&self, state: &mut SettingsViewState, item: &mut SettingItem) {
        item.value = SettingValue::String(state.edit_buffer.clone());
        state.mark_dirty();
        state.stop_editing();
    }
}
```

#### 4. Enum Editor (Cycle Through Options)

```rust
impl SettingsPanel<'_> {
    /// Cycle enum value forward
    fn cycle_enum_next(&self, item: &mut SettingItem) {
        if let SettingValue::Enum { ref mut value, ref options } = item.value {
            let current_idx = options.iter().position(|o| o == value).unwrap_or(0);
            let next_idx = (current_idx + 1) % options.len();
            *value = options[next_idx].clone();
        }
    }

    /// Cycle enum value backward
    fn cycle_enum_prev(&self, item: &mut SettingItem) {
        if let SettingValue::Enum { ref mut value, ref options } = item.value {
            let current_idx = options.iter().position(|o| o == value).unwrap_or(0);
            let next_idx = if current_idx == 0 {
                options.len() - 1
            } else {
                current_idx - 1
            };
            *value = options[next_idx].clone();
        }
    }

    fn render_enum_value(
        &self,
        x: u16,
        y: u16,
        buf: &mut Buffer,
        value: &str,
        options: &[String],
        is_selected: bool,
    ) {
        buf.set_string(
            x,
            y,
            value,
            Style::default().fg(Color::Magenta).add_modifier(
                if is_selected { Modifier::BOLD } else { Modifier::empty() }
            ),
        );

        // Cycle hint
        if is_selected {
            let hint = format!(" [←/→ to cycle: {}]", options.join("/"));
            let truncated_hint = truncate_str(&hint, 30);
            buf.set_string(
                x + value.len() as u16,
                y,
                &truncated_hint,
                Style::default().fg(Color::DarkGray),
            );
        }
    }
}
```

#### 5. List Editor (Add/Remove Items)

```rust
impl SettingsPanel<'_> {
    fn render_list_value(
        &self,
        x: u16,
        y: u16,
        width: u16,
        buf: &mut Buffer,
        items: &[String],
        is_selected: bool,
        is_editing: bool,
        edit_buffer: &str,
    ) {
        if is_editing {
            // Show add item input
            let prompt = "Add: ";
            buf.set_string(x, y, prompt, Style::default().fg(Color::DarkGray));
            buf.set_string(
                x + prompt.len() as u16,
                y,
                &format!("{}▌", edit_buffer),
                Style::default().fg(Color::Yellow).bg(Color::DarkGray),
            );
        } else {
            // Show list items
            let display = if items.is_empty() {
                "(empty)".to_string()
            } else {
                items.join(", ")
            };

            let style = if items.is_empty() {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default().fg(Color::Blue)
            };

            let truncated = truncate_str(&display, width as usize);
            buf.set_string(x, y, &truncated, style);

            // Edit hint
            if is_selected {
                let hint = " [Enter to add, d to remove last]";
                let hint_x = x + truncated.len() as u16;
                if hint_x + 10 < x + width {
                    buf.set_string(hint_x, y, hint, Style::default().fg(Color::DarkGray));
                }
            }
        }
    }

    /// Add item to list
    fn add_list_item(&self, item: &mut SettingItem, new_item: String) {
        if let SettingValue::List(ref mut items) = item.value {
            if !new_item.is_empty() && !items.contains(&new_item) {
                items.push(new_item);
            }
        }
    }

    /// Remove last item from list
    fn remove_last_list_item(&self, item: &mut SettingItem) {
        if let SettingValue::List(ref mut items) = item.value {
            items.pop();
        }
    }
}
```

#### 6. Key Handler Updates

```rust
// In app/handler/keys.rs - handle_key_settings_edit
fn handle_key_settings_edit(state: &AppState, key: KeyEvent) -> Option<Message> {
    let item = get_current_item(&state); // Helper to get current item

    match &item.value {
        SettingValue::Bool(_) => {
            // Bool doesn't use edit mode - toggle directly
            match key.code {
                KeyCode::Enter | KeyCode::Char(' ') => Some(Message::SettingsToggleBool),
                _ => None,
            }
        }

        SettingValue::Number(_) => {
            match key.code {
                KeyCode::Esc => Some(Message::SettingsCancelEdit),
                KeyCode::Enter => Some(Message::SettingsCommitEdit),
                KeyCode::Char('+') | KeyCode::Char('=') => Some(Message::SettingsIncrement(1)),
                KeyCode::Char('-') => Some(Message::SettingsIncrement(-1)),
                KeyCode::Char(c) if c.is_ascii_digit() => Some(Message::SettingsCharInput(c)),
                KeyCode::Backspace => Some(Message::SettingsBackspace),
                _ => None,
            }
        }

        SettingValue::String(_) => {
            match key.code {
                KeyCode::Esc => Some(Message::SettingsCancelEdit),
                KeyCode::Enter => Some(Message::SettingsCommitEdit),
                KeyCode::Char(c) => Some(Message::SettingsCharInput(c)),
                KeyCode::Backspace => Some(Message::SettingsBackspace),
                KeyCode::Delete => Some(Message::SettingsClearBuffer),
                _ => None,
            }
        }

        SettingValue::Enum { .. } => {
            // Enum doesn't use edit mode - cycle directly
            match key.code {
                KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Right => {
                    Some(Message::SettingsCycleEnumNext)
                }
                KeyCode::Left => Some(Message::SettingsCycleEnumPrev),
                _ => None,
            }
        }

        SettingValue::List(_) => {
            match key.code {
                KeyCode::Esc => Some(Message::SettingsCancelEdit),
                KeyCode::Enter => Some(Message::SettingsCommitEdit), // Add item
                KeyCode::Char('d') if !state.settings_view_state.editing => {
                    Some(Message::SettingsRemoveListItem)
                }
                KeyCode::Char(c) if state.settings_view_state.editing => {
                    Some(Message::SettingsCharInput(c))
                }
                KeyCode::Backspace if state.settings_view_state.editing => {
                    Some(Message::SettingsBackspace)
                }
                _ => None,
            }
        }

        _ => None,
    }
}
```

#### 7. Additional Messages

```rust
pub enum Message {
    // ... existing ...

    // Settings editing
    SettingsToggleBool,
    SettingsCycleEnumNext,
    SettingsCycleEnumPrev,
    SettingsIncrement(i64),
    SettingsCharInput(char),
    SettingsBackspace,
    SettingsClearBuffer,
    SettingsCommitEdit,
    SettingsCancelEdit,
    SettingsRemoveListItem,
}
```

### Acceptance Criteria

1. **Boolean**: Toggle with Enter/Space, visual feedback (green/red)
2. **Number**: Increment/decrement with +/-, direct input in edit mode
3. **String**: Full text editing with cursor, backspace, delete
4. **Enum**: Cycle with Enter/Space/←/→, show available options
5. **List**: Add items with Enter, remove with 'd', show as comma-separated
6. Edit mode shows visual cursor indicator (▌)
7. Escape cancels edit, Enter commits
8. Changes mark state as dirty
9. Appropriate hints shown when item is selected
10. Unit tests for each editor type

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_toggle_bool() {
        let mut item = SettingItem::new("test", "Test")
            .value(SettingValue::Bool(false));

        // Simulate toggle
        if let SettingValue::Bool(ref mut val) = item.value {
            *val = !*val;
        }

        assert!(matches!(item.value, SettingValue::Bool(true)));
    }

    #[test]
    fn test_cycle_enum() {
        let mut item = SettingItem::new("test", "Test")
            .value(SettingValue::Enum {
                value: "debug".to_string(),
                options: vec!["debug".to_string(), "profile".to_string(), "release".to_string()],
            });

        // Simulate cycle next
        if let SettingValue::Enum { ref mut value, ref options } = item.value {
            let idx = options.iter().position(|o| o == value).unwrap_or(0);
            *value = options[(idx + 1) % options.len()].clone();
        }

        assert!(matches!(
            item.value,
            SettingValue::Enum { ref value, .. } if value == "profile"
        ));
    }

    #[test]
    fn test_add_list_item() {
        let mut item = SettingItem::new("test", "Test")
            .value(SettingValue::List(vec!["lib".to_string()]));

        // Simulate add
        if let SettingValue::List(ref mut items) = item.value {
            items.push("test".to_string());
        }

        assert!(matches!(
            item.value,
            SettingValue::List(ref items) if items.len() == 2
        ));
    }

    #[test]
    fn test_number_edit_buffer() {
        let mut state = SettingsViewState::new();
        state.start_editing("500");

        assert!(state.editing);
        assert_eq!(state.edit_buffer, "500");

        // Simulate backspace
        state.edit_buffer.pop();
        assert_eq!(state.edit_buffer, "50");
    }

    #[test]
    fn test_string_edit() {
        let mut state = SettingsViewState::new();
        state.start_editing("hello");

        state.edit_buffer.push_str(" world");
        assert_eq!(state.edit_buffer, "hello world");
    }
}
```

### Notes

- Boolean and Enum don't use traditional "edit mode" - they toggle/cycle immediately
- Number allows both increment/decrement and direct digit entry
- List editor is simplified - consider adding full list management modal (future)
- Cursor position tracking for string edit (future enhancement)
- Consider adding undo support (future)

---

## Completion Summary

**Status:** (Not Started)

**Files Modified:**
- (To be filled after implementation)

**Implementation Details:**

(To be filled after implementation)

**Testing Performed:**
- `cargo fmt` -
- `cargo check` -
- `cargo clippy -- -D warnings` -
- `cargo test settings` -

**Notable Decisions:**
- (To be filled after implementation)
