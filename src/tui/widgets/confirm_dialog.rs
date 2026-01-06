//! Confirmation dialog widget for quit/close confirmations

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

use crate::app::message::Message;

/// State for the confirmation dialog
#[derive(Debug, Clone)]
pub struct ConfirmDialogState {
    /// The title of the dialog
    pub title: String,
    /// The message to display
    pub message: String,
    /// Number of running sessions (for display)
    pub session_count: usize,
    /// Available options (label, message)
    pub options: Vec<(String, Message)>,
}

impl ConfirmDialogState {
    /// Create a generic confirmation dialog
    pub fn new(
        title: impl Into<String>,
        message: impl Into<String>,
        options: Vec<(&str, Message)>,
    ) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            session_count: 0,
            options: options
                .into_iter()
                .map(|(label, msg)| (label.to_string(), msg))
                .collect(),
        }
    }

    /// Create a quit confirmation dialog state
    pub fn quit_confirmation(session_count: usize) -> Self {
        Self {
            title: "Quit Flutter Demon?".to_string(),
            message: if session_count == 1 {
                "You have 1 running session.".to_string()
            } else {
                format!("You have {} running sessions.", session_count)
            },
            session_count,
            options: vec![
                ("Quit".to_string(), Message::ConfirmQuit),
                ("Cancel".to_string(), Message::CancelQuit),
            ],
        }
    }
}

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
    use ratatui::{backend::TestBackend, Terminal};

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
