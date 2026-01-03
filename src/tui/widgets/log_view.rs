//! Scrollable log view widget with rich formatting

use crate::core::{LogEntry, LogLevel, LogSource};
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
}

impl<'a> LogView<'a> {
    pub fn new(logs: &'a [LogEntry]) -> Self {
        Self {
            logs,
            title: " Logs ",
            show_timestamps: true,
            show_source: true,
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
    fn format_entry(&self, entry: &LogEntry) -> Line<'static> {
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

        // Message content with inline highlighting
        spans.push(Self::format_message(&entry.message, msg_style));

        Line::from(spans)
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
}

impl<'a> StatefulWidget for LogView<'a> {
    type State = LogViewState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // Handle empty state specially
        if self.logs.is_empty() {
            self.render_empty(area, buf);
            return;
        }

        // Create bordered block
        let block = Block::default()
            .title(self.title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));

        let inner = block.inner(area);
        block.render(area, buf);

        // Update state with current dimensions
        let visible_lines = inner.height as usize;
        state.update_content_size(self.logs.len(), visible_lines);

        // Get visible slice of logs
        let start = state.offset;
        let end = (start + visible_lines).min(self.logs.len());

        // Format visible entries
        let lines: Vec<Line> = self.logs[start..end]
            .iter()
            .map(|entry| self.format_entry(entry))
            .collect();

        // Render log content
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .render(inner, buf);

        // Render scrollbar if content exceeds visible area
        if self.logs.len() > visible_lines {
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("▲"))
                .end_symbol(Some("▼"))
                .track_symbol(Some("│"))
                .thumb_symbol("█");

            let mut scrollbar_state = ScrollbarState::new(self.logs.len()).position(state.offset);

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
        let line = view.format_entry(&logs[0]);

        // Should have multiple spans including timestamp
        assert!(line.spans.len() >= 3);
    }

    #[test]
    fn test_format_entry_no_timestamp() {
        let logs = vec![make_entry(LogLevel::Info, LogSource::App, "Test")];
        let view = LogView::new(&logs).show_timestamps(false);
        let line = view.format_entry(&logs[0]);

        // Fewer spans without timestamp
        let with_ts = LogView::new(&logs).show_timestamps(true);
        let line_with = with_ts.format_entry(&logs[0]);
        assert!(line.spans.len() < line_with.spans.len());
    }

    #[test]
    fn test_format_entry_no_source() {
        let logs = vec![make_entry(LogLevel::Info, LogSource::App, "Test")];
        let view = LogView::new(&logs).show_source(false);
        let line = view.format_entry(&logs[0]);

        // Fewer spans without source
        let with_src = LogView::new(&logs).show_source(true);
        let line_with = with_src.format_entry(&logs[0]);
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
}
