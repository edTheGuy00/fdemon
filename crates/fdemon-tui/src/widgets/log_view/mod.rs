//! Scrollable log view widget with rich formatting

use std::collections::VecDeque;
use std::time::Duration;

use fdemon_app::config::FlutterMode;
use fdemon_app::hyperlinks::LinkHighlightState;
use fdemon_app::log_view_state::{
    char_display_width, wrap_row_starts, wrap_row_starts_widths, wrapped_row_count_widths,
    FocusInfo, LogViewState, SelPoint, SelectionEdge, SelectionRow,
};
use fdemon_core::{
    AppPhase, FilterState, LogEntry, LogLevel, LogLevelFilter, LogSource, LogSourceFilter,
    SearchState, StackFrame,
};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
        StatefulWidget, Widget,
    },
};

use crate::theme::icons::IconSet;
use crate::theme::palette;
use crate::theme::styles as theme_styles;
use crate::widgets::shimmer;
use crate::widgets::spinner::{spinner_char, SPINNER_TICKS_PER_FRAME};
use crate::widgets::MouseCtx;

/// Stack trace styling constants
pub mod styles;

/// Minimum width (in columns) for full status display.
/// Below this width, the bottom metadata bar switches to compact mode.
const MIN_FULL_STATUS_WIDTH: u16 = 60;

/// Status information for the bottom metadata bar
pub struct StatusInfo<'a> {
    pub phase: &'a AppPhase,
    pub is_busy: bool,
    pub mode: Option<&'a FlutterMode>,
    pub flavor: Option<&'a str>,
    pub duration: Option<Duration>,
    pub error_count: usize,
    /// Whether the VM Service WebSocket is connected (shows [VM] badge)
    pub vm_connected: bool,
    /// DAP server port if running (shows [DAP :PORT] badge).
    pub dap_port: Option<u16>,
    /// IDE name for which DAP config was generated (e.g. "VS Code").
    /// When present alongside `dap_port`, badge becomes `[DAP :PORT · IDE]`.
    pub dap_config_ide: Option<String>,
    /// Whether terminal mouse capture is currently active.
    /// Renders `[mouse]` (dim) when active, `[mouse-off]` (warning) when inactive.
    pub mouse_capture_active: bool,
    /// Global animation frame, drives the shimmer on transient status labels.
    pub animation_frame: u64,
    /// Live launch progress line (build / pre-app readiness); shown next to
    /// a transient phase label. `None` when nothing is in flight.
    pub progress: Option<&'a str>,
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
    collapse_state: Option<&'a fdemon_app::session::CollapseState>,
    /// Whether stack traces are collapsed by default
    default_collapsed: bool,
    /// Maximum frames to show when collapsed
    max_collapsed_frames: usize,
    /// Link highlight state for rendering shortcut badges (Phase 3.1)
    link_highlight_state: Option<&'a LinkHighlightState>,
    /// Status info for bottom metadata bar (Phase 2 Task 4)
    status_info: Option<StatusInfo<'a>>,
    /// Icon set for rendering icons
    icons: IconSet,
    /// Whether line wrap mode is enabled. When true, horizontal scroll is skipped.
    wrap_mode: bool,
    /// Pending count of log entries that arrived while the view was scrolled away
    /// from the tail. Used to render the jump-to-latest pill (Phase 4, Task 02).
    /// Default 0 — pill is suppressed when the count is zero.
    unseen_log_count: usize,
}

impl<'a> LogView<'a> {
    pub fn new(logs: &'a VecDeque<LogEntry>, icons: IconSet) -> Self {
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
            status_info: None,
            icons,
            wrap_mode: false,
            unseen_log_count: 0,
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

    /// Set status info for bottom metadata bar (Phase 2 Task 4)
    pub fn with_status(mut self, status: StatusInfo<'a>) -> Self {
        self.status_info = Some(status);
        self
    }

    /// Set wrap mode. When enabled, long lines wrap at terminal width instead of scrolling.
    pub fn wrap_mode(mut self, enabled: bool) -> Self {
        self.wrap_mode = enabled;
        self
    }

    /// Set the count of log entries that arrived while the view was scrolled away
    /// from the tail. Drives the jump-to-latest pill. Default 0 (no pill drawn).
    pub fn unseen_log_count(mut self, count: usize) -> Self {
        self.unseen_log_count = count;
        self
    }

    /// Get style for log level - returns (level_style, message_style)
    fn level_style(level: LogLevel) -> (Style, Style) {
        match level {
            LogLevel::Error => (
                Style::default()
                    .fg(palette::LOG_ERROR)
                    .add_modifier(Modifier::BOLD),
                Style::default().fg(palette::LOG_ERROR_MSG),
            ),
            LogLevel::Warning => (
                Style::default()
                    .fg(palette::LOG_WARNING)
                    .add_modifier(Modifier::BOLD),
                Style::default().fg(palette::LOG_WARNING_MSG),
            ),
            LogLevel::Info => (
                Style::default().fg(palette::LOG_INFO),
                Style::default().fg(palette::LOG_INFO_MSG),
            ),
            LogLevel::Debug => (
                Style::default().fg(palette::LOG_DEBUG),
                Style::default().fg(palette::LOG_DEBUG_MSG),
            ),
        }
    }

    /// Format message with inline highlighting for special content
    fn format_message(message: &str, base_style: Style) -> Span<'static> {
        // Highlight reload success
        if message.contains("Reloaded") || message.contains("reloaded") {
            Span::styled(message.to_string(), base_style.fg(palette::STATUS_GREEN))
        } else if message.contains("Exception") || message.contains("Error") {
            // Highlight exceptions
            Span::styled(message.to_string(), base_style.fg(palette::LOG_ERROR_MSG))
        } else if message.starts_with("    ") {
            // Stack trace lines (indented)
            Span::styled(
                message.to_string(),
                Style::default().fg(palette::TEXT_MUTED),
            )
        } else {
            Span::styled(message.to_string(), base_style)
        }
    }

    /// Get style for log source
    fn source_style(source: &LogSource) -> Style {
        match source {
            LogSource::App => Style::default().fg(palette::SOURCE_APP),
            LogSource::Daemon => Style::default().fg(palette::SOURCE_DAEMON),
            LogSource::Flutter => Style::default().fg(palette::SOURCE_FLUTTER),
            LogSource::FlutterError => Style::default().fg(palette::SOURCE_FLUTTER_ERROR),
            LogSource::Watcher => Style::default().fg(palette::SOURCE_WATCHER),
            LogSource::VmService => Style::default().fg(palette::ACCENT),
            LogSource::Native { .. } => Style::default().fg(palette::SOURCE_NATIVE),
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
                .fg(palette::CONTRAST_FG)
                .bg(palette::ACCENT)
                .add_modifier(Modifier::BOLD),
        )
    }

    /// Style for highlighted file reference text in link mode
    fn link_text_style() -> Style {
        Style::default()
            .fg(palette::ACCENT)
            .add_modifier(Modifier::UNDERLINED)
    }

    /// Return true if `span` is a link badge `[<c>]` (3-char content, ACCENT background).
    ///
    /// Used during badge-region recording to locate the badge span within a rendered
    /// line without re-running the full link-detection logic.
    fn is_link_badge_span(span: &Span<'_>) -> bool {
        let c = &span.content;
        if c.len() < 3 {
            return false;
        }
        let chars: Vec<char> = c.chars().collect();
        if chars.len() != 3 {
            return false;
        }
        if chars[0] != '[' || chars[2] != ']' {
            return false;
        }
        span.style.bg == Some(crate::theme::palette::ACCENT)
    }

    /// Extract the shortcut character from a badge span `[<c>]`.
    ///
    /// Returns `None` if the span content does not have exactly 3 chars in `[X]` form.
    fn badge_span_shortcut(span: &Span<'_>) -> Option<char> {
        let chars: Vec<char> = span.content.chars().collect();
        if chars.len() == 3 && chars[0] == '[' && chars[2] == ']' {
            Some(chars[1])
        } else {
            None
        }
    }

    /// Walk `spans` and push a [`BadgeAction`] for each badge span found.
    ///
    /// `rel_y` is the row's Y coordinate (relative to `content_area.y`).
    /// This is called only when `link_highlight_state.is_active()` and a
    /// `MouseCtx` is present.
    fn collect_badge_actions(spans: &[Span<'_>], rel_y: u16, out: &mut Vec<BadgeAction>) {
        let mut col: u16 = 0;
        for span in spans {
            let width = span.content.chars().count() as u16;
            if Self::is_link_badge_span(span) {
                if let Some(shortcut) = Self::badge_span_shortcut(span) {
                    out.push(BadgeAction {
                        rel_y,
                        col_offset: col,
                        shortcut,
                    });
                }
            }
            col = col.saturating_add(width);
        }
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
        let (_level_style, msg_style) = Self::level_style(entry.level);
        let source_style = Self::source_style(&entry.source);

        let mut spans = Vec::with_capacity(8);

        // Timestamp: "HH:MM:SS "
        if self.show_timestamps {
            spans.push(Span::styled(
                entry.formatted_time(),
                Style::default().fg(palette::TEXT_MUTED),
            ));
        }

        // Bullet separator: " • " between timestamp and source tag
        if self.show_timestamps && self.show_source {
            spans.push(Span::styled(
                " • ",
                Style::default().fg(palette::TEXT_MUTED),
            ));
        } else if self.show_timestamps {
            spans.push(Span::raw(" "));
        }

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
            .bg(palette::SEARCH_HIGHLIGHT_BG)
            .fg(palette::SEARCH_HIGHLIGHT_FG)
            .add_modifier(Modifier::BOLD);
        let current_highlight_style = Style::default()
            .bg(palette::SEARCH_CURRENT_BG)
            .fg(palette::SEARCH_CURRENT_FG)
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
            Span::styled(
                "▶ ".to_string(),
                Style::default().fg(palette::SEARCH_HIGHLIGHT_BG),
            ),
            Span::styled(
                text,
                Style::default()
                    .fg(palette::BORDER_DIM)
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

    /// Terminal rows a rendered line occupies when wrapped, using the same
    /// greedy display-width packing as [`Self::wrap_line_chars`] (wide CJK/emoji
    /// chars count 2 cells). Allocation-free — iterates the spans' chars.
    fn line_wrapped_row_count(line: &Line, visible_width: usize) -> usize {
        wrapped_row_count_widths(
            line.spans
                .iter()
                .flat_map(|s| s.content.chars())
                .map(char_display_width),
            visible_width,
        )
    }

    /// Estimate the display width (terminal cells) of the gutter prefix of a
    /// formatted message line (without full formatting): timestamp, separator,
    /// and source tag — all single-width ASCII plus the `•` bullet (width 1).
    /// Used together with the message text to compute wrapped row counts for
    /// scroll bounds.
    fn estimate_prefix_width(&self, entry: &LogEntry) -> usize {
        let mut w = 0;
        // Timestamp: "HH:MM:SS" = 8 chars
        if self.show_timestamps {
            w += 8;
        }
        // Bullet separator: " • " = 3 cells (when both timestamp and source shown)
        if self.show_timestamps && self.show_source {
            w += 3;
        } else if self.show_timestamps {
            w += 1; // just a space
        }
        // Source: "[app] " or "[flutter] " etc — bracket + prefix + bracket + space
        if self.show_source {
            w += 1 + entry.source.prefix().len() + 2; // "[" + prefix + "] "
        }
        w
    }

    /// Calculate terminal rows for an entry in wrap mode.
    /// Accounts for wrapped message lines; stack frame lines assumed to be 1 row each.
    fn calculate_entry_display_rows(&self, entry: &LogEntry, visible_width: usize) -> usize {
        if visible_width == 0 {
            return self.calculate_entry_lines(entry);
        }
        let prefix_width = self.estimate_prefix_width(entry);
        // Same greedy packing as the renderer: prefix cells first (all width 1),
        // then the message's display widths.
        let msg_rows = wrapped_row_count_widths(
            std::iter::repeat_n(1, prefix_width)
                .chain(entry.message.chars().map(char_display_width)),
            visible_width,
        );
        // Stack frame lines rarely exceed terminal width, count as 1 row each
        let logical_lines = self.calculate_entry_lines(entry);
        let frame_lines = logical_lines.saturating_sub(1);
        msg_rows + frame_lines
    }

    /// Render empty state with centered message
    fn render_empty(&self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(palette::BORDER_DIM))
            .style(Style::default().bg(palette::CARD_BG));

        let inner = block.inner(area);
        block.render(area, buf);

        // Center the instruction message
        let instruction_text = vec![
            Line::from(""),
            Line::from(Span::styled(
                "Not Connected",
                Style::default()
                    .fg(palette::TEXT_MUTED)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Press + to start a new session",
                Style::default().fg(palette::TEXT_MUTED),
            )),
        ];

        Paragraph::new(instruction_text)
            .alignment(ratatui::layout::Alignment::Center)
            .render(inner, buf);
    }

    /// Render the top metadata bar with label and status badge
    fn render_metadata_bar(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let mut spans = Vec::new();

        // Left side: icon + "TERMINAL LOGS" label
        spans.push(Span::styled(
            format!("{} ", self.icons.terminal()),
            Style::default().fg(palette::TEXT_SECONDARY),
        ));
        spans.push(Span::styled(
            "TERMINAL LOGS",
            Style::default().fg(palette::TEXT_SECONDARY),
        ));

        // Add filter/search indicators if present
        let mut indicator_parts = Vec::new();
        if let Some(filter) = self.filter_state {
            if filter.is_active() {
                if filter.level_filter != LogLevelFilter::All {
                    indicator_parts.push(filter.level_filter.display_name().to_string());
                }
                if filter.source_filter != LogSourceFilter::All {
                    indicator_parts.push(filter.source_filter.display_name().to_string());
                }
            }
        }
        if let Some(search) = self.search_state {
            if !search.query.is_empty() {
                let status = search.display_status();
                if !status.is_empty() {
                    indicator_parts.push(status);
                }
            }
        }

        // Wrap mode indicator
        if self.wrap_mode {
            indicator_parts.push("wrap".to_string());
        } else {
            indicator_parts.push("nowrap".to_string());
        }

        if !indicator_parts.is_empty() {
            spans.push(Span::styled(
                format!(" • {}", indicator_parts.join(" | ")),
                Style::default().fg(palette::TEXT_SECONDARY),
            ));
        }

        // Right side: "LIVE FEED" badge
        // Calculate position based on available width
        let right_badge = " LIVE FEED ";
        let left_text_len: usize = spans.iter().map(|s| s.content.chars().count()).sum();
        let badge_len = right_badge.chars().count();

        // Fill space between left text and right badge
        let padding = if area.width as usize > left_text_len + badge_len {
            area.width as usize - left_text_len - badge_len
        } else {
            1
        };

        spans.push(Span::raw(" ".repeat(padding)));
        spans.push(Span::styled(
            right_badge,
            Style::default()
                .fg(palette::TEXT_MUTED)
                .bg(palette::DEEPEST_BG),
        ));

        let line = Line::from(spans);
        buf.set_line(area.x, area.y, &line, area.width);
    }

    /// Build the label spans for the phase indicator.
    ///
    /// For transient phases (`Initializing`, `Reloading`, `Quitting`, or `is_busy`),
    /// returns per-character shimmered spans so the label sweeps with a bright
    /// highlight. For steady states (`Running`, `Stopped`) returns a single static
    /// styled span identical to the pre-shimmer behaviour.
    fn status_label_spans_inner(
        label: &str,
        phase_style: Style,
        is_transient: bool,
        animation_frame: u64,
    ) -> Vec<Span<'static>> {
        if is_transient {
            let phase = shimmer::shimmer_phase(animation_frame);
            let base = phase_style.fg.unwrap_or(palette::TEXT_SECONDARY);
            let highlight = palette::TEXT_BRIGHT;
            let modifier = phase_style.add_modifier; // preserve BOLD etc.
            shimmer::shimmer_spans(label, base, highlight, phase, modifier)
        } else {
            vec![Span::styled(label.to_owned(), phase_style)]
        }
    }

    /// Render the bottom metadata bar with status info
    fn render_bottom_metadata(
        area: Rect,
        buf: &mut Buffer,
        status: &StatusInfo,
        compact: bool,
        icons: &IconSet,
    ) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        // Get phase indicator based on busy state
        let (icon, label, phase_style) = if status.is_busy {
            theme_styles::phase_indicator_busy(icons)
        } else {
            theme_styles::phase_indicator(status.phase, icons)
        };

        // Transient = work in progress; steady = Running/Stopped.
        let is_transient = status.is_busy
            || matches!(
                status.phase,
                AppPhase::Initializing
                    | AppPhase::Preparing
                    | AppPhase::Launching
                    | AppPhase::Reloading
                    | AppPhase::Quitting
            );

        // Launch-lifecycle phases animate their glyph; every other phase (incl. the
        // is_busy / Reloading path) keeps its static phase_indicator icon.
        let is_launch_phase = !status.is_busy
            && matches!(
                status.phase,
                AppPhase::Initializing | AppPhase::Preparing | AppPhase::Launching
            );

        let icon_span = if is_launch_phase {
            let glyph = spinner_char(status.animation_frame / SPINNER_TICKS_PER_FRAME);
            Span::styled(glyph.to_string(), phase_style)
        } else {
            Span::styled(icon, phase_style)
        };

        // Left side: icon (spinner during launch phases, static otherwise) + shimmered or static label
        let mut spans = vec![Span::raw(" "), icon_span, Span::raw(" ")];
        spans.extend(Self::status_label_spans_inner(
            label,
            phase_style,
            is_transient,
            status.animation_frame,
        ));

        // Progress suffix: shown only in transient phases when progress text is available.
        // The text is rendered muted (not shimmered) — only the phase label shimmers.
        if is_transient {
            if let Some(progress) = status.progress {
                spans.push(Span::styled(
                    format!("  {progress}"),
                    Style::default().fg(palette::TEXT_MUTED),
                ));
            }
        }

        // For compact mode, only show phase indicator and errors (if > 0)
        if compact {
            if status.error_count > 0 {
                spans.push(Span::raw("  "));
                spans.push(Span::styled(
                    format!("{} {}", icons.alert(), status.error_count),
                    theme_styles::status_red().add_modifier(Modifier::BOLD),
                ));
            }
        } else {
            // Full mode: add mode badge
            if let Some(mode) = status.mode {
                let mode_text = match mode {
                    FlutterMode::Debug => "Debug",
                    FlutterMode::Profile => "Profile",
                    FlutterMode::Release => "Release",
                };
                spans.push(Span::raw("  "));
                spans.push(Span::styled(mode_text, theme_styles::accent()));
                if let Some(flavor) = status.flavor {
                    spans.push(Span::styled(
                        format!(" ({})", flavor),
                        theme_styles::text_secondary(),
                    ));
                }
            }

            // VM Service connection indicator
            if status.vm_connected {
                spans.push(Span::raw("  "));
                spans.push(Span::styled(
                    "[VM]",
                    Style::default().fg(palette::STATUS_GREEN),
                ));
            }

            // DAP server indicator — optionally embeds the IDE name when a
            // config was generated: [DAP :4711 · VS Code]
            if let Some(port) = status.dap_port {
                spans.push(Span::raw("  "));
                let dap_text = if let Some(ref ide) = status.dap_config_ide {
                    format!("[DAP :{port} \u{00b7} {ide}]")
                } else {
                    format!("[DAP :{port}]")
                };
                spans.push(Span::styled(
                    dap_text,
                    Style::default().fg(palette::STATUS_GREEN),
                ));
            }
        }

        // Right-aligned section: uptime + errors + mouse badge (only in full mode)
        if !compact {
            let mut right_spans = Vec::new();

            // Uptime timer
            if let Some(duration) = status.duration {
                let mins = duration.as_secs() / 60;
                let secs = duration.as_secs() % 60;
                right_spans.push(Span::styled(
                    format!("{} {}:{:02}", icons.activity(), mins, secs),
                    theme_styles::text_secondary(),
                ));
                right_spans.push(Span::raw("  "));
            }

            // Error count
            if status.error_count > 0 {
                right_spans.push(Span::styled(
                    format!("{} {}", icons.alert(), status.error_count),
                    theme_styles::status_red().add_modifier(Modifier::BOLD),
                ));
            } else {
                right_spans.push(Span::styled(
                    format!("{} 0", icons.alert()),
                    theme_styles::text_muted(),
                ));
            }

            // Mouse capture badge — always shown when there is space.
            // [mouse] in dim color when active (default state, no user action needed).
            // [mouse-off] in warning color when inactive (discoverable cue for Alt+m).
            let mouse_badge_text = if status.mouse_capture_active {
                "[mouse]"
            } else {
                "[mouse-off]"
            };
            let mouse_badge_style = if status.mouse_capture_active {
                Style::default().fg(palette::TEXT_MUTED)
            } else {
                Style::default().fg(palette::STATUS_YELLOW)
            };

            // Calculate how much width the badge needs (plus 2 spaces separator)
            let badge_len = mouse_badge_text.chars().count() + 2; // "  " prefix

            // Calculate current widths to decide if badge fits
            let left_width: usize = spans.iter().map(|s| s.content.chars().count()).sum();
            let right_width_no_badge: usize =
                right_spans.iter().map(|s| s.content.chars().count()).sum();
            // Badge fits when there is enough room: left + padding(1) + right + 2 + badge <= area.width
            let fits = (area.width as usize) >= left_width + 1 + right_width_no_badge + badge_len;

            if fits {
                right_spans.push(Span::raw("  "));
                right_spans.push(Span::styled(mouse_badge_text, mouse_badge_style));
            }

            // Calculate padding between left and right sections
            let right_width: usize = right_spans.iter().map(|s| s.content.chars().count()).sum();
            let padding = (area.width as usize).saturating_sub(left_width + right_width + 1);

            spans.push(Span::raw(" ".repeat(padding)));
            spans.extend(right_spans);
        }

        let line = Line::from(spans);
        buf.set_line(area.x, area.y, &line, area.width);
    }

    /// Render empty filtered state
    fn render_no_matches(&self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(palette::BORDER_DIM))
            .style(Style::default().bg(palette::CARD_BG));

        let inner = block.inner(area);
        block.render(area, buf);

        // Render metadata bar
        if inner.height > 0 {
            self.render_metadata_bar(Rect::new(inner.x, inner.y, inner.width, 1), buf);
        }

        // Content area starts 2 lines below metadata bar (1 for bar + 1 for gap)
        let content_area = Rect::new(
            inner.x,
            inner.y.saturating_add(2),
            inner.width,
            inner.height.saturating_sub(2),
        );

        let message = vec![
            Line::from(""),
            Line::from(Span::styled(
                "No logs match current filter",
                Style::default()
                    .fg(palette::STATUS_YELLOW)
                    .add_modifier(Modifier::ITALIC),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Press Ctrl+f to reset filters",
                Style::default().fg(palette::TEXT_MUTED),
            )),
        ];

        Paragraph::new(message)
            .alignment(ratatui::layout::Alignment::Center)
            .render(content_area, buf);
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
                Style::default().fg(palette::BORDER_DIM),
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
                Style::default().fg(palette::BORDER_DIM),
            ));
        }

        Line::from(spans)
    }

    /// Concatenate a rendered line's span contents into its full text.
    fn line_text(line: &Line) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    /// Wrap a logical line into screen rows by **display width** (not word
    /// boundaries), preserving each character's `Style`. Wide CJK/emoji chars
    /// occupy 2 cells and are never split across rows; combining marks stay
    /// with their base char.
    ///
    /// This is the wrap-mode counterpart to [`Self::apply_horizontal_scroll`].
    /// We deliberately avoid ratatui's word-wrap (`Wrap { trim: false }`) so the
    /// on-screen cell→character mapping is exact and shared with the selection
    /// mapping: row boundaries come from [`wrap_row_starts_widths`], the same
    /// greedy packing used by [`Self::line_wrapped_row_count`] and
    /// `SelectionRow::locate`. This is what makes character-precise
    /// drag-selection possible.
    ///
    /// An empty (or whitespace-flattened-to-empty) line yields a single empty
    /// row so the logical line still occupies one terminal row, matching
    /// `line_wrapped_row_count`'s minimum of 1.
    fn wrap_line_chars(line: &Line, width: usize) -> Vec<Line<'static>> {
        // Degenerate width: nothing can be laid out; emit one empty row.
        if width == 0 {
            return vec![Line::from(String::new())];
        }

        // Flatten to (char, style) pairs (same approach as apply_horizontal_scroll).
        let mut chars: Vec<(char, Style)> = Vec::new();
        for span in &line.spans {
            let style = span.style;
            for c in span.content.chars() {
                chars.push((c, style));
            }
        }

        if chars.is_empty() {
            return vec![Line::from(String::new())];
        }

        let starts =
            wrap_row_starts_widths(chars.iter().map(|&(c, _)| char_display_width(c)), width);
        let mut rows: Vec<Line<'static>> = Vec::with_capacity(starts.len());
        for (k, &start) in starts.iter().enumerate() {
            let end = starts.get(k + 1).copied().unwrap_or(chars.len());
            let chunk = &chars[start..end];
            // Group consecutive chars sharing a style into spans.
            let mut spans: Vec<Span<'static>> = Vec::new();
            let mut current_style = chunk[0].1;
            let mut current_text = String::new();
            for &(c, style) in chunk {
                if style == current_style {
                    current_text.push(c);
                } else {
                    spans.push(Span::styled(
                        std::mem::take(&mut current_text),
                        current_style,
                    ));
                    current_style = style;
                    current_text.push(c);
                }
            }
            if !current_text.is_empty() {
                spans.push(Span::styled(current_text, current_style));
            }
            rows.push(Line::from(spans));
        }
        rows
    }

    /// Paint the drag-selection background over the already-rendered content.
    ///
    /// Reads the per-frame `selection` + `selection_rows` published earlier in
    /// this render and sets [`palette::SELECTION_BG`] on every selected cell.
    /// Operates purely on visible rows; off-screen parts of a selection are
    /// simply not drawn (they are reconstructed from the log model at copy time).
    fn render_selection_highlight(state: &LogViewState, buf: &mut Buffer) {
        let Some(sel) = state.selection else {
            return;
        };
        if !sel.is_nonempty() {
            return;
        }
        let (start, end) = sel.ordered();
        let start_key = start.line_key();
        let end_key = end.line_key();
        let sel_style = Style::default().bg(palette::SELECTION_BG);

        for row in &state.selection_rows {
            let key = row.line_key();
            if key < start_key || key > end_key {
                continue;
            }
            // The selected column span on THIS logical line: whole line for the
            // interior, clamped to the anchor/focus columns on the boundary lines.
            let lo = if key == start_key { start.col } else { 0 };
            let hi = if key == end_key {
                end.col
            } else {
                row.text_len
            };
            if lo >= hi {
                continue;
            }

            if row.wrap_width > 0 {
                // Wrap mode: sub-row boundaries and per-char cell positions come
                // from the same display-width packing the renderer used, so the
                // highlight tracks wide (2-cell) glyphs exactly.
                let w = row.wrap_width as usize;
                let starts = wrap_row_starts(&row.text, w);
                let height = row.rect.height as usize;
                let x_end = row.rect.x.saturating_add(row.rect.width);
                for k in 0..height {
                    let sub = row.top_clip + k;
                    let Some(&row_lo) = starts.get(sub) else {
                        break; // no characters on this or any later sub-row
                    };
                    let row_hi = starts.get(sub + 1).copied().unwrap_or(row.text_len);
                    let a = lo.max(row_lo);
                    let b = hi.min(row_hi);
                    if a >= b {
                        continue;
                    }
                    let y = row.rect.y + k as u16;
                    // Walk the sub-row's chars, accumulating display width to
                    // find each selected char's cells (wide chars cover 2).
                    let mut x = row.rect.x;
                    for (i, ch) in row
                        .text
                        .chars()
                        .enumerate()
                        .skip(row_lo)
                        .take(row_hi - row_lo)
                    {
                        let cw = char_display_width(ch) as u16;
                        if i >= b || x >= x_end {
                            break;
                        }
                        if i >= a {
                            for d in 0..cw {
                                if let Some(cell) = buf.cell_mut((x + d, y)) {
                                    cell.set_style(sel_style);
                                }
                            }
                        }
                        x = x.saturating_add(cw);
                    }
                }
            } else {
                // No-wrap mode: a single screen row; `←`/`→` indicators occupy
                // the first/last columns when the line is scrolled/overflows,
                // and must not be painted as selected content.
                let left = row.left_indicator as usize;
                let right = row.right_indicator as usize;
                let cap = (row.rect.width as usize).saturating_sub(left + right);
                let row_lo = row.base_col;
                let row_hi = (row.base_col + cap).min(row.text_len);
                let a = lo.max(row_lo);
                let b = hi.min(row_hi);
                if a >= b {
                    continue;
                }
                let y = row.rect.y;
                for c in a..b {
                    let x = row.rect.x + (left + (c - row.base_col)) as u16;
                    if let Some(cell) = buf.cell_mut((x, y)) {
                        cell.set_style(sel_style);
                    }
                }
            }
        }
    }

    /// Reconstruct the full WYSIWYG text of a drag-selection.
    ///
    /// Re-runs the exact line-format functions used for rendering (so the copied
    /// text always matches what is on screen — zero drift), enumerating the same
    /// logical lines as the render loop (message line + visible stack frames,
    /// respecting expand/collapse). Lines outside the viewport are included too,
    /// so copy stays correct after edge auto-scroll. Lines are joined with `\n`.
    ///
    /// `start`/`end` must already be in document order.
    fn selection_text(&self, filtered_indices: &[usize], start: SelPoint, end: SelPoint) -> String {
        let start_key = start.line_key();
        let end_key = end.line_key();
        let mut out: Vec<String> = Vec::new();

        for &idx in filtered_indices {
            let entry = &self.logs[idx];
            // Entries are ordered by ascending id; bound the scan to [start, end].
            if entry.id > end.entry_id {
                break;
            }
            if entry.id < start.entry_id {
                continue;
            }

            // Message line — line key (id, 0).
            let msg_key = (entry.id, 0usize);
            if msg_key >= start_key && msg_key <= end_key {
                let line = self.format_entry(entry, idx);
                out.push(Self::slice_line(&line, start, end, msg_key));
            }

            // Stack frames, matching the render loop's expand/collapse rules.
            if let Some(trace) = &entry.stack_trace {
                let count = if self.is_entry_expanded(entry) {
                    trace.frames.len()
                } else {
                    self.max_collapsed_frames.min(trace.frames.len())
                };
                for (fi, frame) in trace.frames.iter().take(count).enumerate() {
                    let key = (entry.id, fi + 1);
                    if key < start_key {
                        continue;
                    }
                    if key > end_key {
                        break;
                    }
                    let line = self.format_stack_frame_line_with_links(frame, idx, fi);
                    out.push(Self::slice_line(&line, start, end, key));
                }
            }
        }

        out.join("\n")
    }

    /// Concatenate a rendered line's span text and slice it to the columns
    /// selected on this logical line, given the line's identity `key`.
    fn slice_line(line: &Line, start: SelPoint, end: SelPoint, key: (u64, usize)) -> String {
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        let len = text.chars().count();
        let lo = if key == start.line_key() {
            start.col
        } else {
            0
        }
        .min(len);
        let hi = if key == end.line_key() { end.col } else { len }.min(len);
        if lo >= hi {
            return String::new();
        }
        text.chars().skip(lo).take(hi - lo).collect()
    }
}

/// Per-row metadata accumulated during the render loop for mouse-region recording.
///
/// Collected in `render_inner` when a `MouseCtx` is present, then converted to
/// click regions after all lines have been placed.
struct RowAction {
    /// Y position relative to `content_area.y` (0 = first content row).
    rel_y: u16,
    /// Height in terminal rows (1 in nowrap mode; `line_wrapped_row_count` in wrap mode).
    height: u16,
    /// `LogEntry::id` of the entry this row belongs to.
    entry_id: u64,
    /// `None` for the message line; `Some(i)` for stack frame `i`.
    frame_index: Option<usize>,
    /// Full character length of this logical line's rendered text (the
    /// concatenation of its span contents). Used to bound drag-selection columns.
    text_len: usize,
    /// Full rendered text of the logical line (wrap mode only; empty in no-wrap
    /// mode). Carried into `SelectionRow` for display-width-aware cell→char
    /// mapping of wide (CJK/emoji) characters.
    text: String,
}

/// Per-badge metadata accumulated during the render loop for link-badge region recording.
///
/// Collected when `link_highlight_state.is_active()` and a `MouseCtx` is present.
/// Each badge is a 3-cell `[<shortcut>]` span; its click region emits
/// `Message::SelectLink(shortcut)` at `z_index = 0`.
struct BadgeAction {
    /// Y position relative to `content_area.y` (same coordinate as `RowAction::rel_y`).
    rel_y: u16,
    /// X column offset from the start of the line (0 = leftmost cell of the content area).
    /// In nowrap mode, subtract `h_offset` and skip if the badge is off-screen left.
    col_offset: u16,
    /// The shortcut character embedded in the badge (`[<shortcut>]`).
    shortcut: char,
}

impl<'a> LogView<'a> {
    /// Core rendering implementation shared by [`StatefulWidget::render`] and
    /// [`render_with_regions`].
    ///
    /// When `mouse_ctx` is `Some`, the function additionally records one
    /// [`MouseAction::Emit(Message::ClickLogRow { .. })`] region per visible
    /// row in the content area. When `mouse_ctx` is `None` the behaviour is
    /// identical to the original `render` body — no allocations are made for
    /// region tracking.
    fn render_inner(
        self,
        area: Rect,
        buf: &mut Buffer,
        state: &mut LogViewState,
        mut mouse_ctx: Option<&mut MouseCtx<'_>>,
    ) {
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

        // Create glass container with rounded borders
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(palette::BORDER_DIM))
            .style(Style::default().bg(palette::CARD_BG));

        let inner = block.inner(area);
        block.render(area, buf);

        // Render top metadata bar in first line of inner area
        if inner.height > 0 {
            self.render_metadata_bar(Rect::new(inner.x, inner.y, inner.width, 1), buf);
        }

        // Determine if we have a bottom metadata bar
        let has_footer = self.status_info.is_some();
        let footer_height = if has_footer && inner.height > 1 { 1 } else { 0 };

        // Render bottom metadata bar (if status_info is present)
        if let Some(ref status) = self.status_info {
            if inner.height > 1 {
                // Check for compact mode
                let compact = area.width < MIN_FULL_STATUS_WIDTH;
                let meta_bottom = Rect::new(
                    inner.x,
                    inner.y + inner.height.saturating_sub(1),
                    inner.width,
                    1,
                );
                Self::render_bottom_metadata(meta_bottom, buf, status, compact, &self.icons);
            }
        }

        // Content area: between top and bottom metadata bars (with 1-line gap on each side)
        let top_gap = 1; // 1-line gap after top metadata bar
        let bottom_gap = if has_footer { 1 } else { 0 }; // 1-line gap before bottom metadata bar
        let content_area = Rect::new(
            inner.x,
            inner.y.saturating_add(1 + top_gap),
            inner.width,
            inner
                .height
                .saturating_sub(1 + top_gap + footer_height + bottom_gap),
        );

        let visible_width = content_area.width as usize;
        let visible_lines = content_area.height as usize;

        // Calculate total lines including stack traces (accounting for collapse state).
        // In wrap mode, total_lines counts terminal rows (wrapped); in nowrap, logical lines.
        let total_lines: usize = if self.wrap_mode {
            filtered_indices
                .iter()
                .map(|&idx| self.calculate_entry_display_rows(&self.logs[idx], visible_width))
                .sum()
        } else {
            filtered_indices
                .iter()
                .map(|&idx| self.calculate_entry_lines(&self.logs[idx]))
                .sum()
        };

        // Update state with content dimensions
        state.update_content_size(total_lines, visible_lines);

        // Build a flat list of all lines (entry messages + stack frames)
        // We need to skip `offset` units and take `visible_lines` units.
        // In wrap mode, units are terminal rows; in nowrap, logical lines.
        let mut all_lines: Vec<Line> = Vec::new();
        // Parallel list tracking entry identity and position for click-region recording.
        // Only populated when `mouse_ctx` is `Some` (no allocation otherwise).
        let mut row_actions: Vec<RowAction> = Vec::new();
        // Badge-region actions (Phase 5 Task 08): one entry per link badge visible in this frame.
        // Populated only when `mouse_ctx` is `Some` AND `link_highlight_state.is_active()`.
        let mut badge_actions: Vec<BadgeAction> = Vec::new();
        // Running Y cursor for row_actions (relative to content_area.y).
        let mut rel_y_cursor: u16 = 0;
        let mut units_added = 0;
        let mut units_skipped = 0;
        // In wrap mode, tracks how many terminal rows to scroll past at the top
        // of the first visible entry (handled by Paragraph::scroll)
        let mut wrap_intra_offset: usize = 0;

        // Track focus info for the first visible line (Phase 3 Task 03)
        let mut focus_captured = false;

        // Gate flag: avoids repeating `mouse_ctx.is_some()` at each call site.
        // A Rust closure cannot be used here because it would exclusively borrow
        // `rel_y_cursor` and `row_actions`, preventing reads of `rel_y_cursor` at
        // call sites and the direct advance for the collapsed-indicator row.
        let has_mouse_ctx = mouse_ctx.is_some();
        // Gate flag: badge-region recording fires only in link-highlight mode.
        let has_link_badges =
            has_mouse_ctx && self.link_highlight_state.is_some_and(|s| s.is_active());

        for &idx in &filtered_indices {
            let entry = &self.logs[idx];
            let entry_units = if self.wrap_mode {
                self.calculate_entry_display_rows(entry, visible_width)
            } else {
                self.calculate_entry_lines(entry)
            };

            // Skip entries that are entirely before the offset
            if units_skipped + entry_units <= state.offset {
                units_skipped += entry_units;
                continue;
            }

            // In wrap mode, collect enough to fill visible_lines + intra_offset;
            // in nowrap mode, collect visible_lines logical lines
            let target = if self.wrap_mode {
                visible_lines + wrap_intra_offset
            } else {
                visible_lines
            };
            if units_added >= target {
                break;
            }

            // In wrap mode, don't skip logical lines within an entry —
            // include all lines and use Paragraph::scroll for the row offset.
            // In nowrap mode, skip logical lines as before.
            let skip_in_entry = if self.wrap_mode {
                // Compute intra-offset for the first visible entry
                if units_skipped < state.offset {
                    wrap_intra_offset = state.offset - units_skipped;
                }
                0
            } else {
                state.offset.saturating_sub(units_skipped)
            };

            // Add the main log line if not skipped
            if skip_in_entry == 0 {
                // Track focus if this is the first visible line
                if !focus_captured {
                    state.focus_info.entry_index = Some(idx);
                    state.focus_info.entry_id = Some(entry.id);
                    state.focus_info.frame_index = None;
                    focus_captured = true;
                }

                let line = self.format_entry(entry, idx);
                let text_len = Self::line_width(&line);
                let row_h: u16 = if self.wrap_mode {
                    let wrc = Self::line_wrapped_row_count(&line, visible_width) as u16;
                    units_added += wrc as usize;
                    wrc
                } else {
                    units_added += 1;
                    1u16
                };
                if has_mouse_ctx {
                    // Collect badge regions (Phase 5 Task 08) before advancing rel_y_cursor.
                    if has_link_badges {
                        Self::collect_badge_actions(&line.spans, rel_y_cursor, &mut badge_actions);
                    }
                    row_actions.push(RowAction {
                        rel_y: rel_y_cursor,
                        height: row_h,
                        entry_id: entry.id,
                        frame_index: None,
                        text_len,
                        text: if self.wrap_mode {
                            Self::line_text(&line)
                        } else {
                            String::new()
                        },
                    });
                    rel_y_cursor = rel_y_cursor.saturating_add(row_h);
                }
                all_lines.push(line);
            }

            // Add stack trace frames (respecting collapse state)
            if let Some(trace) = &entry.stack_trace {
                let is_expanded = self.is_entry_expanded(entry);
                let frame_count = trace.frames.len();

                if is_expanded {
                    // Expanded: show all frames
                    for (frame_idx, frame) in trace.frames.iter().enumerate() {
                        let target = if self.wrap_mode {
                            visible_lines + wrap_intra_offset
                        } else {
                            visible_lines
                        };
                        if units_added >= target {
                            break;
                        }

                        // Skip frames if we're starting mid-entry (nowrap only)
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
                        let line = self.format_stack_frame_line_with_links(frame, idx, frame_idx);
                        let text_len = Self::line_width(&line);
                        let row_h: u16 = if self.wrap_mode {
                            let wrc = Self::line_wrapped_row_count(&line, visible_width) as u16;
                            units_added += wrc as usize;
                            wrc
                        } else {
                            units_added += 1;
                            1u16
                        };
                        if has_mouse_ctx {
                            // Collect badge regions (Phase 5 Task 08) before advancing rel_y_cursor.
                            if has_link_badges {
                                Self::collect_badge_actions(
                                    &line.spans,
                                    rel_y_cursor,
                                    &mut badge_actions,
                                );
                            }
                            row_actions.push(RowAction {
                                rel_y: rel_y_cursor,
                                height: row_h,
                                entry_id: entry.id,
                                frame_index: Some(frame_idx),
                                text_len,
                                text: if self.wrap_mode {
                                    Self::line_text(&line)
                                } else {
                                    String::new()
                                },
                            });
                            rel_y_cursor = rel_y_cursor.saturating_add(row_h);
                        }
                        all_lines.push(line);
                    }
                } else {
                    // Collapsed: show max_collapsed_frames + indicator if more
                    let visible_count = self.max_collapsed_frames.min(frame_count);
                    let hidden_count = frame_count.saturating_sub(self.max_collapsed_frames);

                    for (frame_idx, frame) in trace.frames.iter().take(visible_count).enumerate() {
                        let target = if self.wrap_mode {
                            visible_lines + wrap_intra_offset
                        } else {
                            visible_lines
                        };
                        if units_added >= target {
                            break;
                        }

                        // Skip frames if we're starting mid-entry (nowrap only)
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
                        let line = self.format_stack_frame_line_with_links(frame, idx, frame_idx);
                        let text_len = Self::line_width(&line);
                        let row_h: u16 = if self.wrap_mode {
                            let wrc = Self::line_wrapped_row_count(&line, visible_width) as u16;
                            units_added += wrc as usize;
                            wrc
                        } else {
                            units_added += 1;
                            1u16
                        };
                        if has_mouse_ctx {
                            // Collect badge regions (Phase 5 Task 08) before advancing rel_y_cursor.
                            if has_link_badges {
                                Self::collect_badge_actions(
                                    &line.spans,
                                    rel_y_cursor,
                                    &mut badge_actions,
                                );
                            }
                            row_actions.push(RowAction {
                                rel_y: rel_y_cursor,
                                height: row_h,
                                entry_id: entry.id,
                                frame_index: Some(frame_idx),
                                text_len,
                                text: if self.wrap_mode {
                                    Self::line_text(&line)
                                } else {
                                    String::new()
                                },
                            });
                            rel_y_cursor = rel_y_cursor.saturating_add(row_h);
                        }
                        all_lines.push(line);
                    }

                    // Add collapsed indicator if there are hidden frames.
                    // The indicator row is not a clickable entry — skip region recording for it.
                    let target = if self.wrap_mode {
                        visible_lines + wrap_intra_offset
                    } else {
                        visible_lines
                    };
                    if hidden_count > 0 && units_added < target {
                        let indicator_position = 1 + visible_count;
                        if indicator_position > skip_in_entry {
                            all_lines.push(Self::format_collapsed_indicator(hidden_count));
                            units_added += 1; // collapsed indicator is always short
                                              // Advance rel_y_cursor so subsequent rows are placed correctly.
                            if has_mouse_ctx {
                                rel_y_cursor = rel_y_cursor.saturating_add(1);
                            }
                        }
                    }
                }
            }

            units_skipped += entry_units;
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

        // Update horizontal dimensions in state
        state.update_horizontal_size(max_line_width, visible_width);

        // Build final lines: in wrap mode skip horizontal scroll, in nowrap apply it
        let final_lines_base: Vec<Line> = if self.wrap_mode {
            // Wrap mode: pass raw lines directly to ratatui's wrapping paragraph
            all_lines
        } else {
            // No-wrap mode: apply horizontal scroll truncation as before
            all_lines
                .into_iter()
                .map(|line| Self::apply_horizontal_scroll(line, state.h_offset, visible_width))
                .collect()
        };

        // Add blinking cursor at end if auto-scroll is active.
        // The cursor is not a log entry — do not register a click region for it.
        let mut final_lines = final_lines_base;
        if state.auto_scroll && !final_lines.is_empty() {
            // Add cursor to a new line after the last entry
            let cursor_line = Line::from(vec![Span::styled(
                "█",
                Style::default()
                    .fg(palette::ACCENT)
                    .add_modifier(Modifier::SLOW_BLINK),
            )]);
            final_lines.push(cursor_line);
        }

        // Render log content. Wrap mode now uses explicit character-wrapping
        // (instead of ratatui's word-wrap) so the cell→char mapping is exact for
        // drag-selection; we expand each logical line into screen rows, then drop
        // the `wrap_intra_offset` rows that belong to the partially-scrolled top
        // entry (replacing the old `Paragraph::scroll`). No-wrap uses truncation.
        if self.wrap_mode {
            let width = content_area.width as usize;
            let mut rows: Vec<Line> = Vec::new();
            for line in &final_lines {
                rows.extend(Self::wrap_line_chars(line, width));
            }
            let rows: Vec<Line> = rows.into_iter().skip(wrap_intra_offset).collect();
            Paragraph::new(rows).render(content_area, buf);
        } else {
            Paragraph::new(final_lines).render(content_area, buf);
        }

        // Jump-to-latest indicator (Phase 4, Task 02). Only visible when the
        // user is scrolled away from the tail AND new logs have arrived since.
        // Rendered after the Paragraph (so it overlays the last log row's tail)
        // and before the scrollbar (so the scrollbar can still draw on the
        // rightmost column).
        if !state.auto_scroll && self.unseen_log_count > 0 {
            render_jump_to_latest_pill(
                content_area,
                buf,
                self.unseen_log_count,
                mouse_ctx.as_deref_mut(),
            );
        }

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

        // Register click regions for all visible rows (Phase 4 Task 06) and
        // link-highlight badge regions (Phase 5 Task 08).
        // Only executed when a MouseCtx was provided; no-op for the plain render path.
        if let Some(ctx) = mouse_ctx {
            use fdemon_app::message::Message;
            use fdemon_app::{MouseAction, MouseRect};

            // In wrap mode, `r.rel_y` is in "all_lines space" (accumulated from the
            // first row pushed, which may be the partially-scrolled entry at the top).
            // We render with explicit char-wrapping and drop `wrap_intra_offset` rows,
            // so we subtract `wrap_intra_offset` to convert to screen space.
            let wio = wrap_intra_offset as u16;

            // Drag-selection mapping rows (parallel to the click regions). Built
            // here because this is where screen rects + top/bottom clipping are known.
            let mut selection_rows: Vec<SelectionRow> = Vec::with_capacity(row_actions.len());
            let h_offset = state.h_offset;

            for r in row_actions {
                // Skip rows fully scrolled off the top (entirely above the viewport).
                if r.rel_y.saturating_add(r.height) <= wio {
                    continue;
                }

                // Top-clip: rows partially scrolled off the top.
                // `top_clip` is the number of rows of this entry that are above the viewport.
                let top_clip = wio.saturating_sub(r.rel_y);
                // Convert from all_lines space to screen space.
                let visible_y = r.rel_y.saturating_sub(wio);
                let visible_h = r.height.saturating_sub(top_clip);

                // Skip rows fully below the content area (bottom overflow).
                if visible_y >= content_area.height {
                    continue;
                }

                // Bottom-clip: rows partially below the content area.
                let h = visible_h.min(content_area.height.saturating_sub(visible_y));
                if h == 0 {
                    continue;
                }

                let rect = MouseRect::new(
                    content_area.x,
                    content_area.y.saturating_add(visible_y),
                    content_area.width,
                    h,
                );
                // `MouseRegionsBuilder::click` already skips zero-sized rects, but
                // guard here for clarity (content_area.width may be 0 at narrow widths).
                if rect.width == 0 || rect.height == 0 {
                    continue;
                }
                ctx.click(
                    rect,
                    MouseAction::emit(Message::ClickLogRow {
                        entry_id: r.entry_id,
                        frame_index: r.frame_index,
                    }),
                );

                // Parallel selection-mapping row. Wrap mode carries the line's
                // full text plus `top_clip` so `SelectionRow::locate` can
                // recompute the display-width-aware sub-row boundaries. No-wrap
                // mode stays char-cell based: `apply_horizontal_scroll` draws
                // the `←` indicator IN PLACE OF the char at index `h_offset`
                // and starts content at `h_offset + 1`, so the first visible
                // char (`base_col`) must account for the indicator cell; a `→`
                // indicator likewise replaces the last cell when the line
                // continues past the right edge.
                let (base_col, left_indicator, right_indicator, wrap_width, top_clip_rows, text) =
                    if self.wrap_mode {
                        (
                            0,
                            false,
                            false,
                            content_area.width,
                            top_clip as usize,
                            r.text,
                        )
                    } else {
                        let left = h_offset > 0;
                        let right = h_offset + visible_width < r.text_len;
                        (h_offset + left as usize, left, right, 0, 0, String::new())
                    };
                selection_rows.push(SelectionRow {
                    rect,
                    entry_id: r.entry_id,
                    frame_index: r.frame_index,
                    base_col,
                    left_indicator,
                    right_indicator,
                    text_len: r.text_len,
                    wrap_width,
                    top_clip: top_clip_rows,
                    text,
                });
            }

            // Publish render-derived selection data for the mouse handler and the
            // highlight pass (mirrors how `focus_info`/`total_lines` are published).
            //
            // EXCEPTION (TEA): render-hint write-back onto `&mut LogViewState` —
            // see docs/CODE_STANDARDS.md Principle 3 and docs/REVIEW_FOCUS.md
            // "Current usage" ("LogViewState drag-selection geometry fields" and
            // "LogViewState::selection_text"). The geometry fields are per-frame
            // layout hints; `selection_text` is the WYSIWYG clipboard cache whose
            // soundness rests on the event loop rendering before each input event
            // is read (the registry entry spells out the invariant).
            state.content_top_y = content_area.y;
            state.content_bottom_y = content_area.y.saturating_add(content_area.height);
            state.selection_top = selection_rows.first().map(|r| SelectionEdge {
                entry_id: r.entry_id,
                frame_index: r.frame_index,
                text_len: r.text_len,
            });
            state.selection_bottom = selection_rows.last().map(|r| SelectionEdge {
                entry_id: r.entry_id,
                frame_index: r.frame_index,
                text_len: r.text_len,
            });
            state.selection_rows = selection_rows;

            // Recompute the WYSIWYG selection text, but only when the selection
            // actually changed (cheap cache key) — the copy handler reads this.
            match state.selection {
                Some(sel) if sel.is_nonempty() => {
                    if state.selection_text_key != Some(sel) {
                        let (s, e) = sel.ordered();
                        state.selection_text = Some(self.selection_text(&filtered_indices, s, e));
                        state.selection_text_key = Some(sel);
                    }
                }
                _ => {
                    state.selection_text = None;
                    state.selection_text_key = None;
                }
            }

            // Paint the selection highlight over the freshly-rendered content.
            Self::render_selection_highlight(state, buf);

            // Phase 5 Task 08: register one SelectLink region per visible badge.
            //
            // Badges are pushed *after* row regions, so they win over row regions on
            // overlapping cells (last-pushed-wins at equal z_index — see mouse_regions.rs).
            // z_index = 0: link mode is in-place, not modal.
            for b in &badge_actions {
                // Compute the screen-space (dx, dy) of the badge.
                //
                // In nowrap mode `col_offset` maps directly to x (minus h_offset).
                // In wrap mode `col_offset` is an absolute character position within
                // the unwrapped line; when it exceeds `visible_width` the badge renders
                // on a subsequent wrapped sub-row. Convert with modular arithmetic:
                //   dx = col_offset % visible_width  (x within the sub-row)
                //   dy = col_offset / visible_width  (extra rows from wrapping)
                // The badge's "all_lines-space" row is then `b.rel_y + dy`.
                let (dx, dy) = if self.wrap_mode && content_area.width > 0 {
                    let vw = content_area.width as usize;
                    let dx = (b.col_offset as usize % vw) as u16;
                    let dy = (b.col_offset as usize / vw) as u16;
                    (dx, dy)
                } else {
                    // Nowrap mode: dy is always 0; dx is handled below with h_offset.
                    (b.col_offset, 0u16)
                };

                // The badge's row in "all_lines space" (incorporating wrap sub-row offset).
                let badge_all_lines_y = b.rel_y.saturating_add(dy);

                // Apply the same top-clip / bottom-clip logic as for row_actions.
                if badge_all_lines_y.saturating_add(1) <= wio {
                    continue; // entirely above the viewport
                }
                let visible_y = badge_all_lines_y.saturating_sub(wio);
                if visible_y >= content_area.height {
                    continue; // below the content area
                }

                // Compute the rendered x of the badge in the buffer.
                // In nowrap mode, horizontal scroll shifts the line left by `h_offset`.
                // Skip badges that are entirely scrolled off the left edge.
                let badge_x = if self.wrap_mode {
                    // Wrap mode: no horizontal scroll; dx is already the sub-row position.
                    content_area.x.saturating_add(dx)
                } else {
                    // Nowrap mode: account for horizontal scroll offset.
                    let h_off = state.h_offset as u16;
                    if dx < h_off {
                        continue; // badge start is off-screen left
                    }
                    let local_x = dx - h_off;
                    if local_x >= content_area.width {
                        continue; // badge start is off-screen right
                    }
                    content_area.x.saturating_add(local_x)
                };

                // Badge is always 3 cells wide and 1 cell tall.
                // Clip to content area width (a badge at the very right edge may overflow).
                let badge_w = 3u16.min(
                    content_area
                        .x
                        .saturating_add(content_area.width)
                        .saturating_sub(badge_x),
                );
                if badge_w == 0 {
                    continue;
                }

                let rect = MouseRect::new(
                    badge_x,
                    content_area.y.saturating_add(visible_y),
                    badge_w,
                    1,
                );
                ctx.click(rect, MouseAction::emit(Message::SelectLink(b.shortcut)));
            }
        }
    }
}

/// Maximum exact count rendered in the jump-to-latest pill. Counts above this
/// display as `"999+"`. Keeps the pill width bounded for layout sanity even
/// after a long unattended scroll-away.
const JUMP_HINT_MAX_DISPLAY: usize = 999;

/// Static suffix advertising the keybinding. Middle-dot separator is used over
/// em-dash for narrower terminals (see planning notes in `TASKS.md`).
const JUMP_HINT_SUFFIX: &str = " · G to jump";

/// Down-arrow glyph + a single space prefix.
const JUMP_HINT_PREFIX: &str = "↓ ";

/// Render a floating right-aligned `↓ N new · G to jump` pill on the last row
/// of `content_area`. The pill overwrites whatever the Paragraph rendered there
/// (no `Clear` needed — cells are overwritten with the pill's own background).
///
/// Registers a click region that emits `Message::ScrollToBottom` when a
/// `MouseCtx` is provided.
///
/// # Suppression conditions
/// - `content_area.height == 0`: nothing to render.
/// - `content_area.width < pill_width + 1`: pill does not fit cleanly with a
///   1-column right margin; suppress rather than truncate (the truncated form
///   would hide the keybinding, defeating discoverability).
fn render_jump_to_latest_pill(
    content_area: Rect,
    buf: &mut Buffer,
    unseen: usize,
    mouse_ctx: Option<&mut MouseCtx<'_>>,
) {
    use fdemon_app::message::Message;
    use fdemon_app::{MouseAction, MouseRect};

    if content_area.height == 0 {
        return;
    }

    let display_count = if unseen > JUMP_HINT_MAX_DISPLAY {
        format!("{JUMP_HINT_MAX_DISPLAY}+")
    } else {
        unseen.to_string()
    };
    let label = format!("{JUMP_HINT_PREFIX}{display_count} new{JUMP_HINT_SUFFIX}");

    // Width is character-count of the label (all chars are single-column in
    // standard monospace: the down-arrow `↓` and middle-dot `·` are both 1 col).
    let pill_width = label.chars().count() as u16;

    // Narrow-terminal fallback: skip the pill if it doesn't fit cleanly with
    // a 1-column right margin. Better to suppress than to truncate — truncating
    // would hide the keybinding, defeating the discoverability purpose.
    let min_required = pill_width.saturating_add(1);
    if content_area.width < min_required {
        return;
    }

    let y = content_area
        .y
        .saturating_add(content_area.height)
        .saturating_sub(1);
    let x = content_area
        .x
        .saturating_add(content_area.width)
        .saturating_sub(pill_width)
        .saturating_sub(1); // 1-col right margin

    let pill_style = ratatui::style::Style::default()
        .fg(styles::JUMP_HINT_FG)
        .bg(styles::JUMP_HINT_BG);

    let line = Line::from(vec![Span::styled(label, pill_style)]);
    buf.set_line(x, y, &line, pill_width);

    // Mouse routing: clicking the pill emits Message::ScrollToBottom.
    // Registered at z=1 so it wins over the z=0 per-row ClickLogRow region that
    // also covers the pill's cell (hit_test is max-by (z_index, push_index)).
    if let Some(ctx) = mouse_ctx {
        let rect = MouseRect::new(x, y, pill_width, 1);
        ctx.click_at_z(rect, MouseAction::emit(Message::ScrollToBottom), 1);
    }
}

impl<'a> StatefulWidget for LogView<'a> {
    type State = LogViewState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        self.render_inner(area, buf, state, None);
    }
}

// Non-stateful version for simple rendering
impl Widget for LogView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut state = LogViewState::new();
        StatefulWidget::render(self, area, buf, &mut state);
    }
}

/// Render the log view, optionally recording clickable regions.
///
/// This is the click-aware entry point used by `render::view`. The
/// `StatefulWidget::render` impl does not record regions; this function is the
/// canonical path for region-aware rendering.
///
/// Passing `None` for `ctx` makes this function behave identically to calling
/// `frame.render_stateful_widget(view, area, state)` directly — no regions are
/// recorded and no additional allocations are made.
///
/// When `ctx` is `Some`, one [`fdemon_app::message::Message::ClickLogRow`]
/// region is registered per visible row in the content area (Phase 4 Task 06).
pub fn render_with_regions(
    area: Rect,
    buf: &mut Buffer,
    state: &mut LogViewState,
    view: LogView<'_>,
    ctx: Option<&mut MouseCtx<'_>>,
) {
    view.render_inner(area, buf, state, ctx);
}

#[cfg(test)]
mod tests;
