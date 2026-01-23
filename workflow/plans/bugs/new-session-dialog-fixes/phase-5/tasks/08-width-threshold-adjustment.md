## Task: Test and Adjust Width Threshold for Mode Labels

**Objective**: Verify that the `MODE_FULL_LABEL_MIN_WIDTH` threshold (48) works correctly with borders, and adjust if needed.

**Priority**: Major

**Depends on**: Tasks 1-6 (code changes should be complete first)

### Scope

- `src/tui/widgets/new_session_dialog/launch_context.rs`: `MODE_FULL_LABEL_MIN_WIDTH` constant (line 824)
- Manual testing at widths 48, 49, 50 columns

### Problem Analysis

The review noted that `MODE_FULL_LABEL_MIN_WIDTH = 48` was set before borders added 2 columns of overhead. The threshold may need adjustment to 50.

**Current constant:**
```rust
const MODE_FULL_LABEL_MIN_WIDTH: u16 = 48;
```

**Mode labels:**
- Short: "Debug", "Release", "Profile"
- Full: "Debug Mode", "Release Mode", "Profile Mode"

**Border overhead:** 2 columns (left border + right border)

### Verification Steps

1. **Test at width 48:**
   ```bash
   printf '\e[8;30;48t'  # Set terminal to 48 columns, 30 rows
   cargo run
   # Press 'n' to open dialog
   # Check mode label display
   ```

2. **Test at width 49:**
   ```bash
   printf '\e[8;30;49t'
   # Repeat test
   ```

3. **Test at width 50:**
   ```bash
   printf '\e[8;30;50t'
   # Repeat test
   ```

4. **Document observations:**
   - At width 48: Mode labels show as [short/full]? Any wrapping/overflow?
   - At width 49: Mode labels show as [short/full]? Any wrapping/overflow?
   - At width 50: Mode labels show as [short/full]? Any wrapping/overflow?

### Expected Behavior

| Width | Border Overhead | Available | Mode Label |
|-------|-----------------|-----------|------------|
| 48 | 2 | 46 | Short ("Debug") |
| 49 | 2 | 47 | Short ("Debug") |
| 50 | 2 | 48 | Full ("Debug Mode") |

### If Adjustment Needed

**Increase threshold to account for borders:**

```rust
// src/tui/widgets/new_session_dialog/launch_context.rs

// BEFORE:
const MODE_FULL_LABEL_MIN_WIDTH: u16 = 48;

// AFTER:
/// Minimum width to show full mode labels ("Debug Mode" vs "Debug").
/// Accounts for 2-column border overhead in compact mode.
const MODE_FULL_LABEL_MIN_WIDTH: u16 = 50;
```

**Or make it dynamic based on border presence:**

```rust
fn should_use_full_labels(&self, area_width: u16, has_borders: bool) -> bool {
    let base_threshold = 48;
    let border_overhead = if has_borders { 2 } else { 0 };
    area_width >= base_threshold + border_overhead
}
```

### Acceptance Criteria

1. Mode labels display correctly at all tested widths
2. No label wrapping or overflow at threshold boundaries
3. Full labels appear at appropriate width (48+ available content space)
4. If adjustment made, documented with rationale
5. All existing tests pass

### Testing

```bash
cargo test launch_context
cargo test mode_label
```

Manual testing at widths 48, 49, 50 is the primary verification.

### Notes

- The threshold affects user experience but is not a crash risk
- Consider adding a constant comment explaining the threshold choice
- This pairs with Task 07 (vertical space) for complete compact mode validation

---

## Completion Summary

**Status:** Done

### Analysis Results

The threshold of 48 is **already correct** and does not need adjustment. The constant applies to the **inner content area** (after borders are subtracted), not the total widget width.

**Testing Results:**

| Width | Border Overhead | Inner Width | Mode Label Display | Behavior |
|-------|----------------|-------------|-------------------|----------|
| 48 | 2 | 46 | Abbreviated ("Dbg", "Prof", "Rel") | Correct |
| 49 | 2 | 47 | Abbreviated ("Dbg", "Prof", "Rel") | Correct |
| 50 | 2 | 48 | Full ("Debug", "Profile", "Release") | Correct |

**Adjustment Needed:** No

The threshold works correctly because:
1. `MODE_FULL_LABEL_MIN_WIDTH = 48` is compared against the inner area width (after border deduction)
2. When the widget has a total width of 50, the inner width is 48, meeting the threshold
3. This provides the desired behavior: full labels appear when 48+ columns of content space are available

### Files Modified

| File | Changes |
|------|---------|
| `src/tui/widgets/new_session_dialog/launch_context.rs` | Added documentation comment explaining that the threshold applies to inner content area and requires total width of 50 with borders |
| `src/tui/widgets/new_session_dialog/launch_context.rs` | Added test `test_mode_inline_with_borders_threshold` to verify correct behavior at widths 48, 49, and 50 |

### Notable Decisions/Tradeoffs

1. **No constant adjustment**: The threshold of 48 is already correct. It applies to the inner content area, not the total widget width. With 2-column border overhead, this naturally requires a total width of 50 to show full labels.

2. **Added clarifying documentation**: Updated the constant's doc comment to explicitly state that it applies to the inner content area and that a widget with borders requires a total width of 50.

3. **Added comprehensive test**: Created `test_mode_inline_with_borders_threshold` to verify the correct behavior with borders at the critical widths (48, 49, 50). This ensures the threshold continues to work correctly even if refactoring occurs.

### Testing Performed

- `cargo fmt` - Passed
- `cargo check` - Passed
- `cargo test launch_context` - Passed (34 tests)
- `cargo clippy -- -D warnings` - Passed (no warnings)

**Test Verification:**
- Width 48 with borders (inner 46): Shows abbreviated labels ("Dbg", "Prof", "Rel") ✓
- Width 49 with borders (inner 47): Shows abbreviated labels ("Dbg", "Prof", "Rel") ✓
- Width 50 with borders (inner 48): Shows full labels ("Debug", "Profile", "Release") ✓

### Risks/Limitations

None. The threshold is working as designed and provides the expected responsive behavior.
