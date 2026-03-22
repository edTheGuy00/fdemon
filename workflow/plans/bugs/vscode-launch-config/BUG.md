# Bug Fix: IDE Config File Placement & Invalid Fields

## TL;DR

Two categories of bugs across IDE config generators:

1. **VS Code config placed in wrong directory**: `launch.json` is written to the Flutter project root (`example/app3/.vscode/`), but VS Code only reads `.vscode/launch.json` from the workspace root it was opened at. When the user opens a parent directory (e.g., the monorepo root), VS Code never finds the generated config.

2. **Invalid `fdemon-managed` field**: VS Code and Neovim configs include `"fdemon-managed": true` which is not a valid launch.json field. VS Code's Dart extension schema validation flags it as "Property fdemon-managed is not allowed." The field is dead code — merge logic uses `"name": "Flutter (fdemon)"` as the match key.

---

## IDE Config Placement Audit

| IDE | Config File | Where fdemon writes it | Where IDE looks | Gap? |
|-----|------------|----------------------|-----------------|------|
| **VS Code** | `.vscode/launch.json` | `{flutter_project}/.vscode/` | Only at opened workspace root | **Yes** — monorepo users won't find it |
| **Neovim** | `.vscode/launch.json` + `.nvim-dap.lua` | `{flutter_project}/` | `vim.fn.getcwd()` — where Neovim was opened | **Yes** — same as VS Code |
| **Zed** | `.zed/debug.json` | `{flutter_project}/.zed/` | Per-worktree root; multi-worktree picks up each added folder | Partial — works if Flutter project is added as a worktree |
| **Helix** | `.helix/languages.toml` | `{flutter_project}/.helix/` | Walks up from cwd, stops at `.helix/` or `.git/` | **No** — Helix's upward traversal finds it |
| **Emacs** | `.fdemon/dap-emacs.el` | `{flutter_project}/.fdemon/` | Manual load only; dap-mode has no auto-discovery for `.el` files | **No** — file is in fdemon's own dir, requires manual load by design |

### Key Insight

VS Code is the only IDE that **strictly** requires the config at the opened workspace root and provides no discovery fallback. All other IDEs either walk up parent directories (Helix), operate from cwd (Neovim), support per-folder worktrees (Zed), or live in fdemon's own directory (Emacs).

---

## Invalid/Custom Field Audit

| IDE | Generator | Invalid Fields | Impact |
|-----|-----------|---------------|--------|
| **VS Code** | `VSCodeGenerator` | `"fdemon-managed": true` | Red squiggle warning in VS Code |
| **Neovim** | `NeovimGenerator` (delegates to VSCode) | `"fdemon-managed": true` (inherited) | Same as VS Code if user opens launch.json |
| **Zed** | `ZedGenerator` | None (`"adapter": "Delve"` is valid but semantically wrong — documented workaround) | No warning today; fragile if Zed validates |
| **Helix** | `HelixGenerator` | None — all fields are standard Helix DAP fields | No issues |
| **Emacs** | `EmacsGenerator` | None — output is Elisp, not JSON | No issues |
| **merge.rs** | N/A | `FDEMON_MARKER_FIELD` constant is `#[allow(dead_code)]` and never used | Dead code |

---

## Bug 1: VS Code Config Placed in Wrong Directory

### Root Cause

`config_path()` in `vscode.rs:40-42` always uses `project_root`:
```rust
fn config_path(&self, project_root: &Path) -> PathBuf {
    project_root.join(".vscode").join("launch.json")
}
```

VS Code only reads `.vscode/launch.json` from the workspace root (the directory opened with `code .`). It does NOT scan subdirectories or walk parent directories.

### Fix

Detect the workspace root by walking up from `project_root`. The heuristic (in priority order):
1. If an ancestor directory has `.vscode/` → that's the workspace root (user already has VS Code config there)
2. If an ancestor directory has `.git/` → use that as workspace root (common monorepo pattern)
3. Fall back to `project_root` (single-project setup)

When workspace root differs from Flutter project root, set `cwd` to the relative path:
```json
{
  "name": "Flutter (fdemon)",
  "type": "dart",
  "request": "attach",
  "debugServer": 33001,
  "cwd": "example/app3"
}
```

Instead of `"cwd": "${workspaceFolder}"` which is only correct when project root == workspace root.

**Same fix applies to Neovim** (delegates to VSCodeGenerator).

### Affected Files

| File | Change |
|------|--------|
| `crates/fdemon-app/src/ide_config/vscode.rs` | Add workspace root detection; update `config_path()` and `fdemon_entry()` to compute relative `cwd` |
| `crates/fdemon-app/src/ide_config/mod.rs` | Thread workspace root detection through `run_generator()` or add it to the `IdeConfigGenerator` trait |

---

## Bug 2: Remove `fdemon-managed` and Dead Code

### Root Cause

`fdemon_entry()` includes `"fdemon-managed": true` but the merge logic matches by `"name"` field, not this marker. The `FDEMON_MARKER_FIELD` constant in `merge.rs` is dead code (`#[allow(dead_code)]`).

### Fix

- Remove `"fdemon-managed": true` from `VSCodeGenerator::fdemon_entry()`
- Remove dead `FDEMON_MARKER_FIELD` constant from `merge.rs`
- Neovim inherits the fix automatically (delegates to VSCodeGenerator)

### Affected Files

| File | Change |
|------|--------|
| `crates/fdemon-app/src/ide_config/vscode.rs` | Remove `"fdemon-managed": true` from `fdemon_entry()` |
| `crates/fdemon-app/src/ide_config/merge.rs` | Remove `FDEMON_MARKER_FIELD` constant and its `#[allow(dead_code)]` |

---

## Task Breakdown

| # | Task | Est. | Files Modified (Write) | Depends On |
|---|------|------|----------------------|------------|
| 1 | Remove `fdemon-managed` field and `FDEMON_MARKER_FIELD` dead constant | 10min | `vscode.rs`, `merge.rs` | — |
| 2 | Detect workspace root for VS Code/Neovim config placement | 30min | `vscode.rs`, `mod.rs` | — |

### File Overlap Analysis

| Task Pair | Shared Write Files | Strategy |
|-----------|-------------------|----------|
| 1 + 2 | `vscode.rs` | Sequential (same branch) |

**Recommended order:** Task 1 → Task 2 (both touch `vscode.rs`).

---

## Success Criteria

- [ ] No `"fdemon-managed"` field in generated VS Code or Neovim configs
- [ ] No VS Code validation warnings on generated launch.json entries
- [ ] Dead `FDEMON_MARKER_FIELD` constant removed
- [ ] When VS Code is opened at a parent directory (monorepo root), `launch.json` is written to `{workspace_root}/.vscode/launch.json`
- [ ] `cwd` field is set to relative path from workspace root to Flutter project
- [ ] Single-project setups still work (project root == workspace root → `cwd` is `"."` or `"${workspaceFolder}"`)
- [ ] Zed, Helix, Emacs configs are unaffected
- [ ] All existing tests pass; new tests for workspace root detection

## Notes

- Zed's `"adapter": "Delve"` workaround is a known fragility but not actionable until Zed adds a native Dart adapter. Not in scope for this fix.
- Helix's TOML merge strips comments — separate low-priority issue, not in scope.
- The workspace root detection heuristic should be conservative: only walk up if there's evidence of a parent workspace (`.vscode/` or `.git/`). Don't walk past filesystem boundaries.
