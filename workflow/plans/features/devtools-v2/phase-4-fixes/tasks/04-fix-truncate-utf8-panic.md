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
