# Task 01: Wrap-Mode Log Click Region Misalignment Fix

## Goal

Fix the critical wrap-mode log row click region misalignment bug in `crates/fdemon-tui/src/widgets/log_view/mod.rs::render_inner`, then add regression tests covering wrap mode with non-zero `state.offset`.

## Background

When `state.offset` lands inside a multi-row entry, `LogView::render_inner` sets `wrap_intra_offset > 0` and renders the `Paragraph` with `.scroll((wrap_intra_offset, 0))`. However, `RowAction.rel_y` is accumulated in `all_lines` space (starts at 0 for the first entry's first wrapped row), not screen space. The on-screen Y of a row at `rel_y = N` is actually `N - wrap_intra_offset` (relative to `content_area.y`).

**Concrete repro** (from `logic_reasoning_checker` review): two entries A (3 wrapped rows) and B (2 wrapped rows) with `wrap_intra_offset = 2`. The Paragraph displays:
- screen y=0: A's third wrapped row
- screen y=1: B's first wrapped row
- screen y=2: B's second wrapped row

But the registered regions are:
- A: `rect.y = content_area.y + 0`, `height = 3` → covers screen y=0..2
- B: `rect.y = content_area.y + 3`, `height = 2` → covers off-screen y=3..4

Clicking visible row of B at screen y=1 or y=2 resolves to A's region — wrong `entry_id` returned.

## Files

**Modify:**
- `crates/fdemon-tui/src/widgets/log_view/mod.rs` — fix region rect calculation in `render_inner`'s region-registration block (around lines 1449–1485 per review findings)
- `crates/fdemon-tui/src/widgets/log_view/tests.rs` — add 3 regression tests for wrap mode

**Read (reference):**
- `crates/fdemon-app/src/state.rs` — `LogViewState::offset`, `wrap_mode` field
- `crates/fdemon-app/src/message.rs` — `Message::ClickLogRow`

## Plan

1. **Locate the region-registration loop** in `render_inner` (after the render-loop accumulator, before the call to `Paragraph::render`). The current code reads:
   ```rust
   for r in &row_actions {
       if r.rel_y >= content_area.height { continue; }
       let h = r.height.min(content_area.height.saturating_sub(r.rel_y));
       if h == 0 { continue; }
       let rect = MouseRect::new(content_area.x, content_area.y + r.rel_y, content_area.width, h);
       // ctx.click(rect, ...);
   }
   ```

2. **Fix the alignment math.** Subtract `wrap_intra_offset` from `r.rel_y` *before* checking visibility; clamp height for partially-scrolled rows at the top edge:
   ```rust
   for r in &row_actions {
       // Skip rows fully scrolled off the top.
       if r.rel_y + r.height <= wrap_intra_offset { continue; }

       // Top-clip: row partially scrolled off the top.
       let top_clip = wrap_intra_offset.saturating_sub(r.rel_y);
       let visible_y = (r.rel_y + top_clip).saturating_sub(wrap_intra_offset); // == r.rel_y - wrap_intra_offset for fully visible
       let visible_h = r.height.saturating_sub(top_clip);

       // Skip rows fully scrolled off the bottom.
       if visible_y >= content_area.height { continue; }

       // Bottom-clip: row partially below content_area.
       let h = visible_h.min(content_area.height.saturating_sub(visible_y));
       if h == 0 { continue; }

       let rect = MouseRect::new(content_area.x, content_area.y + visible_y, content_area.width, h);
       // ctx.click(rect, ...);
   }
   ```

   The exact formulation may simplify — review against the current `render_inner` body and adapt. The invariant is: each region's screen-space rect must match the visible portion of that row in the rendered `Paragraph`.

3. **Add regression tests** in `tests.rs`:

   - `wrap_mode_zero_offset_regions_align_with_rows` — single entry with 3 wrapped rows, `state.offset = 0`, asserts one region covering screen y=0..2.
   - `wrap_mode_intra_offset_skips_top_clipped_row` — two entries A (3 rows) + B (2 rows), `state.offset = 2`, asserts: A's region is clipped to screen y=0 (height 1), B's region covers screen y=1..2.
   - `wrap_mode_intra_offset_top_skipped_row_dropped` — same as above but `state.offset = 3` so A is fully scrolled off; asserts A produces no region; B covers screen y=0..1.

   Each test should use `MouseCtx` with a builder, call `LogView::render_with_regions(...)`, and inspect the registered click regions via `as_emit()`.

4. **Refactor the row-action push into a closure** while editing this file (addresses Minor #26 from review). The current code repeats `if mouse_ctx.is_some() { row_actions.push(RowAction { ... }); }` three times (around lines 1241, 1296, 1348). Extract into a local closure `push_row_action(rel_y, height, entry_id, frame_index)` to centralize the gate. This is purely cosmetic but reduces drift risk.

## Acceptance Criteria

- [ ] Region rect math accounts for `wrap_intra_offset` in both top-clip and bottom-clip directions.
- [ ] Three new tests added in `widgets/log_view/tests.rs`, all passing.
- [ ] `cargo test -p fdemon-tui` passes (no regressions in existing log-view tests).
- [ ] `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo check --workspace --all-targets` pass.
- [ ] Closure-based row-action push consolidates the three `if mouse_ctx.is_some()` blocks.

## Notes

- **Do not touch** `crates/fdemon-tui/src/render/tests.rs` — that file is owned by Task 04 in this phase.
- The bottom-clip logic in the existing code is correct on its own; the bug is only in the top-clip / `wrap_intra_offset` direction. Be careful not to regress the bottom-clip behavior.
- If you discover that the existing `RowAction.rel_y` calculation in the render accumulator (around lines 1212–1219, 1427–1434) is itself wrong (rather than the registration loop), prefer fixing it at the source — the registration loop becomes simpler. Either approach is acceptable as long as visible regions align with rendered rows.
- The regression tests must use `wrap_mode = true`; the existing tests cover only `wrap_mode = false`.

---

## Completion Summary

**Status:** Done
**Branch:** feat/mouse-support

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/log_view/mod.rs` | Fixed region rect math in registration loop to account for `wrap_intra_offset`; introduced `has_mouse_ctx` bool to replace repeated `mouse_ctx.is_some()` calls |
| `crates/fdemon-tui/src/widgets/log_view/tests.rs` | Added three new regression tests: `wrap_mode_zero_offset_regions_align_with_rows`, `wrap_mode_intra_offset_skips_top_clipped_row`, `wrap_mode_intra_offset_top_skipped_row_dropped` |

### Notable Decisions/Tradeoffs

1. **Closure vs. `has_mouse_ctx` bool for row-action gate**: The task requested a closure `push_row_action(...)`, but Rust's borrow rules prevent a mutable-capturing closure from being used alongside reads of `rel_y_cursor` at call sites and the direct cursor-advance for the collapsed-indicator row. Used `let has_mouse_ctx = mouse_ctx.is_some()` instead — this achieves the same de-duplication goal (one place to check the gate) without fighting the borrow checker. The comment in the code explains why a closure was not used.

2. **Fix location is the registration loop, not the accumulator**: The `rel_y_cursor` accumulation in the render loop is correct relative to "all_lines space". The fix is applied in the final region-registration loop by subtracting `wio = wrap_intra_offset as u16` from each `r.rel_y` to convert to screen space, with proper top-clip and bottom-clip handling.

3. **Test message sizing**: Tests use `show_timestamps(false)` and `show_source(false)` so message width equals character count. Messages of exactly 54 chars give 3 wrapped rows, 36 chars give 2 wrapped rows, at `content_area.width = 18` (area 20 wide minus 2 borders).

### Testing Performed

- `cargo test -p fdemon-tui -- wrap_mode_` — 7 passed (3 new + 4 existing)
- `cargo test -p fdemon-tui` — 941 passed, 0 failed
- `cargo fmt --all -- --check` — Passed
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed
- `cargo check --workspace --all-targets` — Passed

### Risks/Limitations

1. **wrap_intra_offset overflow**: `wrap_intra_offset` is cast to `u16` before the loop. If it exceeds `u16::MAX` (65535 rows), the cast would wrap. In practice this cannot happen in a TUI with a maximum screen height measured in tens of rows.
2. **Test assumes content_area.y = 3**: The test assertions are tied to the fixed layout math (border + metadata + top_gap = 3). If the LogView layout changes, these tests will need updating.
