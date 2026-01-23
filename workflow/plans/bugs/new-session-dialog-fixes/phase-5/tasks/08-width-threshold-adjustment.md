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

**Status:** Not Started

**Testing Results:**

| Width | Mode Label Display | Issues |
|-------|-------------------|--------|
| 48 | | |
| 49 | | |
| 50 | | |

**Adjustment Needed:** Yes/No

**Files Modified:**
(If adjustments were made)
