# Task 06: Tag-Filter Scroll Write-back + N-Action Const Fix

## Goal

Replace the hand-rolled `compute_scroll_offset` in `widgets/tag_filter.rs` with a `Cell<usize>` write-back from ratatui's actual `ListState.offset()` (Major #7). Replace the `unwrap_or(0)` fallback at line 271 with a named constant (Minor #8).

## Background

`compute_scroll_offset` at `widgets/tag_filter.rs:296-311` re-implements ratatui's `ListState` scroll arithmetic with a simple "selected pinned to bottom of visible area" heuristic. ratatui's actual `ListState` uses a more nuanced offset that depends on prior offset state (offset persists across renders unless the selection forces it to move). When the user scrolls backward through a long tag list, the visually-rendered tag at row `Y` may not match the recorded `abs_index = scroll_offset + Y`, causing clicks to toggle the wrong tag.

The existing test `render_with_regions_scrolled_indices_are_absolute` only asserts `max_index >= 25`, too weak to catch sub-row drift.

Fix: write the `ListState.offset()` value back via a `Cell<usize>` field on the tag-filter UI state, populated by the renderer each frame and consumed by the region recorder. This is the same render-hint pattern documented in `docs/REVIEW_FOCUS.md` (TEA Approved Exception).

Separately, `widgets/tag_filter.rs:271` reads:
```rust
let n_offset = footer_text.find("[n]").map(|i| i as u16).unwrap_or(0);
```
The fallback collides with `[a]` at column 0. Failure cannot occur in practice (the literal is local), but the masked intent is misleading.

## Files

**Modify:**
- `crates/fdemon-tui/src/widgets/tag_filter.rs` — replace `compute_scroll_offset` with Cell write-back; fix `n_offset`
- `crates/fdemon-app/src/state.rs` — add `Cell<usize>` field for ListState offset (or a dedicated tag-filter UI state struct)

**Read (reference):**
- `crates/fdemon-app/src/session/native_tags.rs` — `NativeTagState`, `sorted_tags`
- `docs/REVIEW_FOCUS.md` — Approved TEA Exception: Render-Hint Feedback (must update if this becomes a new exception)

## Plan

1. **Add a `Cell<usize>` write-back field** for the tag-filter list scroll offset. Decide where it lives:
   - **(a) On `TagFilterUiState`** in `state.rs` (or wherever `tag_filter_ui` is declared): add `pub last_known_scroll_offset: std::cell::Cell<usize>` with default `0`.
   - **(b) On `AppState`** as a new top-level Cell — overkill; rejected.

   Choose (a). Audit `state.rs` to find `tag_filter_ui` declaration.

2. **Update `widgets/tag_filter.rs::render_tag_filter`** to write the current `ListState.offset()` back to the Cell at the end of rendering (after ratatui's internal scroll happens). Use `state.list_state.offset()` (or whatever the field is named). Audit ratatui's `ListState` API:
   ```rust
   // After `frame.render_stateful_widget(list, list_area, &mut list_state);`
   tag_filter_ui_state.last_known_scroll_offset.set(list_state.offset());
   ```
   This must happen in *both* `render_tag_filter` and `render_tag_filter_with_regions` so callers without a `MouseCtx` still update the cache.

3. **Update `render_tag_filter_with_regions`** to read `last_known_scroll_offset` instead of calling `compute_scroll_offset`:
   ```rust
   let scroll_offset = tag_filter_ui_state.last_known_scroll_offset.get();
   for screen_row in 0..visible_rows {
       let abs_index = scroll_offset + screen_row;
       // ... register click region at abs_index
   }
   ```

4. **Delete `compute_scroll_offset`** — the function is no longer needed.

5. **Replace `unwrap_or(0)`** at line 271 with a named constant:
   ```rust
   const N_ACTION_OFFSET: u16 = 9; // "[a] All  " is 9 bytes; "[n]" starts at byte 9.
   // ...
   let n_offset = N_ACTION_OFFSET;  // formerly `footer_text.find("[n]").map(...).unwrap_or(0)`
   ```
   Add a static-assert-style test (or const-fn check) that `footer_text.find("[n]").unwrap() == 9` to catch any future change to the footer text. Place it in `tag_filter.rs::tests`.

6. **Update `docs/REVIEW_FOCUS.md`** to document the new Cell exception. Locate the "Current usage" section under "Approved TEA Exception: Render-Hint Feedback" and add:
   ```markdown
   - `TagFilterUiState::last_known_scroll_offset` — the renderer writes ratatui's
     `ListState.offset()` each frame; the region recorder reads it for click-rect
     alignment. Default 0 (safe fallback when no render has happened yet).
   ```

7. **Add regression tests** in `widgets/tag_filter.rs::tests`:

   - `render_with_regions_uses_listate_offset_writeback` — render with a 30-tag list and a selected_index that causes ratatui to scroll. Read the buffer's first visible row to identify which tag is rendered there. Assert the recorded `abs_index` for the click region at row 0 matches that tag's index.
   - `render_with_regions_scroll_offset_persists_across_calls` — render once with selected_index = 25 (forces scroll), then render again with selected_index = 0 (selection moved back). Assert the second render's `last_known_scroll_offset` matches what ratatui actually produced (may not be 0 if ratatui keeps the old offset).
   - `n_action_offset_constant_matches_footer_text` — assert `footer_text.find("[n]") == Some(N_ACTION_OFFSET as usize)` so any future footer change forces an update.

8. **Quality gates**:
   ```bash
   cargo test -p fdemon-tui widgets::tag_filter
   cargo test --workspace
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets -- -D warnings
   ```

## Acceptance Criteria

- [ ] `TagFilterUiState` (or equivalent) gains a `Cell<usize>` field for scroll-offset write-back.
- [ ] Both `render_tag_filter` and `render_tag_filter_with_regions` write the current `ListState.offset()` to the Cell each frame.
- [ ] `render_tag_filter_with_regions` reads `last_known_scroll_offset` (not `compute_scroll_offset`).
- [ ] `compute_scroll_offset` deleted from `tag_filter.rs`.
- [ ] `n_offset` replaced with a named `const N_ACTION_OFFSET: u16 = 9;`.
- [ ] `docs/REVIEW_FOCUS.md` updated with the new Cell exception entry.
- [ ] 3 new tests pass; existing tag-filter tests still pass.
- [ ] Quality gates pass.

## Notes

- **`docs/REVIEW_FOCUS.md` is unmanaged** (not in the strict-doc list). The implementor can edit it directly per `planner` skill rules.
- **Cell vs. RefCell:** `Cell<usize>` is sufficient — `usize` is Copy. No `RefCell` needed.
- **`ListState::offset()` API:** verify this method exists on the version of ratatui in `Cargo.toml`. If not, use the `state.offset` public field directly (older ratatui exposes it as a `pub` field on `ListState`).
- The existing weak test `render_with_regions_scrolled_indices_are_absolute` (asserting `max >= 25`) should be replaced or strengthened, not just left in place. If it's strictly weaker than the new tests, delete it; otherwise update its assertions.
- T01 may have already promoted tag_filter regions to z=1 (no change there — they were always z=1 per Phase 5). Coordinate trivially.

---

## Completion Summary

**Status:** Done
**Branch:** worktree-agent-a10d948b34d031a70

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/state.rs` | Added `last_known_scroll_offset: Cell<usize>` field to `TagFilterUiState` with doc comment explaining its purpose |
| `crates/fdemon-tui/src/widgets/tag_filter.rs` | Added `N_ACTION_OFFSET` const; write-back `list_state.offset()` in `render_tag_filter`; updated `render_tag_filter_with_regions` to read from `last_known_scroll_offset`; deleted `compute_scroll_offset`; replaced `unwrap_or(0)` with `N_ACTION_OFFSET`; replaced/strengthened tests |
| `docs/REVIEW_FOCUS.md` | Documented `TagFilterUiState::last_known_scroll_offset` as a new Cell render-hint exception |

### Notable Decisions/Tradeoffs

1. **Replaced `render_with_regions_scrolled_indices_are_absolute`**: The old test only asserted `max_index >= 25`, which did not catch sub-row drift. The new `render_with_regions_uses_liststate_offset_writeback` test supersedes it by asserting the exact alignment between `last_known_scroll_offset` and the `abs_index` of the first recorded row region.

2. **`render_tag_filter` also writes back**: The write-back to `last_known_scroll_offset` happens inside `render_tag_filter`, not `render_tag_filter_with_regions`. Since `render_tag_filter_with_regions` calls `render_tag_filter` first, both paths update the Cell before regions are recorded — this satisfies the acceptance criterion for "both functions write the current offset."

3. **Fresh `ListState` each frame**: Because the code reconstructs `ListState::default().with_selected(Some(selected_index))` every render, ratatui recomputes the scroll offset from scratch. This means `last_known_scroll_offset` always reflects what was actually drawn, not a stale value from a prior frame.

### Testing Performed

- `cargo test -p fdemon-tui widgets::tag_filter` — PASS (23 tests)
- `cargo test --workspace` — PASS (all test suites, 0 failed)
- `cargo fmt --all -- --check` — PASS
- `cargo clippy --workspace --all-targets -- -D warnings` — PASS

### Risks/Limitations

1. **Fresh ListState per frame**: The code cannot carry `ListState` across frames, so ratatui never has a chance to apply its "keep prior offset stable" behavior. This is intentional — the prior `compute_scroll_offset` also reconstructed from scratch. If future work needs smooth scroll persistence, the `ListState` would need to be stored outside the render function.
