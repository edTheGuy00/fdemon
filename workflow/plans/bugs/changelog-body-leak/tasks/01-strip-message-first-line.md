## Task: Strip commit message to first line in build.rs

**Objective**: Fix `website/build.rs` to extract only the first line of `commit.message` before using it as the changelog description, preventing squash-merge bodies from leaking into the website changelog.

**Depends on**: None

**Estimated Time**: 0.5-1 hour

### Scope

- `website/build.rs:138`: Add first-line extraction before `escape(&upper_first(...))`

### Details

The current code at `website/build.rs:138`:

```rust
let desc = escape(&upper_first(&commit.message));
```

Uses the full `commit.message` field from git-cliff's JSON context, which for squash-merged PRs contains the PR title **plus** every individual commit message in the body.

The fix extracts only the first line, matching `cliff.toml`'s Markdown template filter (`commit.message | split(pat="\n") | first`):

```rust
let first_line = commit.message.lines().next().unwrap_or("").trim();
let desc = escape(&upper_first(first_line));
```

Additionally, add a `#[cfg(test)]` module to `build.rs` with unit tests for the `generate_entries()` function covering:

1. Multi-line message (squash-merge with body) — only first line used
2. Single-line message — unchanged behavior
3. Empty/whitespace message — graceful handling
4. Message with `\r\n` line endings — first line extracted correctly
5. Verify `upper_first` and `escape` still apply correctly to the extracted line

### Acceptance Criteria

1. `commit.message` with embedded newlines produces a description containing only the first line
2. Single-line messages are unaffected
3. Empty messages produce an empty description (no panic)
4. `cargo check` passes for the website crate
5. All new unit tests pass

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(message: &str, group: &str) -> VersionEntry {
        VersionEntry {
            version: Some("v1.0.0".to_string()),
            timestamp: Some(1740000000),
            commits: vec![Commit {
                message: message.to_string(),
                group: Some(group.to_string()),
                scope: None,
            }],
        }
    }

    #[test]
    fn multiline_message_uses_first_line_only() {
        let entry = make_entry(
            "feat: add widget\n\nThis is the body\nWith multiple lines",
            "Features",
        );
        let code = generate_entries(&[entry]);
        assert!(code.contains("Add widget"));
        assert!(!code.contains("This is the body"));
    }

    #[test]
    fn single_line_message_unchanged() {
        let entry = make_entry("fix: resolve crash", "Bug Fixes");
        let code = generate_entries(&[entry]);
        assert!(code.contains("Resolve crash"));
    }

    #[test]
    fn empty_message_no_panic() {
        let entry = make_entry("", "Features");
        let code = generate_entries(&[entry]);
        // Should not panic, produces empty or minimal entry
        assert!(code.contains("ChangelogChange"));
    }
}
```

### Notes

- `build.rs` is a build script — tests may need to be structured carefully since `build.rs` files don't normally run under `cargo test`. Consider extracting testable functions or running tests via a separate test file.
- The `escape()` function handles `\` and `"` but not `\n`. After the fix, newlines are stripped before reaching `escape()`, so this is no longer a concern.
- v0.1.0 entries used individual (non-squash) commits with single-line subjects, so this change is a no-op for them.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `website/build.rs` | Lines 133-135: replaced single `let desc = escape(&upper_first(&commit.message))` with two-line first-line extraction (`commit.message.lines().next().unwrap_or("").trim()`) before passing to `upper_first`/`escape`. |
| `website/tests/changelog_gen.rs` | New integration test file: replicates the helper functions from `build.rs` (since build scripts are not compiled during `cargo test`) and adds 5 unit tests covering multi-line, single-line, empty, CRLF, and escape/capitalise cases. |

### Notable Decisions/Tradeoffs

1. **Test file in `tests/` not inline in `build.rs`**: `build.rs` is compiled as a standalone binary by Cargo and its `#[cfg(test)]` blocks are not executed by `cargo test`. The standard approach is to replicate the testable logic in a `tests/` integration test file. The functions in `tests/changelog_gen.rs` are kept in sync with `build.rs` manually.

2. **Test inputs use git-cliff stripped messages**: git-cliff strips the conventional-commit prefix (`feat:`, `fix:`, …) before writing the `message` field to JSON, so test inputs use the post-strip description (e.g. `"add widget"` not `"feat: add widget"`). This matches production data and makes `upper_first` assertions predictable.

3. **`cargo fmt` applied**: Running `cargo fmt` reformatted some pre-existing lines in `build.rs` (the `sort_by` closure and a `format!` call); these are style-only changes that don't affect behaviour.

### Testing Performed

- `cargo check` (website crate) - Passed
- `cargo test` (website crate) - Passed (5 new tests)
- `cargo clippy` (website crate) - Passed (2 pre-existing warnings in unrelated files, no new warnings)
- `cargo fmt --check` - Passed after running `cargo fmt`

### Risks/Limitations

1. **Test duplication**: The helper functions in `tests/changelog_gen.rs` are a manual copy of the ones in `build.rs`. If `build.rs` logic changes (e.g. `escape`, `upper_first`, `group_order`), the test file must be updated in sync.
