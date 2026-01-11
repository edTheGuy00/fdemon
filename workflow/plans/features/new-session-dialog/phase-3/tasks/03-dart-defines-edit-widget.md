## Task: Implement Dart Defines Edit Widget

**Objective**: Create the right pane widget with Key/Value inputs and Save/Delete buttons.

**Depends on**: Task 02 (List Widget)

**Estimated Time**: 25 minutes

### Background

The right pane shows an edit form with Key and Value text inputs, plus Save and Delete buttons. Tab cycles through all four elements. The focused field is highlighted.

### Scope

- `src/tui/widgets/new_session_dialog/dart_defines_modal.rs`: Add edit pane widget

### Changes Required

**Add to `dart_defines_modal.rs`:**

```rust
use super::state::{DartDefinesEditField, DartDefinesModalState, DartDefinesPane};

/// Widget for the right pane (edit form)
pub struct DartDefinesEditPane<'a> {
    state: &'a DartDefinesModalState,
}

impl<'a> DartDefinesEditPane<'a> {
    pub fn new(state: &'a DartDefinesModalState) -> Self {
        Self { state }
    }

    fn is_focused(&self) -> bool {
        self.state.active_pane == DartDefinesPane::Edit
    }

    fn field_style(&self, field: DartDefinesEditField) -> Style {
        let is_active = self.is_focused() && self.state.edit_field == field;

        if is_active {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        }
    }

    fn input_style(&self, field: DartDefinesEditField) -> Style {
        let is_active = self.is_focused() && self.state.edit_field == field;

        if is_active {
            Style::default()
                .fg(Color::White)
                .bg(Color::Rgb(60, 60, 80))
        } else {
            Style::default()
                .fg(Color::Gray)
                .bg(Color::Rgb(40, 40, 50))
        }
    }

    fn button_style(&self, field: DartDefinesEditField) -> Style {
        let is_active = self.is_focused() && self.state.edit_field == field;

        if is_active {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(Color::White)
                .bg(Color::Rgb(50, 50, 60))
        }
    }

    fn render_label(&self, area: Rect, buf: &mut Buffer, text: &str) {
        Paragraph::new(text)
            .style(Style::default().fg(styles::ITEM_FG))
            .render(area, buf);
    }

    fn render_input(&self, area: Rect, buf: &mut Buffer, value: &str, field: DartDefinesEditField) {
        let is_active = self.is_focused() && self.state.edit_field == field;
        let style = self.input_style(field);

        // Add cursor if active
        let display = if is_active {
            format!("{}|", value)
        } else {
            value.to_string()
        };

        // Pad to fill the input box
        let padded = format!("{:<width$}", display, width = area.width as usize);

        Paragraph::new(padded)
            .style(style)
            .render(area, buf);
    }

    fn render_button(&self, area: Rect, buf: &mut Buffer, label: &str, field: DartDefinesEditField) {
        let style = self.button_style(field);

        // Center the label in the button
        let padded = format!("{:^width$}", label, width = area.width as usize);

        Paragraph::new(padded)
            .style(style)
            .render(area, buf);
    }
}

impl Widget for DartDefinesEditPane<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let border_color = if self.is_focused() {
            styles::BORDER_FOCUSED
        } else {
            styles::BORDER_UNFOCUSED
        };

        let title = if self.state.is_new {
            " New Variable "
        } else {
            " Edit Variable "
        };

        let block = Block::default()
            .title(title)
            .title_style(Style::default().fg(styles::HEADER_FG).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_set(symbols::border::ROUNDED)
            .border_style(Style::default().fg(border_color))
            .style(Style::default().bg(styles::MODAL_BG));

        let inner = block.inner(area);
        block.render(area, buf);

        // Layout: vertical stack with spacing
        let chunks = Layout::vertical([
            Constraint::Length(1),  // Key label
            Constraint::Length(1),  // Key input
            Constraint::Length(1),  // Spacing
            Constraint::Length(1),  // Value label
            Constraint::Length(1),  // Value input
            Constraint::Length(2),  // Spacing
            Constraint::Length(1),  // Buttons
            Constraint::Min(0),     // Rest
        ])
        .split(inner);

        // Key field
        self.render_label(chunks[0], buf, "Key:");
        self.render_input(chunks[1], buf, &self.state.editing_key, DartDefinesEditField::Key);

        // Value field
        self.render_label(chunks[3], buf, "Value:");
        self.render_input(chunks[4], buf, &self.state.editing_value, DartDefinesEditField::Value);

        // Buttons row
        let button_chunks = Layout::horizontal([
            Constraint::Length(12),  // Save button
            Constraint::Length(2),   // Spacing
            Constraint::Length(12),  // Delete button
            Constraint::Min(0),      // Rest
        ])
        .split(chunks[6]);

        self.render_button(button_chunks[0], buf, "Save", DartDefinesEditField::Save);
        self.render_button(button_chunks[2], buf, "Delete", DartDefinesEditField::Delete);

        // Show unsaved indicator
        if self.state.has_unsaved_changes() {
            let indicator = " (unsaved) ";
            let x = inner.x + inner.width - indicator.len() as u16 - 1;
            let y = area.y;  // Top border

            buf.set_string(
                x,
                y,
                indicator,
                Style::default().fg(Color::Yellow).add_modifier(Modifier::ITALIC),
            );
        }
    }
}

#[cfg(test)]
mod edit_tests {
    use super::*;
    use crate::tui::widgets::new_session_dialog::state::DartDefine;

    fn create_test_buffer(width: u16, height: u16) -> (Buffer, Rect) {
        let rect = Rect::new(0, 0, width, height);
        let buf = Buffer::empty(rect);
        (buf, rect)
    }

    fn buffer_contains(buf: &Buffer, text: &str) -> bool {
        let content: String = buf.content.iter().map(|c| c.symbol()).collect();
        content.contains(text)
    }

    #[test]
    fn test_edit_renders_labels() {
        let state = DartDefinesModalState::new(vec![]);

        let (mut buf, rect) = create_test_buffer(50, 15);
        let widget = DartDefinesEditPane::new(&state);
        widget.render(rect, &mut buf);

        assert!(buffer_contains(&buf, "Key:"));
        assert!(buffer_contains(&buf, "Value:"));
    }

    #[test]
    fn test_edit_renders_buttons() {
        let state = DartDefinesModalState::new(vec![]);

        let (mut buf, rect) = create_test_buffer(50, 15);
        let widget = DartDefinesEditPane::new(&state);
        widget.render(rect, &mut buf);

        assert!(buffer_contains(&buf, "Save"));
        assert!(buffer_contains(&buf, "Delete"));
    }

    #[test]
    fn test_edit_shows_values() {
        let defines = vec![DartDefine::new("API_KEY", "secret123")];
        let mut state = DartDefinesModalState::new(defines);
        state.load_selected_into_edit();

        let (mut buf, rect) = create_test_buffer(50, 15);
        let widget = DartDefinesEditPane::new(&state);
        widget.render(rect, &mut buf);

        assert!(buffer_contains(&buf, "API_KEY"));
        assert!(buffer_contains(&buf, "secret123"));
    }

    #[test]
    fn test_edit_new_title() {
        let mut state = DartDefinesModalState::new(vec![]);
        state.is_new = true;

        let (mut buf, rect) = create_test_buffer(50, 15);
        let widget = DartDefinesEditPane::new(&state);
        widget.render(rect, &mut buf);

        assert!(buffer_contains(&buf, "New Variable"));
    }
}
```

### Acceptance Criteria

1. `DartDefinesEditPane` widget struct with `new(state)` constructor
2. Key label and input field rendered
3. Value label and input field rendered
4. Save and Delete buttons rendered
5. Active field/button highlighted with cyan background
6. Input fields show cursor (`|`) when active
7. Border color indicates pane focus
8. Title changes: "Edit Variable" vs "New Variable"
9. Unsaved changes indicator shown in title area
10. `cargo check` passes
11. `cargo clippy -- -D warnings` passes

### Testing

```rust
#[test]
fn test_edit_unsaved_indicator() {
    let defines = vec![DartDefine::new("KEY", "original")];
    let mut state = DartDefinesModalState::new(defines);
    state.load_selected_into_edit();
    state.editing_value = "modified".into();

    let (mut buf, rect) = create_test_buffer(50, 15);
    let widget = DartDefinesEditPane::new(&state);
    widget.render(rect, &mut buf);

    assert!(buffer_contains(&buf, "unsaved"));
}

#[test]
fn test_edit_cursor_in_active_field() {
    let mut state = DartDefinesModalState::new(vec![]);
    state.active_pane = DartDefinesPane::Edit;
    state.edit_field = DartDefinesEditField::Key;
    state.editing_key = "test".into();

    let (mut buf, rect) = create_test_buffer(50, 15);
    let widget = DartDefinesEditPane::new(&state);
    widget.render(rect, &mut buf);

    assert!(buffer_contains(&buf, "test|"));
}
```

### Notes

- Input fields should have minimum width for usability
- Button widths are fixed at 12 characters
- Cursor is shown as `|` character appended to value
- Consider max length limits for inputs
- Delete button could be styled differently (red) when focused
