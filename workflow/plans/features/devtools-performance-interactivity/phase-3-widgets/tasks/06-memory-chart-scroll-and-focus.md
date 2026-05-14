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

---

## Completion Summary

**Status:** Done
**Branch:** fix/devtools-improvements

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart/mod.rs` | Added `chart_scroll_offset`, `chart_focused`, `chart_visible_width_cell` fields to `MemoryChart`; added `with_chart_state()` builder; updated `render_impl` and `render_chart_area` to forward new params; re-exported `visible_memory_window` for tests |
| `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart/chart.rs` | Added `visible_memory_window()` helper (Model A slicing); updated `render_sample_chart` to accept `scroll_offset` and `visible_width_cell`, write Cell each frame, and render from windowed slice; added `#[allow(clippy::too_many_arguments)]` |
| `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart/tests.rs` | Added 5 unit tests for `visible_memory_window`: at-offset, live-edge, fewer-samples, offset-beyond-len, exact-fit |
| `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs` | Forwarded `memory_chart_scroll_offset`, `memory_focused`, `memory_chart_visible_width` into `MemoryChart`; applied `memory_border_color` from `COLOR_FOCUSED/UNFOCUSED_BORDER`; registered `PerfFocusSection(MemoryChart)` section click region at z=0; updated doc comments |

### Notable Decisions/Tradeoffs

1. **`visible_memory_window` operates on `&[MemorySample]`**: The test spec required this signature. In `render_sample_chart` the `RingBuffer` is collected into `Vec<MemorySample>` via `.cloned()` (≤120 items, cheap). Avoids double-indirection and matches the public API the tests need.
2. **Focus border uses `Borders::TOP` only for memory section**: The memory section already uses `Borders::TOP` only (to maximize inner height). The focus color is applied to that top border, consistent with the block style for this section.
3. **`#[allow(clippy::too_many_arguments)]` on `render_sample_chart`**: The function reached 8 args after adding scroll_offset and visible_width_cell. A struct refactor would be premature; the args are already well-named inline helpers.
4. **y-axis scaling uses visible window only**: `max_bytes` is computed over the `sample_data` (windowed) slice rather than the full ring buffer. This gives a better zoom when scrolled back into lower-memory history.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo test --workspace` - Passed (1037 fdemon-tui tests; 0 failures)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed

### Risks/Limitations

1. **No scroll clamping at the widget layer**: `scroll_offset` is passed in from `PerformanceState` unchanged; if the handler exceeds `memory_samples.len()`, the window will be empty (graceful, but no data shown). Clamping is the handler's responsibility (Phase 2, task 03/04).
2. **GC markers use windowed timestamp range**: GC events outside the visible window's timestamp range are filtered. This is correct for the scroll-back view.
