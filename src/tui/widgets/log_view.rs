//! Scrollable log view widget with rich formatting

use std::collections::VecDeque;

use crate::core::{
    FilterState, LogEntry, LogLevel, LogLevelFilter, LogSource, LogSourceFilter, SearchState,
    StackFrame,
};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget,
        Widget,
    },
};

/// Stack trace styling constants
mod stack_trace_styles {
    use ratatui::style::{Color, Modifier, Style};

    /// Frame number (#0, #1, etc.)
    pub const FRAME_NUMBER: Style = Style::new().fg(Color::DarkGray);

    /// Function name for project frames
    pub const FUNCTION_PROJECT: Style = Style::new().fg(Color::White);

    /// Function name for package frames
    pub const FUNCTION_PACKAGE: Style = Style::new().fg(Color::DarkGray);

    /// File path for project frames (clickable in Phase 3)
    pub const FILE_PROJECT: Style = Style::new()
        .fg(Color::Blue)
        .add_modifier(Modifier::UNDERLINED);

    /// File path for package frames
    pub const FILE_PACKAGE: Style = Style::new().fg(Color::DarkGray);

    /// Line/column numbers for project frames
    pub const LOCATION_PROJECT: Style = Style::new().fg(Color::Cyan);

    /// Line/column numbers for package frames
    pub const LOCATION_PACKAGE: Style = Style::new().fg(Color::DarkGray);

    /// Async suspension marker
    pub const ASYNC_GAP: Style = Style::new()
        .fg(Color::DarkGray)
        .add_modifier(Modifier::ITALIC);

    /// Punctuation (parentheses, colons)
    pub const PUNCTUATION: Style = Style::new().fg(Color::DarkGray);

    /// Indentation for stack frames
    pub const INDENT: &str = "    ";
}

/// Default buffer lines for virtualized rendering
const DEFAULT_BUFFER_LINES: usize = 10;

/// State for log view scrolling with virtualization support
#[derive(Debug)]
pub struct LogViewState {
    /// Current vertical scroll offset from top
    pub offset: usize,
    /// Current horizontal scroll offset from left
    pub h_offset: usize,
    /// Whether auto-scroll is enabled (follow new content)
    pub auto_scroll: bool,
    /// Total number of lines (set during render)
    pub total_lines: usize,
    /// Visible lines (set during render)
    pub visible_lines: usize,
    /// Maximum line width in current view (for h-scroll bounds)
    pub max_line_width: usize,
    /// Visible width (set during render)
    pub visible_width: usize,
    /// Buffer lines above/below viewport for smooth scrolling (Task 05)
    pub buffer_lines: usize,
}

impl Default for LogViewState {
    fn default() -> Self {
        Self::new()
    }
}

impl LogViewState {
    pub fn new() -> Self {
        Self {
            offset: 0,
            h_offset: 0,
            auto_scroll: true,
            total_lines: 0,
            visible_lines: 0,
            max_line_width: 0,
            visible_width: 0,
            buffer_lines: DEFAULT_BUFFER_LINES,
        }
    }

    /// Get the range of line indices to render (with buffer)
    ///
    /// Returns (start, end) where start is inclusive and end is exclusive.
    /// Includes buffer_lines above and below the visible area for smooth scrolling.
    pub fn visible_range(&self) -> (usize, usize) {
        let start = self.offset.saturating_sub(self.buffer_lines);
        let end = (self.offset + self.visible_lines + self.buffer_lines).min(self.total_lines);
        (start, end)
    }

    /// Set buffer lines for virtualized rendering
    pub fn set_buffer_lines(&mut self, buffer: usize) {
        self.buffer_lines = buffer;
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

    /// Scroll left by n columns
    pub fn scroll_left(&mut self, n: usize) {
        self.h_offset = self.h_offset.saturating_sub(n);
    }

    /// Scroll right by n columns
    pub fn scroll_right(&mut self, n: usize) {
        let max_h_offset = self.max_line_width.saturating_sub(self.visible_width);
        self.h_offset = (self.h_offset + n).min(max_h_offset);
    }

    /// Scroll to start of line (column 0)
    pub fn scroll_to_line_start(&mut self) {
        self.h_offset = 0;
    }

    /// Scroll to end of line
    pub fn scroll_to_line_end(&mut self) {
        let max_h_offset = self.max_line_width.saturating_sub(self.visible_width);
        self.h_offset = max_h_offset;
    }

    /// Update horizontal content dimensions
    pub fn update_horizontal_size(&mut self, max_width: usize, visible_width: usize) {
        self.max_line_width = max_width;
        self.visible_width = visible_width;

        // Clamp h_offset if content shrank
        let max_h_offset = max_width.saturating_sub(visible_width);
        if self.h_offset > max_h_offset {
            self.h_offset = max_h_offset;
        }
    }

    /// Calculate total lines including expanded stack traces
    pub fn calculate_total_lines(logs: &VecDeque<LogEntry>) -> usize {
        logs.iter()
            .map(|entry| 1 + entry.stack_trace_frame_count()) // 1 for message + frames
            .sum()
    }

    /// Calculate total lines for filtered entries (by index)
    pub fn calculate_total_lines_filtered(logs: &VecDeque<LogEntry>, indices: &[usize]) -> usize {
        indices
            .iter()
            .map(|&idx| 1 + logs[idx].stack_trace_frame_count())
            .sum()
    }
}

/// Log view widget with rich formatting
pub struct LogView<'a> {
    logs: &'a VecDeque<LogEntry>,
    title: &'a str,
    show_timestamps: bool,
    show_source: bool,
    /// Filter state for displaying indicator and filtering logs
    filter_state: Option<&'a FilterState>,
    /// Search state for highlighting matches
    search_state: Option<&'a SearchState>,
    /// Collapse state for stack traces (Phase 2 Task 6)
    collapse_state: Option<&'a crate::app::session::CollapseState>,
    /// Whether stack traces are collapsed by default
    default_collapsed: bool,
    /// Maximum frames to show when collapsed
    max_collapsed_frames: usize,
}

impl<'a> LogView<'a> {
    pub fn new(logs: &'a VecDeque<LogEntry>) -> Self {
        Self {
            logs,
            title: " Logs ",
            show_timestamps: true,
            show_source: true,
            filter_state: None,
            search_state: None,
            collapse_state: None,
            default_collapsed: true,
            max_collapsed_frames: 3,
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

    /// Set the collapse state for stack traces
    pub fn collapse_state(mut self, state: &'a crate::app::session::CollapseState) -> Self {
        self.collapse_state = Some(state);
        self
    }

    /// Set whether stack traces are collapsed by default
    pub fn default_collapsed(mut self, collapsed: bool) -> Self {
        self.default_collapsed = collapsed;
        self
    }

    /// Set maximum frames to show when collapsed
    pub fn max_collapsed_frames(mut self, max: usize) -> Self {
        self.max_collapsed_frames = max;
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

    /// Format a single stack frame into styled spans
    fn format_stack_frame(frame: &StackFrame) -> Vec<Span<'static>> {
        use stack_trace_styles::*;

        // Handle async gap specially
        if frame.is_async_gap {
            return vec![
                Span::styled(INDENT.to_string(), Style::default()),
                Span::styled("<asynchronous suspension>".to_string(), ASYNC_GAP),
            ];
        }

        // Determine styles based on frame type (package vs project)
        let (func_style, file_style, loc_style) = if frame.is_package_frame {
            // Package frame - all dimmed
            (FUNCTION_PACKAGE, FILE_PACKAGE, LOCATION_PACKAGE)
        } else {
            // Project frame - highlighted
            (FUNCTION_PROJECT, FILE_PROJECT, LOCATION_PROJECT)
        };

        let mut spans = Vec::with_capacity(10);

        // Indentation
        spans.push(Span::styled(INDENT.to_string(), Style::default()));

        // Frame number: #0, #1, etc.
        spans.push(Span::styled(
            format!("#{:<3}", frame.frame_number),
            FRAME_NUMBER,
        ));

        // Function name
        spans.push(Span::styled(
            format!("{} ", frame.function_name.clone()),
            func_style,
        ));

        // Opening paren
        spans.push(Span::styled("(".to_string(), PUNCTUATION));

        // File path (short version)
        spans.push(Span::styled(frame.short_path().to_string(), file_style));

        // Colon separator
        spans.push(Span::styled(":".to_string(), PUNCTUATION));

        // Line number
        spans.push(Span::styled(frame.line.to_string(), loc_style));

        // Column (if present)
        if frame.column > 0 {
            spans.push(Span::styled(format!(":{}", frame.column), loc_style));
        }

        // Closing paren
        spans.push(Span::styled(")".to_string(), PUNCTUATION));

        spans
    }

    /// Format a stack frame as a Line for rendering
    fn format_stack_frame_line(frame: &StackFrame) -> Line<'static> {
        Line::from(Self::format_stack_frame(frame))
    }

    /// Format collapsed indicator: "▶ N more frames..."
    fn format_collapsed_indicator(hidden_count: usize) -> Line<'static> {
        use stack_trace_styles::*;

        let text = if hidden_count == 1 {
            "1 more frame...".to_string()
        } else {
            format!("{} more frames...", hidden_count)
        };

        Line::from(vec![
            Span::styled(INDENT.to_string(), Style::default()),
            Span::styled("▶ ".to_string(), Style::default().fg(Color::Yellow)),
            Span::styled(
                text,
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            ),
        ])
    }

    /// Check if an entry's stack trace should be expanded
    fn is_entry_expanded(&self, entry: &LogEntry) -> bool {
        if let Some(collapse_state) = self.collapse_state {
            collapse_state.is_expanded(entry.id, self.default_collapsed)
        } else {
            // No collapse state means always expanded (legacy behavior)
            !self.default_collapsed
        }
    }

    /// Calculate lines for a single entry accounting for collapse state
    fn calculate_entry_lines(&self, entry: &LogEntry) -> usize {
        let frame_count = entry.stack_trace_frame_count();
        if frame_count == 0 {
            return 1; // Just the message line
        }

        let is_expanded = self.is_entry_expanded(entry);
        if is_expanded {
            // Expanded: message + all frames
            1 + frame_count
        } else {
            // Collapsed: message + visible frames + indicator (if more)
            let visible = self.max_collapsed_frames.min(frame_count);
            let has_more = frame_count > self.max_collapsed_frames;
            1 + visible + if has_more { 1 } else { 0 }
        }
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

    /// Calculate the display width of a Line (sum of span content widths)
    fn line_width(line: &Line) -> usize {
        line.spans.iter().map(|s| s.content.chars().count()).sum()
    }

    /// Apply horizontal scroll offset to a line, truncating and adding indicators
    fn apply_horizontal_scroll(
        line: Line<'static>,
        h_offset: usize,
        visible_width: usize,
    ) -> Line<'static> {
        let line_width = Self::line_width(&line);

        // No scrolling needed if line fits
        if h_offset == 0 && line_width <= visible_width {
            return line;
        }

        // Build a flat list of (char, style) pairs
        let mut chars: Vec<(char, Style)> = Vec::with_capacity(line_width);
        for span in &line.spans {
            let style = span.style;
            for c in span.content.chars() {
                chars.push((c, style));
            }
        }

        // If offset is beyond content, return empty line
        if h_offset >= chars.len() {
            return Line::from("");
        }

        // Determine visible range
        let visible_start = h_offset;
        let visible_end = (h_offset + visible_width).min(chars.len());
        let has_more_left = h_offset > 0;
        let has_more_right = visible_end < chars.len();

        // Reserve space for indicators
        let indicator_left_space = if has_more_left { 1 } else { 0 };
        let indicator_right_space = if has_more_right { 1 } else { 0 };
        let content_width = visible_width
            .saturating_sub(indicator_left_space)
            .saturating_sub(indicator_right_space);

        // Adjust the visible range for content (leave room for indicators)
        let content_start = visible_start + indicator_left_space;
        let content_end = (content_start + content_width).min(chars.len());

        // Build spans from visible characters
        let mut spans: Vec<Span<'static>> = Vec::new();

        // Add left indicator if needed
        if has_more_left {
            spans.push(Span::styled(
                "←".to_string(),
                Style::default().fg(Color::DarkGray),
            ));
        }

        // Group consecutive chars with same style into spans
        if content_start < content_end {
            let mut current_style = chars[content_start].1;
            let mut current_text = String::new();

            for &(c, style) in &chars[content_start..content_end] {
                if style == current_style {
                    current_text.push(c);
                } else {
                    if !current_text.is_empty() {
                        spans.push(Span::styled(current_text, current_style));
                    }
                    current_text = String::from(c);
                    current_style = style;
                }
            }
            if !current_text.is_empty() {
                spans.push(Span::styled(current_text, current_style));
            }
        }

        // Add right indicator if needed
        if has_more_right {
            spans.push(Span::styled(
                "→".to_string(),
                Style::default().fg(Color::DarkGray),
            ));
        }

        Line::from(spans)
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

        // Calculate total lines including stack traces (accounting for collapse state)
        let total_lines: usize = filtered_indices
            .iter()
            .map(|&idx| self.calculate_entry_lines(&self.logs[idx]))
            .sum();
        let visible_lines = inner.height as usize;

        // Update state with content dimensions (now using total lines, not entry count)
        state.update_content_size(total_lines, visible_lines);

        // Build a flat list of all lines (entry messages + stack frames)
        // We need to skip `offset` lines and take `visible_lines` lines
        let mut all_lines: Vec<Line> = Vec::new();
        let mut lines_added = 0;
        let mut lines_skipped = 0;

        for &idx in &filtered_indices {
            let entry = &self.logs[idx];
            let entry_line_count = self.calculate_entry_lines(entry);

            // Skip entries that are entirely before the offset
            if lines_skipped + entry_line_count <= state.offset {
                lines_skipped += entry_line_count;
                continue;
            }

            // Check if we've added enough lines
            if lines_added >= visible_lines {
                break;
            }

            // Determine how many lines of this entry to skip (partial entry at start)
            let skip_in_entry = state.offset.saturating_sub(lines_skipped);

            // Add the main log line if not skipped
            if skip_in_entry == 0 {
                all_lines.push(self.format_entry(entry, idx));
                lines_added += 1;
            }

            // Add stack trace frames (respecting collapse state)
            if let Some(trace) = &entry.stack_trace {
                let is_expanded = self.is_entry_expanded(entry);
                let frame_count = trace.frames.len();

                if is_expanded {
                    // Expanded: show all frames
                    for (frame_idx, frame) in trace.frames.iter().enumerate() {
                        if lines_added >= visible_lines {
                            break;
                        }

                        // Skip frames if we're starting mid-entry
                        let frame_position = 1 + frame_idx; // +1 for the message line
                        if frame_position <= skip_in_entry {
                            continue;
                        }

                        all_lines.push(Self::format_stack_frame_line(frame));
                        lines_added += 1;
                    }
                } else {
                    // Collapsed: show max_collapsed_frames + indicator if more
                    let visible_count = self.max_collapsed_frames.min(frame_count);
                    let hidden_count = frame_count.saturating_sub(self.max_collapsed_frames);

                    for (frame_idx, frame) in trace.frames.iter().take(visible_count).enumerate() {
                        if lines_added >= visible_lines {
                            break;
                        }

                        // Skip frames if we're starting mid-entry
                        let frame_position = 1 + frame_idx; // +1 for the message line
                        if frame_position <= skip_in_entry {
                            continue;
                        }

                        all_lines.push(Self::format_stack_frame_line(frame));
                        lines_added += 1;
                    }

                    // Add collapsed indicator if there are hidden frames
                    if hidden_count > 0 && lines_added < visible_lines {
                        let indicator_position = 1 + visible_count;
                        if indicator_position > skip_in_entry {
                            all_lines.push(Self::format_collapsed_indicator(hidden_count));
                            lines_added += 1;
                        }
                    }
                }
            }

            lines_skipped += entry_line_count;
        }

        // Calculate max line width for horizontal scroll bounds
        let max_line_width = all_lines
            .iter()
            .map(|l| Self::line_width(l))
            .max()
            .unwrap_or(0);
        let visible_width = inner.width as usize;

        // Update horizontal dimensions in state
        state.update_horizontal_size(max_line_width, visible_width);

        // Apply horizontal scroll to each line
        let scrolled_lines: Vec<Line> = all_lines
            .into_iter()
            .map(|line| Self::apply_horizontal_scroll(line, state.h_offset, visible_width))
            .collect();

        // Render log content WITHOUT wrapping (lines are truncated/scrolled)
        Paragraph::new(scrolled_lines).render(inner, buf);

        // Render scrollbar if content exceeds visible area
        if total_lines > visible_lines {
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("▲"))
                .end_symbol(Some("▼"))
                .track_symbol(Some("│"))
                .thumb_symbol("█");

            let mut scrollbar_state = ScrollbarState::new(total_lines).position(state.offset);

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

    fn make_entry(level: LogLevel, source: LogSource, msg: &str) -> LogEntry {
        LogEntry::new(level, source, msg)
    }

    /// Helper to create a VecDeque of log entries for tests
    fn logs_from(entries: Vec<LogEntry>) -> VecDeque<LogEntry> {
        VecDeque::from(entries)
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
        let logs = logs_from(vec![make_entry(LogLevel::Info, LogSource::App, "Test")]);
        let view = LogView::new(&logs).show_timestamps(true);
        let line = view.format_entry(&logs[0], 0);

        // Should have multiple spans including timestamp
        assert!(line.spans.len() >= 3);
    }

    #[test]
    fn test_format_entry_no_timestamp() {
        let logs = logs_from(vec![make_entry(LogLevel::Info, LogSource::App, "Test")]);
        let view = LogView::new(&logs).show_timestamps(false);
        let line = view.format_entry(&logs[0], 0);

        // Fewer spans without timestamp
        let with_ts = LogView::new(&logs).show_timestamps(true);
        let line_with = with_ts.format_entry(&logs[0], 0);
        assert!(line.spans.len() < line_with.spans.len());
    }

    #[test]
    fn test_format_entry_no_source() {
        let logs = logs_from(vec![make_entry(LogLevel::Info, LogSource::App, "Test")]);
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
        let logs = logs_from(vec![make_entry(LogLevel::Info, LogSource::App, "Test")]);
        let view = LogView::new(&logs).title("Logs");
        assert_eq!(view.build_title(), " Logs ");
    }

    #[test]
    fn test_build_title_with_default_filter() {
        let logs = logs_from(vec![make_entry(LogLevel::Info, LogSource::App, "Test")]);
        let filter = FilterState::default();
        let view = LogView::new(&logs).title("Logs").filter_state(&filter);
        // Default filter (All/All) should not show indicator
        assert_eq!(view.build_title(), " Logs ");
    }

    #[test]
    fn test_build_title_with_level_filter() {
        let logs = logs_from(vec![make_entry(LogLevel::Info, LogSource::App, "Test")]);
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
        let logs = logs_from(vec![make_entry(LogLevel::Info, LogSource::App, "Test")]);
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
        let logs = logs_from(vec![make_entry(LogLevel::Info, LogSource::App, "Test")]);
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
        let logs = logs_from(vec![make_entry(LogLevel::Info, LogSource::App, "Test")]);
        let filter = FilterState::default();
        let view = LogView::new(&logs).filter_state(&filter);
        assert!(view.filter_state.is_some());
    }

    #[test]
    fn test_filtered_logs_count() {
        let logs = logs_from(vec![
            make_entry(LogLevel::Info, LogSource::App, "info"),
            make_entry(LogLevel::Error, LogSource::App, "error"),
            make_entry(LogLevel::Warning, LogSource::Daemon, "warning"),
        ]);
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
        let logs = logs_from(vec![
            make_entry(LogLevel::Info, LogSource::App, "app info"),
            make_entry(LogLevel::Error, LogSource::Flutter, "flutter error"),
            make_entry(LogLevel::Warning, LogSource::Daemon, "daemon warning"),
        ]);
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
        let logs = logs_from(vec![
            make_entry(LogLevel::Error, LogSource::App, "app error"),
            make_entry(LogLevel::Error, LogSource::Flutter, "flutter error"),
            make_entry(LogLevel::Info, LogSource::App, "app info"),
            make_entry(LogLevel::Warning, LogSource::App, "app warning"),
        ]);
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
        let logs = logs_from(vec![make_entry(
            LogLevel::Info,
            LogSource::App,
            "Hello world",
        )]);
        let view = LogView::new(&logs);

        let spans = view.format_message_with_highlights("Hello world", 0, Style::default());

        assert_eq!(spans.len(), 1);
    }

    #[test]
    fn test_format_message_with_highlights_with_match() {
        let logs = logs_from(vec![make_entry(
            LogLevel::Info,
            LogSource::App,
            "Hello world",
        )]);
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
        let logs = logs_from(vec![make_entry(
            LogLevel::Info,
            LogSource::App,
            "test one test two",
        )]);
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
        let logs = logs_from(vec![
            make_entry(LogLevel::Info, LogSource::App, "test here"),
            make_entry(LogLevel::Info, LogSource::App, "no match"),
        ]);
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
        let logs = logs_from(vec![make_entry(LogLevel::Info, LogSource::App, "test")]);
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
        let logs = logs_from(vec![
            make_entry(LogLevel::Info, LogSource::App, "test message"),
            make_entry(LogLevel::Info, LogSource::App, "another test"),
        ]);
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
        let logs = logs_from(vec![make_entry(LogLevel::Info, LogSource::App, "test")]);
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
        let logs = logs_from(vec![make_entry(LogLevel::Info, LogSource::App, "test")]);
        let search = SearchState::default();
        let view = LogView::new(&logs).search_state(&search);
        assert!(view.search_state.is_some());
    }

    #[test]
    fn test_format_entry_with_search_highlights() {
        let logs = logs_from(vec![make_entry(
            LogLevel::Info,
            LogSource::App,
            "error occurred",
        )]);
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

    // ─────────────────────────────────────────────────────────
    // Stack Trace Rendering Tests (Phase 2 - Task 5)
    // ─────────────────────────────────────────────────────────

    use crate::core::stack_trace::ParsedStackTrace;

    #[test]
    fn test_format_stack_frame_project_frame() {
        let frame = StackFrame::new(0, "main", "package:app/main.dart", 15, 3);

        let spans = LogView::format_stack_frame(&frame);

        // Should have multiple spans: indent, frame#, function, (, file, :, line, :col, )
        assert!(spans.len() >= 7, "Got {} spans", spans.len());

        // First span should be indentation
        assert!(spans[0].content.starts_with("    "), "Expected indentation");

        // Check that function name is included
        let content: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(content.contains("main"), "Should contain function name");
        assert!(
            content.contains("main.dart"),
            "Should contain short file path"
        );
        assert!(content.contains("15"), "Should contain line number");
    }

    #[test]
    fn test_format_stack_frame_package_frame() {
        let frame = StackFrame::new(
            1,
            "State.setState",
            "package:flutter/src/widgets/framework.dart",
            1187,
            9,
        );

        let spans = LogView::format_stack_frame(&frame);

        // Package frame should have all dimmed styling
        // Just verify it produces spans
        assert!(!spans.is_empty());

        let content: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(content.contains("State.setState"));
        assert!(content.contains("framework.dart"));
    }

    #[test]
    fn test_format_stack_frame_async_gap() {
        let frame = StackFrame::async_gap(2);

        let spans = LogView::format_stack_frame(&frame);

        // Async gap should have 2 spans: indent + message
        assert_eq!(spans.len(), 2);

        let content: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(
            content.contains("<asynchronous suspension>"),
            "Got: {}",
            content
        );
    }

    #[test]
    fn test_format_stack_frame_no_column() {
        let mut frame = StackFrame::new(0, "test", "package:app/test.dart", 10, 0);
        frame.column = 0;

        let spans = LogView::format_stack_frame(&frame);

        let content: String = spans.iter().map(|s| s.content.as_ref()).collect();
        // Should contain line number but not ":0" for column
        assert!(content.contains(":10"), "Should have line number");
        // Column 0 means no column should be shown
        assert!(
            !content.contains(":0)"),
            "Should not show :0 column, got: {}",
            content
        );
    }

    #[test]
    fn test_calculate_total_lines_no_traces() {
        let logs = logs_from(vec![
            make_entry(LogLevel::Info, LogSource::App, "Hello"),
            make_entry(LogLevel::Error, LogSource::App, "Error"),
        ]);

        let total = LogViewState::calculate_total_lines(&logs);
        assert_eq!(total, 2); // No stack traces, just 2 entries
    }

    #[test]
    fn test_calculate_total_lines_with_traces() {
        let mut entry1 = make_entry(LogLevel::Info, LogSource::App, "Hello");
        // entry1 has no stack trace

        let mut entry2 = make_entry(LogLevel::Error, LogSource::App, "Error");
        let trace = ParsedStackTrace::parse(
            r#"
#0      main (package:app/main.dart:15:3)
#1      runApp (package:flutter/src/widgets/binding.dart:100:5)
#2      _startIsolate (dart:isolate-patch/isolate_patch.dart:307:19)
"#,
        );
        entry2.stack_trace = Some(trace);

        let logs = logs_from(vec![entry1, entry2]);

        let total = LogViewState::calculate_total_lines(&logs);
        // entry1: 1 line, entry2: 1 line + 3 frames = 4 lines, total = 5
        assert_eq!(total, 5);
    }

    #[test]
    fn test_calculate_total_lines_filtered() {
        let mut entry1 = make_entry(LogLevel::Info, LogSource::App, "Hello");
        let mut entry2 = make_entry(LogLevel::Error, LogSource::App, "Error");
        let trace = ParsedStackTrace::parse("#0 main (package:app/main.dart:15:3)");
        entry2.stack_trace = Some(trace);

        let logs = logs_from(vec![entry1, entry2]);

        // Only include entry2 (index 1)
        let indices = vec![1];
        let total = LogViewState::calculate_total_lines_filtered(&logs, &indices);
        assert_eq!(total, 2); // 1 message + 1 frame
    }

    #[test]
    fn test_format_stack_frame_line() {
        let frame = StackFrame::new(0, "test", "package:app/test.dart", 5, 1);

        let line = LogView::format_stack_frame_line(&frame);

        // Should produce a Line with spans
        assert!(!line.spans.is_empty());
    }

    #[test]
    fn test_stack_frame_with_long_function_name() {
        let frame = StackFrame::new(
            0,
            "_SomeVeryLongPrivateClassName.someEvenLongerMethodName",
            "package:app/file.dart",
            100,
            5,
        );

        let spans = LogView::format_stack_frame(&frame);

        let content: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(content.contains("_SomeVeryLongPrivateClassName.someEvenLongerMethodName"));
    }

    #[test]
    fn test_stack_frame_styles_module_constants() {
        // Verify style constants are accessible and have expected properties
        use stack_trace_styles::*;

        assert_eq!(INDENT, "    ");
        assert_eq!(FRAME_NUMBER.fg, Some(Color::DarkGray));
        assert_eq!(FUNCTION_PROJECT.fg, Some(Color::White));
        assert_eq!(FUNCTION_PACKAGE.fg, Some(Color::DarkGray));
        assert_eq!(FILE_PROJECT.fg, Some(Color::Blue));
        assert!(FILE_PROJECT.add_modifier.contains(Modifier::UNDERLINED));
        assert_eq!(LOCATION_PROJECT.fg, Some(Color::Cyan));
        assert!(ASYNC_GAP.add_modifier.contains(Modifier::ITALIC));
    }

    // ─────────────────────────────────────────────────────────
    // Collapsible Stack Traces Tests (Phase 2 Task 6)
    // ─────────────────────────────────────────────────────────

    use crate::app::session::CollapseState;

    #[test]
    fn test_format_collapsed_indicator_singular() {
        let line = LogView::format_collapsed_indicator(1);
        let content: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(content.contains("1 more frame..."), "Got: {}", content);
    }

    #[test]
    fn test_format_collapsed_indicator_plural() {
        let line = LogView::format_collapsed_indicator(5);
        let content: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(content.contains("5 more frames..."), "Got: {}", content);
    }

    #[test]
    fn test_format_collapsed_indicator_has_arrow() {
        let line = LogView::format_collapsed_indicator(3);
        let content: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(content.contains("▶"), "Should have arrow indicator");
    }

    #[test]
    fn test_calculate_entry_lines_no_trace() {
        let entry = make_entry(LogLevel::Info, LogSource::App, "Hello");
        let logs = logs_from(vec![entry]);
        let view = LogView::new(&logs)
            .default_collapsed(true)
            .max_collapsed_frames(3);

        assert_eq!(view.calculate_entry_lines(&logs[0]), 1); // Just message
    }

    #[test]
    fn test_calculate_entry_lines_collapsed() {
        let mut entry = make_entry(LogLevel::Error, LogSource::App, "Error");
        let trace = ParsedStackTrace::parse(
            r#"
#0      main (package:app/main.dart:15:3)
#1      runApp (package:flutter/src/widgets/binding.dart:100:5)
#2      _startIsolate (dart:isolate-patch/isolate_patch.dart:307:19)
#3      frame4 (package:app/other.dart:50:1)
#4      frame5 (package:app/other.dart:60:1)
"#,
        );
        entry.stack_trace = Some(trace);

        let logs = logs_from(vec![entry]);
        let view = LogView::new(&logs)
            .default_collapsed(true)
            .max_collapsed_frames(3);

        // Collapsed: 1 message + 3 visible frames + 1 indicator = 5
        assert_eq!(view.calculate_entry_lines(&logs[0]), 5);
    }

    #[test]
    fn test_calculate_entry_lines_expanded() {
        let mut entry = make_entry(LogLevel::Error, LogSource::App, "Error");
        let trace = ParsedStackTrace::parse(
            r#"
#0      main (package:app/main.dart:15:3)
#1      runApp (package:flutter/src/widgets/binding.dart:100:5)
#2      _startIsolate (dart:isolate-patch/isolate_patch.dart:307:19)
#3      frame4 (package:app/other.dart:50:1)
#4      frame5 (package:app/other.dart:60:1)
"#,
        );
        entry.stack_trace = Some(trace);

        let logs = logs_from(vec![entry]);
        let mut collapse_state = CollapseState::new();
        collapse_state.toggle(logs[0].id, true); // Expand it

        let view = LogView::new(&logs)
            .default_collapsed(true)
            .max_collapsed_frames(3)
            .collapse_state(&collapse_state);

        // Expanded: 1 message + 5 frames = 6
        assert_eq!(view.calculate_entry_lines(&logs[0]), 6);
    }

    #[test]
    fn test_calculate_entry_lines_few_frames() {
        // When there are fewer frames than max, no indicator needed
        let mut entry = make_entry(LogLevel::Error, LogSource::App, "Error");
        let trace = ParsedStackTrace::parse("#0 main (package:app/main.dart:15:3)");
        entry.stack_trace = Some(trace);

        let logs = logs_from(vec![entry]);
        let view = LogView::new(&logs)
            .default_collapsed(true)
            .max_collapsed_frames(3);

        // Only 1 frame, no indicator needed: 1 message + 1 frame = 2
        assert_eq!(view.calculate_entry_lines(&logs[0]), 2);
    }

    #[test]
    fn test_is_entry_expanded_no_collapse_state() {
        let mut entry = make_entry(LogLevel::Error, LogSource::App, "Error");
        let trace = ParsedStackTrace::parse("#0 main (package:app/main.dart:15:3)");
        entry.stack_trace = Some(trace);

        let logs = logs_from(vec![entry]);

        // Without collapse state, use default_collapsed setting
        let view = LogView::new(&logs).default_collapsed(true);
        assert!(!view.is_entry_expanded(&logs[0])); // Collapsed by default

        let view = LogView::new(&logs).default_collapsed(false);
        assert!(view.is_entry_expanded(&logs[0])); // Expanded by default
    }

    #[test]
    fn test_is_entry_expanded_with_collapse_state() {
        let mut entry = make_entry(LogLevel::Error, LogSource::App, "Error");
        let trace = ParsedStackTrace::parse("#0 main (package:app/main.dart:15:3)");
        entry.stack_trace = Some(trace);

        let logs = logs_from(vec![entry]);
        let mut collapse_state = CollapseState::new();

        // Toggle to expanded
        collapse_state.toggle(logs[0].id, true);

        let view = LogView::new(&logs)
            .default_collapsed(true)
            .collapse_state(&collapse_state);

        assert!(view.is_entry_expanded(&logs[0]));
    }

    #[test]
    fn test_collapse_state_builder() {
        let logs: VecDeque<LogEntry> = VecDeque::new();
        let collapse_state = CollapseState::new();

        let view = LogView::new(&logs).collapse_state(&collapse_state);

        assert!(view.collapse_state.is_some());
    }

    #[test]
    fn test_max_collapsed_frames_builder() {
        let logs: VecDeque<LogEntry> = VecDeque::new();

        let view = LogView::new(&logs).max_collapsed_frames(5);

        assert_eq!(view.max_collapsed_frames, 5);
    }

    #[test]
    fn test_default_collapsed_builder() {
        let logs: VecDeque<LogEntry> = VecDeque::new();

        let view = LogView::new(&logs).default_collapsed(false);

        assert!(!view.default_collapsed);
    }

    // ─────────────────────────────────────────────────────────
    // Horizontal Scroll Tests (Phase 2 Task 12)
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_horizontal_scroll_state_default() {
        let state = LogViewState::new();
        assert_eq!(state.h_offset, 0);
        assert_eq!(state.max_line_width, 0);
        assert_eq!(state.visible_width, 0);
    }

    #[test]
    fn test_scroll_left() {
        let mut state = LogViewState::new();
        state.h_offset = 20;
        state.max_line_width = 200;
        state.visible_width = 80;

        state.scroll_left(10);
        assert_eq!(state.h_offset, 10);

        state.scroll_left(20);
        assert_eq!(state.h_offset, 0); // Clamped at 0
    }

    #[test]
    fn test_scroll_right() {
        let mut state = LogViewState::new();
        state.h_offset = 0;
        state.max_line_width = 200;
        state.visible_width = 80;

        state.scroll_right(10);
        assert_eq!(state.h_offset, 10);

        state.scroll_right(200);
        assert_eq!(state.h_offset, 120); // Clamped at max - visible
    }

    #[test]
    fn test_scroll_to_line_start() {
        let mut state = LogViewState::new();
        state.h_offset = 50;

        state.scroll_to_line_start();
        assert_eq!(state.h_offset, 0);
    }

    #[test]
    fn test_scroll_to_line_end() {
        let mut state = LogViewState::new();
        state.h_offset = 0;
        state.max_line_width = 200;
        state.visible_width = 80;

        state.scroll_to_line_end();
        assert_eq!(state.h_offset, 120); // max - visible
    }

    #[test]
    fn test_no_horizontal_scroll_needed() {
        let mut state = LogViewState::new();
        state.max_line_width = 50;
        state.visible_width = 80;

        state.scroll_right(10);
        assert_eq!(state.h_offset, 0); // No scroll when content fits
    }

    #[test]
    fn test_update_horizontal_size() {
        let mut state = LogViewState::new();
        state.h_offset = 50;

        // Update with smaller content
        state.update_horizontal_size(60, 80);

        // h_offset should be clamped to 0 since content now fits
        assert_eq!(state.h_offset, 0);
        assert_eq!(state.max_line_width, 60);
        assert_eq!(state.visible_width, 80);
    }

    #[test]
    fn test_update_horizontal_size_clamps_offset() {
        let mut state = LogViewState::new();
        state.h_offset = 100;
        state.max_line_width = 200;
        state.visible_width = 80;

        // Shrink the content
        state.update_horizontal_size(150, 80);

        // h_offset should be clamped to max_h_offset = 150 - 80 = 70
        assert_eq!(state.h_offset, 70);
    }

    #[test]
    fn test_line_width() {
        let line = Line::from(vec![Span::raw("Hello"), Span::raw(" "), Span::raw("World")]);
        assert_eq!(LogView::line_width(&line), 11);
    }

    #[test]
    fn test_apply_horizontal_scroll_no_scroll_needed() {
        let line = Line::from("Short line");
        let result = LogView::apply_horizontal_scroll(line, 0, 80);
        let content: String = result.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(content, "Short line");
    }

    #[test]
    fn test_apply_horizontal_scroll_truncate_right() {
        let line = Line::from("A very long line that exceeds visible width");
        let result = LogView::apply_horizontal_scroll(line, 0, 20);
        let content: String = result.spans.iter().map(|s| s.content.as_ref()).collect();

        // Should have truncated content + right arrow
        assert!(content.ends_with('→'), "Got: {}", content);
        assert_eq!(content.chars().count(), 20);
    }

    #[test]
    fn test_apply_horizontal_scroll_with_offset() {
        let line = Line::from("A very long line that exceeds visible width");
        let result = LogView::apply_horizontal_scroll(line, 10, 20);
        let content: String = result.spans.iter().map(|s| s.content.as_ref()).collect();

        // Should have left arrow, content, and right arrow
        assert!(content.starts_with('←'), "Got: {}", content);
        assert!(content.ends_with('→'), "Got: {}", content);
        assert_eq!(content.chars().count(), 20);
    }

    #[test]
    fn test_apply_horizontal_scroll_at_end() {
        let line = Line::from("A very long line");
        // Scroll to the end
        let result = LogView::apply_horizontal_scroll(line, 6, 20);
        let content: String = result.spans.iter().map(|s| s.content.as_ref()).collect();

        // Should have left arrow but no right arrow (at end of line)
        assert!(content.starts_with('←'), "Got: {}", content);
        assert!(!content.ends_with('→'), "Got: {}", content);
    }

    #[test]
    fn test_apply_horizontal_scroll_preserves_styles() {
        let line = Line::from(vec![
            Span::styled("Red", Style::default().fg(Color::Red)),
            Span::styled("Blue", Style::default().fg(Color::Blue)),
        ]);
        // Scroll so we see part of both spans
        let result = LogView::apply_horizontal_scroll(line, 0, 20);

        // Should still have styled spans
        assert!(result.spans.len() >= 2);
    }

    #[test]
    fn test_apply_horizontal_scroll_offset_beyond_content() {
        let line = Line::from("Short");
        let result = LogView::apply_horizontal_scroll(line, 100, 20);
        let content: String = result.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(content, "");
    }

    // ─────────────────────────────────────────────────────────
    // Virtualized Rendering Tests (Task 05)
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_visible_range_basic() {
        let mut state = LogViewState::new();
        state.total_lines = 100;
        state.visible_lines = 20;
        state.buffer_lines = 5;
        state.offset = 50;

        let (start, end) = state.visible_range();

        assert_eq!(start, 45); // 50 - 5 buffer
        assert_eq!(end, 75); // 50 + 20 + 5 buffer
    }

    #[test]
    fn test_visible_range_at_start() {
        let mut state = LogViewState::new();
        state.total_lines = 100;
        state.visible_lines = 20;
        state.buffer_lines = 5;
        state.offset = 0;

        let (start, end) = state.visible_range();

        assert_eq!(start, 0); // Can't go negative
        assert_eq!(end, 25); // 0 + 20 + 5
    }

    #[test]
    fn test_visible_range_at_end() {
        let mut state = LogViewState::new();
        state.total_lines = 100;
        state.visible_lines = 20;
        state.buffer_lines = 5;
        state.offset = 80;

        let (start, end) = state.visible_range();

        assert_eq!(start, 75); // 80 - 5
        assert_eq!(end, 100); // Capped at total
    }

    #[test]
    fn test_visible_range_small_content() {
        let mut state = LogViewState::new();
        state.total_lines = 10;
        state.visible_lines = 20;
        state.buffer_lines = 5;
        state.offset = 0;

        let (start, end) = state.visible_range();

        assert_eq!(start, 0);
        assert_eq!(end, 10); // Capped at total
    }

    #[test]
    fn test_visible_range_zero_buffer() {
        let mut state = LogViewState::new();
        state.total_lines = 100;
        state.visible_lines = 20;
        state.buffer_lines = 0;
        state.offset = 50;

        let (start, end) = state.visible_range();

        assert_eq!(start, 50); // No buffer
        assert_eq!(end, 70); // No buffer
    }

    #[test]
    fn test_buffer_lines_default() {
        let state = LogViewState::new();
        assert_eq!(state.buffer_lines, DEFAULT_BUFFER_LINES);
    }

    #[test]
    fn test_set_buffer_lines() {
        let mut state = LogViewState::new();
        state.set_buffer_lines(20);
        assert_eq!(state.buffer_lines, 20);
    }

    #[test]
    fn test_visible_range_with_custom_buffer() {
        let mut state = LogViewState::new();
        state.total_lines = 200;
        state.visible_lines = 30;
        state.set_buffer_lines(15);
        state.offset = 100;

        let (start, end) = state.visible_range();

        assert_eq!(start, 85); // 100 - 15
        assert_eq!(end, 145); // 100 + 30 + 15
    }

    #[test]
    fn test_visible_range_empty_content() {
        let mut state = LogViewState::new();
        state.total_lines = 0;
        state.visible_lines = 20;
        state.buffer_lines = 5;
        state.offset = 0;

        let (start, end) = state.visible_range();

        assert_eq!(start, 0);
        assert_eq!(end, 0);
    }
}
