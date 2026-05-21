//! Timeline event details popup overlay.
//!
//! Renders a centered modal popup with the selected event's metadata:
//! name, category, thread, timestamps, duration, parent chain, and child count.
//! Uses the [`modal_overlay`] helpers for chrome (clear, dim, shadow).
//!
//! The popup is rendered last in the timeline_events `render` function so it
//! draws on top of the Gantt. When the popup is visible, it is a modal — clicks
//! on the Gantt beneath it are no-ops (the caller should pass `None` as
//! `MouseCtx` to the Gantt when `timeline_details_popup_open == true`).

use fdemon_app::session::{PerformanceState, TimelineEventCursor};
use fdemon_core::timeline::{TimelineNode, TimelineTrack};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};

use crate::theme::palette;
use crate::widgets::modal_overlay::{centered_rect, clear_area, dim_background, render_shadow};

use super::text_helpers::truncate_with_ellipsis;

// ── Layout constants ──────────────────────────────────────────────────────────

/// Popup width in columns. Wide enough to show full event names without wrapping.
/// Derived: 60 chars for content + 2 for borders = 62 columns.
const POPUP_WIDTH: u16 = 64;

/// Popup height in rows. Derived: 8 body lines + 2 border + 1 footer + 2 padding = 13.
const POPUP_HEIGHT: u16 = 14;

/// Maximum nodes in the parent chain breadcrumb before truncation with "…".
const MAX_BREADCRUMB_NODES: usize = 4;

/// Maximum length for an individual node name in the breadcrumb.
const BREADCRUMB_NAME_MAX_LEN: usize = 20;

// ── Public render entry ───────────────────────────────────────────────────────

/// Render the timeline event details popup.
///
/// Called by `timeline_events::render` when `state.timeline_details_popup_open == true`
/// and a selected event is present. Renders over the entire Gantt area (dims
/// the background, draws a centered popup).
///
/// If the selected event cannot be found in `timeline_tracks` (evicted), renders
/// a "Event no longer available" placeholder.
pub(super) fn render(area: Rect, buf: &mut Buffer, state: &PerformanceState) {
    let Some(cursor) = state.timeline_selected_event else {
        return;
    };
    if !state.timeline_details_popup_open {
        return;
    }

    // Dim the background to signal modal.
    dim_background(buf, area);

    // Compute popup rect.
    let popup_rect = centered_rect(POPUP_WIDTH, POPUP_HEIGHT, area);

    // Shadow below/right.
    render_shadow(buf, popup_rect);

    // Clear and prepare the popup area.
    clear_area(buf, popup_rect);

    // Render popup content.
    let inner = popup_rect.inner(ratatui::layout::Margin {
        horizontal: 1,
        vertical: 1,
    });

    if let Some(track) = state.timeline_tracks.get(&cursor.tid) {
        if let Some((node, parent_chain)) = find_node_with_chain(track, cursor) {
            render_popup_content(popup_rect, inner, buf, node, track, &parent_chain, cursor);
        } else {
            render_evicted_popup(popup_rect, inner, buf);
        }
    } else {
        render_evicted_popup(popup_rect, inner, buf);
    }
}

// ── Popup chrome and content ──────────────────────────────────────────────────

fn render_popup_content(
    popup_rect: Rect,
    inner: Rect,
    buf: &mut Buffer,
    node: &TimelineNode,
    track: &TimelineTrack,
    parent_chain: &[&TimelineNode],
    cursor: TimelineEventCursor,
) {
    // Draw border.
    let border_style = Style::default().fg(palette::ACCENT);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(Span::styled(
            " Event Details ",
            Style::default()
                .fg(palette::ACCENT)
                .add_modifier(Modifier::BOLD),
        ));
    block.render(popup_rect, buf);

    if inner.height == 0 || inner.width == 0 {
        return;
    }

    // Split inner into body and footer.
    // Footer hint line: 1 row. Body gets the rest.
    let inner_height = inner.height;
    let footer_height: u16 = 1;
    let body_height = inner_height.saturating_sub(footer_height);

    let parts = Layout::vertical([
        Constraint::Length(body_height),
        Constraint::Length(footer_height),
    ])
    .split(inner);

    let body_area = parts[0];
    let footer_area = parts[1];

    render_body(body_area, buf, node, track, parent_chain, cursor);
    render_footer(footer_area, buf);
}

fn render_body(
    area: Rect,
    buf: &mut Buffer,
    node: &TimelineNode,
    track: &TimelineTrack,
    parent_chain: &[&TimelineNode],
    cursor: TimelineEventCursor,
) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let label_style = Style::default()
        .fg(palette::TEXT_SECONDARY)
        .add_modifier(Modifier::BOLD);
    let value_style = Style::default().fg(palette::TEXT_PRIMARY);
    let muted_style = Style::default().fg(palette::TEXT_MUTED);

    // Format duration.
    let dur_micros = node.dur.unwrap_or(0);
    let dur_str = format_micros(dur_micros as u64);

    // Format start timestamp.
    let ts_str = format!("{} μs", node.ts);

    // Thread label.
    let thread_name_fallback = format!("{:?}", track.thread);
    let thread_name = track.name.as_deref().unwrap_or(&thread_name_fallback);
    let thread_str = format!("{} (tid {})", thread_name, cursor.tid);

    // Category.
    let category_str = node.category.as_deref().unwrap_or("—");

    // Parent chain breadcrumb.
    let breadcrumb = build_breadcrumb(parent_chain, &node.name);

    // Children count (direct only).
    let children_count = node.children.len();

    // Build body lines. Temporaries must outlive the `lines` vec.
    let phase_str = format!("{:?}", node.phase);
    let children_str = children_count.to_string();
    let lines: Vec<Line> = vec![
        make_field_line("Name:    ", &node.name, label_style, value_style),
        make_field_line("Category:", category_str, label_style, value_style),
        make_field_line("Thread:  ", &thread_str, label_style, muted_style),
        make_field_line("Start:   ", &ts_str, label_style, muted_style),
        make_field_line("Duration:", &dur_str, label_style, value_style),
        make_field_line("Phase:   ", &phase_str, label_style, muted_style),
        make_field_line("Path:    ", &breadcrumb, label_style, muted_style),
        make_field_line("Children:", &children_str, label_style, value_style),
    ];

    let p = Paragraph::new(lines);
    p.render(area, buf);
}

fn render_footer(area: Rect, buf: &mut Buffer) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let hint = "Esc: close   ←/→: prev/next sibling   ↑/↓: parent/child";
    let style = Style::default()
        .fg(palette::TEXT_MUTED)
        .add_modifier(Modifier::DIM);
    let truncated = truncate_with_ellipsis(hint, area.width as usize);
    buf.set_string(area.x, area.y, &truncated, style);
}

fn render_evicted_popup(popup_rect: Rect, inner: Rect, buf: &mut Buffer) {
    let border_style = Style::default().fg(Color::Yellow);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(Span::styled(
            " Event Details ",
            Style::default().fg(Color::Yellow),
        ));
    block.render(popup_rect, buf);

    if inner.height == 0 || inner.width == 0 {
        return;
    }

    let msg = "Event no longer available (evicted from buffer). Press Esc to close.";
    let p = Paragraph::new(truncate_with_ellipsis(msg, inner.width as usize))
        .style(Style::default().fg(palette::TEXT_MUTED));
    p.render(inner, buf);
}

// ── Helper functions ──────────────────────────────────────────────────────────

fn make_field_line<'a>(
    label: &'a str,
    value: &'a str,
    label_style: Style,
    value_style: Style,
) -> Line<'a> {
    Line::from(vec![
        Span::styled(label, label_style),
        Span::raw(" "),
        Span::styled(value.to_owned(), value_style),
    ])
}

/// Build a breadcrumb path: `root → … → parent → current`.
///
/// When the chain is longer than [`MAX_BREADCRUMB_NODES`], truncates with `…`
/// to keep the breadcrumb short. Individual names are capped at
/// [`BREADCRUMB_NAME_MAX_LEN`] chars.
fn build_breadcrumb(parent_chain: &[&TimelineNode], current_name: &str) -> String {
    let current = truncate_with_ellipsis(current_name, BREADCRUMB_NAME_MAX_LEN);

    if parent_chain.is_empty() {
        return current;
    }

    // Build chain: parents + current
    let mut parts: Vec<String> = parent_chain
        .iter()
        .map(|n| truncate_with_ellipsis(&n.name, BREADCRUMB_NAME_MAX_LEN))
        .collect();
    parts.push(current);

    // Truncate if too long.
    if parts.len() > MAX_BREADCRUMB_NODES {
        let last_two: Vec<String> = parts
            .iter()
            .rev()
            .take(MAX_BREADCRUMB_NODES - 2)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        let first = parts[0].clone();
        let mut truncated = vec![first, "…".to_string()];
        truncated.extend(last_two);
        return truncated.join(" → ");
    }

    parts.join(" → ")
}

/// Format microseconds as a human-readable string (ms for < 1s, μs otherwise).
fn format_micros(micros: u64) -> String {
    if micros >= 1_000_000 {
        format!("{:.3}s ({micros} μs)", micros as f64 / 1_000_000.0)
    } else if micros >= 1_000 {
        format!("{:.3}ms ({micros} μs)", micros as f64 / 1_000.0)
    } else {
        format!("{micros} μs")
    }
}

/// Walk the timeline tree to find the node at `cursor` and collect its parent chain.
///
/// Returns `Some((node, parent_chain))` where `parent_chain` is ordered from
/// root to immediate parent. Returns `None` if the cursor is not found (evicted).
fn find_node_with_chain(
    track: &TimelineTrack,
    cursor: TimelineEventCursor,
) -> Option<(&TimelineNode, Vec<&TimelineNode>)> {
    find_in_slice_with_chain(&track.root_events, cursor, 0, &[])
}

fn find_in_slice_with_chain<'a>(
    nodes: &'a [TimelineNode],
    cursor: TimelineEventCursor,
    depth: u8,
    ancestors: &[&'a TimelineNode],
) -> Option<(&'a TimelineNode, Vec<&'a TimelineNode>)> {
    if depth == cursor.depth {
        // Look for node with matching ts.
        return nodes.iter().find(|n| n.ts == cursor.ts).map(|n| {
            let chain = ancestors.to_vec();
            (n, chain)
        });
    }
    // Descend into children.
    for node in nodes {
        let mut new_ancestors = ancestors.to_vec();
        new_ancestors.push(node);
        if let Some(result) =
            find_in_slice_with_chain(&node.children, cursor, depth + 1, &new_ancestors)
        {
            return Some(result);
        }
    }
    None
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use fdemon_app::session::PerformanceState;
    use fdemon_core::timeline::{TimelineNode, TimelinePhase, TimelineThread, TimelineTrack};
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use std::collections::BTreeMap;

    fn make_node(name: &str, ts: i64, dur: i64) -> TimelineNode {
        TimelineNode {
            name: name.to_owned(),
            category: Some("Embedder".to_owned()),
            ts,
            dur: Some(dur),
            phase: TimelinePhase::Complete,
            thread: TimelineThread::Ui,
            frame_number: Some(1),
            children: vec![],
        }
    }

    #[test]
    fn test_popup_renders_when_open() {
        let ts = 1_000_000i64;
        let dur = 500_000i64;
        let node = make_node("TestEvent", ts, dur);
        let mut tracks = BTreeMap::new();
        tracks.insert(
            1i64,
            TimelineTrack {
                tid: 1,
                name: Some("io.flutter.ui".to_owned()),
                thread: TimelineThread::Ui,
                root_events: vec![node],
            },
        );
        let state = PerformanceState {
            timeline_tracks: tracks,
            timeline_selected_event: Some(TimelineEventCursor {
                tid: 1,
                depth: 0,
                ts,
            }),
            timeline_details_popup_open: true,
            ..Default::default()
        };

        let area = Rect::new(0, 0, 100, 30);
        let mut buf = Buffer::empty(area);
        render(area, &mut buf, &state);

        // The popup should contain the event name.
        let text: String = (0..buf.area.height)
            .flat_map(|y| (0..buf.area.width).map(move |x| (x, y)))
            .filter_map(|(x, y)| buf.cell((x, y)))
            .flat_map(|c| c.symbol().chars())
            .collect();
        assert!(
            text.contains("TestEvent"),
            "popup should show event name, got:\n{text}"
        );
    }

    #[test]
    fn test_popup_not_rendered_when_closed() {
        let state = PerformanceState {
            timeline_selected_event: Some(TimelineEventCursor {
                tid: 1,
                depth: 0,
                ts: 0,
            }),
            timeline_details_popup_open: false,
            ..Default::default()
        };

        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);
        render(area, &mut buf, &state);

        // Buffer should be unchanged (no popup rendered).
        let text: String = (0..buf.area.height)
            .flat_map(|y| (0..buf.area.width).map(move |x| (x, y)))
            .filter_map(|(x, y)| buf.cell((x, y)))
            .flat_map(|c| c.symbol().chars())
            .collect();
        // No content should be written.
        assert!(
            !text.contains("Event Details"),
            "popup should not render when closed"
        );
    }

    #[test]
    fn test_format_micros_sub_ms() {
        assert_eq!(format_micros(500), "500 μs");
    }

    #[test]
    fn test_format_micros_ms_range() {
        let s = format_micros(16_000);
        assert!(s.contains("ms"), "expected ms suffix, got: {s}");
    }

    #[test]
    fn test_format_micros_seconds_range() {
        let s = format_micros(1_500_000);
        assert!(s.contains('s'), "expected s suffix, got: {s}");
    }

    #[test]
    fn test_build_breadcrumb_no_parents() {
        let result = build_breadcrumb(&[], "LeafEvent");
        assert_eq!(result, "LeafEvent");
    }

    #[test]
    fn test_build_breadcrumb_with_parents() {
        let root = make_node("Root", 0, 100);
        let parent = make_node("Parent", 10, 50);
        let chain = vec![&root, &parent];
        let result = build_breadcrumb(&chain, "Child");
        assert_eq!(result, "Root → Parent → Child");
    }

    #[test]
    fn test_popup_evicted_event_shows_placeholder() {
        // Selected event references tid=99 which has no track.
        let state = PerformanceState {
            timeline_selected_event: Some(TimelineEventCursor {
                tid: 99,
                depth: 0,
                ts: 0,
            }),
            timeline_details_popup_open: true,
            ..Default::default()
        };

        let area = Rect::new(0, 0, 100, 30);
        let mut buf = Buffer::empty(area);
        render(area, &mut buf, &state);

        let text: String = (0..buf.area.height)
            .flat_map(|y| (0..buf.area.width).map(move |x| (x, y)))
            .filter_map(|(x, y)| buf.cell((x, y)))
            .flat_map(|c| c.symbol().chars())
            .collect();
        assert!(
            text.contains("Event Details"),
            "should still show popup chrome for evicted events"
        );
    }
}
