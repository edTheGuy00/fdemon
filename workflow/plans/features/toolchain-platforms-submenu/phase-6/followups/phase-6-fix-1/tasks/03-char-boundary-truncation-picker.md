## Task: Fix char-boundary panics in version picker TUI widget

**Objective**: Replace two direct byte-index string slices in
`crates/fdemon-tui/src/widgets/install_wizard/version_picker.rs` with safe
character-boundary truncation so that version strings or date strings containing
multi-byte UTF-8 characters (unlikely in practice but possible for localised
manifests or unusual version tags like `1.12.13+hotfix.5`) cannot produce a
`ByteIndex not a char boundary` panic at runtime.

**Depends on**: Phase 6 Tasks 01–05 merged.

**Agent:** implementor

**Complexity:** low

**Estimated Time**: 30 minutes

### Scope

**Files Modified (Write):**
- `crates/fdemon-tui/src/widgets/install_wizard/version_picker.rs`

**Files Read (Dependencies):**
- none beyond the file itself

### Details

There are two unsafe byte-index slices inside `render_list`:

#### 1. Version string truncation (~line 211)

```rust
// UNSAFE — panics if version_max lands in a multi-byte char
let version_str = if row.version.len() > version_max && version_max > 0 {
    &row.version[..version_max]
} else {
    &row.version
};
```

Replace with a helper that uses `char_indices` to find the byte offset of the
`version_max`-th character:

```rust
/// Truncate `s` to at most `max_chars` **Unicode characters**.
/// Returns a `&str` that is always a valid char boundary.
fn truncate_chars(s: &str, max_chars: usize) -> &str {
    match s.char_indices().nth(max_chars) {
        Some((byte_pos, _)) => &s[..byte_pos],
        None => s,
    }
}
```

Then use it:

```rust
let version_str = if row.version.chars().count() > version_max && version_max > 0 {
    truncate_chars(&row.version, version_max)
} else {
    &row.version
};
```

#### 2. Date string truncation (~line 220)

```rust
// UNSAFE — panics if s[..10] spans a multi-byte char boundary
let date_str: &str = row
    .release_date
    .as_deref()
    .map(|s| if s.len() >= 10 { &s[..10] } else { s })
    .unwrap_or("");
```

Replace with:

```rust
let date_str: &str = row
    .release_date
    .as_deref()
    .map(|s| truncate_chars(s, 10))
    .unwrap_or("");
```

### Acceptance Criteria

1. Neither string slice uses a raw `[..n]` byte index when `n` was computed from
   pixel/cell width or a fixed literal; all truncation goes through
   `truncate_chars`.
2. A new test `test_no_panic_multibyte_version_string` exercises a `PickerRow`
   whose `version` is a multi-byte string (e.g. `"3.\u{1F916}0"`) with a narrow
   `version_max` derived from a small but valid render area (≥ `MIN_PICKER_WIDTH`
   + 4). The test must pass on all platforms without panic.
3. A new test `test_no_panic_multibyte_date_string` exercises a `PickerRow` with
   a `release_date` that starts with a multi-byte character. The test must render
   without panic.
4. Existing tests all continue to pass (no regression).
5. `cargo test -p fdemon-tui --lib` green; `cargo clippy --workspace --all-targets -- -D warnings` clean.

### Testing

```bash
cargo test -p fdemon-tui --lib widgets::install_wizard::version_picker
cargo test --workspace --lib
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

---

## Completion Summary

**Status:** Done
**Branch:** feat/toolchain-platforms-submenu

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/install_wizard/version_picker.rs` | Added `truncate_chars` helper; replaced `&row.version[..version_max]` byte slice with `truncate_chars(&row.version, version_max)` + changed `.len()` guard to `.chars().count()`; replaced `&s[..10]` date slice with `truncate_chars(s, 10)`; added two new tests: `test_no_panic_multibyte_version_string` and `test_no_panic_multibyte_date_string`. |

### Notable Decisions/Tradeoffs

1. **`truncate_chars` uses `char_indices().nth(n)`**: This is the idiomatic Rust way to find the byte offset of the n-th character boundary. It returns `None` when the string is shorter than `n` chars, in which case the full string is returned — no extra length check needed at call sites.
2. **`chars().count()` in the version truncation guard**: Changed the `len() > version_max` guard to `chars().count() > version_max` for consistency. The `len()` check was comparing byte length against a cell-width limit, which is only correct for ASCII. Using `chars().count()` keeps the semantics consistent with how `truncate_chars` clips — both now operate in Unicode scalar values, not bytes.
3. **Date truncation simplified**: The original `if s.len() >= 10 { &s[..10] } else { s }` idiom is replaced by the unconditional `truncate_chars(s, 10)` which handles the short-string case naturally.

### Testing Performed

- `cargo test -p fdemon-tui --lib widgets::install_wizard::version_picker` — Passed (20 tests, 2 new multibyte tests included)
- `cargo test --workspace --lib` — Passed (3079 + 514 + 1236 + 842 + 1532 = 7203 total, 0 failures)
- `cargo fmt --all -- --check` — Passed
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed (no warnings)

### Risks/Limitations

1. **Cell-width vs char-count**: For CJK double-width characters, `chars().count()` counts each character as 1 but the terminal occupies 2 columns. The version truncation still uses Unicode scalar count, not display column width. In practice, Flutter version strings are always ASCII so this is not a practical concern — the fix eliminates the panic hazard without introducing display regressions for the real-world data.
