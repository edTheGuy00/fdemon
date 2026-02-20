## Task: Implement Memory Chart Widget

**Objective**: Create a new `MemoryChart` widget that replaces the existing gauge in the Performance panel. The widget renders a time-series chart using Unicode braille characters showing stacked memory layers (Dart heap, native, raster cache) with line overlays (allocated, RSS), GC event markers, a legend, and a class allocation table below.

**Depends on**: Task 01 (core types), Task 02 (PerformanceState with memory_samples)

### Scope

- `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart.rs`: **NEW** file

### Details

#### Widget API

```rust
/// Time-series memory chart with stacked area layers, GC markers, and allocation table.
pub(crate) struct MemoryChart<'a> {
    memory_samples: &'a RingBuffer<MemorySample>,
    memory_history: &'a RingBuffer<MemoryUsage>,  // fallback if no samples
    gc_history: &'a RingBuffer<GcEvent>,
    allocation_profile: Option<&'a AllocationProfile>,
    icons: bool,
}
```

#### Layout

The widget area is split into two vertical sections:

```
┌─ Memory ──────────────────────────────────────┐
│ Legend: ■ Dart Heap  ■ Native  ■ Raster       │
│                                               │
│ 128MB ┤                     ╭──── RSS         │
│       │              ╭──────╯                 │
│  64MB ┤       ╭──────╯                        │
│       │ ╭─────╯                               │
│  32MB ┤─╯                                     │
│       └───────────────────────────────────────│
│        60s ago              30s        now     │
├───────────────────────────────────────────────┤
│ Class              Instances   Size            │
│ _String            12,345      2.4 MB          │
│ _List              8,901       1.8 MB          │
│ _Map               5,678       1.2 MB          │
│ ...                                           │
└───────────────────────────────────────────────┘
```

Split ratio:
- **Chart area**: 60% of total height (minimum 6 rows)
- **Allocation table**: 40% of total height (minimum 3 rows)
- If total height < 10: show chart only, no table
- If total height < 6: show single-line memory summary (fallback to gauge-like display)

#### Braille Canvas

Implement a minimal braille canvas helper for high-resolution plotting:

```rust
/// A simple braille-based plotting canvas.
///
/// Each character cell is a 2x4 grid of dots (Unicode braille, U+2800–U+28FF).
/// This gives 2x horizontal and 4x vertical sub-character resolution.
///
/// Coordinates are in "dot space": x ranges 0..width*2, y ranges 0..height*4.
struct BrailleCanvas {
    cells: Vec<Vec<u16>>,  // cells[row][col] = braille dot pattern offset
    width: usize,          // character columns
    height: usize,         // character rows
}

impl BrailleCanvas {
    fn new(width: usize, height: usize) -> Self { ... }

    /// Set a dot at (x, y) in dot-space coordinates.
    /// x: 0..width*2, y: 0..height*4
    fn set(&mut self, x: usize, y: usize) { ... }

    /// Render the canvas into a ratatui Buffer at the given position.
    /// Each cell gets the specified color. Multiple overlapping series
    /// use the last-set color for the cell.
    fn render_to_buffer(&self, buf: &mut Buffer, area: Rect, color: Color) { ... }
}
```

Braille dot mapping (standard Unicode braille order):
```
Dot 1 (0x01) | Dot 4 (0x08)
Dot 2 (0x02) | Dot 5 (0x10)
Dot 3 (0x04) | Dot 6 (0x20)
Dot 7 (0x40) | Dot 8 (0x80)
```

For coordinate `(x, y)`: column = `x / 2`, row = `y / 4`, bit = braille_bit_map[y % 4][x % 2].

#### Chart rendering

**Data mapping**:
- X-axis: time (0 = oldest sample, N = most recent). Map sample index to dot-space x coordinates.
- Y-axis: memory in bytes, auto-scaled. Map byte values to dot-space y coordinates (0 = top, max = bottom in screen coords, but invert for chart).

**Stacked area layers** (bottom to top):
1. **Dart Heap** (`dart_heap`) — Cyan
2. **Native** (`dart_native`) — Blue
3. **Raster Cache** (`raster_cache`) — Magenta

Each layer is additive: the Native layer starts where Heap ends, Raster starts where Native ends. Fill using braille dots for the area between the layer bottom and top.

**Line overlays**:
- **Allocated** (`allocated`) — Yellow, dashed appearance (draw every other dot)
- **RSS** (`rss`) — White/Gray, solid line (skip if all values are 0)

**GC event markers**: For each GC event whose timestamp falls within the chart's time range, draw a small marker (`▼` or `•`) at the x position on the bottom axis. Color: Yellow for major GC.

**Y-axis labels**: Show 2–3 labels on the left edge (0, mid, max) in human-readable format (KB/MB/GB). Use `MemoryUsage::format_bytes()` utility.

**X-axis labels**: Show "60s" at left edge and "now" at right edge (or similar relative time indicators).

#### Legend row

Single line at the top of the chart area:

```
■ Heap  ■ Native  ■ Raster  ─ Allocated  ─ RSS  ▼ GC
```

Colors match the chart layers. If `raster_cache` is all 0, omit "Raster" from legend. If `rss` is all 0, omit "RSS".

#### Fallback: use `MemoryUsage` when no `MemorySample` data

If `memory_samples` is empty but `memory_history` (existing `RingBuffer<MemoryUsage>`) has data, render a simplified chart:
- Single area layer: Heap usage (Cyan)
- Single line: Heap capacity (Yellow dashed)
- No native/raster/RSS layers
- Same overall layout and interaction

#### Class allocation table

Below the chart, render a simple table:

```
Class               Instances    Shallow Size
─────────────────────────────────────────────
dart:core/String    12,345       2.4 MB
dart:core/_List     8,901        1.8 MB
dart:core/_Map      5,678        1.2 MB
...
```

- Use `AllocationProfile::top_by_size(10)` to get the top 10 classes
- Columns: Class name (truncated), total instances (`total_instances()`), total size (`total_size()` formatted)
- If `allocation_profile` is `None`, show: "Allocation data loading..."
- Format sizes using `MemoryUsage::format_bytes()`

#### Constants

```rust
const LEGEND_HEIGHT: u16 = 1;
const MIN_CHART_HEIGHT: u16 = 6;
const MIN_TABLE_HEIGHT: u16 = 3;
const TABLE_HEADER_HEIGHT: u16 = 2; // header + separator
const MAX_TABLE_ROWS: usize = 10;
const CHART_PROPORTION: f64 = 0.6;  // 60% chart, 40% table
```

### Acceptance Criteria

1. `MemoryChart` widget renders without panic for all states: empty samples, single sample, full buffer
2. Braille canvas correctly maps coordinates to Unicode braille characters
3. Stacked area layers show Dart Heap (Cyan), Native (Blue), Raster Cache (Magenta)
4. Allocated capacity shown as Yellow line overlay
5. RSS shown as White/Gray line (omitted when all zeros)
6. GC event markers drawn at correct x positions
7. Legend row shows active layers with matching colors
8. Y-axis auto-scales with human-readable labels (KB/MB/GB)
9. Fallback mode works with `MemoryUsage` data when `MemorySample` buffer is empty
10. Class allocation table shows top 10 classes sorted by size
11. Table handles `None` allocation profile gracefully
12. Compact mode for small areas (chart only, or summary only)
13. 15+ unit tests covering rendering and braille canvas logic

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // --- BrailleCanvas tests ---

    #[test]
    fn test_braille_canvas_single_dot() {
        let mut canvas = BrailleCanvas::new(1, 1);
        canvas.set(0, 0); // top-left dot
        // Should produce braille char U+2801 (dot 1)
    }

    #[test]
    fn test_braille_canvas_all_dots() {
        let mut canvas = BrailleCanvas::new(1, 1);
        for y in 0..4 { for x in 0..2 { canvas.set(x, y); } }
        // Should produce U+28FF (all 8 dots)
    }

    #[test]
    fn test_braille_canvas_out_of_bounds_ignored() {
        let mut canvas = BrailleCanvas::new(2, 2);
        canvas.set(100, 100); // should not panic
    }

    #[test]
    fn test_braille_canvas_multi_cell() {
        let mut canvas = BrailleCanvas::new(3, 2);
        canvas.set(0, 0);
        canvas.set(5, 7); // cell (2, 1)
    }

    // --- Chart rendering tests ---

    #[test]
    fn test_renders_empty_samples_without_panic() {
        let samples = RingBuffer::new(120);
        let memory_history = RingBuffer::new(60);
        let gc_history = RingBuffer::new(50);
        let widget = MemoryChart::new(&samples, &memory_history, &gc_history, None, false);
        let area = Rect::new(0, 0, 80, 20);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);
    }

    #[test]
    fn test_renders_with_memory_usage_fallback() {
        // Empty memory_samples, populated memory_history
    }

    #[test]
    fn test_legend_omits_raster_when_zero() { ... }

    #[test]
    fn test_legend_omits_rss_when_zero() { ... }

    #[test]
    fn test_gc_markers_positioned_correctly() { ... }

    #[test]
    fn test_allocation_table_shows_top_classes() { ... }

    #[test]
    fn test_allocation_table_none_profile() { ... }

    #[test]
    fn test_compact_mode_small_height() {
        let area = Rect::new(0, 0, 80, 5); // too small for table
        // Should show chart only
    }

    #[test]
    fn test_very_small_area_no_panic() {
        let area = Rect::new(0, 0, 10, 3);
        // Should show fallback summary
    }

    #[test]
    fn test_y_axis_auto_scaling() {
        // Samples with max 50MB should scale y-axis to ~50MB
    }

    #[test]
    fn test_zero_area_no_panic() {
        let area = Rect::new(0, 0, 0, 0);
        // Should not panic
    }
}
```

### Notes

- **Braille canvas is inline**: The `BrailleCanvas` is a private struct within `memory_chart.rs`. It is NOT a separate file or public utility. If it proves useful elsewhere later, it can be extracted.
- **No module wiring yet**: Like Task 05, this task creates a standalone file. Wiring into `performance/mod.rs` happens in Task 07.
- **Stacked area simplification**: True filled area charts in braille are complex. A pragmatic approach: for each x column, fill dots from the bottom up to each layer's height. This gives an approximate stacked area appearance without needing flood-fill algorithms.
- **Color limitation**: Braille characters are single-colored per cell. When multiple layers overlap in the same cell, the topmost layer's color wins. This is acceptable for the stacked layout since layers are additive (no overlap).
- **Performance**: Braille rendering is O(samples * height) per frame. With 120 samples and ~20 rows of height, this is ~2400 operations — negligible for a 60fps TUI.
- **File size target**: ~400–500 lines including braille canvas, chart rendering, table, and tests. The braille canvas is ~80 lines, the chart ~200, the table ~80, tests ~150.

---

## Completion Summary

**Status:** Not started
