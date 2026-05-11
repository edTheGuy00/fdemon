## Task: Memory Chart — Scroll Offset, Focus Highlight, Mouse Region

**Objective**: Make the memory time-series chart honor `memory_chart_scroll_offset`, highlight when focused, and register a section-level click region.

**Depends on**: Phase 2; ideally rebases onto 05-frame-chart for the shared `widgets/devtools/performance/mod.rs` changes.

**Estimated Time**: 3-4 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart/mod.rs`:
  - Add `scroll_offset: usize` and `focused: bool` to constructor.
  - Accept `MouseCtx` parameter (was `None` per the performance/mod.rs caller).
  - Register section-level click region → `Message::PerfFocusSection(PerfSection::MemoryChart)`.
- `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart/chart.rs`:
  - In `render_sample_chart`, slice the sample window using `scroll_offset` (anchor `len - offset` like the frame chart).
  - Write `memory_chart_visible_width` Cell each frame with the EXCEPTION annotation.
- `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs`:
  - Forward `memory_chart_scroll_offset` and `focused_section == MemoryChart` into the memory chart widget.
  - Forward `MouseCtx` to memory chart (currently `None` at ~line 215).
  - Apply focus-highlight border style.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/session/performance.rs`: For state shape.
- 05-frame-chart-scroll-and-focus for parallel visible-range logic.

### Details

Use the same visible-range pattern as the frame chart:

```rust
fn visible_memory_window(samples: &[MemorySample], visible_width: usize, scroll_offset: usize) -> &[MemorySample] {
    let end = samples.len().saturating_sub(scroll_offset);
    let start = end.saturating_sub(visible_width);
    &samples[start..end]
}
```

The braille time-series renderer should take the windowed slice and render it across the available width. If the slice is shorter than `visible_width` (e.g., at session start), pad-or-left-align consistently with current behavior.

### Acceptance Criteria

1. Memory chart honors `scroll_offset`; live-edge drift correct (Model A from task 05).
2. `memory_chart_visible_width` Cell written every render with EXCEPTION annotation.
3. Section click registers `PerfFocusSection(MemoryChart)`.
4. Focus highlight applied when focused.
5. Unit tests cover scroll-offset rendering bounds.
6. `cargo test --workspace` and clippy pass.

### Testing

```rust
#[test]
fn memory_chart_window_at_offset() {
    let samples: Vec<MemorySample> = (0..120).map(|i| sample_at(i)).collect();
    let window = visible_memory_window(&samples, 60, 30);
    assert_eq!(window.len(), 60);
    assert_eq!(window.first().unwrap().tick_index, 30);
    assert_eq!(window.last().unwrap().tick_index, 89);
}

#[test]
fn memory_chart_window_at_live_edge() {
    let samples: Vec<MemorySample> = (0..120).map(|i| sample_at(i)).collect();
    let window = visible_memory_window(&samples, 60, 0);
    assert_eq!(window.last().unwrap().tick_index, 119);
}
```

### Notes

- Memory sample ring buffer is 120 × 500 ms = 60 s — useful for scroll-back without enlargement.
- Don't touch the memory allocation table here — that's task 07.
- Be careful not to break the `MemoryChart::render()` caller signature without updating `performance/mod.rs` at the same time.
