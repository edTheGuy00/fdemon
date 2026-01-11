## Task: Implement Dart Defines Modal Container Widget

**Objective**: Create the full-screen modal container that combines list and edit panes.

**Depends on**: Task 03 (Edit Widget)

**Estimated Time**: 20 minutes

### Background

The dart defines modal is a full-screen overlay that replaces the main dialog visually. It contains the list pane on the left and edit pane on the right, plus a footer with keybinding hints.

### Scope

- `src/tui/widgets/new_session_dialog/dart_defines_modal.rs`: Add container widget

### Changes Required

**Add to `dart_defines_modal.rs`:**

```rust
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
                    .fg(Color::Cyan)
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
            DartDefinesPane::Edit => {
                "[Tab] Next Field  [Enter] Activate  [Esc] Save & Close"
            }
        };

        Paragraph::new(hints)
            .style(Style::default().fg(styles::HINT_FG))
            .alignment(Alignment::Center)
            .render(area, buf);
    }
}

impl Widget for DartDefinesModal<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Clear the entire area first
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.reset();
                    cell.set_style(Style::default().bg(Color::Rgb(20, 20, 30)));
                }
            }
        }

        let modal_area = Self::modal_rect(area);

        // Outer border
        let outer_block = Block::default()
            .borders(Borders::ALL)
            .border_set(symbols::border::DOUBLE)
            .border_style(Style::default().fg(Color::Cyan))
            .style(Style::default().bg(styles::MODAL_BG));

        let inner = outer_block.inner(modal_area);
        outer_block.render(modal_area, buf);

        // Layout: header | content | footer
        let vertical = Layout::vertical([
            Constraint::Length(2),  // Header
            Constraint::Min(10),    // Content (panes)
            Constraint::Length(1),  // Footer
        ])
        .split(inner);

        self.render_header(vertical[0], buf);

        // Content: list pane (40%) | edit pane (60%)
        let panes = Layout::horizontal([
            Constraint::Percentage(40),
            Constraint::Percentage(60),
        ])
        .split(vertical[1]);

        // Render both panes
        let list_pane = DartDefinesListPane::new(self.state);
        list_pane.render(panes[0], buf);

        let edit_pane = DartDefinesEditPane::new(self.state);
        edit_pane.render(panes[1], buf);

        self.render_footer(vertical[2], buf);
    }
}

/// Render dimmed background for modal overlay
pub fn render_dart_defines_dim_overlay(area: Rect, buf: &mut Buffer) {
    for y in area.y..area.y + area.height {
        for x in area.x..area.x + area.width {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_style(Style::default().fg(Color::DarkGray).bg(Color::Black));
            }
        }
    }
}

#[cfg(test)]
mod modal_tests {
    use super::*;

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
}
```

### Acceptance Criteria

1. `DartDefinesModal` widget struct with `new(state)` constructor
2. Full-screen layout with 2-cell margin
3. Double-line outer border in cyan
4. Header shows "📝 Manage Dart Defines" title
5. Content area split 40/60 between list and edit panes
6. Footer shows context-sensitive keybinding hints
7. Footer hints change based on active pane
8. `render_dart_defines_dim_overlay()` helper for background
9. `cargo check` passes
10. `cargo clippy -- -D warnings` passes

### Testing

```rust
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
```

### Notes

- Modal uses double-line border to distinguish from inner pane borders
- Header emoji may need fallback for non-Unicode terminals
- Footer hints are context-sensitive based on active pane
- Consider minimum size requirements for usability
- Background dimming applied before modal render
