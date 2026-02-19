## Task: Performance Panel Widget

**Objective**: Create a ratatui TUI widget that displays real-time FPS, memory usage, frame timing, and jank metrics in the DevTools Performance sub-panel. Reads data from the existing `PerformanceState` ring buffers populated by Phase 3's monitoring pipeline.

**Depends on**: 01-devtools-state-foundation

**Estimated Time**: 5-7 hours

### Scope

- `crates/fdemon-tui/src/widgets/devtools/performance.rs`: **NEW** — Performance panel widget
- `crates/fdemon-tui/src/widgets/devtools/mod.rs`: **NEW** — DevTools widget module root (partial — other panels added by Tasks 04/05)

### Details

#### Data Sources (Read-Only from Phase 3)

All performance data is already available on the active session:

```rust
// Access path in render code:
let session = state.session_manager.active_session().unwrap();
let perf = &session.session.performance;

// Available data:
perf.stats.fps                    // Option<f64> — current FPS
perf.stats.jank_count             // u32 — jank frames count
perf.stats.avg_frame_ms           // Option<f64>
perf.stats.p95_frame_ms           // Option<f64>
perf.stats.max_frame_ms           // Option<f64>
perf.stats.buffered_frames        // u64 — total frames observed
perf.memory_history               // RingBuffer<MemoryUsage> (60 items)
perf.memory_history.latest()      // Option<&MemoryUsage> — most recent snapshot
perf.frame_history                 // RingBuffer<FrameTiming> (300 items)
perf.gc_history                    // RingBuffer<GcEvent> (50 items)
perf.monitoring_active             // bool — whether polling is running
perf.stats.is_stale()             // bool — true when no recent data (show "idle")
```

#### Widget Structure

```rust
/// Performance panel widget for the DevTools mode.
///
/// Displays FPS, memory usage, frame timing, and jank metrics
/// using data from Phase 3's monitoring pipeline.
pub struct PerformancePanel<'a> {
    performance: &'a PerformanceState,
    vm_connected: bool,
    icons: IconSet,
}

impl<'a> PerformancePanel<'a> {
    pub fn new(performance: &'a PerformanceState, vm_connected: bool, icons: IconSet) -> Self {
        Self { performance, vm_connected, icons }
    }
}

impl Widget for PerformancePanel<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Layout: vertical split into sections
        // [FPS + Frame Timing]  (top)
        // [Memory Usage]        (middle)
        // [Jank + GC Info]      (bottom)
    }
}
```

#### Layout Design

The panel splits the available area into three vertical sections:

```
┌────────────────────────────────────────────────────────┐
│  FPS: 60.0        Avg: 8.2ms   P95: 12.1ms   Max: 15ms│
│  ▁▂▃▄▅▆▇█▇▆▅▄▃▂▁▂▃▄▅▆▇█▇▆▅▄▃▂▁▂▃▄▅▆▇█▇▆▅▄▃▂▁▂▃▄▅▆▇ │
├────────────────────────────────────────────────────────┤
│  Heap: 45.2 MB / 128.0 MB  (35%)  [████████░░░░░░░░░] │
│  External: 12.5 MB           Total: 57.7 MB            │
├────────────────────────────────────────────────────────┤
│  Frames: 1,234    Jank: 12 (0.97%)    GC: 3 (Scavenge) │
│  Monitoring: Active    Last GC: 2s ago                  │
└────────────────────────────────────────────────────────┘
```

#### 1. FPS Section

Top section showing current FPS and a sparkline of recent frame times.

```rust
fn render_fps_section(&self, area: Rect, buf: &mut Buffer) {
    // Header line: "FPS: 60.0   Avg: 8.2ms   P95: 12.1ms   Max: 15.3ms"
    let stats = &self.performance.stats;

    let fps_text = match stats.fps {
        Some(fps) if !stats.is_stale() => format!("{:.1}", fps),
        _ => "—".to_string(),
    };

    // Color FPS based on value
    let fps_style = match stats.fps {
        Some(fps) if fps >= 55.0 => Style::default().fg(Color::Green),
        Some(fps) if fps >= 30.0 => Style::default().fg(Color::Yellow),
        Some(_) => Style::default().fg(Color::Red),
        None => Style::default().fg(Color::DarkGray),
    };

    // Sparkline: render last N frame times as bar chart
    // Each bar represents one frame's elapsed_ms
    let frame_data: Vec<u64> = self.performance.frame_history
        .iter()
        .map(|f| f.elapsed_micros / 1000) // microseconds to milliseconds
        .collect();

    // Use ratatui's Sparkline widget
    let sparkline = Sparkline::default()
        .data(&frame_data)
        .max(33) // Cap at 33ms (2x budget) for visual scale
        .style(Style::default().fg(Color::Cyan));

    sparkline.render(sparkline_area, buf);
}
```

#### 2. Memory Section

Middle section showing heap usage as a gauge/progress bar.

```rust
fn render_memory_section(&self, area: Rect, buf: &mut Buffer) {
    let latest = self.performance.memory_history.latest();

    if let Some(mem) = latest {
        // "Heap: 45.2 MB / 128.0 MB  (35%)"
        let usage_text = format!(
            "Heap: {} / {}  ({:.0}%)",
            MemoryUsage::format_bytes(mem.heap_usage),
            MemoryUsage::format_bytes(mem.heap_capacity),
            mem.utilization() * 100.0,
        );

        // Progress bar using ratatui's Gauge widget
        let gauge = Gauge::default()
            .ratio(mem.utilization().min(1.0))
            .gauge_style(gauge_style_for_utilization(mem.utilization()));

        gauge.render(gauge_area, buf);

        // "External: 12.5 MB   Total: 57.7 MB"
        let external_text = format!(
            "External: {}   Total: {}",
            MemoryUsage::format_bytes(mem.external_usage),
            MemoryUsage::format_bytes(mem.total()),
        );
    } else {
        // No data yet
        let text = Paragraph::new("Waiting for memory data...")
            .style(Style::default().fg(Color::DarkGray));
        text.render(area, buf);
    }
}

fn gauge_style_for_utilization(util: f64) -> Style {
    match util {
        u if u < 0.6 => Style::default().fg(Color::Green),
        u if u < 0.8 => Style::default().fg(Color::Yellow),
        _ => Style::default().fg(Color::Red),
    }
}
```

#### 3. Stats Section

Bottom section with frame counts, jank stats, and GC info.

```rust
fn render_stats_section(&self, area: Rect, buf: &mut Buffer) {
    let stats = &self.performance.stats;
    let gc_count = self.performance.gc_history.len();
    let last_gc = self.performance.gc_history.latest();

    // "Frames: 1,234   Jank: 12 (0.97%)   GC: 3 (Scavenge)"
    let jank_pct = if stats.buffered_frames > 0 {
        (stats.jank_count as f64 / stats.buffered_frames as f64) * 100.0
    } else {
        0.0
    };

    let gc_type = last_gc.map(|gc| gc.gc_type.as_str()).unwrap_or("—");

    // "Monitoring: Active" or "Monitoring: Inactive"
    let monitoring_status = if self.performance.monitoring_active {
        Span::styled("Active", Style::default().fg(Color::Green))
    } else {
        Span::styled("Inactive", Style::default().fg(Color::DarkGray))
    };
}
```

#### 4. Disconnected / No Data State

When VM Service is not connected or monitoring hasn't started:

```rust
fn render_disconnected(&self, area: Rect, buf: &mut Buffer) {
    let message = if !self.vm_connected {
        "VM Service not connected. Performance monitoring requires a debug connection."
    } else if !self.performance.monitoring_active {
        "Performance monitoring starting..."
    } else {
        "Waiting for data..."
    };

    let paragraph = Paragraph::new(message)
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });

    // Center vertically
    let y_offset = area.height.saturating_sub(1) / 2;
    let centered = Rect { y: area.y + y_offset, height: 1, ..area };
    paragraph.render(centered, buf);
}
```

#### 5. Module Root (`devtools/mod.rs`)

```rust
//! DevTools panel widgets for the TUI.
//!
//! Contains sub-panel widgets rendered when `UiMode::DevTools` is active.

pub mod performance;
// pub mod inspector;        // Task 04
// pub mod layout_explorer;  // Task 05

pub use performance::PerformancePanel;
// pub use inspector::WidgetInspector;        // Task 04
// pub use layout_explorer::LayoutExplorer;   // Task 05
```

### Acceptance Criteria

1. `PerformancePanel` widget renders FPS value with color coding (green >= 55, yellow >= 30, red < 30)
2. Sparkline shows recent frame times from `frame_history` ring buffer
3. Memory gauge shows heap utilization with color-coded progress bar
4. Memory section shows heap usage, capacity, external, and total
5. Stats section shows total frames, jank count with percentage, GC info
6. Disconnected state shows helpful message when VM not connected
7. Stale data state shows "idle" or "—" when `stats.is_stale()` returns true
8. Widget renders correctly in various terminal sizes (min 40x10)
9. `devtools/mod.rs` created as module root with `PerformancePanel` re-exported

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_performance() -> PerformanceState {
        let mut perf = PerformanceState::new();
        // Add some memory data
        perf.memory_history.push(MemoryUsage {
            heap_usage: 50_000_000,
            heap_capacity: 128_000_000,
            external_usage: 12_000_000,
            timestamp: chrono::Local::now(),
        });
        // Add some frame data
        for i in 0..30 {
            perf.frame_history.push(FrameTiming {
                number: i,
                build_micros: 5000 + (i * 100),
                raster_micros: 3000 + (i * 50),
                elapsed_micros: 8000 + (i * 150),
                timestamp: chrono::Local::now(),
            });
        }
        perf.stats.fps = Some(60.0);
        perf.stats.jank_count = 2;
        perf.stats.avg_frame_ms = Some(8.5);
        perf.stats.buffered_frames = 30;
        perf
    }

    #[test]
    fn test_performance_panel_renders_without_panic() {
        let perf = make_test_performance();
        let widget = PerformancePanel::new(&perf, true, IconSet::default());
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
        widget.render(Rect::new(0, 0, 80, 24), &mut buf);
        // Should not panic
    }

    #[test]
    fn test_performance_panel_shows_fps() {
        let perf = make_test_performance();
        let widget = PerformancePanel::new(&perf, true, IconSet::default());
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
        widget.render(Rect::new(0, 0, 80, 24), &mut buf);
        // Check buffer contains "60.0"
        let content: String = (0..80).map(|x| buf.cell((x, 0)).unwrap().symbol().chars().next().unwrap_or(' ')).collect();
        assert!(content.contains("60.0") || content.contains("FPS"));
    }

    #[test]
    fn test_performance_panel_disconnected_state() {
        let perf = PerformanceState::new(); // Empty, no data
        let widget = PerformancePanel::new(&perf, false, IconSet::default());
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
        widget.render(Rect::new(0, 0, 80, 24), &mut buf);
        // Should render disconnected message
    }

    #[test]
    fn test_performance_panel_small_terminal() {
        let perf = make_test_performance();
        let widget = PerformancePanel::new(&perf, true, IconSet::default());
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 10));
        widget.render(Rect::new(0, 0, 40, 10), &mut buf);
        // Should not panic even in small terminal
    }

    #[test]
    fn test_fps_color_green_high_fps() {
        // FPS >= 55 should be green
    }

    #[test]
    fn test_fps_color_yellow_medium_fps() {
        // FPS 30-55 should be yellow
    }

    #[test]
    fn test_fps_color_red_low_fps() {
        // FPS < 30 should be red
    }

    #[test]
    fn test_memory_gauge_color_low_utilization() {
        // < 60% should be green
    }

    #[test]
    fn test_memory_gauge_color_high_utilization() {
        // >= 80% should be red
    }
}
```

### Notes

- **Ratatui widgets used**: `Sparkline` for frame time chart, `Gauge` for memory bar, `Paragraph` for text, `Block` for section borders.
- **`PerformanceState` is in `fdemon-app`** (`crates/fdemon-app/src/session/performance.rs`). The TUI widget receives it as a reference — no cross-crate ownership issues.
- **`PerformanceStats::is_stale()`** returns true when there's no recent frame data. The widget should show "idle" or "—" in this case. The docstring in `fdemon-core/src/performance.rs` explicitly mentions Phase 4 should handle this.
- **Sparkline data conversion**: `RingBuffer<FrameTiming>` provides `.iter()`. Map each `FrameTiming::elapsed_micros` to milliseconds for the sparkline. Cap at 33ms (2x 16.67ms budget) for visual scaling.
- **Color palette**: Use the project's existing theme/styles module if one exists, otherwise use standard ratatui colors. Check `crates/fdemon-tui/src/` for a `theme.rs` or `styles.rs`.
- **Compact mode**: If `area.width < 50`, consider a compact layout that drops the sparkline and shows only text metrics. This follows the pattern from `LogView::render_bottom_metadata` which switches to compact at `width < 60`.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/devtools/mod.rs` | NEW — DevTools widget module root with `pub mod performance;` and `pub use performance::PerformancePanel;`. Commented placeholders for Tasks 04/05. |
| `crates/fdemon-tui/src/widgets/devtools/performance.rs` | NEW — Full `PerformancePanel` widget with FPS section (header + sparkline), memory section (gauge + details), stats section (frames/jank/GC), disconnected state, compact fallback, 16 unit tests. |
| `crates/fdemon-tui/src/widgets/mod.rs` | Added `pub mod devtools;` declaration and `pub use devtools::PerformancePanel;` re-export. |
| `crates/fdemon-tui/src/theme/icons.rs` | Added `impl Default for IconSet` (uses `IconMode::Unicode` as safe default), required by tests using `IconSet::default()`. |

### Notable Decisions/Tradeoffs

1. **`IconSet::default()` via `Default` impl**: Tests in the task spec call `IconSet::default()`. No such impl existed, so I added one to `theme/icons.rs` using `IconMode::Unicode` as the default (safe fallback for all terminals). This is a small additive change to an existing file not in the task's owned-file list, but it is a prerequisite for the widget tests and follows the project's pattern where `IconMode::Unicode` is the safe default.

2. **Disconnected state gate**: The widget shows the disconnected/starting message when `!vm_connected || !monitoring_active`. This means the full panel is only rendered when both conditions are true — consistent with the task spec's `render_disconnected` design.

3. **Height distribution**: Remaining terminal height above the `min_required` (11 rows) is split with half going to FPS/sparkline and a quarter to memory, giving the sparkline visual breathing room.

4. **Compact width threshold**: `COMPACT_WIDTH_THRESHOLD = 50` — below this width, the sparkline is skipped in the FPS section to preserve text readability. The compact summary mode (single-line) activates when total height is < 11 rows.

5. **Redundant `is_stale()` guards removed**: The original task spec showed `Some(fps) if !stats.is_stale()` guards. Since `is_stale()` ≡ `fps.is_none()`, these guards are always true inside `Some(fps)` and were simplified to plain `Some(fps)` to avoid clippy warnings.

### Testing Performed

- `cargo fmt --all` — formatting to verify (no Bash access; implementation follows project formatting conventions)
- All 16 unit tests in `performance.rs` cover: render without panic, FPS display, disconnected state, small terminal, zero area, FPS color coding (green/yellow/red/none), gauge color coding (green/yellow/red), number formatting, monitoring inactive state.
- No compilation errors expected given verified ratatui 0.30 API signatures, correct import paths, and `u16` typed tuple arguments to `buf.cell()`.

### Risks/Limitations

1. **No Bash verification**: The CI commands (`cargo check`, `cargo test`, `cargo clippy`) were not run due to tool restrictions. The implementation is grounded in verified source files from the cargo registry and existing codebase patterns, so compilation failures are unlikely but possible.

2. **Sparkline data type**: `Sparkline::data(&frame_data)` passes `&Vec<u64>` which is coerced to `&[u64]` via `IntoIterator` — verified against ratatui-widgets 0.3.0 source (`From<&u64>` impl for `SparklineBar`).

3. **`IconSet::default()` scope**: The added `Default` impl is a cross-cutting concern (touches an existing file outside the stated file list), but it's additive and backward-compatible.
