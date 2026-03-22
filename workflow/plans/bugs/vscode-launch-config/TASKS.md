# VS Code Launch Config Bug Fixes — Task Index

## Overview

Fix two bugs in IDE config generation: remove invalid `fdemon-managed` field that causes VS Code warnings, and detect the workspace root so `launch.json` is placed where VS Code can find it.

**Total Tasks:** 2
**Estimated Time:** 40 minutes

## Task Dependency Graph

```
Task 1 (remove fdemon-managed) → Task 2 (workspace root detection)

Both touch vscode.rs — must run sequentially.
```

## Tasks

| # | Task | Status | Depends On | Est. | Modules |
|---|------|--------|------------|------|---------|
| 1 | [01-remove-fdemon-managed](tasks/01-remove-fdemon-managed.md) | Done ✅ | — | 10min | `ide_config/vscode.rs`, `ide_config/merge.rs` |
| 2 | [02-workspace-root-detection](tasks/02-workspace-root-detection.md) | Done ✅ | 1 | 30min | `ide_config/vscode.rs`, `ide_config/mod.rs` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|----------------------|--------------------------|
| 01-remove-fdemon-managed | `ide_config/vscode.rs`, `ide_config/merge.rs` | — |
| 02-workspace-root-detection | `ide_config/vscode.rs`, `ide_config/mod.rs` | `ide_config/neovim.rs` (delegates to vscode) |

### Overlap Matrix

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|-------------------|-------------------|
| 1 + 2 | `vscode.rs` | Sequential (same branch) |

## Success Criteria

- [ ] No `"fdemon-managed"` field in generated VS Code or Neovim configs
- [ ] No VS Code validation warnings on generated entries
- [ ] Dead `FDEMON_MARKER_FIELD` constant removed from `merge.rs`
- [ ] `launch.json` written to workspace root `.vscode/` when VS Code is opened at a parent directory
- [ ] `cwd` field set to relative path from workspace root to Flutter project
- [ ] Single-project setups still work unchanged
- [ ] All existing tests pass + new tests for workspace detection
- [ ] `cargo clippy --workspace` clean
