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

**Status:** Not Started

**Files Modified:**
- (pending)

**Implementation Details:**
(pending)

**Testing Performed:**
(pending)

**Notable Decisions:**
(pending)

**Risks/Limitations:**
(pending)
