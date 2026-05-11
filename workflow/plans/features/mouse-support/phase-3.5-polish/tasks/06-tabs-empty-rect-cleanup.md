# Task 06: Tabs empty-rect guard cleanup

**Status:** Not Started
**Estimated Hours:** 0.1h
**Depends On:** —
**Crate / Area:** `fdemon-tui`

## Goal

Discharge review item 7: `crates/fdemon-tui/src/widgets/tabs.rs:138` calls `padded_area.height.max(1)` when constructing the per-tab `MouseRect`. This bypasses the natural empty-rect guard inside `MouseRegionsBuilder::click_left_middle`, which already drops zero-height rects. Drop the `.max(1)` and let the builder do its job.

The current line:
```rust
let rect = MouseRect::new(cursor_x, padded_area.y, w, padded_area.height.max(1));
```

The fix is to either pass `padded_area.height` directly (letting the builder skip empty rects naturally), or to short-circuit at the top of the function with `if padded_area.height == 0 { return; }` for clarity.

## Files Modified (Write)

- `crates/fdemon-tui/src/widgets/tabs.rs`

## Files Read

- `crates/fdemon-app/src/mouse_regions.rs` — verify that `MouseRegionsBuilder::click_left_middle` and the underlying `is_empty` guard correctly skip rects with `height == 0` (it does — `MouseRect::is_empty` returns `width == 0 || height == 0`)

## Implementation Steps

1. **Locate the call** in `render_session_tabs`'s multi-session branch:
   ```rust
   let rect = MouseRect::new(cursor_x, padded_area.y, w, padded_area.height.max(1));
   ctx.click_left_middle(rect, ...);
   ```

2. **Drop the `.max(1)`** by passing `padded_area.height` directly:
   ```rust
   let rect = MouseRect::new(cursor_x, padded_area.y, w, padded_area.height);
   ctx.click_left_middle(rect, ...);
   ```

3. **(Optional but recommended) Add an early-return at the top of the multi-session branch** for explicitness:
   ```rust
   if padded_area.height == 0 || padded_area.width == 0 {
       return;
   }
   ```
   Add this right after the `is_empty()` early-return in `render_session_tabs`. It's redundant with the builder's `is_empty` check but documents intent.

## Acceptance Criteria

- [ ] `padded_area.height.max(1)` does not appear in `tabs.rs`
- [ ] Tab regions are constructed using the natural `padded_area.height` value
- [ ] If a zero-height rect is registered, `MouseRegionsBuilder::click_left_middle`'s built-in `is_empty` guard silently drops it (no test should fail on this — the entry simply isn't pushed)
- [ ] Existing tabs tests pass without modification: `multi_session_records_one_region_per_tab`, `nine_sessions_record_nine_tab_regions`, `divider_width_matches_rendered_buffer`, `empty_session_manager_registers_no_regions`
- [ ] `cargo test -p fdemon-tui --lib widgets::tabs` passes
- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes

## Notes

- This is a one-token deletion (`.max(1)`). The builder already skips empty rects.
- The optional early-return adds about three lines of code but makes the zero-height case explicit. Apply if you find it improves readability; skip otherwise.
- Do not change the single-session device-pill rect (`render_single_session_with_ctx`) — it already has its own `padded_area.width > 0 && padded_area.height > 0` guard and uses `padded_area.width` / `padded_area.height` directly.

---

## Completion Summary

**Status:** Done
**Branch:** feat/mouse-support

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/tabs.rs` | Removed `.max(1)` from `padded_area.height.max(1)` on line 138; added early-return guard `if padded_area.height == 0 || padded_area.width == 0 { return; }` after `padded_area` construction |

### Notable Decisions/Tradeoffs

1. **Early-return guard included**: The optional early-return guard was added for explicitness. It documents intent clearly even though `click_left_middle`'s built-in `is_empty` check would silently drop zero-height rects anyway. This matches the task's recommendation.

### Testing Performed

- `cargo test -p fdemon-tui --lib widgets::tabs` - Passed (13 tests, including the 4 acceptance-criteria tests)
- `cargo fmt --all -- --check` - Passed (no formatting issues)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed (no warnings)

### Risks/Limitations

None. This is a minimal cleanup: one token removed (`max(1)`), and three lines added (early-return guard). The builder's `is_empty` check was already correct; this change just removes the redundant workaround.
