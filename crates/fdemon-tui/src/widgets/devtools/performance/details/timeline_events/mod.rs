//! Gantt-style Timeline Events tab widget.
//!
//! Replaces the Phase-3 flat-list `timeline_events_tab.rs` with a proper
//! Gantt renderer: per-thread rows with depth-stacked event bars, a time axis,
//! and a filter strip (`[All] [UI] [Raster]`).
//!
//! ## Module structure
//!
//! - [`mod.rs`] (this file) — public `render` entry, filter strip orchestration
//! - [`gantt`] — thread-row layout, bar rendering, depth stacking
//! - [`minimap`] — 1-row compressed history ribbon with viewport bracket overlay
//! - [`palette`] — color constants per `TimelineThread` and nesting depth
//! - [`viewport`] — pure math helpers: viewport bounds, column mapping, bar clipping

use fdemon_app::session::{PerformanceState, TimelineFilter};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
};

mod gantt;
mod minimap;
mod palette;
pub(super) mod popup;
pub(super) mod search;
mod viewport;

// Re-export text_helpers from parent module via pub(super) path
use super::text_helpers;

use crate::theme::palette as theme;

// ── Layout constants ──────────────────────────────────────────────────────────

/// Width of the thread-name label column on the left of each row.
/// Derived: 25 chars accommodates "io.flutter.raster 45067" (24 chars) with
/// 1 char margin.
pub(super) const THREAD_LABEL_WIDTH: u16 = 25;

/// Default viewport span — show the most recent N microseconds.
/// Equals 5 seconds. Phase 5 will make this configurable.
pub(super) const TIMELINE_VIEWPORT_MICROS: u64 = 5_000_000;

/// Maximum nesting depth rendered per thread row. Deeper children are
/// flattened to the deepest visible level.
pub(super) const MAX_DEPTH: u8 = 5;

/// Minimum bar width in columns. Bars narrower than this are drawn as 1-col
/// vertical marks, preventing flicker for sub-pixel events.
pub(super) const MIN_BAR_WIDTH: u16 = 1;

/// Height of the time axis row (in terminal lines).
pub(super) const TIME_AXIS_HEIGHT: u16 = 1;

/// Height of a single thread row's content area (in terminal lines).
/// 2 lines: 1 for the depth-stacked bars (children overlay onto the same
/// line as their root, since deep nesting is rare in practice) + 1 spacer
/// to separate adjacent thread rows visually. Keep tight so more threads
/// fit on screen.
pub(super) const THREAD_ROW_HEIGHT: u16 = 2;

/// Filter strip height (1 row).
const FILTER_STRIP_HEIGHT: u16 = 1;

// ── Public entry point ────────────────────────────────────────────────────────

/// Render the Timeline Events tab content area.
///
/// Layout (top to bottom):
/// 1. Search bar (1 row, Phase 5 T04, optional) — shown when a search query is active.
/// 2. Filter strip (1 row) — `[All] [UI] [Raster]` selector.
/// 3. Minimap ribbon (1 row, Phase 5 T02) — compressed history with viewport bracket.
/// 4. Gantt area (remaining rows) — per-thread event bars and time axis.
///
/// Signature matches the Phase-3 dispatch convention: `(area, buf, state)`.
pub(super) fn render(area: Rect, buf: &mut Buffer, state: &PerformanceState) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    // Determine whether the search bar should be shown and how many header rows
    // it occupies.
    let search_visible = search::search_bar_visible(state);
    let search_height = if search_visible {
        search::SEARCH_BAR_HEIGHT
    } else {
        0
    };

    // Total header rows: search bar (optional) + filter strip.
    let header_height = search_height + FILTER_STRIP_HEIGHT;

    if area.height <= header_height {
        // Not enough room for the header rows — render what fits.
        if search_visible && area.height >= 1 {
            let search_area = Rect { height: 1, ..area };
            search::render_search_bar(search_area, buf, state);
        }
        if area.height > search_height {
            let filter_area = Rect {
                y: area.y + search_height,
                height: area.height - search_height,
                ..area
            };
            render_filter_strip(filter_area, buf, state);
        }
        return;
    }

    // Resolve the active viewport once here so both the minimap and the Gantt
    // use the same (viewport_start, viewport_end) without duplicating logic.
    let (vp_start, vp_end) = viewport::compute_active_viewport(state);

    // Build layout with optional search bar, filter strip, optional minimap, gantt.
    let remaining_after_header = area.height - header_height;
    if remaining_after_header <= minimap::MINIMAP_HEIGHT {
        // Not enough room for the minimap — search bar + filter strip + Gantt.
        let mut constraints = Vec::new();
        if search_visible {
            constraints.push(Constraint::Length(search::SEARCH_BAR_HEIGHT));
        }
        constraints.push(Constraint::Length(FILTER_STRIP_HEIGHT));
        constraints.push(Constraint::Min(0)); // gantt area

        let chunks = Layout::vertical(constraints).split(area);
        let mut idx = 0;
        if search_visible {
            search::render_search_bar(chunks[idx], buf, state);
            idx += 1;
        }
        render_filter_strip(chunks[idx], buf, state);
        idx += 1;
        gantt::render_gantt(chunks[idx], buf, state);
    } else {
        // Full layout: search bar + filter strip + minimap + gantt.
        let mut constraints = Vec::new();
        if search_visible {
            constraints.push(Constraint::Length(search::SEARCH_BAR_HEIGHT));
        }
        constraints.push(Constraint::Length(FILTER_STRIP_HEIGHT));
        constraints.push(Constraint::Length(minimap::MINIMAP_HEIGHT)); // Phase 5 T02
        constraints.push(Constraint::Min(0)); // gantt area

        let chunks = Layout::vertical(constraints).split(area);
        let mut idx = 0;
        if search_visible {
            search::render_search_bar(chunks[idx], buf, state);
            idx += 1;
        }
        render_filter_strip(chunks[idx], buf, state);
        idx += 1;

        // Minimap: pass resolved viewport bounds; minimap itself is stateless.
        minimap::render(
            chunks[idx],
            buf,
            &state.timeline_tracks,
            vp_start,
            vp_end,
            state.timeline_events_filter,
        );
        idx += 1;

        gantt::render_gantt(chunks[idx], buf, state);
    }

    // Render the event details popup last (on top of everything).
    // When `timeline_details_popup_open == true`, this overlays the entire
    // tab area (dims background, draws centered popup).
    if state.timeline_details_popup_open {
        popup::render(area, buf, state);
    }
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

    let mut spans: Vec<Span> = Vec::new();
    for (i, (filter, label)) in filters.iter().enumerate() {
        let is_active = state.timeline_events_filter == *filter;
        let style = if is_active {
            Style::default()
                .fg(theme::ACCENT)
                .add_modifier(Modifier::BOLD | Modifier::REVERSED)
        } else {
            Style::default().fg(theme::TEXT_MUTED)
        };
        spans.push(Span::styled(*label, style));
        if i + 1 < filters.len() {
            spans.push(Span::raw(" "));
        }
    }

    // Append track count info on the right
    let track_count = state.timeline_tracks.len();
    let count_str = format!("  Threads: {}", track_count);
    spans.push(Span::styled(
        count_str,
        Style::default().fg(theme::TEXT_SECONDARY),
    ));

    let line = Line::from(spans);
    buf.set_line(area.x, area.y, &line, area.width);
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use fdemon_app::session::PerformanceState;
    use fdemon_core::timeline::{TimelineNode, TimelineThread, TimelineTrack};
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use ratatui::style::Modifier;
    use std::collections::BTreeMap;

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

    // ── AC14: Filter strip preserved ─────────────────────────────────────────

    #[test]
    fn timeline_events_filter_strip_shows_all_chips() {
        let state = PerformanceState::default();
        let area = Rect::new(0, 0, 80, 10);
        let mut buf = Buffer::empty(area);
        render(area, &mut buf, &state);
        let text = collect_text(&buf);
        assert!(text.contains("[All]"), "expected [All] chip, got:\n{text}");
        assert!(text.contains("[UI]"), "expected [UI] chip, got:\n{text}");
        assert!(
            text.contains("[Raster]"),
            "expected [Raster] chip, got:\n{text}"
        );
    }

    #[test]
    fn timeline_events_filter_strip_highlights_active_chip() {
        let state = PerformanceState {
            timeline_events_filter: TimelineFilter::Raster,
            ..Default::default()
        };
        let area = Rect::new(0, 0, 80, 10);
        let mut buf = Buffer::empty(area);
        render(area, &mut buf, &state);

        // [Raster] chip starts at column: "[All] [UI] " = 11 chars
        let raster_x = 11u16;
        if let Some(cell) = buf.cell((raster_x, 0)) {
            assert!(
                cell.style().add_modifier.contains(Modifier::REVERSED),
                "[Raster] chip should have REVERSED modifier when active"
            );
        }
    }

    // ── AC9: Empty state via render entry ────────────────────────────────────

    /// When committed_frame_anchor == None (default), the "Select a frame"
    /// prompt is shown via the Gantt's anchor gate.
    #[test]
    fn timeline_events_renders_empty_state() {
        let state = PerformanceState::default();
        let area = Rect::new(0, 0, 80, 10);
        let mut buf = Buffer::empty(area);
        render(area, &mut buf, &state);
        let text = collect_text(&buf);
        assert!(
            text.contains("Select a frame"),
            "expected 'Select a frame' placeholder when no anchor, got:\n{text}"
        );
    }

    // ── AC13: No panic on zero area ───────────────────────────────────────────

    #[test]
    fn timeline_events_no_panic_zero_area() {
        let state = PerformanceState::default();
        let mut buf = Buffer::empty(Rect::ZERO);
        render(Rect::ZERO, &mut buf, &state); // must not panic
    }

    // ── AC12: Render-hint via render entry ────────────────────────────────────

    #[test]
    fn timeline_events_render_hint_updated() {
        let ts = 1_000_000i64;
        let dur = 500_000i64;
        let mut state = PerformanceState {
            // Set anchor = 1 so the Gantt renders (not placeholder)
            committed_frame_anchor: Some(1),
            ..Default::default()
        };
        // Populate frame_anchor_map so the viewport resolves for frame 1.
        state
            .frame_anchor_map
            .insert(1u64, (ts as u64, (ts + dur) as u64));
        let mut tracks = BTreeMap::new();
        tracks.insert(
            1,
            TimelineTrack {
                tid: 1,
                name: None,
                thread: TimelineThread::Ui,
                // frame_number=1 matches committed_frame_anchor=1
                root_events: vec![TimelineNode {
                    name: "Frame".to_owned(),
                    category: None,
                    ts,
                    dur: Some(dur),
                    phase: fdemon_core::timeline::TimelinePhase::Complete,
                    thread: TimelineThread::Ui,
                    frame_number: Some(1),
                    children: vec![],
                }],
            },
        );
        state.timeline_tracks = tracks;

        let area = Rect::new(0, 0, 80, 20);
        let mut buf = Buffer::empty(area);
        render(area, &mut buf, &state);

        // Render hint should be updated (non-zero for one track with sufficient height)
        assert_eq!(
            state.timeline_visible_row_count.get(),
            1,
            "render hint should be 1 for one visible thread"
        );
    }
}
