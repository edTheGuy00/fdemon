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
