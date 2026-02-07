//! Search input prompt widget

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

use fdemon_core::SearchState;

/// Search input prompt widget
pub struct SearchInput<'a> {
    /// The search state containing query and status
    search_state: &'a SearchState,
    /// Whether to show as inline or popup
    inline: bool,
}

impl<'a> SearchInput<'a> {
    pub fn new(search_state: &'a SearchState) -> Self {
        Self {
            search_state,
            inline: false,
        }
    }

    /// Render as inline prompt (at bottom of log view)
    pub fn inline(mut self) -> Self {
        self.inline = true;
        self
    }
}

impl Widget for SearchInput<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if self.inline {
            self.render_inline(area, buf);
        } else {
            self.render_popup(area, buf);
        }
    }
}

impl SearchInput<'_> {
    /// Render as inline search bar
    fn render_inline(self, area: Rect, buf: &mut Buffer) {
        // Format: "/query" or "/query [3/10 matches]"
        let mut spans = vec![
            Span::styled(
                "/",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(&self.search_state.query, Style::default().fg(Color::White)),
        ];

        // Add cursor
        if self.search_state.is_active {
            spans.push(Span::styled("_", Style::default().fg(Color::Yellow)));
        }

        // Add match count if query is not empty
        if !self.search_state.query.is_empty() {
            let status = self.search_state.display_status();
            if !status.is_empty() {
                spans.push(Span::raw(" "));

                // Color based on whether matches were found
                let status_style = if self.search_state.has_matches() {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::Red)
                };
                spans.push(Span::styled(status, status_style));
            }

            // Show error if regex is invalid
            if let Some(ref error) = self.search_state.error {
                spans.push(Span::raw(" "));
                // Truncate error message if too long
                let short_error = if error.len() > 30 {
                    format!("{}...", &error[..27])
                } else {
                    error.clone()
                };
                spans.push(Span::styled(
                    format!("({})", short_error),
                    Style::default().fg(Color::Red),
                ));
            }
        }

        let line = Line::from(spans);
        Paragraph::new(line).render(area, buf);
    }

    /// Render as centered popup
    fn render_popup(self, area: Rect, buf: &mut Buffer) {
        // Calculate popup dimensions
        let width = 50.min(area.width.saturating_sub(4));
        let height = 3;

        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;

        let popup_area = Rect::new(x, y, width, height);

        // Clear the area behind the popup
        Clear.render(popup_area, buf);

        // Draw popup with border
        let block = Block::default()
            .title(" Search ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow));

        let inner = block.inner(popup_area);
        block.render(popup_area, buf);

        // Render search content
        let mut spans = vec![
            Span::styled(
                "/",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(&self.search_state.query, Style::default().fg(Color::White)),
            Span::styled("_", Style::default().fg(Color::Yellow)),
        ];

        // Add status on same line if room
        let status = self.search_state.display_status();
        if !status.is_empty() && inner.width > 30 {
            spans.push(Span::raw("  "));
            let status_style = if self.search_state.has_matches() {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::Red)
            };
            spans.push(Span::styled(status, status_style));
        }

        let line = Line::from(spans);
        Paragraph::new(line).render(inner, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_search_state(query: &str, active: bool) -> SearchState {
        let mut state = SearchState::default();
        state.set_query(query);
        state.is_active = active;
        state
    }

    #[test]
    fn test_search_input_new() {
        let state = make_search_state("test", true);
        let widget = SearchInput::new(&state);
        assert!(!widget.inline);
    }

    #[test]
    fn test_search_input_inline() {
        let state = make_search_state("test", true);
        let widget = SearchInput::new(&state).inline();
        assert!(widget.inline);
    }

    #[test]
    fn test_search_input_empty_query() {
        let state = make_search_state("", true);
        let widget = SearchInput::new(&state);
        assert!(widget.search_state.query.is_empty());
    }

    #[test]
    fn test_search_input_with_valid_regex() {
        let state = make_search_state("error|warn", true);
        let widget = SearchInput::new(&state);
        assert!(widget.search_state.is_valid);
        assert!(widget.search_state.error.is_none());
    }

    #[test]
    fn test_search_input_with_invalid_regex() {
        let state = make_search_state("[invalid", true);
        let widget = SearchInput::new(&state);
        assert!(!widget.search_state.is_valid);
        assert!(widget.search_state.error.is_some());
    }
}
