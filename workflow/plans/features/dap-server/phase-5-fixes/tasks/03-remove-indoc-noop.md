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

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/ide_config/helix.rs` | Deleted `indoc()` function (lines 160-164); replaced `indoc(r#"..."#)` call in `dart_debugger_toml()` with `r#"..."#.to_string()` |

### Notable Decisions/Tradeoffs

1. **No behavior change**: The `indoc()` function was literally just `s.to_string()`, so the replacement is semantically identical. The raw string content is unchanged.
2. **Pre-existing compile errors not in scope**: The branch has pre-existing `E0050` trait signature mismatches in `helix.rs`, `neovim.rs`, and `zed.rs` (from a prior task adding `project_root` to `merge_config`). These prevent `cargo check` from passing but are unrelated to this task's scope.

### Testing Performed

- `cargo fmt --all` - Passed (no output)
- `cargo check --workspace` - Failed due to pre-existing trait signature mismatch errors in `helix.rs`, `neovim.rs`, `zed.rs` (not caused by this task)
- `cargo test -p fdemon-app -- helix` - Not runnable due to pre-existing compile errors in the workspace
- Verified via `git diff` that changes are exactly the removal of `indoc()` and substitution of `.to_string()` at the call site
- Verified no `indoc` references remain in `helix.rs` via grep

### Risks/Limitations

1. **Pre-existing compile errors**: The workspace does not compile due to `merge_config` signature mismatches in other files (from a different task). The helix tests cannot be run until those are fixed.
