//! Gantt-style renderer for the Timeline Events tab.
//!
//! Renders per-thread rows with depth-stacked event bars and a time axis
//! strip above the rows. Thread labels appear in a fixed-width left column.
//!
//! # Test organization
//!
//! Unit tests live in the sibling `gantt_tests.rs` module (declared below),
//! keeping this file under the 800-line ceiling so Phase 5 overlay additions
//! can land in T03/T04.

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
    viewport::{clip_bar, compute_active_viewport},
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
/// ## Viewport resolution (Phase 5, PLAN D2)
///
/// `compute_active_viewport` resolves the viewport through three modes in
/// priority order:
///   1. `!follow_latest` → manual viewport `(start, start + width)` (pan/zoom mode)
///   2. `follow_latest && committed_frame_anchor.is_some()` → frame-anchored
///   3. `follow_latest && no frame anchor` → live-edge auto-scroll
///
/// When follow_latest is true and `committed_frame_anchor == None`, a
/// "Select a frame" placeholder is shown instead of an empty Gantt.
///
/// When `committed_frame_anchor == Some(N)` but the frame_anchor_map has no
/// entry for N and follow_latest is true: shows a "not available" placeholder.
///
/// When `!follow_latest`, the "PAUSED" indicator is rendered to signal manual
/// viewport mode; press `g` or `End` to resume follow-latest.
pub(super) fn render_gantt(area: Rect, buf: &mut Buffer, state: &PerformanceState) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    // ── Anchor gate: in follow_latest mode, require an explicit frame anchor ──
    if state.timeline_follow_latest {
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

        // Validate that the frame anchor is resolvable (in follow_latest mode,
        // we need the frame_anchor_map to be populated for this frame).
        if !state.frame_anchor_map.contains_key(&frame_number) {
            let msg = format!(
                "No timeline data recorded for frame #{frame_number} \
                 (the frame may pre-date the Performance panel opening, or its \
                 anchor events lacked args.frame_number — try a more recent frame)"
            );
            render_empty_placeholder(area, buf, &msg);
            // EXCEPTION: TEA render-hint write-back via Cell — see docs/REVIEW_FOCUS.md
            state.timeline_visible_row_count.set(0);
            return;
        }
    }

    // ── Resolve viewport (3-mode priority from PLAN D2) ──────────────────────
    let (vp_start, vp_end) = compute_active_viewport(state);

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

    // ── PAUSED indicator: shown when in manual-viewport mode (follow_latest=false) ──
    // Rendered in the time-axis row at the right edge so it doesn't overlap bars.
    if !state.timeline_follow_latest {
        render_paused_indicator(chunks[0], buf);
    }

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

// ── PAUSED indicator ──────────────────────────────────────────────────────────

/// Render the "PAUSED" indicator at the right edge of the time-axis row.
///
/// Shown when `state.timeline_follow_latest == false` (manual-viewport mode).
/// The indicator reads `⏸ PAUSED (g=resume)` and is rendered right-aligned
/// so it does not overlap the time-axis tick labels on the left.
fn render_paused_indicator(area: Rect, buf: &mut Buffer) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let label = "\u{23f8} PAUSED (g=resume)"; // ⏸ PAUSED (g=resume)
    let style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::DIM);
    let label_len = label.chars().count() as u16;
    if area.width < label_len {
        return;
    }
    let x_start = area.x + area.width - label_len;
    let mut x = x_start;
    for ch in label.chars() {
        if x >= area.x + area.width {
            break;
        }
        if let Some(cell) = buf.cell_mut((x, area.y)) {
            cell.set_symbol(&ch.to_string());
            cell.set_style(style);
        }
        x += 1;
    }
}

// ── Test helper shim (accessible from gantt_tests.rs) ────────────────────────

/// Public-in-super wrapper around the private `render_time_axis` for use by
/// the sibling `gantt_tests.rs` module. Only compiled in `#[cfg(test)]`.
#[cfg(test)]
pub(super) fn render_time_axis_pub(
    area: Rect,
    buf: &mut Buffer,
    vp_start: u64,
    vp_end: u64,
    use_ms_labels: bool,
) {
    render_time_axis(area, buf, vp_start, vp_end, use_ms_labels);
}

// TODO (Phase 5, AC12 stretch): Mouse scroll wheel zoom.
//   Mouse scroll up on the Gantt canvas area → TimelineZoomIn { session_id }.
//   Mouse scroll down → TimelineZoomOut { session_id }.
//   Requires `Mouse(MouseInput)` handler in `handler/mouse.rs` to detect the
//   canvas area and dispatch the correct session-scoped message.

// ── Tests (moved to gantt_tests.rs — see Drift #7, Phase 5 Task 01) ─────────

#[cfg(test)]
#[path = "gantt_tests.rs"]
mod tests;
