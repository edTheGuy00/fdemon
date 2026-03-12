## Task: Fix `scroll_offset` Dead State in Tag Filter UI

**Objective**: Either wire `scroll_offset` to the rendering path so the tag filter overlay scrolls correctly with 15+ tags, or remove it as dead state. Currently `TagFilterUiState.scroll_offset` is declared and reset but never read during rendering — navigating past the visible window scrolls the selection off-screen.

**Depends on**: None

**Review Issue:** #4 (Major)

### Scope

- `crates/fdemon-app/src/state.rs`: `TagFilterUiState` struct and methods
- `crates/fdemon-tui/src/widgets/tag_filter.rs`: `render_tag_filter` function
- `crates/fdemon-app/src/handler/update.rs`: Tag filter message handlers (lines 2060-2074)

### Details

#### Problem

`TagFilterUiState` declares `scroll_offset: usize` (line 838) and resets it to 0 in `reset()` (line 857), but:

1. `move_up()` and `move_down()` only update `selected_index` — they never touch `scroll_offset`
2. `render_tag_filter` in `tag_filter.rs` reads only `ui_state.selected_index` (line 112) to highlight the selected row. It passes all items to a plain `List::new(items)` without applying any scroll offset. No `ratatui::widgets::ListState` is used.
3. The result: with 15+ tags, navigating down past the visible area moves `selected_index` beyond the rendered items, and the selection disappears off-screen.

#### Fix: Option A — Wire It Up (Recommended)

Follow the project's Responsive Layout Guidelines (Principle 3 in `docs/CODE_STANDARDS.md`): use `ratatui::widgets::ListState` with a `Cell<usize>` render-hint for the visible height.

**Step 1: Update `TagFilterUiState`** in `state.rs`:
- Remove `scroll_offset: usize`
- Add `last_known_visible_height: Cell<usize>` with default 0

**Step 2: Update `move_up`/`move_down`** to accept a `visible_height` parameter and keep the selected item within the scroll window. Or calculate scroll adjustment using `last_known_visible_height.get()`.

**Step 3: Update `render_tag_filter`** in `tag_filter.rs`:
- Write `ui_state.last_known_visible_height.set(available_height)` each frame (with the TEA render-hint exception comment)
- Use `ratatui::widgets::ListState` with `select(Some(ui_state.selected_index))` and render via `frame.render_stateful_widget(list, area, &mut list_state)`
- ListState handles scroll-following automatically when a selection is set

**Step 4: Update handler** in `update.rs`:
- Read `state.tag_filter_ui.last_known_visible_height.get()` if needed for scroll adjustment in key handlers

#### Fix: Option B — Remove Dead State

If tag filter is capped at a reasonable number (e.g., TAG_FILTER_MAX_VISIBLE_TAGS = 15), remove `scroll_offset` entirely:

1. Remove `scroll_offset` from `TagFilterUiState`
2. Remove the `self.scroll_offset = 0` from `reset()`
3. Update test struct literals that set `scroll_offset: 0`
4. Add a doc comment noting the tag cap

This is simpler but means the overlay breaks with 15+ tags (possible with many native plugins or subsystems).

#### Recommendation

**Option A is recommended** because:
- The project's own guidelines (Principle 3) mandate `Cell<usize>` render-hints for scrollable lists
- `ratatui::widgets::ListState` handles scroll-following automatically — minimal custom logic needed
- The number of native tags is unbounded (each iOS framework can produce its own tag)

### Acceptance Criteria

1. Navigating down past the visible area in the tag filter overlay keeps the selected item visible
2. No `scroll_offset` dead state remains (either wired or removed)
3. `cargo test --workspace --lib` passes
4. `cargo clippy --workspace -- -D warnings` passes

### Testing

```rust
#[test]
fn test_tag_filter_scroll_follows_selection() {
    // Create state with 20 tags, move_down 18 times
    // Verify selected_index is at 18
    // If Option A: verify render-hint is updated and list renders the correct visible window
    // If Option B: verify selected_index is clamped to max visible
}
```

### Notes

- If Option A is chosen, add the standard TEA exception comment: `// EXCEPTION: TEA render-hint write-back via Cell — see docs/CODE_STANDARDS.md`
- The existing `TAG_FILTER_MAX_VISIBLE_TAGS` constant may need adjustment if the overlay now scrolls.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/state.rs` | Added `use std::cell::Cell`; replaced `scroll_offset: usize` with `last_known_visible_height: Cell<usize>`; updated doc comment; removed `self.scroll_offset = 0` from `reset()` |
| `crates/fdemon-tui/src/widgets/tag_filter.rs` | Added `ListState` to ratatui imports; added render-hint write-back (`ui_state.last_known_visible_height.set(visible_height)`) with TEA exception comment; replaced `render_widget(list, ...)` with `render_stateful_widget(list, ..., &mut list_state)` using `ListState::default().with_selected(Some(ui_state.selected_index))`; updated all test struct literals from `scroll_offset: 0` to `..Default::default()`; added two new tests |

### Notable Decisions/Tradeoffs

1. **Option A chosen (ListState)**: `ratatui::widgets::ListState::with_selected(Some(index))` causes ratatui to automatically scroll the list viewport to keep the selected item visible. This eliminates all custom scroll-offset arithmetic and is the canonical ratatui pattern for scrollable lists.

2. **No highlight_style set on List**: The per-item styling already applies the accent highlight to the selected row. Setting a `highlight_style` on `List` would double-apply styling on the selected row. Leaving it unset preserves the existing visual appearance while getting ratatui's scroll-following for free.

3. **Handler unchanged**: `update.rs` has no code that reads `scroll_offset`, so no handler changes were needed. The `last_known_visible_height` field is available if future scroll-clamping logic is needed.

### Testing Performed

- `cargo fmt --all -- --check` — Passed
- `cargo check --workspace` — Passed
- `cargo test --workspace --lib` — 817 passed; 4 pre-existing snapshot failures (version string `v0.1.0` vs `v0.2.1`, unrelated to this task); +3 new tests from this task
- `cargo clippy --workspace -- -D warnings` — Passed (zero warnings)

### Risks/Limitations

1. **Snapshot test failures are pre-existing**: The 4 `render::tests::snapshot_*` failures existed before this change and are due to a version string mismatch (`v0.1.0` vs `v0.2.1`) in snapshot files, not related to scroll offset changes.
