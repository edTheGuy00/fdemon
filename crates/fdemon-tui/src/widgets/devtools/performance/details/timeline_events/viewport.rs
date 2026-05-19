//! Viewport math helpers for the Gantt-style timeline events widget.
//!
//! Pure functions — no rendering, no state mutation. All computations map
//! microsecond timestamps to terminal column offsets within a fixed-width
//! canvas.

use std::collections::BTreeMap;

use fdemon_core::timeline::TimelineTrack;

use super::TIMELINE_VIEWPORT_MICROS;

/// Returns the `(start_micros, end_micros)` viewport bounds based on the
/// latest event timestamp across all tracks.
///
/// If tracks are empty, returns `(0, TIMELINE_VIEWPORT_MICROS)`.
/// The viewport always spans exactly [`TIMELINE_VIEWPORT_MICROS`] microseconds,
/// ending at the latest observed event timestamp (auto-scroll to live edge).
pub(super) fn compute_viewport(tracks: &BTreeMap<i64, TimelineTrack>) -> (u64, u64) {
    let latest_ts: u64 = tracks
        .values()
        .flat_map(|track| track.root_events.iter())
        .map(|node| {
            let end = node.ts + node.dur.unwrap_or(0);
            end.max(node.ts) as u64
        })
        .max()
        .unwrap_or(0);

    if latest_ts == 0 {
        return (0, TIMELINE_VIEWPORT_MICROS);
    }

    let end = latest_ts.max(TIMELINE_VIEWPORT_MICROS);
    let start = end - TIMELINE_VIEWPORT_MICROS;
    (start, end)
}

/// Maps a microsecond timestamp to a column offset within `time_canvas_width`.
///
/// Returns a value in `[0, time_canvas_width)`, clamped at both ends.
///
/// When `end <= start` (degenerate viewport), returns 0.
pub(super) fn micros_to_column(ts: u64, start: u64, end: u64, width: u16) -> u16 {
    if end <= start || width == 0 {
        return 0;
    }
    let span = end - start;
    let clamped_ts = ts.clamp(start, end);
    let offset = clamped_ts - start;
    // offset / span * width, computed in u64 to avoid overflow
    let col = (offset as u128 * width as u128 / span as u128) as u16;
    col.min(width.saturating_sub(1))
}

/// Clips a `(ts, dur)` event bar to the viewport.
///
/// Returns `(col_start, col_width)` in column coordinates within the canvas,
/// or `None` if the bar is entirely outside the viewport.
///
/// # Arguments
/// * `ts`           — event start timestamp in microseconds
/// * `dur`          — event duration in microseconds
/// * `vp_start`     — viewport start in microseconds
/// * `vp_end`       — viewport end in microseconds
/// * `canvas_width` — width of the time canvas in terminal columns
pub(super) fn clip_bar(
    ts: u64,
    dur: u64,
    vp_start: u64,
    vp_end: u64,
    canvas_width: u16,
) -> Option<(u16, u16)> {
    if canvas_width == 0 || vp_end <= vp_start {
        return None;
    }

    let bar_end = ts.saturating_add(dur);

    // Entirely before viewport
    if bar_end <= vp_start {
        return None;
    }
    // Entirely after viewport
    if ts >= vp_end {
        return None;
    }

    // Clip to viewport bounds
    let clipped_start = ts.max(vp_start);
    let clipped_end = bar_end.min(vp_end);

    let col_start = micros_to_column(clipped_start, vp_start, vp_end, canvas_width);
    let col_end = micros_to_column(clipped_end, vp_start, vp_end, canvas_width);

    // col_end is exclusive (past the last column of the bar)
    // For zero-duration or sub-pixel bars, ensure at least MIN_BAR_WIDTH=1 column.
    let width = if col_end > col_start {
        col_end - col_start
    } else {
        1
    };

    Some((col_start, width))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use fdemon_core::timeline::{TimelineNode, TimelinePhase, TimelineThread, TimelineTrack};
    use std::collections::BTreeMap;

    fn make_track_with_event(tid: i64, ts: i64, dur: i64) -> TimelineTrack {
        TimelineTrack {
            tid,
            name: None,
            thread: TimelineThread::Ui,
            root_events: vec![TimelineNode {
                name: "Test".to_owned(),
                category: None,
                ts,
                dur: Some(dur),
                phase: TimelinePhase::Complete,
                thread: TimelineThread::Ui,
                children: vec![],
            }],
        }
    }

    // ── compute_viewport ──────────────────────────────────────────────────────

    #[test]
    fn compute_viewport_empty_tracks_returns_default() {
        let tracks: BTreeMap<i64, TimelineTrack> = BTreeMap::new();
        let (start, end) = compute_viewport(&tracks);
        assert_eq!(start, 0);
        assert_eq!(end, TIMELINE_VIEWPORT_MICROS);
    }

    #[test]
    fn compute_viewport_single_event_spans_viewport_micros() {
        let mut tracks = BTreeMap::new();
        // Event at ts=10_000_000, dur=1_000_000 → latest=11_000_000
        tracks.insert(1, make_track_with_event(1, 10_000_000, 1_000_000));
        let (start, end) = compute_viewport(&tracks);
        assert_eq!(end - start, TIMELINE_VIEWPORT_MICROS);
        assert!(end >= 11_000_000, "viewport end should be >= event end");
    }

    #[test]
    fn compute_viewport_multiple_tracks_uses_latest() {
        let mut tracks = BTreeMap::new();
        tracks.insert(1, make_track_with_event(1, 1_000_000, 500_000));
        tracks.insert(2, make_track_with_event(2, 8_000_000, 200_000));
        let (start, end) = compute_viewport(&tracks);
        // Latest event ends at 8_200_000; viewport spans TIMELINE_VIEWPORT_MICROS=5_000_000
        assert_eq!(end - start, TIMELINE_VIEWPORT_MICROS);
        assert!(end >= 8_200_000);
    }

    // ── micros_to_column ──────────────────────────────────────────────────────

    #[test]
    fn micros_to_column_start_maps_to_zero() {
        assert_eq!(micros_to_column(0, 0, 1_000_000, 100), 0);
    }

    #[test]
    fn micros_to_column_end_maps_to_width_minus_one() {
        let col = micros_to_column(1_000_000, 0, 1_000_000, 100);
        assert_eq!(col, 99); // clamped to width-1
    }

    #[test]
    fn micros_to_column_midpoint_maps_to_half_width() {
        // midpoint of [0, 1_000_000] → column 50 in a 100-wide canvas
        let col = micros_to_column(500_000, 0, 1_000_000, 100);
        assert_eq!(col, 50);
    }

    #[test]
    fn micros_to_column_zero_width_returns_zero() {
        assert_eq!(micros_to_column(500_000, 0, 1_000_000, 0), 0);
    }

    #[test]
    fn micros_to_column_degenerate_viewport_returns_zero() {
        // end <= start
        assert_eq!(micros_to_column(500_000, 1_000_000, 0, 100), 0);
    }

    // ── clip_bar ──────────────────────────────────────────────────────────────

    #[test]
    fn clip_bar_entirely_before_viewport_returns_none() {
        // bar: [0, 100_000), viewport: [200_000, 1_200_000)
        assert!(clip_bar(0, 100_000, 200_000, 1_200_000, 100).is_none());
    }

    #[test]
    fn clip_bar_entirely_after_viewport_returns_none() {
        // bar: [2_000_000, 2_500_000), viewport: [0, 1_000_000)
        assert!(clip_bar(2_000_000, 500_000, 0, 1_000_000, 100).is_none());
    }

    #[test]
    fn clip_bar_fully_inside_viewport_returns_correct_columns() {
        // viewport: [0, 1_000_000), canvas: 100 cols
        // bar: [250_000, 750_000) → cols 25..75 → width=50
        let result = clip_bar(250_000, 500_000, 0, 1_000_000, 100);
        assert!(result.is_some(), "bar inside viewport should return Some");
        let (col_start, col_width) = result.unwrap();
        assert_eq!(col_start, 25);
        assert_eq!(col_width, 50);
    }

    #[test]
    fn clip_bar_zero_duration_returns_min_bar_width() {
        // Zero-duration event at midpoint
        let result = clip_bar(500_000, 0, 0, 1_000_000, 100);
        assert!(result.is_some());
        let (_, col_width) = result.unwrap();
        assert!(
            col_width >= 1,
            "zero-duration bar should have at least 1 column"
        );
    }

    #[test]
    fn clip_bar_zero_canvas_width_returns_none() {
        assert!(clip_bar(500_000, 100_000, 0, 1_000_000, 0).is_none());
    }
}
