## Task: Fix `truncate()` Panic on Multi-byte UTF-8

**Objective**: Replace the byte-level string slicing in `request_table.rs::truncate()` with the existing Unicode-safe `truncate_str()` function to prevent panics on non-ASCII URLs.

**Depends on**: None
**Severity**: HIGH
**Review ref**: REVIEW.md Issue #4

### Scope

- `crates/fdemon-tui/src/widgets/devtools/network/request_table.rs`: Replace `truncate()` with `truncate_str()`
- `crates/fdemon-tui/src/widgets/devtools/network/request_table.rs`: Update tests

### Root Cause

The `truncate` function (line ~340-346) uses `&s[..max.saturating_sub(1)]` which is byte-level slicing. This panics when the slice point falls within a multi-byte UTF-8 character. A URL like `https://api.example.com/réservations` would trigger this.

A safe `truncate_str` already exists in `crates/fdemon-tui/src/widgets/devtools/mod.rs` (line ~320-330) using `char_indices()`. The Inspector module already imports and uses it.

### Fix

In `request_table.rs`, replace the local `truncate()` function with a wrapper around the existing `truncate_str`:

```rust
/// Truncate `s` to at most `max` characters, appending `…` when truncated.
pub(super) fn truncate(s: &str, max: usize) -> String {
    let truncated = super::truncate_str(s, max.saturating_sub(1));
    if truncated.len() < s.len() {
        format!("{truncated}…")
    } else {
        s.to_string()
    }
}
```

Note: `truncate_str` returns `&str` (zero allocation when within bounds), so the wrapper only allocates when actually truncating. The `…` character takes 1 column, hence `max.saturating_sub(1)` to leave room.

Alternatively, if the `truncate` function is only used internally, consider replacing all call sites with direct `truncate_str` usage and removing the function entirely.

Verify that the `super::truncate_str` import works — it's declared `pub(super)` in `devtools/mod.rs` which is the parent of the `network/` module.

### Tests

Update existing truncate tests and add:
1. Test with multi-byte UTF-8 characters (e.g., `"héllo"`, `"日本語テスト"`)
2. Test where slice boundary falls mid-character
3. Test empty string and zero max
4. Verify the `…` ellipsis is appended when truncated

### Verification

```bash
cargo test -p fdemon-tui -- truncate
cargo test -p fdemon-tui -- request_table
cargo clippy -p fdemon-tui
```

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/devtools/network/request_table.rs` | Replaced byte-level `truncate()` body with Unicode-safe wrapper using `super::super::truncate_str()`. Added 10 new tests covering UTF-8 safety, edge cases, and ellipsis behavior. |

### Notable Decisions/Tradeoffs

1. **`super::super::truncate_str` path**: The task mentioned `super::truncate_str`, but `request_table.rs` is in `devtools::network`, so `super` is `network` and `super::super` is `devtools`. Used `super::super::truncate_str` matching the pattern already used in `request_details.rs` in the same package.

2. **Correctness fix for task's suggested implementation**: The task's suggested wrapper used `truncated.len() < s.len()` (byte lengths) to detect truncation. This produced a false positive for strings that were exactly `max` chars long (e.g., `"hello"` with `max=5` — byte lengths differ when `truncate_str` is given `max-1`). Replaced with `s.chars().count() <= max` guard which correctly avoids truncation when the string fits.

3. **Single additional allocation**: `chars().count()` traverses the string once to count chars. For strings that fit (no truncation needed), this is the only cost — no allocation. For strings that need truncation, `truncate_str` does a second traversal to find the char boundary, then `format!` allocates. This is acceptable for a UI widget called per-frame.

### Testing Performed

- `cargo test -p fdemon-tui -- truncate` — Passed (37 tests)
- `cargo test -p fdemon-tui -- request_table` — Passed (36 tests)
- `cargo clippy -p fdemon-tui -- -D warnings` — Passed (no warnings)

### Risks/Limitations

1. **`chars().count()` performance**: For very long strings (e.g., multi-megabyte URIs), `chars().count()` is O(n). In practice, HTTP URIs and network widget data are short, so this is not a concern.
