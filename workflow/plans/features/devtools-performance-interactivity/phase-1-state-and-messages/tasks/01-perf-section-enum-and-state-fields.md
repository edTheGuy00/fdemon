## Task: `PerfSection` Enum + Performance State Fields

**Objective**: Add the `PerfSection` enum and the scroll/focus/render-hint fields on `PerformanceState`. Bump the frame-history ring buffer size so scroll-back is meaningful.

**Depends on**: None

**Estimated Time**: 1-2 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/session/performance.rs`:
  - Add `pub enum PerfSection { FrameChart, MemoryChart, MemoryList }` with `Default = FrameChart` and `Clone, Copy, Debug, PartialEq, Eq` derives.
  - Add helper methods `next() -> Self` and `prev() -> Self` for `Tab`/`Shift+Tab` cycling.
  - Add fields to `PerformanceState`:
    ```rust
    pub focused_section: PerfSection,                      // default: FrameChart
    pub frame_chart_scroll_offset: usize,                  // default: 0 (live edge)
    pub memory_chart_scroll_offset: usize,                 // default: 0
    pub alloc_table_selected_row: Option<usize>,           // default: None
    pub alloc_table_scroll_offset: usize,                  // default: 0
    pub frame_chart_visible_width: Cell<usize>,            // render-hint; EXCEPTION-annotated
    pub memory_chart_visible_width: Cell<usize>,           // render-hint; EXCEPTION-annotated
    pub alloc_table_visible_height: Cell<usize>,           // render-hint; EXCEPTION-annotated
    ```
  - Annotate each Cell field with `// EXCEPTION (TEA): render-hint Cell — see docs/CODE_STANDARDS.md "Region Registry Pattern" and Principle 3.`
  - Update `Default` impl and any constructors / `with_*` helpers.
  - Bump `DEFAULT_FRAME_HISTORY_SIZE` from `300` to `1800` with a doc comment: `/// 30 seconds at 60 FPS — enables meaningful scroll-back.`

**Files Read (Dependencies):**
- `crates/fdemon-app/src/session/performance.rs`: Existing struct shape.
- `crates/fdemon-app/src/state.rs`: For existing `Cell<usize>` exception precedent.

### Details

```rust
use std::cell::Cell;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PerfSection {
    #[default]
    FrameChart,
    MemoryChart,
    MemoryList,
}

impl PerfSection {
    pub fn next(self) -> Self {
        match self {
            PerfSection::FrameChart => PerfSection::MemoryChart,
            PerfSection::MemoryChart => PerfSection::MemoryList,
            PerfSection::MemoryList => PerfSection::FrameChart,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            PerfSection::FrameChart => PerfSection::MemoryList,
            PerfSection::MemoryChart => PerfSection::FrameChart,
            PerfSection::MemoryList => PerfSection::MemoryChart,
        }
    }
}
```

`PerformanceState` derives `Clone` per the research findings — `Cell<usize>` is `Clone`-able (value-copy), so no special handling required.

### Acceptance Criteria

1. `PerfSection` exists with 3 variants + `next`/`prev` helpers.
2. `PerformanceState` has 5 new behavioral fields + 3 Cell render-hint fields, each with `// EXCEPTION` annotations matching the project standard.
3. `DEFAULT_FRAME_HISTORY_SIZE = 1800` with a doc comment.
4. `Default` impl initializes new fields to sensible defaults (focused = FrameChart, all offsets = 0, selected_row = None, Cells = `Cell::new(0)`).
5. Unit tests cover `PerfSection::next`/`prev` cycling.
6. `cargo check --workspace --all-targets` and `cargo test --workspace` pass.

### Testing

```rust
#[test]
fn perf_section_next_cycles_forward() {
    assert_eq!(PerfSection::FrameChart.next(), PerfSection::MemoryChart);
    assert_eq!(PerfSection::MemoryChart.next(), PerfSection::MemoryList);
    assert_eq!(PerfSection::MemoryList.next(), PerfSection::FrameChart);
}

#[test]
fn perf_section_prev_cycles_backward() {
    assert_eq!(PerfSection::FrameChart.prev(), PerfSection::MemoryList);
}

#[test]
fn performance_state_defaults() {
    let s = PerformanceState::default();
    assert_eq!(s.focused_section, PerfSection::FrameChart);
    assert_eq!(s.frame_chart_scroll_offset, 0);
    assert_eq!(s.alloc_table_selected_row, None);
    assert_eq!(s.frame_chart_visible_width.get(), 0);
}
```

### Notes

- Don't touch widgets or handlers in this task — they're Phase 2 / Phase 3.
- `PerformanceState`'s `with_memory_history_size(...)` constructor (if it exists) needs the new fields wired through.
- The frame-history bump may break tests that assume `len() == 300`; fix those alongside this task.
