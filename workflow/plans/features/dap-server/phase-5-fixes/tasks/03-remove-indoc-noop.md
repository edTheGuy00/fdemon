## Task: Remove `indoc()` No-Op Function

**Objective**: Delete the `indoc()` function from `helix.rs` which claims to strip leading newlines but just calls `.to_string()`. Replace the single call site with a direct `.to_string()` call.

**Depends on**: None

**Severity**: Major

### Scope

- `crates/fdemon-app/src/ide_config/helix.rs`: Delete `indoc()` function, update call site

### Details

**Current code** (lines 160-164):
```rust
/// Strip a leading `\n` from a string literal used with `indoc!`-style
/// indentation. This is a minimal helper to keep the raw string readable.
fn indoc(s: &str) -> String {
    s.to_string()
}
```

**Single call site** — `dart_debugger_toml()` (line ~88):
```rust
fn dart_debugger_toml() -> String {
    indoc(
        r#"# fdemon DAP configuration for Helix ..."#,
    )
}
```

**Fix:**
1. Delete the `indoc` function (lines 160-164)
2. Replace the call site:
   ```rust
   fn dart_debugger_toml() -> String {
       r#"# fdemon DAP configuration for Helix ..."#.to_string()
   }
   ```

### Acceptance Criteria

1. `indoc()` function removed from `helix.rs`
2. `dart_debugger_toml()` returns the same string content as before
3. `cargo test -p fdemon-app` — all helix tests pass unchanged

### Testing

- Existing helix generator tests verify the content of `dart_debugger_toml()` output — they serve as the regression guard.

### Notes

- This is a trivial cleanup. The function is dead logic with a misleading doc comment.

---

## Completion Summary

**Status:** Not Started
