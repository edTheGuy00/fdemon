//! Timeline Events tab — Phase 3.
//!
//! Renders a filter strip (`[All] [UI] [Raster]`) and a scrollable list of
//! timeline events from the Dart VM, filtered by thread classification.

use fdemon_app::session::{PerformanceState, TimelineFilter};
use fdemon_core::timeline::{TimelineEvent, TimelineThread};
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget, Wrap},
};

use crate::theme::palette;

// ── Thread badge colours ──────────────────────────────────────────────────────

/// UI thread badge colour — matches Flutter DevTools convention.
const COLOR_UI: Color = Color::Cyan;
/// Raster thread badge colour — matches Flutter DevTools convention.
const COLOR_RASTER: Color = Color::Magenta;
/// Other/unknown thread badge colour.
const COLOR_OTHER: Color = Color::DarkGray;

// ── Layout constants ──────────────────────────────────────────────────────────

/// Filter strip height (1 row).
const FILTER_STRIP_HEIGHT: u16 = 1;
/// Column header height (1 row).
const COL_HEADER_HEIGHT: u16 = 1;

/// Width of the thread badge column (e.g. `"  UI  "` or `"Raster"`).
const BADGE_COL_WIDTH: u16 = 6;
/// Width of the duration column (e.g. `"16.2 ms"`).
const DURATION_COL_WIDTH: u16 = 10;
/// Width of the ts-relative column (e.g. `"  -150ms"`).
const TS_REL_COL_WIDTH: u16 = 10;
/// Separator between columns.
const COL_SEP: u16 = 1;

// ── Public entry point ────────────────────────────────────────────────────────

/// Render the Timeline Events tab content area.
///
/// Signature matches the Phase-3 dispatch convention: `(area, buf, state)`.
pub(super) fn render(area: Rect, buf: &mut Buffer, state: &PerformanceState) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let overhead = FILTER_STRIP_HEIGHT + COL_HEADER_HEIGHT;
    if area.height <= overhead {
        render_filter_strip(area, buf, state);
        return;
    }

    let chunks = Layout::vertical([
        Constraint::Length(FILTER_STRIP_HEIGHT),
        Constraint::Length(COL_HEADER_HEIGHT),
        Constraint::Min(0), // event list
    ])
    .split(area);

    let filter_area = chunks[0];
    let col_header_area = chunks[1];
    let list_area = chunks[2];

    render_filter_strip(filter_area, buf, state);
    render_col_headers(col_header_area, buf, list_area.width);
    render_event_list(list_area, buf, state);
}

// ── Filter strip ──────────────────────────────────────────────────────────────

fn render_filter_strip(area: Rect, buf: &mut Buffer, state: &PerformanceState) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let filters = [
        (TimelineFilter::All, "[All]"),
        (TimelineFilter::Ui, "[UI]"),
        (TimelineFilter::Raster, "[Raster]"),
    ];

    // Build filter chip spans.
    let mut spans: Vec<Span> = Vec::new();
    for (i, (filter, label)) in filters.iter().enumerate() {
        let is_active = state.timeline_events_filter == *filter;
        let style = if is_active {
            Style::default()
                .fg(palette::ACCENT)
                .add_modifier(Modifier::BOLD | Modifier::REVERSED)
        } else {
            Style::default().fg(palette::TEXT_MUTED)
        };
        spans.push(Span::styled(*label, style));
        if i + 1 < filters.len() {
            spans.push(Span::raw(" "));
        }
    }

    // Append event count to the right side of the filter strip.
    let visible_count = count_visible_events(state);
    let total_count = state.timeline_events.len();
    let count_str = format!("  Events: {}/{}", visible_count, total_count);
    spans.push(Span::styled(
        count_str,
        Style::default().fg(palette::TEXT_SECONDARY),
    ));

    let line = Line::from(spans);
    buf.set_line(area.x, area.y, &line, area.width);
}

// ── Column headers ────────────────────────────────────────────────────────────

fn render_col_headers(area: Rect, buf: &mut Buffer, list_width: u16) {
    if area.height == 0 || list_width == 0 {
        return;
    }

    let event_col_width = event_name_col_width(list_width);
    let col_header_line = Line::from(vec![
        Span::styled(
            pad_right("Thread", BADGE_COL_WIDTH as usize),
            Style::default()
                .fg(palette::TEXT_SECONDARY)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            pad_right("Event", event_col_width as usize),
            Style::default()
                .fg(palette::TEXT_SECONDARY)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            pad_left("Duration", DURATION_COL_WIDTH as usize),
            Style::default()
                .fg(palette::TEXT_SECONDARY)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            pad_left("ts (rel)", TS_REL_COL_WIDTH as usize),
            Style::default()
                .fg(palette::TEXT_SECONDARY)
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    buf.set_line(area.x, area.y, &col_header_line, area.width);
}

// ── Event list ────────────────────────────────────────────────────────────────

fn render_event_list(area: Rect, buf: &mut Buffer, state: &PerformanceState) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    // EXCEPTION: TEA render-hint write-back via Cell — see docs/REVIEW_FOCUS.md
    state.details_pane_visible_height.set(area.height as usize);

    // Build filtered slice (newest-first display).
    let visible: Vec<&TimelineEvent> = state
        .timeline_events
        .iter()
        .filter(|e| matches_filter(e.thread, state.timeline_events_filter))
        .collect();

    if visible.is_empty() {
        render_empty_placeholder(area, buf, "Waiting for timeline events…");
        return;
    }

    // Newest event has the highest ts — use it as the reference for relative time.
    let newest_ts = visible.iter().map(|e| e.ts).max().unwrap_or(0);

    // Display newest-first: reverse iterator.
    let display_order: Vec<&&TimelineEvent> = visible.iter().rev().collect();

    // Apply scroll offset with clamping.
    let scroll_offset = state
        .timeline_events_scroll_offset
        .min(display_order.len().saturating_sub(1));

    let visible_slice = &display_order[scroll_offset..];
    let event_col_width = event_name_col_width(area.width);

    for (row_idx, event) in visible_slice.iter().enumerate() {
        let y = area.y + row_idx as u16;
        if y >= area.y + area.height {
            break;
        }

        let (badge_text, badge_color) = thread_badge(event.thread);
        let name_truncated = truncate_with_ellipsis(&event.name, event_col_width as usize);

        let duration_str = match event.dur {
            Some(dur_us) => format!("{:.1} ms", dur_us as f64 / 1000.0),
            None => "—".to_owned(),
        };

        // ts relative to the newest event (newest = "0ms", older = negative).
        let ts_diff_us = newest_ts.saturating_sub(event.ts);
        let ts_rel_str = if ts_diff_us == 0 {
            "0ms".to_owned()
        } else {
            format!("-{}ms", (ts_diff_us / 1000).max(1))
        };

        let row_line = Line::from(vec![
            Span::styled(
                format!("{:>width$}", badge_text, width = BADGE_COL_WIDTH as usize),
                Style::default().fg(badge_color),
            ),
            Span::raw(" "),
            Span::styled(
                pad_right(&name_truncated, event_col_width as usize),
                Style::default().fg(palette::TEXT_PRIMARY),
            ),
            Span::raw(" "),
            Span::styled(
                pad_left(&duration_str, DURATION_COL_WIDTH as usize),
                Style::default().fg(palette::TEXT_SECONDARY),
            ),
            Span::raw(" "),
            Span::styled(
                pad_left(&ts_rel_str, TS_REL_COL_WIDTH as usize),
                Style::default().fg(palette::TEXT_MUTED),
            ),
        ]);

        buf.set_line(area.x, y, &row_line, area.width);
    }
}

// ── Filter helpers ────────────────────────────────────────────────────────────

fn matches_filter(thread: TimelineThread, filter: TimelineFilter) -> bool {
    match filter {
        TimelineFilter::All => true,
        TimelineFilter::Ui => thread == TimelineThread::Ui,
        TimelineFilter::Raster => thread == TimelineThread::Raster,
    }
}

fn count_visible_events(state: &PerformanceState) -> usize {
    state
        .timeline_events
        .iter()
        .filter(|e| matches_filter(e.thread, state.timeline_events_filter))
        .count()
}

// ── Badge helper ──────────────────────────────────────────────────────────────

fn thread_badge(thread: TimelineThread) -> (&'static str, Color) {
    match thread {
        TimelineThread::Ui => ("UI", COLOR_UI),
        TimelineThread::Raster => ("Raster", COLOR_RASTER),
        TimelineThread::Other => ("Other", COLOR_OTHER),
    }
}

// ── Column-width helper ───────────────────────────────────────────────────────

/// Compute the event-name column width given the total available list width.
///
/// Fixed columns: badge (6) + sep (1) + duration (10) + sep (1) + ts_rel (10) = 28.
/// Event name gets the remainder (minimum 0).
fn event_name_col_width(total_width: u16) -> u16 {
    let fixed = BADGE_COL_WIDTH + COL_SEP + DURATION_COL_WIDTH + COL_SEP + TS_REL_COL_WIDTH;
    // Also account for the separating space after badge.
    let overhead = fixed + COL_SEP; // badge, sep, name, sep, dur, sep, ts_rel
    total_width.saturating_sub(overhead)
}

// ── Placeholder helper ────────────────────────────────────────────────────────

fn render_empty_placeholder(area: Rect, buf: &mut Buffer, message: &str) {
    let p = Paragraph::new(message)
        .style(Style::default().fg(palette::TEXT_MUTED))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });
    let y_offset = area.height.saturating_sub(2) / 2;
    let centered = Rect {
        y: area.y + y_offset,
        height: area.height.saturating_sub(y_offset),
        ..area
    };
    p.render(centered, buf);
}

// ── String formatting helpers ─────────────────────────────────────────────────

fn truncate_with_ellipsis(s: &str, max_chars: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_chars {
        s.to_owned()
    } else {
        let truncated: String = chars[..max_chars.saturating_sub(1)].iter().collect();
        format!("{truncated}…")
    }
}

fn pad_right(s: &str, width: usize) -> String {
    let len = s.chars().count();
    if len >= width {
        s.to_owned()
    } else {
        format!("{}{}", s, " ".repeat(width - len))
    }
}

fn pad_left(s: &str, width: usize) -> String {
    let len = s.chars().count();
    if len >= width {
        s.to_owned()
    } else {
        format!("{}{}", " ".repeat(width - len), s)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use fdemon_app::session::PerformanceState;
    use fdemon_core::timeline::{TimelineEvent, TimelinePhase, TimelineThread};
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

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

    fn make_event(name: &str, thread: TimelineThread, ts: u64, dur: Option<u64>) -> TimelineEvent {
        TimelineEvent {
            name: name.to_owned(),
            category: "Embedder".to_owned(),
            thread,
            tid: 1,
            phase: TimelinePhase::Complete,
            ts,
            dur,
            frame_number: None,
        }
    }

    // ── Empty state ───────────────────────────────────────────────────────────

    #[test]
    fn timeline_events_tab_renders_empty_state() {
        let state = PerformanceState::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 10));
        render(buf.area, &mut buf, &state);
        let text = collect_text(&buf);
        assert!(
            text.contains("Waiting for timeline events"),
            "expected empty-state placeholder, got:\n{text}"
        );
    }

    // ── Filter by UI thread ───────────────────────────────────────────────────

    #[test]
    fn timeline_events_tab_filters_by_ui_thread() {
        let mut state = PerformanceState {
            timeline_events_filter: TimelineFilter::Ui,
            ..Default::default()
        };
        state.timeline_events.push_back(make_event(
            "UIEvent",
            TimelineThread::Ui,
            1000,
            Some(5000),
        ));
        state.timeline_events.push_back(make_event(
            "UIEvent2",
            TimelineThread::Ui,
            2000,
            Some(3000),
        ));
        state.timeline_events.push_back(make_event(
            "RasterEvent",
            TimelineThread::Raster,
            3000,
            Some(8000),
        ));
        state.timeline_events.push_back(make_event(
            "RasterEvent2",
            TimelineThread::Raster,
            4000,
            Some(2000),
        ));

        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 15));
        render(buf.area, &mut buf, &state);
        let text = collect_text(&buf);

        assert!(
            text.contains("UIEvent"),
            "expected UI event in output, got:\n{text}"
        );
        assert!(
            !text.contains("RasterEvent"),
            "expected no Raster event in output when UI filter active, got:\n{text}"
        );
    }

    // ── Filter by Raster thread ───────────────────────────────────────────────

    #[test]
    fn timeline_events_tab_filters_by_raster_thread() {
        let mut state = PerformanceState {
            timeline_events_filter: TimelineFilter::Raster,
            ..Default::default()
        };
        state.timeline_events.push_back(make_event(
            "UIFrame",
            TimelineThread::Ui,
            1000,
            Some(5000),
        ));
        state.timeline_events.push_back(make_event(
            "GPURasterizer",
            TimelineThread::Raster,
            2000,
            Some(8000),
        ));

        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 15));
        render(buf.area, &mut buf, &state);
        let text = collect_text(&buf);

        assert!(
            text.contains("GPURasterizer"),
            "expected Raster event in output, got:\n{text}"
        );
        assert!(
            !text.contains("UIFrame"),
            "expected no UI event when Raster filter active, got:\n{text}"
        );
    }

    // ── Filter strip active chip ──────────────────────────────────────────────

    #[test]
    fn timeline_events_tab_filter_strip_highlights_active() {
        let state = PerformanceState {
            timeline_events_filter: TimelineFilter::Raster,
            ..Default::default()
        };
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 10));
        render(buf.area, &mut buf, &state);
        let text = collect_text(&buf);

        // The filter strip is on row 0; all three chips should appear.
        assert!(text.contains("[All]"), "expected [All] chip, got:\n{text}");
        assert!(text.contains("[UI]"), "expected [UI] chip, got:\n{text}");
        assert!(
            text.contains("[Raster]"),
            "expected [Raster] chip, got:\n{text}"
        );

        // Active chip ([Raster]) should have REVERSED modifier on its first char.
        // The [Raster] chip starts at column 6 (after "[All] [UI] ").
        let raster_x = 11u16; // "[All] [UI] " = 11 chars
        if let Some(cell) = buf.cell((raster_x, 0)) {
            assert!(
                cell.style().add_modifier.contains(Modifier::REVERSED),
                "[Raster] chip should have REVERSED modifier on active filter"
            );
        }
    }

    // ── Render-hint write-back ────────────────────────────────────────────────

    #[test]
    fn timeline_events_tab_writes_render_hint_height() {
        let mut state = PerformanceState::default();
        state
            .timeline_events
            .push_back(make_event("Frame", TimelineThread::Ui, 1000, Some(5000)));

        let area = Rect::new(0, 0, 80, 12);
        let mut buf = Buffer::empty(area);
        render(area, &mut buf, &state);

        // List area height = total - filter strip (1) - col header (1) = 10.
        let expected = 10usize;
        assert_eq!(
            state.details_pane_visible_height.get(),
            expected,
            "render-hint height should equal list area height"
        );
    }

    // ── Zero-area guard ───────────────────────────────────────────────────────

    #[test]
    fn timeline_events_tab_no_panic_zero_area() {
        let state = PerformanceState::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        render(buf.area, &mut buf, &state); // must not panic
    }

    // ── All filter shows all events ───────────────────────────────────────────

    #[test]
    fn timeline_events_tab_all_filter_shows_every_thread() {
        let mut state = PerformanceState {
            timeline_events_filter: TimelineFilter::All,
            ..Default::default()
        };
        state
            .timeline_events
            .push_back(make_event("UIWork", TimelineThread::Ui, 1000, Some(1000)));
        state.timeline_events.push_back(make_event(
            "RasterWork",
            TimelineThread::Raster,
            2000,
            Some(2000),
        ));
        state.timeline_events.push_back(make_event(
            "OtherWork",
            TimelineThread::Other,
            3000,
            Some(500),
        ));

        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 15));
        render(buf.area, &mut buf, &state);
        let text = collect_text(&buf);

        assert!(text.contains("UIWork"), "expected UI event: {text}");
        assert!(text.contains("RasterWork"), "expected Raster event: {text}");
        assert!(text.contains("OtherWork"), "expected Other event: {text}");
    }
}
