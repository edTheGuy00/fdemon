## Task: Keyboard Handlers for Performance Tab Interactivity

**Objective**: Bind `Tab`/`Shift+Tab`, `j/k`/arrows, `PageUp/Down`, `Home/End` on the Performance tab. Route them to per-section handlers that update `PerformanceState`.

**Depends on**: Phase 1 (PerfSection + state fields + Message variants)

**Estimated Time**: 4-5 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/keys.rs`:
  - Add `in_performance` guard alongside existing `in_inspector` / `in_network`.
  - Bind keys (under `in_performance`):
    - `Tab` → `Message::PerfFocusSection(state.focused_section.next())`
    - `Shift+Tab` → `Message::PerfFocusSection(state.focused_section.prev())`
    - `Up` / `k` → `Message::PerfScrollUp`
    - `Down` / `j` → `Message::PerfScrollDown`
    - `PageUp` → `Message::PerfPageUp`
    - `PageDown` → `Message::PerfPageDown`
    - `Home` → `Message::PerfJumpToStart`
    - `End` → `Message::PerfJumpToEnd`
  - Preserve existing Left/Right (frame selection) and `s` (sort toggle) bindings.
- `crates/fdemon-app/src/handler/devtools/performance.rs`: Add handler functions:
  - `handle_perf_focus_section(state, section: PerfSection)` — sets `perf_state.focused_section = section`.
  - `handle_perf_scroll(state, direction: ScrollDir)` — branch on `focused_section`:
    - `FrameChart`: adjust `frame_chart_scroll_offset`, clamp to `[0, frame_history.len() - frame_chart_visible_width.get().max(1)]`.
    - `MemoryChart`: same against `memory_samples`.
    - `MemoryList`: move `alloc_table_selected_row`; if selection scrolls off-screen (using `alloc_table_visible_height` hint), adjust `alloc_table_scroll_offset`.
  - `handle_perf_page(state, direction)` — same as scroll but shifts by `visible_height`/`width` (fallback = 10 if hint == 0).
  - `handle_perf_jump_to_start(state)` — set offsets to max-back; for list, set selection to last row.
  - `handle_perf_jump_to_end(state)` — set offsets to 0 (live edge); for list, set selection to row 0.
  - `handle_perf_select_alloc_row(state, index: Option<usize>)` — set `alloc_table_selected_row = index`; set `focused_section = MemoryList`.
- `crates/fdemon-app/src/handler/update.rs`: Route the 7 new `Message` variants to the handler functions above. Add a sub-section comment for readability.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/session/performance.rs`: Field access + `PerfSection`.
- `crates/fdemon-app/src/message.rs`: `Message` variants.

### Details

`ScrollDir` is a small enum local to the handler:

```rust
enum ScrollDir { Up, Down }
```

Scroll-bound calculation skeleton:

```rust
fn clamp_chart_scroll(buffer_len: usize, visible_width: usize, current: usize, delta: i64) -> usize {
    let max_back = buffer_len.saturating_sub(visible_width.max(1));
    let new = current as i64 + delta;
    new.clamp(0, max_back as i64) as usize
}
```

For the list, scrolling adjusts selection first, then nudges `alloc_table_scroll_offset` to keep selection visible — same pattern as Network panel.

### Acceptance Criteria

1. `Tab`/`Shift+Tab` cycles focus visibly in handler-level tests.
2. `Up/Down/j/k` scrolls focused section. Bounds respected (no negative offsets, no scrolling past buffer length).
3. `PageUp/Down` shifts by visible_height/width.
4. `Home`/`End` jumps correctly.
5. Selecting an alloc row sets `focused_section = MemoryList`.
6. All 7 new `Message` variants routed in `update.rs`.
7. Unit tests cover each handler + edge cases (offset clamp at 0, at max).
8. `cargo test --workspace` and `cargo clippy -- -D warnings` pass.

### Testing

```rust
#[test]
fn perf_scroll_down_in_frame_chart_increments_offset() {
    let mut state = AppState::test_default();
    let sid = state.add_test_session_in_devtools_performance();
    let perf = active_perf_state(&mut state).unwrap();
    perf.frame_chart_visible_width.set(50);
    push_frames(perf, 1000);
    perf.focused_section = PerfSection::FrameChart;

    handle_perf_scroll(&mut state, ScrollDir::Down);

    assert_eq!(active_perf_state(&state).unwrap().frame_chart_scroll_offset, 1);
}

#[test]
fn perf_scroll_clamps_to_buffer_bounds() {
    let mut state = AppState::test_default();
    let perf = active_perf_state(&mut state).unwrap();
    perf.frame_chart_visible_width.set(50);
    push_frames(perf, 100);
    perf.frame_chart_scroll_offset = 50; // already at max-back

    handle_perf_scroll(&mut state, ScrollDir::Down);

    assert_eq!(active_perf_state(&state).unwrap().frame_chart_scroll_offset, 50);
}

#[test]
fn perf_jump_to_end_resets_to_live_edge() { /* ... */ }
#[test]
fn perf_focus_section_message_updates_state() { /* ... */ }
#[test]
fn perf_select_alloc_row_focuses_memory_list() { /* ... */ }
```

### Notes

- Use the existing `last_known_visible_height` pattern (Principle 3 of CODE_STANDARDS.md) — if hint == 0, fall back to a constant like `DEFAULT_PERF_PAGE_SIZE = 10`.
- Do not duplicate existing Left/Right or `s` handlers — they remain untouched.
- Be careful with `in_performance` guard placement — must precede generic `in_devtools` handlers.

---

## Completion Summary

**Status:** Done
**Branch:** fix/devtools-improvements

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/devtools/performance.rs` | Added `ScrollDir` enum, `clamp_chart_scroll` helper, `DEFAULT_PERF_PAGE_SIZE` constant, and 6 new public handler functions (`handle_perf_focus_section`, `handle_perf_scroll`, `handle_perf_page`, `handle_perf_jump_to_start`, `handle_perf_jump_to_end`, `handle_perf_select_alloc_row`) plus private helpers (`alloc_row_count`, `scroll_alloc_table`). Added 42 unit tests. |
| `crates/fdemon-app/src/handler/keys.rs` | Added `in_performance` guard block before main `match key` in `handle_key_devtools`; binds Tab/Shift+Tab → `PerfFocusSection`, Up/k/Down/j → `PerfScrollUp/Down`, PageUp/PageDown → `PerfPageUp/Down`, Home → `PerfJumpToStart`, End → `PerfJumpToEnd`. |
| `crates/fdemon-app/src/handler/update.rs` | Replaced 7 Phase 2 stub `UpdateResult::none()` arms with real routing calls to the new handler functions. |

### Notable Decisions/Tradeoffs

1. **ScrollDir semantics for charts vs lists**: For charts, "Up" increments the scroll offset (scrolls back in time) and "Down" decrements it (toward live edge). For the MemoryList, "Down" increments the row index (moves toward end) and "Up" decrements it. This matches standard TUI conventions where Up/Down move the visual cursor, but charts scroll their viewport.

2. **in_performance guard is an early-return block**: Rather than polluting the main `match` with many `if in_performance` guards, all 8 new bindings are handled in an early-return `if in_performance { match key { ... } }` block before the main match. This avoids conflicts with Tab (session navigation in normal devtools) and j/k (inspector navigation).

3. **Left/Right and `s` bindings preserved untouched**: The existing frame selection (Left/Right) and sort toggle (`s`) bindings remain in the main `match` with their `in_performance` guards as before — not moved into the new guard block.

4. **`scroll_alloc_table` shared helper**: Both `handle_perf_scroll` and `handle_perf_page` delegate list scrolling to a private `scroll_alloc_table(perf, direction, steps)` function, avoiding duplication.

### Testing Performed

- `cargo test -p fdemon-app` — PASS (2241 tests, 0 failed)
- `cargo test --workspace` — PASS (all crates)
- `cargo clippy --workspace -- -D warnings` — PASS (0 warnings)

### Risks/Limitations

1. **Tab key now consumed by Performance panel**: When in the Performance panel, Tab/Shift+Tab cycles sections instead of switching DevTools sub-panels. Users must use `i`, `p`, `n` keys to change panels. This is the intended behavior per task spec.

2. **alloc_table_scroll_offset nudge uses `visible_height - 1` formula**: The keep-visible logic uses `visible_height - 1` as the scroll threshold. If `visible_height` is 1, this correctly keeps the single visible row in frame.
