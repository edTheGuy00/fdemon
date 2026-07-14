//! Log view state - scroll position, viewport bounds, and focus tracking.
//!
//! This module defines the state types used by both the app handler layer
//! (for scroll commands) and the TUI layer (for rendering the log view).

use std::collections::{HashMap, VecDeque};

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
/// that line, gutter included. Cell→char conversion is display-width aware in
/// wrap mode (see [`SelectionRow::locate`]); no-wrap horizontal scrolling
/// remains char-cell based (wide chars there are a pre-existing limitation).
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

// ─────────────────────────────────────────────────────────────────────────────
// Display-width wrap math (shared with the log-view renderer)
// ─────────────────────────────────────────────────────────────────────────────
//
// The renderer (fdemon-tui) wraps logical lines into screen rows by **display
// width**, and the selection mapping below must reproduce those row boundaries
// exactly, or highlight/copy drift from what is on screen. Both sides call
// these functions so the greedy packing rule lives in one place.
//
// The unit of measurement is the **grapheme cluster**, not the `char`: ratatui
// lays out each output span by walking `UnicodeSegmentation::graphemes` and
// measuring each cluster with `UnicodeWidthStr`. Per-char sums diverge from
// that for variation-selector emoji (`⚠️` = base + U+FE0F: char-sum 1, rendered
// 2 cells) and ZWJ sequences (👨‍👩‍👧: char-sum 6, rendered 2), so all wrap math
// here iterates clusters and measures them with the same str-width function.
// Clusters are atomic: never split across rows, and every cell of a cluster
// maps to its first char's index. Positions (`SelPoint::col`, row starts) stay
// **char** offsets — grapheme boundaries always fall on char indices.

/// Grapheme clusters of `text` as `(start char index, char count, cell width)`.
///
/// Width is measured per cluster with `UnicodeWidthStr`, plus one cell per
/// halfwidth katakana sound mark (U+FF9E/U+FF9F), matching how ratatui 0.30
/// measures each grapheme at render time (`ratatui-core::buffer::cell_width`).
/// Zero-width clusters (control chars, stray combining marks) yield width 0.
pub fn grapheme_cell_widths(text: &str) -> impl Iterator<Item = (usize, usize, usize)> + '_ {
    use unicode_segmentation::UnicodeSegmentation;
    let mut char_idx = 0usize;
    text.graphemes(true).map(move |g| {
        let n = g.chars().count();
        let sound_marks = g
            .chars()
            .filter(|&c| c == '\u{FF9E}' || c == '\u{FF9F}')
            .count();
        let w = unicode_width::UnicodeWidthStr::width(g) + sound_marks;
        let start = char_idx;
        char_idx += n;
        (start, n, w)
    })
}

/// Greedy display-width row packing over `(start char index, cell width)`
/// cluster atoms (as produced by [`grapheme_cell_widths`]).
///
/// Returns the char index at which each wrapped sub-row starts (always begins
/// with `0`, so the result is never empty). A cluster is placed on the current
/// row when its width still fits; otherwise a new row starts at that cluster —
/// a cluster is never split across rows. Zero-width clusters never trigger a
/// wrap — they stay with the preceding cluster. A single cluster wider than
/// `width` is placed alone on its own row (the terminal clips it; the
/// alternative is an infinite loop).
pub fn wrap_row_starts_cells(
    cells: impl Iterator<Item = (usize, usize)>,
    width: usize,
) -> Vec<usize> {
    let mut starts = vec![0usize];
    if width == 0 {
        return starts;
    }
    let mut col = 0usize;
    for (i, w) in cells {
        if w == 0 {
            continue;
        }
        if col + w > width && col > 0 {
            starts.push(i);
            col = 0;
        }
        col += w;
    }
    starts
}

/// [`wrap_row_starts_cells`] over the grapheme clusters of `text`.
pub fn wrap_row_starts(text: &str, width: usize) -> Vec<usize> {
    wrap_row_starts_cells(grapheme_cell_widths(text).map(|(i, _, w)| (i, w)), width)
}

/// Number of screen rows the given per-cluster widths occupy under the same
/// greedy packing rule as [`wrap_row_starts_cells`], without allocating (this
/// runs per entry per frame in the scroll-bounds calculation). Items are
/// **grapheme-cluster** widths (see [`grapheme_cell_widths`]), not per-char.
pub fn wrapped_row_count_widths(widths: impl Iterator<Item = usize>, width: usize) -> usize {
    if width == 0 {
        return 1;
    }
    let mut rows = 1usize;
    let mut col = 0usize;
    for w in widths {
        if w == 0 {
            continue;
        }
        if col + w > width && col > 0 {
            rows += 1;
            col = 0;
        }
        col += w;
    }
    rows
}

/// Map a display-column offset `dx` within one wrapped sub-row (`row_start..
/// row_end` in char indices) back to the char index whose cells cover `dx`.
/// Any cell of a multi-char cluster maps to the cluster's first char index.
/// Returns `row_end` when `dx` lies past the row's last cell.
fn char_index_at_display_col(text: &str, row_start: usize, row_end: usize, dx: usize) -> usize {
    let mut cum = 0usize;
    for (start, _, w) in grapheme_cell_widths(text) {
        if start < row_start {
            continue;
        }
        if start >= row_end {
            break;
        }
        if w == 0 {
            continue;
        }
        if dx < cum + w {
            return start;
        }
        cum += w;
    }
    row_end
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionRow {
    /// Screen rect of the (clipped) visible portion of the logical line.
    pub rect: MouseRect,
    pub entry_id: u64,
    pub frame_index: Option<usize>,
    /// No-wrap only: char offset of the first **visible** character (`h_offset`
    /// plus one when the `←` indicator replaces the first content cell). Unused
    /// (0) in wrap mode, which derives offsets from `top_clip` + `text`.
    pub base_col: usize,
    /// No-wrap only: a `←` indicator occupies the first column when scrolled.
    pub left_indicator: bool,
    /// No-wrap only: a `→` indicator occupies the last column when the line
    /// continues past the right edge.
    pub right_indicator: bool,
    /// Full character length of the logical line's rendered text.
    pub text_len: usize,
    /// Wrap width (content width) in wrap mode; `0` in no-wrap mode (each logical
    /// line is exactly one screen row).
    pub wrap_width: u16,
    /// Wrap mode only: number of wrapped sub-rows scrolled off the top of the
    /// viewport (the rect covers sub-rows `top_clip..top_clip + rect.height`).
    pub top_clip: usize,
    /// Wrap mode only: the logical line's full rendered text, used for
    /// display-width-aware sub-row boundaries and cell→char mapping. Empty in
    /// no-wrap mode (which stays char-cell based).
    pub text: String,
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
            // Wrap mode: find the sub-row's char range with the same
            // grapheme-cluster packing the renderer used (measured with
            // `UnicodeWidthStr` to match ratatui), then map the cell offset
            // back to a char index (any cell of a cluster → its first char).
            let sub_row = self.top_clip + (y - self.rect.y) as usize;
            let starts = wrap_row_starts(&self.text, self.wrap_width as usize);
            match starts.get(sub_row) {
                Some(&row_start) => {
                    let row_end = starts.get(sub_row + 1).copied().unwrap_or(self.text_len);
                    char_index_at_display_col(&self.text, row_start, row_end, dx)
                }
                None => self.text_len,
            }
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

/// One entry's cached wrap-mode display-row count (issue #75).
///
/// Render-written by the log-view renderer (`fdemon-tui`) whenever it
/// computes an entry's row count the exact-but-slow way (a cache miss);
/// consulted on every later frame while both the entry's identity
/// (`LogViewState::row_cache` key) and its `expanded` flag stay the same.
/// Lookup-time keyed — no handler-side invalidation wiring is needed: the
/// renderer re-reads `expanded` fresh from `CollapseState` every frame and
/// compares it against the cached value, so a toggle produces a guaranteed
/// miss (recompute + overwrite) rather than a correctness hazard.
/// Registered in docs/REVIEW_FOCUS.md ("Current usage" list of the approved TEA exception).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CachedRows {
    /// The entry's collapse/expand state at the time `rows` was computed.
    pub expanded: bool,
    /// Terminal rows the entry occupies in wrap mode, at the render's
    /// current global key — see [`LogViewState::row_cache_key`].
    pub rows: u16,
}

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
    /// Per-entry cached wrap-mode display-row count, keyed by [`LogEntry::id`]
    /// (issue #75). Render-written: populated by the log-view renderer on a
    /// cache miss, read on every subsequent frame. Safe default: empty (every
    /// entry starts as a miss). Wrap mode only — nowrap uses
    /// `calculate_entry_lines` (no measured widths) and leaves this untouched.
    /// Registered in docs/REVIEW_FOCUS.md ("Current usage" list of the approved TEA exception).
    pub row_cache: HashMap<u64, CachedRows>,
    /// Global cache key for `row_cache`: `(content width, wrap_mode)` at the
    /// last render that touched the cache. A mismatch at render start clears
    /// `row_cache` wholesale (a resize or the first wrap-mode render
    /// invalidates every cached row count). Safe default: `None` (a `None`
    /// key never matches, so the first render always "clears" an already-
    /// empty map). Registered in docs/REVIEW_FOCUS.md ("Current usage" list of the approved TEA exception).
    pub row_cache_key: Option<(u16, bool)>,
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
            row_cache: HashMap::new(),
            row_cache_key: None,
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

    /// True while a drag-selection is in progress (mouse button held).
    fn drag_active(&self) -> bool {
        self.selection.as_ref().is_some_and(|s| s.dragging)
    }

    /// Scroll down by n lines
    pub fn scroll_down(&mut self, n: usize) {
        let max_offset = self.total_lines.saturating_sub(self.visible_lines);
        self.offset = (self.offset + n).min(max_offset);

        // Re-enable auto-scroll if at bottom — but never while a drag is in
        // progress: tail-follow mid-drag would chase every new arrival and
        // grow the selection unbounded on a live session (follow re-arms
        // normally once the drag ends).
        if self.offset >= max_offset && !self.drag_active() {
            self.auto_scroll = true;
        }
    }

    /// Scroll to top
    pub fn scroll_to_top(&mut self) {
        self.offset = 0;
        self.auto_scroll = false;
    }

    /// Scroll to bottom and enable auto-scroll (unless a drag is in progress —
    /// the viewport still jumps, but tail-follow mid-drag would grow the
    /// selection unbounded on a live session).
    pub fn scroll_to_bottom(&mut self) {
        self.offset = self.total_lines.saturating_sub(self.visible_lines);
        self.auto_scroll = !self.drag_active();
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

    // ── Selection: display-width wrap math ────────────────────────────────────

    #[test]
    fn wrap_row_starts_ascii_chunks_by_width() {
        assert_eq!(wrap_row_starts(&"a".repeat(25), 10), vec![0, 10, 20]);
    }

    #[test]
    fn wrap_row_starts_wide_chars_fit_half_as_many() {
        // 10 CJK chars × 2 cells in a 10-cell row → 5 chars per row.
        let text: String = "漢".repeat(10);
        assert_eq!(wrap_row_starts(&text, 10), vec![0, 5]);
    }

    #[test]
    fn wrap_row_starts_wide_char_never_splits_across_rows() {
        // width 3: 'a'(1) + '漢'(2) fills row 0 exactly; next '漢' starts row 1.
        assert_eq!(wrap_row_starts("a漢漢b", 3), vec![0, 2]);
        // width 2 with alternating widths: a row holding 'a' cannot also take a
        // 2-cell char, so every wide char starts its own row.
        assert_eq!(wrap_row_starts("a漢b漢", 2), vec![0, 1, 2, 3]);
    }

    #[test]
    fn wrap_row_starts_zero_width_stays_with_base_char() {
        // "e" + combining acute (width 0) at a row boundary must not split.
        let text = "abce\u{0301}f";
        assert_eq!(wrap_row_starts(text, 4), vec![0, 5]);
    }

    #[test]
    fn wrapped_row_count_widths_matches_wrap_row_starts() {
        for text in [
            "",
            "abc",
            "a漢b漢",
            &"漢".repeat(13),
            "abce\u{0301}fghij",
            "a⚠\u{FE0F}b❤\u{FE0F}c👨\u{200D}👩\u{200D}👧d",
        ] {
            for width in 1..8 {
                assert_eq!(
                    wrapped_row_count_widths(grapheme_cell_widths(text).map(|(_, _, w)| w), width),
                    wrap_row_starts(text, width).len(),
                    "row-count and row-starts must agree for {text:?} at width {width}"
                );
            }
        }
    }

    #[test]
    fn wrapped_row_count_widths_empty_is_one() {
        assert_eq!(wrapped_row_count_widths(std::iter::empty(), 10), 1);
        assert_eq!(
            wrapped_row_count_widths(grapheme_cell_widths("abc").map(|(_, _, w)| w), 0),
            1
        );
    }

    #[test]
    fn grapheme_cell_widths_reports_cluster_geometry() {
        // "a" (1 char, 1 cell), "⚠️" = ⚠ + VS16 (2 chars, 2 cells),
        // "b" (1 char, 1 cell), 👨‍👩‍👧 = 3 emoji + 2 ZWJ (5 chars, 2 cells).
        let text = "a⚠\u{FE0F}b👨\u{200D}👩\u{200D}👧";
        let clusters: Vec<_> = grapheme_cell_widths(text).collect();
        assert_eq!(clusters, vec![(0, 1, 1), (1, 2, 2), (3, 1, 1), (4, 5, 2)]);
    }

    #[test]
    fn wrap_row_starts_never_splits_vs16_or_zwj_clusters() {
        // width 2: 'a' fills 1 cell; the 2-cell ⚠️ cluster cannot share the
        // row, so it starts row 1 at char index 1; 'b' starts row 2 at 3.
        assert_eq!(wrap_row_starts("a⚠\u{FE0F}b", 2), vec![0, 1, 3]);
        // The 5-char ZWJ family renders 2 cells: it fills row 0 alone (width
        // 2) and 'x' starts row 1 at char index 5.
        assert_eq!(wrap_row_starts("👨\u{200D}👩\u{200D}👧x", 2), vec![0, 5]);
    }

    // ── Selection: SelectionRow::locate ───────────────────────────────────────

    fn wrap_row(text: &str) -> SelectionRow {
        SelectionRow {
            rect: MouseRect::new(2, 5, 10, 2),
            entry_id: 7,
            frame_index: None,
            base_col: 0,
            left_indicator: false,
            right_indicator: false,
            text_len: text.chars().count(),
            wrap_width: 10,
            top_clip: 0,
            text: text.to_string(),
        }
    }

    #[test]
    fn locate_wrap_first_subrow_maps_column() {
        let row = wrap_row(&"a".repeat(25));
        assert_eq!(row.locate(2, 5).unwrap().col, 0);
        assert_eq!(row.locate(5, 5).unwrap().col, 3);
    }

    #[test]
    fn locate_wrap_second_subrow_adds_width() {
        let row = wrap_row(&"a".repeat(25));
        // sub-row 1 starts at col 10 (= 1 * wrap_width for all-ASCII text).
        assert_eq!(row.locate(2, 6).unwrap().col, 10);
        assert_eq!(row.locate(11, 6).unwrap().col, 19);
    }

    #[test]
    fn locate_wrap_wide_chars_map_cells_to_chars() {
        // 10-cell rows of 2-cell chars: sub-row 1 starts at char 5, and each
        // char covers two cells (both cells of a glyph map to the same char).
        let row = wrap_row(&"漢".repeat(10));
        assert_eq!(row.locate(2, 5).unwrap().col, 0);
        assert_eq!(
            row.locate(3, 5).unwrap().col,
            0,
            "second cell of a wide glyph"
        );
        assert_eq!(row.locate(4, 5).unwrap().col, 1);
        assert_eq!(
            row.locate(2, 6).unwrap().col,
            5,
            "sub-row 1 starts at char 5"
        );
        assert_eq!(row.locate(8, 6).unwrap().col, 8);
    }

    #[test]
    fn locate_wrap_cluster_cells_map_to_cluster_start() {
        // "⚠️" (⚠ + VS16) is one 2-char cluster rendering 2 cells: 5 clusters
        // fill a 10-cell row, so sub-row 1 starts at char index 10. Both cells
        // of every cluster map to the cluster's first char index (0, 2, 4, …).
        let row = wrap_row(&"⚠\u{FE0F}".repeat(10));
        assert_eq!(row.locate(2, 5).unwrap().col, 0);
        assert_eq!(
            row.locate(3, 5).unwrap().col,
            0,
            "second cell of a VS16 cluster"
        );
        assert_eq!(row.locate(4, 5).unwrap().col, 2);
        assert_eq!(row.locate(5, 5).unwrap().col, 2);
        assert_eq!(
            row.locate(2, 6).unwrap().col,
            10,
            "sub-row 1 starts at char 10 (5 clusters × 2 chars)"
        );
        assert_eq!(row.locate(8, 6).unwrap().col, 16);
    }

    #[test]
    fn locate_wrap_top_clip_offsets_base_col() {
        // First wrapped sub-row scrolled off the top: rect covers sub-row 1.
        let mut row = wrap_row(&"a".repeat(25));
        row.rect = MouseRect::new(2, 5, 10, 1);
        row.top_clip = 1;
        assert_eq!(row.locate(2, 5).unwrap().col, 10);
        assert_eq!(row.locate(4, 5).unwrap().col, 12);
    }

    #[test]
    fn locate_clamps_to_text_len() {
        let row = wrap_row("abc");
        // Pointer maps past the 3-char text → clamped to its end.
        assert_eq!(row.locate(7, 5).unwrap().col, 3);
    }

    #[test]
    fn locate_outside_rect_is_none() {
        let row = wrap_row(&"a".repeat(25));
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
            // h_offset = 7; the `←` indicator replaces the char at index 7, so
            // the first visible char is index 8 (see apply_horizontal_scroll).
            base_col: 8,
            left_indicator: true,
            right_indicator: true,
            text_len: 50,
            wrap_width: 0,
            top_clip: 0,
            text: String::new(),
        };
        // The `←` indicator occupies the first column → maps to base_col.
        assert_eq!(row.locate(2, 5).unwrap().col, 8);
        assert_eq!(row.locate(3, 5).unwrap().col, 8);
        assert_eq!(row.locate(4, 5).unwrap().col, 9);
    }

    #[test]
    fn locate_nowrap_no_indicator_uses_h_offset() {
        let row = SelectionRow {
            rect: MouseRect::new(0, 0, 10, 1),
            entry_id: 7,
            frame_index: None,
            base_col: 0,
            left_indicator: false,
            right_indicator: false,
            text_len: 50,
            wrap_width: 0,
            top_clip: 0,
            text: String::new(),
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

    // ── Tail-follow vs. drag ─────────────────────────────────────────────────

    /// State scrolled away from the bottom with content that has a bottom to
    /// reach: total 20 lines, 5 visible, offset 10 (max_offset = 15).
    fn scrolled_state() -> LogViewState {
        let mut state = LogViewState::new();
        state.total_lines = 20;
        state.visible_lines = 5;
        state.offset = 10;
        state.auto_scroll = false;
        state
    }

    #[test]
    fn scroll_down_rearms_follow_at_bottom_without_drag() {
        let mut state = scrolled_state();
        state.scroll_down(100);
        assert!(state.auto_scroll, "reaching bottom re-arms tail-follow");
    }

    #[test]
    fn scroll_down_does_not_rearm_follow_while_dragging() {
        let mut state = scrolled_state();
        state.selection = Some(LogSelection::new(sp(1, None, 0)));
        state.scroll_down(100);
        assert_eq!(state.offset, 15, "still scrolls to the bottom");
        assert!(
            !state.auto_scroll,
            "tail-follow must not re-arm mid-drag (selection would grow unbounded)"
        );
    }

    #[test]
    fn scroll_to_bottom_does_not_rearm_follow_while_dragging() {
        let mut state = scrolled_state();
        state.selection = Some(LogSelection::new(sp(1, None, 0)));
        state.scroll_to_bottom();
        assert_eq!(state.offset, 15, "viewport still jumps to the bottom");
        assert!(!state.auto_scroll, "tail-follow must not re-arm mid-drag");
    }

    #[test]
    fn scroll_to_bottom_rearms_follow_after_drag_ends() {
        let mut state = scrolled_state();
        // A completed (released) selection no longer counts as a drag.
        let mut sel = LogSelection::new(sp(1, None, 0));
        sel.dragging = false;
        state.selection = Some(sel);
        state.scroll_to_bottom();
        assert!(state.auto_scroll, "follow re-arms once the drag has ended");
    }
}
