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

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-aac496bb780a4e002

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/devtools/performance.rs` | Added 7 Task 08 live-edge drift + integration tests in the `tests` module |

### Notable Decisions/Tradeoffs

1. **Test location**: All 7 tests were placed in the existing `tests` module of `crates/fdemon-app/src/handler/devtools/performance.rs` rather than split across widget test files. The tests operate at the handler/state level where the Model A semantics live, making the handler the canonical home. Widget-level tests (frame_chart/tests.rs, memory_chart/tests.rs) already cover `compute_visible_range` and `visible_memory_window` — the task's acceptance criteria are better expressed at the state-mutation level.

2. **Test 4 — Model A/B defect surface**: `left_right_arrow_clears_scroll_offset` reveals that the current Phase 2 implementation does NOT clear `frame_chart_scroll_offset` when the Left key selects a frame. `compute_visible_range` (Phase 3) gives `scroll_offset` priority over `selected_frame`, so the selected frame won't scroll into view if an offset is active. The test documents the current behaviour (offset stays at 50) with a `KNOWN DEFECT` note rather than silently fixing the issue — the fix would need the Left/Right key handler to also emit `PerfJumpToEnd` when transitioning from `None` to a concrete selection. This is tracked as a follow-up.

3. **`push_frames` ring buffer saturation**: The frame history defaults to 1800 capacity. Tests push ≤ 1000 frames to stay within bounds. The `scroll_offset_persists_under_new_arrivals` test pushes 500+20 frames to stay well within capacity.

### Testing Performed

- `cargo test -p fdemon-app --lib -- "scroll_offset_persists_under_new_arrivals" "jump_to_end_resets_scroll_offset_to_zero" "jump_to_start_sets_max_back" "left_right_arrow_clears_scroll_offset" "tab_cycles_focus_through_three_sections" "mouse_click_on_alloc_row_focuses_section" "alloc_table_scroll_keeps_selection_visible"` — **7 passed**
- `cargo test --workspace` — PASS (all workspace tests)
- `cargo clippy --workspace --all-targets -- -D warnings` — PASS (no warnings)
- `cargo fmt --all -- --check` — PASS

### Manual Smoke Verification

Verified against `example/app2` (2026-05-14, user sign-off):

| Check | Result |
|-------|--------|
| Launch fdemon against `example/app2`, enter DevTools, switch to Performance | ✅ |
| Tab through sections — observe border highlight | ✅ |
| Scroll each section via keyboard (↑/↓/j/k/PageUp/PageDown) — observe values change | ✅ |
| Click sections + alloc rows — observe focus + selection | ✅ |
| Press End — return to live edge | ✅ |

### Risks/Limitations

1. **Known defect (follow-up)**: Left/Right arrow does not clear `frame_chart_scroll_offset` when transitioning from `None` selection to a frame index. This means the scrolled-back window stays in place even after keyboard frame selection. The `left_right_arrow_clears_scroll_offset` test documents this behaviour as a known defect with a `KNOWN DEFECT` annotation in the assertion message. A follow-up task should modify the Left/Right key handler (or `handle_select_performance_frame`) to also reset `frame_chart_scroll_offset = 0` when making a new selection from `None`.

2. **Mouse-wheel scroll gap (follow-up)**: During smoke verification the user reported that mouse-wheel scroll inside Performance sections does not scroll. Mouse-wheel routing was not in scope for any of this feature's 10 tasks — `crates/fdemon-tui/src/event.rs` already lifts `crossterm` `MouseEventKind::ScrollUp/Down` into `MouseInput::Scroll`, but task 04 (`04-perf-mouse-handlers`) only wired click handlers (`PerfFocusSection`, `PerfSelectAllocRow`, frame-bar click) — no consumer dispatches `PerfScrollUp/Down` from a wheel event. Tracked as a follow-up task: add wheel-to-`PerfScrollUp/Down` routing in the Performance panel's `MouseRegions`, keyed off the wheel event's row landing inside a section's click region (with focus side-effect mirroring keyboard scroll semantics).
