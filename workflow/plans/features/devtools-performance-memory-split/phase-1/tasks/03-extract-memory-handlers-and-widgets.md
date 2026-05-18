## Task: Extract Memory Handlers, Add `Mem*` Messages, Move Memory Widgets, Wire Real Memory Panel

**Objective**: Complete the Phase 1 split by (a) extracting all memory-side handlers from `handler/devtools/performance.rs` into a new `handler/devtools/memory.rs`, (b) introducing `Mem*` `Message` variants and the `in_memory` keymap guard, (c) moving the memory widget subtree from `widgets/devtools/performance/memory_chart/` to `widgets/devtools/memory/` and renaming the widget to `MemoryPanel`, and (d) wiring the `DevToolsPanel::Memory` dispatch in `widgets/devtools/mod.rs` to render the real widget instead of T01's placeholder. After this task the Memory tab fully replaces the dual-section memory bottom-half from the original Performance panel — each tab now uses its full inner area.

**Depends on**: 01-add-memory-panel-placeholder, 02-extract-memory-state

**Agent:** implementor

**Estimated Time**: 6–8 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/devtools/memory.rs` — **NEW.** All memory-side handlers move here.
- `crates/fdemon-app/src/handler/devtools/performance.rs` — Remove the memory-side handlers and their helpers; trim the file to ~700 lines.
- `crates/fdemon-app/src/handler/devtools/mod.rs` — Declare the new `memory` submodule; route `Mem*` `Message` arms to it.
- `crates/fdemon-app/src/handler/keys.rs` — Add `in_memory` guard block (Tab/j/k/PageUp/Down/Home/End/`s`); add Esc-deselect-row path for Memory tab; **move** the existing `'s'` shortcut from the `in_performance` to the `in_memory` arm.
- `crates/fdemon-app/src/message.rs` — Add `Mem*` variants (`MemFocusSection`, `MemScrollUp/Down`, `MemPageUp/Down`, `MemJumpToStart/End`, `MemSelectAllocRow`, `MemToggleSort`).
- `crates/fdemon-tui/src/widgets/devtools/memory/mod.rs` — **NEW** (moved from `widgets/devtools/performance/memory_chart/mod.rs`). Widget renamed `MemoryPanel`. Top-level render entry point becomes `memory::render_with_regions`.
- `crates/fdemon-tui/src/widgets/devtools/memory/chart.rs` — **NEW** (moved from `performance/memory_chart/chart.rs`). Internal pub paths re-anchored.
- `crates/fdemon-tui/src/widgets/devtools/memory/table.rs` — **NEW** (moved from `performance/memory_chart/table.rs`). Click message changed from `PerfFocusSection(PerfSection::MemoryList)` / `PerfSelectAllocRow` → `MemFocusSection(MemorySection::AllocationList)` / `MemSelectAllocRow`.
- `crates/fdemon-tui/src/widgets/devtools/memory/braille_canvas.rs` — **NEW** (moved verbatim).
- `crates/fdemon-tui/src/widgets/devtools/memory/tests.rs` — **NEW.** Contains the 36 tests from the old `performance/memory_chart/tests.rs` PLUS the 6 migrated tests from `performance/tests.rs`. `use super::*` adjusted.
- `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs` — **Remove the memory section block** (lines ≈252–304) entirely. Delete the 45/55 vertical split (`DUAL_SECTION_MIN_HEIGHT`, the `chunks` layout, the memory `Block`, the `MemoryChart::new(...)` call). The frame-chart-only path now becomes the only dual-or-better path. Simplify `render_impl` accordingly. The `memory: &MemoryState` field added in T02 also goes away (no longer needed since the widget no longer renders memory).
- `crates/fdemon-tui/src/widgets/devtools/performance/tests.rs` — Delete the 3 obsolete dual-section tests; keep the 12 frame-only tests.
- `crates/fdemon-tui/src/widgets/devtools/mod.rs` — Replace the Memory placeholder body (added in T01) with a real call to `memory::render_with_regions`. Update the `PerformancePanel::new` call site to drop the `memory: &MemoryState` argument that T02 added (the widget no longer renders memory). Update the footer hint for the Memory tab to its final keymap.

**Files Deleted:**
- `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart/` — the entire directory (5 files). Their content moves to `widgets/devtools/memory/`.

**Files Read (Dependencies):**
- T01 and T02 task files (for the placeholder hooks and the new `MemoryState` API).

### Details

#### 1. Add `Mem*` Message variants

In `crates/fdemon-app/src/message.rs`, add (alphabetical or beside `Perf*` neighbours):

```rust
/// Cycle focus within the Memory panel sections (Chart ↔ AllocationList).
MemFocusSection(MemorySection),

/// Scroll the focused Memory section by one unit (one row / one sample).
MemScrollUp,
MemScrollDown,

/// Page the focused Memory section by a viewport-height unit.
MemPageUp,
MemPageDown,

/// Jump to the oldest / live edge of the focused Memory section.
MemJumpToStart,
MemJumpToEnd,

/// Select an allocation table row (or deselect with `None`).
MemSelectAllocRow { index: Option<usize> },

/// Toggle the allocation table sort column (BySize ↔ ByInstances).
MemToggleSort,
```

Imports `use crate::session::MemorySection;` at the top of `message.rs`.

The existing `PerfFocusSection(PerfSection)` variant stays (now carries the slim `PerfSection { FrameChart, DetailsTab }`).
The existing `PerfSelectAllocRow` and `ToggleAllocationSort` variants are renamed to `MemSelectAllocRow` and `MemToggleSort` — **rename in place** (single-shot find/replace). The renamed message names match the new handler names.

#### 2. Create `handler/devtools/memory.rs`

```rust
//! Memory panel handlers.
//!
//! Mirrors `handler::devtools::performance` but routes to `session.memory`.
//! Handles allocation profile updates, alloc-table sort/row selection,
//! memory chart scroll, and Tab cycling between Memory subsections.

use crate::handler::UpdateResult;
use crate::message::Message;
use crate::session::{MemorySection, AllocationSortColumn};
use crate::state::AppState;
use fdemon_core::performance::{AllocationProfile, MemorySample, MemoryUsage, GcEvent};

/// Fallback page size when the render-hint visible dimension is 0 (not yet rendered).
const DEFAULT_MEM_PAGE_SIZE: usize = 10;

pub(crate) enum ScrollDir { Up, Down }

fn clamp_chart_scroll(buffer_len: usize, visible_width: usize, current: usize, delta: i64) -> usize {
    let max_back = buffer_len.saturating_sub(visible_width.max(1));
    let new = current as i64 + delta;
    new.clamp(0, max_back as i64) as usize
}

pub(crate) fn handle_memory_sample_received(state: &mut AppState, session_id: SessionId, sample: MemorySample) -> UpdateResult { /* ... */ }
pub(crate) fn handle_allocation_profile_received(state: &mut AppState, session_id: SessionId, profile: AllocationProfile) -> UpdateResult { /* ... */ }
pub(crate) fn handle_toggle_allocation_sort(state: &mut AppState) -> UpdateResult { /* ... */ }
pub(crate) fn handle_mem_focus_section(state: &mut AppState, section: MemorySection) -> UpdateResult { /* ... */ }
pub(crate) fn handle_mem_scroll(state: &mut AppState, dir: ScrollDir) -> UpdateResult { /* ... */ }
pub(crate) fn handle_mem_page(state: &mut AppState, dir: ScrollDir) -> UpdateResult { /* ... */ }
pub(crate) fn handle_mem_jump_to_start(state: &mut AppState) -> UpdateResult { /* ... */ }
pub(crate) fn handle_mem_jump_to_end(state: &mut AppState) -> UpdateResult { /* ... */ }
pub(crate) fn handle_mem_select_alloc_row(state: &mut AppState, index: Option<usize>) -> UpdateResult { /* ... */ }

fn alloc_row_count(handle: &SessionHandle) -> usize { /* moved from performance.rs */ }
fn scroll_alloc_table(memory: &mut MemoryState, dir: ScrollDir, units: i64) { /* moved */ }
```

**Move from `handler/devtools/performance.rs`:**

| Symbol | New name in `memory.rs` |
|---|---|
| `handle_memory_sample_received` | unchanged |
| `handle_allocation_profile_received` | unchanged |
| `handle_toggle_allocation_sort` | unchanged |
| `handle_perf_select_alloc_row` | renamed → `handle_mem_select_alloc_row` |
| Memory branches of `handle_perf_scroll` | extract to `handle_mem_scroll` |
| Memory branches of `handle_perf_page` | extract to `handle_mem_page` |
| Memory branches of `handle_perf_jump_to_start` | extract to `handle_mem_jump_to_start` |
| Memory branches of `handle_perf_jump_to_end` | extract to `handle_mem_jump_to_end` |
| Helper `alloc_row_count` | unchanged |
| Helper `scroll_alloc_table` | parameter changes from `&mut PerformanceState` to `&mut MemoryState` |
| Helper `clamp_chart_scroll` | **duplicate** the 9-line helper in `memory.rs` (cleaner than cross-module visibility). The existing copy in `performance.rs` stays for frame-chart scrolling. |
| `ScrollDir` | **duplicate** the 4-line enum in `memory.rs` |
| `DEFAULT_PERF_PAGE_SIZE` | rename to `DEFAULT_MEM_PAGE_SIZE` and place in `memory.rs` (the constant in `performance.rs` keeps its name) |

**Remaining in `handler/devtools/performance.rs` after the split:**
- `handle_select_performance_frame` (unchanged)
- `handle_perf_focus_section` (cycles `PerfSection { FrameChart, DetailsTab }` only — no memory branch)
- `handle_perf_scroll`, `_page`, `_jump_to_start`, `_jump_to_end` — only the `FrameChart` branch survives; the `DetailsTab` branch is a no-op for Phase 1
- `clamp_chart_scroll`, `ScrollDir`, `DEFAULT_PERF_PAGE_SIZE` — shared helpers retained

#### 3. `handler/devtools/mod.rs` — declare submodule + route messages

Near the top with the other submodule declarations:

```rust
pub(crate) mod inspector;
pub(crate) mod performance;
pub(crate) mod memory;       // NEW
pub(crate) mod network;
```

Inside the central dispatch (search for `Message::ToggleAllocationSort` / `Message::PerfSelectAllocRow` etc.), update arms:

```rust
// BEFORE:
Message::ToggleAllocationSort => performance::handle_toggle_allocation_sort(state),
Message::PerfSelectAllocRow { index } => performance::handle_perf_select_alloc_row(state, index),
// AFTER:
Message::MemToggleSort => memory::handle_toggle_allocation_sort(state),
Message::MemSelectAllocRow { index } => memory::handle_mem_select_alloc_row(state, index),

// NEW arms:
Message::MemFocusSection(section) => memory::handle_mem_focus_section(state, section),
Message::MemScrollUp => memory::handle_mem_scroll(state, memory::ScrollDir::Up),
Message::MemScrollDown => memory::handle_mem_scroll(state, memory::ScrollDir::Down),
Message::MemPageUp => memory::handle_mem_page(state, memory::ScrollDir::Up),
Message::MemPageDown => memory::handle_mem_page(state, memory::ScrollDir::Down),
Message::MemJumpToStart => memory::handle_mem_jump_to_start(state),
Message::MemJumpToEnd => memory::handle_mem_jump_to_end(state),

// Existing arms simplified (no longer dispatch memory branches):
Message::PerfScrollUp => performance::handle_perf_scroll(state, performance::ScrollDir::Up),
// ...etc...

// Allocation profile and memory sample arms (currently routed to performance.rs):
Message::VmServiceAllocationProfileReceived { session_id, profile }
    => memory::handle_allocation_profile_received(state, session_id, profile),
Message::VmServiceMemorySample { session_id, sample }
    => memory::handle_memory_sample_received(state, session_id, sample),
```

The `VmServiceMemorySnapshot` and `VmServiceGcEvent` inline blocks in `update.rs` already write to `session.memory.*` from T02 — no further changes required.

#### 4. `handler/keys.rs` — add `in_memory` guard, move `'s'`

Add a new `in_memory` flag at the top of `handle_key_devtools`:

```rust
let in_inspector = state.devtools_view_state.active_panel == DevToolsPanel::Inspector;
let in_performance = state.devtools_view_state.active_panel == DevToolsPanel::Performance;
let in_memory = state.devtools_view_state.active_panel == DevToolsPanel::Memory;   // NEW
let in_network = state.devtools_view_state.active_panel == DevToolsPanel::Network;
```

After the existing `in_performance` block (lines ≈486–523), add a parallel `in_memory` block:

```rust
if in_memory {
    match key {
        InputKey::Tab => {
            let next = state.session_manager.selected()
                .map(|h| h.session.memory.focused_section.next())
                .unwrap_or_default();
            return Some(Message::MemFocusSection(next));
        }
        InputKey::BackTab => {
            let prev = state.session_manager.selected()
                .map(|h| h.session.memory.focused_section.prev())
                .unwrap_or_default();
            return Some(Message::MemFocusSection(prev));
        }
        InputKey::Up | InputKey::Char('k') => return Some(Message::MemScrollUp),
        InputKey::Down | InputKey::Char('j') => return Some(Message::MemScrollDown),
        InputKey::PageUp => return Some(Message::MemPageUp),
        InputKey::PageDown => return Some(Message::MemPageDown),
        InputKey::Home => return Some(Message::MemJumpToStart),
        InputKey::End => return Some(Message::MemJumpToEnd),
        _ => {}
    }
}
```

Update the `'s'` binding (line ≈696):

```rust
// BEFORE:
InputKey::Char('s') if in_performance => Some(Message::ToggleAllocationSort),
// AFTER:
InputKey::Char('s') if in_memory => Some(Message::MemToggleSort),
```

Update the `Esc` block (lines ≈534–555) to handle Memory:

```rust
InputKey::Esc => {
    if in_performance {
        let frame_selected = state.session_manager.selected()
            .map(|h| h.session.performance.selected_frame.is_some())
            .unwrap_or(false);
        if frame_selected {
            return Some(Message::SelectPerformanceFrame { index: None });
        }
    }
    if in_memory {
        let row_selected = state.session_manager.selected()
            .map(|h| h.session.memory.alloc_table_selected_row.is_some())
            .unwrap_or(false);
        if row_selected {
            return Some(Message::MemSelectAllocRow { index: None });
        }
    }
    if in_network { /* ...existing... */ }
    Some(Message::DevToolsEscape)
}
```

#### 5. Move widgets — `performance/memory_chart/` → `memory/`

Five file moves (use `git mv` if working in a real branch — orchestrator should handle this via the standard `Write`+delete pattern):

```
crates/fdemon-tui/src/widgets/devtools/performance/memory_chart/braille_canvas.rs
  → crates/fdemon-tui/src/widgets/devtools/memory/braille_canvas.rs
crates/fdemon-tui/src/widgets/devtools/performance/memory_chart/chart.rs
  → crates/fdemon-tui/src/widgets/devtools/memory/chart.rs
crates/fdemon-tui/src/widgets/devtools/performance/memory_chart/table.rs
  → crates/fdemon-tui/src/widgets/devtools/memory/table.rs
crates/fdemon-tui/src/widgets/devtools/performance/memory_chart/mod.rs
  → crates/fdemon-tui/src/widgets/devtools/memory/mod.rs
crates/fdemon-tui/src/widgets/devtools/performance/memory_chart/tests.rs
  → crates/fdemon-tui/src/widgets/devtools/memory/tests.rs
```

##### Per-file changes after the move

**`memory/mod.rs`:**
- Rename struct `MemoryChart` → `MemoryPanel` (find/replace within the file). The widget keeps its existing builder pattern (`new`, `with_chart_state`, `with_alloc_state`, `render_with_regions`).
- Module doc-comment header updated:
  ```rust
  //! Memory panel widget for the DevTools TUI mode.
  //!
  //! Displays the memory usage time-series chart and class allocation table
  //! using data from `MemoryState` (rich memory samples, allocation profile,
  //! GC events). This widget gets the full panel inner area — chart on top,
  //! allocation table below.
  ```
- The `MemoryPanel::new` constructor signature accepts a single `&MemoryState` rather than the long list of individual fields (this consolidation simplifies the call site).
  ```rust
  pub fn new(memory: &'a MemoryState, focused: bool /* for whole-panel border colour */) -> Self { ... }
  ```
- Add a top-level `pub fn render_with_regions(area, buf, widget, ctx)` similar to the Inspector / Performance pattern.
- Layout: chart on top ≈55%, allocation table on bottom ≈45% (use `MIN_CHART_HEIGHT`, `MIN_TABLE_HEIGHT` constants from chart.rs/table.rs to set responsive thresholds — chart-only if height < `MIN_TABLE_HEIGHT + 6`, alloc-list-only if height < `MIN_CHART_HEIGHT + 6`, else both).

**`memory/chart.rs`:**
- No structural changes — internal `pub(super)` API stays.
- Any `use super::super::performance::...` paths re-anchor to the new location.

**`memory/table.rs`:**
- Line 15 import: `use fdemon_app::session::{MemorySection};` (replacing the `PerfSection` import from before).
- Click action change:
  ```rust
  // BEFORE (T02 transitional):
  ctx.click(rect, MouseAction::emit(Message::PerfFocusSection(PerfSection::FrameChart)));
  // AFTER:
  ctx.click(rect, MouseAction::emit(Message::MemFocusSection(MemorySection::AllocationList)));
  ```
- Per-row click action change:
  ```rust
  // BEFORE: ctx.click(row_rect, MouseAction::emit(Message::PerfSelectAllocRow { index: Some(i) }));
  // AFTER:  ctx.click(row_rect, MouseAction::emit(Message::MemSelectAllocRow { index: Some(i) }));
  ```

**`memory/braille_canvas.rs`:** moves verbatim. No changes.

**`memory/tests.rs`:**
- The existing 36 tests migrate verbatim with `use super::*` adjusted.
- Add the 6 migrated tests from `widgets/devtools/performance/tests.rs`:
  - `test_performance_panel_no_stats_section` → rename `test_memory_panel_no_stats_section`
  - `test_performance_panel_allocation_table_visible_on_24_row_terminal` → rename `test_memory_panel_allocation_table_visible_on_24_row_terminal`
  - `test_performance_panel_allocation_table_visible_on_30_row_terminal` → rename `test_memory_panel_allocation_table_visible_on_30_row_terminal`
  - (And any other memory-related ones identified by the codebase research.)
- Add new tests:
  - `test_memory_panel_allocation_table_full_height_at_20_rows` — the bug-regression test. Builds a `MemoryState` with a 30-class allocation profile, renders into a 80×20 buffer, asserts at least 12 alloc-table rows visible (vs. the ~2 rows visible under the old dual-section split).

##### Register the new submodule

In `crates/fdemon-tui/src/widgets/devtools/mod.rs`, near the top:

```rust
pub mod inspector;
pub mod network;
pub mod performance;
pub mod memory;          // NEW

pub use inspector::WidgetInspector;
pub use memory::MemoryPanel;        // NEW
pub use network::NetworkMonitor;
pub use performance::PerformancePanel;
```

In `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs`, **remove** the `mod memory_chart;` declaration.

#### 6. Update `widgets/devtools/performance/mod.rs` — drop the memory section

- Remove `mod memory_chart;`.
- Remove the `memory: &'a MemoryState` field added in T02 (no longer needed — Performance widget renders only Frame Chart in Phase 1).
- Remove the `DUAL_SECTION_MIN_HEIGHT` constant.
- Replace the dual-section render path (lines ≈193–304) with a frame-chart-only path. The structure simplifies to: disconnected → compact_summary → frame_chart_full. The frame_chart_only logic from lines 150–191 becomes the only non-degenerate path.
- The "Reserve 1 row at the bottom for the footer" comment + `usable_area` calculation moves into the frame-chart-full block.
- Update `PerformancePanel::new` signature to drop the `memory` parameter.

#### 7. Update `widgets/devtools/mod.rs` — wire the real Memory panel

Replace the placeholder body (added in T01) with the real widget call. After the `Performance` arm:

```rust
DevToolsPanel::Memory => {
    static DEFAULT_MEMORY: std::sync::LazyLock<fdemon_app::session::MemoryState> =
        std::sync::LazyLock::new(fdemon_app::session::MemoryState::default);
    // Actually — MemoryState contains Cell render-hints which are !Sync.
    // Use the same stack-local default pattern as Performance:
    let default_memory;
    let (mem, vm_connected) = match self.session {
        Some(s) => (&s.session.memory, s.session.vm_connected),
        None => {
            default_memory = fdemon_app::session::MemoryState::default();
            (&default_memory, false)
        }
    };

    let widget = MemoryPanel::new(mem, true);
    memory::render_with_regions(chunks[1], buf, widget, ctx.as_deref_mut());
}
```

Update the `Performance` arm's `PerformancePanel::new(...)` call to drop the `&s.session.memory` argument that T02 added.

Update the footer hint for the Memory tab:

```rust
DevToolsPanel::Memory => {
    let has_alloc_selection = self.session
        .is_some_and(|s| s.session.memory.alloc_table_selected_row.is_some());
    if has_alloc_selection {
        "[Esc] Deselect  [Tab] Switch  [j/k] Scroll  [s] Sort  [b] Browser"
    } else {
        "[Esc] Logs  [Tab] Switch  [j/k] Scroll  [s] Sort  [b] Browser"
    }
}
```

#### 8. Test cleanup

In `widgets/devtools/performance/tests.rs`:
- Delete `test_performance_panel_renders_two_sections`.
- Delete `test_performance_panel_dual_section_at_min_height`.
- Delete `test_footer_does_not_overlap_memory_border`.
- The remaining 12 frame-only tests stay; update any that constructed a `PerformancePanel` with the T02-introduced `memory` argument (drop the argument).

In `widgets/devtools/mod.rs` tests:
- Update `devtools_tab_bar_registers_four_click_regions` count assertion (if not already 4 from T01).
- Add `test_devtools_view_renders_memory_panel_with_alloc_data` — constructs a `Session` with `memory.allocation_profile = Some(profile)` and asserts buffer contains class names from the profile.

### Acceptance Criteria

1. `cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings` passes.
2. `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart/` directory **no longer exists**. (`find . -path '*performance/memory_chart*' -type d` returns empty.)
3. `crates/fdemon-tui/src/widgets/devtools/memory/` directory exists with 5 files.
4. `crates/fdemon-app/src/handler/devtools/memory.rs` exists; `crates/fdemon-app/src/handler/devtools/performance.rs` has shrunk to ≤ 1100 lines.
5. The Memory tab in a 200×20 terminal shows the allocation table with 12+ visible rows (regression test `test_memory_panel_allocation_table_full_height_at_20_rows`).
6. The Performance tab in a 200×20 terminal shows the Frame Chart filling the inner area.
7. Pressing `Tab` in the Memory tab cycles `{Chart, AllocationList}`.
8. Pressing `s` in the Memory tab toggles allocation sort.
9. Pressing `s` in the Performance tab does NOT toggle anything (the binding is now under `in_memory` only).
10. `Esc` with an alloc row selected on the Memory tab deselects the row first; pressing `Esc` again exits to Logs.
11. Switching between Performance and Memory tabs preserves their independent state (e.g., a selected alloc row stays selected when toggling Performance→Memory).
12. The footer hint shows the correct keymap for each panel.

### Testing

##### Handler routing tests (`handler/devtools/memory.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{AllocationSortColumn, MemorySection};
    use fdemon_core::performance::AllocationProfile;

    #[test]
    fn handle_toggle_allocation_sort_cycles_columns() {
        let mut state = AppState::default();
        let session_id = /* register session */;
        // initial: BySize
        handle_toggle_allocation_sort(&mut state);
        let handle = state.session_manager.find(session_id).unwrap();
        assert_eq!(handle.session.memory.allocation_sort, AllocationSortColumn::ByInstances);

        handle_toggle_allocation_sort(&mut state);
        let handle = state.session_manager.find(session_id).unwrap();
        assert_eq!(handle.session.memory.allocation_sort, AllocationSortColumn::BySize);
    }

    #[test]
    fn handle_mem_select_alloc_row_sets_focus() {
        let mut state = AppState::default();
        let session_id = /* register session */;

        handle_mem_select_alloc_row(&mut state, Some(3));
        let handle = state.session_manager.find(session_id).unwrap();
        assert_eq!(handle.session.memory.alloc_table_selected_row, Some(3));
        assert_eq!(handle.session.memory.focused_section, MemorySection::AllocationList);

        handle_mem_select_alloc_row(&mut state, None);
        let handle = state.session_manager.find(session_id).unwrap();
        assert!(handle.session.memory.alloc_table_selected_row.is_none());
        // focused_section does NOT revert — that would be jarring during repeated selections.
    }

    #[test]
    fn handle_mem_focus_section_cycles() {
        let mut state = AppState::default();
        let session_id = /* register session */;
        handle_mem_focus_section(&mut state, MemorySection::AllocationList);
        let handle = state.session_manager.find(session_id).unwrap();
        assert_eq!(handle.session.memory.focused_section, MemorySection::AllocationList);
    }
}
```

##### Key handler routing tests (`handler/keys.rs`)

```rust
fn make_devtools_state_with_panel(panel: DevToolsPanel) -> AppState {
    let mut state = AppState::default();
    state.ui_mode = UiMode::DevTools;
    state.devtools_view_state.active_panel = panel;
    state
}

#[test]
fn memory_panel_tab_cycles_memory_section() {
    let state = make_devtools_state_with_panel(DevToolsPanel::Memory);
    let msg = handle_key_normal(&state, InputKey::Tab);
    assert!(matches!(msg, Some(Message::MemFocusSection(_))));
}

#[test]
fn memory_panel_j_emits_mem_scroll_down() {
    let state = make_devtools_state_with_panel(DevToolsPanel::Memory);
    let msg = handle_key_normal(&state, InputKey::Char('j'));
    assert!(matches!(msg, Some(Message::MemScrollDown)));
}

#[test]
fn memory_panel_s_emits_mem_toggle_sort() {
    let state = make_devtools_state_with_panel(DevToolsPanel::Memory);
    let msg = handle_key_normal(&state, InputKey::Char('s'));
    assert!(matches!(msg, Some(Message::MemToggleSort)));
}

#[test]
fn performance_panel_s_no_longer_toggles_sort() {
    let state = make_devtools_state_with_panel(DevToolsPanel::Performance);
    let msg = handle_key_normal(&state, InputKey::Char('s'));
    // 's' under in_performance is now dead — falls through to default (None or other binding).
    assert!(!matches!(msg, Some(Message::MemToggleSort)));
    assert!(!matches!(msg, Some(Message::ToggleAllocationSort)));  // removed variant
}

#[test]
fn memory_panel_esc_with_selection_deselects_first() {
    let mut state = make_devtools_state_with_panel(DevToolsPanel::Memory);
    /* set selected row */;
    let msg = handle_key_normal(&state, InputKey::Esc);
    assert!(matches!(msg, Some(Message::MemSelectAllocRow { index: None })));
}

#[test]
fn memory_panel_esc_without_selection_exits() {
    let state = make_devtools_state_with_panel(DevToolsPanel::Memory);
    let msg = handle_key_normal(&state, InputKey::Esc);
    assert!(matches!(msg, Some(Message::DevToolsEscape)));
}
```

##### Widget regression test (the layout bug fix)

```rust
// In widgets/devtools/memory/tests.rs:
#[test]
fn test_memory_panel_allocation_table_full_height_at_20_rows() {
    // Build a MemoryState with 30 distinct classes in the allocation profile.
    let mut mem = MemoryState::default();
    mem.allocation_profile = Some(make_profile_with_n_classes(30));
    mem.allocation_sort = AllocationSortColumn::BySize;

    // Render into a 200×20 terminal (mimicking the bug-report scenario).
    let widget = MemoryPanel::new(&mem, true);
    let mut buf = Buffer::empty(Rect::new(0, 0, 200, 20));
    widget.render(Rect::new(0, 0, 200, 20), &mut buf);

    // Count rows in the table area that contain class names (start with 'Class').
    let count = count_rows_matching(&buf, |row| row.starts_with("Class"));
    assert!(count >= 12,
        "expected ≥ 12 visible alloc-table rows in 20-row terminal, got {count}");
}
```

##### Tab bar mouse region test (`widgets/devtools/mod.rs`)

```rust
#[test]
fn devtools_memory_panel_click_emits_switch_to_memory() {
    let state = DevToolsViewState::default();
    let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));

    let mut regions = MouseRegions::with_capacity();
    {
        let builder = regions.builder();
        let mut ctx = MouseCtx::new(builder);
        super::render_with_regions(Rect::new(0, 0, 80, 24), &mut buf,
            DevToolsView::new(&state, None, IconSet::default()),
            Some(&mut ctx));
    }

    let memory_region = regions.iter().find(|e| matches!(
        &e.on_left,
        Some(MouseAction::Emit(msg)) if matches!(**msg, Message::SwitchDevToolsPanel(DevToolsPanel::Memory))
    ));
    assert!(memory_region.is_some(), "expected a SwitchDevToolsPanel(Memory) region");
}
```

### Notes

- **`PerformancePanel::new` arity changes twice.** T02 added a `memory` argument; T03 removes it. This is the cost of keeping T02 a pure data-layer change. Alternatively the team could land T02+T03 in a single PR — but the worktree-parallel design needs them split. Keep the two-step dance.
- **`clamp_chart_scroll` and `ScrollDir` are duplicated** across `performance.rs` and `memory.rs` (9 + 4 = 13 lines duplicated). Extraction to a shared `handler/devtools/scroll_helpers.rs` is a follow-up clean-up — out of scope for Phase 1 to avoid expanding the file overlap matrix.
- **`MemorySection` has 2 variants** — `next()` and `prev()` collapse to the same toggle. The `prev` method delegates to `next` to keep the API parallel with `PerfSection`.
- **`Esc` behaviour on Memory** mirrors Performance: select-then-deselect-then-exit. The order in `keys.rs` matters — Memory's `if in_memory` branch must come after `if in_performance` and before `if in_network` for consistent precedence.
- **The deleted `ToggleAllocationSort` and `PerfSelectAllocRow` Message variants** are a breaking change but no MCP / external consumer uses them — they only existed inside the TEA dispatch.
- **`session.memory.monitoring_active`** mirrors `session.performance.monitoring_active`. Both are set to `true` on `VmServicePerformanceMonitoringStarted` (already done in T02). The Memory panel's disconnected state check now reads `session.memory.monitoring_active`.

---

## Completion Summary

**Status:** Not Started
**Branch:** TBD

### Files Modified

| File | Changes |
|------|---------|
| | |

### Notable Decisions/Tradeoffs

1. **<Decision>**: <Rationale and implications>

### Testing Performed

- `cargo fmt --all -- --check` — TBD
- `cargo check --workspace --all-targets` — TBD
- `cargo test --workspace` — TBD
- `cargo clippy --workspace --all-targets -- -D warnings` — TBD

### Risks/Limitations

1. **<Risk>**: <Description and mitigation if any>
