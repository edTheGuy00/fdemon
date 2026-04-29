## Task: Normalise VS Code `cwd` field to forward slashes

**Objective**: Make `compute_cwd` in `vscode.rs` return a forward-slash path on every platform, so the generated `.vscode/launch.json` `cwd` field is portable and matches VS Code's cross-platform JSON convention.

**Depends on**: None

**Estimated Time**: 0.5 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/ide_config/vscode.rs`: Production fix in `compute_cwd` (and any sibling site that emits a path into the JSON output).

**Files Read (Dependencies):**
- None — change is contained in one file.

### Details

#### Why forward-slash

`.vscode/launch.json` is a JSON file consumed by VS Code on every OS. JSON string values use `\` as the escape character, so Windows backslashes in `cwd` are technically valid (each must be escaped to `\\`) but are awkward and the project-cross-platform convention is forward slash. VS Code resolves forward-slash `cwd` correctly on Windows. Forward slash is also what the test suite asserts (`"example/app3"`).

#### Production fix

`compute_cwd` currently does:

```rust
fn compute_cwd(project_root: &Path, workspace_root: &Path) -> String {
    // ... canonicalize both sides ...
    match canonical_project.strip_prefix(&canonical_workspace) {
        Ok(rel) => rel.to_string_lossy().into_owned(),
        Err(_) => /* absolute fallback */,
    }
}
```

`rel.to_string_lossy()` emits OS-native separators. Replace with a normalisation step:

```rust
match canonical_project.strip_prefix(&canonical_workspace) {
    Ok(rel) => rel.to_string_lossy().replace('\\', "/"),
    Err(_) => /* absolute fallback — also normalise */,
}
```

Apply the same normalisation to the absolute-path fallback branch — a Windows absolute path serialised as a JSON string should also use forward slashes.

If `vscode.rs` has any other site that emits a path into the generated JSON (e.g., the `program` or `args[]` fields), apply the same `.replace('\\', "/")` there. Search for `to_string_lossy()` and `.display().to_string()` within the file before finishing.

#### Tests already correct

The two failing tests (`test_compute_cwd_project_is_child_returns_relative_path`, `test_vscode_monorepo_cwd_is_relative_path`) already assert forward-slash literals. They pass automatically once the production fix lands. **Do not modify the tests.**

### Acceptance Criteria

1. `compute_cwd` returns a string using `/` as the path separator on every platform.
2. Any other path-to-JSON emission sites in `vscode.rs` use the same normalisation.
3. `test_compute_cwd_project_is_child_returns_relative_path` passes on Linux, macOS, and Windows.
4. `test_vscode_monorepo_cwd_is_relative_path` passes on Linux, macOS, and Windows.
5. `cargo clippy -p fdemon-app --all-targets -- -D warnings` exits 0.
6. `cargo test -p fdemon-app` passes.
7. `cargo fmt --all -- --check` is clean.
8. No tests in `vscode.rs` are modified — only production code.

### Testing

```bash
cargo test -p fdemon-app ide_config::vscode
```

All `vscode::tests::*` cases must pass on macOS. Windows verification is via the post-merge CI matrix.

### Notes

- A more rigorous alternative is `rel.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/")`. This handles drive letters and root components more cleanly. For a strip-prefixed relative path the simple `.replace('\\', "/")` is sufficient — drive letters never appear in a relative path.
- Do not introduce a shared `path_utils` module. Two call sites (emacs + vscode) is below the threshold for extraction.
- VS Code accepts both `\` (with escapes) and `/` in `cwd` on Windows. Choosing `/` matches the existing test assertions and the cross-platform convention.

---

## Completion Summary

**Status:** Done
**Branch:** fix/detect-windows-bat

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/ide_config/vscode.rs` | Changed `rel.to_string_lossy().into_owned()` to `rel.to_string_lossy().replace('\\', "/")` in `compute_cwd` to normalise path separators to forward slashes on all platforms |

### Notable Decisions/Tradeoffs

1. **Simple `.replace('\\', "/")` vs component iteration**: The task notes that the simple replace is sufficient for strip-prefixed relative paths since drive letters never appear in them. Kept the minimal change rather than the more complex component-join approach.
2. **Single site**: There was only one `to_string_lossy()` call in the file (confirmed by grep). The `Err(_)` fallback already returns `"${workspaceFolder}"` which has no path separators.

### Testing Performed

- `cargo test -p fdemon-app ide_config::vscode` — Passed (25 tests)
- `cargo clippy -p fdemon-app --all-targets -- -D warnings` — Passed (clean)
- `cargo fmt --all -- --check` — Passed (clean)

### Risks/Limitations

1. **Windows CI verification only**: The actual backslash-to-forward-slash normalisation can only be observed on Windows (macOS/Linux already use forward slashes). The fix is correct by construction and verified by the test assertions on macOS, with Windows coverage delegated to CI.
