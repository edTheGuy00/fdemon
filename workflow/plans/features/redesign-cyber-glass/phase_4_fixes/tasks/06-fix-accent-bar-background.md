## Task: Fix Accent Bar Losing Selected Row Background

**Objective**: Ensure the accent bar character `▎` on selected rows preserves the `SELECTED_ROW_BG` background color instead of resetting it to default.

**Depends on**: None

**Severity**: Minor (subtle visual glitch — leftmost cell has different background than rest of row)

### Scope

- `crates/fdemon-tui/src/widgets/settings_panel/mod.rs`: Fix 2 accent bar rendering locations

### Details

#### Root Cause

In `render_setting_row()` (line 432-447), the selected row background is applied first:
```rust
// Step 1: Fill entire row with SELECTED_ROW_BG
if is_selected {
    let bg_style = styles::selected_row_bg();
    for col in x..x + width {
        if let Some(cell) = buf.cell_mut((col, y)) {
            cell.set_style(bg_style);
        }
    }
}

// Step 2: Render accent bar (OVERWRITES the cell at position col)
if is_selected {
    let bar = Span::styled("▎", styles::accent_bar_style());
    buf.set_line(col, y, &Line::from(bar), 1);  // Replaces cell entirely
}
```

`buf.set_line()` calls `Cell::set_style()` which **replaces** the entire cell style. Since `accent_bar_style()` returns `Style::default().fg(palette::ACCENT)` with no background, the background resets to default at that cell.

Same issue exists in `render_user_pref_row()` (line 661):
```rust
buf.set_string(col, y, "▎", styles::accent_bar_style());
```

#### Fix

Use `buf.cell_mut()` to set only the foreground color while preserving the existing background:

```rust
// BEFORE (render_setting_row, line 444-447):
if is_selected {
    let bar = Span::styled("▎", styles::accent_bar_style());
    buf.set_line(col, y, &Line::from(bar), 1);
}

// AFTER:
if is_selected {
    if let Some(cell) = buf.cell_mut((col, y)) {
        cell.set_symbol("▎");
        cell.set_fg(palette::ACCENT);
    }
}
```

Or alternatively, create a combined style that includes both fg and bg:

```rust
// Alternative: combined style
if is_selected {
    let bar_style = Style::default()
        .fg(palette::ACCENT)
        .bg(palette::SELECTED_ROW_BG);
    let bar = Span::styled("▎", bar_style);
    buf.set_line(col, y, &Line::from(bar), 1);
}
```

Apply the same fix to `render_user_pref_row()` (line 661).

#### Locations

| Line | Function | Current Code |
|------|----------|--------------|
| 444-447 | `render_setting_row()` | `buf.set_line(col, y, &Line::from(bar), 1)` |
| 661 | `render_user_pref_row()` | `buf.set_string(col, y, "▎", styles::accent_bar_style())` |

### Acceptance Criteria

1. Selected rows have a consistent `SELECTED_ROW_BG` background across all cells, including the accent bar cell
2. Accent bar `▎` character displays in `palette::ACCENT` foreground color
3. Unselected rows are unaffected (no accent bar rendered)
4. Existing accent bar tests still pass

### Testing

The existing test `test_selected_row_has_tinted_background` should verify the background at the accent bar position. If it doesn't check that specific cell, extend it:

```rust
#[test]
fn test_accent_bar_preserves_selected_background() {
    // Render a selected setting row
    // Check that the accent bar cell (col 0) has:
    //   - fg: palette::ACCENT
    //   - bg: palette::SELECTED_ROW_BG
}
```

### Notes

- The `▎` character (U+258E LEFT THREE EIGHTHS BLOCK) visually fills most of the cell, so the background color is only visible at the rightmost sliver. The fix is still important for visual consistency.
- The `cell_mut` approach is preferred because it explicitly preserves existing cell state, matching the rendering pattern used for the background fill pass.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/settings_panel/mod.rs` | Fixed 2 accent bar rendering locations (lines 443-446 and 661-664) to use `cell_mut()` approach, preserving `SELECTED_ROW_BG` background color while setting `palette::ACCENT` foreground |

### Implementation Details

1. **Location 1 (`render_setting_row()`, lines 443-446)**:
   - Replaced `buf.set_line(col, y, &Line::from(bar), 1)` with `cell_mut()` approach
   - Now sets only symbol and foreground color, preserving existing background

2. **Location 2 (`render_user_pref_row()`, lines 661-664)**:
   - Replaced `buf.set_string(col, y, "▎", styles::accent_bar_style())` with `cell_mut()` approach
   - Same approach as Location 1 for consistency

### Notable Decisions/Tradeoffs

1. **cell_mut() over combined style**: Used `buf.cell_mut()` to modify only the symbol and foreground color, rather than creating a combined style with both fg and bg. This approach:
   - Explicitly preserves the existing background that was set in the first rendering pass
   - Matches the pattern used for the background fill (also uses cell_mut)
   - Is more explicit about what's being changed vs preserved
   - Avoids potential issues if SELECTED_ROW_BG constant changes in the future

2. **Left accent_bar_style() function in place**: Did not remove the now-unused `accent_bar_style()` function from `styles.rs` as per instructions (Task 04 handles styles.rs cleanup)

### Testing Performed

- `cargo check -p fdemon-tui` - Passed (with expected dead_code warning for accent_bar_style)
- `cargo test -p fdemon-tui` - Passed (446 tests, 0 failed)
- `cargo clippy -p fdemon-tui` - Passed (only expected dead_code warning)

All existing tests pass, including:
- `test_selected_row_has_tinted_background` - Verifies background consistency
- `test_unselected_row_has_no_accent_bar` - Verifies no accent bar when not selected
- All other settings panel tests (42 total)

### Acceptance Criteria Verification

1. Selected rows have consistent `SELECTED_ROW_BG` background across all cells, including the accent bar cell - **PASS** (cell_mut preserves background)
2. Accent bar `▎` character displays in `palette::ACCENT` foreground color - **PASS** (set via cell.set_fg)
3. Unselected rows are unaffected (no accent bar rendered) - **PASS** (only renders when is_selected)
4. Existing accent bar tests still pass - **PASS** (all 446 tests pass)

### Risks/Limitations

None. The fix is localized to 2 rendering locations and preserves all existing behavior while fixing the visual glitch.
