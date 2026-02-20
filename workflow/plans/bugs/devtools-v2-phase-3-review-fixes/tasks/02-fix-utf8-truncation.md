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
