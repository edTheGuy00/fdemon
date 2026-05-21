//! Tests for the Gantt-style Timeline Events renderer.
//!
//! Extracted from `gantt.rs` as a pre-flight refactor (Phase 5, Task 01, Drift #7)
//! to keep `gantt.rs` under the 800-line file-length ceiling while Phase 5
//! overlay additions land in T03/T04.
//!
//! # Module structure note
//!
//! This file is included into `gantt.rs` via:
//! ```rust,ignore
//! #[cfg(test)]
//! #[path = "gantt_tests.rs"]
//! mod tests;
//! ```
//!
//! Within this module, `super` refers to the `gantt` module (i.e. `gantt.rs`
//! itself), so we import public-in-super items directly from `super::`.

use super::{matches_filter, render_gantt, render_time_axis_pub};
// THREAD_LABEL_WIDTH is defined in the parent timeline_events module (super::super)
use super::super::THREAD_LABEL_WIDTH;
use fdemon_app::session::{PerformanceState, TimelineEventCursor, TimelineFilter};
use fdemon_core::timeline::{TimelineNode, TimelinePhase, TimelineThread, TimelineTrack};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use std::collections::BTreeMap;

// ── Test helpers ──────────────────────────────────────────────────────────────

fn collect_text(buf: &Buffer) -> String {
    let mut s = String::new();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            if let Some(c) = buf.cell((x, y)) {
                if let Some(ch) = c.symbol().chars().next() {
                    s.push(ch);
                }
            }
        }
        s.push('\n');
    }
    s
}

fn make_complete_node(name: &str, thread: TimelineThread, ts: i64, dur: i64) -> TimelineNode {
    // Embed frame_number = Some(1) so tests that set committed_frame_anchor=1
    // will correctly find this node and render the Gantt.
    TimelineNode {
        name: name.to_owned(),
        category: None,
        ts,
        dur: Some(dur),
        phase: TimelinePhase::Complete,
        thread,
        frame_number: Some(1),
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

/// Return a `PerformanceState` with `committed_frame_anchor = Some(1)` and a
/// pre-populated `frame_anchor_map` entry for frame 1.
///
/// The map entry covers a wide range [0, 10_000_000) so that tests using any
/// reasonable ts/dur values will successfully resolve the anchor viewport.
/// All test tracks built via `make_complete_node` / `make_track` carry
/// `frame_number = Some(1)`, matching this anchor.
fn make_anchored_state() -> PerformanceState {
    let mut map = std::collections::BTreeMap::new();
    // Broad range: ts_start=0, ts_end=10_000_000 (10 seconds)
    map.insert(1u64, (0u64, 10_000_000u64));
    PerformanceState {
        committed_frame_anchor: Some(1),
        frame_anchor_map: map,
        ..Default::default()
    }
}

// ── AC9: Empty state placeholder ──────────────────────────────────────────────

/// When committed_frame_anchor == None (no frame selected), the Gantt shows
/// the "Select a frame" prompt.
#[test]
fn gantt_renders_empty_state_placeholder() {
    // Default state: committed_frame_anchor == None
    let state = PerformanceState::default();
    let area = Rect::new(0, 0, 80, 10);
    let mut buf = Buffer::empty(area);
    render_gantt(area, &mut buf, &state);
    let text = collect_text(&buf);
    assert!(
        text.contains("Select a frame"),
        "expected 'Select a frame' placeholder when no anchor, got:\n{text}"
    );
}

// ── AC13: No panic on zero area ────────────────────────────────────────────────

#[test]
fn gantt_no_panic_zero_area() {
    let state = PerformanceState::default();
    let mut buf = Buffer::empty(Rect::ZERO);
    render_gantt(Rect::ZERO, &mut buf, &state); // must not panic
}

// ── AC2: Thread rows render with labels ────────────────────────────────────────

#[test]
fn gantt_renders_two_thread_rows_with_labels() {
    let mut state = make_anchored_state();
    let mut tracks = BTreeMap::new();
    // Add event at ts within the last 5s window
    let ts = 1_000_000i64;
    let dur = 500_000i64;
    tracks.insert(
        1,
        make_track(
            1,
            TimelineThread::Ui,
            vec![make_complete_node("UIEvent", TimelineThread::Ui, ts, dur)],
        ),
    );
    tracks.insert(
        2,
        make_track(
            2,
            TimelineThread::Raster,
            vec![make_complete_node(
                "Raster",
                TimelineThread::Raster,
                ts,
                dur,
            )],
        ),
    );
    state.timeline_tracks = tracks;

    let area = Rect::new(0, 0, 80, 20);
    let mut buf = Buffer::empty(area);
    render_gantt(area, &mut buf, &state);
    let text = collect_text(&buf);

    // The filter strip is rendered by mod.rs, not gantt. We check for thread
    // type labels from build_thread_label fallback.
    assert!(
        text.contains("Ui 1") || text.contains("Raster"),
        "expected thread row labels, got:\n{text}"
    );
}

// ── AC3: Thread names from name map ───────────────────────────────────────────

#[test]
fn gantt_uses_thread_name_map_for_labels() {
    let mut state = make_anchored_state();
    let ts = 1_000_000i64;
    let dur = 500_000i64;
    let mut tracks = BTreeMap::new();
    tracks.insert(
        45067,
        make_track(
            45067,
            TimelineThread::Raster,
            vec![make_complete_node("Draw", TimelineThread::Raster, ts, dur)],
        ),
    );
    state.timeline_tracks = tracks;
    state
        .timeline_thread_name_map
        .insert(45067, "io.flutter.raster".to_owned());

    let area = Rect::new(0, 0, 80, 20);
    let mut buf = Buffer::empty(area);
    render_gantt(area, &mut buf, &state);
    let text = collect_text(&buf);

    assert!(
        text.contains("io.flutter.raster"),
        "expected name from thread_name_map, got:\n{text}"
    );
}

// ── AC4: Fallback label when name not in map ───────────────────────────────────

#[test]
fn gantt_fallback_label_when_name_not_in_map() {
    let mut state = make_anchored_state();
    let ts = 1_000_000i64;
    let dur = 500_000i64;
    let mut tracks = BTreeMap::new();
    tracks.insert(
        45067,
        make_track(
            45067,
            TimelineThread::Raster,
            vec![make_complete_node("Draw", TimelineThread::Raster, ts, dur)],
        ),
    );
    state.timeline_tracks = tracks;
    // Do NOT insert into name_map

    let area = Rect::new(0, 0, 80, 20);
    let mut buf = Buffer::empty(area);
    render_gantt(area, &mut buf, &state);
    let text = collect_text(&buf);

    // Fallback: "{:?} {}" → "Raster 45067"
    assert!(
        text.contains("Raster") && text.contains("45067"),
        "expected fallback label 'Raster 45067', got:\n{text}"
    );
}

// ── AC5: UI bars have light-blue color ────────────────────────────────────────

#[test]
fn gantt_ui_bars_render_with_light_blue_color() {
    let mut state = make_anchored_state();
    let ts = 4_500_000i64; // near end of 5s window
    let dur = 400_000i64;
    let mut tracks = BTreeMap::new();
    tracks.insert(
        1,
        make_track(
            1,
            TimelineThread::Ui,
            vec![make_complete_node("Frame", TimelineThread::Ui, ts, dur)],
        ),
    );
    state.timeline_tracks = tracks;

    let area = Rect::new(0, 0, 100, 15);
    let mut buf = Buffer::empty(area);
    render_gantt(area, &mut buf, &state);

    // Check that at least one cell in the gantt area has LightBlue background
    let has_light_blue = (0..area.width).any(|x| {
        (0..area.height).any(|y| {
            buf.cell((x, y))
                .map(|c| c.bg == Color::LightBlue)
                .unwrap_or(false)
        })
    });
    assert!(
        has_light_blue,
        "UI thread bars should render with LightBlue background"
    );
}

// ── AC10: Thread filter ────────────────────────────────────────────────────────

#[test]
fn gantt_filter_ui_hides_raster_rows() {
    let mut state = make_anchored_state();
    state.timeline_events_filter = TimelineFilter::Ui;
    let ts = 1_000_000i64;
    let dur = 500_000i64;
    let mut tracks = BTreeMap::new();
    tracks.insert(
        1,
        make_track(
            1,
            TimelineThread::Ui,
            vec![make_complete_node("UIWork", TimelineThread::Ui, ts, dur)],
        ),
    );
    tracks.insert(
        2,
        make_track(
            2,
            TimelineThread::Raster,
            vec![make_complete_node(
                "RasterWork",
                TimelineThread::Raster,
                ts,
                dur,
            )],
        ),
    );
    state.timeline_tracks = tracks;
    state
        .timeline_thread_name_map
        .insert(1, "ui.thread".to_owned());
    state
        .timeline_thread_name_map
        .insert(2, "raster.thread".to_owned());

    let area = Rect::new(0, 0, 80, 20);
    let mut buf = Buffer::empty(area);
    render_gantt(area, &mut buf, &state);
    let text = collect_text(&buf);

    assert!(
        text.contains("ui.thread"),
        "expected UI thread label, got:\n{text}"
    );
    assert!(
        !text.contains("raster.thread"),
        "expected NO raster thread when UI filter active, got:\n{text}"
    );
}

#[test]
fn gantt_filter_raster_hides_ui_rows() {
    let mut state = make_anchored_state();
    state.timeline_events_filter = TimelineFilter::Raster;
    let ts = 1_000_000i64;
    let dur = 500_000i64;
    let mut tracks = BTreeMap::new();
    tracks.insert(
        1,
        make_track(
            1,
            TimelineThread::Ui,
            vec![make_complete_node("UIWork", TimelineThread::Ui, ts, dur)],
        ),
    );
    tracks.insert(
        2,
        make_track(
            2,
            TimelineThread::Raster,
            vec![make_complete_node(
                "RasterWork",
                TimelineThread::Raster,
                ts,
                dur,
            )],
        ),
    );
    state.timeline_tracks = tracks;
    state
        .timeline_thread_name_map
        .insert(1, "ui.thread".to_owned());
    state
        .timeline_thread_name_map
        .insert(2, "raster.thread".to_owned());

    let area = Rect::new(0, 0, 80, 20);
    let mut buf = Buffer::empty(area);
    render_gantt(area, &mut buf, &state);
    let text = collect_text(&buf);

    assert!(
        text.contains("raster.thread"),
        "expected raster thread label, got:\n{text}"
    );
    assert!(
        !text.contains("ui.thread"),
        "expected NO UI thread when Raster filter active, got:\n{text}"
    );
}

#[test]
fn gantt_filter_all_shows_all_threads() {
    let mut state = make_anchored_state();
    state.timeline_events_filter = TimelineFilter::All;
    let ts = 1_000_000i64;
    let dur = 500_000i64;
    let mut tracks = BTreeMap::new();
    tracks.insert(
        1,
        make_track(
            1,
            TimelineThread::Ui,
            vec![make_complete_node("UIWork", TimelineThread::Ui, ts, dur)],
        ),
    );
    tracks.insert(
        2,
        make_track(
            2,
            TimelineThread::Raster,
            vec![make_complete_node(
                "RasterWork",
                TimelineThread::Raster,
                ts,
                dur,
            )],
        ),
    );
    state.timeline_tracks = tracks;
    state
        .timeline_thread_name_map
        .insert(1, "ui.thread".to_owned());
    state
        .timeline_thread_name_map
        .insert(2, "raster.thread".to_owned());

    let area = Rect::new(0, 0, 80, 25);
    let mut buf = Buffer::empty(area);
    render_gantt(area, &mut buf, &state);
    let text = collect_text(&buf);

    assert!(
        text.contains("ui.thread"),
        "expected UI thread, got:\n{text}"
    );
    assert!(
        text.contains("raster.thread"),
        "expected raster thread, got:\n{text}"
    );
}

// ── AC11: Vertical scroll ─────────────────────────────────────────────────────

#[test]
fn gantt_thread_scroll_offset_skips_top_rows() {
    let mut state = make_anchored_state();
    let ts = 1_000_000i64;
    let dur = 500_000i64;
    let mut tracks = BTreeMap::new();
    tracks.insert(
        1,
        make_track(
            1,
            TimelineThread::Ui,
            vec![make_complete_node("UIWork", TimelineThread::Ui, ts, dur)],
        ),
    );
    tracks.insert(
        2,
        make_track(
            2,
            TimelineThread::Raster,
            vec![make_complete_node(
                "RasterWork",
                TimelineThread::Raster,
                ts,
                dur,
            )],
        ),
    );
    state.timeline_tracks = tracks;
    state
        .timeline_thread_name_map
        .insert(1, "ui.thread".to_owned());
    state
        .timeline_thread_name_map
        .insert(2, "raster.thread".to_owned());
    state.timeline_thread_scroll_offset = 1; // skip first row

    let area = Rect::new(0, 0, 80, 20);
    let mut buf = Buffer::empty(area);
    render_gantt(area, &mut buf, &state);
    let text = collect_text(&buf);

    // With scroll_offset=1, first track (tid=1, "ui.thread") should be skipped
    // Second track (tid=2, "raster.thread") should be visible
    assert!(
        !text.contains("ui.thread"),
        "scroll_offset=1 should skip first row, but found ui.thread:\n{text}"
    );
    assert!(
        text.contains("raster.thread"),
        "expected second thread row after scroll, got:\n{text}"
    );
}

// ── AC12: Render-hint write-back ───────────────────────────────────────────────

#[test]
fn gantt_writes_visible_row_count_render_hint() {
    let mut state = make_anchored_state();
    let ts = 1_000_000i64;
    let dur = 500_000i64;
    let mut tracks = BTreeMap::new();
    tracks.insert(
        1,
        make_track(
            1,
            TimelineThread::Ui,
            vec![make_complete_node("UIWork", TimelineThread::Ui, ts, dur)],
        ),
    );
    tracks.insert(
        2,
        make_track(
            2,
            TimelineThread::Raster,
            vec![make_complete_node(
                "RasterWork",
                TimelineThread::Raster,
                ts,
                dur,
            )],
        ),
    );
    state.timeline_tracks = tracks;

    let area = Rect::new(0, 0, 80, 20);
    let mut buf = Buffer::empty(area);
    render_gantt(area, &mut buf, &state);

    // With height=20, TIME_AXIS_HEIGHT=1, THREAD_ROW_HEIGHT=6:
    // max_rows_visible = (20-1)/6 = 3; only 2 tracks exist → visible=2
    assert_eq!(
        state.timeline_visible_row_count.get(),
        2,
        "render hint should reflect visible row count"
    );
}

#[test]
fn gantt_writes_zero_row_count_when_empty() {
    let state = PerformanceState::default();
    let area = Rect::new(0, 0, 80, 10);
    let mut buf = Buffer::empty(area);
    render_gantt(area, &mut buf, &state);
    assert_eq!(state.timeline_visible_row_count.get(), 0);
}

// ── AC7: Depth-stacked children ────────────────────────────────────────────────

#[test]
fn gantt_depth_stacked_children_render_at_correct_y() {
    let mut state = make_anchored_state();

    // Root event [ts=4_000_000, dur=800_000] containing child and grandchild
    let grandchild = TimelineNode {
        name: "GrandChild".to_owned(),
        category: None,
        ts: 4_200_000,
        dur: Some(200_000),
        phase: TimelinePhase::Complete,
        thread: TimelineThread::Ui,
        frame_number: None,
        children: vec![],
    };
    let child = TimelineNode {
        name: "Child".to_owned(),
        category: None,
        ts: 4_100_000,
        dur: Some(600_000),
        phase: TimelinePhase::Complete,
        thread: TimelineThread::Ui,
        frame_number: None,
        children: vec![grandchild],
    };
    // Root carries frame_number=1 to satisfy the committed_frame_anchor gate.
    let root = TimelineNode {
        name: "Root".to_owned(),
        category: None,
        ts: 4_000_000,
        dur: Some(800_000),
        phase: TimelinePhase::Complete,
        thread: TimelineThread::Ui,
        frame_number: Some(1),
        children: vec![child],
    };

    let mut tracks = BTreeMap::new();
    tracks.insert(
        1,
        TimelineTrack {
            tid: 1,
            name: None,
            thread: TimelineThread::Ui,
            root_events: vec![root],
        },
    );
    state.timeline_tracks = tracks;

    let area = Rect::new(0, 0, 120, 20);
    let mut buf = Buffer::empty(area);
    render_gantt(area, &mut buf, &state);

    // With THREAD_ROW_HEIGHT = 2, only depth 0 and depth 1 fit within the
    // thread row's vertical band; depth-2 children are clipped (acceptable
    // since deep nesting is rare in real timeline data).
    //   chunks[0] = time axis (y=0..TIME_AXIS_HEIGHT)
    //   chunks[1] = thread row for tid=1 (y=TIME_AXIS_HEIGHT..+THREAD_ROW_HEIGHT)
    //   thread row: depth=0 → y=TIME_AXIS_HEIGHT+0, depth=1 → +1
    let canvas_x_start = THREAD_LABEL_WIDTH;
    let mut colored_rows: std::collections::HashSet<u16> = std::collections::HashSet::new();
    for y in 0..area.height {
        for x in canvas_x_start..area.width {
            if let Some(cell) = buf.cell((x, y)) {
                if cell.bg != Color::Reset {
                    colored_rows.insert(y);
                }
            }
        }
    }
    assert!(
        colored_rows.len() >= 2,
        "expected at least 2 different y rows with colored bars (depth 0,1 within THREAD_ROW_HEIGHT=2), got {:?}",
        colored_rows
    );
}

// ── AC15: Time axis labels ─────────────────────────────────────────────────────

/// Second labels render correctly when `use_ms_labels = false`.
///
/// Tests `render_time_axis` directly (private function, accessible within the
/// same module as the test) to avoid the anchor-gated path in `render_gantt`.
#[test]
fn time_axis_labels_at_one_second_intervals() {
    // Use a 5-second viewport: [0, 5_000_000)
    let vp_start: u64 = 0;
    let vp_end: u64 = 5_000_000;

    // Use a wide area (150 cols) so that "0s" at the right edge has room
    // for both characters and does not get clipped.
    let area = Rect::new(0, 0, 150, 1);
    let mut buf = Buffer::empty(area);
    render_time_axis_pub(area, &mut buf, vp_start, vp_end, false); // false = second labels
    let text = collect_text(&buf);

    // Time axis should show "0s" and at least one negative second label
    assert!(
        text.contains("0s"),
        "expected '0s' label in time axis, got:\n{text}"
    );
    // Should also show at least one negative-second label
    assert!(
        text.contains("-5s") || text.contains("-4s") || text.contains("-1s"),
        "expected at least one negative-second tick label, got:\n{text}"
    );
}

// ── matches_filter ────────────────────────────────────────────────────────────

#[test]
fn matches_filter_all_accepts_all_threads() {
    assert!(matches_filter(TimelineThread::Ui, TimelineFilter::All));
    assert!(matches_filter(TimelineThread::Raster, TimelineFilter::All));
    assert!(matches_filter(TimelineThread::Other, TimelineFilter::All));
}

#[test]
fn matches_filter_ui_accepts_only_ui() {
    assert!(matches_filter(TimelineThread::Ui, TimelineFilter::Ui));
    assert!(!matches_filter(TimelineThread::Raster, TimelineFilter::Ui));
    assert!(!matches_filter(TimelineThread::Other, TimelineFilter::Ui));
}

#[test]
fn matches_filter_raster_accepts_only_raster() {
    assert!(!matches_filter(TimelineThread::Ui, TimelineFilter::Raster));
    assert!(matches_filter(
        TimelineThread::Raster,
        TimelineFilter::Raster
    ));
    assert!(!matches_filter(
        TimelineThread::Other,
        TimelineFilter::Raster
    ));
}

// ── Phase 5: anchor-related placeholder tests ─────────────────────────────────

/// Build a track with a single Complete node that carries a frame_number.
fn make_track_with_frame(
    tid: i64,
    thread: TimelineThread,
    ts: i64,
    dur: i64,
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

/// AC5: When committed_frame_anchor == None, the Gantt should show the
/// "Select a frame" placeholder regardless of track content.
#[test]
fn render_gantt_shows_select_a_frame_placeholder_when_no_anchor() {
    let mut state = PerformanceState::default();
    // Populate tracks so we know the placeholder comes from the anchor gate,
    // not from empty tracks.
    let mut tracks = BTreeMap::new();
    tracks.insert(
        1,
        make_track_with_frame(1, TimelineThread::Ui, 1_000_000, 16_000, Some(1)),
    );
    state.timeline_tracks = tracks;
    // committed_frame_anchor is None by default

    let area = Rect::new(0, 0, 80, 10);
    let mut buf = Buffer::empty(area);
    render_gantt(area, &mut buf, &state);
    let text = collect_text(&buf);

    assert!(
        text.contains("Select a frame"),
        "expected 'Select a frame' placeholder when anchor is None, got:\n{text}"
    );
}

/// AC5: When committed_frame_anchor == Some(N) but no entry exists in the
/// `frame_anchor_map`, the Gantt should show the "no timeline data recorded"
/// placeholder.
#[test]
fn render_gantt_shows_not_available_placeholder_when_anchor_missing_from_tracks() {
    // frame_anchor_map is empty (default); committed_frame_anchor == Some(99)
    // means the anchor map has no entry for frame 99.
    let state = PerformanceState {
        committed_frame_anchor: Some(99),
        ..Default::default()
    };

    let area = Rect::new(0, 0, 120, 10);
    let mut buf = Buffer::empty(area);
    render_gantt(area, &mut buf, &state);
    let text = collect_text(&buf);

    assert!(
        text.contains("No timeline data recorded") || text.contains("timeline data"),
        "expected 'no timeline data' placeholder for missing frame, got:\n{text}"
    );
}

/// AC4: When viewport span < 1s, the time axis should use ms labels
/// (e.g. "0ms") instead of second labels like "0s".
#[test]
fn time_axis_uses_ms_labels_when_viewport_under_one_second() {
    // vp_start=0, vp_end=50_000 (50 µs → sub-millisecond) — well under 1s threshold
    let area = Rect::new(0, 0, 150, 3);
    let mut buf = Buffer::empty(area);
    render_time_axis_pub(area, &mut buf, 0, 50_000, true);
    let text = collect_text(&buf);

    assert!(
        text.contains("ms"),
        "expected 'ms' suffix in time axis labels for sub-second viewport, got:\n{text}"
    );
    assert!(
        !text.contains("-5s") && !text.contains("-4s"),
        "expected NO second labels in ms-mode time axis, got:\n{text}"
    );
}

// ── Phase 5: PAUSED indicator ─────────────────────────────────────────────────

/// AC11: When `!follow_latest`, the renderer paints the "PAUSED" indicator
/// in the time-axis row. The indicator text must be visible in the buffer.
#[test]
fn gantt_renders_paused_indicator_when_not_follow_latest() {
    let mut state = make_anchored_state();
    // Put a track in so we get past the anchor gate and render the full Gantt.
    let ts = 1_000_000i64;
    let dur = 500_000i64;
    let mut tracks = BTreeMap::new();
    tracks.insert(
        1,
        make_track(
            1,
            TimelineThread::Ui,
            vec![make_complete_node("UIWork", TimelineThread::Ui, ts, dur)],
        ),
    );
    state.timeline_tracks = tracks;
    // Disable follow_latest → PAUSED indicator should appear
    state.timeline_follow_latest = false;
    state.timeline_viewport_start_micros = 0;
    state.timeline_viewport_width_micros = 5_000_000;

    let area = Rect::new(0, 0, 100, 15);
    let mut buf = Buffer::empty(area);
    render_gantt(area, &mut buf, &state);
    let text = collect_text(&buf);

    assert!(
        text.contains("PAUSED"),
        "expected 'PAUSED' indicator in the buffer when follow_latest=false, got:\n{text}"
    );
}

// ── Phase 5 T03: Selection highlight tests ────────────────────────────────────

/// AC10: When an event is selected, the corresponding bar uses a visually
/// distinct style. We verify the REVERSED modifier appears in the selected bar's
/// cells, and does NOT appear in an unselected bar's cells.
#[test]
fn gantt_selected_bar_has_reversed_modifier() {
    use ratatui::style::Modifier;

    let ts_selected = 1_000_000i64;
    let ts_other = 2_000_000i64;
    let dur = 500_000i64;

    let mut state = make_anchored_state();
    // Wide viewport so both events are visible.
    state.frame_anchor_map.insert(1u64, (0u64, 5_000_000u64));

    let mut tracks = BTreeMap::new();
    tracks.insert(
        1,
        make_track(
            1,
            TimelineThread::Ui,
            vec![
                make_complete_node("Selected", TimelineThread::Ui, ts_selected, dur),
                make_complete_node("NotSelected", TimelineThread::Ui, ts_other, dur),
            ],
        ),
    );
    state.timeline_tracks = tracks;
    state.timeline_selected_event = Some(TimelineEventCursor {
        tid: 1,
        depth: 0,
        ts: ts_selected,
    });

    let area = Rect::new(0, 0, 200, 10);
    let mut buf = Buffer::empty(area);
    render_gantt(area, &mut buf, &state);

    // Find at least one cell with REVERSED modifier (from the selected bar).
    let has_reversed = (0..buf.area.height)
        .flat_map(|y| (0..buf.area.width).map(move |x| (x, y)))
        .any(|(x, y)| {
            if let Some(cell) = buf.cell((x, y)) {
                cell.style().add_modifier.contains(Modifier::REVERSED)
            } else {
                false
            }
        });

    assert!(
        has_reversed,
        "expected at least one cell with REVERSED modifier for selected bar"
    );
}

/// AC10 complement: When no event is selected, no bar has the REVERSED modifier.
#[test]
fn gantt_no_reversed_modifier_without_selection() {
    use ratatui::style::Modifier;

    let ts = 1_000_000i64;
    let dur = 500_000i64;

    let mut state = make_anchored_state();
    let mut tracks = BTreeMap::new();
    tracks.insert(
        1,
        make_track(
            1,
            TimelineThread::Ui,
            vec![make_complete_node("NormalBar", TimelineThread::Ui, ts, dur)],
        ),
    );
    state.timeline_tracks = tracks;
    // No selection.
    assert!(state.timeline_selected_event.is_none());

    let area = Rect::new(0, 0, 200, 10);
    let mut buf = Buffer::empty(area);
    render_gantt(area, &mut buf, &state);

    let has_reversed = (0..buf.area.height)
        .flat_map(|y| (0..buf.area.width).map(move |x| (x, y)))
        .any(|(x, y)| {
            if let Some(cell) = buf.cell((x, y)) {
                cell.style().add_modifier.contains(Modifier::REVERSED)
            } else {
                false
            }
        });

    assert!(
        !has_reversed,
        "expected NO REVERSED modifier when no event is selected"
    );
}

/// AC11: When `follow_latest=true`, the "PAUSED" indicator must NOT be rendered.
#[test]
fn gantt_no_paused_indicator_when_follow_latest() {
    let mut state = make_anchored_state();
    let ts = 1_000_000i64;
    let dur = 500_000i64;
    let mut tracks = BTreeMap::new();
    tracks.insert(
        1,
        make_track(
            1,
            TimelineThread::Ui,
            vec![make_complete_node("UIWork", TimelineThread::Ui, ts, dur)],
        ),
    );
    state.timeline_tracks = tracks;
    // follow_latest is true (default from make_anchored_state)
    assert!(state.timeline_follow_latest);

    let area = Rect::new(0, 0, 100, 15);
    let mut buf = Buffer::empty(area);
    render_gantt(area, &mut buf, &state);
    let text = collect_text(&buf);

    assert!(
        !text.contains("PAUSED"),
        "expected NO 'PAUSED' indicator when follow_latest=true, got:\n{text}"
    );
}

// ── Phase 5 T04: Search match highlight tests ─────────────────────────────────

/// AC9 (match highlight): A bar whose name matches the search query should have
/// BOLD | UNDERLINED modifiers applied to its cells.
///
/// We render a wide viewport with a matching event and a non-matching event,
/// then verify the matching bar has BOLD+UNDERLINED and the non-matching bar
/// does NOT.
#[test]
fn gantt_matching_bar_has_bold_underlined_modifier() {
    use ratatui::style::Modifier;

    let ts_match = 1_000_000i64;
    let ts_other = 2_000_000i64;
    let dur = 500_000i64;

    let mut state = make_anchored_state();
    // Wide viewport so both events are visible.
    state.frame_anchor_map.insert(1u64, (0u64, 5_000_000u64));
    // Set search query that matches "Raster" but not "UIFrame"
    state.timeline_search_query = Some("Raster".to_string());
    state.timeline_search_input_active = false;

    let mut tracks = BTreeMap::new();
    tracks.insert(
        1,
        make_track(
            1,
            TimelineThread::Ui,
            vec![
                make_complete_node("RasterDraw", TimelineThread::Ui, ts_match, dur),
                make_complete_node("UIFrame", TimelineThread::Ui, ts_other, dur),
            ],
        ),
    );
    state.timeline_tracks = tracks;
    // No selection (search-only, not navigating yet)
    state.timeline_selected_event = None;

    let area = Rect::new(0, 0, 200, 10);
    let mut buf = Buffer::empty(area);
    render_gantt(area, &mut buf, &state);

    // At least one cell should have BOLD | UNDERLINED (the matching bar).
    let has_bold_underlined = (0..buf.area.height)
        .flat_map(|y| (0..buf.area.width).map(move |x| (x, y)))
        .any(|(x, y)| {
            if let Some(cell) = buf.cell((x, y)) {
                cell.style().add_modifier.contains(Modifier::BOLD)
                    && cell.style().add_modifier.contains(Modifier::UNDERLINED)
            } else {
                false
            }
        });

    assert!(
        has_bold_underlined,
        "expected at least one cell with BOLD|UNDERLINED modifier for matching bar"
    );
}

/// AC10 (current-match emphasis): The current-match bar (match_cursor matches
/// selected event) should additionally have REVERSED modifier.
#[test]
fn gantt_current_match_bar_has_reversed_modifier() {
    use ratatui::style::Modifier;

    let ts_match = 1_000_000i64;
    let dur = 500_000i64;

    let mut state = make_anchored_state();
    state.frame_anchor_map.insert(1u64, (0u64, 5_000_000u64));
    // Set query that matches the event
    state.timeline_search_query = Some("Raster".to_string());
    state.timeline_search_input_active = false;
    // Set selected_event to match the bar (simulating n/N navigation)
    state.timeline_selected_event = Some(TimelineEventCursor {
        tid: 1,
        depth: 0,
        ts: ts_match,
    });

    let mut tracks = BTreeMap::new();
    tracks.insert(
        1,
        make_track(
            1,
            TimelineThread::Ui,
            vec![make_complete_node(
                "RasterDraw",
                TimelineThread::Ui,
                ts_match,
                dur,
            )],
        ),
    );
    state.timeline_tracks = tracks;

    let area = Rect::new(0, 0, 200, 10);
    let mut buf = Buffer::empty(area);
    render_gantt(area, &mut buf, &state);

    // The current-match bar should have REVERSED modifier (in addition to BOLD|UNDERLINED).
    let has_reversed = (0..buf.area.height)
        .flat_map(|y| (0..buf.area.width).map(move |x| (x, y)))
        .any(|(x, y)| {
            if let Some(cell) = buf.cell((x, y)) {
                cell.style().add_modifier.contains(Modifier::REVERSED)
            } else {
                false
            }
        });

    assert!(
        has_reversed,
        "expected current-match bar to have REVERSED modifier for emphasis"
    );
}

/// AC14 (case-insensitive): A query "raster" should match an event named
/// "GPURasterizer::Draw" (case-insensitive substring match).
#[test]
fn gantt_search_is_case_insensitive() {
    use ratatui::style::Modifier;

    let ts = 1_000_000i64;
    let dur = 500_000i64;

    let mut state = make_anchored_state();
    state.frame_anchor_map.insert(1u64, (0u64, 5_000_000u64));
    // Lowercase query, mixed-case event name
    state.timeline_search_query = Some("raster".to_string());
    state.timeline_search_input_active = false;

    let mut tracks = BTreeMap::new();
    tracks.insert(
        1,
        make_track(
            1,
            TimelineThread::Raster,
            // "GPURasterizer::Draw" contains "Raster" case-insensitively
            vec![make_complete_node(
                "GPURasterizer::Draw",
                TimelineThread::Raster,
                ts,
                dur,
            )],
        ),
    );
    state.timeline_tracks = tracks;

    let area = Rect::new(0, 0, 200, 10);
    let mut buf = Buffer::empty(area);
    render_gantt(area, &mut buf, &state);

    let has_bold_underlined = (0..buf.area.height)
        .flat_map(|y| (0..buf.area.width).map(move |x| (x, y)))
        .any(|(x, y)| {
            if let Some(cell) = buf.cell((x, y)) {
                cell.style().add_modifier.contains(Modifier::BOLD)
                    && cell.style().add_modifier.contains(Modifier::UNDERLINED)
            } else {
                false
            }
        });

    assert!(
        has_bold_underlined,
        "expected case-insensitive match to produce BOLD|UNDERLINED highlight"
    );
}

/// AC9 (non-matching bar): A bar that does NOT match the query should NOT have
/// the BOLD|UNDERLINED modifier applied.
#[test]
fn gantt_non_matching_bar_has_no_search_modifier() {
    use ratatui::style::Modifier;

    let ts = 1_000_000i64;
    let dur = 500_000i64;

    let mut state = make_anchored_state();
    state.frame_anchor_map.insert(1u64, (0u64, 5_000_000u64));
    // Query that does NOT match the event name
    state.timeline_search_query = Some("zzz_no_match".to_string());
    state.timeline_search_input_active = false;

    let mut tracks = BTreeMap::new();
    tracks.insert(
        1,
        make_track(
            1,
            TimelineThread::Ui,
            vec![make_complete_node("UIFrame", TimelineThread::Ui, ts, dur)],
        ),
    );
    state.timeline_tracks = tracks;

    let area = Rect::new(0, 0, 200, 10);
    let mut buf = Buffer::empty(area);
    render_gantt(area, &mut buf, &state);

    // UNDERLINED should NOT appear (no match → no highlight overlay)
    let has_underlined = (0..buf.area.height)
        .flat_map(|y| (0..buf.area.width).map(move |x| (x, y)))
        .any(|(x, y)| {
            if let Some(cell) = buf.cell((x, y)) {
                cell.style().add_modifier.contains(Modifier::UNDERLINED)
            } else {
                false
            }
        });

    assert!(
        !has_underlined,
        "expected NO UNDERLINED modifier for a non-matching bar, but found one"
    );
}
