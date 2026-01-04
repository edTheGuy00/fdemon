//! Scrollable log view widget with rich formatting

use crate::core::{
    FilterState, LogEntry, LogLevel, LogLevelFilter, LogSource, LogSourceFilter, SearchState,
};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget,
        Widget, Wrap,
    },
};

/// State for log view scrolling
#[derive(Debug, Default)]
pub struct LogViewState {
    /// Current scroll offset from top
    pub offset: usize,
    /// Whether auto-scroll is enabled (follow new content)
    pub auto_scroll: bool,
    /// Total number of lines (set during render)
    pub total_lines: usize,
    /// Visible lines (set during render)
    pub visible_lines: usize,
}

impl LogViewState {
    pub fn new() -> Self {
        Self {
            offset: 0,
            auto_scroll: true,
            total_lines: 0,
            visible_lines: 0,
        }
    }

    /// Scroll up by n lines
    pub fn scroll_up(&mut self, n: usize) {
        self.offset = self.offset.saturating_sub(n);
        self.auto_scroll = false;
    }

    /// Scroll down by n lines
    pub fn scroll_down(&mut self, n: usize) {
        let max_offset = self.total_lines.saturating_sub(self.visible_lines);
        self.offset = (self.offset + n).min(max_offset);

        // Re-enable auto-scroll if at bottom
        if self.offset >= max_offset {
            self.auto_scroll = true;
        }
    }

    /// Scroll to top
    pub fn scroll_to_top(&mut self) {
        self.offset = 0;
        self.auto_scroll = false;
    }

    /// Scroll to bottom and enable auto-scroll
    pub fn scroll_to_bottom(&mut self) {
        self.offset = self.total_lines.saturating_sub(self.visible_lines);
        self.auto_scroll = true;
    }

    /// Page up
    pub fn page_up(&mut self) {
        let page = self.visible_lines.saturating_sub(2);
        self.scroll_up(page);
    }

    /// Page down
    pub fn page_down(&mut self) {
        let page = self.visible_lines.saturating_sub(2);
        self.scroll_down(page);
    }

    /// Update with new content size
    pub fn update_content_size(&mut self, total: usize, visible: usize) {
        self.total_lines = total;
        self.visible_lines = visible;

        // Auto-scroll if enabled
        if self.auto_scroll && total > visible {
            self.offset = total.saturating_sub(visible);
        }
    }
}

/// Log view widget with rich formatting
pub struct LogView<'a> {
    logs: &'a [LogEntry],
    title: &'a str,
    show_timestamps: bool,
    show_source: bool,
    /// Filter state for displaying indicator and filtering logs
    filter_state: Option<&'a FilterState>,
    /// Search state for highlighting matches
    search_state: Option<&'a SearchState>,
}

impl<'a> LogView<'a> {
    pub fn new(logs: &'a [LogEntry]) -> Self {
        Self {
            logs,
            title: " Logs ",
            show_timestamps: true,
            show_source: true,
            filter_state: None,
            search_state: None,
        }
    }

    pub fn title(mut self, title: &'a str) -> Self {
        self.title = title;
        self
    }

    pub fn show_timestamps(mut self, show: bool) -> Self {
        self.show_timestamps = show;
        self
    }

    pub fn show_source(mut self, show: bool) -> Self {
        self.show_source = show;
        self
    }

    /// Set the filter state for filtering and indicator display
    pub fn filter_state(mut self, state: &'a FilterState) -> Self {
        self.filter_state = Some(state);
        self
    }

    /// Set the search state for match highlighting
    pub fn search_state(mut self, state: &'a SearchState) -> Self {
        self.search_state = Some(state);
        self
    }

    /// Get style for log level - returns (level_style, message_style)
    fn level_style(level: LogLevel) -> (Style, Style) {
        match level {
            LogLevel::Error => (
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                Style::default().fg(Color::LightRed),
            ),
            LogLevel::Warning => (
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
                Style::default().fg(Color::Yellow),
            ),
            LogLevel::Info => (
                Style::default().fg(Color::Green),
                Style::default().fg(Color::White), // Brighter for better readability
            ),
            LogLevel::Debug => (
                Style::default().fg(Color::DarkGray),
                Style::default().fg(Color::DarkGray),
            ),
        }
    }

    /// Get icon for log level
    fn level_icon(level: LogLevel) -> &'static str {
        match level {
            LogLevel::Error => "✗",
            LogLevel::Warning => "⚠",
            LogLevel::Info => "•",
            LogLevel::Debug => "·",
        }
    }

    /// Format message with inline highlighting for special content
    fn format_message(message: &str, base_style: Style) -> Span<'static> {
        // Highlight reload success
        if message.contains("Reloaded") || message.contains("reloaded") {
            Span::styled(message.to_string(), base_style.fg(Color::Green))
        } else if message.contains("Exception") || message.contains("Error") {
            // Highlight exceptions
            Span::styled(message.to_string(), base_style.fg(Color::LightRed))
        } else if message.starts_with("    ") {
            // Stack trace lines (indented)
            Span::styled(message.to_string(), Style::default().fg(Color::DarkGray))
        } else {
            Span::styled(message.to_string(), base_style)
        }
    }

    /// Get style for log source
    fn source_style(source: LogSource) -> Style {
        match source {
            LogSource::App => Style::default().fg(Color::Magenta),
            LogSource::Daemon => Style::default().fg(Color::Yellow),
            LogSource::Flutter => Style::default().fg(Color::Blue),
            LogSource::FlutterError => Style::default().fg(Color::Red),
            LogSource::Watcher => Style::default().fg(Color::Cyan),
        }
    }

    /// Format a single log entry as a styled Line with icons
    fn format_entry(&self, entry: &LogEntry, entry_index: usize) -> Line<'static> {
        let (level_style, msg_style) = Self::level_style(entry.level);
        let source_style = Self::source_style(entry.source);

        let mut spans = Vec::with_capacity(8);

        // Timestamp: "HH:MM:SS "
        if self.show_timestamps {
            spans.push(Span::styled(
                entry.formatted_time(),
                Style::default().fg(Color::DarkGray),
            ));
            spans.push(Span::raw(" "));
        }

        // Level indicator with icon: "✗ " or "• " etc.
        spans.push(Span::styled(
            format!("{} ", Self::level_icon(entry.level)),
            level_style,
        ));

        // Source: "[flutter] " or "[app] "
        if self.show_source {
            spans.push(Span::styled(
                format!("[{}] ", entry.source.prefix()),
                source_style,
            ));
        }

        // Message content with search highlighting
        let message_spans =
            self.format_message_with_highlights(&entry.message, entry_index, msg_style);
        spans.extend(message_spans);

        Line::from(spans)
    }

    /// Format message text with search match highlighting
    fn format_message_with_highlights(
        &self,
        message: &str,
        entry_index: usize,
        base_style: Style,
    ) -> Vec<Span<'static>> {
        let Some(search) = self.search_state else {
            // No search active, return plain message
            return vec![Self::format_message(message, base_style)];
        };

        if search.query.is_empty() || !search.is_valid {
            return vec![Self::format_message(message, base_style)];
        }

        // Get matches for this entry
        let matches = search.matches_for_entry(entry_index);
        if matches.is_empty() {
            return vec![Self::format_message(message, base_style)];
        }

        // Build spans with highlighted regions
        let mut spans = Vec::new();
        let mut last_end = 0;

        // Highlight styles
        let highlight_style = Style::default()
            .bg(Color::Yellow)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD);
        let current_highlight_style = Style::default()
            .bg(Color::LightYellow)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED);

        for mat in matches {
            // Add text before match
            if mat.start > last_end {
                let before = &message[last_end..mat.start];
                spans.push(Span::styled(before.to_string(), base_style));
            }

            // Add highlighted match
            let matched_text = &message[mat.start..mat.end];
            let style = if search.is_current_match(mat) {
                current_highlight_style
            } else {
                highlight_style
            };
            spans.push(Span::styled(matched_text.to_string(), style));

            last_end = mat.end;
        }

        // Add remaining text after last match
        if last_end < message.len() {
            let after = &message[last_end..];
            spans.push(Span::styled(after.to_string(), base_style));
        }

        spans
    }

    /// Render empty state with centered message
    fn render_empty(&self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .title(self.title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));

        let inner = block.inner(area);
        block.render(area, buf);

        // Center the waiting message
        let waiting_text = vec![
            Line::from(""),
            Line::from(Span::styled(
                "Waiting for Flutter...",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Make sure you're in a Flutter project directory",
                Style::default().fg(Color::DarkGray),
            )),
        ];

        Paragraph::new(waiting_text)
            .alignment(ratatui::layout::Alignment::Center)
            .render(inner, buf);
    }

    /// Generate the title string including filter and search indicators
    fn build_title(&self) -> String {
        let base = self.title.trim();
        let mut parts = Vec::new();

        // Add filter indicators
        if let Some(filter) = self.filter_state {
            if filter.is_active() {
                let mut indicators = Vec::new();
                if filter.level_filter != LogLevelFilter::All {
                    indicators.push(filter.level_filter.display_name());
                }
                if filter.source_filter != LogSourceFilter::All {
                    indicators.push(filter.source_filter.display_name());
                }
                if !indicators.is_empty() {
                    parts.push(indicators.join(" | "));
                }
            }
        }

        // Add search status
        if let Some(search) = self.search_state {
            if !search.query.is_empty() {
                let status = search.display_status();
                if !status.is_empty() {
                    parts.push(status);
                }
            }
        }

        if parts.is_empty() {
            format!(" {} ", base)
        } else {
            format!(" {} {} ", base, parts.join(" • "))
        }
    }

    /// Render empty filtered state
    fn render_no_matches(&self, area: Rect, buf: &mut Buffer) {
        let title = self.build_title();
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));

        let inner = block.inner(area);
        block.render(area, buf);

        let message = vec![
            Line::from(""),
            Line::from(Span::styled(
                "No logs match current filter",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::ITALIC),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Press Ctrl+f to reset filters",
                Style::default().fg(Color::DarkGray),
            )),
        ];

        Paragraph::new(message)
            .alignment(ratatui::layout::Alignment::Center)
            .render(inner, buf);
    }
}

impl<'a> StatefulWidget for LogView<'a> {
    type State = LogViewState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // Handle empty state specially
        if self.logs.is_empty() {
            self.render_empty(area, buf);
            return;
        }

        // Apply filter to get visible log indices
        let filtered_indices: Vec<usize> = if let Some(filter) = self.filter_state {
            self.logs
                .iter()
                .enumerate()
                .filter(|(_, entry)| filter.matches(entry))
                .map(|(i, _)| i)
                .collect()
        } else {
            (0..self.logs.len()).collect()
        };

        // Handle empty filtered state
        if filtered_indices.is_empty() {
            self.render_no_matches(area, buf);
            return;
        }

        // Create bordered block with dynamic title
        let title = self.build_title();
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));

        let inner = block.inner(area);
        block.render(area, buf);

        // Update state with filtered content dimensions
        let visible_lines = inner.height as usize;
        state.update_content_size(filtered_indices.len(), visible_lines);

        // Get visible slice of filtered logs
        let start = state.offset;
        let end = (start + visible_lines).min(filtered_indices.len());

        // Format visible entries using original log indices
        let lines: Vec<Line> = filtered_indices[start..end]
            .iter()
            .map(|&idx| self.format_entry(&self.logs[idx], idx))
            .collect();

        // Render log content
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .render(inner, buf);

        // Render scrollbar if content exceeds visible area
        if filtered_indices.len() > visible_lines {
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("▲"))
                .end_symbol(Some("▼"))
                .track_symbol(Some("│"))
                .thumb_symbol("█");

            let mut scrollbar_state =
                ScrollbarState::new(filtered_indices.len()).position(state.offset);

            scrollbar.render(area, buf, &mut scrollbar_state);
        }
    }
}

// Non-stateful version for simple rendering
impl Widget for LogView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut state = LogViewState::new();
        StatefulWidget::render(self, area, buf, &mut state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Local;

    fn make_entry(level: LogLevel, source: LogSource, msg: &str) -> LogEntry {
        LogEntry {
            timestamp: Local::now(),
            level,
            source,
            message: msg.to_string(),
        }
    }

    #[test]
    fn test_log_view_state_default() {
        let state = LogViewState::new();
        assert_eq!(state.offset, 0);
        assert!(state.auto_scroll);
    }

    #[test]
    fn test_scroll_up_disables_auto_scroll() {
        let mut state = LogViewState::new();
        state.total_lines = 100;
        state.visible_lines = 20;
        state.offset = 50;

        state.scroll_up(1);

        assert_eq!(state.offset, 49);
        assert!(!state.auto_scroll);
    }

    #[test]
    fn test_scroll_to_bottom_enables_auto_scroll() {
        let mut state = LogViewState::new();
        state.total_lines = 100;
        state.visible_lines = 20;
        state.auto_scroll = false;

        state.scroll_to_bottom();

        assert_eq!(state.offset, 80);
        assert!(state.auto_scroll);
    }

    #[test]
    fn test_scroll_up_at_top() {
        let mut state = LogViewState::new();
        state.offset = 0;

        state.scroll_up(5);

        assert_eq!(state.offset, 0);
    }

    #[test]
    fn test_update_content_size_auto_scrolls() {
        let mut state = LogViewState::new();
        state.auto_scroll = true;

        state.update_content_size(100, 20);

        assert_eq!(state.offset, 80);
    }

    #[test]
    fn test_page_up_down() {
        let mut state = LogViewState::new();
        state.total_lines = 100;
        state.visible_lines = 20;
        state.offset = 50;

        state.page_down();
        assert_eq!(state.offset, 68); // 50 + 18

        state.page_up();
        assert_eq!(state.offset, 50); // 68 - 18
    }

    #[test]
    fn test_format_entry_includes_timestamp() {
        let logs = vec![make_entry(LogLevel::Info, LogSource::App, "Test")];
        let view = LogView::new(&logs).show_timestamps(true);
        let line = view.format_entry(&logs[0], 0);

        // Should have multiple spans including timestamp
        assert!(line.spans.len() >= 3);
    }

    #[test]
    fn test_format_entry_no_timestamp() {
        let logs = vec![make_entry(LogLevel::Info, LogSource::App, "Test")];
        let view = LogView::new(&logs).show_timestamps(false);
        let line = view.format_entry(&logs[0], 0);

        // Fewer spans without timestamp
        let with_ts = LogView::new(&logs).show_timestamps(true);
        let line_with = with_ts.format_entry(&logs[0], 0);
        assert!(line.spans.len() < line_with.spans.len());
    }

    #[test]
    fn test_format_entry_no_source() {
        let logs = vec![make_entry(LogLevel::Info, LogSource::App, "Test")];
        let view = LogView::new(&logs).show_source(false);
        let line = view.format_entry(&logs[0], 0);

        // Fewer spans without source
        let with_src = LogView::new(&logs).show_source(true);
        let line_with = with_src.format_entry(&logs[0], 0);
        assert!(line.spans.len() < line_with.spans.len());
    }

    #[test]
    fn test_level_styles_are_distinct() {
        let (err_level, _) = LogView::level_style(LogLevel::Error);
        let (info_level, _) = LogView::level_style(LogLevel::Info);

        // Error should be red, Info should be green
        assert_ne!(err_level.fg, info_level.fg);
    }

    #[test]
    fn test_source_styles_are_distinct() {
        let app_style = LogView::source_style(LogSource::App);
        let flutter_style = LogView::source_style(LogSource::Flutter);

        assert_ne!(app_style.fg, flutter_style.fg);
    }

    #[test]
    fn test_warning_has_bold_modifier() {
        let (warn_level, _) = LogView::level_style(LogLevel::Warning);
        assert!(warn_level.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn test_error_has_bold_modifier() {
        let (err_level, _) = LogView::level_style(LogLevel::Error);
        assert!(err_level.add_modifier.contains(Modifier::BOLD));
    }

    // ─────────────────────────────────────────────────────────
    // Filter Tests (Phase 1 - Task 4)
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_build_title_no_filter() {
        let logs = vec![make_entry(LogLevel::Info, LogSource::App, "Test")];
        let view = LogView::new(&logs).title("Logs");
        assert_eq!(view.build_title(), " Logs ");
    }

    #[test]
    fn test_build_title_with_default_filter() {
        let logs = vec![make_entry(LogLevel::Info, LogSource::App, "Test")];
        let filter = FilterState::default();
        let view = LogView::new(&logs).title("Logs").filter_state(&filter);
        // Default filter (All/All) should not show indicator
        assert_eq!(view.build_title(), " Logs ");
    }

    #[test]
    fn test_build_title_with_level_filter() {
        let logs = vec![make_entry(LogLevel::Info, LogSource::App, "Test")];
        let filter = FilterState {
            level_filter: LogLevelFilter::Errors,
            source_filter: LogSourceFilter::All,
        };
        let view = LogView::new(&logs).title("Logs").filter_state(&filter);
        let title = view.build_title();
        assert!(title.contains("Errors only"), "Title was: {}", title);
    }

    #[test]
    fn test_build_title_with_source_filter() {
        let logs = vec![make_entry(LogLevel::Info, LogSource::App, "Test")];
        let filter = FilterState {
            level_filter: LogLevelFilter::All,
            source_filter: LogSourceFilter::App,
        };
        let view = LogView::new(&logs).title("Logs").filter_state(&filter);
        let title = view.build_title();
        assert!(title.contains("App logs"), "Title was: {}", title);
    }

    #[test]
    fn test_build_title_with_combined_filter() {
        let logs = vec![make_entry(LogLevel::Info, LogSource::App, "Test")];
        let filter = FilterState {
            level_filter: LogLevelFilter::Errors,
            source_filter: LogSourceFilter::Flutter,
        };
        let view = LogView::new(&logs).title("Logs").filter_state(&filter);
        let title = view.build_title();
        assert!(title.contains("Errors only"), "Title was: {}", title);
        assert!(title.contains("Flutter logs"), "Title was: {}", title);
        assert!(title.contains(" | "), "Title was: {}", title);
    }

    #[test]
    fn test_filter_state_builder() {
        let logs = vec![make_entry(LogLevel::Info, LogSource::App, "Test")];
        let filter = FilterState::default();
        let view = LogView::new(&logs).filter_state(&filter);
        assert!(view.filter_state.is_some());
    }

    #[test]
    fn test_filtered_logs_count() {
        let logs = vec![
            make_entry(LogLevel::Info, LogSource::App, "info"),
            make_entry(LogLevel::Error, LogSource::App, "error"),
            make_entry(LogLevel::Warning, LogSource::Daemon, "warning"),
        ];
        let filter = FilterState {
            level_filter: LogLevelFilter::Errors,
            source_filter: LogSourceFilter::All,
        };

        let filtered: Vec<_> = logs.iter().filter(|e| filter.matches(e)).collect();

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].level, LogLevel::Error);
    }

    #[test]
    fn test_filtered_logs_by_source() {
        let logs = vec![
            make_entry(LogLevel::Info, LogSource::App, "app info"),
            make_entry(LogLevel::Error, LogSource::Flutter, "flutter error"),
            make_entry(LogLevel::Warning, LogSource::Daemon, "daemon warning"),
        ];
        let filter = FilterState {
            level_filter: LogLevelFilter::All,
            source_filter: LogSourceFilter::App,
        };

        let filtered: Vec<_> = logs.iter().filter(|e| filter.matches(e)).collect();

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].source, LogSource::App);
    }

    #[test]
    fn test_combined_filter() {
        let logs = vec![
            make_entry(LogLevel::Error, LogSource::App, "app error"),
            make_entry(LogLevel::Error, LogSource::Flutter, "flutter error"),
            make_entry(LogLevel::Info, LogSource::App, "app info"),
            make_entry(LogLevel::Warning, LogSource::App, "app warning"),
        ];
        let filter = FilterState {
            level_filter: LogLevelFilter::Errors,
            source_filter: LogSourceFilter::App,
        };

        let filtered: Vec<_> = logs.iter().filter(|e| filter.matches(e)).collect();

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].message, "app error");
    }

    // ─────────────────────────────────────────────────────────
    // Search Highlighting Tests (Phase 1 - Task 6)
    // ─────────────────────────────────────────────────────────

    use crate::core::SearchState;

    #[test]
    fn test_format_message_with_highlights_no_search() {
        let logs = vec![make_entry(LogLevel::Info, LogSource::App, "Hello world")];
        let view = LogView::new(&logs);

        let spans = view.format_message_with_highlights("Hello world", 0, Style::default());

        assert_eq!(spans.len(), 1);
    }

    #[test]
    fn test_format_message_with_highlights_with_match() {
        let logs = vec![make_entry(LogLevel::Info, LogSource::App, "Hello world")];
        let mut search = SearchState::default();
        search.set_query("world");
        search.execute_search(&logs);

        let view = LogView::new(&logs).search_state(&search);

        let spans = view.format_message_with_highlights("Hello world", 0, Style::default());

        // Should be: "Hello " + "world" (highlighted)
        assert_eq!(spans.len(), 2);
    }

    #[test]
    fn test_format_message_with_highlights_multiple_matches() {
        let logs = vec![make_entry(
            LogLevel::Info,
            LogSource::App,
            "test one test two",
        )];
        let mut search = SearchState::default();
        search.set_query("test");
        search.execute_search(&logs);

        let view = LogView::new(&logs).search_state(&search);

        let spans = view.format_message_with_highlights("test one test two", 0, Style::default());

        // Should be: "test" (highlighted) + " one " + "test" (highlighted) + " two"
        assert_eq!(spans.len(), 4);
    }

    #[test]
    fn test_format_message_with_highlights_no_match_in_entry() {
        let logs = vec![
            make_entry(LogLevel::Info, LogSource::App, "test here"),
            make_entry(LogLevel::Info, LogSource::App, "no match"),
        ];
        let mut search = SearchState::default();
        search.set_query("test");
        search.execute_search(&logs);

        let view = LogView::new(&logs).search_state(&search);

        // Entry 1 has no matches - should return single span
        let spans = view.format_message_with_highlights("no match", 1, Style::default());

        assert_eq!(spans.len(), 1);
    }

    #[test]
    fn test_format_message_with_highlights_invalid_regex() {
        let logs = vec![make_entry(LogLevel::Info, LogSource::App, "test")];
        let mut search = SearchState::default();
        search.set_query("[invalid");
        search.execute_search(&logs);

        let view = LogView::new(&logs).search_state(&search);

        // Invalid regex should not highlight
        let spans = view.format_message_with_highlights("test", 0, Style::default());

        assert_eq!(spans.len(), 1);
    }

    #[test]
    fn test_build_title_with_search_status() {
        let logs = vec![
            make_entry(LogLevel::Info, LogSource::App, "test message"),
            make_entry(LogLevel::Info, LogSource::App, "another test"),
        ];
        let mut search = SearchState::default();
        search.set_query("test");
        search.execute_search(&logs);

        let view = LogView::new(&logs).title("Logs").search_state(&search);

        let title = view.build_title();
        assert!(title.contains("["), "Title was: {}", title);
        assert!(title.contains("2"), "Title was: {}", title);
        assert!(title.contains("matches"), "Title was: {}", title);
    }

    #[test]
    fn test_build_title_with_filter_and_search() {
        let logs = vec![make_entry(LogLevel::Info, LogSource::App, "test")];
        let filter = FilterState {
            level_filter: LogLevelFilter::Errors,
            source_filter: LogSourceFilter::All,
        };
        let mut search = SearchState::default();
        search.set_query("test");
        search.execute_search(&logs);

        let view = LogView::new(&logs)
            .title("Logs")
            .filter_state(&filter)
            .search_state(&search);

        let title = view.build_title();
        // Should contain both filter and search indicators
        assert!(title.contains("Errors"), "Title was: {}", title);
        assert!(title.contains("•"), "Title was: {}", title); // separator
    }

    #[test]
    fn test_search_state_builder() {
        let logs = vec![make_entry(LogLevel::Info, LogSource::App, "test")];
        let search = SearchState::default();
        let view = LogView::new(&logs).search_state(&search);
        assert!(view.search_state.is_some());
    }

    #[test]
    fn test_format_entry_with_search_highlights() {
        let logs = vec![make_entry(LogLevel::Info, LogSource::App, "error occurred")];
        let mut search = SearchState::default();
        search.set_query("error");
        search.execute_search(&logs);

        let view = LogView::new(&logs)
            .show_timestamps(false)
            .show_source(false)
            .search_state(&search);

        let line = view.format_entry(&logs[0], 0);

        // Should have at least 2 spans for message: "error" (highlighted) + " occurred"
        // Plus the level indicator span
        assert!(line.spans.len() >= 3, "Got {} spans", line.spans.len());
    }
}
