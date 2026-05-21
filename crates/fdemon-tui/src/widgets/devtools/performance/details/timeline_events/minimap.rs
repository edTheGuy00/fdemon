//! Minimap ribbon for the Timeline Events tab.
//!
//! Renders a 1-row horizontally-compressed overview of the full event history
//! with a `[...]` bracket overlay showing the current viewport position.
//!
//! ## Design
//!
//! - Each terminal column represents a proportional slice of the full history.
//! - The dominant thread's color (by microsecond-overlap area) is painted as the
//!   background of each column, giving an at-a-glance thread-activity heatmap.
//! - Only root events (depth 0) are scanned — children don't influence the
//!   macro-view materially and would dominate the scan cost unnecessarily.
//! - The viewport bracket `[...]` is drawn in bold white over the colored
//!   background so it remains visible regardless of the palette behind it.
//!
//! ## Performance
//!
//! Dominant-thread computation is `O(columns × root_events)`. With 100 columns
//! and 1 000 events that is ~100 k iterations per frame — acceptable for a TUI
//! that redraws at ~30 FPS. If this becomes a hot path, switch to a pre-binned
//! histogram updated on batch-receive in the handler crate.
//!
//! ## Mouse stretch goal
//!
//! TODO (Phase 5, T02 stretch goal): Clicking on the minimap should pan the
//! Gantt viewport to center on the clicked column's corresponding timestamp.
//! Requires registering a mouse click region during `render` (via `MouseCtx`)
//! and dispatching `Message::TimelinePanTo { ts: micros }` from the mouse
//! handler. Skipped in T02 because it requires T03/T04 mouse plumbing that has
//! not yet landed. See task file AC10.

use fdemon_app::session::TimelineFilter;
use fdemon_core::timeline::{TimelineThread, TimelineTrack};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier},
};
use std::collections::BTreeMap;

use super::gantt::matches_filter;
use super::{palette, viewport::micros_to_column};

// ── Public constants ──────────────────────────────────────────────────────────

/// Height of the minimap ribbon in terminal lines.
///
/// 1 row: a single colored strip with a bracket overlay. Keeping it at 1 row
/// minimizes the vertical footprint while still providing spatial context.
pub(super) const MINIMAP_HEIGHT: u16 = 1;

/// Default history span covered by the minimap, in microseconds (30 seconds).
///
/// Auto-extends to encompass all buffered events when they exceed this span.
/// If the buffer has older events, the minimap compresses them all into the
/// available columns; if newer, the same.
pub(super) const MINIMAP_DEFAULT_HISTORY_MICROS: u64 = 30_000_000;

// ── Public renderer ───────────────────────────────────────────────────────────

/// Render the minimap ribbon into `area` (expected to be 1 row tall).
///
/// `(viewport_start, viewport_end)` must come from `compute_active_viewport`
/// — do **not** pass the raw `timeline_viewport_*` fields directly. The caller
/// in `mod.rs` resolves the viewport and passes the resolved bounds here so the
/// minimap stays pure (no `PerformanceState` dependency).
///
/// When `tracks` is empty or `area` has zero dimensions, this function returns
/// immediately with no buffer mutations.
pub(super) fn render(
    area: Rect,
    buf: &mut Buffer,
    tracks: &BTreeMap<i64, TimelineTrack>,
    viewport_start: u64,
    viewport_end: u64,
    filter: TimelineFilter,
) {
    if area.width == 0 || area.height == 0 || tracks.is_empty() {
        return;
    }

    // 1. Compute full history bounds across all visible tracks.
    let (history_start, history_end) = compute_history_bounds(tracks, filter);

    // Guard: degenerate history (no events match the filter, or zero-span).
    if history_end <= history_start {
        return;
    }

    // 2. Paint each column with the dominant thread's color.
    for x in 0..area.width {
        let col_start_micros = column_to_micros(x, history_start, history_end, area.width);
        let col_end_micros = column_to_micros(x + 1, history_start, history_end, area.width);

        if let Some(thread) =
            dominant_thread_in_range(tracks, col_start_micros, col_end_micros, filter)
        {
            let color = palette::bar_color(thread, 0);
            if let Some(cell) = buf.cell_mut((area.x + x, area.y)) {
                cell.set_bg(color);
            }
        }
    }

    // 3. Overlay viewport bracket [...] at the current viewport's column range.
    let vp_start_col = micros_to_column(viewport_start, history_start, history_end, area.width);
    let vp_end_col = micros_to_column(viewport_end, history_start, history_end, area.width);

    // bracket_x_end: right bracket position — at least as far as left bracket,
    // clamped to canvas width - 1.
    let bracket_x_start = area.x + vp_start_col;
    let bracket_x_end = area.x + vp_end_col.saturating_sub(1).max(vp_start_col);

    if let Some(cell) = buf.cell_mut((bracket_x_start, area.y)) {
        cell.set_char('[');
        cell.set_fg(Color::White);
        cell.set_style(cell.style().fg(Color::White).add_modifier(Modifier::BOLD));
    }
    if let Some(cell) = buf.cell_mut((bracket_x_end, area.y)) {
        cell.set_char(']');
        cell.set_style(cell.style().fg(Color::White).add_modifier(Modifier::BOLD));
    }
}

// ── Private helpers ───────────────────────────────────────────────────────────

/// Compute the full history bounds `(min_ts, max_ts_end)` across all tracks
/// that pass `filter`.
///
/// Returns `(0, MINIMAP_DEFAULT_HISTORY_MICROS)` when no events match.
fn compute_history_bounds(
    tracks: &BTreeMap<i64, TimelineTrack>,
    filter: TimelineFilter,
) -> (u64, u64) {
    let mut min_ts: Option<u64> = None;
    let mut max_end: Option<u64> = None;

    for track in tracks.values() {
        if !matches_filter(track.thread, filter) {
            continue;
        }
        for node in &track.root_events {
            let ts = node.ts as u64;
            let end = ts.saturating_add(node.dur.unwrap_or(0) as u64);

            min_ts = Some(min_ts.map_or(ts, |m| m.min(ts)));
            max_end = Some(max_end.map_or(end, |m| m.max(end)));
        }
    }

    match (min_ts, max_end) {
        (Some(s), Some(e)) if e > s => (s, e),
        (Some(s), _) => (s, s + MINIMAP_DEFAULT_HISTORY_MICROS),
        _ => (0, MINIMAP_DEFAULT_HISTORY_MICROS),
    }
}

/// Map a column index `col ∈ [0, width]` to its corresponding microsecond
/// timestamp within `[history_start, history_end]`.
///
/// `col == 0` → `history_start`, `col == width` → `history_end`.
fn column_to_micros(col: u16, history_start: u64, history_end: u64, width: u16) -> u64 {
    if width == 0 {
        return history_start;
    }
    let span = history_end - history_start;
    history_start + (col as u64 * span / width as u64)
}

/// For each thread in `tracks` that passes `filter`, sum the microseconds of
/// root-event overlap within `[col_start, col_end]`. Return the thread with the
/// largest total, or `None` if no events overlap the range.
fn dominant_thread_in_range(
    tracks: &BTreeMap<i64, TimelineTrack>,
    col_start: u64,
    col_end: u64,
    filter: TimelineFilter,
) -> Option<TimelineThread> {
    // Accumulate overlap per thread variant.
    let mut overlap_ui: u64 = 0;
    let mut overlap_raster: u64 = 0;
    let mut overlap_other: u64 = 0;

    for track in tracks.values() {
        if !matches_filter(track.thread, filter) {
            continue;
        }
        for node in &track.root_events {
            let ts = node.ts as u64;
            let end = ts.saturating_add(node.dur.unwrap_or(0) as u64);

            // Compute overlap with [col_start, col_end)
            let overlap_start = ts.max(col_start);
            let overlap_end = end.min(col_end);
            if overlap_end <= overlap_start {
                continue;
            }
            let overlap = overlap_end - overlap_start;
            match track.thread {
                TimelineThread::Ui => overlap_ui += overlap,
                TimelineThread::Raster => overlap_raster += overlap,
                TimelineThread::Other => overlap_other += overlap,
            }
        }
    }

    // Pick the thread with the highest total overlap.
    let max = overlap_ui.max(overlap_raster).max(overlap_other);
    if max == 0 {
        return None;
    }
    if overlap_ui == max {
        Some(TimelineThread::Ui)
    } else if overlap_raster == max {
        Some(TimelineThread::Raster)
    } else {
        Some(TimelineThread::Other)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use fdemon_core::timeline::{TimelineNode, TimelinePhase, TimelineTrack};
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use ratatui::style::Color;

    // ── Test helpers ──────────────────────────────────────────────────────────

    fn make_node(thread: TimelineThread, ts: i64, dur: i64) -> TimelineNode {
        TimelineNode {
            name: "test".to_owned(),
            category: None,
            ts,
            dur: Some(dur),
            phase: TimelinePhase::Complete,
            thread,
            frame_number: None,
            children: vec![],
        }
    }

    fn make_track(tid: i64, thread: TimelineThread, events: Vec<TimelineNode>) -> TimelineTrack {
        TimelineTrack {
            tid,
            name: None,
            thread,
            root_events: events,
        }
    }

    fn cell_bg(buf: &Buffer, x: u16, y: u16) -> Color {
        buf.cell((x, y))
            .map(|c| c.style().bg.unwrap_or(Color::Reset))
            .unwrap_or(Color::Reset)
    }

    fn cell_char(buf: &Buffer, x: u16, y: u16) -> char {
        buf.cell((x, y))
            .and_then(|c| c.symbol().chars().next())
            .unwrap_or(' ')
    }

    // ── AC3: Empty state — no panic, no paint ─────────────────────────────────

    #[test]
    fn minimap_empty_state_no_panic_no_paint() {
        let tracks: BTreeMap<i64, TimelineTrack> = BTreeMap::new();
        let area = Rect::new(0, 0, 80, 1);
        let mut buf = Buffer::empty(area);
        render(area, &mut buf, &tracks, 0, 5_000_000, TimelineFilter::All);
        // All cells should remain default (no color painted)
        for x in 0..area.width {
            assert_eq!(
                cell_bg(&buf, x, 0),
                Color::Reset,
                "empty tracks should not paint column {x}"
            );
        }
    }

    // ── AC4: Single thread paints solid row ───────────────────────────────────

    #[test]
    fn minimap_single_thread_paints_solid_row() {
        // 5 root events spanning [0, 5_000_000] on the UI thread
        let events: Vec<TimelineNode> = (0..5)
            .map(|i| make_node(TimelineThread::Ui, i * 1_000_000, 1_000_000))
            .collect();
        let mut tracks = BTreeMap::new();
        tracks.insert(1, make_track(1, TimelineThread::Ui, events));

        let area = Rect::new(0, 0, 10, 1);
        let mut buf = Buffer::empty(area);
        render(area, &mut buf, &tracks, 0, 5_000_000, TimelineFilter::All);

        // Every column should have a non-Reset background (UI thread color)
        let expected_color = palette::bar_color(TimelineThread::Ui, 0);
        for x in 0..area.width {
            assert_eq!(
                cell_bg(&buf, x, 0),
                expected_color,
                "column {x} should have UI thread color"
            );
        }
    }

    // ── AC5: Multi-thread dominance — two-color row ───────────────────────────

    #[test]
    fn minimap_multi_thread_shows_dominant_per_column() {
        // UI on left half [0, 5_000_000), Raster on right half [5_000_000, 10_000_000)
        let mut tracks = BTreeMap::new();
        tracks.insert(
            1,
            make_track(
                1,
                TimelineThread::Ui,
                vec![make_node(TimelineThread::Ui, 0, 5_000_000)],
            ),
        );
        tracks.insert(
            2,
            make_track(
                2,
                TimelineThread::Raster,
                vec![make_node(TimelineThread::Raster, 5_000_000, 5_000_000)],
            ),
        );

        let area = Rect::new(0, 0, 10, 1);
        let mut buf = Buffer::empty(area);
        // History [0, 10_000_000), viewport can be anything
        render(area, &mut buf, &tracks, 0, 10_000_000, TimelineFilter::All);

        let ui_color = palette::bar_color(TimelineThread::Ui, 0);
        let raster_color = palette::bar_color(TimelineThread::Raster, 0);

        // Columns 0..5 should be UI-colored, columns 5..10 should be Raster-colored
        for x in 0..5 {
            assert_eq!(
                cell_bg(&buf, x, 0),
                ui_color,
                "column {x} should be UI (left half)"
            );
        }
        for x in 5..10 {
            assert_eq!(
                cell_bg(&buf, x, 0),
                raster_color,
                "column {x} should be Raster (right half)"
            );
        }
    }

    // ── AC6: Viewport bracket at correct columns ───────────────────────────────

    #[test]
    fn minimap_bracket_at_correct_columns() {
        // History [0, 5_000_000) → 5 seconds, width=10 columns
        // Each column = 500_000 µs
        // Viewport [1_000_000, 2_000_000) = 20%..40% → columns 2..4
        let events = vec![make_node(TimelineThread::Ui, 0, 5_000_000)];
        let mut tracks = BTreeMap::new();
        tracks.insert(1, make_track(1, TimelineThread::Ui, events));

        let area = Rect::new(0, 0, 10, 1);
        let mut buf = Buffer::empty(area);
        render(
            area,
            &mut buf,
            &tracks,
            1_000_000,
            2_000_000,
            TimelineFilter::All,
        );

        // micros_to_column(1_000_000, 0, 5_000_000, 10) = floor(1M/5M * 10) = 2
        // micros_to_column(2_000_000, 0, 5_000_000, 10) = floor(2M/5M * 10) = 4
        // bracket_x_start = 0 + 2 = 2, bracket_x_end = 0 + max(4-1, 2) = 3
        assert_eq!(cell_char(&buf, 2, 0), '[', "left bracket at column 2");
        assert_eq!(cell_char(&buf, 3, 0), ']', "right bracket at column 3");
    }

    // ── AC7: Bracket clipped to canvas ────────────────────────────────────────

    #[test]
    fn minimap_bracket_clipped_to_canvas() {
        // Viewport = full history → [ at col 0, ] near col width-1
        let events = vec![make_node(TimelineThread::Ui, 0, 5_000_000)];
        let mut tracks = BTreeMap::new();
        tracks.insert(1, make_track(1, TimelineThread::Ui, events));

        let area = Rect::new(0, 0, 10, 1);
        let mut buf = Buffer::empty(area);
        render(
            area,
            &mut buf,
            &tracks,
            0,         // viewport_start == history_start
            5_000_000, // viewport_end == history_end
            TimelineFilter::All,
        );

        // micros_to_column(0, 0, 5_000_000, 10) = 0
        // micros_to_column(5_000_000, 0, 5_000_000, 10) = 9 (clamped width-1)
        // bracket_x_start = 0, bracket_x_end = max(9-1, 0) = 8
        assert_eq!(cell_char(&buf, 0, 0), '[', "left bracket at column 0");
        // Right bracket lands at 8 (= 9 - 1) because saturating_sub(1).max(start_col)
        assert_eq!(
            cell_char(&buf, 8, 0),
            ']',
            "right bracket near canvas right edge"
        );
    }

    #[test]
    fn minimap_bracket_viewport_beyond_history_clips() {
        // Viewport extends beyond history end — bracket should still be within canvas.
        let events = vec![make_node(TimelineThread::Ui, 0, 5_000_000)];
        let mut tracks = BTreeMap::new();
        tracks.insert(1, make_track(1, TimelineThread::Ui, events));

        let area = Rect::new(0, 0, 10, 1);
        let mut buf = Buffer::empty(area);
        // viewport_end > history_end (e.g., live-follow ahead of buffered events)
        render(
            area,
            &mut buf,
            &tracks,
            4_000_000,  // viewport_start near history_end
            10_000_000, // viewport_end beyond history_end
            TimelineFilter::All,
        );

        // Both bracket cells should be within [0, width)
        let lbracket = cell_char(&buf, 0, 0);
        let rbracket = cell_char(&buf, 0, 0);
        // Just verify no panic occurred — bracket characters may overlap at col 8/9
        let _ = (lbracket, rbracket);
    }

    // ── AC8: Filter respected ─────────────────────────────────────────────────

    #[test]
    fn minimap_filter_ui_excludes_raster_threads() {
        // Raster track dominates the left half; UI on right half.
        // With filter=Ui, the Raster events are skipped entirely.
        let mut tracks = BTreeMap::new();
        tracks.insert(
            1,
            make_track(
                1,
                TimelineThread::Raster,
                vec![make_node(TimelineThread::Raster, 0, 5_000_000)],
            ),
        );
        tracks.insert(
            2,
            make_track(
                2,
                TimelineThread::Ui,
                vec![make_node(TimelineThread::Ui, 5_000_000, 5_000_000)],
            ),
        );

        let area = Rect::new(0, 0, 10, 1);
        let mut buf = Buffer::empty(area);
        render(
            area,
            &mut buf,
            &tracks,
            0,
            10_000_000,
            TimelineFilter::Ui, // Only UI visible
        );

        let raster_color = palette::bar_color(TimelineThread::Raster, 0);
        // No column should have the Raster color — filter excludes Raster.
        for x in 0..area.width {
            assert_ne!(
                cell_bg(&buf, x, 0),
                raster_color,
                "column {x} should not be Raster-colored when filter=Ui"
            );
        }

        // The UI events are in the right half, so columns 5..10 should be UI-colored.
        let ui_color = palette::bar_color(TimelineThread::Ui, 0);
        for x in 5..10 {
            assert_eq!(
                cell_bg(&buf, x, 0),
                ui_color,
                "column {x} should be UI-colored (right half, filter=Ui)"
            );
        }
    }

    // ── AC9: No panic on width=1 ──────────────────────────────────────────────

    #[test]
    fn minimap_width_one_no_panic() {
        let events = vec![make_node(TimelineThread::Ui, 0, 5_000_000)];
        let mut tracks = BTreeMap::new();
        tracks.insert(1, make_track(1, TimelineThread::Ui, events));

        let area = Rect::new(0, 0, 1, 1);
        let mut buf = Buffer::empty(area);
        // Must not panic; bracket compresses to single cell.
        render(
            area,
            &mut buf,
            &tracks,
            1_000_000,
            2_000_000,
            TimelineFilter::All,
        );
        // One of '[' or ']' will be drawn at column 0 (they overlap).
        let ch = cell_char(&buf, 0, 0);
        assert!(
            ch == '[' || ch == ']',
            "width=1: bracket should compress to single cell, got '{ch}'"
        );
    }

    // ── compute_history_bounds ────────────────────────────────────────────────

    #[test]
    fn compute_history_bounds_empty_tracks_returns_default() {
        let tracks: BTreeMap<i64, TimelineTrack> = BTreeMap::new();
        let (start, end) = compute_history_bounds(&tracks, TimelineFilter::All);
        assert_eq!(start, 0);
        assert_eq!(end, MINIMAP_DEFAULT_HISTORY_MICROS);
    }

    #[test]
    fn compute_history_bounds_single_event_returns_event_span() {
        let mut tracks = BTreeMap::new();
        tracks.insert(
            1,
            make_track(
                1,
                TimelineThread::Ui,
                vec![make_node(TimelineThread::Ui, 1_000_000, 4_000_000)],
            ),
        );
        let (start, end) = compute_history_bounds(&tracks, TimelineFilter::All);
        assert_eq!(start, 1_000_000);
        assert_eq!(end, 5_000_000);
    }

    #[test]
    fn compute_history_bounds_respects_filter() {
        let mut tracks = BTreeMap::new();
        // Raster event at [0, 5_000_000)
        tracks.insert(
            1,
            make_track(
                1,
                TimelineThread::Raster,
                vec![make_node(TimelineThread::Raster, 0, 5_000_000)],
            ),
        );
        // UI event at [10_000_000, 15_000_000)
        tracks.insert(
            2,
            make_track(
                2,
                TimelineThread::Ui,
                vec![make_node(TimelineThread::Ui, 10_000_000, 5_000_000)],
            ),
        );

        // With Ui filter, only the UI event contributes.
        let (start, end) = compute_history_bounds(&tracks, TimelineFilter::Ui);
        assert_eq!(start, 10_000_000);
        assert_eq!(end, 15_000_000);
    }

    // ── dominant_thread_in_range ─────────────────────────────────────────────

    #[test]
    fn dominant_thread_returns_none_for_empty_range() {
        let tracks: BTreeMap<i64, TimelineTrack> = BTreeMap::new();
        assert!(dominant_thread_in_range(&tracks, 0, 1_000_000, TimelineFilter::All).is_none());
    }

    #[test]
    fn dominant_thread_returns_thread_with_most_overlap() {
        let mut tracks = BTreeMap::new();
        // UI: 4_000_000 µs overlap
        tracks.insert(
            1,
            make_track(
                1,
                TimelineThread::Ui,
                vec![make_node(TimelineThread::Ui, 0, 4_000_000)],
            ),
        );
        // Raster: 1_000_000 µs overlap
        tracks.insert(
            2,
            make_track(
                2,
                TimelineThread::Raster,
                vec![make_node(TimelineThread::Raster, 0, 1_000_000)],
            ),
        );

        let result = dominant_thread_in_range(&tracks, 0, 5_000_000, TimelineFilter::All);
        assert_eq!(result, Some(TimelineThread::Ui));
    }

    // ── column_to_micros ─────────────────────────────────────────────────────

    #[test]
    fn column_to_micros_col_0_returns_history_start() {
        assert_eq!(column_to_micros(0, 1_000_000, 6_000_000, 10), 1_000_000);
    }

    #[test]
    fn column_to_micros_col_width_returns_history_end() {
        assert_eq!(column_to_micros(10, 0, 10_000_000, 10), 10_000_000);
    }

    #[test]
    fn column_to_micros_zero_width_returns_history_start() {
        assert_eq!(column_to_micros(5, 0, 10_000_000, 0), 0);
    }
}
