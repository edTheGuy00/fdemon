## Task: Normalise embedded Lisp paths to forward slashes

**Objective**: Make `generate_elisp` (the function that serialises a config-file path into the generated `dap-emacs.el` content) emit forward-slash paths on every platform, so the resulting Elisp string is valid on Windows. Update one test (`test_emacs_merge_produces_absolute_path`) to compare against the normalised form. The other two failing tests pass automatically once production normalises.

**Depends on**: None

**Estimated Time**: 0.5–1 hour

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/ide_config/emacs.rs`: Production fix in `generate_elisp` (and any other site that embeds a path in Elisp); test fix in `test_emacs_merge_produces_absolute_path`.

**Files Read (Dependencies):**
- None — the change is contained in one file.

### Details

#### Why forward-slash

Emacs Lisp string literals treat `\` as the escape character: `"\f"` is form-feed, `"\n"` is newline, etc. A Windows path embedded as `"C:\Users\foo\.fdemon\dap-emacs.el"` is **silently wrong** — it contains accidental escapes. Emacs `load-file` accepts forward-slash paths on Windows, and the convention in cross-platform Elisp config is forward slash. The production code must emit `/`.

#### Production fix

`generate_elisp` (called by both `generate` and `merge_config`) currently does something like:

```rust
let path_string = config_path.display().to_string();
// ... embed path_string in Elisp output ...
```

Replace with a normalisation helper:

```rust
/// Render `path` as a forward-slash string suitable for embedding in Elisp.
/// Emacs accepts `/` on Windows, and `\` would be misinterpreted as escape sequences in Elisp strings.
fn to_lisp_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}
```

Use `to_lisp_path(&config_path)` at every site in `emacs.rs` that embeds a path into Elisp output. If there is only one such site, inline the `.replace('\\', "/")` rather than introducing a helper.

#### Test fixes

- `test_emacs_generate_embeds_absolute_path` (line 182) and `test_emacs_merge_uses_absolute_path` (line 192) assert the literal `/my/flutter/app/.fdemon/dap-emacs.el`. After the production fix, both pass on every platform without change.
- `test_emacs_merge_produces_absolute_path` (line 203) constructs `expected_path = dir.path().join(".fdemon/dap-emacs.el")` and compares it via `expected_path.display().to_string()`. The tempdir prefix has `\` on Windows, so the expected string contains backslashes. After production normalises, the expected value must also normalise:

```rust
// Before (Windows-broken):
let expected = expected_path.display().to_string();
assert!(result.contains(&expected), "expected absolute path '{}' in merged output", expected);

// After:
let expected = expected_path.to_string_lossy().replace('\\', "/");
assert!(result.contains(&expected), "expected absolute path '{}' in merged output", expected);
```

### Acceptance Criteria

1. `crates/fdemon-app/src/ide_config/emacs.rs::generate_elisp` (and any other Elisp-emission site) embeds paths using forward slashes on every platform.
2. `test_emacs_generate_embeds_absolute_path` passes on Linux, macOS, and Windows.
3. `test_emacs_merge_uses_absolute_path` passes on Linux, macOS, and Windows.
4. `test_emacs_merge_produces_absolute_path` passes on Linux, macOS, and Windows after the test-side normalisation.
5. `cargo clippy -p fdemon-app --all-targets -- -D warnings` exits 0.
6. `cargo test -p fdemon-app` passes.
7. `cargo fmt --all -- --check` is clean.
8. No other production functions or tests are modified.
9. If there are non-test callers of `generate_elisp` outside `emacs.rs`, verify they still work correctly with forward-slash paths (search `crates/fdemon-app/src/` and `crates/fdemon-tui/src/` for `generate_elisp` and `merge_config`).

### Testing

```bash
cargo test -p fdemon-app ide_config::emacs
```

This must pass all `emacs::tests::*` cases on macOS. The Windows verification happens in CI after merge.

Manually inspect the generated Elisp to confirm forward slashes:

```bash
cargo test -p fdemon-app ide_config::emacs::tests::test_emacs_merge_produces_absolute_path -- --nocapture
```

### Notes

- The same `to_lisp_path` pattern is **not** needed for `vscode.rs` — VS Code's `cwd` field is a JSON value, and JSON treats `\` as escape just like Lisp, but the existing VS Code task (#04) handles its own normalisation.
- Do not over-engineer this into a shared `path_utils` module unless a third call site appears. Two call sites (emacs + vscode) are not enough to justify the abstraction.

---

## Completion Summary

**Status:** Not Started
**Branch:** _to be filled by implementor_

### Files Modified

| File | Changes |
|------|---------|
| _tbd_ | _tbd_ |

### Notable Decisions/Tradeoffs

_tbd_

### Testing Performed

- `cargo clippy -p fdemon-app --all-targets -- -D warnings` — _tbd_
- `cargo test -p fdemon-app` — _tbd_
- `cargo fmt --all -- --check` — _tbd_

### Risks/Limitations

_tbd_
