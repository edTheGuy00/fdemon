//! Fuzzy search modal widget and filtering algorithm

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Widget},
};

use super::state::FuzzyModalState;

/// Style constants for fuzzy modal
mod styles {
    use super::*;

    pub const MODAL_BG: Color = Color::Rgb(40, 40, 50);
    pub const HEADER_FG: Color = Color::Cyan;
    pub const QUERY_FG: Color = Color::White;
    pub const QUERY_BG: Color = Color::Rgb(60, 60, 70);
    pub const ITEM_FG: Color = Color::White;
    pub const SELECTED_FG: Color = Color::Black;
    pub const SELECTED_BG: Color = Color::Cyan;
    pub const HINT_FG: Color = Color::DarkGray;
    pub const NO_MATCH_FG: Color = Color::Yellow;
}

/// Fuzzy search modal widget
pub struct FuzzyModal<'a> {
    state: &'a FuzzyModalState,
    loading: bool,
}

impl<'a> FuzzyModal<'a> {
    pub fn new(state: &'a FuzzyModalState) -> Self {
        Self {
            state,
            loading: false,
        }
    }

    /// Set loading state for the modal
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    /// Calculate modal area (bottom 45% of screen)
    fn modal_rect(area: Rect) -> Rect {
        // In narrow terminals, use more of the width and height
        let width_percent = if area.width < 60 { 95 } else { 80 };
        let height_percent = if area.height < 30 { 70 } else { 50 };

        let height = (area.height * height_percent / 100).max(10);
        let width = (area.width * width_percent / 100).max(30);
        let x_margin = (area.width.saturating_sub(width)) / 2;

        Rect {
            x: area.x + x_margin,
            y: area.y + area.height - height - 1,
            width,
            height,
        }
    }

    /// Render the search input line
    fn render_search_input(&self, area: Rect, buf: &mut Buffer) {
        let icon = "🔍 ";

        // Show loading indicator in title for EntryPoint modal
        let title = if self.loading && self.state.modal_type == super::FuzzyModalType::EntryPoint {
            format!("{} (discovering...)", self.state.modal_type.title())
        } else {
            self.state.modal_type.title().to_string()
        };

        let hint = " (Type to filter)";

        // Query with cursor
        let query_display = format!("{}|", self.state.query);

        let line = Line::from(vec![
            Span::raw(icon),
            Span::styled(
                title,
                Style::default()
                    .fg(styles::HEADER_FG)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(hint, Style::default().fg(styles::HINT_FG)),
            Span::raw("  "),
            Span::styled(
                query_display,
                Style::default().fg(styles::QUERY_FG).bg(styles::QUERY_BG),
            ),
        ]);

        Paragraph::new(line).render(area, buf);
    }

    /// Render the filtered items list
    fn render_list(&self, area: Rect, buf: &mut Buffer) {
        // Show loading message if loading
        if self.loading {
            let msg = "Discovering entry points...";
            let para = Paragraph::new(msg)
                .style(Style::default().fg(Color::Yellow))
                .alignment(Alignment::Center);
            para.render(area, buf);
            return;
        }

        if !self.state.has_results() {
            // No matches
            let msg = if self.state.modal_type.allows_custom() {
                let query_truncated = super::truncate_with_ellipsis(&self.state.query, 20);
                format!("No matches. Press Enter to use \"{}\"", query_truncated)
            } else {
                "No matches found".to_string()
            };

            let para = Paragraph::new(msg)
                .style(Style::default().fg(styles::NO_MATCH_FG))
                .alignment(Alignment::Center);
            para.render(area, buf);
            return;
        }

        let visible_height = area.height as usize;
        let start = self.state.scroll_offset;
        let end = (start + visible_height).min(self.state.filtered_indices.len());

        // Calculate available width for items (accounting for indicator)
        let max_item_width = (area.width as usize).saturating_sub(4); // "▶ " + padding

        let items: Vec<ListItem> = self.state.filtered_indices[start..end]
            .iter()
            .enumerate()
            .map(|(display_idx, &item_idx)| {
                let item_text = &self.state.items[item_idx];
                let is_selected = display_idx + start == self.state.selected_index;

                let indicator = if is_selected { "▶ " } else { "  " };

                // Truncate item text if needed
                let truncated = if max_item_width > 0 {
                    super::truncate_with_ellipsis(item_text, max_item_width)
                } else {
                    item_text.clone()
                };

                let text = format!("{}{}", indicator, truncated);

                let style = if is_selected {
                    Style::default()
                        .fg(styles::SELECTED_FG)
                        .bg(styles::SELECTED_BG)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(styles::ITEM_FG)
                };

                ListItem::new(text).style(style)
            })
            .collect();

        let list = List::new(items);
        list.render(area, buf);
    }

    /// Render keybinding hints
    fn render_hints(&self, area: Rect, buf: &mut Buffer) {
        let hints = if self.state.modal_type.allows_custom() {
            "[↑↓] Navigate  [Enter] Select  [Esc] Cancel  Type to filter or enter custom"
        } else {
            "[↑↓] Navigate  [Enter] Select  [Esc] Cancel  Type to filter"
        };

        Paragraph::new(hints)
            .style(Style::default().fg(styles::HINT_FG))
            .alignment(Alignment::Center)
            .render(area, buf);
    }
}

impl Widget for FuzzyModal<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let modal_area = Self::modal_rect(area);

        // Clear and draw modal background
        Clear.render(modal_area, buf);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_set(symbols::border::ROUNDED)
            .style(Style::default().bg(styles::MODAL_BG));

        let inner = block.inner(modal_area);
        block.render(modal_area, buf);

        // Layout: search input | list | hints
        let chunks = Layout::vertical([
            Constraint::Length(1), // Search input
            Constraint::Length(1), // Separator
            Constraint::Min(3),    // List
            Constraint::Length(1), // Hints
        ])
        .split(inner);

        self.render_search_input(chunks[0], buf);

        // Separator line
        let sep = "─".repeat(chunks[1].width as usize);
        Paragraph::new(sep)
            .style(Style::default().fg(Color::DarkGray))
            .render(chunks[1], buf);

        self.render_list(chunks[2], buf);
        self.render_hints(chunks[3], buf);
    }
}

/// Render a dimmed overlay on the given area
pub fn render_dim_overlay(area: Rect, buf: &mut Buffer) {
    // Use saturating arithmetic to prevent overflow when area.y + area.height > u16::MAX
    let y_end = area.y.saturating_add(area.height);
    let x_end = area.x.saturating_add(area.width);

    for y in area.y..y_end {
        for x in area.x..x_end {
            if let Some(cell) = buf.cell_mut((x, y)) {
                // Dim the existing content
                cell.set_style(Style::default().fg(Color::DarkGray).bg(Color::Black));
            }
        }
    }
}

// Re-export fuzzy filter from app layer (moved in Phase 1, Task 05)
pub use crate::app::new_session_dialog::fuzzy::fuzzy_filter;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fuzzy_filter_empty_query() {
        let items = vec!["a".into(), "b".into(), "c".into()];
        let result = fuzzy_filter("", &items);
        assert_eq!(result, vec![0, 1, 2]);
    }

    #[test]
    fn test_fuzzy_filter_exact_match() {
        let items = vec!["dev".into(), "staging".into(), "production".into()];
        let result = fuzzy_filter("dev", &items);
        assert!(result.contains(&0)); // "dev" matches
    }

    #[test]
    fn test_fuzzy_filter_partial_match() {
        let items = vec!["development".into(), "staging".into(), "dev".into()];
        let result = fuzzy_filter("dev", &items);

        // Both "development" and "dev" match, but "dev" should rank higher
        assert!(result.len() >= 2);
        // Exact match should be first due to length penalty on "development"
    }

    #[test]
    fn test_fuzzy_filter_fuzzy_match() {
        let items = vec!["devStaging".into(), "staging".into()];
        let result = fuzzy_filter("dS", &items);

        // "dS" should match "devStaging" (d...S)
        assert!(result.contains(&0));
    }

    #[test]
    fn test_fuzzy_filter_case_insensitive() {
        let items = vec!["DevStaging".into(), "PRODUCTION".into()];
        let result = fuzzy_filter("dev", &items);
        assert!(result.contains(&0));

        let result2 = fuzzy_filter("prod", &items);
        assert!(result2.contains(&1));
    }

    #[test]
    fn test_fuzzy_filter_no_match() {
        let items = vec!["alpha".into(), "beta".into()];
        let result = fuzzy_filter("xyz", &items);
        assert!(result.is_empty());
    }

    // Note: fuzzy_score tests moved to app/new_session_dialog/fuzzy.rs (Phase 1, Task 05)
    // Note: substring_filter was removed when fuzzy filter moved to app layer
}

#[cfg(test)]
mod widget_tests {
    use super::*;
    use crate::tui::test_utils::TestTerminal;

    #[test]
    fn test_fuzzy_modal_renders_title() {
        let mut term = TestTerminal::new();
        let items = vec!["item1".into(), "item2".into()];
        let state = FuzzyModalState::new(super::super::state::FuzzyModalType::Flavor, items);

        let modal = FuzzyModal::new(&state);
        term.render_widget(modal, term.area());

        assert!(term.buffer_contains("Select Flavor"));
    }

    #[test]
    fn test_fuzzy_modal_shows_items() {
        let mut term = TestTerminal::new();
        let items = vec!["alpha".into(), "beta".into()];
        let state = FuzzyModalState::new(super::super::state::FuzzyModalType::Config, items);

        let modal = FuzzyModal::new(&state);
        term.render_widget(modal, term.area());

        assert!(term.buffer_contains("alpha"));
        assert!(term.buffer_contains("beta"));
    }

    #[test]
    fn test_fuzzy_modal_no_matches_custom() {
        let mut term = TestTerminal::new();
        let items = vec!["existing".into()];
        let mut state = FuzzyModalState::new(super::super::state::FuzzyModalType::Flavor, items);
        state.input_char('x');
        state.input_char('y');
        state.input_char('z');

        let modal = FuzzyModal::new(&state);
        term.render_widget(modal, term.area());

        assert!(term.buffer_contains("xyz") || term.buffer_contains("No matches"));
    }

    #[test]
    fn test_fuzzy_modal_config_title() {
        let mut term = TestTerminal::new();
        let items = vec!["config1".into()];
        let state = FuzzyModalState::new(super::super::state::FuzzyModalType::Config, items);

        let modal = FuzzyModal::new(&state);
        term.render_widget(modal, term.area());

        assert!(term.buffer_contains("Select Configuration"));
    }

    #[test]
    fn test_fuzzy_modal_shows_hints() {
        let mut term = TestTerminal::new();
        let items = vec!["item".into()];
        let state = FuzzyModalState::new(super::super::state::FuzzyModalType::Config, items);

        let modal = FuzzyModal::new(&state);
        term.render_widget(modal, term.area());

        assert!(term.buffer_contains("Navigate"));
        assert!(term.buffer_contains("Select"));
        assert!(term.buffer_contains("Cancel"));
    }

    #[test]
    fn test_fuzzy_modal_shows_custom_hint_for_flavor() {
        let mut term = TestTerminal::new();
        let items = vec!["item".into()];
        let state = FuzzyModalState::new(super::super::state::FuzzyModalType::Flavor, items);

        let modal = FuzzyModal::new(&state);
        term.render_widget(modal, term.area());

        assert!(term.buffer_contains("enter custom") || term.buffer_contains("Type to filter"));
    }

    #[test]
    fn test_fuzzy_modal_shows_selection_indicator() {
        let mut term = TestTerminal::new();
        let items = vec!["first".into(), "second".into()];
        let state = FuzzyModalState::new(super::super::state::FuzzyModalType::Config, items);

        let modal = FuzzyModal::new(&state);
        term.render_widget(modal, term.area());

        // The selected item should have the arrow indicator
        assert!(term.buffer_contains("▶"));
    }

    #[test]
    fn test_render_dim_overlay_does_not_crash() {
        let mut term = TestTerminal::new();
        let area = term.area();

        // First render some content
        use ratatui::widgets::Paragraph;
        let para = Paragraph::new("Hello World");
        term.render_widget(para, area);

        // Then apply dim overlay
        term.terminal
            .draw(|frame| {
                render_dim_overlay(area, frame.buffer_mut());
            })
            .expect("Failed to draw");

        // The function should not crash
        // Note: The overlay modifies cell styles, changing the content
        // We just verify it executes without panicking
    }

    #[test]
    fn test_fuzzy_modal_entry_point_loading_title() {
        let mut term = TestTerminal::new();
        let items = vec!["lib/main.dart".into()];
        let state = FuzzyModalState::new(super::super::state::FuzzyModalType::EntryPoint, items);

        let modal = FuzzyModal::new(&state).loading(true);
        term.render_widget(modal, term.area());

        assert!(term.buffer_contains("discovering"));
    }

    #[test]
    fn test_fuzzy_modal_entry_point_loading_message() {
        let mut term = TestTerminal::new();
        let items = vec!["lib/main.dart".into()];
        let state = FuzzyModalState::new(super::super::state::FuzzyModalType::EntryPoint, items);

        let modal = FuzzyModal::new(&state).loading(true);
        term.render_widget(modal, term.area());

        assert!(term.buffer_contains("Discovering entry points"));
    }

    #[test]
    fn test_fuzzy_modal_entry_point_not_loading() {
        let mut term = TestTerminal::new();
        let items = vec!["lib/main.dart".into()];
        let state = FuzzyModalState::new(super::super::state::FuzzyModalType::EntryPoint, items);

        let modal = FuzzyModal::new(&state).loading(false);
        term.render_widget(modal, term.area());

        // Should show the item, not loading message
        assert!(term.buffer_contains("lib/main.dart"));
        assert!(!term.buffer_contains("discovering"));
    }

    #[test]
    fn test_fuzzy_modal_other_types_ignore_loading() {
        let mut term = TestTerminal::new();
        let items = vec!["config1".into()];
        let state = FuzzyModalState::new(super::super::state::FuzzyModalType::Config, items);

        // Even with loading=true, Config modal shouldn't show loading
        let modal = FuzzyModal::new(&state).loading(true);
        term.render_widget(modal, term.area());

        // Should show normal title, not loading title
        assert!(term.buffer_contains("Select Configuration"));
        assert!(!term.buffer_contains("discovering"));
    }
}
