//! Rebuild Stats tab — Phase 3.
//!
//! Renders a 3-column table of widget rebuild counts for the most recent frame
//! (or a disabled/empty placeholder when tracking is off or no data has arrived).

use fdemon_app::session::PerformanceState;
use fdemon_core::rebuild_stats::RebuildLocation;
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget, Wrap},
};

use crate::theme::palette;

// ── Layout constants ──────────────────────────────────────────────────────────

/// Column widths for the rebuild stats table.
///
/// Widget name — wide enough for most class names.
const COL_WIDGET_WIDTH: u16 = 24;
/// Count — right-aligned, up to 5 digits.
const COL_COUNT_WIDTH: u16 = 7;

/// Maximum number of characters to render in the widget name column before
/// truncating with `…`.
const WIDGET_NAME_MAX_CHARS: usize = 23;

/// Maximum number of characters to render in the location column before
/// truncating with `…`.
const LOCATION_MAX_CHARS: usize = 42;

// ── Public entry point ────────────────────────────────────────────────────────

/// Render the Rebuild Stats tab content area.
///
/// Signature matches the Phase-3 dispatch convention: `(area, buf, state)`.
pub(super) fn render(area: Rect, buf: &mut Buffer, state: &PerformanceState) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    if !state.rebuild_stats_enabled {
        render_disabled_placeholder(area, buf);
        return;
    }

    if state.rebuild_stats_frames.is_empty() {
        render_empty_placeholder(
            area,
            buf,
            "Rebuild tracking is ON — waiting for first frame…",
        );
        return;
    }

    render_table(area, buf, state);
}

// ── Table rendering ───────────────────────────────────────────────────────────

fn render_table(area: Rect, buf: &mut Buffer, state: &PerformanceState) {
    // Latest-frame view (matches DevTools default).
    let snapshot = match state.rebuild_stats_frames.back() {
        Some(s) => s,
        None => return,
    };

    // Rows sorted by count descending (default sort, no interactive toggle in Phase 3).
    let mut rows: Vec<&RebuildLocation> = snapshot.rebuilds.iter().collect();
    rows.sort_by(|a, b| b.build_count.cmp(&a.build_count));

    // ── Layout ────────────────────────────────────────────────────────────────
    //
    // Row 0: "Rebuild tracking: ON — R to disable  Frame: 142  Locations: 47"
    // Row 1: Column headers
    // Row 2+: Data rows (scrollable)
    let header_height: u16 = 1;
    let col_header_height: u16 = 1;
    let overhead = header_height + col_header_height;

    if area.height < overhead {
        // Not enough space even for header rows — bail.
        return;
    }

    let chunks = Layout::vertical([
        Constraint::Length(header_height),
        Constraint::Length(col_header_height),
        Constraint::Min(0), // data rows
    ])
    .split(area);

    let header_area = chunks[0];
    let col_header_area = chunks[1];
    let data_area = chunks[2];

    // ── Render-hint write-back ────────────────────────────────────────────────
    let visible_rows = data_area.height as usize;
    // EXCEPTION: TEA render-hint write-back via Cell — see docs/REVIEW_FOCUS.md
    state.details_pane_visible_height.set(visible_rows);

    // ── Header row ────────────────────────────────────────────────────────────
    let frame_num = snapshot.frame_number;
    let location_count = rows.len();
    let header_line = Line::from(vec![
        Span::styled(
            "Rebuild tracking: ON — R to disable",
            Style::default().fg(palette::STATUS_GREEN),
        ),
        Span::raw("    "),
        Span::styled("Frame: ", Style::default().fg(palette::TEXT_SECONDARY)),
        Span::styled(
            format!("{}", frame_num),
            Style::default().fg(palette::TEXT_PRIMARY),
        ),
        Span::raw("    "),
        Span::styled("Locations: ", Style::default().fg(palette::TEXT_SECONDARY)),
        Span::styled(
            format!("{}", location_count),
            Style::default().fg(palette::TEXT_PRIMARY),
        ),
    ]);
    buf.set_line(
        header_area.x,
        header_area.y,
        &header_line,
        header_area.width,
    );

    // ── Column headers ────────────────────────────────────────────────────────
    //
    // Layout: [Widget (fixed)] [Location (fill)] [Count (fixed, right-aligned)]
    // Use the same column widths as the data rows.
    let (widget_col_w, location_col_w, count_col_w) = compute_col_widths(col_header_area.width);

    let col_header_line = Line::from(vec![
        Span::styled(
            pad_right("Widget", widget_col_w as usize),
            Style::default()
                .fg(palette::TEXT_SECONDARY)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            pad_right("Location", location_col_w as usize),
            Style::default()
                .fg(palette::TEXT_SECONDARY)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            pad_left("Count", count_col_w as usize),
            Style::default()
                .fg(palette::TEXT_SECONDARY)
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    buf.set_line(
        col_header_area.x,
        col_header_area.y,
        &col_header_line,
        col_header_area.width,
    );

    // ── Data rows ─────────────────────────────────────────────────────────────
    if data_area.height == 0 {
        return;
    }

    // Apply scroll offset with clamp.
    let scroll_offset = state
        .rebuild_stats_scroll_offset
        .min(rows.len().saturating_sub(1));

    let visible_slice = &rows[scroll_offset..];

    for (row_idx, rebuild) in visible_slice.iter().enumerate() {
        let y = data_area.y + row_idx as u16;
        if y >= data_area.y + data_area.height {
            break;
        }

        let absolute_row = scroll_offset + row_idx;
        let is_selected = state.rebuild_stats_selected_row == Some(absolute_row);

        let widget_name = truncate_with_ellipsis(&rebuild.location.name, WIDGET_NAME_MAX_CHARS);
        // Format: "package:foo/bar.dart:42"
        let location_str = format!("{}:{}", rebuild.location.file_uri, rebuild.location.line);
        let location_display = truncate_with_ellipsis(&location_str, LOCATION_MAX_CHARS);

        let (w_col, l_col, c_col) = compute_col_widths(data_area.width);

        let base_style = if is_selected {
            Style::default()
                .bg(palette::SELECTED_ROW_BG)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        let name_style = if is_selected {
            base_style.fg(palette::TEXT_BRIGHT)
        } else {
            base_style.fg(palette::TEXT_PRIMARY)
        };
        let location_style = if is_selected {
            base_style.fg(palette::TEXT_SECONDARY)
        } else {
            base_style.fg(palette::TEXT_MUTED)
        };
        let count_style = if is_selected {
            base_style.fg(palette::ACCENT).add_modifier(Modifier::BOLD)
        } else {
            base_style.fg(palette::TEXT_PRIMARY)
        };

        let row_line = Line::from(vec![
            Span::styled(pad_right(&widget_name, w_col as usize), name_style),
            Span::raw(" "),
            Span::styled(pad_right(&location_display, l_col as usize), location_style),
            Span::raw(" "),
            Span::styled(
                pad_left(&rebuild.build_count.to_string(), c_col as usize),
                count_style,
            ),
        ]);

        // Fill the full row width with the selection background if selected.
        if is_selected {
            for dx in 0..data_area.width {
                if let Some(cell) = buf.cell_mut((data_area.x + dx, y)) {
                    cell.set_style(base_style);
                }
            }
        }

        buf.set_line(data_area.x, y, &row_line, data_area.width);
    }

    // Empty state within data area when all rows are consumed.
    if rows.is_empty() {
        render_empty_placeholder(
            data_area,
            buf,
            "No rebuilds in the most recent frame. Interact with the app to trigger widget builds.",
        );
    }
}

// ── Column-width helper ───────────────────────────────────────────────────────

/// Compute the three column widths (widget, location, count) given the total
/// available width.
///
/// Fixed: widget = [`COL_WIDGET_WIDTH`], count = [`COL_COUNT_WIDTH`], two
/// separator spaces. Location gets the remainder (minimum 0).
fn compute_col_widths(total_width: u16) -> (u16, u16, u16) {
    let separators = 2u16; // one space before location, one before count
    let fixed = COL_WIDGET_WIDTH + COL_COUNT_WIDTH + separators;
    let location_col = total_width.saturating_sub(fixed);
    (COL_WIDGET_WIDTH, location_col, COL_COUNT_WIDTH)
}

// ── Placeholder helpers ───────────────────────────────────────────────────────

fn render_disabled_placeholder(area: Rect, buf: &mut Buffer) {
    let message =
        "Rebuild tracking is OFF.\nPress R to enable.\n(Tab will be hidden when toggle settles.)";
    let p = Paragraph::new(message)
        .style(Style::default().fg(palette::TEXT_MUTED))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });
    let line_count = 3u16;
    let y_offset = area.height.saturating_sub(line_count) / 2;
    let centered = Rect {
        y: area.y + y_offset,
        height: area.height.saturating_sub(y_offset),
        ..area
    };
    p.render(centered, buf);
}

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

/// Truncate to `max_chars` Unicode scalar values, appending `…` if truncated.
fn truncate_with_ellipsis(s: &str, max_chars: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_chars {
        s.to_owned()
    } else {
        let truncated: String = chars[..max_chars.saturating_sub(1)].iter().collect();
        format!("{truncated}…")
    }
}

/// Right-pad `s` with spaces to exactly `width` grapheme positions.
///
/// If `s` is already wider than `width`, it is returned unchanged (no
/// truncation — callers should pre-truncate with [`truncate_with_ellipsis`]).
fn pad_right(s: &str, width: usize) -> String {
    let len = s.chars().count();
    if len >= width {
        s.to_owned()
    } else {
        format!("{}{}", s, " ".repeat(width - len))
    }
}

/// Left-pad `s` with spaces to exactly `width` grapheme positions.
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
    use fdemon_core::rebuild_stats::{Location, RebuildLocation, RebuildStatsSnapshot};
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

    fn make_rebuild_location(name: &str, file: &str, line: u32, count: u32) -> RebuildLocation {
        RebuildLocation {
            location: Location {
                file_uri: file.to_owned(),
                line,
                column: 1,
                name: name.to_owned(),
            },
            build_count: count,
        }
    }

    fn make_snapshot(rebuilds: Vec<RebuildLocation>) -> RebuildStatsSnapshot {
        RebuildStatsSnapshot {
            frame_number: 142,
            start_time_micros: 0,
            rebuilds,
        }
    }

    // ── Disabled state ────────────────────────────────────────────────────────

    #[test]
    fn rebuild_stats_tab_renders_disabled_state() {
        let state = PerformanceState {
            rebuild_stats_enabled: false,
            ..Default::default()
        };
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 10));
        render(buf.area, &mut buf, &state);
        let text = collect_text(&buf);
        assert!(
            text.contains("Press R to enable"),
            "expected 'Press R to enable' in disabled placeholder, got:\n{text}"
        );
    }

    // ── Empty-frames state ────────────────────────────────────────────────────

    #[test]
    fn rebuild_stats_tab_renders_empty_frames_state() {
        let state = PerformanceState {
            rebuild_stats_enabled: true,
            ..Default::default()
        };
        // rebuild_stats_frames is empty by default
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 10));
        render(buf.area, &mut buf, &state);
        let text = collect_text(&buf);
        assert!(
            text.contains("waiting for first frame"),
            "expected 'waiting for first frame' in empty placeholder, got:\n{text}"
        );
    }

    // ── Table with selection ──────────────────────────────────────────────────

    #[test]
    fn rebuild_stats_tab_renders_table_with_selection() {
        let mut state = PerformanceState {
            rebuild_stats_enabled: true,
            rebuild_stats_selected_row: Some(2),
            ..Default::default()
        };
        let rebuilds = vec![
            make_rebuild_location("Container", "package:foo/main.dart", 23, 18),
            make_rebuild_location("Padding", "package:foo/main.dart", 45, 12),
            make_rebuild_location("Text", "package:foo/widgets/title.dart", 12, 8),
            make_rebuild_location("Column", "package:foo/main.dart", 67, 5),
            make_rebuild_location("Row", "package:foo/main.dart", 89, 3),
        ];
        state
            .rebuild_stats_frames
            .push_back(make_snapshot(rebuilds));

        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 15));
        render(buf.area, &mut buf, &state);
        let text = collect_text(&buf);

        // Table must contain widget names.
        assert!(
            text.contains("Container"),
            "expected Container in output, got:\n{text}"
        );
        assert!(
            text.contains("18"),
            "expected count 18 in output, got:\n{text}"
        );

        // Verify that the selected row (index 2 = "Text" after sort-by-count-desc)
        // has the BOLD modifier on its cell.
        // Row 2 (0-based) in the data area: y = 2 (header) + 1 (col header) + 2 (data offset).
        let data_start_y = 2u16; // header (1) + col header (1)
        let selected_data_y = data_start_y + 2; // row index 2
        let cell = buf.cell((0, selected_data_y));
        if selected_data_y < buf.area.height {
            if let Some(c) = cell {
                assert!(
                    c.style().add_modifier.contains(Modifier::BOLD),
                    "selected row at y={} should be bold",
                    selected_data_y
                );
            }
        }
    }

    // ── Render-hint write-back ────────────────────────────────────────────────

    #[test]
    fn rebuild_stats_tab_writes_render_hint_height() {
        let mut state = PerformanceState {
            rebuild_stats_enabled: true,
            ..Default::default()
        };
        let rebuilds = vec![make_rebuild_location(
            "Container",
            "package:foo/main.dart",
            23,
            18,
        )];
        state
            .rebuild_stats_frames
            .push_back(make_snapshot(rebuilds));

        let area = Rect::new(0, 0, 80, 12);
        let mut buf = Buffer::empty(area);
        render(area, &mut buf, &state);

        // The visible height = area.height - header (1) - col_header (1) = 10.
        let expected_height = 10usize;
        assert_eq!(
            state.details_pane_visible_height.get(),
            expected_height,
            "render-hint height should equal data area height"
        );
    }

    // ── Zero-area guard ───────────────────────────────────────────────────────

    #[test]
    fn rebuild_stats_tab_no_panic_zero_area() {
        let state = PerformanceState::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        render(buf.area, &mut buf, &state); // must not panic
    }

    // ── Sorting: highest count first ──────────────────────────────────────────

    #[test]
    fn rebuild_stats_tab_sorts_by_count_descending() {
        let mut state = PerformanceState {
            rebuild_stats_enabled: true,
            ..Default::default()
        };
        let rebuilds = vec![
            make_rebuild_location("Low", "package:foo/main.dart", 1, 2),
            make_rebuild_location("High", "package:foo/main.dart", 2, 99),
            make_rebuild_location("Mid", "package:foo/main.dart", 3, 50),
        ];
        state
            .rebuild_stats_frames
            .push_back(make_snapshot(rebuilds));

        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 12));
        render(buf.area, &mut buf, &state);
        let text = collect_text(&buf);

        // "High" should appear before "Mid" and "Low" in the output.
        let high_pos = text.find("High").unwrap_or(usize::MAX);
        let mid_pos = text.find("Mid").unwrap_or(usize::MAX);
        let low_pos = text.find("Low").unwrap_or(usize::MAX);
        assert!(
            high_pos < mid_pos && mid_pos < low_pos,
            "expected sort order High > Mid > Low, got positions: high={}, mid={}, low={}",
            high_pos,
            mid_pos,
            low_pos
        );
    }

    // ── Helper unit tests ─────────────────────────────────────────────────────

    #[test]
    fn truncate_with_ellipsis_short_string_unchanged() {
        assert_eq!(truncate_with_ellipsis("Hello", 10), "Hello");
    }

    #[test]
    fn truncate_with_ellipsis_exact_length_unchanged() {
        assert_eq!(truncate_with_ellipsis("Hello", 5), "Hello");
    }

    #[test]
    fn truncate_with_ellipsis_long_string_truncated() {
        let result = truncate_with_ellipsis("Hello World", 7);
        assert_eq!(result.chars().count(), 7);
        assert!(result.ends_with('…'));
    }

    #[test]
    fn pad_right_pads_shorter_string() {
        let result = pad_right("Hi", 5);
        assert_eq!(result, "Hi   ");
        assert_eq!(result.len(), 5);
    }

    #[test]
    fn pad_left_pads_shorter_string() {
        let result = pad_left("42", 5);
        assert_eq!(result, "   42");
        assert_eq!(result.len(), 5);
    }
}
