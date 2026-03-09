## Task: Fix Emacs `merge_config` Relative Path Regression

**Objective**: When an Emacs config file already exists and gets regenerated, ensure the `(load-file ...)` instruction contains an absolute path instead of the hardcoded relative placeholder `".fdemon/dap-emacs.el"`.

**Depends on**: None

**Severity**: Major

### Scope

- `crates/fdemon-app/src/ide_config/mod.rs`: Modify `run_generator()` to pass `project_root` context to the merge path
- `crates/fdemon-app/src/ide_config/emacs.rs`: Fix `merge_config()` to produce absolute paths

### Details

**Current trait signature:**
```rust
fn generate(&self, port: u16, project_root: &Path) -> Result<String>;
fn merge_config(&self, existing: &str, port: u16) -> Result<String>;
```

`generate()` receives `project_root` and embeds the absolute path. `merge_config()` does not, so Emacs hardcodes `".fdemon/dap-emacs.el"`.

**Emacs behavior is unique:** Unlike VS Code/Neovim/Helix/Zed which genuinely merge into existing configs (preserving user entries), Emacs "merge" is a full overwrite — the entire file is regenerated. This means `merge_config` for Emacs is semantically identical to `generate`.

**Recommended fix — special-case in `run_generator()`:**

In `run_generator()`, when the file already exists, compare the merged content against what `generate()` would produce. For Emacs, since merge is an overwrite, simply call `generate()` instead of `merge_config()`:

```rust
let (content, action) = if generator.config_exists(project_root) {
    let existing = std::fs::read_to_string(&config_path)?;
    let merged = generator.merge_config(&existing, port)?;
    (merged, ConfigAction::Updated)
} else {
    let fresh = generator.generate(port, project_root)?;
    (fresh, ConfigAction::Created)
};
```

**Option A — Add `project_root` to `merge_config` trait:**
```rust
fn merge_config(&self, existing: &str, port: u16, project_root: &Path) -> Result<String>;
```
This is the cleanest long-term fix. All 5 implementations need updating (4 just add `_project_root: &Path` to their signatures). Emacs uses it to construct the absolute path.

**Option B — Call `generate()` for Emacs overwrite case:**
In `run_generator()`, detect that the merged content is a full overwrite (Emacs) by adding a trait method `fn is_full_overwrite(&self) -> bool` defaulting to `false`, or by simply calling `generate()` always and comparing the result against existing. This avoids changing the trait signature but adds complexity.

**Recommended: Option A** — it's a single clean change, the 4 non-Emacs impls just ignore the parameter, and it future-proofs any generator that might need project_root during merge.

### Acceptance Criteria

1. When Emacs config is regenerated (file already exists), `(load-file "/absolute/path/.fdemon/dap-emacs.el")` appears with the full absolute path
2. No regression for VS Code, Neovim, Helix, or Zed merge behavior
3. All existing tests pass, new test verifies absolute path in merge output

### Testing

```rust
#[test]
fn test_emacs_merge_produces_absolute_path() {
    let dir = tempdir().unwrap();
    let gen = EmacsGenerator;
    let existing = "(some old elisp)";
    let result = gen.merge_config(existing, 12345, dir.path()).unwrap();
    let expected_path = dir.path().join(".fdemon/dap-emacs.el");
    assert!(result.contains(&expected_path.display().to_string()));
    assert!(!result.contains("\".fdemon/dap-emacs.el\""));  // no relative path
}
```

### Notes

- If Option A is chosen, update the `merge_config` call in `run_generator()` to pass `project_root`.
- Task 05 (content comparison) also modifies `run_generator()` — do this task first.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/ide_config/mod.rs` | Added `project_root: &Path` to `merge_config` trait signature; updated `run_generator()` call site to pass `project_root` |
| `crates/fdemon-app/src/ide_config/emacs.rs` | Updated `merge_config` to accept `project_root: &Path` and use `self.config_path(project_root)` for absolute path; replaced stale placeholder-path test; added `test_emacs_merge_produces_absolute_path` |
| `crates/fdemon-app/src/ide_config/vscode.rs` | Added `_project_root: &Path` to `merge_config` signature; updated all 9 test call sites |
| `crates/fdemon-app/src/ide_config/neovim.rs` | Added `_project_root: &Path` to `merge_config` signature; delegates to `vscode.merge_config(existing, port, Path::new(""))` ; updated 3 test call sites |
| `crates/fdemon-app/src/ide_config/helix.rs` | Added `_project_root: &Path` to `merge_config` signature; updated 10 test call sites |
| `crates/fdemon-app/src/ide_config/zed.rs` | Added `_project_root: &Path` to `merge_config` signature; updated 7 test call sites |

### Notable Decisions/Tradeoffs

1. **Option A chosen**: Added `project_root: &Path` to the `merge_config` trait as specified. The 4 non-Emacs implementations receive `_project_root` (unused). This is the cleanest approach and future-proofs any generator that might need `project_root` during merge.
2. **Test call sites updated to `Path::new("")`**: Non-Emacs test call sites pass an empty path since those implementations ignore the parameter. This is consistent with how VSCode's existing `generate()` tests use `Path::new("")`.
3. **Stale test renamed**: `test_emacs_merge_uses_placeholder_path` was renamed to `test_emacs_merge_uses_absolute_path` and updated to assert the absolute path is present, since the placeholder behaviour was the bug being fixed.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check --workspace` - Passed (0 errors)
- `cargo test -p fdemon-app -- emacs` - Passed (19 tests including new `test_emacs_merge_produces_absolute_path`)
- `cargo test -p fdemon-app` - Passed (1444 tests, 0 failures)
- `cargo clippy --workspace -- -D warnings` - Passed (0 warnings)

### Risks/Limitations

1. **None**: The change is purely additive to the trait signature. All implementations compile and all tests pass.
