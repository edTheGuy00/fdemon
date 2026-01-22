## Task: Extract Scroll Indicator Width Threshold to Constant

**Objective**: Replace hardcoded `50` for scroll indicator text switching with a named constant.

**Depends on**: None

**Estimated Time**: 10m

**Priority**: Minor

**Source**: Code Review - Code Quality Inspector

### Scope

- `src/tui/widgets/new_session_dialog/device_list.rs`: Extract constant and update both usages

### Details

The value `50` is used to determine whether to show compact ("↑"/"↓") or verbose ("↑ more"/"↓ more") scroll indicators. This magic number appears twice in the file.

**Current code (lines 138-142, 305-309):**
```rust
// In ConnectedDeviceList::render_scroll_indicators
let (up_indicator, down_indicator) = if area.width < 50 {
    ("↑", "↓")
} else {
    ("↑ more", "↓ more")
};

// In BootableDeviceList::render_scroll_indicators (same code)
let (up_indicator, down_indicator) = if area.width < 50 {
    ("↑", "↓")
} else {
    ("↑ more", "↓ more")
};
```

**Required fix:**
```rust
/// Minimum width (in columns) to show verbose scroll indicators ("↑ more").
/// Below this threshold, compact indicators ("↑") are shown.
const VERBOSE_INDICATOR_WIDTH_THRESHOLD: u16 = 50;

// In both render_scroll_indicators functions:
let (up_indicator, down_indicator) = if area.width < VERBOSE_INDICATOR_WIDTH_THRESHOLD {
    ("↑", "↓")
} else {
    ("↑ more", "↓ more")
};
```

### Acceptance Criteria

1. Named constant `VERBOSE_INDICATOR_WIDTH_THRESHOLD` defined at module level
2. Doc comment explains what the constant controls
3. Both usages of `50` replaced with the constant
4. No code duplication for the constant (define once, use twice)
5. Existing tests continue to pass
6. No functional behavior change

### Testing

No new tests needed - this is a refactoring for code clarity. Run existing tests:

```bash
cargo test device_list
cargo test scroll_indicator
```

### Notes

- This is a pure refactoring with no functional change
- The value `50` is reasonable - verbose text adds ~4 chars which fits in wider terminals
- Consider if this should be in a shared constants module if used elsewhere
- Could potentially derive from layout constants in the future

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/tui/widgets/new_session_dialog/device_list.rs` | Added module-level constant `VERBOSE_INDICATOR_WIDTH_THRESHOLD` and replaced both hardcoded `50` values with the constant |

### Notable Decisions/Tradeoffs

1. **Single constant definition**: Defined the constant once at module level (line 22) and used it in both `ConnectedDeviceList::render_scroll_indicators` (line 142) and `BootableDeviceList::render_scroll_indicators` (line 309)
2. **Doc comment clarity**: Added comprehensive documentation explaining that the constant controls the threshold for switching between compact and verbose scroll indicators
3. **No behavioral change**: This is a pure refactoring - the value and logic remain identical, only improving code maintainability

### Testing Performed

- `cargo fmt` - Passed
- `cargo build` - Passed (6.88s)
- `cargo test device_list` - Passed (14/14 tests)
- `cargo test scroll_indicator` - Passed (3/3 tests)
- `cargo clippy` - Passed (no warnings)

### Risks/Limitations

1. **None identified**: This is a straightforward refactoring with no functional changes
2. **Constant location**: The constant is scoped to the module since it's only used within device_list.rs. If this threshold needs to be shared across other widgets in the future, it could be moved to a shared constants module
