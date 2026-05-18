## Task: Frame Analysis Tab Content — Proportional Phase Bar + Hints

**Objective**: Replace the T04 stub in `widgets/devtools/performance/details/frame_analysis_tab.rs` with the populated Phase 2 content: frame number header, total/budget verdict line, proportional 4-segment phase bar (build / layout / paint / raster) when `FramePhases` data is available, hint list driven by T01's `frame_hints()`, and graceful no-data / no-selection / narrow-terminal fallbacks. Also trim `frame_chart/detail.rs` — the per-frame summary moves here.

**Depends on**: 01 (frame_hints + FrameHint), 04 (frame_analysis_tab.rs file exists; layout constants `MIN_PHASE_BAR_WIDTH` defined)

**Estimated Time**: 3–4 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/devtools/performance/details/frame_analysis_tab.rs` — replace the T04 stub with the full Frame Analysis renderer described below.
- `crates/fdemon-tui/src/widgets/devtools/performance/frame_chart/detail.rs` — **trim**: the per-frame `render_frame_detail` body (lines 35–85) is now duplicated by `frame_analysis_tab.rs` when the dual-pane is visible. The `frame_chart/detail.rs` path is still used by the chart-only fallback (`area.height < MIN_DUAL_PANE_HEIGHT`) — keep the existing implementation there so the short-terminal experience is unchanged. **Net edit**: only the per-frame `Frame #N  Total: …` summary stays in `frame_chart/detail.rs` for the chart-only path; do not delete it. (See "Notes" for the layered-fallback rationale.) Add a doc-comment cross-reference to `frame_analysis_tab.rs` so future readers know the dual-pane code path lives elsewhere.

**Files Read (Dependencies):**
- `crates/fdemon-core/src/{performance, frame_hints}.rs` (T01: `FrameTiming`, `FramePhases`, `frame_hints()`, `FrameHint`).
- `crates/fdemon-app/src/session/performance.rs` (T02: `PerformanceState`, `display_refresh_rate`).
- `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs` (T04: `MIN_PHASE_BAR_WIDTH`).
- `crates/fdemon-tui/src/widgets/devtools/performance/frame_chart/detail.rs` — reference for the existing per-frame text formatting style.

### Details

#### Renderer structure

```rust
//! Frame Analysis tab content — Phase 2.
//!
//! Renders the selected frame's phase breakdown, total / budget verdict, and
//! a diagnostic hint list. When no frame is selected, shows a prompt. When the
//! selected frame has no `FramePhases` data, falls back to the aggregate
//! build+raster split. When width is below [`MIN_PHASE_BAR_WIDTH`], the
//! proportional bar degrades to an inline `B/L/P/R` summary line.

use fdemon_app::session::PerformanceState;
use fdemon_core::frame_hints::{frame_hints, FrameHint, FramePhaseKind};
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

pub(super) fn render(area: Rect, buf: &mut Buffer, performance: &PerformanceState) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    match performance.selected_frame_timing() {
        Some(frame) => render_selected(area, buf, frame, performance.display_refresh_rate),
        None => render_no_selection(area, buf),
    }
}

fn render_no_selection(area: Rect, buf: &mut Buffer) { /* centered prompt */ }

fn render_selected(area: Rect, buf: &mut Buffer, frame: &FrameTiming, refresh_rate_hz: f64) {
    // Layout: header line (1) + verdict line (1) + spacer (1) + phase bar (1 or 3) + hints (≤ 5)
    let chunks = Layout::vertical([
        Constraint::Length(1), // header
        Constraint::Length(1), // verdict
        Constraint::Length(1), // spacer
        Constraint::Length(phase_bar_rows(area.width, frame)),
        Constraint::Min(0),    // hints
    ])
    .split(area);

    render_header(chunks[0], buf, frame);
    render_verdict(chunks[1], buf, frame, refresh_rate_hz);
    render_phase_bar(chunks[3], buf, frame);
    render_hints(chunks[4], buf, frame, refresh_rate_hz);
}
```

#### Phase bar — two render paths

```rust
/// Choose phase-bar height based on available width.
/// - 3 rows: proportional 4-segment bar (label row + 1 bar row + spacer).
/// - 1 row: inline `B 6.1ms | L 2.0ms | P 3.4ms | R 6.7ms` fallback.
fn phase_bar_rows(width: u16, frame: &FrameTiming) -> u16 {
    if frame.phases.is_none() { return 1; } // aggregate-only fallback also uses 1 row
    if width < MIN_PHASE_BAR_WIDTH { 1 } else { 3 }
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
fn render_proportional_phase_bar(area: Rect, buf: &mut Buffer, phases: &FramePhases) {
    let total = phases.total_micros().max(1);
    let cols = area.width as u64;

    // Compute cell counts; ensure they sum to area.width (handle rounding by giving
    // the remainder to raster — the longest segment in practice).
    let build_cells   = (phases.build_micros  * cols / total) as u16;
    let layout_cells  = (phases.layout_micros * cols / total) as u16;
    let paint_cells   = (phases.paint_micros  * cols / total) as u16;
    let raster_cells  = (area.width).saturating_sub(build_cells + layout_cells + paint_cells);

    // Row 0: label line — show "Build 6.1ms" / "Layout 2.0ms" / "Paint 3.4ms" / "Raster 6.7ms"
    //   each centred within its segment if width allows; otherwise just the first letter.
    // Row 1: █ block characters in each segment's colour.
    // Row 2: spacer (blank).
}

/// Inline `B 6.1ms | L 2.0ms | P 3.4ms | R 6.7ms` rendered on one row.
fn render_inline_phase_summary(area: Rect, buf: &mut Buffer, phases: &FramePhases) {
    let line = Line::from(vec![
        Span::styled(format!("B {:.1}ms", phases.build_micros  as f64 / 1000.0), Style::default().fg(COLOR_BUILD)),
        Span::raw(" | "),
        Span::styled(format!("L {:.1}ms", phases.layout_micros as f64 / 1000.0), Style::default().fg(COLOR_LAYOUT)),
        Span::raw(" | "),
        Span::styled(format!("P {:.1}ms", phases.paint_micros  as f64 / 1000.0), Style::default().fg(COLOR_PAINT)),
        Span::raw(" | "),
        Span::styled(format!("R {:.1}ms", phases.raster_micros as f64 / 1000.0), Style::default().fg(COLOR_RASTER)),
    ]);
    buf.set_line(area.x, area.y, &line, area.width);
}

/// `Phase data not available. Aggregate: build 4.2 ms, raster 6.7 ms.`
fn render_aggregate_split(area: Rect, buf: &mut Buffer, frame: &FrameTiming) { ... }
```

#### Header and verdict

```rust
fn render_header(area: Rect, buf: &mut Buffer, frame: &FrameTiming) {
    let line = Line::from(vec![
        Span::styled("Flutter frame: ", Style::default().fg(palette::TEXT_SECONDARY)),
        Span::styled(
            format!("{}", frame.number),
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
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
            Span::styled(format!("Total: {total_ms:.1} ms"), Style::default().fg(Color::White)),
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
            Span::styled(format!("Total: {total_ms:.1} ms"), Style::default().fg(Color::White)),
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
```

#### Hints

```rust
fn render_hints(area: Rect, buf: &mut Buffer, frame: &FrameTiming, refresh_rate_hz: f64) {
    if area.height == 0 { return; }
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
    let max = (area.height as usize).saturating_sub(1).min(MAX_HINT_LINES).min(hints.len());
    for (i, hint) in hints.iter().take(max).enumerate() {
        let y = area.y + 1 + i as u16;
        let line = Line::from(vec![
            Span::styled("  • ", Style::default().fg(palette::TEXT_MUTED)),
            Span::raw(hint.message()),
        ]);
        buf.set_line(area.x, y, &line, area.width);
    }
}
```

#### No-selection prompt

```rust
fn render_no_selection(area: Rect, buf: &mut Buffer) {
    let message = "Select a frame above (←/→) to view analysis.";
    let p = Paragraph::new(message)
        .style(Style::default().fg(palette::TEXT_MUTED))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });
    let y_offset = area.height.saturating_sub(1) / 2;
    let centered = Rect { y: area.y + y_offset, height: 1, ..area };
    p.render(centered, buf);
}
```

#### `frame_chart/detail.rs` trim

Add a doc-comment at the top of `render_detail_panel`:

```rust
/// Render the 3-line detail panel below the chart.
///
/// **Used only in the chart-only fallback** (`area.height < MIN_DUAL_PANE_HEIGHT`).
/// The dual-pane Performance layout renders frame details inside the Details
/// pane via [`super::super::details::frame_analysis_tab`] — that path supersedes
/// this one when the terminal is tall enough.
pub(super) fn render_detail_panel(...) { ... }
```

No structural changes to `detail.rs` body — the path is still live for short terminals.

### Acceptance Criteria

1. **Frame selected with `phases = Some(...)` at width ≥ 40**: Frame Analysis tab shows (top-to-bottom):
   - `Flutter frame: <number>` header (bold white number).
   - `Total: 18.2 ms  Budget @ 60 Hz: 16.7 ms  — JANK +1.5 ms` verdict (red bold when over budget) OR `... OK` (green) when within budget.
   - Blank spacer row.
   - Proportional 4-segment phase bar (3 rows: labels, █ row, spacer).
   - `Hints:` heading + up to 5 bullet lines.
2. **Frame selected with `phases = None`**: instead of the proportional bar, an `Aggregate: build 4.2 ms, raster 6.7 ms.` line is shown. The hint list still renders.
3. **Frame selected at width < `MIN_PHASE_BAR_WIDTH (40)`**: the proportional bar degrades to a single-line `B 6.1ms | L 2.0ms | P 3.4ms | R 6.7ms` summary. Header, verdict, and hints still render.
4. **No frame selected**: the tab shows the centered prompt `"Select a frame above (←/→) to view analysis."` and nothing else.
5. **No hints (balanced in-budget frame)**: the hints section shows `"No issues detected for this frame."` in muted text — not a blank.
6. **Hint list ordering**: when both `OverBudget` and `ShaderCompilation` apply, OverBudget appears first (matches T01's salience order).
7. **120 Hz refresh rate handling**: setting `performance.display_refresh_rate = 120.0` and selecting a 12 ms frame produces the JANK verdict with `Budget @ 120 Hz: 8.3 ms` — exercises the wiring from state through `frame_hints()`.
8. **Chart-only fallback unchanged**: `cargo test --workspace` continues to pass the existing chart-only path tests in `frame_chart/tests.rs` and `widgets/devtools/performance/tests.rs::compact_mode_*` (or equivalents).
9. **No clippy warnings**: `cargo clippy --workspace --all-targets -- -D warnings` clean.

### Testing

Inline `#[cfg(test)] mod tests` in `frame_analysis_tab.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use fdemon_core::performance::{FramePhases, FrameTiming};
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    fn collect(buf: &Buffer) -> String { /* … */ }

    fn frame(elapsed_us: u64, build_us: u64, raster_us: u64, phases: Option<FramePhases>) -> FrameTiming { /* … */ }

    fn perf_with_frame(frame: FrameTiming, refresh_rate: f64) -> PerformanceState {
        let mut perf = PerformanceState::default();
        perf.frame_history.push(frame);
        perf.selected_frame = Some(0);
        perf.display_refresh_rate = refresh_rate;
        perf
    }

    #[test]
    fn renders_no_selection_prompt_when_unselected() {
        let perf = PerformanceState::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 10));
        render(buf.area, &mut buf, &perf);
        assert!(collect(&buf).contains("Select a frame above"));
    }

    #[test]
    fn renders_jank_verdict_when_over_budget() {
        let f = frame(24_000, 10_000, 10_000, None);
        let perf = perf_with_frame(f, 60.0);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 20));
        render(buf.area, &mut buf, &perf);
        let text = collect(&buf);
        assert!(text.contains("Total: 24.0 ms"));
        assert!(text.contains("Budget @ 60 Hz: 16.7 ms"));
        assert!(text.contains("JANK"));
    }

    #[test]
    fn renders_ok_verdict_when_within_budget() {
        let f = frame(10_000, 4_000, 4_000, None);
        let perf = perf_with_frame(f, 60.0);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 20));
        render(buf.area, &mut buf, &perf);
        assert!(collect(&buf).contains("OK"));
    }

    #[test]
    fn 120hz_budget_shows_8_3ms() {
        let f = frame(10_000, 4_000, 4_000, None);
        let perf = perf_with_frame(f, 120.0);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 20));
        render(buf.area, &mut buf, &perf);
        let text = collect(&buf);
        assert!(text.contains("Budget @ 120 Hz: 8.3 ms"));
        assert!(text.contains("JANK"), "10ms frame is janky at 120 Hz");
    }

    #[test]
    fn renders_proportional_bar_when_phases_and_wide_enough() {
        let phases = FramePhases { build_micros: 6_000, layout_micros: 2_000, paint_micros: 3_000, raster_micros: 7_000, shader_compilation: false };
        let f = frame(18_000, 11_000, 7_000, Some(phases));
        let perf = perf_with_frame(f, 60.0);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 20));
        render(buf.area, &mut buf, &perf);
        let text = collect(&buf);
        // Phase labels appear somewhere in the rendered output.
        assert!(text.contains("Build"));
        assert!(text.contains("Layout"));
        assert!(text.contains("Paint"));
        assert!(text.contains("Raster"));
    }

    #[test]
    fn renders_inline_phase_summary_below_width_threshold() {
        let phases = FramePhases { build_micros: 6_000, layout_micros: 2_000, paint_micros: 3_000, raster_micros: 7_000, shader_compilation: false };
        let f = frame(18_000, 11_000, 7_000, Some(phases));
        let perf = perf_with_frame(f, 60.0);
        let mut buf = Buffer::empty(Rect::new(0, 0, 36, 20)); // < MIN_PHASE_BAR_WIDTH
        render(buf.area, &mut buf, &perf);
        let text = collect(&buf);
        assert!(text.contains("B 6.0ms"));
        assert!(text.contains("L 2.0ms"));
        assert!(text.contains("P 3.0ms"));
        assert!(text.contains("R 7.0ms"));
    }

    #[test]
    fn renders_aggregate_split_when_no_phases() {
        let f = frame(10_000, 4_000, 6_000, None);
        let perf = perf_with_frame(f, 60.0);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 20));
        render(buf.area, &mut buf, &perf);
        let text = collect(&buf);
        assert!(text.contains("Aggregate") || (text.contains("build") && text.contains("raster")));
    }

    #[test]
    fn shows_no_issues_when_balanced_in_budget() {
        let f = frame(10_000, 4_000, 4_000, None);
        let perf = perf_with_frame(f, 60.0);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 20));
        render(buf.area, &mut buf, &perf);
        let text = collect(&buf);
        assert!(text.contains("No issues detected"));
    }

    #[test]
    fn renders_hints_when_janky() {
        let mut f = frame(24_000, 10_000, 10_000, None);
        f.shader_compilation = true;
        let perf = perf_with_frame(f, 60.0);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 20));
        render(buf.area, &mut buf, &perf);
        let text = collect(&buf);
        assert!(text.contains("Hints:"));
        assert!(text.matches('•').count() >= 1);
    }
}
```

### Notes

- **Why keep `frame_chart/detail.rs` body intact?** The chart-only fallback (when `usable.height < MIN_DUAL_PANE_HEIGHT`) still uses `frame_chart`'s built-in detail panel — that panel is part of the `FrameChart` widget contract, not the dual-pane layout. Removing it would break the small-terminal path. The dual-pane path uses `frame_analysis_tab.rs` instead. Two slightly-different renderers is acceptable because the dual-pane path can use more real estate (proportional bar, hints) than the 3-line chart-only detail panel.
- **Cell colours for phase segments**: `COLOR_BUILD = Cyan`, `COLOR_LAYOUT = LightBlue`, `COLOR_PAINT = Magenta`, `COLOR_RASTER = Green`. These match the existing `frame_chart` UI/raster colours and add two new shades for layout/paint. If `palette` exports semantically named constants, prefer those (`palette::ACCENT_*`) over raw `Color::*`.
- **Proportional bar rounding**: integer division produces a final segment width = `area.width - sum(other_three)`. Give the remainder to raster (the practical longest segment) so the bar always fills the row exactly.
- **Hint message length cap**: T01 guarantees ≤ 80 chars; the renderer can clip to `area.width` defensively but should not truncate at less than 80 unless area is smaller.
- **`PerformanceStats` is NOT consumed** here — the tab is per-frame. The chart's no-selection summary (FPS / Avg / Jank / Shader) lives in `frame_chart/detail.rs`'s `render_summary_line` and stays put.
- **No mouse regions** in this task.
- **Performance / TUI cost**: hint generation runs every frame the Performance panel is rendered, but only when a frame is selected. The work is `O(1)` per frame (≤ 5 hints). No memoization needed for Phase 2.

---

## Completion Summary

(Filled in by implementor after work completes.)
