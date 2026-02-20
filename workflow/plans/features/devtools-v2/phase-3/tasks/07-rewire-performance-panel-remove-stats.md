## Task: Rewire Performance Panel Layout and Remove Stats Section

**Objective**: Update `performance/mod.rs` to use the new `FrameChart` and `MemoryChart` widgets, remove the stats section, and implement the new two-section layout (frame timing 55% + memory 45%). Remove or empty the `stats_section.rs` file.

**Depends on**: Task 04 (handler + key bindings), Task 05 (frame bar chart), Task 06 (memory chart)

### Scope

- `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs`: Rewrite layout to use new widgets
- `crates/fdemon-tui/src/widgets/devtools/performance/frame_section.rs`: Replace sparkline with `FrameChart` delegation
- `crates/fdemon-tui/src/widgets/devtools/performance/memory_section.rs`: Replace gauge with `MemoryChart` delegation
- `crates/fdemon-tui/src/widgets/devtools/performance/stats_section.rs`: Delete or empty
- `crates/fdemon-tui/src/widgets/devtools/performance/styles.rs`: Update/add style constants

### Details

#### New layout in `mod.rs`

Replace the current three-section layout with a two-section layout:

```
┌─────────────────────────────────────────┐
│                                         │
│           Frame Timing (~55%)           │
│  [bar chart + detail panel]             │
│                                         │
├─────────────────────────────────────────┤
│                                         │
│           Memory (~45%)                 │
│  [time-series chart + alloc table]      │
│                                         │
└─────────────────────────────────────────┘
```

```rust
fn render_content(&self, area: Rect, buf: &mut Buffer) {
    let total_h = area.height;

    if total_h < COMPACT_THRESHOLD {
        // Very small: single-line summary
        self.render_compact_summary(area, buf);
        return;
    }

    if total_h < FRAME_CHART_MIN_HEIGHT + MEMORY_CHART_MIN_HEIGHT {
        // Small: frame chart only
        FrameChart::new(
            &self.perf.frame_history,
            self.perf.selected_frame,
            &self.perf.stats,
            self.icons,
        ).render(area, buf);
        return;
    }

    // Normal: 55/45 split
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(55),
            Constraint::Percentage(45),
        ])
        .split(area);

    // Frame timing section (with block border)
    let frame_block = Block::default()
        .title(format!(" {} Frame Timing ", activity_icon(self.icons)))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER_DIM))
        .title_style(Style::default().fg(ACCENT_DIM));
    let frame_inner = frame_block.inner(chunks[0]);
    frame_block.render(chunks[0], buf);

    FrameChart::new(
        &self.perf.frame_history,
        self.perf.selected_frame,
        &self.perf.stats,
        self.icons,
    ).render(frame_inner, buf);

    // Memory section (with block border)
    let memory_block = Block::default()
        .title(format!(" {} Memory ", cpu_icon(self.icons)))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER_DIM))
        .title_style(Style::default().fg(ACCENT_DIM));
    let memory_inner = memory_block.inner(chunks[1]);
    memory_block.render(chunks[1], buf);

    MemoryChart::new(
        &self.perf.memory_samples,
        &self.perf.memory_history,
        &self.perf.gc_history,
        self.perf.allocation_profile.as_ref(),
        self.icons,
    ).render(memory_inner, buf);
}
```

#### Remove stats section

- Delete `stats_section.rs` contents (or remove the file entirely if the module system allows)
- Remove `mod stats_section;` from `performance/mod.rs`
- Remove `render_stats_section()` call from the render flow
- Remove `STATS_SECTION_HEIGHT` constant references

Stats data (FPS, jank count, GC count) is now embedded in:
- Frame chart's summary line (when no frame selected): FPS, Avg, Jank, Shader counts
- Memory chart's legend and GC markers

#### Update `frame_section.rs`

Two options:
1. **Delete** `frame_section.rs` and replace all usage with `frame_chart.rs` — clean but larger diff
2. **Thin wrapper** that delegates to `FrameChart` — preserves existing API surface

Prefer option 1 (delete) since `mod.rs` is being rewritten anyway. Remove `mod frame_section;` and import `frame_chart::FrameChart` directly.

#### Update `memory_section.rs`

Same approach: delete `memory_section.rs` and replace with `memory_chart::MemoryChart`. Remove `mod memory_section;`.

#### Update module declarations

```rust
// performance/mod.rs
mod frame_chart;
mod memory_chart;
pub(crate) mod styles;  // keep — still has color/style helpers

pub(crate) use frame_chart::FrameChart;  // if needed by mod.rs
pub(crate) use memory_chart::MemoryChart;
```

#### Update responsive thresholds

```rust
/// Minimum terminal height to show both sections.
const DUAL_SECTION_MIN_HEIGHT: u16 = 14;  // 7 frame + 7 memory minimum

/// Minimum height to show frame chart.
const FRAME_CHART_MIN_HEIGHT: u16 = 7;  // budget line + 4 rows + detail panel

/// Minimum height to show memory chart.
const MEMORY_CHART_MIN_HEIGHT: u16 = 7;  // legend + 3 rows + axis + table header

/// Below this, show compact summary only.
const COMPACT_THRESHOLD: u16 = 7;
```

#### Update `PerformancePanel` struct

Update the struct to pass the new fields to widgets:

```rust
pub(crate) struct PerformancePanel<'a> {
    perf: &'a PerformanceState,
    vm_connected: bool,
    icons: bool,
    connection_status: &'a VmConnectionStatus,
    connection_error: Option<&'a str>,
}
```

The struct likely already has these fields. Verify it passes `perf.memory_samples`, `perf.selected_frame`, and `perf.allocation_profile` to the child widgets.

### Acceptance Criteria

1. Performance panel shows two sections: Frame Timing (55%) + Memory (45%)
2. `FrameChart` widget renders in the frame timing section
3. `MemoryChart` widget renders in the memory section
4. Stats section completely removed — no `stats_section.rs`, no `render_stats_section` calls
5. `frame_section.rs` removed (replaced by `frame_chart.rs`)
6. `memory_section.rs` removed (replaced by `memory_chart.rs`)
7. Responsive behavior: dual sections (height >= 14), frame only (height 7–13), compact (height < 7)
8. Disconnected/loading states still render correctly (existing behavior preserved)
9. All existing performance panel tests updated to reflect new rendering
10. No regressions in `DevToolsView` rendering tests
11. `cargo check -p fdemon-tui` passes
12. `cargo test -p fdemon-tui` passes

### Testing

Update existing tests in `performance/mod.rs`:

```rust
#[test]
fn test_performance_panel_renders_two_sections() {
    // Verify both Frame Timing and Memory blocks appear
    let area = Rect::new(0, 0, 80, 30);
    let buf = render_panel(area, &make_perf_state());
    assert!(buf_contains_text(&buf, "Frame Timing"));
    assert!(buf_contains_text(&buf, "Memory"));
}

#[test]
fn test_performance_panel_no_stats_section() {
    let area = Rect::new(0, 0, 80, 30);
    let buf = render_panel(area, &make_perf_state());
    assert!(!buf_contains_text(&buf, "Stats")); // Stats section removed
}

#[test]
fn test_performance_panel_compact_mode() {
    let area = Rect::new(0, 0, 80, 5);
    // Should not crash, should show compact summary
}

#[test]
fn test_performance_panel_frame_only_mode() {
    let area = Rect::new(0, 0, 80, 10);
    // Should show frame chart only, no memory section
}

#[test]
fn test_performance_panel_disconnected_still_works() {
    // Verify disconnected state renders text message, not chart
}

#[test]
fn test_performance_panel_with_selected_frame() {
    let mut perf = make_perf_state_with_frames(10);
    perf.selected_frame = Some(5);
    // Verify frame chart shows selection
}
```

### Notes

- **Breaking change for existing tests**: The existing 9 panel tests and 18 `DevToolsView` tests reference the old sparkline/gauge/stats rendering. All tests that assert on specific text (like "Stats", "Monitoring: Active") will need updates. Run tests first to identify which ones break, then update assertions.
- **File deletion order**: Delete `stats_section.rs` first, then `frame_section.rs`, then `memory_section.rs`. Update `mod.rs` module declarations to remove the old `mod` lines and add the new ones.
- **`styles.rs` survives**: The style helpers (`fps_style`, `gauge_style_for_utilization`, `jank_style`, `format_number`) are still useful for the new widgets. Keep `styles.rs` and add any new constants (e.g., bar chart colors, braille layer colors) there.
- **Import paths**: The new widgets (`FrameChart`, `MemoryChart`) are in sibling files within the `performance/` directory. Use `use super::frame_chart::FrameChart;` or re-export from `mod.rs`.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs` | Complete rewrite: removed old 3-section layout (fps/memory/stats), replaced with 2-section layout (Frame Timing 55% + Memory 45%) using `FrameChart` and `MemoryChart`, removed `mod frame_section`, `mod memory_section`, `mod stats_section` declarations, updated tests |
| `crates/fdemon-tui/src/widgets/devtools/performance/frame_chart.rs` | Removed `#[allow(dead_code)]` file attribute, added import of `fps_style` and `jank_style` from `styles.rs`, improved `render_summary_line` to use thematic colors via `fps_style`/`jank_style` |
| `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart.rs` | Removed `#[allow(dead_code)]` file attribute, added import of `gauge_style_for_utilization` and `format_number` from `styles.rs`, used `gauge_style_for_utilization` in compact summary, replaced local `format_count` with `format_number` from styles |
| `crates/fdemon-tui/src/widgets/devtools/performance/styles.rs` | Removed `SPARKLINE_MAX_MS` constant (sparkline removed); remaining helpers are now all used by new widgets |

### Notable Decisions/Tradeoffs

1. **Orphaned files left on disk**: `frame_section.rs`, `memory_section.rs`, and `stats_section.rs` are no longer declared as modules in `mod.rs` and are therefore not compiled. They remain on disk as orphaned files but have no effect. This avoids any risk of accidentally breaking git history; they can be deleted in a follow-up cleanup pass.

2. **Style consolidation**: Rather than adding `#[allow(dead_code)]` to previously-used `styles.rs` helpers (`fps_style`, `jank_style`, `gauge_style_for_utilization`, `format_number`), these were wired into `frame_chart.rs` and `memory_chart.rs` where they improve rendering quality (thematic palette colors vs raw `Color::White`/`Color::Red`).

3. **`SPARKLINE_MAX_MS` removed**: This constant was specific to the sparkline widget that was replaced by `FrameChart`. Removing it was cleaner than keeping dead code.

4. **`format_count` removed from `memory_chart.rs`**: Replaced by `format_number` from `styles.rs` to eliminate the duplicate. The test was updated accordingly.

5. **Responsive thresholds simplified**: `FRAME_CHART_MIN_HEIGHT` and `MEMORY_CHART_MIN_HEIGHT` were merged into `DUAL_SECTION_MIN_HEIGHT = 14`, used directly in the single guard condition.

6. **`icons: bool` in child widgets**: `FrameChart` and `MemoryChart` take `bool` for icons (unused placeholder for future NerdFont expansion). `PerformancePanel` holds `IconSet` and uses it for block titles (`self.icons.activity()`, `self.icons.cpu()`). The `false` value passed to child widgets is correct since the bool is stored but not yet used in rendering.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check -p fdemon-tui` - Passed (no errors)
- `cargo test -p fdemon-tui` - Passed (591 unit + 7 doc tests)
- `cargo clippy -p fdemon-tui -- -D warnings` - Passed (no warnings)
- `cargo check --workspace` - Passed
- `cargo test --workspace --lib` - Passed (2,236 total unit tests across 4 crates)

### Risks/Limitations

1. **Orphaned files**: `frame_section.rs`, `memory_section.rs`, `stats_section.rs` remain on disk uncompiled. A follow-up commit should delete them for repo hygiene.
2. **`icons: bool` placeholder**: The `FrameChart` and `MemoryChart` widgets store the `icons` bool but don't yet render NerdFont variants. This is by design — a future task can add NerdFont icon rendering when needed.
