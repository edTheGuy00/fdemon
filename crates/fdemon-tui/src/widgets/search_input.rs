//! Search input prompt widget

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
};

use fdemon_core::SearchState;

use crate::theme::palette;

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
                    .fg(palette::STATUS_YELLOW)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                &self.search_state.query,
                Style::default().fg(palette::TEXT_PRIMARY),
            ),
        ];

        // Add cursor
        if self.search_state.is_active {
            spans.push(Span::styled(
                "_",
                Style::default().fg(palette::STATUS_YELLOW),
            ));
        }

        // Add match count if query is not empty
        if !self.search_state.query.is_empty() {
            let status = self.search_state.display_status();
            if !status.is_empty() {
                spans.push(Span::raw(" "));

                // Color based on whether matches were found
                let status_style = if self.search_state.has_matches() {
                    Style::default().fg(palette::STATUS_GREEN)
                } else {
                    Style::default().fg(palette::STATUS_RED)
                };
                spans.push(Span::styled(status, status_style));
            }

            // Show error if regex is invalid
            if let Some(ref error) = self.search_state.error {
                spans.push(Span::raw(" "));
                // Truncate error message if too long (char-aware to avoid panic on multi-byte UTF-8)
                let short_error = if error.chars().count() > 30 {
                    format!("{}...", error.chars().take(27).collect::<String>())
                } else {
                    error.clone()
                };
                spans.push(Span::styled(
                    format!("({})", short_error),
                    Style::default().fg(palette::STATUS_RED),
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
            .border_style(Style::default().fg(palette::STATUS_YELLOW));

        let inner = block.inner(popup_area);
        block.render(popup_area, buf);

        // Render search content
        let mut spans = vec![
            Span::styled(
                "/",
                Style::default()
                    .fg(palette::STATUS_YELLOW)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                &self.search_state.query,
                Style::default().fg(palette::TEXT_PRIMARY),
            ),
            Span::styled("_", Style::default().fg(palette::STATUS_YELLOW)),
        ];

        // Add status on same line if room
        let status = self.search_state.display_status();
        if !status.is_empty() && inner.width > 30 {
            spans.push(Span::raw("  "));
            let status_style = if self.search_state.has_matches() {
                Style::default().fg(palette::STATUS_GREEN)
            } else {
                Style::default().fg(palette::STATUS_RED)
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

    // ── UTF-8 error truncation tests ─────────────────────────────────────────

    /// Build a SearchState with a manually set error message (bypassing regex).
    fn make_state_with_error(error_msg: &str) -> SearchState {
        let mut state = SearchState::default();
        // Set a non-empty query so the error display branch is reached
        state.query = "x".to_string();
        state.is_active = true;
        state.error = Some(error_msg.to_string());
        state
    }

    #[test]
    fn test_search_error_truncation_with_cyrillic() {
        // Cyrillic characters are 2 bytes each; byte-indexing at 27 would
        // panic mid-codepoint without the char-based fix.
        let cyrillic_error = "ошибка разбора регулярного выражения: неверный синтаксис";
        assert!(cyrillic_error.chars().count() > 30);
        let state = make_state_with_error(cyrillic_error);
        let widget = SearchInput::new(&state).inline();
        let area = ratatui::layout::Rect::new(0, 0, 80, 1);
        let mut buf = ratatui::buffer::Buffer::empty(area);
        // Must not panic
        widget.render(area, &mut buf);
    }

    #[test]
    fn test_search_error_truncation_with_cjk() {
        // CJK characters are 3 bytes each; byte-indexing would panic without the char-based fix.
        let cjk_error =
            "正则表达式解析错误：模式无效，因为存在未闭合的括号结构以及其他问题导致失败";
        assert!(cjk_error.chars().count() > 30);
        let state = make_state_with_error(cjk_error);
        let widget = SearchInput::new(&state).inline();
        let area = ratatui::layout::Rect::new(0, 0, 80, 1);
        let mut buf = ratatui::buffer::Buffer::empty(area);
        // Must not panic
        widget.render(area, &mut buf);
    }

    #[test]
    fn test_search_error_truncation_result_contains_ellipsis() {
        // Long ASCII error should be truncated with "..."
        let long_error = "regex parse error: unclosed group in pattern at position 5";
        assert!(long_error.chars().count() > 30);
        let state = make_state_with_error(long_error);
        let widget = SearchInput::new(&state).inline();
        let area = ratatui::layout::Rect::new(0, 0, 120, 1);
        let mut buf = ratatui::buffer::Buffer::empty(area);
        widget.render(area, &mut buf);
        let content: String = (0..120u16)
            .filter_map(|x| buf.cell((x, 0u16)).map(|c| c.symbol().to_string()))
            .collect();
        assert!(
            content.contains("..."),
            "Truncated error should end with '...'"
        );
    }

    #[test]
    fn test_search_error_no_truncation_for_short_error() {
        // Short error (<= 30 chars) should be shown in full
        let short_error = "invalid pattern";
        assert!(short_error.chars().count() <= 30);
        let state = make_state_with_error(short_error);
        let widget = SearchInput::new(&state).inline();
        let area = ratatui::layout::Rect::new(0, 0, 80, 1);
        let mut buf = ratatui::buffer::Buffer::empty(area);
        widget.render(area, &mut buf);
        let content: String = (0..80u16)
            .filter_map(|x| buf.cell((x, 0u16)).map(|c| c.symbol().to_string()))
            .collect();
        assert!(
            content.contains("invalid pattern"),
            "Short error should appear untruncated"
        );
    }
}
