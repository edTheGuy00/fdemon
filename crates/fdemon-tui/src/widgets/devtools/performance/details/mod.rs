//! Performance Details pane — tab strip and per-tab dispatch.
//!
//! Renders the tabbed details panel below the Flutter Frames bar chart. The
//! panel contains three tabs:
//!
//! - **Frame Analysis** — populated in Phase 2 ([`frame_analysis_tab`]).
//! - **Rebuild Stats** — Phase 3 ([`rebuild_stats_tab`]); visible only when
//!   `PerformanceState::rebuild_stats_enabled == true`.
//! - **Timeline Events** — Phase 3 ([`timeline_events_tab`]).
//!
//! Tab cycling is keyboard-only (`]` / `[`). The `RebuildStats` tab chip is
//! conditionally hidden from the strip when rebuild tracking is disabled; if the
//! currently selected tab is the hidden `RebuildStats`, the dispatcher falls
//! through to `TimelineEvents`.

use fdemon_app::session::PerformanceState;
use fdemon_app::state::PerfDetailsTab;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
};

use crate::theme::palette;

mod frame_analysis_tab;
mod rebuild_stats_tab;
mod timeline_events_tab;

// ── Layout constants ──────────────────────────────────────────────────────────

/// Tab strip height — labels row (1) + underline row (1).
const TAB_STRIP_HEIGHT: u16 = 2;

/// Horizontal gap between tab labels in cells.
const TAB_GAP: usize = 3;

fn label_for(tab: PerfDetailsTab) -> &'static str {
    match tab {
        PerfDetailsTab::FrameAnalysis => "Frame Analysis",
        PerfDetailsTab::RebuildStats => "Rebuild Stats",
        PerfDetailsTab::TimelineEvents => "Timeline Events",
    }
}

/// Build the list of tabs visible in the strip given the current state.
///
/// `RebuildStats` is included only when `rebuild_stats_enabled == true`.
/// Order is always `FrameAnalysis → RebuildStats? → TimelineEvents`.
fn visible_tabs(state: &PerformanceState) -> Vec<PerfDetailsTab> {
    let mut v = vec![PerfDetailsTab::FrameAnalysis];
    if state.rebuild_stats_enabled {
        v.push(PerfDetailsTab::RebuildStats);
    }
    v.push(PerfDetailsTab::TimelineEvents);
    v
}

/// Resolve the effective tab to dispatch to, applying the selection fall-through.
///
/// If `state.details_tab == RebuildStats` but `!state.rebuild_stats_enabled`,
/// fall through to `TimelineEvents`. This handles the single-frame window
/// between the disable event and the handler's snap-to-next update.
fn effective_tab(state: &PerformanceState) -> PerfDetailsTab {
    if state.details_tab == PerfDetailsTab::RebuildStats && !state.rebuild_stats_enabled {
        PerfDetailsTab::TimelineEvents
    } else {
        state.details_tab
    }
}

/// Render the details pane content inside the supplied inner area.
///
/// `area` is the area inside the block — the caller is responsible for the
/// surrounding border and title.
pub(super) fn render(area: Rect, buf: &mut Buffer, performance: &PerformanceState) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let tabs = visible_tabs(performance);
    let active = effective_tab(performance);

    if area.height <= TAB_STRIP_HEIGHT {
        render_tab_strip(area, buf, active, &tabs);
        return;
    }

    let chunks =
        Layout::vertical([Constraint::Length(TAB_STRIP_HEIGHT), Constraint::Min(0)]).split(area);
    let strip_area = chunks[0];
    let content_area = chunks[1];

    render_tab_strip(strip_area, buf, active, &tabs);

    match active {
        PerfDetailsTab::FrameAnalysis => frame_analysis_tab::render(content_area, buf, performance),
        PerfDetailsTab::RebuildStats => rebuild_stats_tab::render(content_area, buf, performance),
        PerfDetailsTab::TimelineEvents => {
            timeline_events_tab::render(content_area, buf, performance)
        }
    }
}

/// Render the two-row tab strip with the active tab underlined.
///
/// Only tabs in `visible` are rendered. The active tab is highlighted
/// (bold + accent colour) and has an underline of `━` characters in the
/// second row. Inactive tabs use `TEXT_MUTED`.
fn render_tab_strip(
    area: Rect,
    buf: &mut Buffer,
    active: PerfDetailsTab,
    visible: &[PerfDetailsTab],
) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    // ── Row 0: labels ─────────────────────────────────────────────────────────
    let label_y = area.y;

    let mut tab_starts: Vec<u16> = Vec::with_capacity(visible.len());
    let mut tab_widths: Vec<u16> = Vec::with_capacity(visible.len());

    let mut cursor_x = area.x;
    for (i, tab) in visible.iter().enumerate() {
        if cursor_x >= area.x + area.width {
            break;
        }
        let label = label_for(*tab);
        let label_len = label.chars().count() as u16;
        let available = (area.x + area.width).saturating_sub(cursor_x);
        let render_len = label_len.min(available);

        tab_starts.push(cursor_x);
        tab_widths.push(render_len);

        let is_active = *tab == active;
        let style = if is_active {
            Style::default()
                .fg(palette::ACCENT)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(palette::TEXT_MUTED)
        };

        let label_text: String = label.chars().take(render_len as usize).collect();
        buf.set_string(cursor_x, label_y, &label_text, style);
        cursor_x += render_len;

        // Advance past the gap (unless this is the last tab).
        if i + 1 < visible.len() {
            cursor_x += TAB_GAP as u16;
        }
    }

    // ── Row 1: underline ──────────────────────────────────────────────────────
    if area.height < 2 {
        return;
    }
    let underline_y = area.y + 1;

    for (i, (tab, start)) in visible.iter().zip(tab_starts.iter().copied()).enumerate() {
        if i >= tab_widths.len() {
            break;
        }
        let width = tab_widths[i];
        if *tab == active {
            // Draw `━` characters spanning the tab label width.
            for dx in 0..width {
                let x = start + dx;
                if x < area.x + area.width {
                    buf.set_string(
                        x,
                        underline_y,
                        "\u{2501}", // ━
                        Style::default().fg(palette::ACCENT),
                    );
                }
            }
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use fdemon_app::session::PerformanceState;
    use fdemon_app::state::PerfDetailsTab;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    use super::{effective_tab, render, render_tab_strip, visible_tabs};

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

    fn collect_row(buf: &Buffer, y: u16) -> String {
        (0..buf.area.width)
            .filter_map(|x| buf.cell((x, y)))
            .filter_map(|c| c.symbol().chars().next())
            .collect()
    }

    fn make_perf_with_tab(tab: PerfDetailsTab) -> PerformanceState {
        PerformanceState {
            details_tab: tab,
            monitoring_active: true,
            ..Default::default()
        }
    }

    fn make_perf_with_tab_and_rebuild_enabled(
        tab: PerfDetailsTab,
        enabled: bool,
    ) -> PerformanceState {
        PerformanceState {
            details_tab: tab,
            monitoring_active: true,
            rebuild_stats_enabled: enabled,
            ..Default::default()
        }
    }

    // ── visible_tabs helper ──────────────────────────────────────────────────

    #[test]
    fn visible_tabs_excludes_rebuild_stats_when_disabled() {
        let state = make_perf_with_tab_and_rebuild_enabled(PerfDetailsTab::FrameAnalysis, false);
        let tabs = visible_tabs(&state);
        assert!(
            !tabs.contains(&PerfDetailsTab::RebuildStats),
            "RebuildStats should be excluded when disabled"
        );
        assert!(tabs.contains(&PerfDetailsTab::FrameAnalysis));
        assert!(tabs.contains(&PerfDetailsTab::TimelineEvents));
    }

    #[test]
    fn visible_tabs_includes_rebuild_stats_when_enabled() {
        let state = make_perf_with_tab_and_rebuild_enabled(PerfDetailsTab::FrameAnalysis, true);
        let tabs = visible_tabs(&state);
        assert!(tabs.contains(&PerfDetailsTab::RebuildStats));
        assert!(tabs.contains(&PerfDetailsTab::FrameAnalysis));
        assert!(tabs.contains(&PerfDetailsTab::TimelineEvents));
    }

    // ── effective_tab helper ─────────────────────────────────────────────────

    #[test]
    fn effective_tab_falls_through_rebuild_stats_when_disabled() {
        let state = make_perf_with_tab_and_rebuild_enabled(PerfDetailsTab::RebuildStats, false);
        assert_eq!(
            effective_tab(&state),
            PerfDetailsTab::TimelineEvents,
            "RebuildStats selected + disabled should fall through to TimelineEvents"
        );
    }

    #[test]
    fn effective_tab_returns_rebuild_stats_when_enabled() {
        let state = make_perf_with_tab_and_rebuild_enabled(PerfDetailsTab::RebuildStats, true);
        assert_eq!(effective_tab(&state), PerfDetailsTab::RebuildStats);
    }

    #[test]
    fn effective_tab_returns_frame_analysis_unchanged() {
        let state = make_perf_with_tab(PerfDetailsTab::FrameAnalysis);
        assert_eq!(effective_tab(&state), PerfDetailsTab::FrameAnalysis);
    }

    #[test]
    fn effective_tab_returns_timeline_events_unchanged() {
        let state = make_perf_with_tab(PerfDetailsTab::TimelineEvents);
        assert_eq!(effective_tab(&state), PerfDetailsTab::TimelineEvents);
    }

    // ── Tab strip: label visibility (rebuild_stats_enabled = false) ──────────

    #[test]
    fn details_mod_hides_rebuild_stats_tab_when_disabled() {
        let state = make_perf_with_tab_and_rebuild_enabled(PerfDetailsTab::FrameAnalysis, false);
        let tabs = visible_tabs(&state);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 2));
        render_tab_strip(buf.area, &mut buf, PerfDetailsTab::FrameAnalysis, &tabs);
        let text = collect_text(&buf);
        assert!(
            text.contains("Frame Analysis"),
            "expected 'Frame Analysis' label, got:\n{text}"
        );
        assert!(
            text.contains("Timeline Events"),
            "expected 'Timeline Events' label, got:\n{text}"
        );
        assert!(
            !text.contains("Rebuild Stats"),
            "expected NO 'Rebuild Stats' label when disabled, got:\n{text}"
        );
    }

    #[test]
    fn details_mod_shows_rebuild_stats_tab_when_enabled() {
        let state = make_perf_with_tab_and_rebuild_enabled(PerfDetailsTab::FrameAnalysis, true);
        let tabs = visible_tabs(&state);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 2));
        render_tab_strip(buf.area, &mut buf, PerfDetailsTab::FrameAnalysis, &tabs);
        let text = collect_text(&buf);
        assert!(
            text.contains("Frame Analysis"),
            "expected 'Frame Analysis', got:\n{text}"
        );
        assert!(
            text.contains("Rebuild Stats"),
            "expected 'Rebuild Stats', got:\n{text}"
        );
        assert!(
            text.contains("Timeline Events"),
            "expected 'Timeline Events', got:\n{text}"
        );
    }

    // ── Dispatch: falls through when selected tab is hidden ──────────────────

    #[test]
    fn details_mod_dispatch_falls_through_when_selected_tab_hidden() {
        // details_tab == RebuildStats + rebuild_stats_enabled == false
        // → dispatcher should render TimelineEvents content.
        let state = make_perf_with_tab_and_rebuild_enabled(PerfDetailsTab::RebuildStats, false);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 15));
        render(buf.area, &mut buf, &state);
        let text = collect_text(&buf);
        // Timeline Events renders "Waiting for timeline events…" as empty placeholder.
        assert!(
            text.contains("Waiting for timeline events"),
            "expected Timeline Events content (fall-through), got:\n{text}"
        );
    }

    // ── Tab strip: underline ─────────────────────────────────────────────────

    #[test]
    fn tab_strip_underlines_active_frame_analysis_tab() {
        // "Frame Analysis" is 14 chars. With area width 60 and starting at x=0,
        // the underline row (y=1) should start with 14 '━' characters.
        let tabs = vec![
            PerfDetailsTab::FrameAnalysis,
            PerfDetailsTab::RebuildStats,
            PerfDetailsTab::TimelineEvents,
        ];
        let mut buf = Buffer::empty(Rect::new(0, 0, 60, 2));
        render_tab_strip(buf.area, &mut buf, PerfDetailsTab::FrameAnalysis, &tabs);
        let underline_row = collect_row(&buf, 1);
        let underline_count = underline_row.chars().filter(|&c| c == '\u{2501}').count();
        let expected = "Frame Analysis".chars().count();
        assert_eq!(
            underline_count, expected,
            "expected {expected} ━ chars under 'Frame Analysis', got {underline_count}. Row: {underline_row:?}"
        );
    }

    #[test]
    fn tab_strip_underlines_active_rebuild_stats_tab() {
        let tabs = vec![
            PerfDetailsTab::FrameAnalysis,
            PerfDetailsTab::RebuildStats,
            PerfDetailsTab::TimelineEvents,
        ];
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 2));
        render_tab_strip(buf.area, &mut buf, PerfDetailsTab::RebuildStats, &tabs);
        let underline_row = collect_row(&buf, 1);
        let underline_count = underline_row.chars().filter(|&c| c == '\u{2501}').count();
        let expected = "Rebuild Stats".chars().count();
        assert_eq!(
            underline_count, expected,
            "expected {expected} ━ chars under 'Rebuild Stats', got {underline_count}. Row: {underline_row:?}"
        );
    }

    #[test]
    fn tab_strip_underlines_active_timeline_events_tab() {
        let tabs = vec![
            PerfDetailsTab::FrameAnalysis,
            PerfDetailsTab::RebuildStats,
            PerfDetailsTab::TimelineEvents,
        ];
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 2));
        render_tab_strip(buf.area, &mut buf, PerfDetailsTab::TimelineEvents, &tabs);
        let underline_row = collect_row(&buf, 1);
        let underline_count = underline_row.chars().filter(|&c| c == '\u{2501}').count();
        let expected = "Timeline Events".chars().count();
        assert_eq!(
            underline_count, expected,
            "expected {expected} ━ chars under 'Timeline Events', got {underline_count}. Row: {underline_row:?}"
        );
    }

    // ── Dispatch: frame_analysis tab ─────────────────────────────────────────

    #[test]
    fn render_dispatches_frame_analysis_content() {
        let perf = make_perf_with_tab(PerfDetailsTab::FrameAnalysis);
        let mut buf = Buffer::empty(Rect::new(0, 0, 60, 10));
        render(buf.area, &mut buf, &perf);
        let text = collect_text(&buf);
        // No frame selected → shows the no-selection prompt.
        assert!(
            text.contains("Select a frame above"),
            "expected frame analysis no-selection prompt, got:\n{text}"
        );
    }

    // ── Dispatch: rebuild_stats tab (disabled — shows placeholder) ───────────

    #[test]
    fn render_dispatches_rebuild_stats_shows_disabled_placeholder() {
        // rebuild_stats_enabled must be true so the dispatcher routes to the
        // RebuildStats tab (otherwise it falls through to TimelineEvents).
        // With no frames present, it shows the "waiting for first frame" placeholder.
        let perf = PerformanceState {
            details_tab: PerfDetailsTab::RebuildStats,
            rebuild_stats_enabled: true,
            ..Default::default()
        };
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 10));
        render(buf.area, &mut buf, &perf);
        let text = collect_text(&buf);
        // rebuild_stats_enabled = true but no frames → "waiting for first frame"
        assert!(
            text.contains("waiting for first frame"),
            "expected rebuild stats empty placeholder (enabled, no frames), got:\n{text}"
        );
    }

    // ── Dispatch: timeline_events tab ────────────────────────────────────────

    #[test]
    fn render_dispatches_timeline_events_content() {
        let perf = make_perf_with_tab(PerfDetailsTab::TimelineEvents);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 10));
        render(buf.area, &mut buf, &perf);
        let text = collect_text(&buf);
        // No events → shows empty placeholder.
        assert!(
            text.contains("Waiting for timeline events"),
            "expected timeline events empty placeholder, got:\n{text}"
        );
    }

    // ── Edge cases ───────────────────────────────────────────────────────────

    #[test]
    fn render_no_panic_zero_area() {
        let perf = make_perf_with_tab(PerfDetailsTab::FrameAnalysis);
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        render(buf.area, &mut buf, &perf);
        // must not panic
    }

    #[test]
    fn render_no_panic_single_row() {
        let perf = make_perf_with_tab(PerfDetailsTab::FrameAnalysis);
        let mut buf = Buffer::empty(Rect::new(0, 0, 60, 1));
        render(buf.area, &mut buf, &perf);
        // must not panic
    }

    #[test]
    fn render_no_panic_strip_only_height() {
        // Exactly TAB_STRIP_HEIGHT rows — renders strip only, no dispatch.
        let perf = make_perf_with_tab_and_rebuild_enabled(PerfDetailsTab::RebuildStats, true);
        let mut buf = Buffer::empty(Rect::new(0, 0, 60, 2));
        render(buf.area, &mut buf, &perf);
        // must not panic
    }

    #[test]
    fn tab_strip_no_panic_zero_height() {
        let tabs = vec![PerfDetailsTab::FrameAnalysis];
        let mut buf = Buffer::empty(Rect::new(0, 0, 60, 0));
        render_tab_strip(buf.area, &mut buf, PerfDetailsTab::FrameAnalysis, &tabs);
        // must not panic
    }

    #[test]
    fn tab_strip_no_panic_narrow_width() {
        // 10 columns — labels will be truncated.
        let tabs = vec![
            PerfDetailsTab::FrameAnalysis,
            PerfDetailsTab::TimelineEvents,
        ];
        let mut buf = Buffer::empty(Rect::new(0, 0, 10, 2));
        render_tab_strip(buf.area, &mut buf, PerfDetailsTab::FrameAnalysis, &tabs);
        // must not panic
    }
}
