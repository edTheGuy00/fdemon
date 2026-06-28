//! Log view state - scroll position, viewport bounds, and focus tracking.
//!
//! This module defines the state types used by both the app handler layer
//! (for scroll commands) and the TUI layer (for rendering the log view).

use std::collections::VecDeque;

use fdemon_core::LogEntry;

use crate::mouse_regions::MouseRect;

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
// Text selection (drag-to-select)
// ─────────────────────────────────────────────────────────────────────────────

/// A point in the log "document", anchored to a logical line's identity (so it
/// survives scrolling and new log arrivals) plus a character column.
///
/// `col` is a character (Unicode scalar) offset into the line's **full rendered
/// text** — i.e., the concatenation of the styled spans the log view draws for
/// that line, gutter included. Char offsets (not display columns) match the
/// widget's existing width math (`line_width` counts `chars()`); wide-character
/// display width is a pre-existing limitation and out of scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelPoint {
    /// [`fdemon_core::LogEntry::id`] of the entry this point falls in.
    pub entry_id: u64,
    /// `None` = the entry's message line; `Some(i)` = stack frame `i`.
    pub frame_index: Option<usize>,
    /// Character offset into the line's full rendered text.
    pub col: usize,
}

impl SelPoint {
    /// Line-identity key in document order: entry id, then message-line-before-
    /// frames (`None` sorts before any `Some(i)`, frames ascending).
    pub fn line_key(&self) -> (u64, usize) {
        (self.entry_id, frame_rank(self.frame_index))
    }

    /// Full document-order key (line identity then column).
    fn order_key(&self) -> (u64, usize, usize) {
        let (e, f) = self.line_key();
        (e, f, self.col)
    }

    /// Returns `(start, end)` of `a` and `b` in document order.
    pub fn ordered(a: SelPoint, b: SelPoint) -> (SelPoint, SelPoint) {
        if a.order_key() <= b.order_key() {
            (a, b)
        } else {
            (b, a)
        }
    }
}

/// Rank for ordering: the message line (`None`) precedes all stack frames.
fn frame_rank(frame_index: Option<usize>) -> usize {
    match frame_index {
        None => 0,
        Some(i) => i + 1,
    }
}

/// An active or just-completed drag-selection in the log view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogSelection {
    pub anchor: SelPoint,
    pub focus: SelPoint,
    /// `true` while the mouse button is held and the drag is in progress.
    pub dragging: bool,
}

impl LogSelection {
    /// Start a fresh single-point selection (a press with no drag yet).
    pub fn new(point: SelPoint) -> Self {
        Self {
            anchor: point,
            focus: point,
            dragging: true,
        }
    }

    /// `(start, end)` of the selection in document order.
    pub fn ordered(&self) -> (SelPoint, SelPoint) {
        SelPoint::ordered(self.anchor, self.focus)
    }

    /// True when the selection spans at least one cell (anchor != focus).
    pub fn is_nonempty(&self) -> bool {
        self.anchor != self.focus
    }
}

/// Render-published mapping from one visible logical line's screen rect to the
/// data needed to convert a pointer cell into a [`SelPoint`].
///
/// Populated each frame by the log-view renderer (alongside the click regions)
/// and consumed by the mouse handler ([`LogViewState::locate_selection_point`])
/// and the selection highlight pass. This keeps all fragile cell↔char mapping in
/// one place — the renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionRow {
    /// Screen rect of the (clipped) visible portion of the logical line.
    pub rect: MouseRect,
    pub entry_id: u64,
    pub frame_index: Option<usize>,
    /// Char offset of the first character of the first **visible** row of this
    /// logical line (wrap: `top_clip * wrap_width`; no-wrap: `h_offset`).
    pub base_col: usize,
    /// No-wrap only: a `←` indicator occupies the first column when scrolled.
    pub left_indicator: bool,
    /// Full character length of the logical line's rendered text.
    pub text_len: usize,
    /// Wrap width (content width) in wrap mode; `0` in no-wrap mode (each logical
    /// line is exactly one screen row).
    pub wrap_width: u16,
}

impl SelectionRow {
    /// Line-identity key, matching [`SelPoint::line_key`].
    pub fn line_key(&self) -> (u64, usize) {
        (self.entry_id, frame_rank(self.frame_index))
    }

    /// Map a pointer cell `(x, y)` within this row to a [`SelPoint`].
    /// Returns `None` when the cell is outside the row's rect.
    pub fn locate(&self, x: u16, y: u16) -> Option<SelPoint> {
        if !self.rect.contains(x, y) {
            return None;
        }
        let dx = (x - self.rect.x) as usize;
        let col = if self.wrap_width > 0 {
            // Wrap mode: which visible sub-row, then column within it.
            let sub_row = (y - self.rect.y) as usize;
            self.base_col + sub_row * self.wrap_width as usize + dx
        } else {
            // No-wrap mode: single row; discount the leading `←` indicator.
            self.base_col + dx.saturating_sub(self.left_indicator as usize)
        };
        Some(SelPoint {
            entry_id: self.entry_id,
            frame_index: self.frame_index,
            col: col.min(self.text_len),
        })
    }
}

/// Top/bottom visible edge line — used to extend the selection focus during
/// auto-scroll while the cursor is held beyond the viewport.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SelectionEdge {
    pub entry_id: u64,
    pub frame_index: Option<usize>,
    pub text_len: usize,
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
    /// Active drag-selection, or `None` when nothing is selected.
    pub selection: Option<LogSelection>,
    /// Per-frame screen→document mapping for visible logical lines.
    /// Render-published; consumed by the mouse handler and the highlight pass.
    pub selection_rows: Vec<SelectionRow>,
    /// Screen Y of the first content row (render-published; for edge auto-scroll).
    pub content_top_y: u16,
    /// Screen Y just past the last content row, exclusive (render-published).
    pub content_bottom_y: u16,
    /// First visible logical line this frame (render-published; upward auto-scroll).
    pub selection_top: Option<SelectionEdge>,
    /// Last visible logical line this frame (render-published; downward auto-scroll).
    pub selection_bottom: Option<SelectionEdge>,
    /// Auto-scroll direction while dragging past a viewport edge:
    /// `Some(-1)` up, `Some(1)` down, `None` when the cursor is inside the content.
    pub drag_autoscroll: Option<i8>,
    /// Render-published full text of the current non-empty selection (WYSIWYG,
    /// reconstructed by the renderer from the exact line-format functions). Read
    /// by the copy handler on release. `None` when there is no non-empty selection.
    pub selection_text: Option<String>,
    /// Cache key for `selection_text`: the selection it was computed for, so the
    /// renderer only rebuilds the string when the selection actually changes.
    pub selection_text_key: Option<LogSelection>,
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
            selection: None,
            selection_rows: Vec::new(),
            content_top_y: 0,
            content_bottom_y: 0,
            selection_top: None,
            selection_bottom: None,
            drag_autoscroll: None,
            selection_text: None,
            selection_text_key: None,
        }
    }

    /// Find the [`SelPoint`] at screen cell `(x, y)` using the rows published by
    /// the last render. Returns `None` when no visible logical line contains it.
    pub fn locate_selection_point(&self, x: u16, y: u16) -> Option<SelPoint> {
        self.selection_rows.iter().find_map(|r| r.locate(x, y))
    }

    /// Clear any active selection, drag state, and cached selection text.
    pub fn clear_selection(&mut self) {
        self.selection = None;
        self.drag_autoscroll = None;
        self.selection_text = None;
        self.selection_text_key = None;
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

    // ── Selection: SelPoint ordering ──────────────────────────────────────────

    fn sp(entry_id: u64, frame_index: Option<usize>, col: usize) -> SelPoint {
        SelPoint {
            entry_id,
            frame_index,
            col,
        }
    }

    #[test]
    fn selpoint_orders_by_column_on_same_line() {
        let a = sp(1, None, 5);
        let b = sp(1, None, 2);
        assert_eq!(SelPoint::ordered(a, b), (b, a));
        assert_eq!(SelPoint::ordered(b, a), (b, a));
    }

    #[test]
    fn selpoint_message_line_precedes_frames() {
        let msg = sp(1, None, 9);
        let frame0 = sp(1, Some(0), 0);
        assert_eq!(SelPoint::ordered(frame0, msg), (msg, frame0));
    }

    #[test]
    fn selpoint_frames_order_ascending() {
        let f0 = sp(1, Some(0), 3);
        let f1 = sp(1, Some(1), 0);
        assert_eq!(SelPoint::ordered(f1, f0), (f0, f1));
    }

    #[test]
    fn selpoint_orders_by_entry_id_first() {
        let later = sp(2, None, 0);
        let earlier = sp(1, Some(9), 99);
        assert_eq!(SelPoint::ordered(later, earlier), (earlier, later));
    }

    // ── Selection: SelectionRow::locate ───────────────────────────────────────

    fn wrap_row(text_len: usize) -> SelectionRow {
        SelectionRow {
            rect: MouseRect::new(2, 5, 10, 2),
            entry_id: 7,
            frame_index: None,
            base_col: 0,
            left_indicator: false,
            text_len,
            wrap_width: 10,
        }
    }

    #[test]
    fn locate_wrap_first_subrow_maps_column() {
        let row = wrap_row(25);
        assert_eq!(row.locate(2, 5).unwrap().col, 0);
        assert_eq!(row.locate(5, 5).unwrap().col, 3);
    }

    #[test]
    fn locate_wrap_second_subrow_adds_width() {
        let row = wrap_row(25);
        // sub-row 1 starts at col 10 (= 1 * wrap_width).
        assert_eq!(row.locate(2, 6).unwrap().col, 10);
        assert_eq!(row.locate(11, 6).unwrap().col, 19);
    }

    #[test]
    fn locate_wrap_top_clip_offsets_base_col() {
        // First wrapped row scrolled off the top: base_col = 1 * width.
        let mut row = wrap_row(25);
        row.rect = MouseRect::new(2, 5, 10, 1);
        row.base_col = 10;
        assert_eq!(row.locate(2, 5).unwrap().col, 10);
        assert_eq!(row.locate(4, 5).unwrap().col, 12);
    }

    #[test]
    fn locate_clamps_to_text_len() {
        let row = wrap_row(3);
        // Pointer maps to col 5 but text is only 3 chars long.
        assert_eq!(row.locate(7, 5).unwrap().col, 3);
    }

    #[test]
    fn locate_outside_rect_is_none() {
        let row = wrap_row(25);
        assert!(row.locate(2, 7).is_none(), "below the 2-row rect");
        assert!(row.locate(1, 5).is_none(), "left of the rect");
        assert!(row.locate(12, 5).is_none(), "right of the rect");
    }

    #[test]
    fn locate_nowrap_discounts_left_indicator() {
        let row = SelectionRow {
            rect: MouseRect::new(2, 5, 10, 1),
            entry_id: 7,
            frame_index: None,
            base_col: 7, // h_offset
            left_indicator: true,
            text_len: 50,
            wrap_width: 0,
        };
        // The `←` indicator occupies the first column → maps to base_col.
        assert_eq!(row.locate(2, 5).unwrap().col, 7);
        assert_eq!(row.locate(3, 5).unwrap().col, 7);
        assert_eq!(row.locate(4, 5).unwrap().col, 8);
    }

    #[test]
    fn locate_nowrap_no_indicator_uses_h_offset() {
        let row = SelectionRow {
            rect: MouseRect::new(0, 0, 10, 1),
            entry_id: 7,
            frame_index: None,
            base_col: 0,
            left_indicator: false,
            text_len: 50,
            wrap_width: 0,
        };
        assert_eq!(row.locate(0, 0).unwrap().col, 0);
        assert_eq!(row.locate(5, 0).unwrap().col, 5);
    }

    // ── Selection: clear ──────────────────────────────────────────────────────

    #[test]
    fn clear_selection_resets_all_selection_fields() {
        let mut state = LogViewState::new();
        let p = sp(1, None, 0);
        state.selection = Some(LogSelection::new(p));
        state.drag_autoscroll = Some(1);
        state.selection_text = Some("x".to_string());
        state.selection_text_key = Some(LogSelection::new(p));
        state.clear_selection();
        assert!(state.selection.is_none());
        assert!(state.drag_autoscroll.is_none());
        assert!(state.selection_text.is_none());
        assert!(state.selection_text_key.is_none());
    }

    #[test]
    fn log_selection_nonempty_detects_movement() {
        let p = sp(1, None, 0);
        let still = LogSelection::new(p);
        assert!(!still.is_nonempty());
        let mut moved = still;
        moved.focus = sp(1, None, 4);
        assert!(moved.is_nonempty());
    }
}
