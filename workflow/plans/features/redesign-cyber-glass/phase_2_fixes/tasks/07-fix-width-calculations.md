## Task: Fix Inconsistent Width Calculations in Metadata Bars

**Objective**: Standardize width calculations in the log view's top and bottom metadata bars to use consistent character counting, fixing potential "LIVE FEED" badge misalignment.

**Depends on**: Task 02 (icons must be finalized — safe Unicode chars have different byte widths than Nerd Font glyphs)

**Review Reference**: REVIEW.md #6 (Major), ACTION_ITEMS.md #9

### Scope

- `crates/fdemon-tui/src/widgets/log_view/mod.rs:686`: Top metadata bar uses `.content.len()` (byte count)
- `crates/fdemon-tui/src/widgets/log_view/mod.rs:786`: Bottom metadata bar uses `.content.chars().count()` (char count)

### Details

**The bug**: The top metadata bar at line 686 uses `.len()` which counts bytes, not characters. For ASCII-only strings this works fine, but for the Unicode icons introduced in the redesign (even safe Unicode like "●", "⚠", "⏱"), `.len()` over-counts because these are multi-byte UTF-8 characters. This causes the padding calculation between the left label and right "LIVE FEED" badge to be off.

The bottom metadata bar at line 786 correctly uses `.chars().count()`, but this is still not perfectly accurate for double-width characters (emoji, CJK). However, `.chars().count()` is sufficient for the single-width Unicode symbols we're using.

**Fix approach**:

1. In `render_metadata_bar()` at line 686, replace:
   ```rust
   let left_text_len: usize = spans.iter().map(|s| s.content.len()).sum();
   let badge_len = right_badge.len();
   ```
   with:
   ```rust
   let left_text_len: usize = spans.iter().map(|s| s.content.chars().count()).sum();
   let badge_len = right_badge.chars().count();
   ```

2. Verify `render_bottom_metadata()` at line 786 is already using `.chars().count()` consistently — no change needed there.

### Acceptance Criteria

1. Both metadata bars use `.chars().count()` for width calculations
2. "LIVE FEED" badge is properly right-aligned in the top metadata bar
3. Bottom metadata bar layout is unchanged
4. No visual regression in normal rendering
5. `cargo check -p fdemon-tui` passes

### Testing

- Add a unit test that creates a metadata bar with Unicode characters and verifies the padding calculation is correct
- Visual inspection: "LIVE FEED" badge should be flush-right in the top metadata bar

### Notes

- Using `unicode-width` crate would be the most accurate solution for display width, but it's an additional dependency. `.chars().count()` is sufficient for the single-width Unicode symbols used in this project. If double-width emoji are ever used, `unicode-width` should be reconsidered.
- After Task 02 replaces Nerd Font icons with safe Unicode, the byte-vs-char discrepancy becomes larger (e.g., "⚠" is 3 bytes but 1 character), making this fix more important.
