//! Viewport math helpers for the Gantt-style timeline events widget.
//!
//! Pure functions — no rendering, no state mutation. All computations map
//! microsecond timestamps to terminal column offsets within a fixed-width
//! canvas.

use std::collections::BTreeMap;

use fdemon_core::timeline::{TimelineNode, TimelineThread, TimelineTrack};

use super::TIMELINE_VIEWPORT_MICROS;

// ── Frame-anchored viewport ───────────────────────────────────────────────────

/// Padding added to each side of the anchored viewport, as a fraction of the
/// frame duration. Clamped to [`ANCHOR_PADDING_MIN_MICROS`]..=
/// [`ANCHOR_PADDING_MAX_MICROS`].
const ANCHOR_PADDING_FRACTION: f64 = 0.20;

/// Minimum viewport padding on each side (2 ms) — prevents a zero-duration
/// event from producing a zero-width viewport.
const ANCHOR_PADDING_MIN_MICROS: u64 = 2_000;

/// Maximum viewport padding on each side (50 ms) — avoids swamping a very
/// long frame with excessive whitespace.
const ANCHOR_PADDING_MAX_MICROS: u64 = 50_000;

/// Compute a viewport anchored to the events belonging to `frame_number`.
///
/// Searches all tracks (UI thread first for stability, then others) for any
/// root-level [`TimelineNode`] whose `frame_number == Some(frame_number)`.
/// Uses the first match found.
///
/// Returns `(vp_start_micros, vp_end_micros)` where:
/// - `vp_start = ts - padding`
/// - `vp_end   = ts + dur + padding`
/// - `padding ≈ 20% of dur`, clamped to [`ANCHOR_PADDING_MIN_MICROS`] ..=
///   [`ANCHOR_PADDING_MAX_MICROS`].
///
/// Returns `None` when no node carrying `frame_number` exists in `tracks`.
pub(super) fn compute_frame_anchored_viewport(
    tracks: &BTreeMap<i64, TimelineTrack>,
    frame_number: u64,
) -> Option<(u64, u64)> {
    // Search UI thread first for a stable, canonical frame boundary.
    let node = find_node_with_frame_number(tracks, frame_number, true)
        .or_else(|| find_node_with_frame_number(tracks, frame_number, false))?;

    let ts = node.ts as u64;
    let dur = node.dur.unwrap_or(0) as u64;

    let raw_padding = (dur as f64 * ANCHOR_PADDING_FRACTION) as u64;
    let padding = raw_padding.clamp(ANCHOR_PADDING_MIN_MICROS, ANCHOR_PADDING_MAX_MICROS);

    let vp_start = ts.saturating_sub(padding);
    let vp_end = ts.saturating_add(dur).saturating_add(padding);

    Some((vp_start, vp_end))
}

/// Walk `tracks` looking for a root-level `TimelineNode` whose
/// `frame_number == Some(target)`.
///
/// When `ui_only` is `true` only tracks classified as [`TimelineThread::Ui`]
/// are searched; when `false` all tracks (excluding Ui) are searched.
/// Children are not searched — frame numbers appear on root nodes.
fn find_node_with_frame_number(
    tracks: &BTreeMap<i64, TimelineTrack>,
    target: u64,
    ui_only: bool,
) -> Option<&TimelineNode> {
    for track in tracks.values() {
        let is_ui = track.thread == TimelineThread::Ui;
        if ui_only != is_ui {
            continue;
        }
        for node in &track.root_events {
            if node.frame_number == Some(target) {
                return Some(node);
            }
        }
    }
    None
}

/// Returns the `(start_micros, end_micros)` viewport bounds based on the
/// latest event timestamp across all tracks.
///
/// If tracks are empty, returns `(0, TIMELINE_VIEWPORT_MICROS)`.
/// The viewport always spans exactly [`TIMELINE_VIEWPORT_MICROS`] microseconds,
/// ending at the latest observed event timestamp (auto-scroll to live edge).
///
/// The Gantt renderer now uses `compute_frame_anchored_viewport` instead;
/// this function is retained for tests and potential future use.
#[allow(dead_code)]
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
                frame_number: None,
                children: vec![],
            }],
        }
    }

    // ── compute_frame_anchored_viewport ──────────────────────────────────────

    fn make_track_with_frame_event(
        tid: i64,
        ts: i64,
        dur: i64,
        thread: TimelineThread,
        frame_number: Option<u64>,
    ) -> TimelineTrack {
        TimelineTrack {
            tid,
            name: None,
            thread,
            root_events: vec![TimelineNode {
                name: "Frame".to_owned(),
                category: None,
                ts,
                dur: Some(dur),
                phase: TimelinePhase::Complete,
                thread,
                frame_number,
                children: vec![],
            }],
        }
    }

    #[test]
    fn compute_frame_anchored_viewport_finds_event_by_frame_number() {
        let mut tracks = BTreeMap::new();
        // UI thread event at ts=1_000_000, dur=16_000, frame_number=42
        tracks.insert(
            1,
            make_track_with_frame_event(1, 1_000_000, 16_000, TimelineThread::Ui, Some(42)),
        );

        let result = compute_frame_anchored_viewport(&tracks, 42);
        assert!(result.is_some(), "should find frame 42");
        let (start, end) = result.unwrap();

        // Duration = 16_000µs, padding = max(16_000 * 0.2, 2_000) = max(3_200, 2_000) = 3_200
        // vp_start = 1_000_000 - 3_200 = 996_800
        // vp_end   = 1_000_000 + 16_000 + 3_200 = 1_019_200
        assert_eq!(start, 996_800, "start should be ts minus padding");
        assert_eq!(end, 1_019_200, "end should be ts+dur plus padding");
    }

    #[test]
    fn compute_frame_anchored_viewport_returns_none_if_frame_not_in_tracks() {
        let mut tracks = BTreeMap::new();
        tracks.insert(
            1,
            make_track_with_frame_event(1, 1_000_000, 16_000, TimelineThread::Ui, Some(1)),
        );

        let result = compute_frame_anchored_viewport(&tracks, 99);
        assert!(
            result.is_none(),
            "should return None for a frame number not in any track"
        );
    }

    #[test]
    fn compute_frame_anchored_viewport_prefers_ui_thread() {
        let mut tracks = BTreeMap::new();
        // UI thread event for frame 7 at ts=2_000_000
        tracks.insert(
            1,
            make_track_with_frame_event(1, 2_000_000, 8_000, TimelineThread::Ui, Some(7)),
        );
        // Raster thread event for frame 7 at ts=2_010_000
        tracks.insert(
            2,
            make_track_with_frame_event(2, 2_010_000, 8_000, TimelineThread::Raster, Some(7)),
        );

        let result = compute_frame_anchored_viewport(&tracks, 7);
        assert!(result.is_some());
        let (start, _end) = result.unwrap();

        // UI thread ts=2_000_000 should be preferred; padding = max(8000*0.2, 2000) = 2000
        // vp_start = 2_000_000 - 2_000 = 1_998_000
        assert_eq!(start, 1_998_000, "UI thread event should be preferred");
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
