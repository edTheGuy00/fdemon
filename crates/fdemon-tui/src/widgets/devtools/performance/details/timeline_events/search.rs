//! Search bar widget for the Timeline Events tab.
//!
//! Renders a single-row search bar above the filter strip when the search input
//! is active OR a committed query is present.
//!
//! ## Layout
//!
//! Input-active mode (`timeline_search_input_active == true`):
//! ```text
//! / Raster▏                                             12 matches • n/N
//! ```
//!
//! Committed mode (`search_input_active == false`, `search_query.is_some()`):
//! ```text
//! / "Raster" • 12 matches • n/N for next/prev • Esc to clear
//! ```
//!
//! ## Match counting
//!
//! Match count is recomputed on every render pass from the current
//! `timeline_tracks` state. This is O(events × query.len) per render but
//! fast enough for typical event counts (≤ 10 000 events). The alternative
//! (cache in state) would require manual invalidation on every batch receive,
//! complicating the TEA update handler for marginal gain.

use std::collections::BTreeMap;

use fdemon_app::session::{PerformanceState, TimelineFilter};
use fdemon_core::timeline::{TimelineNode, TimelineTrack};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use crate::theme::palette as theme;

/// Height of the search bar row (always 1 line).
pub(super) const SEARCH_BAR_HEIGHT: u16 = 1;

/// Render the search bar when a search query is present (active or committed).
///
/// Returns immediately (no-op) when `search_query` is `None` — the caller
/// should check `search_bar_visible` before allocating a row in the layout.
///
/// Match count is computed internally from `state.timeline_tracks` on every
/// render call. Cost is O(events × query.len); acceptable at TUI render rates
/// with typical event counts (≤ 10 000 events × 20-char query ≈ 200 k char
/// comparisons).
pub(super) fn render_search_bar(area: Rect, buf: &mut Buffer, state: &PerformanceState) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let query_str = match &state.timeline_search_query {
        Some(q) => q.as_str(),
        None => return, // nothing to render
    };

    // Compute match count from current tracks.
    let match_count = if query_str.is_empty() {
        0
    } else {
        count_matches_in_tracks(
            &state.timeline_tracks,
            query_str,
            state.timeline_events_filter,
        )
    };

    let line = if state.timeline_search_input_active {
        build_input_line(query_str, match_count, area.width)
    } else {
        build_committed_line(query_str, match_count, area.width)
    };

    buf.set_line(area.x, area.y, &line, area.width);
}

/// Returns `true` when the search bar should be rendered (i.e., the caller
/// should allocate a row for it in the layout).
pub(super) fn search_bar_visible(state: &PerformanceState) -> bool {
    state.timeline_search_query.is_some()
}

// ── Match counting (TUI-local, mirrors handler's collect_matches) ─────────────

/// Count matching events in the timeline tracks (case-insensitive substring).
///
/// Returns 0 for an empty query. Respects the current `filter`.
fn count_matches_in_tracks(
    tracks: &BTreeMap<i64, TimelineTrack>,
    query: &str,
    filter: TimelineFilter,
) -> usize {
    if query.is_empty() {
        return 0;
    }
    let query_lower = query.to_lowercase();
    let mut count = 0usize;
    for track in tracks.values() {
        if !filter_matches(track.thread, filter) {
            continue;
        }
        count_in_nodes(&track.root_events, &query_lower, &mut count);
    }
    count
}

fn count_in_nodes(nodes: &[TimelineNode], query_lower: &str, count: &mut usize) {
    for node in nodes {
        if node.name.to_lowercase().contains(query_lower) {
            *count += 1;
        }
        count_in_nodes(&node.children, query_lower, count);
    }
}

fn filter_matches(thread: fdemon_core::timeline::TimelineThread, filter: TimelineFilter) -> bool {
    match filter {
        TimelineFilter::All => true,
        TimelineFilter::Ui => thread == fdemon_core::timeline::TimelineThread::Ui,
        TimelineFilter::Raster => thread == fdemon_core::timeline::TimelineThread::Raster,
    }
}

// ── Line builders ─────────────────────────────────────────────────────────────

/// Build the search bar line for active input mode.
///
/// Format: `/ <query>▏   <count> matches • n/N`
fn build_input_line(query: &str, match_count: usize, _width: u16) -> Line<'static> {
    let prompt_style = Style::default()
        .fg(theme::ACCENT)
        .add_modifier(Modifier::BOLD);
    let query_style = Style::default().fg(Color::White);
    let cursor_style = Style::default()
        .fg(Color::Black)
        .bg(Color::White)
        .add_modifier(Modifier::BOLD);
    let hint_style = Style::default().fg(theme::TEXT_SECONDARY);

    let match_hint = if match_count == 0 {
        "  no matches".to_string()
    } else if match_count == 1 {
        "  1 match • n/N".to_string()
    } else {
        format!("  {match_count} matches • n/N")
    };

    Line::from(vec![
        Span::styled("/ ", prompt_style),
        Span::styled(query.to_string(), query_style),
        Span::styled("\u{258f}", cursor_style), // ▏ block cursor
        Span::styled(match_hint, hint_style),
    ])
}

/// Build the search bar line for committed (non-input) mode.
///
/// Format: `/ "<query>" • <count> matches • n/N for next/prev • Esc to clear`
fn build_committed_line(query: &str, match_count: usize, _width: u16) -> Line<'static> {
    let prompt_style = Style::default()
        .fg(theme::ACCENT)
        .add_modifier(Modifier::BOLD);
    let query_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let hint_style = Style::default().fg(theme::TEXT_SECONDARY);

    let count_str = if match_count == 0 {
        " • no matches".to_string()
    } else if match_count == 1 {
        " • 1 match".to_string()
    } else {
        format!(" • {match_count} matches")
    };

    let nav_hint = if match_count > 0 {
        " • n/N for next/prev • Esc to clear".to_string()
    } else {
        " • Esc to clear".to_string()
    };

    Line::from(vec![
        Span::styled("/ ", prompt_style),
        Span::styled(format!("\"{query}\""), query_style),
        Span::styled(count_str, hint_style),
        Span::styled(nav_hint, hint_style),
    ])
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use fdemon_app::session::PerformanceState;
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

    // ── AC11: Search bar visibility ───────────────────────────────────────────

    #[test]
    fn search_bar_visible_false_when_no_query() {
        let state = PerformanceState::default();
        assert!(!search_bar_visible(&state));
    }

    #[test]
    fn search_bar_visible_true_when_query_some() {
        let state = PerformanceState {
            timeline_search_query: Some("Raster".to_string()),
            ..Default::default()
        };
        assert!(search_bar_visible(&state));
    }

    #[test]
    fn search_bar_visible_true_when_input_active_empty_query() {
        let state = PerformanceState {
            timeline_search_query: Some(String::new()),
            timeline_search_input_active: true,
            ..Default::default()
        };
        assert!(search_bar_visible(&state));
    }

    // ── AC11: Search bar render in input mode ─────────────────────────────────

    #[test]
    fn search_bar_renders_slash_prefix_when_input_active() {
        let state = PerformanceState {
            timeline_search_query: Some("Raster".to_string()),
            timeline_search_input_active: true,
            ..Default::default()
        };
        let area = Rect::new(0, 0, 80, 1);
        let mut buf = Buffer::empty(area);
        render_search_bar(area, &mut buf, &state);
        let text = collect_text(&buf);
        assert!(text.contains('/'), "search bar should contain '/' prefix");
        assert!(text.contains("Raster"), "search bar should show query text");
        // No tracks means 0 matches (correct — tracks are empty in this test)
        assert!(
            text.contains("no matches"),
            "search bar with empty tracks should show 'no matches', got: {text:?}"
        );
    }

    // ── AC11: Search bar render in committed mode ─────────────────────────────

    #[test]
    fn search_bar_renders_committed_hints_when_not_input_active() {
        let state = PerformanceState {
            timeline_search_query: Some("Raster".to_string()),
            timeline_search_input_active: false,
            ..Default::default()
        };
        let area = Rect::new(0, 0, 80, 1);
        let mut buf = Buffer::empty(area);
        render_search_bar(area, &mut buf, &state);
        let text = collect_text(&buf);
        assert!(
            text.contains('/'),
            "committed bar should contain '/' prefix"
        );
        assert!(text.contains("Raster"), "committed bar should show query");
    }

    // ── AC11: No render when query is None ────────────────────────────────────

    #[test]
    fn search_bar_no_render_when_query_none() {
        let state = PerformanceState::default();
        let area = Rect::new(0, 0, 80, 1);
        let mut buf = Buffer::empty(area);
        render_search_bar(area, &mut buf, &state);
        // Buffer should be all spaces (empty)
        let text = collect_text(&buf);
        assert!(
            !text.contains('/'),
            "no render when query is None, but got: {text:?}"
        );
    }

    // ── AC15: Empty query shows "no matches" ─────────────────────────────────

    #[test]
    fn search_bar_shows_no_matches_for_empty_query() {
        let state = PerformanceState {
            timeline_search_query: Some(String::new()),
            timeline_search_input_active: true,
            ..Default::default()
        };
        let area = Rect::new(0, 0, 80, 1);
        let mut buf = Buffer::empty(area);
        render_search_bar(area, &mut buf, &state);
        let text = collect_text(&buf);
        assert!(
            text.contains("no matches"),
            "empty query should show 'no matches', got: {text:?}"
        );
    }

    // ── AC11: No panic on zero area ───────────────────────────────────────────

    #[test]
    fn search_bar_no_panic_on_zero_area() {
        let state = PerformanceState {
            timeline_search_query: Some("test".to_string()),
            timeline_search_input_active: true,
            ..Default::default()
        };
        let mut buf = Buffer::empty(Rect::ZERO);
        render_search_bar(Rect::ZERO, &mut buf, &state); // must not panic
    }
}
