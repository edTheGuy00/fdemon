## Task: Extract `MemoryState` — Split `PerformanceState`, Redirect Writers, Update Readers

**Objective**: Move all memory-related fields out of `PerformanceState` into a new sibling `MemoryState` type on `Session`. Redirect every writer (VM Service event handlers, allocation profile handlers, etc.) to the new location, and update every reader (widgets, tests) to follow. After this task the `Memory` placeholder panel from T01 still renders a placeholder, but the data plumbing is fully ready for T03 to attach the real widget. Performance tab continues to render correctly — its memory section now reads from `session.memory.*` instead of `session.performance.memory_*`.

**Depends on**: None (parallel with T01)

**Agent:** implementor

**Estimated Time**: 4–6 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/session/memory.rs` — **NEW.** Contains `MemoryState`, `MemorySection`, `AllocationSortColumn`, memory-related default constants, and the `with_history_size` constructor.
- `crates/fdemon-app/src/session/performance.rs` — Slim down: remove memory fields, simplify `PerfSection` to `{ FrameChart, DetailsTab }`, drop memory-related constants + `AllocationSortColumn`.
- `crates/fdemon-app/src/session/session.rs` — Add `pub memory: MemoryState` field, initialise in `Session::new()`.
- `crates/fdemon-app/src/session/mod.rs` — Add `mod memory;` and re-export `MemoryState`, `MemorySection`, `AllocationSortColumn` from the new module; remove the latter two from the `performance` re-export.
- `crates/fdemon-app/src/update.rs` — Redirect `VmServiceMemorySnapshot` and `VmServiceGcEvent` writes from `session.performance.memory_history` / `gc_history` → `session.memory.memory_history` / `gc_history`.
- `crates/fdemon-app/src/handler/devtools/performance.rs` — Update the memory-side handlers (`handle_memory_sample_received`, `handle_allocation_profile_received`, `handle_toggle_allocation_sort`, `handle_perf_select_alloc_row`, memory branches of `handle_perf_scroll` / `handle_perf_page` / `handle_perf_jump_*`) to write to `session.memory.*` rather than `session.performance.*`. Handlers stay in this file for now; T03 moves them to a new `memory.rs`.
- `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs` — Update the memory section render (lines ≈252–304) to read from `&session.memory` rather than `&session.performance`. The widget still renders the memory section in this task — it will be deleted in T03.
- `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart/mod.rs` — Update `MemoryChart::new` signature: accept references from `MemoryState` rather than `PerformanceState`. Update `with_chart_state` and `with_alloc_state` to take parameters from `MemoryState`.
- `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart/chart.rs` — Update internal calls if it reads `PerformanceState` fields directly (it should already accept its data via the widget constructor — minimal changes expected).
- `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart/table.rs` — Update reads of `PerformanceState.allocation_*` / `alloc_table_*` to take inputs from `MemoryState`. **Do not** change emitted `Message` variants in this task; the table still emits `Message::PerfSelectAllocRow` and `Message::PerfFocusSection(PerfSection::MemoryList)` — those become `Mem*` in T03.
- `crates/fdemon-tui/src/widgets/devtools/performance/tests.rs` — Update memory-related tests to construct `MemoryState` and assert on it; tests that read `state.performance.memory_*` change to `state.memory.*`. **Do not** move tests yet — T03 splits them.
- `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart/tests.rs` — Same: update construction to use `MemoryState`; do not move yet.

**Files Read (Dependencies):**
- `crates/fdemon-core/src/performance.rs` — `MemoryUsage`, `GcEvent`, `MemorySample`, `AllocationProfile`, `RingBuffer<T>` types.

### Details

#### 1. Create `crates/fdemon-app/src/session/memory.rs`

```rust
//! Memory monitoring state — heap usage, GC events, allocation profile.
//!
//! Holds rolling ring-buffer history for memory snapshots, GC events, and
//! rich memory samples, plus the latest allocation profile snapshot and the
//! per-panel sort/selection state.

use std::cell::Cell;

use fdemon_core::performance::{
    AllocationProfile, GcEvent, MemorySample, MemoryUsage, RingBuffer,
};

/// Default number of memory snapshots to keep (at 2s interval = 2 minutes).
pub(crate) const DEFAULT_MEMORY_HISTORY_SIZE: usize = 60;

/// Default number of major GC events to keep.
///
/// Only major GC events (MarkSweep, MarkCompact) are stored — Scavenge events
/// are filtered out in the handler. Major GCs are rare, so 50 slots provides
/// ample history without wasting memory.
pub(crate) const DEFAULT_GC_HISTORY_SIZE: usize = 50;

/// Memory sample buffer size: 120 samples at 500ms polling = 60 seconds of history.
pub(crate) const DEFAULT_MEMORY_SAMPLE_SIZE: usize = 120;

/// Column by which the class allocation table is sorted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AllocationSortColumn {
    #[default]
    BySize,
    ByInstances,
}

/// Active section within the Memory DevTools panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MemorySection {
    /// Memory usage time-series chart (default section on open).
    #[default]
    Chart,
    /// Class allocation table (from `getAllocationProfile`).
    AllocationList,
}

impl MemorySection {
    pub fn next(self) -> Self {
        match self {
            MemorySection::Chart => MemorySection::AllocationList,
            MemorySection::AllocationList => MemorySection::Chart,
        }
    }
    pub fn prev(self) -> Self { self.next() }  // 2-state cycle
}

/// Per-session memory monitoring state.
#[derive(Debug, Clone)]
pub struct MemoryState {
    pub memory_history: RingBuffer<MemoryUsage>,
    pub gc_history: RingBuffer<GcEvent>,
    pub memory_samples: RingBuffer<MemorySample>,
    pub allocation_profile: Option<AllocationProfile>,
    pub allocation_sort: AllocationSortColumn,
    pub monitoring_active: bool,

    pub focused_section: MemorySection,
    pub memory_chart_scroll_offset: usize,
    pub alloc_table_selected_row: Option<usize>,
    pub alloc_table_scroll_offset: usize,

    // EXCEPTION (TEA): render-hint Cell — see docs/CODE_STANDARDS.md Principle 3.
    pub memory_chart_visible_width: Cell<usize>,
    // EXCEPTION (TEA): render-hint Cell — see docs/CODE_STANDARDS.md Principle 3.
    pub alloc_table_visible_height: Cell<usize>,
}

impl Default for MemoryState {
    fn default() -> Self {
        Self {
            memory_history: RingBuffer::new(DEFAULT_MEMORY_HISTORY_SIZE),
            gc_history: RingBuffer::new(DEFAULT_GC_HISTORY_SIZE),
            memory_samples: RingBuffer::new(DEFAULT_MEMORY_SAMPLE_SIZE),
            allocation_profile: None,
            allocation_sort: AllocationSortColumn::default(),
            monitoring_active: false,
            focused_section: MemorySection::default(),
            memory_chart_scroll_offset: 0,
            alloc_table_selected_row: None,
            alloc_table_scroll_offset: 0,
            memory_chart_visible_width: Cell::new(0),
            alloc_table_visible_height: Cell::new(0),
        }
    }
}

impl MemoryState {
    /// Create a `MemoryState` with a configurable memory history size.
    pub fn with_history_size(memory_history_size: usize) -> Self {
        Self {
            memory_history: RingBuffer::new(memory_history_size),
            ..Self::default()
        }
    }
}
```

#### 2. Update `session/performance.rs` — remove memory fields

**Delete** the following fields from `PerformanceState`:
- `memory_history`, `gc_history`, `memory_samples`
- `allocation_profile`, `allocation_sort`
- `memory_chart_scroll_offset`, `memory_chart_visible_width`
- `alloc_table_selected_row`, `alloc_table_scroll_offset`, `alloc_table_visible_height`

**Delete** the constants `DEFAULT_MEMORY_HISTORY_SIZE`, `DEFAULT_GC_HISTORY_SIZE`, `DEFAULT_MEMORY_SAMPLE_SIZE`.

**Delete** the `AllocationSortColumn` enum (it moved to `memory.rs`).

**Update** `PerfSection`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PerfSection {
    #[default]
    FrameChart,
    /// Phase 2 anchor — the tabbed details pane. In Phase 1 cycling Tab to
    /// this section is a no-op (no content yet).
    DetailsTab,
}

impl PerfSection {
    pub fn next(self) -> Self {
        match self {
            PerfSection::FrameChart => PerfSection::DetailsTab,
            PerfSection::DetailsTab => PerfSection::FrameChart,
        }
    }
    pub fn prev(self) -> Self { self.next() }
}
```

**Update** `with_memory_history_size` — rename or delete. The plan calls for the constructor to move to `MemoryState::with_history_size`. The slim `PerformanceState::default()` is sufficient for the frame-only state.

**Surviving fields on `PerformanceState`:**
- `frame_history: RingBuffer<FrameTiming>`
- `stats: PerformanceStats`
- `monitoring_active: bool`
- `selected_frame: Option<usize>`
- `focused_section: PerfSection`
- `frame_chart_scroll_offset: usize`
- `frame_chart_visible_width: Cell<usize>`

**Surviving methods:** `compute_prev_frame_index`, `compute_next_frame_index`, `select_next_frame`, `select_prev_frame`, `deselect_frame`, `selected_frame_timing`, `recompute_stats`, `compute_stats`, `calculate_fps`, `percentile`.

#### 3. Update `session/session.rs`

```rust
// In the Session struct (≈ line 182):
pub performance: PerformanceState,
pub memory: MemoryState,        // NEW

// In Session::new() (≈ line 236):
performance: PerformanceState::default(),
memory: MemoryState::default(), // NEW
```

#### 4. Update `session/mod.rs`

```rust
mod memory;
mod performance;
// ...other mod declarations...

pub use memory::{AllocationSortColumn, MemorySection, MemoryState};
pub use performance::{PerfSection, PerformanceState};
pub(crate) use performance::STATS_RECOMPUTE_INTERVAL;
```

Note `AllocationSortColumn` is now exported from `memory`, not `performance`.

#### 5. Update `crates/fdemon-app/src/update.rs`

Two inline blocks must redirect their writes. The exact line ranges are approximate — search for the message names:

**`VmServiceMemorySnapshot` handler (≈ line 1796–1805):**

```rust
Message::VmServiceMemorySnapshot { session_id, memory } => {
    if let Some(handle) = state.session_manager.find_mut(session_id) {
        // BEFORE: handle.session.performance.memory_history.push(...);
        handle.session.memory.memory_history.push(memory);
    }
    UpdateResult::default()
}
```

**`VmServiceGcEvent` handler (≈ line 1807–1838):**

```rust
Message::VmServiceGcEvent { session_id, gc_event } => {
    if let Some(handle) = state.session_manager.find_mut(session_id) {
        if gc_event.is_major_gc() {
            // BEFORE: handle.session.performance.gc_history.push(...);
            handle.session.memory.gc_history.push(gc_event);
        }
    }
    UpdateResult::default()
}
```

The `monitoring_active` flag on `MemoryState` should be set to `true` on `VmServicePerformanceMonitoringStarted` alongside the existing flag on `PerformanceState`. Locate that handler (≈ line 1873) and add:

```rust
handle.session.performance.monitoring_active = true;
handle.session.memory.monitoring_active = true;   // NEW
```

(`alloc_pause_tx` storage is unchanged — that lives on `SessionHandle`, not on the state structs.)

#### 6. Update `crates/fdemon-app/src/handler/devtools/performance.rs`

Handlers stay in this file in T02 (they move to `memory.rs` in T03). Their **destination** changes:

| Handler | Lines (approx) | Field update |
|---|---|---|
| `handle_memory_sample_received` | 74–83 | `handle.session.performance.memory_samples` → `handle.session.memory.memory_samples`. Also set `handle.session.memory.monitoring_active = true`. |
| `handle_allocation_profile_received` | 90–104 | `…performance.allocation_profile` → `…memory.allocation_profile` |
| `handle_toggle_allocation_sort` | 110–119 | `…performance.allocation_sort` → `…memory.allocation_sort` |
| `handle_perf_select_alloc_row` | 331–345 | `…performance.alloc_table_selected_row` → `…memory.alloc_table_selected_row`; `…performance.focused_section = PerfSection::MemoryList` → `…memory.focused_section = MemorySection::AllocationList` |
| `handle_perf_scroll`, `_page`, `_jump_to_start`, `_jump_to_end` | 145–319 | Memory branches (`PerfSection::MemoryChart` / `PerfSection::MemoryList`) now read `MemorySection` from `…memory.focused_section` and write to `…memory.memory_chart_scroll_offset` / `alloc_table_scroll_offset`. This is a sub-task on its own — see Detail 6.1 below. |

##### Detail 6.1 — handler split logic

`PerformanceState.focused_section` is now `PerfSection { FrameChart, DetailsTab }` only. The memory branches must read from `MemoryState.focused_section`. Two approaches:

**Approach A (preferred for T02):** Keep the four scroll/page/jump handlers reading **both** states. Use `match handle.session.memory.focused_section` after exhausting the `PerfSection` branches. The handler dispatch in T02 still routes all four messages to these multi-target handlers; T03 will split them into `handle_perf_*` (frame-only) and `handle_mem_*` (memory-only) when `Mem*` messages are introduced.

Example pseudo-code:

```rust
pub fn handle_perf_scroll(state: &mut AppState, dir: ScrollDir) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        match handle.session.performance.focused_section {
            PerfSection::FrameChart => { /* frame chart scroll, unchanged */ }
            PerfSection::DetailsTab => { /* no-op in Phase 1 */ }
        }
        match handle.session.memory.focused_section {
            MemorySection::Chart => {
                let len = handle.session.memory.memory_samples.len();
                let visible = handle.session.memory.memory_chart_visible_width.get();
                handle.session.memory.memory_chart_scroll_offset = clamp_chart_scroll(
                    len, visible, handle.session.memory.memory_chart_scroll_offset,
                    if matches!(dir, ScrollDir::Up) { 1 } else { -1 },
                );
            }
            MemorySection::AllocationList => { scroll_alloc_table(&mut handle.session.memory, dir, 1); }
        }
    }
    UpdateResult::default()
}
```

This is intentionally a transitional shape — T03 will split into clean `handle_perf_scroll` (frame only) and `handle_mem_scroll` (memory only).

**Approach B:** Add `Mem*` message variants in T02. We've intentionally deferred that to T03 (see T03's scope) so T02 stays at the data-layer boundary. Use Approach A.

#### 7. Update `widgets/devtools/performance/mod.rs`

In the dual-section render path (lines ≈252–304), the memory section's data inputs change:

```rust
// BEFORE:
MemoryChart::new(
    &self.performance.memory_samples,
    &self.performance.memory_history,
    &self.performance.gc_history,
    self.performance.allocation_profile.as_ref(),
    self.performance.allocation_sort,
    false,
)
.with_chart_state(
    self.performance.memory_chart_scroll_offset,
    memory_focused,
    &self.performance.memory_chart_visible_width,
)
.with_alloc_state(
    self.performance.alloc_table_scroll_offset,
    self.performance.alloc_table_selected_row,
    self.performance.focused_section == PerfSection::MemoryList,
    &self.performance.alloc_table_visible_height,
)
.render_with_regions(memory_inner, buf, ctx);

// AFTER:
MemoryChart::new(
    &self.memory.memory_samples,
    &self.memory.memory_history,
    &self.memory.gc_history,
    self.memory.allocation_profile.as_ref(),
    self.memory.allocation_sort,
    false,
)
.with_chart_state(
    self.memory.memory_chart_scroll_offset,
    self.memory.focused_section == MemorySection::Chart,
    &self.memory.memory_chart_visible_width,
)
.with_alloc_state(
    self.memory.alloc_table_scroll_offset,
    self.memory.alloc_table_selected_row,
    self.memory.focused_section == MemorySection::AllocationList,
    &self.memory.alloc_table_visible_height,
)
.render_with_regions(memory_inner, buf, ctx);
```

This requires `PerformancePanel` to gain a `memory: &MemoryState` field. Update the struct and `new()` signature:

```rust
pub struct PerformancePanel<'a> {
    performance: &'a PerformanceState,
    memory: &'a MemoryState,    // NEW
    vm_connected: bool,
    vm_connection_error: Option<&'a str>,
    connection_status: &'a VmConnectionStatus,
    icons: IconSet,
}

impl<'a> PerformancePanel<'a> {
    pub fn new(
        performance: &'a PerformanceState,
        memory: &'a MemoryState,    // NEW
        vm_connected: bool,
        icons: IconSet,
        connection_status: &'a VmConnectionStatus,
    ) -> Self { ... }
}
```

Update the single call site in `widgets/devtools/mod.rs:147–154`:

```rust
let widget = PerformancePanel::new(
    perf,
    &s.session.memory,      // NEW
    vm_connected,
    self.icons,
    &self.state.connection_status,
).with_connection_error(...);
```

The `default_perf` fallback also needs a `default_memory = MemoryState::default()` companion.

#### 8. Update `widgets/devtools/performance/memory_chart/*`

The widget already takes its data as constructor parameters — internal code does NOT directly read `PerformanceState`. The only adjustment is the **type annotation** in the `&Cell<usize>` parameters for `with_chart_state` and `with_alloc_state` (they were borrowed from `PerformanceState`; they're now borrowed from `MemoryState`). The lifetime / `&Cell<usize>` shape is unchanged.

`table.rs` line 15: `use fdemon_app::session::PerfSection;` — keep as-is in T02 (the table still emits `Message::PerfFocusSection(PerfSection::MemoryList)`). **Do not** change the emitted message in T02 — wait for T03. The handler can absorb the legacy message in T02 because `PerfSection::MemoryList` no longer exists.

Wait — that's a problem. `PerfSection::MemoryList` is deleted in T02 step 2. Two fixes:

**Fix option 1 (T02-internal):** Temporarily emit `Message::PerfFocusSection(PerfSection::FrameChart)` (a no-op surrogate) from the click handler in `table.rs`. The functionality is broken on row click in T02 — but the panel placeholder doesn't render the table anyway, so the click can never fire. T03 will replace this with the proper `MemFocusSection`.

**Fix option 2 (T02-aware):** Add a temporary `PerfSection::MemoryListLegacy` variant just for the deprecation period. Too invasive — reject.

Use **Fix option 1**. Add this comment at the emission site:

```rust
// T02 transitional: row click emits a no-op message because PerfSection no longer
// has MemoryList. T03 will replace this with Message::MemFocusSection(MemorySection::AllocationList).
ctx.click(rect, MouseAction::emit(Message::PerfFocusSection(PerfSection::FrameChart)));
```

#### 9. Update tests in `widgets/devtools/performance/tests.rs`

Every test that constructs `PerformanceState { memory_history: ..., memory_samples: ..., allocation_profile: ..., ... }` must now construct both a `PerformanceState` and a `MemoryState`. Build a small helper at the top of the test module:

```rust
fn make_perf_with_memory() -> (PerformanceState, MemoryState) {
    let mut perf = PerformanceState::default();
    let mut mem = MemoryState::default();
    perf.monitoring_active = true;
    mem.monitoring_active = true;
    (perf, mem)
}
```

Update each test that uses the dual-section path to construct both states and pass `&mem` as the new `PerformancePanel::new` argument.

Tests to update (from the codebase research):
- `test_performance_panel_renders_two_sections` (line ≈?? — search)
- `test_performance_panel_no_stats_section`
- `test_performance_panel_dual_section_at_min_height`
- `test_performance_panel_allocation_table_visible_on_24_row_terminal`
- `test_performance_panel_allocation_table_visible_on_30_row_terminal`
- `test_footer_does_not_overlap_memory_border`

These tests stay in place in T02 (no file move). T03 will sort them — some delete, some move to `widgets/devtools/memory/tests.rs`.

#### 10. Update tests in `widgets/devtools/performance/memory_chart/tests.rs`

Same pattern — tests that construct a state struct change from `PerformanceState` to `MemoryState`. The whole file moves in T03.

### Acceptance Criteria

1. `cargo check --workspace --all-targets` succeeds.
2. `cargo test --workspace` passes. All previously-passing memory-related tests still pass.
3. `cargo clippy --workspace --all-targets -- -D warnings` is clean.
4. Manually starting fdemon, entering DevTools, and pressing `p` shows the Performance panel with both Frame Chart AND Memory section rendering identically to before (the memory section now reads from `session.memory` under the hood — no visible difference).
5. Memory snapshots from `VmServiceMemorySnapshot` accumulate in `session.memory.memory_history` (verify by re-running with a Flutter app — the chart still plots data).
6. `session.performance` no longer carries `memory_history`, `gc_history`, `memory_samples`, `allocation_*`, `memory_chart_*`, or `alloc_table_*` fields.
7. `PerfSection` has exactly two variants: `FrameChart`, `DetailsTab`.
8. `MemorySection` has exactly two variants: `Chart`, `AllocationList`.

### Testing

Add new tests inside `crates/fdemon-app/src/session/memory.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_state_default_uses_named_constants() {
        let mem = MemoryState::default();
        assert_eq!(mem.memory_history.capacity(), DEFAULT_MEMORY_HISTORY_SIZE);
        assert_eq!(mem.gc_history.capacity(), DEFAULT_GC_HISTORY_SIZE);
        assert_eq!(mem.memory_samples.capacity(), DEFAULT_MEMORY_SAMPLE_SIZE);
        assert!(mem.allocation_profile.is_none());
        assert_eq!(mem.allocation_sort, AllocationSortColumn::BySize);
        assert!(!mem.monitoring_active);
        assert_eq!(mem.focused_section, MemorySection::Chart);
    }

    #[test]
    fn memory_section_next_cycles() {
        assert_eq!(MemorySection::Chart.next(), MemorySection::AllocationList);
        assert_eq!(MemorySection::AllocationList.next(), MemorySection::Chart);
    }

    #[test]
    fn with_history_size_overrides_default() {
        let mem = MemoryState::with_history_size(120);
        assert_eq!(mem.memory_history.capacity(), 120);
        // Other buffers use defaults.
        assert_eq!(mem.gc_history.capacity(), DEFAULT_GC_HISTORY_SIZE);
        assert_eq!(mem.memory_samples.capacity(), DEFAULT_MEMORY_SAMPLE_SIZE);
    }
}
```

Add a regression test inside `update.rs` (or the existing update test module):

```rust
#[test]
fn vm_service_memory_snapshot_writes_to_memory_state() {
    let mut state = AppState::default();
    let session_id = SessionId::new();
    // ...register a session with id session_id...

    let memory = MemoryUsage { /* ... */ };
    let msg = Message::VmServiceMemorySnapshot { session_id, memory };
    update(&mut state, msg);

    let handle = state.session_manager.find(session_id).unwrap();
    assert_eq!(handle.session.memory.memory_history.len(), 1);
    assert_eq!(handle.session.performance.frame_history.len(), 0); // perf untouched
}
```

### Notes

- **`PerfSection::DetailsTab` is a Phase-2 anchor.** Cycling Tab to it is a visible no-op in Phase 1 — the panel still renders the frame chart only. T03 keeps the variant; Phase 2 attaches the details pane.
- **The `PerfSection::MemoryList` transitional emission** in `table.rs` (a no-op surrogate `PerfFocusSection(FrameChart)`) is intentional and short-lived — T03 replaces it within the same merge.
- **`STATS_RECOMPUTE_INTERVAL`** stays in `performance.rs` — it governs frame stat recomputation only.
- **`AllocationSortColumn`** moves from `performance.rs` to `memory.rs`. Update the `use` paths in all readers (handler, widgets, tests). The orchestrator will catch missed imports at `cargo check`.
- **Do not** rename `MemoryChart` to `MemoryPanel` in T02 — the widget rename happens in T03 when the file moves. The widget keeps its current name with updated state inputs.
- **Do not** add `Mem*` `Message` variants in T02. T03 owns message-layer changes.
- **Do not** touch `keys.rs` in T02 — the `'s'` allocation sort guard stays under `in_performance` for one task. T03 moves it to `in_memory`.
- **`alloc_pause_tx` storage** lives on `SessionHandle`, not on the state structs. It is unchanged in T02.

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-abcceafdcde15dd39

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/session/memory.rs` | NEW: `MemoryState`, `MemorySection`, `AllocationSortColumn`, constants, inline tests |
| `crates/fdemon-app/src/session/performance.rs` | Slimmed down: removed memory fields, updated `PerfSection` to 2-state cycle (`FrameChart`, `DetailsTab`), removed memory-related constants and `AllocationSortColumn`, updated tests |
| `crates/fdemon-app/src/session/session.rs` | Added `pub memory: MemoryState` field, initialized in `Session::new()` |
| `crates/fdemon-app/src/session/mod.rs` | Added `pub mod memory`, updated re-exports (`MemoryState`, `MemorySection`, `AllocationSortColumn` from memory; removed from performance) |
| `crates/fdemon-app/src/handler/update.rs` | Redirected `VmServiceMemorySnapshot` to `session.memory.memory_history`, `VmServiceGcEvent` to `session.memory.gc_history`, `VmServicePerformanceMonitoringStarted` sets both `performance.monitoring_active` and `memory.monitoring_active`; `VmServiceConnected` resets both states |
| `crates/fdemon-app/src/handler/devtools/performance.rs` | Updated all memory handlers to write to `session.memory.*`; scroll/page/jump handlers now use Approach A (dual-state: perf section then memory section); tests updated |
| `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs` | Added `memory: &MemoryState` to `PerformancePanel`, updated `new()` signature, memory section reads from `self.memory.*` |
| `crates/fdemon-tui/src/widgets/devtools/mod.rs` | Updated `PerformancePanel::new` call to pass `&s.session.memory` as second arg |
| `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart/table.rs` | Fixed `PerfSection::MemoryList` → T02 no-op surrogate `PerfSection::FrameChart` with comment |
| `crates/fdemon-tui/src/widgets/devtools/performance/tests.rs` | Complete rewrite of test construction to use `(PerformanceState, MemoryState)` pair |
| `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart/tests.rs` | Updated empty-area click assertion for T02 transitional no-op |
| `crates/fdemon-app/src/handler/tests.rs` | Updated memory history / gc history / allocation_profile assertions to read from `session.memory.*` |
| `crates/fdemon-app/src/session/tests.rs` | Fixed `test_performance_state_default` and `test_performance_state_memory_ring_buffer_capacity` to use new split state |

### Notable Decisions/Tradeoffs

1. **Approach A for scroll/page/jump handlers**: The T02 handlers now read BOTH `perf.focused_section` (for FrameChart/DetailsTab) and `memory.focused_section` (for Chart/AllocationList) in sequence. This is intentionally transitional — T03 will split these into `handle_perf_*` (frame only) and `handle_mem_*` (memory only) once `Mem*` messages are introduced.
2. **T02 no-op click surrogate**: `PerfSection::MemoryList` no longer exists, so `table.rs` and `performance/mod.rs` emit `PerfFocusSection(FrameChart)` as a harmless no-op. T03 replaces this with `MemFocusSection(AllocationList)`.
3. **`session/memory` made `pub`**: The module was initially `pub(crate)` but `fdemon-tui` needs to access `MemoryState` directly via `fdemon_app::session::memory::MemoryState`, so it was made `pub`.
4. **`with_memory_history_size` removed**: The constructor moved to `MemoryState::with_history_size`. The `VmServiceConnected` handler now calls `PerformanceState::default()` + `MemoryState::with_history_size(memory_history_size)` separately.
5. **`monitoring_active` check**: The disconnected state guard in `PerformancePanel` now checks `!perf.monitoring_active && !mem.monitoring_active` so the panel stays usable if either subsystem is active.

### Testing Performed

- `cargo check --workspace --all-targets` — Passed
- `cargo test --workspace` — Passed (5,832+ tests, 0 failed)
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed (clean)

### Risks/Limitations

1. **Transitional no-ops**: The T02 click surrogates and dual-match scroll handlers are intentionally temporary. T03 must clean them up; if T03 is delayed, the click behavior for empty table space and the memory section focus-click will be incorrect (clicking focuses FrameChart instead of AllocationList).
2. **`PerfSection` test updates**: Tests that previously checked a 3-way cycle (`FrameChart → MemoryChart → MemoryList → FrameChart`) now check the 2-state cycle (`FrameChart → DetailsTab → FrameChart`). Old cycle tests have been removed; they will be reimplemented as `MemorySection` cycle tests in T03.
