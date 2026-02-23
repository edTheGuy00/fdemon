//! Log view state - scroll position, viewport bounds, and focus tracking.
//!
//! This module defines the state types used by both the app handler layer
//! (for scroll commands) and the TUI layer (for rendering the log view).

use std::collections::VecDeque;

use fdemon_core::LogEntry;

/// Default buffer lines for virtualized rendering
const DEFAULT_BUFFER_LINES: usize = 10;

// ─────────────────────────────────────────────────────────────────────────────
// FocusInfo
// ─────────────────────────────────────────────────────────────────────────────

/// Information about the currently focused element in the log view.
///
/// Updated during render to track which log entry and optional stack frame
/// is at the "focus" position (top of visible area).
/// Note: file_ref removed in Phase 3.1 - link detection now happens in link highlight mode.
#[derive(Debug, Default, Clone)]
pub struct FocusInfo {
    /// Index of the focused entry in the log buffer
    pub entry_index: Option<usize>,
    /// ID of the focused entry (for stability across buffer changes)
    pub entry_id: Option<u64>,
    /// Index of the focused frame within a stack trace (if applicable)
    pub frame_index: Option<usize>,
}

impl FocusInfo {
    /// Create a new empty focus info
    pub fn new() -> Self {
        Self::default()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// LogViewState
// ─────────────────────────────────────────────────────────────────────────────

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
    /// Information about the currently focused element (Phase 3 Task 03)
    pub focus_info: FocusInfo,
    /// Whether line wrap is enabled. When true, horizontal scroll is a no-op.
    pub wrap_mode: bool,
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
            focus_info: FocusInfo::default(),
            wrap_mode: true,
        }
    }

    /// Toggle line wrap mode. When enabling wrap, resets horizontal offset to 0.
    pub fn toggle_wrap_mode(&mut self) {
        self.wrap_mode = !self.wrap_mode;
        if self.wrap_mode {
            self.h_offset = 0;
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

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // --- Wrap mode tests ---

    #[test]
    fn test_wrap_mode_defaults_to_true() {
        let state = LogViewState::new();
        assert!(state.wrap_mode);
    }

    #[test]
    fn test_toggle_wrap_mode_disables() {
        let mut state = LogViewState::new();
        assert!(state.wrap_mode); // default true
        state.toggle_wrap_mode();
        assert!(!state.wrap_mode);
    }

    #[test]
    fn test_toggle_wrap_mode_enables_and_resets_h_offset() {
        let mut state = LogViewState::new();
        state.wrap_mode = false;
        state.h_offset = 42; // simulate horizontal scroll position
        state.toggle_wrap_mode();
        assert!(state.wrap_mode);
        assert_eq!(
            state.h_offset, 0,
            "h_offset should reset to 0 when wrap enabled"
        );
    }

    #[test]
    fn test_toggle_wrap_mode_does_not_reset_h_offset_when_disabling() {
        let mut state = LogViewState::new();
        // wrap is on by default, h_offset should be 0
        state.toggle_wrap_mode(); // disable wrap
        assert!(!state.wrap_mode);
        // h_offset stays at whatever it was (0 in this case, but point is no reset)
        assert_eq!(state.h_offset, 0);
    }

    #[test]
    fn test_toggle_wrap_mode_roundtrip() {
        let mut state = LogViewState::new();
        assert!(state.wrap_mode); // start: wrap on
        state.toggle_wrap_mode(); // wrap off
        assert!(!state.wrap_mode);
        state.toggle_wrap_mode(); // wrap on again
        assert!(state.wrap_mode);
        assert_eq!(state.h_offset, 0);
    }

    #[test]
    fn test_wrap_mode_h_offset_unchanged_when_disabling_from_nonzero() {
        let mut state = LogViewState::new();
        // Enable nowrap, set some horizontal scroll
        state.wrap_mode = true;
        state.toggle_wrap_mode(); // now nowrap
        state.h_offset = 20;
        // Disabling again (re-enabling wrap) should reset h_offset
        state.toggle_wrap_mode(); // back to wrap
        assert!(state.wrap_mode);
        assert_eq!(state.h_offset, 0, "Re-enabling wrap resets h_offset");
    }
}
