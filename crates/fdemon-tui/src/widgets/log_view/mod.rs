//! Scrollable log view widget with rich formatting

use std::collections::VecDeque;

use fdemon_app::hyperlinks::LinkHighlightState;
use fdemon_app::log_view_state::{FocusInfo, LogViewState};
use fdemon_core::{
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
pub mod styles;

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
    collapse_state: Option<&'a fdemon_app::session::CollapseState>,
    /// Whether stack traces are collapsed by default
    default_collapsed: bool,
    /// Maximum frames to show when collapsed
    max_collapsed_frames: usize,
    /// Link highlight state for rendering shortcut badges (Phase 3.1)
    link_highlight_state: Option<&'a LinkHighlightState>,
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
            link_highlight_state: None,
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
    pub fn collapse_state(mut self, state: &'a fdemon_app::session::CollapseState) -> Self {
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

    /// Set link highlight state for rendering shortcut badges (Phase 3.1)
    pub fn link_highlight_state(mut self, state: &'a LinkHighlightState) -> Self {
        if state.is_active() {
            self.link_highlight_state = Some(state);
        }
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

    // ─────────────────────────────────────────────────────────────────────────────
    // Link Highlight Mode Badge Helpers (Phase 3.1 Task 07)
    // ─────────────────────────────────────────────────────────────────────────────

    /// Create a styled shortcut badge like "[1]" or "[a]"
    fn link_badge(shortcut: char) -> Span<'static> {
        Span::styled(
            format!("[{}]", shortcut),
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
    }

    /// Style for highlighted file reference text in link mode
    fn link_text_style() -> Style {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::UNDERLINED)
    }

    /// Insert a link badge into spans at the position of a file reference.
    ///
    /// This finds the span containing the display_text and splits it to insert
    /// the badge before the file reference, applying link styling to the reference.
    fn insert_link_badge_into_spans(
        spans: Vec<Span<'static>>,
        display_text: &str,
        shortcut: char,
    ) -> Vec<Span<'static>> {
        let mut result = Vec::with_capacity(spans.len() + 2);
        let badge = Self::link_badge(shortcut);
        let link_style = Self::link_text_style();
        let mut badge_inserted = false;

        for span in spans {
            if !badge_inserted {
                if let Some(pos) = span.content.find(display_text) {
                    // Found the file reference in this span - split it
                    let before = &span.content[..pos];
                    let file_part = &span.content[pos..pos + display_text.len()];
                    let after = &span.content[pos + display_text.len()..];

                    // Add text before the file reference
                    if !before.is_empty() {
                        result.push(Span::styled(before.to_string(), span.style));
                    }

                    // Add the badge
                    result.push(badge.clone());

                    // Add the file reference with link styling
                    result.push(Span::styled(file_part.to_string(), link_style));

                    // Add text after the file reference
                    if !after.is_empty() {
                        result.push(Span::styled(after.to_string(), span.style));
                    }

                    badge_inserted = true;
                    continue;
                }
            }
            result.push(span);
        }

        result
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

        // Check for link badge in link highlight mode (Phase 3.1)
        // Links from log messages have frame_index == None
        if let Some(link_state) = self.link_highlight_state {
            if let Some(link) = link_state
                .links
                .iter()
                .find(|l| l.entry_index == entry_index && l.frame_index.is_none())
            {
                spans =
                    Self::insert_link_badge_into_spans(spans, &link.display_text, link.shortcut);
            }
        }

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
    #[allow(dead_code)] // Used in tests
    fn format_stack_frame(frame: &StackFrame) -> Vec<Span<'static>> {
        use styles::*;

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
    #[allow(dead_code)] // Used in tests
    fn format_stack_frame_line(frame: &StackFrame) -> Line<'static> {
        Line::from(Self::format_stack_frame(frame))
    }

    /// Format a stack frame as a Line with optional link badge (Phase 3.1)
    ///
    /// When link highlight mode is active and this frame has a detected link,
    /// inserts a shortcut badge before the file reference.
    fn format_stack_frame_line_with_links(
        &self,
        frame: &StackFrame,
        entry_index: usize,
        frame_index: usize,
    ) -> Line<'static> {
        use styles::*;

        // Handle async gap specially - no links possible
        if frame.is_async_gap {
            return Line::from(vec![
                Span::styled(INDENT.to_string(), Style::default()),
                Span::styled("<asynchronous suspension>".to_string(), ASYNC_GAP),
            ]);
        }

        // Check if we have a link for this frame
        let link = self.link_highlight_state.and_then(|state| {
            state
                .links
                .iter()
                .find(|l| l.entry_index == entry_index && l.frame_index == Some(frame_index))
        });

        // Determine styles based on frame type and link state
        let (func_style, file_style, loc_style) = if link.is_some() {
            // Link mode - use link styling for the file reference
            let link_style = Self::link_text_style();
            (
                if frame.is_package_frame {
                    FUNCTION_PACKAGE
                } else {
                    FUNCTION_PROJECT
                },
                link_style,
                link_style,
            )
        } else if frame.is_package_frame {
            // Package frame - all dimmed
            (FUNCTION_PACKAGE, FILE_PACKAGE, LOCATION_PACKAGE)
        } else {
            // Project frame - highlighted
            (FUNCTION_PROJECT, FILE_PROJECT, LOCATION_PROJECT)
        };

        let mut spans = Vec::with_capacity(12);

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

        // Insert link badge before file path if we have a link
        if let Some(link) = link {
            spans.push(Self::link_badge(link.shortcut));
        }

        // File path (short version)
        spans.push(Span::styled(frame.short_path().to_string(), file_style));

        // Colon separator
        spans.push(Span::styled(
            ":".to_string(),
            if link.is_some() {
                Self::link_text_style()
            } else {
                PUNCTUATION
            },
        ));

        // Line number
        spans.push(Span::styled(frame.line.to_string(), loc_style));

        // Column (if present)
        if frame.column > 0 {
            spans.push(Span::styled(format!(":{}", frame.column), loc_style));
        }

        // Closing paren
        spans.push(Span::styled(")".to_string(), PUNCTUATION));

        Line::from(spans)
    }

    /// Format collapsed indicator: "▶ N more frames..."
    fn format_collapsed_indicator(hidden_count: usize) -> Line<'static> {
        use styles::*;

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

        // Center the instruction message
        let instruction_text = vec![
            Line::from(""),
            Line::from(Span::styled(
                "Not Connected",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Press + to start a new session",
                Style::default().fg(Color::DarkGray),
            )),
        ];

        Paragraph::new(instruction_text)
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

        // Track focus info for the first visible line (Phase 3 Task 03)
        let mut focus_captured = false;

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
                // Track focus if this is the first visible line
                if !focus_captured {
                    state.focus_info.entry_index = Some(idx);
                    state.focus_info.entry_id = Some(entry.id);
                    state.focus_info.frame_index = None;
                    focus_captured = true;
                }

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

                        // Track focus if this is the first visible line
                        if !focus_captured {
                            state.focus_info.entry_index = Some(idx);
                            state.focus_info.entry_id = Some(entry.id);
                            state.focus_info.frame_index = Some(frame_idx);
                            focus_captured = true;
                        }

                        // Use link-aware formatting (Phase 3.1)
                        all_lines
                            .push(self.format_stack_frame_line_with_links(frame, idx, frame_idx));
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

                        // Track focus if this is the first visible line
                        if !focus_captured {
                            state.focus_info.entry_index = Some(idx);
                            state.focus_info.entry_id = Some(entry.id);
                            state.focus_info.frame_index = Some(frame_idx);
                            focus_captured = true;
                        }

                        // Use link-aware formatting (Phase 3.1)
                        all_lines
                            .push(self.format_stack_frame_line_with_links(frame, idx, frame_idx));
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

        // Clear focus info if nothing was captured (empty view)
        if !focus_captured {
            state.focus_info = FocusInfo::default();
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
mod tests;
