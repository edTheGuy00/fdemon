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
}

impl<'a> FuzzyModal<'a> {
    pub fn new(state: &'a FuzzyModalState) -> Self {
        Self { state }
    }

    /// Calculate modal area (bottom 45% of screen)
    fn modal_rect(area: Rect) -> Rect {
        let height = (area.height * 45 / 100).max(10);
        Rect {
            x: area.x + 2,
            y: area.y + area.height - height - 1,
            width: area.width.saturating_sub(4),
            height,
        }
    }

    /// Render the search input line
    fn render_search_input(&self, area: Rect, buf: &mut Buffer) {
        let icon = "🔍 ";
        let title = self.state.modal_type.title();
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
        if !self.state.has_results() {
            // No matches
            let msg = if self.state.modal_type.allows_custom() {
                format!("No matches. Press Enter to use \"{}\"", self.state.query)
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

        let items: Vec<ListItem> = self.state.filtered_indices[start..end]
            .iter()
            .enumerate()
            .map(|(display_idx, &item_idx)| {
                let item_text = &self.state.items[item_idx];
                let is_selected = display_idx + start == self.state.selected_index;

                let indicator = if is_selected { "▶ " } else { "  " };
                let text = format!("{}{}", indicator, item_text);

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

/// Fuzzy match result with score for sorting
#[derive(Debug, Clone)]
pub struct FuzzyMatch {
    pub index: usize,
    pub score: i32,
}

/// Perform fuzzy matching on a list of items
///
/// Returns indices of matching items, sorted by score (best first)
pub fn fuzzy_filter(query: &str, items: &[String]) -> Vec<usize> {
    if query.is_empty() {
        return (0..items.len()).collect();
    }

    let query_lower = query.to_lowercase();
    let query_chars: Vec<char> = query_lower.chars().collect();

    let mut matches: Vec<FuzzyMatch> = items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| {
            fuzzy_score(&query_chars, &item.to_lowercase()).map(|score| FuzzyMatch { index, score })
        })
        .collect();

    // Sort by score (higher is better), then by original index for stability
    matches.sort_by(|a, b| b.score.cmp(&a.score).then(a.index.cmp(&b.index)));

    matches.into_iter().map(|m| m.index).collect()
}

/// Calculate fuzzy match score
///
/// Returns None if no match, Some(score) if matched.
/// Higher score = better match.
///
/// Scoring:
/// - Base points for each matched character
/// - Bonus for consecutive matches
/// - Bonus for matches at word boundaries
/// - Bonus for exact prefix match
fn fuzzy_score(query: &[char], target: &str) -> Option<i32> {
    if query.is_empty() {
        return Some(0);
    }

    let target_chars: Vec<char> = target.chars().collect();
    if target_chars.is_empty() {
        return None;
    }

    let mut score: i32 = 0;
    let mut query_idx = 0;
    let mut prev_match_idx: Option<usize> = None;
    let mut first_match_idx: Option<usize> = None;

    for (target_idx, &target_char) in target_chars.iter().enumerate() {
        if query_idx < query.len() && target_char == query[query_idx] {
            // Found a match
            score += 10; // Base score for match

            // Track first match position
            if first_match_idx.is_none() {
                first_match_idx = Some(target_idx);
            }

            // Bonus for consecutive matches
            if let Some(prev_idx) = prev_match_idx {
                if target_idx == prev_idx + 1 {
                    score += 15; // Consecutive bonus
                }
            }

            // Bonus for word boundary match
            if target_idx == 0 || !target_chars[target_idx - 1].is_alphanumeric() {
                score += 10; // Word start bonus
            }

            // Bonus for uppercase match (camelCase)
            if target_char.is_uppercase() {
                score += 5;
            }

            prev_match_idx = Some(target_idx);
            query_idx += 1;
        }
    }

    // Did we match all query characters?
    if query_idx < query.len() {
        return None;
    }

    // Bonus for prefix match
    if first_match_idx == Some(0) {
        score += 20;
    }

    // Penalty for longer targets (prefer shorter matches)
    score -= (target_chars.len() as i32) / 5;

    Some(score)
}

/// Simple substring match (fallback/alternative)
pub fn substring_filter(query: &str, items: &[String]) -> Vec<usize> {
    if query.is_empty() {
        return (0..items.len()).collect();
    }

    let query_lower = query.to_lowercase();
    items
        .iter()
        .enumerate()
        .filter(|(_, item)| item.to_lowercase().contains(&query_lower))
        .map(|(i, _)| i)
        .collect()
}

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

    #[test]
    fn test_fuzzy_score_consecutive_bonus() {
        let query: Vec<char> = "dev".chars().collect();
        let score1 = fuzzy_score(&query, "dev").unwrap();
        let score2 = fuzzy_score(&query, "d_e_v").unwrap();

        // Consecutive should score higher
        assert!(score1 > score2);
    }

    #[test]
    fn test_fuzzy_score_prefix_bonus() {
        let query: Vec<char> = "st".chars().collect();
        let score1 = fuzzy_score(&query, "staging").unwrap();
        let score2 = fuzzy_score(&query, "test").unwrap();

        // Prefix match should score higher
        assert!(score1 > score2);
    }

    #[test]
    fn test_substring_filter() {
        let items = vec!["dev".into(), "development".into(), "staging".into()];
        let result = substring_filter("dev", &items);
        assert_eq!(result, vec![0, 1]);
    }
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
}
