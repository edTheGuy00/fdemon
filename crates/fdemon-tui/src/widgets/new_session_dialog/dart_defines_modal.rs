//! Dart defines master-detail modal widget

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Widget},
};

use super::state::{DartDefinesEditField, DartDefinesModalState, DartDefinesPane};
use crate::theme::palette;

/// Widget for the left pane (list of defines)
pub struct DartDefinesListPane<'a> {
    state: &'a DartDefinesModalState,
}

impl<'a> DartDefinesListPane<'a> {
    pub fn new(state: &'a DartDefinesModalState) -> Self {
        Self { state }
    }

    fn is_focused(&self) -> bool {
        self.state.active_pane == DartDefinesPane::List
    }
}

impl Widget for DartDefinesListPane<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let border_color = if self.is_focused() {
            palette::ACCENT
        } else {
            palette::BORDER_DIM
        };

        let block = Block::default()
            .title(" Active Variables ")
            .title_style(
                Style::default()
                    .fg(palette::ACCENT)
                    .add_modifier(Modifier::BOLD),
            )
            .borders(Borders::ALL)
            .border_set(symbols::border::ROUNDED)
            .border_style(Style::default().fg(border_color))
            .style(Style::default().bg(palette::MODAL_DART_DEFINES_BG));

        let inner = block.inner(area);
        block.render(area, buf);

        // Calculate visible range
        let visible_height = inner.height as usize;
        let total_items = self.state.list_item_count();
        let start = self.state.scroll_offset;
        let end = (start + visible_height).min(total_items);

        // Build list items
        let items: Vec<ListItem> = (start..end)
            .map(|idx| {
                let is_selected = idx == self.state.selected_index;
                let is_add_new = idx >= self.state.defines.len();

                let (text, base_style) = if is_add_new {
                    (
                        "[+] Add New".to_string(),
                        Style::default().fg(palette::STATUS_GREEN),
                    )
                } else {
                    let define = &self.state.defines[idx];
                    (
                        define.key.clone(),
                        Style::default().fg(palette::TEXT_PRIMARY),
                    )
                };

                let indicator = if is_selected { "> " } else { "  " };
                let display_text = format!("{}{}", indicator, text);

                let style = if is_selected {
                    Style::default()
                        .fg(Color::Black)
                        .bg(palette::ACCENT)
                        .add_modifier(Modifier::BOLD)
                } else {
                    base_style
                };

                ListItem::new(display_text).style(style)
            })
            .collect();

        let list = List::new(items);
        list.render(inner, buf);

        // Show scroll indicator if needed
        if total_items > visible_height {
            let scroll_info = format!(" {}/{} ", self.state.selected_index + 1, total_items);
            let info_width = scroll_info.len() as u16;

            if inner.width > info_width + 2 {
                let x = inner.x + inner.width - info_width;
                let y = area.y; // Top border

                buf.set_string(x, y, &scroll_info, Style::default().fg(palette::TEXT_MUTED));
            }
        }
    }
}

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

    fn input_style(&self, field: DartDefinesEditField) -> Style {
        let is_active = self.is_focused() && self.state.edit_field == field;

        if is_active {
            Style::default()
                .fg(palette::TEXT_PRIMARY)
                .bg(palette::MODAL_DART_DEFINES_INPUT_ACTIVE_BG)
        } else {
            Style::default()
                .fg(palette::TEXT_SECONDARY)
                .bg(palette::MODAL_DART_DEFINES_INPUT_INACTIVE_BG)
        }
    }

    fn button_style(&self, field: DartDefinesEditField) -> Style {
        let is_active = self.is_focused() && self.state.edit_field == field;

        if is_active {
            Style::default()
                .fg(Color::Black)
                .bg(palette::ACCENT)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(palette::TEXT_PRIMARY)
                .bg(palette::MODAL_DART_DEFINES_BUTTON_INACTIVE_BG)
        }
    }

    fn render_label(&self, area: Rect, buf: &mut Buffer, text: &str) {
        Paragraph::new(text)
            .style(Style::default().fg(palette::TEXT_PRIMARY))
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

        Paragraph::new(padded).style(style).render(area, buf);
    }

    fn render_button(
        &self,
        area: Rect,
        buf: &mut Buffer,
        label: &str,
        field: DartDefinesEditField,
    ) {
        let style = self.button_style(field);

        // Center the label in the button
        let padded = format!("{:^width$}", label, width = area.width as usize);

        Paragraph::new(padded).style(style).render(area, buf);
    }
}

impl Widget for DartDefinesEditPane<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let border_color = if self.is_focused() {
            palette::ACCENT
        } else {
            palette::BORDER_DIM
        };

        let title = if self.state.is_new {
            " New Variable "
        } else {
            " Edit Variable "
        };

        let block = Block::default()
            .title(title)
            .title_style(
                Style::default()
                    .fg(palette::ACCENT)
                    .add_modifier(Modifier::BOLD),
            )
            .borders(Borders::ALL)
            .border_set(symbols::border::ROUNDED)
            .border_style(Style::default().fg(border_color))
            .style(Style::default().bg(palette::MODAL_DART_DEFINES_BG));

        let inner = block.inner(area);
        block.render(area, buf);

        // Layout: vertical stack with spacing
        let chunks = Layout::vertical([
            Constraint::Length(1), // Key label
            Constraint::Length(1), // Key input
            Constraint::Length(1), // Spacing
            Constraint::Length(1), // Value label
            Constraint::Length(1), // Value input
            Constraint::Length(2), // Spacing
            Constraint::Length(1), // Buttons
            Constraint::Min(0),    // Rest
        ])
        .split(inner);

        // Key field
        self.render_label(chunks[0], buf, "Key:");
        self.render_input(
            chunks[1],
            buf,
            &self.state.editing_key,
            DartDefinesEditField::Key,
        );

        // Value field
        self.render_label(chunks[3], buf, "Value:");
        self.render_input(
            chunks[4],
            buf,
            &self.state.editing_value,
            DartDefinesEditField::Value,
        );

        // Buttons row
        let button_chunks = Layout::horizontal([
            Constraint::Length(12), // Save button
            Constraint::Length(2),  // Spacing
            Constraint::Length(12), // Delete button
            Constraint::Min(0),     // Rest
        ])
        .split(chunks[6]);

        self.render_button(button_chunks[0], buf, "Save", DartDefinesEditField::Save);
        self.render_button(
            button_chunks[2],
            buf,
            "Delete",
            DartDefinesEditField::Delete,
        );

        // Show unsaved indicator
        if self.state.has_unsaved_changes() {
            let indicator = " (unsaved) ";
            let x = inner.x + inner.width - indicator.len() as u16 - 1;
            let y = area.y; // Top border

            buf.set_string(
                x,
                y,
                indicator,
                Style::default()
                    .fg(palette::STATUS_YELLOW)
                    .add_modifier(Modifier::ITALIC),
            );
        }
    }
}

#[cfg(test)]
mod list_tests {
    use super::*;
    use crate::widgets::new_session_dialog::state::DartDefine;

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
    fn test_list_renders_defines() {
        let defines = vec![
            DartDefine::new("API_KEY", "secret"),
            DartDefine::new("DEBUG", "true"),
        ];
        let state = DartDefinesModalState::new(defines);

        let (mut buf, rect) = create_test_buffer(40, 10);
        let widget = DartDefinesListPane::new(&state);
        widget.render(rect, &mut buf);

        assert!(buffer_contains(&buf, "API_KEY"));
        assert!(buffer_contains(&buf, "DEBUG"));
        assert!(buffer_contains(&buf, "Add New"));
    }

    #[test]
    fn test_list_shows_selection() {
        let defines = vec![DartDefine::new("KEY", "value")];
        let state = DartDefinesModalState::new(defines);

        let (mut buf, rect) = create_test_buffer(40, 10);
        let widget = DartDefinesListPane::new(&state);
        widget.render(rect, &mut buf);

        // First item should have selection indicator
        assert!(buffer_contains(&buf, "> KEY"));
    }

    #[test]
    fn test_empty_list_shows_add_new() {
        let state = DartDefinesModalState::new(vec![]);

        let (mut buf, rect) = create_test_buffer(40, 10);
        let widget = DartDefinesListPane::new(&state);
        widget.render(rect, &mut buf);

        assert!(buffer_contains(&buf, "Add New"));
    }

    #[test]
    fn test_list_focused_border() {
        let mut state = DartDefinesModalState::new(vec![]);
        state.active_pane = DartDefinesPane::List;

        let (mut buf, rect) = create_test_buffer(40, 10);
        let widget = DartDefinesListPane::new(&state);
        widget.render(rect, &mut buf);

        // Check that focused border style is applied
        // Border cells should have cyan color
        let border_color = buf.cell((0, 0)).unwrap().fg;
        assert_eq!(border_color, palette::ACCENT);
    }

    #[test]
    fn test_list_unfocused_border() {
        let mut state = DartDefinesModalState::new(vec![]);
        state.active_pane = DartDefinesPane::Edit;

        let (mut buf, rect) = create_test_buffer(40, 10);
        let widget = DartDefinesListPane::new(&state);
        widget.render(rect, &mut buf);

        // Check that unfocused border style is applied
        let border_color = buf.cell((0, 0)).unwrap().fg;
        assert_eq!(border_color, palette::BORDER_DIM);
    }

    #[test]
    fn test_list_scroll_indicator() {
        let defines: Vec<DartDefine> = (0..20)
            .map(|i| DartDefine::new(format!("KEY_{}", i), "value"))
            .collect();
        let state = DartDefinesModalState::new(defines);

        let (mut buf, rect) = create_test_buffer(40, 10);
        let widget = DartDefinesListPane::new(&state);
        widget.render(rect, &mut buf);

        // Should show scroll position
        assert!(buffer_contains(&buf, "1/21"));
    }

    #[test]
    fn test_list_renders_title() {
        let state = DartDefinesModalState::new(vec![]);

        let (mut buf, rect) = create_test_buffer(40, 10);
        let widget = DartDefinesListPane::new(&state);
        widget.render(rect, &mut buf);

        assert!(buffer_contains(&buf, "Active Variables"));
    }

    #[test]
    fn test_list_selection_indicator_position() {
        let defines = vec![
            DartDefine::new("FIRST", "1"),
            DartDefine::new("SECOND", "2"),
        ];
        let mut state = DartDefinesModalState::new(defines);
        state.selected_index = 1;

        let (mut buf, rect) = create_test_buffer(40, 10);
        let widget = DartDefinesListPane::new(&state);
        widget.render(rect, &mut buf);

        // Second item should have selection indicator
        assert!(buffer_contains(&buf, "> SECOND"));
        // First should not
        assert!(buffer_contains(&buf, "  FIRST"));
    }
}

#[cfg(test)]
mod edit_tests {
    use super::*;
    use crate::widgets::new_session_dialog::state::DartDefine;

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

    #[test]
    fn test_edit_edit_title() {
        let defines = vec![DartDefine::new("KEY", "value")];
        let mut state = DartDefinesModalState::new(defines);
        state.load_selected_into_edit();

        let (mut buf, rect) = create_test_buffer(50, 15);
        let widget = DartDefinesEditPane::new(&state);
        widget.render(rect, &mut buf);

        assert!(buffer_contains(&buf, "Edit Variable"));
    }

    #[test]
    fn test_edit_focused_border() {
        let mut state = DartDefinesModalState::new(vec![]);
        state.active_pane = DartDefinesPane::Edit;

        let (mut buf, rect) = create_test_buffer(50, 15);
        let widget = DartDefinesEditPane::new(&state);
        widget.render(rect, &mut buf);

        // Check that focused border style is applied
        let border_color = buf.cell((0, 0)).unwrap().fg;
        assert_eq!(border_color, palette::ACCENT);
    }

    #[test]
    fn test_edit_unfocused_border() {
        let mut state = DartDefinesModalState::new(vec![]);
        state.active_pane = DartDefinesPane::List;

        let (mut buf, rect) = create_test_buffer(50, 15);
        let widget = DartDefinesEditPane::new(&state);
        widget.render(rect, &mut buf);

        // Check that unfocused border style is applied
        let border_color = buf.cell((0, 0)).unwrap().fg;
        assert_eq!(border_color, palette::BORDER_DIM);
    }
}

/// Full-screen dart defines modal widget
pub struct DartDefinesModal<'a> {
    state: &'a DartDefinesModalState,
}

impl<'a> DartDefinesModal<'a> {
    pub fn new(state: &'a DartDefinesModalState) -> Self {
        Self { state }
    }

    /// Calculate modal area (full screen with margin)
    fn modal_rect(area: Rect) -> Rect {
        Rect {
            x: area.x + 2,
            y: area.y + 1,
            width: area.width.saturating_sub(4),
            height: area.height.saturating_sub(2),
        }
    }

    fn render_header(&self, area: Rect, buf: &mut Buffer) {
        let title = Line::from(vec![
            Span::raw(" "),
            Span::styled(
                "📝 Manage Dart Defines",
                Style::default()
                    .fg(palette::ACCENT)
                    .add_modifier(Modifier::BOLD),
            ),
        ]);

        Paragraph::new(title)
            .alignment(Alignment::Center)
            .render(area, buf);
    }

    fn render_footer(&self, area: Rect, buf: &mut Buffer) {
        let hints = match self.state.active_pane {
            DartDefinesPane::List => {
                "[Tab] Switch Pane  [↑↓] Navigate  [Enter] Edit/Add  [Esc] Save & Close"
            }
            DartDefinesPane::Edit => "[Tab] Next Field  [Enter] Activate  [Esc] Save & Close",
        };

        Paragraph::new(hints)
            .style(Style::default().fg(palette::TEXT_MUTED))
            .alignment(Alignment::Center)
            .render(area, buf);
    }

    /// Render horizontal layout (list pane left, edit pane right)
    fn render_horizontal(&self, inner: Rect, buf: &mut Buffer) {
        // Layout: header | content | footer
        let vertical = Layout::vertical([
            Constraint::Length(2), // Header
            Constraint::Min(10),   // Content (panes)
            Constraint::Length(1), // Footer
        ])
        .split(inner);

        self.render_header(vertical[0], buf);

        // Content: list pane (40%) | edit pane (60%)
        let panes = Layout::horizontal([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(vertical[1]);

        // Render both panes
        let list_pane = DartDefinesListPane::new(self.state);
        list_pane.render(panes[0], buf);

        let edit_pane = DartDefinesEditPane::new(self.state);
        edit_pane.render(panes[1], buf);

        self.render_footer(vertical[2], buf);
    }

    /// Render vertical layout (list pane top, edit pane bottom)
    fn render_vertical(&self, inner: Rect, buf: &mut Buffer) {
        // Layout: header | list pane | edit pane | footer
        let vertical = Layout::vertical([
            Constraint::Length(2),      // Header
            Constraint::Percentage(40), // List pane
            Constraint::Min(8),         // Edit pane (needs min height for form)
            Constraint::Length(1),      // Footer
        ])
        .split(inner);

        self.render_header(vertical[0], buf);

        // Render both panes stacked vertically
        let list_pane = DartDefinesListPane::new(self.state);
        list_pane.render(vertical[1], buf);

        let edit_pane = DartDefinesEditPane::new(self.state);
        edit_pane.render(vertical[2], buf);

        self.render_footer(vertical[3], buf);
    }
}

impl Widget for DartDefinesModal<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Clear the entire area first
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.reset();
                    cell.set_style(Style::default().bg(palette::MODAL_DART_DEFINES_CLEAR_BG));
                }
            }
        }

        let modal_area = Self::modal_rect(area);

        // Outer border
        let outer_block = Block::default()
            .borders(Borders::ALL)
            .border_set(symbols::border::DOUBLE)
            .border_style(Style::default().fg(palette::ACCENT))
            .style(Style::default().bg(palette::MODAL_DART_DEFINES_BG));

        let inner = outer_block.inner(modal_area);
        outer_block.render(modal_area, buf);

        // Decide layout based on modal width
        if modal_area.width < 60 {
            self.render_vertical(inner, buf);
        } else {
            self.render_horizontal(inner, buf);
        }
    }
}

/// Render dimmed background for modal overlay
pub fn render_dart_defines_dim_overlay(area: Rect, buf: &mut Buffer) {
    for y in area.y..area.y + area.height {
        for x in area.x..area.x + area.width {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_style(
                    Style::default()
                        .fg(palette::TEXT_MUTED)
                        .bg(palette::DEEPEST_BG),
                );
            }
        }
    }
}

#[cfg(test)]
mod modal_tests {
    use super::*;
    use crate::widgets::new_session_dialog::state::DartDefine;

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
    fn test_modal_renders_title() {
        let state = DartDefinesModalState::new(vec![]);

        let (mut buf, rect) = create_test_buffer(80, 24);
        let widget = DartDefinesModal::new(&state);
        widget.render(rect, &mut buf);

        assert!(buffer_contains(&buf, "Manage Dart Defines"));
    }

    #[test]
    fn test_modal_renders_both_panes() {
        let defines = vec![DartDefine::new("TEST_KEY", "test_value")];
        let state = DartDefinesModalState::new(defines);

        let (mut buf, rect) = create_test_buffer(80, 24);
        let widget = DartDefinesModal::new(&state);
        widget.render(rect, &mut buf);

        // List pane content
        assert!(buffer_contains(&buf, "Active Variables"));
        assert!(buffer_contains(&buf, "TEST_KEY"));
        assert!(buffer_contains(&buf, "Add New"));

        // Edit pane content
        assert!(buffer_contains(&buf, "Key:"));
        assert!(buffer_contains(&buf, "Value:"));
        assert!(buffer_contains(&buf, "Save"));
        assert!(buffer_contains(&buf, "Delete"));
    }

    #[test]
    fn test_modal_shows_footer_hints() {
        let state = DartDefinesModalState::new(vec![]);

        let (mut buf, rect) = create_test_buffer(80, 24);
        let widget = DartDefinesModal::new(&state);
        widget.render(rect, &mut buf);

        assert!(buffer_contains(&buf, "Tab"));
        assert!(buffer_contains(&buf, "Esc"));
    }

    #[test]
    fn test_modal_footer_changes_by_pane() {
        let mut state = DartDefinesModalState::new(vec![]);

        // List pane footer
        state.active_pane = DartDefinesPane::List;
        let (mut buf, rect) = create_test_buffer(80, 24);
        DartDefinesModal::new(&state).render(rect, &mut buf);
        assert!(buffer_contains(&buf, "Navigate"));

        // Edit pane footer
        state.active_pane = DartDefinesPane::Edit;
        let (mut buf2, rect2) = create_test_buffer(80, 24);
        DartDefinesModal::new(&state).render(rect2, &mut buf2);
        assert!(buffer_contains(&buf2, "Next Field"));
    }

    #[test]
    fn test_modal_layout_proportions() {
        let state = DartDefinesModalState::new(vec![]);

        let (mut buf, rect) = create_test_buffer(100, 30);
        let widget = DartDefinesModal::new(&state);
        widget.render(rect, &mut buf);

        // Verify the layout renders without panicking
        // Visual inspection needed for exact proportions
    }

    #[test]
    fn test_modal_minimum_size() {
        let state = DartDefinesModalState::new(vec![]);

        // Minimum usable size
        let (mut buf, rect) = create_test_buffer(60, 15);
        let widget = DartDefinesModal::new(&state);
        widget.render(rect, &mut buf);

        // Should render without panic
        assert!(buffer_contains(&buf, "Dart Defines"));
    }
}
