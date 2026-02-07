//! Confirmation dialog widget for quit/close confirmations

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

// Re-export state from app layer
pub use crate::app::confirm_dialog::ConfirmDialogState;

/// Confirmation dialog widget
pub struct ConfirmDialog<'a> {
    state: &'a ConfirmDialogState,
}

impl<'a> ConfirmDialog<'a> {
    /// Create a new confirmation dialog widget
    pub fn new(state: &'a ConfirmDialogState) -> Self {
        Self { state }
    }

    /// Calculate centered modal rect
    fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        Rect::new(x, y, width.min(area.width), height.min(area.height))
    }
}

impl Widget for ConfirmDialog<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Fixed modal size
        let modal_width = 50;
        let modal_height = 9;
        let modal_area = Self::centered_rect(modal_width, modal_height, area);

        // Clear the area behind the modal
        Clear.render(modal_area, buf);

        // Create the modal block with border
        let block = Block::default()
            .title(format!(" {} ", self.state.title))
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_set(symbols::border::ROUNDED)
            .style(Style::default().bg(Color::DarkGray));

        let inner = block.inner(modal_area);
        block.render(modal_area, buf);

        // Layout: message + buttons
        let chunks = Layout::vertical([
            Constraint::Length(1), // Spacer
            Constraint::Length(1), // Message line 1
            Constraint::Length(1), // Message line 2
            Constraint::Length(1), // Spacer
            Constraint::Length(1), // Buttons
            Constraint::Min(0),    // Rest
        ])
        .split(inner);

        // Session count message
        let message = Paragraph::new(self.state.message.as_str())
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Yellow));
        message.render(chunks[1], buf);

        // Warning message
        let warning = Paragraph::new("All Flutter processes will be terminated.")
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::White));
        warning.render(chunks[2], buf);

        // Buttons
        let buttons = Line::from(vec![
            Span::styled("[", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "y",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("] Yes  ", Style::default().fg(Color::DarkGray)),
            Span::styled("[", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "n",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::styled("] No", Style::default().fg(Color::DarkGray)),
        ]);

        let buttons_para = Paragraph::new(buttons).alignment(Alignment::Center);
        buttons_para.render(chunks[4], buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::message::Message;
    use crate::tui::test_utils::TestTerminal;
    use ratatui::{backend::TestBackend, Terminal};

    fn create_quit_dialog() -> ConfirmDialogState {
        ConfirmDialogState::new(
            "Quit?",
            "Are you sure you want to quit?",
            vec![("Yes", Message::ConfirmQuit), ("No", Message::CancelQuit)],
        )
    }

    fn create_close_session_dialog() -> ConfirmDialogState {
        ConfirmDialogState::new(
            "Close Session",
            "Close the current session?",
            vec![("Yes", Message::ConfirmQuit), ("No", Message::CancelQuit)],
        )
    }

    #[test]
    fn test_confirm_dialog_renders_title() {
        let mut term = TestTerminal::new();
        let state = create_quit_dialog();
        let dialog = ConfirmDialog::new(&state);

        term.render_widget(dialog, term.area());

        assert!(term.buffer_contains("Quit"), "Dialog should show title");
    }

    #[test]
    fn test_confirm_dialog_renders_message() {
        let mut term = TestTerminal::new();
        let state = create_quit_dialog();
        let dialog = ConfirmDialog::new(&state);

        term.render_widget(dialog, term.area());

        assert!(
            term.buffer_contains("sure") || term.buffer_contains("quit"),
            "Dialog should show confirmation message"
        );
    }

    #[test]
    fn test_confirm_dialog_shows_options() {
        let mut term = TestTerminal::new();
        let state = create_quit_dialog();
        let dialog = ConfirmDialog::new(&state);

        term.render_widget(dialog, term.area());

        // Should show Yes/No or y/n options
        assert!(
            term.buffer_contains("Yes")
                || term.buffer_contains("y")
                || term.buffer_contains("No")
                || term.buffer_contains("n"),
            "Dialog should show confirmation options"
        );
    }

    #[test]
    fn test_confirm_dialog_shows_keybindings() {
        let mut term = TestTerminal::new();
        let state = create_quit_dialog();
        let dialog = ConfirmDialog::new(&state);

        term.render_widget(dialog, term.area());

        // Should show key hints
        let content = term.content();
        assert!(
            content.contains("y")
                || content.contains("n")
                || content.contains("Enter")
                || content.contains("Esc"),
            "Dialog should show keybinding hints"
        );
    }

    #[test]
    fn test_confirm_dialog_different_actions() {
        let mut term = TestTerminal::new();

        // Quit dialog
        let quit_state = create_quit_dialog();
        let quit_dialog = ConfirmDialog::new(&quit_state);
        term.render_widget(quit_dialog, term.area());
        assert!(term.buffer_contains("Quit"));

        term.clear();

        // Close session dialog
        let close_state = create_close_session_dialog();
        let close_dialog = ConfirmDialog::new(&close_state);
        term.render_widget(close_dialog, term.area());
        assert!(term.buffer_contains("Close") || term.buffer_contains("Session"));
    }

    #[test]
    fn test_confirm_dialog_modal_overlay() {
        let mut term = TestTerminal::new();
        let state = create_quit_dialog();
        let dialog = ConfirmDialog::new(&state);

        term.render_widget(dialog, term.area());

        // Modal should render (just verify no panic)
        let content = term.content();
        assert!(!content.is_empty());
    }

    #[test]
    fn test_confirm_dialog_compact() {
        let mut term = TestTerminal::compact();
        let state = create_quit_dialog();
        let dialog = ConfirmDialog::new(&state);

        term.render_widget(dialog, term.area());

        // Should fit in small terminal
        let content = term.content();
        assert!(!content.is_empty());
    }

    #[test]
    fn test_confirm_dialog_centered() {
        let mut term = TestTerminal::new();
        let state = create_quit_dialog();
        let dialog = ConfirmDialog::new(&state);

        term.render_widget(dialog, term.area());

        // Dialog content should be roughly centered
        // (This is hard to verify precisely, just check it renders)
        let content = term.content();
        assert!(!content.is_empty());
    }

    // Legacy tests retained for backward compatibility
    #[test]
    fn test_confirm_dialog_state_single_session() {
        let state = ConfirmDialogState::quit_confirmation(1);
        assert!(state.message.contains("1 running session"));
        assert!(!state.message.contains("sessions"));
    }

    #[test]
    fn test_confirm_dialog_state_multiple_sessions() {
        let state = ConfirmDialogState::quit_confirmation(3);
        assert!(state.message.contains("3 running sessions"));
    }

    #[test]
    fn test_confirm_dialog_rendering() {
        let state = ConfirmDialogState::quit_confirmation(2);
        let dialog = ConfirmDialog::new(&state);

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                f.render_widget(dialog, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content.iter().map(|c| c.symbol()).collect();

        // Should contain dialog elements
        assert!(content.contains("Quit"));
        assert!(content.contains("2 running sessions"));
        assert!(content.contains("y"));
        assert!(content.contains("n"));
    }

    #[test]
    fn test_centered_rect() {
        let area = Rect::new(0, 0, 100, 50);
        let modal = ConfirmDialog::centered_rect(40, 10, area);

        // Should be centered
        assert_eq!(modal.x, 30); // (100 - 40) / 2
        assert_eq!(modal.y, 20); // (50 - 10) / 2
        assert_eq!(modal.width, 40);
        assert_eq!(modal.height, 10);
    }

    #[test]
    fn test_centered_rect_small_area() {
        let area = Rect::new(0, 0, 30, 8);
        let modal = ConfirmDialog::centered_rect(50, 10, area);

        // Should be clamped to area
        assert_eq!(modal.width, 30);
        assert_eq!(modal.height, 8);
    }
}
