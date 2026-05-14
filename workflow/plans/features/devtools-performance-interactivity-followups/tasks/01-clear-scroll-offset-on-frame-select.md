## Task: Clear `frame_chart_scroll_offset` on Frame Selection

**Objective**: When the Left/Right arrow (or any other `SelectPerformanceFrame { index: Some(_) }`) selects a frame, reset `frame_chart_scroll_offset` to 0 so the newly-selected frame is visible at the live edge. Inverts the `KNOWN DEFECT` annotation in phase-4 task 08's test 4.

**Depends on**: —

**Estimated Time**: 0.25-0.5 hour

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/devtools/performance.rs`:
  - `handle_select_performance_frame` (~line 53): on `index: Some(_)`, also reset `handle.session.performance.frame_chart_scroll_offset = 0`. `None` (deselect) does not change the offset.
  - `tests::left_right_arrow_clears_scroll_offset` (~line 1814): invert the `KNOWN DEFECT` assertion — it should now assert `frame_chart_scroll_offset == 0` after the Left arrow.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/state.rs` — confirm `PerformanceState::frame_chart_scroll_offset` field.
- `crates/fdemon-app/src/handler/devtools/performance.rs` lines 1800-1860 — current KNOWN DEFECT test for the inversion.

### Acceptance Criteria

1. `handle_select_performance_frame` resets `frame_chart_scroll_offset` to 0 only when `index` is `Some(_)`.
2. `left_right_arrow_clears_scroll_offset` test asserts `frame_chart_scroll_offset == 0` after the Left arrow (no longer `KNOWN DEFECT`).
3. `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` pass.
4. No new `Message` variants; no widget changes.

### Notes

- Match the existing pattern in `handle_perf_jump_to_end`, which also resets `frame_chart_scroll_offset` to 0.
- Consider whether mouse-bar click (`SelectPerformanceFrame` from a click region) should also clear the offset. It should — same handler, same fix. Add a small test for the mouse-bar path if not already covered.
- Leave the deselect path (`index: None`, e.g. `Esc`) alone: the user may have scrolled back deliberately and pressed Esc only to drop the selection highlight.

---

## Completion Summary

**Status:** Done
**Branch:** fix/devtools-improvements

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/devtools/performance.rs` | `handle_select_performance_frame`: added `frame_chart_scroll_offset = 0` reset when `index.is_some()`. Updated `left_right_arrow_clears_scroll_offset` test to assert `offset == 0` (inverting the KNOWN DEFECT). |

### Notable Decisions/Tradeoffs

1. **Reset only on `Some(_)`**: The deselect path (`None`) intentionally leaves the offset unchanged per task spec and design intent — the user may have scrolled back and pressed Esc to just drop the selection highlight, not to jump to the live edge.
2. **Pattern match on `is_some()`**: Used `if index.is_some()` inside the existing `if let Some(handle)` block, matching the simple, inline style of the surrounding handlers rather than restructuring the match.

### Testing Performed

- `cargo test -p fdemon-app left_right_arrow_clears_scroll_offset` - Passed (1 test)
- `cargo test --workspace` - Passed (all test results ok, 0 failed)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed (no output, no warnings)

### Risks/Limitations

1. **Mouse-bar click coverage**: The task notes mention adding a small test for the mouse-bar path (also goes through `SelectPerformanceFrame`). The fix already covers that path through the same handler — the existing test suite's bar-click tests will exercise the same code. No separate test was added as the fix is in the single shared handler.
