## Task: Fix UTF-8 Truncation Panic Risk

**Objective**: Replace byte-based string slicing with character-based iteration in `truncate_with_ellipsis` and `truncate_middle` to prevent panics on multi-byte UTF-8 characters.

**Depends on**: None

**Estimated Time**: 20m

**Priority**: Critical

**Source**: Code Review - Risks & Tradeoffs Analyzer

### Scope

- `src/tui/widgets/new_session_dialog/mod.rs`: Fix both truncation functions

### Details

The current implementations use byte indexing (`&text[..n]`) which will panic if the slice boundary falls in the middle of a multi-byte UTF-8 character (e.g., emoji, Chinese characters).

**Current code (UNSAFE):**
```rust
// Line 48 - truncate_with_ellipsis
format!("{}...", &text[..max_width - 3])

// Lines 60-61 - truncate_middle
let start = &text[..half];
let end = &text[text.len() - half..];
```

**Required fix for truncate_with_ellipsis:**
```rust
pub fn truncate_with_ellipsis(text: &str, max_width: usize) -> String {
    let char_count = text.chars().count();
    if char_count <= max_width {
        text.to_string()
    } else if max_width <= 3 {
        ".".repeat(max_width)
    } else {
        let truncated: String = text.chars().take(max_width - 3).collect();
        format!("{}...", truncated)
    }
}
```

**Required fix for truncate_middle:**
```rust
pub fn truncate_middle(text: &str, max_width: usize) -> String {
    let char_count = text.chars().count();
    if char_count <= max_width {
        text.to_string()
    } else if max_width <= 3 {
        ".".repeat(max_width)
    } else {
        // Reserve space for "..." (3 chars)
        let available = max_width - 3;
        let half = available / 2;
        let extra = available % 2; // Give extra char to start

        let start: String = text.chars().take(half + extra).collect();
        let end: String = text.chars().skip(char_count - half).collect();
        format!("{}...{}", start, end)
    }
}
```

### Acceptance Criteria

1. Both functions use `.chars()` iteration instead of byte indexing
2. Test `test_truncate_with_ellipsis_utf8()` passes with emoji input
3. Test `test_truncate_middle_utf8()` passes with multi-byte characters
4. No panics when truncating "iPhone 🔥" at various widths
5. No panics when truncating "Pixel 日本語" at various widths
6. Existing ASCII truncation tests continue to pass

### Testing

Add new tests for UTF-8 handling:

```rust
#[test]
fn test_truncate_with_ellipsis_utf8() {
    // Emoji (4 bytes per char)
    assert_eq!(truncate_with_ellipsis("iPhone 🔥", 8), "iPhon...");
    assert_eq!(truncate_with_ellipsis("iPhone 🔥", 9), "iPhone...");
    assert_eq!(truncate_with_ellipsis("iPhone 🔥", 10), "iPhone 🔥");

    // Multi-byte chars (3 bytes per char)
    assert_eq!(truncate_with_ellipsis("日本語テスト", 6), "日本語テスト");
    assert_eq!(truncate_with_ellipsis("日本語テスト", 5), "日本...");

    // Mixed ASCII and emoji
    assert_eq!(truncate_with_ellipsis("Test 🚀 Device", 10), "Test 🚀...");
}

#[test]
fn test_truncate_middle_utf8() {
    // Emoji in name
    assert_eq!(truncate_middle("🔥Hot🔥Device🔥", 10), "🔥H...e🔥");

    // Multi-byte chars
    assert_eq!(truncate_middle("日本語デバイス", 8), "日本...イス");
}
```

### Notes

- This is a **critical fix** - the current code will crash fdemon if a user has a device with emoji in its name (common for iOS simulators)
- Use `.chars().count()` for length instead of `.len()` which returns bytes
- Emoji are typically 4 bytes; CJK characters are typically 3 bytes
- The fix prioritizes correctness over performance (acceptable for short strings like device names)

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/tui/widgets/new_session_dialog/mod.rs` | Replaced byte-based string slicing with character-based iteration in `truncate_with_ellipsis` and `truncate_middle` functions; Added comprehensive UTF-8 test cases |
| `src/app/handler/update.rs` | Fixed pattern match for `DeviceDiscoveryFailed` message (pre-existing compilation error blocking tests) |

### Notable Decisions/Tradeoffs

1. **Character-based iteration over byte-based slicing**: Changed from `&text[..n]` (byte indexing) to `.chars().take(n).collect()` (character iteration). This prevents panics on multi-byte UTF-8 boundaries but has a small performance cost for character counting. This is acceptable since these functions are used for short strings like device names.

2. **Test expectations corrected**: The original task specification had incorrect test expectations (e.g., expecting truncation at exact character count boundaries). Corrected these to reflect actual character counts and proper truncation behavior. All test assertions now pass with correct UTF-8 handling.

3. **Fixed pre-existing compilation error**: Resolved a pattern match issue in `update.rs` where `DeviceDiscoveryFailed` message was missing the `is_background` field. This was blocking test compilation but unrelated to the UTF-8 fix.

### Testing Performed

- `cargo fmt` - Passed
- `cargo check` - Passed
- `cargo clippy --lib` - Passed (no warnings)
- Manual UTF-8 verification with standalone test programs:
  - No panics when truncating "iPhone 🔥" at various widths (tested 1-15)
  - No panics when truncating "Pixel 日本語" at various widths (tested 1-15)
  - No panics when truncating "🔥Hot🔥Device🔥" at various widths (tested 1-20)
  - All UTF-8 test assertions pass (verified with standalone Rust program)
- Existing ASCII truncation tests verified to still work correctly

### Risks/Limitations

1. **Branch has pre-existing test compilation errors**: The `app/handler/tests.rs` file has errors due to missing `move_down` method on `TargetSelectorState`. These are unrelated to this UTF-8 fix and prevent running the full test suite via `cargo test`. The production code compiles and passes all quality checks.

2. **Performance consideration**: Character iteration (`.chars().count()`, `.chars().take()`) is O(n) compared to byte length which is O(1). For device name truncation (typically <50 chars), this overhead is negligible (~microseconds).

3. **Unicode width not considered**: The implementation counts Unicode scalar values (characters), not display width. Some characters (e.g., wide CJK chars, combining marks) may have different display widths. For basic device names with emoji, this is acceptable. A future enhancement could use the `unicode-width` crate for more accurate display width calculations.
