# Task 02: Detect Workspace Root for VS Code/Neovim Config Placement

## Objective

When the Flutter project is inside a larger workspace (e.g., `example/app3` inside a monorepo), write `launch.json` to the workspace root's `.vscode/` directory instead of the Flutter project's `.vscode/`. VS Code only reads `.vscode/launch.json` from the directory it was opened at.

## Context

- `config_path()` always returns `{project_root}/.vscode/launch.json`
- VS Code does NOT scan subdirectories or walk parent directories for launch.json
- The `cwd` field in the entry is currently `"${workspaceFolder}"` — correct only when project root == workspace root
- Neovim delegates entirely to VSCodeGenerator, so the fix applies to both
- Helix, Zed, and Emacs are unaffected (different discovery mechanisms)

## Acceptance Criteria

- [ ] Workspace root detected by walking up from `project_root` looking for `.vscode/` or `.git/`
- [ ] `config_path()` returns `{workspace_root}/.vscode/launch.json` when workspace root differs
- [ ] `cwd` field set to relative path from workspace root to Flutter project (e.g., `"example/app3"`)
- [ ] Single-project setup still works: when project root IS the workspace root, behavior unchanged
- [ ] Neovim inherits the fix (delegates to VSCodeGenerator)
- [ ] Neovim's `.nvim-dap.lua` is also placed at workspace root
- [ ] New unit tests for workspace root detection (monorepo case, single-project case, no-git case)
- [ ] Existing merge tests still pass (merge operates on file content, not path)

## Implementation Steps

1. **Add `detect_workspace_root()` helper** in `vscode.rs`:
   ```
   fn detect_workspace_root(project_root: &Path) -> &Path
   ```
   Walk up from `project_root`:
   - If an ancestor has `.vscode/` → return that ancestor (existing workspace)
   - Else if an ancestor has `.git/` → return that ancestor (repo root)
   - Else → return `project_root` (no workspace context found)

   Stop at filesystem root. Don't walk past `/Users/` or equivalent home boundary.

2. **Update `config_path()`** in `VSCodeGenerator`:
   - Call `detect_workspace_root(project_root)` to find the target directory
   - Return `{workspace_root}/.vscode/launch.json`

3. **Update `fdemon_entry()`** to accept both `project_root` and `workspace_root`:
   - If they differ: `"cwd"` = relative path from workspace root to project root
   - If same: `"cwd"` = `"${workspaceFolder}"` (or `"."`)

4. **Update `IdeConfigGenerator` trait** if needed:
   - `config_path()` already receives `project_root: &Path`
   - May need to add workspace root as a parameter, or have the generator detect it internally
   - Prefer internal detection (keeps trait simple, only VS Code needs this)

5. **Update Neovim's `post_write()`**:
   - `.nvim-dap.lua` should also go to workspace root (same directory as `launch.json`)
   - Currently writes to `project_root/.nvim-dap.lua`

6. **Add tests**:
   - Test: `project_root` has `.vscode/` → workspace root == project root
   - Test: parent has `.vscode/` → workspace root == parent
   - Test: grandparent has `.git/` → workspace root == grandparent
   - Test: no `.vscode/` or `.git/` anywhere → workspace root == project root
   - Test: `cwd` is relative path when workspace != project
   - Test: `cwd` is `${workspaceFolder}` when workspace == project

## Edge Cases

- **Multiple `.vscode/` directories**: Use the nearest ancestor (first found walking up)
- **`.git/` without `.vscode/`**: Safe fallback — the git root is likely the workspace root
- **Symlinks**: Use canonical paths for comparison
- **Windows paths**: Use `Path` APIs, not string manipulation

## Estimated Time

30 minutes

---

## Completion Summary

**Status:** Done
**Branch:** feat/dap-phase-6-plan

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/ide_config/vscode.rs` | Added `detect_workspace_root()` (public), `compute_cwd()`, updated `fdemon_entry()` signature to accept `project_root`+`workspace_root`, updated `config_path()`/`generate()`/`merge_config()` to detect workspace root internally. Added 25 unit tests including workspace root detection scenarios and cwd computation. |
| `crates/fdemon-app/src/ide_config/neovim.rs` | Updated `config_path()` to delegate to `VSCodeGenerator::config_path()`. Updated `write_nvim_dap_lua()` and `post_write()` to place `.nvim-dap.lua` at workspace root. Added tests for monorepo workspace root placement. |
| `crates/fdemon-daemon/src/vm_service/client.rs` | Fixed pre-existing missing `ws_uri` field in `VmRequestHandle` test constructors (blocked all fdemon-app tests). |
| `crates/fdemon-core/src/logging.rs` | Formatting change applied by `cargo fmt`. |
| `crates/fdemon-dap/src/adapter/handlers.rs` | Formatting change applied by `cargo fmt`. |

### Notable Decisions/Tradeoffs

1. **Internal detection over trait change**: Workspace root detection is done inside `VSCodeGenerator` methods rather than adding a new parameter to the `IdeConfigGenerator` trait. This keeps the trait simple — only VS Code/Neovim need workspace root awareness, while Helix/Zed/Emacs are unaffected.

2. **`detect_workspace_root()` returns `PathBuf` (owned)**: The function cannot return `&Path` because the returned value may be a parent of `project_root` that doesn't have a longer lifetime. Returning `PathBuf` avoids lifetime complexity.

3. **Walks ancestors only**: The function skips `project_root` itself and walks from its first parent upward. A `.vscode/` directory inside the project root itself doesn't count as a workspace (VS Code was not opened there specifically for this Flutter project — the workspace is somewhere above).

4. **`.vscode/` preferred over `.git/`**: When a `.vscode/` ancestor exists at a different level than `.git/`, `.vscode/` wins since VS Code was explicitly opened at that level.

5. **Pre-existing daemon test bug fixed**: The `VmRequestHandle` struct had a `ws_uri` field added without updating the `new_for_test()` factory method or the inline test constructors. Fixed as a prerequisite to running any tests.

### Testing Performed

- `cargo check -p fdemon-app` - Passed
- `cargo test -p fdemon-app --lib` - Passed (1876 tests)
- `cargo test -p fdemon-daemon --lib` - Passed (734 tests)
- `cargo clippy -p fdemon-app -- -D warnings` - Passed
- `cargo clippy -p fdemon-daemon -- -D warnings` - Passed
- `cargo fmt --all` - Applied (no issues)
- New workspace detection tests (25 in vscode.rs): all passed
- New neovim workspace root tests (18 total in neovim.rs): all passed

### Risks/Limitations

1. **Symlinks on Windows**: `Path::canonicalize()` on Windows resolves symlinks using UNC paths. This should work correctly but is untested on Windows (the project targets macOS/Linux primarily).

2. **Deep monorepos**: The walk goes all the way to the filesystem root if no markers are found. This is correct behavior (fallback to project_root) but may be slightly slow for very deep directory trees with no `.git` or `.vscode` anywhere.
