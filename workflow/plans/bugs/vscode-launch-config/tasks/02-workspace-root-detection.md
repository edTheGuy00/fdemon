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
