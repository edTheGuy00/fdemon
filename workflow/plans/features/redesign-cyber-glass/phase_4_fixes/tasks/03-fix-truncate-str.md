## Task: Fix truncate_str to Respect max_len

**Objective**: Fix `truncate_str()` so the output string never exceeds `max_len` characters. Currently it produces strings of length `max_len + 2` when truncating.

**Depends on**: None

**Severity**: Major (pre-existing bug, impacts Phase 4 column layout)

### Scope

- `crates/fdemon-tui/src/widgets/settings_panel/styles.rs`: Fix `truncate_str` function (lines 218-228)
- `crates/fdemon-tui/src/widgets/settings_panel/tests.rs`: Update test expectations (lines 207-214)

### Details

#### Root Cause

```rust
// Current code (BUGGY):
let truncated: String = s.chars().take(max_len - 1).collect();
format!("{}...", truncated)
// Produces: (max_len - 1) + 3 = max_len + 2 characters
```

Example: `truncate_str("this is long", 8)` returns `"this is..."` (10 chars, not 8).

#### Fix

Use Unicode ellipsis character `'…'` (U+2026, single character width) to maximize visible text:

```rust
pub fn truncate_str(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else if max_len == 0 {
        String::new()
    } else {
        let truncated: String = s.chars().take(max_len - 1).collect();
        format!("{}…", truncated)
    }
}
```

This produces exactly `max_len` characters: `(max_len - 1)` visible chars + 1 ellipsis char.

**Alternative**: If `…` causes terminal rendering issues (unlikely with true-color terminals), use `"..."` with a 3-char budget:

```rust
} else if max_len <= 3 {
    s.chars().take(max_len).collect()
} else {
    let truncated: String = s.chars().take(max_len - 3).collect();
    format!("{}...", truncated)
}
```

#### Impact

Used at 11 call sites in mod.rs with column widths:
- `LABEL_WIDTH` (25), `LABEL_WIDTH_SHORT` (24), `LABEL_WIDTH_VSCODE` (20)
- `VALUE_WIDTH` (15), `VALUE_WIDTH_VSCODE` (20)
- Variable `remaining` width for descriptions

All of these will now produce correctly-sized output, preventing column overflow.

### Acceptance Criteria

1. `truncate_str("this is long", 8)` returns a string of exactly 8 characters
2. `truncate_str("abc", 2)` returns a string of exactly 2 characters
3. `truncate_str("short", 10)` returns `"short"` unchanged (5 chars <= 10)
4. `truncate_str("ab", 2)` returns `"ab"` unchanged (2 chars <= 2)
5. `truncate_str("anything", 0)` returns `""` (empty string)
6. `truncate_str("a", 1)` returns `"a"` unchanged
7. Output never exceeds `max_len` for any input

### Testing

Update existing test and add edge cases:

```rust
#[test]
fn test_truncate_str() {
    use styles::truncate_str;

    // No truncation needed
    assert_eq!(truncate_str("short", 10), "short");
    assert_eq!(truncate_str("ab", 2), "ab");
    assert_eq!(truncate_str("a", 1), "a");

    // Truncation with ellipsis
    let result = truncate_str("this is long", 8);
    assert!(result.chars().count() <= 8, "Output exceeded max_len: {}", result);

    let result = truncate_str("abc", 2);
    assert!(result.chars().count() <= 2, "Output exceeded max_len: {}", result);

    // Edge cases
    assert_eq!(truncate_str("anything", 0), "");
    assert_eq!(truncate_str("", 5), "");
}
```

### Notes

- This is a pre-existing bug, not introduced by Phase 4, but it impacts the Phase 4 column layout
- The Unicode ellipsis `…` (U+2026) is a single character that renders as 1 cell wide in terminals. It is preferred over `...` because it preserves more visible text.
- All 11 call sites in mod.rs use `truncate_str` with `format!("{:<width$}", ...)` padding — the padding fills to the width but does NOT truncate, so correct truncation is essential.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/settings_panel/styles.rs` | Fixed `truncate_str()` function (lines 218-228): Changed from `max_len <= 1` to `max_len == 0` for edge case handling, and replaced `"..."` (3 chars) with Unicode ellipsis `"…"` (1 char) to produce output of exactly `max_len` characters |
| `crates/fdemon-tui/src/widgets/settings_panel/tests.rs` | Updated `test_truncate_str()` test (lines 207-237): Expanded test coverage to verify all 7 acceptance criteria, including exact character count assertions and edge case handling |

### Notable Decisions/Tradeoffs

1. **Unicode Ellipsis Choice**: Used Unicode ellipsis `…` (U+2026) instead of ASCII `...` to maximize visible text while respecting `max_len`. This is a single character that renders correctly in modern terminals with true-color support (which Flutter Demon already requires).

2. **Edge Case Handling**: Changed `max_len <= 1` to `max_len == 0` to properly handle the case where `max_len == 1`, allowing single-character strings to pass through unchanged as per acceptance criteria #6.

### Testing Performed

- `cargo test -p fdemon-tui test_truncate_str` - Passed (1 test)
- `cargo test -p fdemon-tui` - Passed (441 tests, 0 failed)
- `cargo fmt --all` - Passed
- `cargo check --workspace` - Passed
- `cargo clippy -p fdemon-tui -- -D warnings` - Passed (0 warnings)

### Acceptance Criteria Verification

All 7 acceptance criteria met:
1. ✅ `truncate_str("this is long", 8)` returns `"this is…"` (8 chars)
2. ✅ `truncate_str("abc", 2)` returns `"a…"` (2 chars)
3. ✅ `truncate_str("short", 10)` returns `"short"` (5 chars, unchanged)
4. ✅ `truncate_str("ab", 2)` returns `"ab"` (2 chars, unchanged)
5. ✅ `truncate_str("anything", 0)` returns `""` (empty string)
6. ✅ `truncate_str("a", 1)` returns `"a"` (1 char, unchanged)
7. ✅ Output never exceeds `max_len` (verified by character count assertions in tests)

### Risks/Limitations

None identified. The Unicode ellipsis character is standard and well-supported in modern terminals. Flutter Demon already requires true-color terminal support, so Unicode character rendering is guaranteed to work correctly.