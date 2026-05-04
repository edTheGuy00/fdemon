# Task 05: Header constants & overflow hardening

**Status:** Not Started
**Estimated Hours:** 0.25h
**Depends On:** —
**Crate / Area:** `fdemon-tui`

## Goal

Discharge three small Phase-3 review findings on `crates/fdemon-tui/src/widgets/header.rs::register_shortcut_clicks`:

1. **Magic literal `4`** (review item 5): Line 159's `(4 + label.len()) as u16` uses a bare `4` for the prefix length (`'[' + key + ']' + ' '`). Per `docs/CODE_STANDARDS.md` Principle 4 ("Every numeric threshold used in layout decisions must be a named constant"), extract a `SHORTCUT_SEGMENT_PREFIX` constant adjacent to the existing `SHORTCUT_CLICK_WIDTH`.
2. **Inconsistent saturating arithmetic** (review item 6): Line 163's overflow guard `click_x + SHORTCUT_CLICK_WIDTH > area.x + area.width` uses bare `u16` `+` while the line above uses `cursor_x.saturating_add(...)`. Make both consistent — replace the bare additions in the guard with `saturating_add`.
3. **`(4 + label.len()) as u16` cast can silently truncate** (review item 17): If a future contributor adds a label longer than `u16::MAX − 4 ≈ 65 531` chars, the cast silently wraps. Use `u16::try_from(SHORTCUT_SEGMENT_PREFIX as usize + label.len()).expect(...)` instead.

## Files Modified (Write)

- `crates/fdemon-tui/src/widgets/header.rs`

## Files Read

- (none required)

## Implementation Steps

1. **Add the new constant.** Above the existing:
   ```rust
   /// Width in terminal cells of the clickable `[X` portion of each shortcut.
   /// Only the bracket and letter are clickable, not the closing bracket or label.
   const SHORTCUT_CLICK_WIDTH: u16 = 2;
   ```
   add:
   ```rust
   /// Width in terminal cells of the non-clickable prefix of each shortcut segment:
   /// `'[' (1) + key_char (1) + ']' (1) + ' ' (1)`. The full segment is this prefix plus the
   /// trailing label text (e.g., `"Run  "`). Used in `register_shortcut_clicks` to advance the
   /// cursor between adjacent shortcuts.
   const SHORTCUT_SEGMENT_PREFIX: u16 = 4;
   ```

2. **Replace the magic `4` in the segment-width computation.** In `register_shortcut_clicks` (around line 159), change:
   ```rust
   let segment_width: u16 = (4 + label.len()) as u16;
   ```
   to:
   ```rust
   let segment_width: u16 = u16::try_from(SHORTCUT_SEGMENT_PREFIX as usize + label.len())
       .expect("shortcut label fits in u16 segment width");
   ```

3. **Make the overflow guard use `saturating_add`.** Around line 163, change:
   ```rust
   if click_x + SHORTCUT_CLICK_WIDTH > area.x + area.width {
       continue;
   }
   ```
   to:
   ```rust
   if click_x.saturating_add(SHORTCUT_CLICK_WIDTH) > area.x.saturating_add(area.width) {
       continue;
   }
   ```

## Acceptance Criteria

- [ ] `SHORTCUT_SEGMENT_PREFIX: u16 = 4` is defined alongside `SHORTCUT_CLICK_WIDTH` with a deriving comment
- [ ] No bare `4` literal appears in `register_shortcut_clicks`
- [ ] `register_shortcut_clicks`'s overflow guard uses `saturating_add` for both additions, matching the style of `cursor_x.saturating_add(segment_width)` on the surrounding line
- [ ] `segment_width` is computed via `u16::try_from(...).expect(...)` rather than `as u16`
- [ ] All existing header tests pass without modification
- [ ] `cargo test -p fdemon-tui --lib widgets::header` passes
- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes

## Notes

- This task is local to `register_shortcut_clicks` — do not touch other functions in `header.rs`.
- The `expect` message is reachable only if a future contributor adds a label longer than `u16::MAX − 4` chars, which is exceedingly unlikely. The `expect` exists to give a clear panic message rather than silent truncation.
- Do not change the public signature of `register_shortcut_clicks`.
