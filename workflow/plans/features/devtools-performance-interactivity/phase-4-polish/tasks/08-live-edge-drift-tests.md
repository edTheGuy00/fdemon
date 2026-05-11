## Task: Live-Edge Drift + Integration Tests

**Objective**: Lock in the Model A scroll-offset semantics with integration tests. Cover: scroll-then-grow buffer behavior, scroll + selection conflict resolution, Home/End jumps, focus-cycle invariants.

**Depends on**: Phase 3 (tasks 05, 06, 07)

**Estimated Time**: 1 hour

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/devtools/performance/frame_chart/tests.rs` (or wherever frame-chart tests live): Add tests asserting Model A semantics.
- `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart/` tests: Mirror tests for the memory chart.
- `crates/fdemon-app/src/handler/devtools/performance.rs` tests (if not already added in Phase 2): Cover the integrated focus-section + scroll-bound flow.

**Files Read (Dependencies):**
- All Phase 3 widget code.
- `compute_visible_range` and `visible_memory_window` from tasks 05/06.

### Details

Tests to add:

1. **`scroll_offset_persists_under_new_arrivals`** — Set `scroll_offset = 100`, append 20 new frames, recompute visible range. Expected: the window stays 100 frames back from the *new* live edge (Model A).

2. **`jump_to_end_resets_scroll_offset_to_zero`** — Set `scroll_offset = 50`, dispatch `PerfJumpToEnd`. Expected: `scroll_offset == 0`.

3. **`jump_to_start_sets_max_back`** — Push 1000 frames, set `frame_chart_visible_width = 50`, dispatch `PerfJumpToStart`. Expected: `scroll_offset == 950` (1000 - 50).

4. **`left_right_arrow_clears_scroll_offset`** — Set `scroll_offset = 50` and `selected_frame = None`. Press Left arrow. Expected: `scroll_offset == 0` and selection moves to live-edge-relative position. (Confirm Phase 2 implementation enforces this.)

5. **`tab_cycles_focus_through_three_sections`** — Tab three times from any starting state, expect to return to original section.

6. **`mouse_click_on_alloc_row_focuses_section`** — Dispatch `PerfSelectAllocRow { index: Some(0) }`. Expected: `focused_section = MemoryList` and `alloc_table_selected_row = Some(0)`.

7. **`alloc_table_scroll_keeps_selection_visible`** — Set `alloc_table_selected_row = Some(20)`, `alloc_table_visible_height = 10`. Scroll down. Expected: `alloc_table_scroll_offset` adjusts so selected row stays visible.

### Acceptance Criteria

1. All 7 tests pass.
2. Tests have descriptive names per CODE_STANDARDS.md.
3. Manual smoke verification documented in this task's completion summary:
   - Launch fdemon against `example/app2`, enter DevTools, switch to Performance.
   - Tab through sections — observe border highlight.
   - Scroll each section — observe values change.
   - Click sections + alloc rows — observe focus + selection.
   - Press End — return to live edge.
4. `cargo test --workspace` and `cargo clippy --workspace -- -D warnings` pass.

### Notes

- Manual smoke results go in the Completion Summary at the bottom of this task file.
- If a test reveals a design defect (Model A vs B mismatch), surface it as a follow-up task rather than silently fixing.
