## Task: Implement Frame Bar Chart Widget

**Objective**: Create a new `FrameChart` widget that replaces the existing sparkline in the Performance panel. The bar chart shows each frame as a pair of vertical bars (UI + Raster), with color coding for jank and shader compilation, a 16ms budget line, frame selection, and a detail panel showing breakdown info for the selected frame.

**Depends on**: Task 01 (core types), Task 02 (PerformanceState with selected_frame)

### Scope

- `crates/fdemon-tui/src/widgets/devtools/performance/frame_chart.rs`: **NEW** file

### Details

#### Widget API

```rust
/// Frame timing bar chart with selectable frames.
///
/// Renders each frame as a pair of vertical bars (UI thread + Raster thread)
/// with color coding for jank/shader compilation, a 16ms budget line,
/// and a detail panel below for the selected frame.
pub(crate) struct FrameChart<'a> {
    frame_history: &'a RingBuffer<FrameTiming>,
    selected_frame: Option<usize>,
    stats: &'a PerformanceStats,
    icons: bool,
}
```

Constructor and `Widget` implementation:

```rust
impl<'a> FrameChart<'a> {
    pub fn new(
        frame_history: &'a RingBuffer<FrameTiming>,
        selected_frame: Option<usize>,
        stats: &'a PerformanceStats,
        icons: bool,
    ) -> Self {
        Self { frame_history, selected_frame, stats, icons }
    }
}

impl Widget for FrameChart<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) { ... }
}
```

#### Bar chart rendering

The chart area is split into two regions:
1. **Bar chart** (top, `area.height - DETAIL_PANEL_HEIGHT`): frame bars
2. **Detail panel** (bottom, `DETAIL_PANEL_HEIGHT = 3` lines): selected frame info or summary

**Bar layout** (per frame):
- Each frame occupies 3 columns: `[UI bar][Raster bar][gap]`
- Bars are rendered using half-block characters (`▄`, `▀`, `█`) for sub-character vertical resolution
- Bar height is proportional to frame time, auto-scaled to fit available chart height
- Maximum frames visible = `(chart_width) / 3`
- Show the most recent N frames that fit; scroll with selection if needed

**Scaling**:
- Y-axis auto-scales based on the max frame time in visible frames
- Minimum y-range: 20ms (to prevent flat charts for fast apps)
- Scale increments: round up to nearest 10ms boundary

**Color scheme**:

| Condition | UI Bar | Raster Bar |
|-----------|--------|------------|
| Normal (< 16ms total) | Cyan | Green |
| Jank (> 16ms total) | Red | Red |
| Shader compilation | Magenta | Magenta |

**Budget line**: Horizontal dashed line at the 16ms mark (using `─` or `╌` character). Color: `DarkGray`. Label: `"16ms"` at the left edge.

**Selection highlight**: The selected frame pair has a `White` background or underline character. Use `▔` above the bars or a distinct bottom highlight.

#### Half-block bar rendering

Use Unicode half-block characters for 2x vertical resolution:

```rust
/// Render a vertical bar using half-block characters.
///
/// With half-blocks, each character row represents 2 height units:
/// - Full block `█` = both halves filled
/// - Upper half `▀` = top half only
/// - Lower half `▄` = bottom half only
/// - Space ` ` = empty
fn render_bar(buf: &mut Buffer, x: u16, bottom_y: u16, height_half_blocks: u16, color: Color) {
    let full_rows = height_half_blocks / 2;
    let has_half = height_half_blocks % 2 == 1;

    for row in 0..full_rows {
        let y = bottom_y - row;
        buf.set_string(x, y, "█", Style::default().fg(color));
    }

    if has_half {
        let y = bottom_y - full_rows;
        buf.set_string(x, y, "▄", Style::default().fg(color));
    }
}
```

#### Detail panel

When a frame IS selected (3-line panel):

```
Frame #1234  Total: 18.2ms (JANK)
UI: 12.2ms  (Build: 6.1ms  Layout: 2.3ms  Paint: 3.8ms)
Raster: 6.0ms
```

When phases are not available:

```
Frame #1234  Total: 18.2ms (JANK)
UI: 12.2ms   Raster: 6.0ms
```

When no frame is selected (1-line summary):

```
FPS: 60  Avg: 8.2ms  Jank: 2 (1.3%)  Shader: 0
```

**Color coding in detail**:
- "JANK" label: Red + Bold
- "SHADER" label: Magenta + Bold
- Phase times: dim/gray
- Frame number: White + Bold

#### Constants

```rust
const DETAIL_PANEL_HEIGHT: u16 = 3;
const CHARS_PER_FRAME: u16 = 3;  // UI bar + Raster bar + gap
const MIN_CHART_HEIGHT: u16 = 4;  // minimum bar chart area height
const MIN_Y_RANGE_MS: f64 = 20.0; // minimum y-axis range
const BUDGET_LINE_MS: f64 = 16.667;
```

### Acceptance Criteria

1. `FrameChart` widget renders without panic for all states: empty history, single frame, full buffer
2. Each frame shown as two vertical bars (UI=Cyan, Raster=Green)
3. Jank frames (>16ms) colored Red
4. Shader compilation frames colored Magenta
5. 16ms budget line drawn as dashed horizontal line with "16ms" label
6. Selected frame visually distinguished (highlight/underline)
7. Detail panel shows frame breakdown when selected
8. Summary line shows FPS/Avg/Jank/Shader when no frame selected
9. Half-block characters used for sub-character vertical resolution
10. Auto-scaling y-axis with minimum 20ms range
11. Shows most recent frames that fit in available width
12. Compact mode: if `area.height < MIN_CHART_HEIGHT + DETAIL_PANEL_HEIGHT`, show summary only
13. 15+ unit tests covering rendering edge cases

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    fn make_frame(number: u64, build: u64, raster: u64) -> FrameTiming {
        FrameTiming {
            number,
            build_micros: build,
            raster_micros: raster,
            elapsed_micros: build + raster,
            timestamp: chrono::Local::now(),
            phases: None,
            shader_compilation: false,
        }
    }

    fn make_janky_frame(number: u64) -> FrameTiming {
        make_frame(number, 12_000, 8_000) // 20ms total > 16ms budget
    }

    fn make_shader_frame(number: u64) -> FrameTiming {
        let mut f = make_frame(number, 5_000, 30_000);
        f.shader_compilation = true;
        f
    }

    #[test]
    fn test_renders_empty_history_without_panic() {
        let history = RingBuffer::new(100);
        let stats = PerformanceStats::default();
        let widget = FrameChart::new(&history, None, &stats, false);
        let area = Rect::new(0, 0, 80, 20);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
    }

    #[test]
    fn test_renders_single_frame() { ... }

    #[test]
    fn test_jank_frame_uses_red_color() { ... }

    #[test]
    fn test_shader_frame_uses_magenta() { ... }

    #[test]
    fn test_budget_line_drawn() {
        // Verify the "16ms" label appears in the buffer
    }

    #[test]
    fn test_selected_frame_highlighted() { ... }

    #[test]
    fn test_detail_panel_shows_frame_info() {
        // When selected_frame is Some, verify frame number and timing appear
    }

    #[test]
    fn test_summary_line_when_no_selection() {
        // When selected_frame is None, verify FPS/Avg/Jank summary
    }

    #[test]
    fn test_compact_mode_for_small_area() {
        let area = Rect::new(0, 0, 80, 3); // too small for chart
        // Should show summary only, no panic
    }

    #[test]
    fn test_zero_area_no_panic() {
        let area = Rect::new(0, 0, 0, 0);
        // Should not panic
    }

    #[test]
    fn test_frame_count_fits_width() {
        // With width=30, should show 10 frames max (30/3)
    }

    #[test]
    fn test_auto_scaling_minimum_range() {
        // All frames < 5ms, y-axis should still be at least 20ms
    }

    #[test]
    fn test_detail_panel_with_phases() {
        let mut frame = make_frame(42, 6_000, 6_000);
        frame.phases = Some(FramePhases {
            build_micros: 3_000,
            layout_micros: 1_500,
            paint_micros: 1_500,
            raster_micros: 6_000,
            shader_compilation: false,
        });
        // Verify phase breakdown appears in detail panel
    }

    #[test]
    fn test_detail_panel_jank_label() {
        // Janky frame should show "(JANK)" in detail panel
    }

    #[test]
    fn test_many_frames_shows_most_recent() {
        // Push 100 frames, width fits 10: should show frames 91-100
    }
}
```

### Notes

- **No module wiring yet**: This task creates `frame_chart.rs` as a standalone file. It is NOT yet referenced from `performance/mod.rs` — that wiring happens in Task 07.
- **Half-block vs braille**: The frame bar chart uses half-block characters (`▀▄█`), not braille. Braille is for the memory chart's line plots. Half-blocks give clean vertical bars.
- **No horizontal scrolling**: The chart always shows the most recent frames. If the user selects a frame and new frames arrive, the view scrolls to keep the selected frame visible. If the selected frame scrolls out of the visible window, implement one of: (a) auto-deselect, or (b) scroll to keep selected frame at the left edge. Prefer (b).
- **Style consistency**: Use the existing `styles.rs` helpers (`fps_style`, `jank_style`) where applicable. Add new color constants to `styles.rs` if needed.
- **File size target**: ~300–400 lines including tests. If it grows beyond 500, consider extracting `render_bar` and scaling math into a `bar_helpers.rs`.

---

## Completion Summary

**Status:** Not started
