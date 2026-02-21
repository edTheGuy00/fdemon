## Task: Fix UTF-8 byte-slice panic in string truncation

**Objective**: Replace all byte-index string truncation (`&s[..N]`) with char-based truncation to prevent panics on multi-byte UTF-8 strings.

**Depends on**: None

**Source**: Review Critical Issue #2 (Code Quality Inspector)

### Scope

- `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart.rs:681-684`: Class name truncation
- `crates/fdemon-tui/src/widgets/search_input.rs:92-93`: Regex error truncation
- `crates/fdemon-app/src/session/session.rs:518-519`: Device name truncation

### Details

All three sites use `&string[..N]` which indexes by **byte position**, not character position. This panics at runtime if byte `N` falls within a multi-byte UTF-8 codepoint.

#### Site 1: `memory_chart.rs:681-684` (HIGH risk)

```rust
// Before
let name = if class.class_name.len() > 30 {
    format!("{:.27}...", &class.class_name[..27])
} else {
    class.class_name.clone()
};

// After
let name = if class.class_name.chars().count() > 30 {
    format!("{}...", class.class_name.chars().take(27).collect::<String>())
} else {
    class.class_name.clone()
};
```

`class_name` originates from `parse_class_heap_stats()` in `fdemon-daemon/src/vm_service/performance.rs:230` — raw VM Service JSON with no sanitisation. Dart supports Unicode identifiers, and third-party packages from non-English ecosystems use CJK, Cyrillic, or emoji characters.

#### Site 2: `search_input.rs:92-93` (MEDIUM risk)

```rust
// Before
let truncated = if error.len() > 30 {
    format!("{}...", &error[..27])
} else { ... };

// After
let truncated = if error.chars().count() > 30 {
    format!("{}...", error.chars().take(27).collect::<String>())
} else { ... };
```

`error` is from `regex::Error::to_string()`. If the user's search pattern contains multi-byte characters, those appear in the error message and can straddle byte 27.

#### Site 3: `session.rs:518-519` (MEDIUM risk)

```rust
// Before
let name = if self.name.len() > 16 {
    format!("{}...", &self.name[..14])
} else { ... };

// After - use char-aware truncation with the ellipsis character
let name = if self.name.chars().count() > 16 {
    format!("{}...", self.name.chars().take(14).collect::<String>())
} else { ... };
```

Device names come from Flutter daemon JSON. Chinese Android device names (e.g., `"小米 14 Ultra"`) and emulators with diacritics are common in non-English locales.

#### Performance Note

`chars().count()` is O(n) but these strings are always short (class names, error messages, device names — typically < 100 chars). The overhead is negligible compared to the rendering cost.

For higher-performance scenarios, an alternative is `str::floor_char_boundary(N)` (stabilized in Rust 1.73):
```rust
let end = class.class_name.floor_char_boundary(27);
format!("{}...", &class.class_name[..end])
```

This is O(1) since it only needs to check the bytes around position N. However, it truncates by byte budget (not character count), so the visual width may differ from char-based truncation. Either approach is acceptable.

### Acceptance Criteria

1. No panic when `class.class_name` contains multi-byte UTF-8 characters (CJK, emoji, Cyrillic)
2. No panic when regex error message contains multi-byte characters
3. No panic when device name contains multi-byte characters
4. Truncated strings end with `"..."` and don't exceed the intended display width
5. Existing tests pass; new tests cover multi-byte edge cases

### Testing

Add tests for each truncation site:

```rust
#[test]
fn test_class_name_truncation_with_cjk() {
    // class_name with CJK characters exceeding 30 chars
    let class_name = "这是一个非常长的类名称用于测试截断功能是否正确工作";
    // Should not panic, should truncate to ~27 chars + "..."
}

#[test]
fn test_class_name_truncation_with_emoji() {
    let class_name = "MyClass🎉🎊🎈PaddingToMakeItLongEnoughToTruncate";
    // Should not panic, emoji are 4-byte sequences
}

#[test]
fn test_device_name_truncation_with_chinese() {
    let name = "小米 14 Ultra 测试设备名称";
    // Should not panic on &name[..14]
}

#[test]
fn test_search_error_truncation_with_unicode() {
    // Trigger regex error with unicode pattern
    let error = "regex parse error: pattern 'привет' is invalid because...";
    // Should not panic
}
```

### Notes

- The `session.rs` site uses a Unicode ellipsis `"…"` while the other two use `"..."`. Maintain the existing style per site.
- Consider adding a shared `truncate_chars(s: &str, max: usize) -> String` utility if this pattern appears elsewhere in the future. For now, three inline fixes are simpler than adding a utility function.
- Instance 2 (`search_input.rs`) and instance 3 (`session.rs`) are pre-existing issues discovered by the researcher — they predate the Phase 3 work but should be fixed in the same pass.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart.rs` | Replaced `&class.class_name[..27]` byte-slice with `chars().count() > 30` / `chars().take(27).collect::<String>()` |
| `crates/fdemon-tui/src/widgets/search_input.rs` | Replaced `&error[..27]` byte-slice with char-based truncation; added 4 UTF-8 test cases |
| `crates/fdemon-app/src/session/session.rs` | Replaced `&self.name[..14]` byte-slice with char-based truncation (preserving `"…"` ellipsis style) |
| `crates/fdemon-tui/src/widgets/devtools/performance/memory_chart/tests.rs` | Added 4 tests: CJK truncation, emoji truncation, ellipsis check, short-name no-truncation |
| `crates/fdemon-app/src/session/tests.rs` | Added 3 tests: Chinese device name, emoji device name, short Chinese name no-truncation |

### Notable Decisions/Tradeoffs

1. **Kept inline fixes, no shared utility**: The task notes suggest a `truncate_chars()` helper for future use but recommends inline fixes for now. All three sites are independent and the inline approach keeps the diff minimal.
2. **Preserved ellipsis style per site**: `memory_chart.rs` and `search_input.rs` use `"..."` (three ASCII dots); `session.rs` uses `"…"` (U+2026 Unicode ellipsis). Both preserved as-is.
3. **chars().count() vs floor_char_boundary**: Used `chars().count()` as specified in the task (O(n) but negligible for short strings < 100 chars). The `floor_char_boundary` alternative would truncate by byte budget rather than character count.
4. **Test string length assertion**: The initial CJK test string in `search_input.rs` had exactly 30 chars, causing the `> 30` assertion to fail. Fixed by using a 32-character CJK string.

### Testing Performed

- `cargo check -p fdemon-tui` - Passed
- `cargo check -p fdemon-app` - Passed
- `cargo test -p fdemon-tui` - Passed (604 tests)
- `cargo test -p fdemon-app` - Passed (955 tests)
- `cargo clippy -p fdemon-tui -p fdemon-app` - No errors

### Risks/Limitations

1. **None identified**: All three fixes are semantically equivalent to the original code for ASCII-only input, and now correctly handle multi-byte UTF-8 codepoints.
