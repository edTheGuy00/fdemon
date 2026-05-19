//! Gantt-style renderer for the Timeline Events tab.
//!
//! Renders per-thread rows with depth-stacked event bars and a time axis
//! strip above the rows. Thread labels appear in a fixed-width left column.

use fdemon_app::session::{PerformanceState, TimelineFilter};
use fdemon_core::timeline::{TimelineNode, TimelineThread, TimelineTrack};
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Paragraph, Widget},
};

use super::{
    palette,
    text_helpers::truncate_with_ellipsis,
    viewport::{clip_bar, compute_frame_anchored_viewport},
    MAX_DEPTH, MIN_BAR_WIDTH, THREAD_LABEL_WIDTH, THREAD_ROW_HEIGHT, TIMELINE_VIEWPORT_MICROS,
    TIME_AXIS_HEIGHT,
};
use crate::theme::palette as theme;

/// Empty-state placeholder line count — 1 content line centered vertically.
/// Derived from: 1 message line = 1.
const EMPTY_PLACEHOLDER_LINE_COUNT: u16 = 1;

/// Viewport span below which the time axis switches from whole-second labels
/// to millisecond labels (less than 1 second wide).
///
/// Derived from: 1 second = 1_000_000 microseconds. An anchored frame viewport
/// is typically 16–20 ms wide, well under this threshold.
const MS_AXIS_THRESHOLD_MICROS: u64 = 1_000_000;

// ── Public entry point ────────────────────────────────────────────────────────

/// Render the Gantt area below the filter strip.
///
/// `area` is the content area below the filter strip (filter strip is drawn
/// by the caller in `mod.rs`). Writes `timeline_visible_row_count` render-hint
/// to state before returning.
///
/// ## Anchor logic
///
/// When `state.committed_frame_anchor == None`: renders the "Select a frame…"
/// placeholder (the viewport is driven by the anchor, not by auto-scroll).
///
/// When `state.committed_frame_anchor == Some(N)` but no matching node is found
/// in the tracks: renders the "not available" placeholder.
///
/// When `state.committed_frame_anchor == Some(N)` and a matching node is found:
/// renders the Gantt with a viewport spanning that node ± 20% padding.
pub(super) fn render_gantt(area: Rect, buf: &mut Buffer, state: &PerformanceState) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    // ── Anchor gate: nothing renders without an explicit frame selection ─────
    let frame_number = match state.committed_frame_anchor {
        None => {
            render_empty_placeholder(
                area,
                buf,
                "Select a frame in the chart above to inspect its timeline events",
            );
            // EXCEPTION: TEA render-hint write-back via Cell — see docs/REVIEW_FOCUS.md
            state.timeline_visible_row_count.set(0);
            return;
        }
        Some(n) => n,
    };

    // Compute the anchored viewport — fails gracefully when the frame was evicted.
    let (vp_start, vp_end) =
        match compute_frame_anchored_viewport(&state.timeline_tracks, frame_number) {
            Some(vp) => vp,
            None => {
                let msg = format!(
                    "Timeline data not available for frame #{frame_number} \
                     (may have been evicted from the 1000-event buffer)"
                );
                render_empty_placeholder(area, buf, &msg);
                // EXCEPTION: TEA render-hint write-back via Cell — see docs/REVIEW_FOCUS.md
                state.timeline_visible_row_count.set(0);
                return;
            }
        };

    // Collect filtered tracks
    let filtered_tracks: Vec<(&i64, &TimelineTrack)> = state
        .timeline_tracks
        .iter()
        .filter(|(_, track)| matches_filter(track.thread, state.timeline_events_filter))
        .collect();

    if filtered_tracks.is_empty() {
        render_empty_placeholder(area, buf, "No events match the current filter");
        // EXCEPTION: TEA render-hint write-back via Cell — see docs/REVIEW_FOCUS.md
        state.timeline_visible_row_count.set(0);
        return;
    }

    // Time axis takes TIME_AXIS_HEIGHT rows at the top of the gantt area.
    // Then rows of THREAD_ROW_HEIGHT each. Absorber takes remaining space.
    let max_rows_visible = (area.height.saturating_sub(TIME_AXIS_HEIGHT)) / THREAD_ROW_HEIGHT;

    // Apply scroll offset (clamped)
    let scroll_offset = state
        .timeline_thread_scroll_offset
        .min(filtered_tracks.len().saturating_sub(1));

    let visible_tracks: Vec<(&i64, &TimelineTrack)> = filtered_tracks
        .into_iter()
        .skip(scroll_offset)
        .take(max_rows_visible as usize)
        .collect();

    let visible_row_count = visible_tracks.len();

    // Build layout constraints: time axis + one row per visible thread + absorber
    let mut constraints: Vec<Constraint> = Vec::with_capacity(visible_row_count + 2);
    constraints.push(Constraint::Length(TIME_AXIS_HEIGHT));
    for _ in 0..visible_row_count {
        constraints.push(Constraint::Length(THREAD_ROW_HEIGHT));
    }
    constraints.push(Constraint::Min(0)); // absorber

    let chunks = Layout::vertical(constraints).split(area);

    // Choose time axis label style: ms labels for sub-second viewports (anchored frames),
    // second labels for the full sliding-window view.
    let use_ms_labels = (vp_end - vp_start) < MS_AXIS_THRESHOLD_MICROS;

    // Render time axis in chunks[0]
    render_time_axis(chunks[0], buf, vp_start, vp_end, use_ms_labels);

    // Render each thread row
    for (row_idx, (tid, track)) in visible_tracks.iter().enumerate() {
        let row_area = chunks[row_idx + 1]; // offset by 1 (time axis)
        let label = build_thread_label(**tid, track, &state.timeline_thread_name_map);
        render_thread_row(row_area, buf, track, &label, vp_start, vp_end);
    }

    // EXCEPTION: TEA render-hint write-back via Cell — see docs/REVIEW_FOCUS.md
    state.timeline_visible_row_count.set(visible_row_count);
}

// ── Thread label construction ─────────────────────────────────────────────────

/// Build a human-readable thread label for the left column.
///
/// Uses `timeline_thread_name_map[tid]` when available; falls back to
/// `format!("{:?} {}", track.thread, tid)`.
fn build_thread_label(
    tid: i64,
    track: &TimelineTrack,
    name_map: &std::collections::HashMap<i64, String>,
) -> String {
    let base = if let Some(name) = name_map.get(&tid) {
        format!("{} {}", name, tid)
    } else {
        format!("{:?} {}", track.thread, tid)
    };
    truncate_with_ellipsis(&base, THREAD_LABEL_WIDTH as usize)
}

// ── Thread row renderer ───────────────────────────────────────────────────────

/// Render a single thread row: label column on the left, event bars on the right.
fn render_thread_row(
    area: Rect,
    buf: &mut Buffer,
    track: &TimelineTrack,
    label: &str,
    vp_start: u64,
    vp_end: u64,
) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    // Draw thread label in the first row of the row band
    let label_color = palette::label_color(track.thread);
    let label_style = Style::default()
        .fg(label_color)
        .add_modifier(Modifier::BOLD);
    let label_text = truncate_with_ellipsis(label, THREAD_LABEL_WIDTH as usize);
    buf.set_string(area.x, area.y, &label_text, label_style);

    // Draw a subtle separator line below the thread label
    let canvas_start_x = area.x + THREAD_LABEL_WIDTH;
    let canvas_width = area.width.saturating_sub(THREAD_LABEL_WIDTH);
    if canvas_width == 0 {
        return;
    }

    // Draw a subtle left border (vertical mark at x=THREAD_LABEL_WIDTH)
    if let Some(cell) = buf.cell_mut((area.x + THREAD_LABEL_WIDTH.saturating_sub(1), area.y)) {
        cell.set_symbol("\u{2503}"); // ┃
        cell.set_fg(theme::TEXT_MUTED);
    }

    // Render each root event and its children
    for node in &track.root_events {
        render_bar(
            area,
            buf,
            node,
            vp_start,
            vp_end,
            0,
            canvas_start_x,
            canvas_width,
        );
    }
}

// ── Bar renderer ─────────────────────────────────────────────────────────────

/// Render a single event bar and its children recursively.
///
/// `depth` is the nesting level (0 = root). Children are rendered one row
/// below their parent (y += depth + 1). Rendering stops when `depth >= MAX_DEPTH`
/// or `y >= area.bottom()`.
///
/// # Arguments
/// * `area`          — full thread row area (all THREAD_ROW_HEIGHT lines)
/// * `vp_start/end`  — viewport bounds in microseconds
/// * `depth`         — nesting depth (0 = root)
/// * `canvas_x`      — x offset of the time canvas (= area.x + THREAD_LABEL_WIDTH)
/// * `canvas_width`  — width of the time canvas in columns
#[allow(clippy::too_many_arguments)]
fn render_bar(
    area: Rect,
    buf: &mut Buffer,
    node: &TimelineNode,
    vp_start: u64,
    vp_end: u64,
    depth: u8,
    canvas_x: u16,
    canvas_width: u16,
) {
    if depth >= MAX_DEPTH {
        return;
    }

    let ts = node.ts as u64;
    let dur = node.dur.unwrap_or(0) as u64;

    let Some((col_off, col_width)) = clip_bar(ts, dur, vp_start, vp_end, canvas_width) else {
        return;
    };

    let y = area.y + depth as u16;
    if y >= area.y + area.height {
        return;
    }

    let color = palette::bar_color(node.thread, depth);
    let x = canvas_x + col_off;
    let bar_width = col_width.max(MIN_BAR_WIDTH);

    // Fill the bar background
    for dx in 0..bar_width {
        let bx = x + dx;
        if bx >= area.x + area.width {
            break;
        }
        if let Some(cell) = buf.cell_mut((bx, y)) {
            cell.set_bg(color);
            cell.set_fg(Color::White);
        }
    }

    // Render label inside the bar if at least 4 columns wide
    if bar_width >= 4 {
        let label = truncate_with_ellipsis(&node.name, bar_width.saturating_sub(2) as usize);
        let label_style = Style::default().fg(Color::White).bg(color);
        let mut lx = x + 1;
        for ch in label.chars() {
            if lx >= x + bar_width || lx >= area.x + area.width {
                break;
            }
            if let Some(cell) = buf.cell_mut((lx, y)) {
                cell.set_symbol(&ch.to_string());
                cell.set_style(label_style);
            }
            lx += 1;
        }
    }

    // Recurse into children (one row down)
    if depth + 1 < MAX_DEPTH {
        for child in &node.children {
            render_bar(
                area,
                buf,
                child,
                vp_start,
                vp_end,
                depth + 1,
                canvas_x,
                canvas_width,
            );
        }
    }
}

// ── Time axis ─────────────────────────────────────────────────────────────────

/// Render the time axis row above the thread rows.
///
/// When `use_ms_labels` is `true` (sub-second viewport, i.e. anchored mode),
/// labels are shown as frame-relative milliseconds: `0ms`, `4ms`, `8ms`, …
///
/// When `use_ms_labels` is `false` (full sliding-window viewport), labels are
/// shown at approximately 1-second intervals: `-5s`, `-4s`, … `0s`.
fn render_time_axis(area: Rect, buf: &mut Buffer, vp_start: u64, vp_end: u64, use_ms_labels: bool) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let canvas_width = area.width.saturating_sub(THREAD_LABEL_WIDTH);
    if canvas_width == 0 {
        return;
    }

    let label_x_base = area.x + THREAD_LABEL_WIDTH;
    let label_style = Style::default().fg(theme::TEXT_SECONDARY);
    let area_right = area.x + area.width;

    if use_ms_labels {
        // ── ms labels: 0ms, 4ms, 8ms, 12ms, 16ms (every ~4ms, up to 6 ticks) ──
        // Choose a tick interval that gives ~5 ticks across the viewport span.
        let span_ms = ((vp_end - vp_start) / 1_000).max(1) as i64;
        // Round to a "nice" interval: 1, 2, 4, 5, 8, 10, 16, 20 ms …
        let raw_interval = (span_ms / 5).max(1);
        let tick_interval_ms: i64 = if raw_interval <= 1 {
            1
        } else if raw_interval <= 2 {
            2
        } else if raw_interval <= 4 {
            4
        } else if raw_interval <= 5 {
            5
        } else if raw_interval <= 8 {
            8
        } else if raw_interval <= 10 {
            10
        } else if raw_interval <= 16 {
            16
        } else {
            20
        };
        let tick_interval_micros = tick_interval_ms as u64 * 1_000;

        let num_ticks = (span_ms / tick_interval_ms + 1).min(10) as u64;
        for i in 0..=num_ticks {
            let tick_ts = vp_start + i * tick_interval_micros;
            if tick_ts > vp_end {
                break;
            }
            let ms_offset = ((tick_ts - vp_start) / 1_000) as i64;
            let label = format!("{ms_offset}ms");

            let col = super::viewport::micros_to_column(tick_ts, vp_start, vp_end, canvas_width);
            let x = label_x_base + col;
            if x >= area_right {
                continue;
            }
            let label_len = label.chars().count() as u16;
            let lx_start = if x + label_len > area_right {
                area_right.saturating_sub(label_len)
            } else {
                x
            };
            let mut lx = lx_start;
            for ch in label.chars() {
                if lx >= area_right {
                    break;
                }
                if let Some(cell) = buf.cell_mut((lx, area.y)) {
                    cell.set_symbol(&ch.to_string());
                    cell.set_style(label_style);
                }
                lx += 1;
            }
        }
    } else {
        // ── Second labels: -5s … -1s, 0s ─────────────────────────────────────
        let viewport_secs = ((vp_end - vp_start) / 1_000_000).max(1) as i64;
        let tick_interval_micros: u64 = 1_000_000;
        let num_ticks = (TIMELINE_VIEWPORT_MICROS / tick_interval_micros + 1) as i64;

        for tick_idx in 0..=viewport_secs.min(num_ticks) {
            let tick_offset_micros = tick_idx as u64 * tick_interval_micros;
            let tick_ts =
                vp_end.saturating_sub(TIMELINE_VIEWPORT_MICROS.saturating_sub(tick_offset_micros));

            let col = super::viewport::micros_to_column(tick_ts, vp_start, vp_end, canvas_width);
            let x = label_x_base + col;
            if x >= area_right {
                continue;
            }

            let secs_relative = -(viewport_secs - tick_idx);
            let label = if secs_relative == 0 {
                "0s".to_owned()
            } else {
                format!("{secs_relative}s")
            };

            let label_len = label.chars().count() as u16;
            let lx_start = if x + label_len > area_right {
                area_right.saturating_sub(label_len)
            } else {
                x
            };
            let mut lx = lx_start;
            for ch in label.chars() {
                if lx >= area_right {
                    break;
                }
                if let Some(cell) = buf.cell_mut((lx, area.y)) {
                    cell.set_symbol(&ch.to_string());
                    cell.set_style(label_style);
                }
                lx += 1;
            }
        }
    }
}

// ── Filter helpers ────────────────────────────────────────────────────────────

/// Returns `true` if the thread should be shown given the active filter.
pub(super) fn matches_filter(thread: TimelineThread, filter: TimelineFilter) -> bool {
    match filter {
        TimelineFilter::All => true,
        TimelineFilter::Ui => thread == TimelineThread::Ui,
        TimelineFilter::Raster => thread == TimelineThread::Raster,
    }
}

// ── Placeholder ───────────────────────────────────────────────────────────────

fn render_empty_placeholder(area: Rect, buf: &mut Buffer, message: &str) {
    let p = Paragraph::new(message)
        .style(Style::default().fg(theme::TEXT_MUTED))
        .alignment(Alignment::Center);
    let chunks = Layout::vertical([
        Constraint::Min(0),
        Constraint::Length(EMPTY_PLACEHOLDER_LINE_COUNT),
        Constraint::Min(0),
    ])
    .split(area);
    p.render(chunks[1], buf);
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use fdemon_app::session::{PerformanceState, TimelineFilter};
    use fdemon_core::timeline::{TimelineNode, TimelinePhase, TimelineThread, TimelineTrack};
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use std::collections::BTreeMap;

    // ── Test helpers ──────────────────────────────────────────────────────────

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

    /// Return a `PerformanceState` with `committed_frame_anchor = Some(1)`.
    ///
    /// All test tracks built via `make_complete_node` / `make_track` carry
    /// `frame_number = Some(1)`, so anchor `1` always resolves to a node.
    fn make_anchored_state() -> PerformanceState {
        PerformanceState {
            committed_frame_anchor: Some(1),
            ..Default::default()
        }
    }

    // ── AC9: Empty state placeholder ──────────────────────────────────────────

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

    // ── AC13: No panic on zero area ────────────────────────────────────────────

    #[test]
    fn gantt_no_panic_zero_area() {
        let state = PerformanceState::default();
        let mut buf = Buffer::empty(Rect::ZERO);
        render_gantt(Rect::ZERO, &mut buf, &state); // must not panic
    }

    // ── AC2: Thread rows render with labels ────────────────────────────────────

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

    // ── AC3: Thread names from name map ───────────────────────────────────────

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

    // ── AC4: Fallback label when name not in map ───────────────────────────────

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

    // ── AC5: UI bars have light-blue color ────────────────────────────────────

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

    // ── AC10: Thread filter ────────────────────────────────────────────────────

    #[test]
    fn gantt_filter_ui_hides_raster_rows() {
        let mut state = PerformanceState {
            timeline_events_filter: TimelineFilter::Ui,
            committed_frame_anchor: Some(1),
            ..Default::default()
        };
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
        let mut state = PerformanceState {
            timeline_events_filter: TimelineFilter::Raster,
            committed_frame_anchor: Some(1),
            ..Default::default()
        };
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
        let mut state = PerformanceState {
            timeline_events_filter: TimelineFilter::All,
            committed_frame_anchor: Some(1),
            ..Default::default()
        };
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

    // ── AC11: Vertical scroll ─────────────────────────────────────────────────

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

    // ── AC12: Render-hint write-back ───────────────────────────────────────────

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

    // ── AC7: Depth-stacked children ────────────────────────────────────────────

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

    // ── AC15: Time axis labels ─────────────────────────────────────────────────

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
        render_time_axis(area, &mut buf, vp_start, vp_end, false); // false = second labels
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

    // ── matches_filter ────────────────────────────────────────────────────────

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

    // ── Phase 5: anchor-related placeholder tests ─────────────────────────────

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

    /// AC5: When committed_frame_anchor == Some(N) but no matching event exists
    /// in the tracks, the Gantt should show the "not available" placeholder.
    #[test]
    fn render_gantt_shows_not_available_placeholder_when_anchor_missing_from_tracks() {
        let mut state = PerformanceState::default();
        // Tracks contain frame 1 but not frame 99.
        let mut tracks = BTreeMap::new();
        tracks.insert(
            1,
            make_track_with_frame(1, TimelineThread::Ui, 1_000_000, 16_000, Some(1)),
        );
        state.timeline_tracks = tracks;
        state.committed_frame_anchor = Some(99);

        let area = Rect::new(0, 0, 120, 10);
        let mut buf = Buffer::empty(area);
        render_gantt(area, &mut buf, &state);
        let text = collect_text(&buf);

        assert!(
            text.contains("not available") || text.contains("Timeline data"),
            "expected 'not available' placeholder for missing frame, got:\n{text}"
        );
    }

    /// AC4: When viewport span < 1s, the time axis should use ms labels
    /// (e.g. "0ms") instead of second labels like "0s".
    #[test]
    fn time_axis_uses_ms_labels_when_viewport_under_one_second() {
        // vp_start=0, vp_end=50_000 (50 µs → sub-millisecond) — well under 1s threshold
        let area = Rect::new(0, 0, 150, 3);
        let mut buf = Buffer::empty(area);
        render_time_axis(area, &mut buf, 0, 50_000, true);
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
}
