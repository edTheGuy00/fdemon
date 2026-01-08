## Task: Fix OR→AND Assertion Logic

**Objective**: Fix weak OR assertions in transition tests that incorrectly pass when content is partially present.

**Depends on**: None

**Priority**: Critical (required before merge)

### Scope

- `src/tui/render/tests.rs`: Lines 279, 287

### Details

The transition tests use OR (`||`) logic which can pass incorrectly:

**Line 279 - Current (incorrect):**
```rust
assert!(!before.contains("Select") || !before.contains("Device"));
// Problem: Passes if EITHER word is missing (but one could be present)
```

**Line 279 - Fixed:**
```rust
assert!(
    !before.contains("Select") && !before.contains("Device"),
    "Normal mode should not show device selector"
);
```

**Line 287 - Current (incorrect):**
```rust
assert!(after.contains("Select") || after.contains("Device") || after.contains("selector"));
// Problem: Only checks if ANY word present
```

**Line 287 - Fixed:**
```rust
// Check for device selector being visible (at least one key indicator)
assert!(
    after.contains("Select") && after.contains("Device"),
    "DeviceSelector mode should show selector dialog"
);
```

**Lines 306, 334 - Keep as-is:**
These use `"Quit" || "quit"` which is correct case-insensitive matching.

### Acceptance Criteria

1. Line 279: Uses `&&` to verify NEITHER "Select" NOR "Device" appears
2. Line 287: Uses `&&` to verify BOTH indicators appear
3. All assertions include failure messages
4. `cargo test --lib render` passes

### Testing

```bash
# Run render tests specifically
cargo test --lib render

# Verify assertions would catch bugs by temporarily breaking them
# (manual verification during implementation)
```

### Notes

- The OR→AND fix is for validating content presence/absence
- Case-insensitive OR (`"Quit" || "quit"`) is correct - don't change these
- Failure messages help debug test failures

---

## Completion Summary

**Status:** ❌ Not done

**Files Modified:**
- (pending)

**Testing Performed:**
- (pending)
