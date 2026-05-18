//! Frame Analysis tab content — Phase 2.
//!
//! Renders the selected frame's phase breakdown, total / budget verdict, and
//! a diagnostic hint list. When no frame is selected, shows a prompt. When the
//! selected frame has no `FramePhases` data, falls back to the aggregate
//! build+raster split. When width is below [`MIN_PHASE_BAR_WIDTH`], the
//! proportional bar degrades to an inline `B/L/P/R` summary line.

use fdemon_app::session::PerformanceState;
use fdemon_core::frame_hints::frame_hints;
use fdemon_core::performance::{FramePhases, FrameTiming};
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget, Wrap},
};

use super::super::MIN_PHASE_BAR_WIDTH;
use crate::theme::palette;

// ── Phase bar colours (kept in sync with frame_chart styling) ────────────────

const COLOR_BUILD: Color = Color::Cyan;
const COLOR_LAYOUT: Color = Color::LightBlue;
const COLOR_PAINT: Color = Color::Magenta;
const COLOR_RASTER: Color = Color::Green;

/// Maximum number of hint lines to render. Mirrors `MAX_HINTS_PER_FRAME`.
const MAX_HINT_LINES: usize = 5;

/// Render the Frame Analysis tab content area.
pub(super) fn render(area: Rect, buf: &mut Buffer, performance: &PerformanceState) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    match performance.selected_frame_timing() {
        Some(frame) => render_selected(area, buf, frame, performance.display_refresh_rate),
        None => render_no_selection(area, buf),
    }
}

fn render_no_selection(area: Rect, buf: &mut Buffer) {
    let message = "Select a frame above (←/→) to view analysis.";
    let p = Paragraph::new(message)
        .style(Style::default().fg(palette::TEXT_MUTED))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });
    let y_offset = area.height.saturating_sub(1) / 2;
    let centered = Rect {
        y: area.y + y_offset,
        height: 1,
        ..area
    };
    p.render(centered, buf);
}

fn render_selected(area: Rect, buf: &mut Buffer, frame: &FrameTiming, refresh_rate_hz: f64) {
    // Layout: header line (1) + verdict line (1) + spacer (1) + phase bar (1 or 3) + hints (≤ 5)
    let bar_rows = phase_bar_rows(area.width, frame);
    let chunks = Layout::vertical([
        Constraint::Length(1),        // header
        Constraint::Length(1),        // verdict
        Constraint::Length(1),        // spacer
        Constraint::Length(bar_rows), // phase bar (1 or 3 rows)
        Constraint::Min(0),           // hints
    ])
    .split(area);

    render_header(chunks[0], buf, frame);
    render_verdict(chunks[1], buf, frame, refresh_rate_hz);
    // chunks[2] is the spacer — intentionally left blank
    render_phase_bar(chunks[3], buf, frame);
    render_hints(chunks[4], buf, frame, refresh_rate_hz);
}

// ── Phase bar ─────────────────────────────────────────────────────────────────

/// Choose phase-bar height based on available width and data.
///
/// - 3 rows: proportional 4-segment bar (label row + 1 bar row + spacer).
/// - 1 row: inline `B 6.1ms | L 2.0ms | P 3.4ms | R 6.7ms` fallback, or
///   aggregate split when no `FramePhases` data is available.
fn phase_bar_rows(width: u16, frame: &FrameTiming) -> u16 {
    if frame.phases.is_none() {
        return 1; // aggregate-only fallback also uses 1 row
    }
    if width < MIN_PHASE_BAR_WIDTH {
        1
    } else {
        3
    }
}

fn render_phase_bar(area: Rect, buf: &mut Buffer, frame: &FrameTiming) {
    match &frame.phases {
        Some(phases) if area.width >= MIN_PHASE_BAR_WIDTH && area.height >= 3 => {
            render_proportional_phase_bar(area, buf, phases);
        }
        Some(phases) => {
            render_inline_phase_summary(area, buf, phases);
        }
        None => {
            render_aggregate_split(area, buf, frame);
        }
    }
}

/// Proportional 4-segment bar: each phase's width = (phase / total) * available_cols.
///
/// Renders 3 rows:
///  - Row 0: label line — "Build 6.1ms" / "Layout 2.0ms" etc., centred within segment.
///  - Row 1: █ block characters in each segment's colour.
///  - Row 2: spacer (blank).
fn render_proportional_phase_bar(area: Rect, buf: &mut Buffer, phases: &FramePhases) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let total = phases.total_micros().max(1);
    let cols = area.width as u64;

    // Compute cell counts; ensure they sum to area.width (handle rounding by giving
    // the remainder to raster — the longest segment in practice).
    let build_cells = (phases.build_micros * cols / total) as u16;
    let layout_cells = (phases.layout_micros * cols / total) as u16;
    let paint_cells = (phases.paint_micros * cols / total) as u16;
    let raster_cells = area
        .width
        .saturating_sub(build_cells + layout_cells + paint_cells);

    let segments: &[(u16, Color, &str, u64)] = &[
        (build_cells, COLOR_BUILD, "Build", phases.build_micros),
        (layout_cells, COLOR_LAYOUT, "Layout", phases.layout_micros),
        (paint_cells, COLOR_PAINT, "Paint", phases.paint_micros),
        (raster_cells, COLOR_RASTER, "Raster", phases.raster_micros),
    ];

    // ── Row 0: label line ─────────────────────────────────────────────────────
    if area.height >= 1 {
        let label_y = area.y;
        let mut x = area.x;
        for &(width, color, name, micros) in segments {
            if width == 0 {
                continue;
            }
            let ms = micros as f64 / 1000.0;
            // Full label e.g. "Build 6.1ms"; short label "B"; min label single char.
            let full_label = format!("{} {:.1}ms", name, ms);
            let short_label = format!("{} {:.1}ms", &name[..1], ms);
            let min_label = &name[..1];

            let label: &str = if full_label.len() as u16 <= width {
                &full_label
            } else if short_label.len() as u16 <= width {
                &short_label
            } else {
                min_label
            };

            let avail = width.saturating_sub(label.len() as u16);
            let pad = avail / 2;
            let label_x = x + pad;
            let render_width = label.len().min(width as usize) as u16;
            buf.set_string(label_x, label_y, label, Style::default().fg(color));
            let _ = render_width; // used implicitly by ratatui set_string
            x += width;
        }
    }

    // ── Row 1: █ bar ──────────────────────────────────────────────────────────
    if area.height >= 2 {
        let bar_y = area.y + 1;
        let mut x = area.x;
        for &(width, color, _, _) in segments {
            let style = Style::default().bg(color).fg(color);
            for dx in 0..width {
                if x + dx < area.x + area.width {
                    if let Some(cell) = buf.cell_mut((x + dx, bar_y)) {
                        cell.set_style(style).set_char('\u{2588}'); // █
                    }
                }
            }
            x += width;
        }
    }

    // Row 2 is the spacer — intentionally left blank.
}

/// Inline `B 6.1ms | L 2.0ms | P 3.4ms | R 6.7ms` rendered on one row.
fn render_inline_phase_summary(area: Rect, buf: &mut Buffer, phases: &FramePhases) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let line = Line::from(vec![
        Span::styled(
            format!("B {:.1}ms", phases.build_micros as f64 / 1000.0),
            Style::default().fg(COLOR_BUILD),
        ),
        Span::raw(" | "),
        Span::styled(
            format!("L {:.1}ms", phases.layout_micros as f64 / 1000.0),
            Style::default().fg(COLOR_LAYOUT),
        ),
        Span::raw(" | "),
        Span::styled(
            format!("P {:.1}ms", phases.paint_micros as f64 / 1000.0),
            Style::default().fg(COLOR_PAINT),
        ),
        Span::raw(" | "),
        Span::styled(
            format!("R {:.1}ms", phases.raster_micros as f64 / 1000.0),
            Style::default().fg(COLOR_RASTER),
        ),
    ]);
    buf.set_line(area.x, area.y, &line, area.width);
}

/// `Phase data not available. Aggregate: build 4.2 ms, raster 6.7 ms.`
fn render_aggregate_split(area: Rect, buf: &mut Buffer, frame: &FrameTiming) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let build_ms = frame.build_ms();
    let raster_ms = frame.raster_ms();
    let line = Line::from(vec![
        Span::styled(
            "Phase data not available. ",
            Style::default().fg(palette::TEXT_MUTED),
        ),
        Span::raw("Aggregate: build "),
        Span::styled(
            format!("{:.1} ms", build_ms),
            Style::default().fg(COLOR_BUILD),
        ),
        Span::raw(", raster "),
        Span::styled(
            format!("{:.1} ms", raster_ms),
            Style::default().fg(COLOR_RASTER),
        ),
        Span::raw("."),
    ]);
    buf.set_line(area.x, area.y, &line, area.width);
}

// ── Header and verdict ────────────────────────────────────────────────────────

fn render_header(area: Rect, buf: &mut Buffer, frame: &FrameTiming) {
    let line = Line::from(vec![
        Span::styled(
            "Flutter frame: ",
            Style::default().fg(palette::TEXT_SECONDARY),
        ),
        Span::styled(
            format!("{}", frame.number),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    buf.set_line(area.x, area.y, &line, area.width);
}

fn render_verdict(area: Rect, buf: &mut Buffer, frame: &FrameTiming, refresh_rate_hz: f64) {
    let total_ms = frame.elapsed_ms();
    let budget_ms = 1000.0 / refresh_rate_hz;
    let line = if total_ms > budget_ms {
        let excess = total_ms - budget_ms;
        Line::from(vec![
            Span::styled(
                format!("Total: {total_ms:.1} ms"),
                Style::default().fg(Color::White),
            ),
            Span::raw("  "),
            Span::styled(
                format!("Budget @ {:.0} Hz: {budget_ms:.1} ms", refresh_rate_hz),
                Style::default().fg(palette::TEXT_SECONDARY),
            ),
            Span::raw("  "),
            Span::styled(
                format!("— JANK +{excess:.1} ms"),
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
        ])
    } else {
        Line::from(vec![
            Span::styled(
                format!("Total: {total_ms:.1} ms"),
                Style::default().fg(Color::White),
            ),
            Span::raw("  "),
            Span::styled(
                format!("Budget @ {:.0} Hz: {budget_ms:.1} ms", refresh_rate_hz),
                Style::default().fg(palette::TEXT_SECONDARY),
            ),
            Span::raw("  "),
            Span::styled("OK", Style::default().fg(Color::Green)),
        ])
    };
    buf.set_line(area.x, area.y, &line, area.width);
}

// ── Hints ─────────────────────────────────────────────────────────────────────

fn render_hints(area: Rect, buf: &mut Buffer, frame: &FrameTiming, refresh_rate_hz: f64) {
    if area.height == 0 {
        return;
    }
    let hints = frame_hints(frame, refresh_rate_hz);
    if hints.is_empty() {
        let line = Line::from(Span::styled(
            "No issues detected for this frame.",
            Style::default().fg(palette::TEXT_MUTED),
        ));
        buf.set_line(area.x, area.y, &line, area.width);
        return;
    }
    // "Hints:" header + bullet lines, capped at MAX_HINT_LINES.
    buf.set_line(area.x, area.y, &Line::from("Hints:"), area.width);
    let max = (area.height as usize)
        .saturating_sub(1)
        .min(MAX_HINT_LINES)
        .min(hints.len());
    for (i, hint) in hints.iter().take(max).enumerate() {
        let y = area.y + 1 + i as u16;
        if y >= area.y + area.height {
            break;
        }
        let line = Line::from(vec![
            Span::styled("  • ", Style::default().fg(palette::TEXT_MUTED)),
            Span::raw(hint.message()),
        ]);
        buf.set_line(area.x, y, &line, area.width);
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use fdemon_app::session::PerformanceState;
    use fdemon_core::performance::{FramePhases, FrameTiming};
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    fn collect(buf: &Buffer) -> String {
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

    fn make_frame(
        elapsed_us: u64,
        build_us: u64,
        raster_us: u64,
        phases: Option<FramePhases>,
    ) -> FrameTiming {
        FrameTiming {
            number: 42,
            build_micros: build_us,
            raster_micros: raster_us,
            elapsed_micros: elapsed_us,
            timestamp: chrono::Local::now(),
            phases,
            shader_compilation: false,
        }
    }

    fn perf_with_frame(frame: FrameTiming, refresh_rate: f64) -> PerformanceState {
        let mut perf = PerformanceState::default();
        perf.frame_history.push(frame);
        perf.selected_frame = Some(0);
        perf.display_refresh_rate = refresh_rate;
        perf
    }

    // ── No-selection prompt ──────────────────────────────────────────────────

    #[test]
    fn renders_no_selection_prompt_when_unselected() {
        let perf = PerformanceState::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 10));
        render(buf.area, &mut buf, &perf);
        assert!(
            collect(&buf).contains("Select a frame above"),
            "expected no-selection prompt"
        );
    }

    // ── Verdict: jank ────────────────────────────────────────────────────────

    #[test]
    fn renders_jank_verdict_when_over_budget() {
        let f = make_frame(24_000, 10_000, 10_000, None);
        let perf = perf_with_frame(f, 60.0);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 20));
        render(buf.area, &mut buf, &perf);
        let text = collect(&buf);
        assert!(text.contains("Total: 24.0 ms"), "missing total: {text}");
        assert!(
            text.contains("Budget @ 60 Hz: 16.7 ms"),
            "missing budget: {text}"
        );
        assert!(text.contains("JANK"), "missing JANK: {text}");
    }

    // ── Verdict: OK ──────────────────────────────────────────────────────────

    #[test]
    fn renders_ok_verdict_when_within_budget() {
        let f = make_frame(10_000, 4_000, 4_000, None);
        let perf = perf_with_frame(f, 60.0);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 20));
        render(buf.area, &mut buf, &perf);
        assert!(collect(&buf).contains("OK"), "missing OK verdict");
    }

    // ── 120 Hz budget ────────────────────────────────────────────────────────

    #[test]
    fn hz_120_budget_shows_8_3ms() {
        let f = make_frame(10_000, 4_000, 4_000, None);
        let perf = perf_with_frame(f, 120.0);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 20));
        render(buf.area, &mut buf, &perf);
        let text = collect(&buf);
        assert!(
            text.contains("Budget @ 120 Hz: 8.3 ms"),
            "missing 120 Hz budget: {text}"
        );
        assert!(
            text.contains("JANK"),
            "10ms frame is janky at 120 Hz: {text}"
        );
    }

    // ── Proportional phase bar ───────────────────────────────────────────────

    #[test]
    fn renders_proportional_bar_when_phases_and_wide_enough() {
        // Use more even distribution so all 4 phases get enough label space.
        // build=8000, layout=6000, paint=5000, raster=9000 → total=28000
        //   build:  8/28 * 80 ≈ 22 cols — "Build 8.0ms"  (11 chars) fits
        //   layout: 6/28 * 80 ≈ 17 cols — "Layout 6.0ms" (12 chars) fits
        //   paint:  5/28 * 80 ≈ 14 cols — "Paint 5.0ms"  (11 chars) fits
        //   raster: 9/28 * 80 ≈ 27 cols — "Raster 9.0ms" (12 chars) fits
        let phases = FramePhases {
            build_micros: 8_000,
            layout_micros: 6_000,
            paint_micros: 5_000,
            raster_micros: 9_000,
            shader_compilation: false,
        };
        let f = make_frame(28_000, 19_000, 9_000, Some(phases));
        let perf = perf_with_frame(f, 60.0);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 20));
        render(buf.area, &mut buf, &perf);
        let text = collect(&buf);
        // Phase labels appear somewhere in the rendered output.
        assert!(text.contains("Build"), "missing Build label: {text}");
        assert!(text.contains("Layout"), "missing Layout label: {text}");
        assert!(text.contains("Paint"), "missing Paint label: {text}");
        assert!(text.contains("Raster"), "missing Raster label: {text}");
    }

    // ── Inline phase summary (narrow terminal) ───────────────────────────────

    #[test]
    fn renders_inline_phase_summary_below_width_threshold() {
        let phases = FramePhases {
            build_micros: 6_000,
            layout_micros: 2_000,
            paint_micros: 3_000,
            raster_micros: 7_000,
            shader_compilation: false,
        };
        let f = make_frame(18_000, 11_000, 7_000, Some(phases));
        let perf = perf_with_frame(f, 60.0);
        // Use exactly MIN_PHASE_BAR_WIDTH - 1 (39 wide) to trigger the inline path.
        // The full line "B 6.0ms | L 2.0ms | P 3.0ms | R 7.0ms" is 40 chars, which
        // just barely overflows a 39-wide buffer — check for the first 3 segments.
        // Use a taller buffer so the phase bar area is actually rendered.
        let mut buf = Buffer::empty(Rect::new(0, 0, 39, 20)); // < MIN_PHASE_BAR_WIDTH (40)
        render(buf.area, &mut buf, &perf);
        let text = collect(&buf);
        assert!(text.contains("B 6.0ms"), "missing B: {text}");
        assert!(text.contains("L 2.0ms"), "missing L: {text}");
        assert!(text.contains("P 3.0ms"), "missing P: {text}");
        // "R 7.0ms" starts at col 30, which fits within 39 cols.
        assert!(text.contains("R 7.0ms"), "missing R: {text}");
    }

    // ── Aggregate split (no phases) ──────────────────────────────────────────

    #[test]
    fn renders_aggregate_split_when_no_phases() {
        let f = make_frame(10_000, 4_000, 6_000, None);
        let perf = perf_with_frame(f, 60.0);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 20));
        render(buf.area, &mut buf, &perf);
        let text = collect(&buf);
        assert!(
            text.contains("Aggregate") || (text.contains("build") && text.contains("raster")),
            "missing aggregate split: {text}"
        );
    }

    // ── No issues ────────────────────────────────────────────────────────────

    #[test]
    fn shows_no_issues_when_balanced_in_budget() {
        let f = make_frame(10_000, 4_000, 4_000, None);
        let perf = perf_with_frame(f, 60.0);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 20));
        render(buf.area, &mut buf, &perf);
        let text = collect(&buf);
        assert!(
            text.contains("No issues detected"),
            "missing no-issues message: {text}"
        );
    }

    // ── Hints rendered ───────────────────────────────────────────────────────

    #[test]
    fn renders_hints_when_janky() {
        let mut f = make_frame(24_000, 10_000, 10_000, None);
        f.shader_compilation = true;
        let perf = perf_with_frame(f, 60.0);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 20));
        render(buf.area, &mut buf, &perf);
        let text = collect(&buf);
        assert!(text.contains("Hints:"), "missing Hints: header: {text}");
        assert!(
            text.matches('\u{2022}').count() >= 1,
            "missing bullet points: {text}"
        );
    }

    // ── Zero-area guard ──────────────────────────────────────────────────────

    #[test]
    fn no_panic_on_zero_area() {
        let perf = PerformanceState::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        render(buf.area, &mut buf, &perf); // must not panic
    }

    #[test]
    fn no_panic_single_row() {
        let f = make_frame(10_000, 4_000, 4_000, None);
        let perf = perf_with_frame(f, 60.0);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 1));
        render(buf.area, &mut buf, &perf); // must not panic
    }

    // ── Header content ───────────────────────────────────────────────────────

    #[test]
    fn renders_frame_number_in_header() {
        let mut f = make_frame(10_000, 4_000, 4_000, None);
        f.number = 1337;
        let perf = perf_with_frame(f, 60.0);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 20));
        render(buf.area, &mut buf, &perf);
        let text = collect(&buf);
        assert!(text.contains("1337"), "frame number not rendered: {text}");
        assert!(
            text.contains("Flutter frame"),
            "header prefix missing: {text}"
        );
    }
}
