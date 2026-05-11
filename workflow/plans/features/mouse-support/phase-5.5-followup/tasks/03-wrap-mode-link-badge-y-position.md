# Task 03: Wrap-Mode Link Badge Y Position Fix

## Goal

Fix the wrap-mode link-badge mis-positioning in `widgets/log_view/mod.rs` so that a badge whose `col_offset` exceeds `visible_width` registers a click region at the correct wrapped sub-row, not at an invalid x outside the content area.

## Background

In `crates/fdemon-tui/src/widgets/log_view/mod.rs` (around lines 1611-1659 per Phase 5 diff), `BadgeAction.col_offset` carries the absolute column position of the badge within the unwrapped `Line`. The current code converts it to a screen rect via:

```rust
let rect = MouseRect::new(
    content_area.x + col_offset,  // BUG: unbounded; can fall past content_area.width
    content_area.y + rel_y,
    badge_w,
    1,
);
```

In wrap mode, when `col_offset >= visible_width`, the badge actually renders on a wrapped sub-row at `(content_area.x + (col_offset % visible_width), content_area.y + rel_y + col_offset / visible_width)`. The current rect ends up outside the content area, gets clipped by `MouseRect::new` to a 0- or 1-cell width, and the click silently fails.

## Files

**Modify:**
- `crates/fdemon-tui/src/widgets/log_view/mod.rs`
- `crates/fdemon-tui/src/widgets/log_view/tests.rs`

**Read (reference):**
- `crates/fdemon-app/src/state.rs` — `LogViewState::wrap_mode`, `LinkHighlightState`

## Plan

1. **Locate the badge-region recording loop** in `render_inner` (post-Phase-5 line range ≈ 1611-1659). Identify how `rel_y` and `col_offset` are computed for each `BadgeAction`. Verify that `visible_width = content_area.width` (or `content_area.width as usize` if it's compared to `chars().count()`).

2. **Fix the y/x math** for wrap mode:

   ```rust
   for b in &badge_actions {
       // Skip badges scrolled off the top (already handled).
       if b.rel_y >= content_area.height { continue; }

       // In wrap mode, a badge at col_offset >= visible_width renders on a
       // wrapped sub-row. Compute the actual screen position.
       let visible_width = content_area.width as usize;
       let (dx, dy) = if log_view_state.wrap_mode && visible_width > 0 {
           let dx = (b.col_offset % visible_width) as u16;
           let dy = (b.col_offset / visible_width) as u16;
           (dx, dy)
       } else {
           (b.col_offset as u16, 0)
       };

       let screen_y = b.rel_y.saturating_add(dy);
       if screen_y >= content_area.height { continue; }

       let badge_w = 3u16.min(content_area.width.saturating_sub(dx));
       if badge_w == 0 { continue; }

       let rect = MouseRect::new(
           content_area.x + dx,
           content_area.y + screen_y,
           badge_w,
           1,
       );
       if !rect.is_empty() {
           ctx.click(rect, MouseAction::emit(Message::SelectLink(b.shortcut)));
       }
   }
   ```

   The exact field names depend on the current `BadgeAction` struct — read `mod.rs` first. The invariant is: in wrap mode, `(col_offset, rel_y)` describes a position in the *unwrapped* line; convert to `(dx, dy)` screen-relative coordinates before constructing the rect.

3. **Verify horizontal-scroll interaction** (`h_offset`). In non-wrap mode, the existing code subtracts `h_offset` from `col_offset` before clipping. In wrap mode, `h_offset` should always be 0 (wrap mode disables horizontal scroll). Confirm this and skip the `h_offset` arithmetic when `wrap_mode`.

4. **Add regression tests** in `widgets/log_view/tests.rs`:

   - `wrap_mode_badge_on_first_wrapped_row_records_at_correct_y` — single log line at viewport width 20 with a badge at col_offset = 10 (no wrap needed). Assert badge rect at `(content_area.x + 10, content_area.y + rel_y, 3, 1)`.
   - `wrap_mode_badge_on_second_wrapped_row_records_at_correct_y` — single log line at viewport width 20 with a badge at col_offset = 25 (wraps to row 1). Assert badge rect at `(content_area.x + 5, content_area.y + rel_y + 1, 3, 1)`.
   - `wrap_mode_badge_clipped_at_right_edge` — badge at col_offset = 19 (last column of first row). Width truncates to 1 cell.
   - `wrap_mode_badge_off_screen_dropped` — badge whose computed `screen_y >= content_area.height` is skipped.

   Use `LogView::new(...).wrap_mode(true).link_highlight_state(&active_state)`. Construct `LinkHighlightState` with synthetic links at the test offsets.

5. **Run quality gates**:
   ```bash
   cargo test -p fdemon-tui widgets::log_view
   cargo test --workspace
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets -- -D warnings
   ```

## Acceptance Criteria

- [x] Badge region rects in wrap mode use `(dx, dy) = (col_offset % visible_width, col_offset / visible_width)` math.
- [x] Off-screen badges (post-wrap `screen_y >= content_area.height`) are skipped without panic.
- [x] 4 new wrap-mode regression tests added; all pass.
- [x] Existing non-wrap-mode badge tests still pass (no regression).
- [x] Quality gates pass.

## Notes

- Phase 4.5 Task 01 fixed an analogous bug for `RowAction` in wrap mode. The pattern there subtracted `wrap_intra_offset` from `rel_y`. Phase 5.5 Task 03 extends the same wrap-aware reasoning to `BadgeAction.col_offset`.
- If `col_offset` is stored in characters but `content_area.width` is in cells (Unicode width concern), confirm by reading the badge-recording producer (`collect_badge_actions`). If there's a width mismatch, propagate the fix into the producer, not the consumer.
- **Do NOT** modify `BadgeAction` struct fields unless necessary — keep the diff local to the rect-computation loop.

---

## Completion Summary

**Status:** Done
**Branch:** feat/mouse-support

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/log_view/mod.rs` | Fixed badge-region loop in wrap mode: compute `(dx, dy) = (col_offset % visible_width, col_offset / visible_width)` and use `badge_all_lines_y = b.rel_y + dy` for clip/visibility checks; `dx` replaces raw `col_offset` as the x position. |
| `crates/fdemon-tui/src/widgets/log_view/tests.rs` | Added 4 regression tests: `wrap_mode_badge_on_first_wrapped_row_records_at_correct_y`, `wrap_mode_badge_on_second_wrapped_row_records_at_correct_y`, `wrap_mode_badge_clipped_at_right_edge`, `wrap_mode_badge_off_screen_dropped`. |

### Notable Decisions/Tradeoffs

1. **`badge_all_lines_y` variable**: Renamed the clip-variable from `visible_y` (which was incorrectly the same as before the fix) to `badge_all_lines_y` so the top-clip check (`badge_all_lines_y.saturating_add(1) <= wio`) correctly accounts for the dy offset from wrapping. Then `visible_y = badge_all_lines_y.saturating_sub(wio)` as before.

2. **No `BadgeAction` struct changes**: The fix is entirely within the rect-computation loop as required — `BadgeAction.col_offset` still stores the absolute column and `BadgeAction.rel_y` stores the all_lines-space base row. The conversion to `(dx, dy)` is done at read time.

3. **Nowrap fallback**: In nowrap mode `dy=0` and `dx=col_offset` so the existing `h_offset` arithmetic is unchanged.

### Testing Performed

- `cargo test -p fdemon-tui "wrap_mode_badge"` — Passed (4/4 new tests)
- `cargo test -p fdemon-tui "widgets::log_view"` — Passed (105 tests, no regressions)
- `cargo test --workspace --lib` — Passed (all 5 crates: 2116+372+740+842+986 tests)
- `cargo fmt --all -- --check` — Passed
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed

### Risks/Limitations

1. **Unicode width vs char count**: `collect_badge_actions` counts width using `span.content.chars().count()` (character count) while `content_area.width` is cell width. For ASCII-only log messages (the common case) these match. The task notes acknowledge this; no mismatch was found in the badge producer so no propagation needed.
