//! Viewport math helpers for the Gantt-style timeline events widget.
//!
//! Pure functions — no rendering, no state mutation. All computations map
//! microsecond timestamps to terminal column offsets within a fixed-width
//! canvas.

use std::collections::BTreeMap;

use fdemon_app::session::PerformanceState;
use fdemon_core::timeline::TimelineTrack;

use super::TIMELINE_VIEWPORT_MICROS;

// ── Phase 5: Viewport constants ───────────────────────────────────────────────

/// Minimum viewport width — prevents over-zoom into a single μs.
///
/// 100 ms: the narrowest useful window for inspecting a single frame's events.
pub(super) const TIMELINE_VIEWPORT_MIN_MICROS: u64 = 100_000;

/// Maximum viewport width — prevents over-zoom out beyond useful history.
///
/// 60 s: matches the approximate size of the timeline track ring buffer.
// The handler math lives in the app crate; this constant is retained here for
// tests and for future TUI-side validation (T03/T04).
#[allow(dead_code)]
pub(super) const TIMELINE_VIEWPORT_MAX_MICROS: u64 = 60_000_000;

/// Zoom factor per `+`/`-` keypress.
///
/// 2× per keypress matches the DevTools convention; finer granularity would
/// require more keypresses to cover the 100 ms → 60 s range (4 keypresses
/// each way at 2×).
// Used in viewport tests; the handler math lives in the app crate.
#[allow(dead_code)]
pub(super) const TIMELINE_ZOOM_FACTOR: f64 = 2.0;

/// Pan distance per `←`/`→` keypress as a fraction of the current viewport width.
///
/// 10% per keypress: 10 keypresses pan one full viewport width.
// Used in viewport tests; the handler math lives in the app crate.
#[allow(dead_code)]
pub(super) const TIMELINE_PAN_FRACTION: f64 = 0.10;

// ── PanDirection ─────────────────────────────────────────────────────────────

/// Direction of a single pan action.
// Used in viewport tests; the handler math lives in the app crate.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PanDirection {
    Left,
    Right,
}

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

/// Compute a viewport anchored to the recorded range for `frame_number`.
///
/// Looks up `frame_number` in the persistent `frame_anchor_map` which is
/// populated during timeline event ingestion. Unlike the previous
/// track-scanning approach, this map survives `timeline_tracks` eviction, so
/// anchoring on older frames works even after their raw events have aged out
/// of the event buffer.
///
/// Returns `(vp_start_micros, vp_end_micros)` where:
/// - `vp_start = ts_start - padding`
/// - `vp_end   = ts_end   + padding`
/// - `padding ≈ 20% of (ts_end - ts_start)`, clamped to
///   [`ANCHOR_PADDING_MIN_MICROS`] ..= [`ANCHOR_PADDING_MAX_MICROS`].
///
/// Returns `None` when `frame_number` has no entry in `frame_anchor_map`
/// (the frame pre-dates the Performance panel opening, or its anchor events
/// lacked `args.frame_number`).
pub(super) fn compute_frame_anchored_viewport(
    frame_anchor_map: &BTreeMap<u64, (u64, u64)>,
    frame_number: u64,
) -> Option<(u64, u64)> {
    let &(ts_start, ts_end) = frame_anchor_map.get(&frame_number)?;
    let dur = ts_end.saturating_sub(ts_start);
    let raw_padding = (dur as f64 * ANCHOR_PADDING_FRACTION) as u64;
    let padding = raw_padding.clamp(ANCHOR_PADDING_MIN_MICROS, ANCHOR_PADDING_MAX_MICROS);
    Some((
        ts_start.saturating_sub(padding),
        ts_end.saturating_add(padding),
    ))
}

/// Returns the `(start_micros, end_micros)` viewport bounds for the Gantt canvas.
///
/// Resolves the active viewport through three modes in priority order (PLAN D2):
///
/// 1. **`!follow_latest`** — manual viewport `(start, start + width)`.
///    Pan/zoom mode; the frame anchor is preserved but ignored until the user
///    presses `g`/`End` to resume follow-latest.
/// 2. **`follow_latest && committed_frame_anchor.is_some()`** — frame-anchored
///    viewport computed from `frame_anchor_map`. Returns the live-edge fallback
///    (mode 3) if the frame number is missing from the map.
/// 3. **`follow_latest && no frame anchor`** — live-edge window: the most recent
///    [`TIMELINE_VIEWPORT_MICROS`] of timeline history.
///
/// This function is the **single source of truth** for viewport resolution; the
/// Gantt renderer calls it unconditionally rather than dispatching to individual
/// compute helpers.
pub(super) fn compute_active_viewport(perf: &PerformanceState) -> (u64, u64) {
    if !perf.timeline_follow_latest {
        let start = perf.timeline_viewport_start_micros;
        let width = perf
            .timeline_viewport_width_micros
            .max(TIMELINE_VIEWPORT_MIN_MICROS);
        return (start, start.saturating_add(width));
    }
    if let Some(frame) = perf.committed_frame_anchor {
        if let Some((s, e)) = compute_frame_anchored_viewport(&perf.frame_anchor_map, frame) {
            return (s, e);
        }
    }
    // Live-edge fallback.
    compute_viewport(&perf.timeline_tracks)
}

/// Compute new viewport bounds after a zoom action.
///
/// `factor < 1.0` zooms in (narrows the viewport); `factor > 1.0` zooms out.
/// `anchor_micros` is the timestamp that should remain at the same column
/// after the zoom (defaults to viewport center when the caller passes the
/// midpoint).
///
/// Returns `(new_start, new_end)` where `new_end - new_start == new_width`.
/// The result is NOT clamped to `[MIN, MAX]` — the caller must clamp the width
/// and re-derive start/end if needed (see `handle_zoom_in`/`handle_zoom_out`).
// Used in viewport tests and available for T03/T04 overlay hit-testing;
// the zooming is also re-implemented in the app crate for the message handler.
#[allow(dead_code)]
pub(super) fn zoom_viewport(start: u64, width: u64, factor: f64, anchor_micros: u64) -> (u64, u64) {
    let new_width_f = width as f64 * factor;
    let new_width = (new_width_f.round() as u64).max(1);

    // Preserve the relative position of anchor_micros within the viewport.
    // anchor_fraction ∈ [0.0, 1.0] (clamped for safety).
    let anchor_fraction = if width == 0 {
        0.5
    } else {
        let offset = anchor_micros.saturating_sub(start);
        (offset as f64 / width as f64).clamp(0.0, 1.0)
    };

    let anchor_new_offset = (anchor_fraction * new_width as f64).round() as u64;
    let new_start = anchor_micros.saturating_sub(anchor_new_offset);
    let new_end = new_start.saturating_add(new_width);
    (new_start, new_end)
}

/// Compute the new viewport start after panning by `direction * fraction * width`.
///
/// Returns the new `start_micros`. The caller keeps the width unchanged.
/// Panning left saturates at 0; panning right has no upper bound (the caller
/// may choose to clamp against the latest known event timestamp if desired, but
/// this function does not require that information).
// Used in viewport tests; the handler math lives in the app crate.
#[allow(dead_code)]
pub(super) fn pan_viewport(start: u64, width: u64, direction: PanDirection, fraction: f64) -> u64 {
    let delta = (width as f64 * fraction).round() as u64;
    match direction {
        PanDirection::Left => start.saturating_sub(delta),
        PanDirection::Right => start.saturating_add(delta),
    }
}

/// Returns the `(start_micros, end_micros)` viewport bounds based on the
/// latest event timestamp across all tracks.
///
/// If tracks are empty, returns `(0, TIMELINE_VIEWPORT_MICROS)`.
/// The viewport always spans exactly [`TIMELINE_VIEWPORT_MICROS`] microseconds,
/// ending at the latest observed event timestamp (auto-scroll to live edge).
///
/// Used as the live-edge fallback in [`compute_active_viewport`].
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

    /// Helper: build a frame_anchor_map entry `frame_number → (ts, ts+dur)`.
    fn make_anchor_map(entries: &[(u64, u64, u64)]) -> BTreeMap<u64, (u64, u64)> {
        entries
            .iter()
            .map(|&(frame, ts, dur)| (frame, (ts, ts + dur)))
            .collect()
    }

    /// Task-specified test: map with `{42: (1_000_000, 1_016_000)}` must produce
    /// a viewport that covers the frame with padding.
    #[test]
    fn compute_frame_anchored_viewport_reads_from_map() {
        // frame 42: ts_start=1_000_000, ts_end=1_016_000, dur=16_000
        let map = make_anchor_map(&[(42, 1_000_000, 16_000)]);

        let result = compute_frame_anchored_viewport(&map, 42);
        assert!(result.is_some(), "should find frame 42 in the map");
        let (start, end) = result.unwrap();

        // dur = 16_000µs, padding = max(16_000 * 0.2, 2_000) = max(3_200, 2_000) = 3_200
        // vp_start = 1_000_000 - 3_200 = 996_800
        // vp_end   = 1_016_000 + 3_200 = 1_019_200
        assert_eq!(start, 996_800, "start should be ts_start minus padding");
        assert_eq!(end, 1_019_200, "end should be ts_end plus padding");
    }

    /// Task-specified test: empty map → None for any frame.
    #[test]
    fn compute_frame_anchored_viewport_returns_none_for_missing_frame() {
        let map: BTreeMap<u64, (u64, u64)> = BTreeMap::new();
        let result = compute_frame_anchored_viewport(&map, 42);
        assert!(result.is_none(), "empty map should return None");
    }

    #[test]
    fn compute_frame_anchored_viewport_returns_none_if_frame_not_in_map() {
        let map = make_anchor_map(&[(1, 1_000_000, 16_000)]);
        let result = compute_frame_anchored_viewport(&map, 99);
        assert!(
            result.is_none(),
            "should return None for a frame number not in the map"
        );
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

    // ── Phase 5: compute_active_viewport ─────────────────────────────────────

    use fdemon_app::session::PerformanceState;

    /// AC7a: `!follow_latest` → manual viewport overrides frame anchor.
    ///
    /// Uses a width of 200_000 µs (200 ms), which is above `TIMELINE_VIEWPORT_MIN_MICROS`
    /// (100 ms) so the min-clamp does not affect the result.
    #[test]
    fn test_compute_active_viewport_manual_overrides_anchor() {
        let mut anchor_map = BTreeMap::new();
        anchor_map.insert(1u64, (9_000_000u64, 9_016_000u64));
        // Manual viewport: follow_latest=false, start=1_000_000, width=200_000
        let perf = PerformanceState {
            committed_frame_anchor: Some(1),
            frame_anchor_map: anchor_map,
            timeline_follow_latest: false,
            timeline_viewport_start_micros: 1_000_000,
            timeline_viewport_width_micros: 200_000,
            ..Default::default()
        };

        let (start, end) = compute_active_viewport(&perf);
        assert_eq!(start, 1_000_000, "manual start should be 1_000_000");
        assert_eq!(
            end, 1_200_000,
            "manual end should be start + width = 1_200_000"
        );
    }

    /// AC7b: `follow_latest && Some(frame)` → frame-anchored viewport.
    #[test]
    fn test_compute_active_viewport_frame_anchor_when_follow_latest() {
        let mut anchor_map = BTreeMap::new();
        anchor_map.insert(42u64, (1_000_000u64, 1_016_000u64));
        let perf = PerformanceState {
            timeline_follow_latest: true,
            committed_frame_anchor: Some(42),
            frame_anchor_map: anchor_map,
            ..Default::default()
        };

        let (start, end) = compute_active_viewport(&perf);
        // Frame-anchored: padding = max(16_000 * 0.2, 2_000) = 3_200
        // vp_start = 1_000_000 - 3_200 = 996_800
        // vp_end   = 1_016_000 + 3_200 = 1_019_200
        assert_eq!(start, 996_800, "should be frame-anchored start");
        assert_eq!(end, 1_019_200, "should be frame-anchored end");
    }

    /// AC7c: `follow_latest && None` → live-edge from compute_viewport.
    #[test]
    fn test_compute_active_viewport_live_edge_fallback() {
        // No committed_frame_anchor.
        // Add a track with a known latest event so we can predict the viewport.
        let mut tracks = BTreeMap::new();
        tracks.insert(
            1,
            TimelineTrack {
                tid: 1,
                name: None,
                thread: TimelineThread::Ui,
                root_events: vec![TimelineNode {
                    name: "E".to_owned(),
                    category: None,
                    ts: 8_000_000,
                    dur: Some(200_000),
                    phase: TimelinePhase::Complete,
                    thread: TimelineThread::Ui,
                    frame_number: None,
                    children: vec![],
                }],
            },
        );
        let perf = PerformanceState {
            timeline_follow_latest: true,
            timeline_tracks: tracks,
            ..Default::default()
        };

        let (start, end) = compute_active_viewport(&perf);
        // live-edge: end = max(8_200_000, TIMELINE_VIEWPORT_MICROS)
        //            start = end - TIMELINE_VIEWPORT_MICROS
        assert_eq!(
            end - start,
            TIMELINE_VIEWPORT_MICROS,
            "live-edge viewport should span exactly TIMELINE_VIEWPORT_MICROS"
        );
        assert!(end >= 8_200_000, "live-edge end should be >= event end");
    }

    // ── Phase 5: zoom_viewport ────────────────────────────────────────────────

    /// AC10: Zooming in from (start=1000, width=4000) with anchor=3000 preserves
    /// the center: result start=2000, width=2000.
    #[test]
    fn test_zoom_viewport_preserves_anchor_column() {
        // start=1000, width=4000, factor=0.5 (zoom in), anchor=3000
        // anchor_fraction = (3000 - 1000) / 4000 = 0.5
        // new_width = 4000 * 0.5 = 2000
        // anchor_new_offset = 0.5 * 2000 = 1000
        // new_start = 3000 - 1000 = 2000
        let (new_start, new_end) = zoom_viewport(1000, 4000, 0.5, 3000);
        assert_eq!(new_start, 2000, "zoom-in start should be 2000");
        assert_eq!(new_end - new_start, 2000, "zoom-in width should be 2000");
    }

    #[test]
    fn test_zoom_viewport_out_doubles_width() {
        // start=0, width=1000, factor=2.0 (zoom out), anchor=500 (center)
        // new_width = 2000
        // anchor_fraction = 0.5, anchor_new_offset = 1000
        // new_start = 500 - 1000 → saturating = 0
        let (new_start, new_end) = zoom_viewport(0, 1000, 2.0, 500);
        assert_eq!(new_end - new_start, 2000, "zoom-out width should be 2000");
        // new_start saturates at 0 since 500 - 1000 underflows
        assert_eq!(new_start, 0, "saturated start at 0");
    }

    // ── Phase 5: pan_viewport ─────────────────────────────────────────────────

    #[test]
    fn test_pan_viewport_right_increments_start() {
        // start=1000, width=10000, fraction=0.10 → delta=1000
        let new_start = pan_viewport(1000, 10_000, PanDirection::Right, 0.10);
        assert_eq!(new_start, 2000);
    }

    #[test]
    fn test_pan_viewport_left_decrements_start() {
        let new_start = pan_viewport(5000, 10_000, PanDirection::Left, 0.10);
        assert_eq!(new_start, 4000);
    }

    #[test]
    fn test_pan_viewport_left_saturates_at_zero() {
        // start=100, delta=1000 → would underflow → saturate at 0
        let new_start = pan_viewport(100, 10_000, PanDirection::Left, 0.10);
        assert_eq!(new_start, 0);
    }
}
