## Task: Fix `truncate_tag()` Panic on Multi-Byte UTF-8

**Objective**: Replace byte-level string slicing in `truncate_tag()` with character-based truncation to prevent panics when tag names contain multi-byte UTF-8 characters (CJK subsystem names, emoji, accented characters).

**Depends on**: None

**Review Issue:** #6 (Major)

### Scope

- `crates/fdemon-tui/src/widgets/tag_filter.rs`: Fix `truncate_tag` function (lines 147-155)

### Details

#### Problem

The current implementation uses byte-level slicing:

```rust
pub fn truncate_tag(tag: &str, max_len: usize) -> String {
    if tag.len() <= max_len {           // tag.len() is BYTE length
        tag.to_string()
    } else if max_len <= 3 {
        tag[..max_len].to_string()      // byte slice — panics if mid-char
    } else {
        format!("{}...", &tag[..max_len - 3])  // byte slice — panics if mid-char
    }
}
```

Both `tag[..n]` slices are byte-index operations. If `n` falls in the middle of a multi-byte UTF-8 sequence, Rust panics with `byte index N is not a char boundary`. This is in the render path — a panic here crashes the entire TUI.

**Example panic:** Tag `"日本語tag"` with `max_len = 4`. `tag.len()` = 12 bytes (3×3 + 3×1), exceeds 4. The `else` branch runs `&tag[..1]` (since `max_len - 3 = 1`). Byte index 1 is inside the 3-byte `日` codepoint — panic.

The guard `tag.len() <= max_len` is also wrong: it compares byte length against a character-count limit, so a 4-character tag with multi-byte chars would bypass the guard and render wider than `max_len` columns.

#### Fix

Replace with character-based operations:

```rust
pub fn truncate_tag(tag: &str, max_len: usize) -> String {
    let char_count = tag.chars().count();
    if char_count <= max_len {
        tag.to_string()
    } else if max_len <= 3 {
        tag.chars().take(max_len).collect()
    } else {
        let truncated: String = tag.chars().take(max_len - 3).collect();
        format!("{}...", truncated)
    }
}
```

### Acceptance Criteria

1. `truncate_tag` does not panic on any valid UTF-8 input
2. Multi-byte characters are handled correctly:
   - `truncate_tag("日本語タグ", 4)` → `"日..."` (1 char + `...`)
   - `truncate_tag("日本語", 3)` → `"日本語"` (fits exactly)
   - `truncate_tag("abc", 3)` → `"abc"` (ASCII fits)
   - `truncate_tag("abcdef", 5)` → `"ab..."` (2 chars + `...`)
3. Existing tests still pass: `cargo test -p fdemon-tui -- truncate_tag`
4. `cargo clippy -p fdemon-tui -- -D warnings` passes

### Testing

Update existing tests and add multi-byte cases:

```rust
#[test]
fn test_truncate_tag_multibyte_utf8() {
    // CJK characters (3 bytes each)
    assert_eq!(truncate_tag("日本語タグ名", 5), "日本...");
    assert_eq!(truncate_tag("日本語", 3), "日本語");
    assert_eq!(truncate_tag("日本語", 2), "日本");  // max_len <= 3

    // Mixed ASCII and multi-byte
    assert_eq!(truncate_tag("Go日本", 4), "G...");

    // Emoji (4 bytes each)
    assert_eq!(truncate_tag("🔥🔥🔥", 2), "🔥🔥");  // max_len <= 3
}
```

### Notes

- This is a **render-path crash** — any tag with multi-byte characters crashes the TUI. The fix is simple and low-risk.
- The `chars().count()` and `chars().take()` approach is O(n) in the length of the tag, but tag names are short strings (typically < 50 chars), so this is negligible.
- Note: `chars().count()` counts Unicode scalar values, not grapheme clusters. For display-width accuracy, `unicode-width` crate would be better, but that's a broader concern and out of scope for this bug fix.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/tag_filter.rs` | Replaced byte-level slicing in `truncate_tag` with `chars().count()` / `chars().take()`. Updated doc comment. Added `test_truncate_tag_multibyte_utf8` test with CJK, mixed, and emoji cases. Corrected one test assertion from the task spec (`"Go日本"` has 4 chars and fits exactly at `max_len=4`; added a 5-char variant `"Go日本語"` to cover the truncation path). |

### Notable Decisions/Tradeoffs

1. **Task spec assertion correction**: The spec asserted `truncate_tag("Go日本", 4) == "G..."`. "Go日本" is exactly 4 Unicode scalar values, so with the correct char-based guard (`char_count <= max_len`) it is returned unchanged as `"Go日本"`. The test was corrected to assert `"Go日本"` and a separate 5-char case `"Go日本語"` was added to exercise the truncation path to `"G..."`.

2. **`chars().count()` vs grapheme clusters**: The fix uses Unicode scalar values (not grapheme clusters). The task notes this is acceptable and out of scope; display-width accuracy with `unicode-width` is a separate concern.

### Testing Performed

- `cargo test -p fdemon-tui -- truncate_tag` - Passed (7 tests)
- `cargo clippy -p fdemon-tui -- -D warnings` - Passed (no warnings)

### Risks/Limitations

1. **Display-width vs char count**: Multi-byte characters like CJK glyphs occupy 2 terminal columns each. `chars().count()` counts scalar values, not display columns. This is a pre-existing limitation noted in the task spec and out of scope for this fix.
