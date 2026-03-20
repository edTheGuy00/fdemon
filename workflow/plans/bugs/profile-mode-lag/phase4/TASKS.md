# Phase 4: PR Review Fixes — Task Index

## Overview

Address the 6 review findings from PR #27. Three categories: (A) silent mutex poison handling on transfer-slot reads, (B) bare `unwrap()` on `selected_id()` in production handler code, (C) `device = "ios"` vs `"auto"` mismatch between fixture and docs. All fixes are small, low-risk, and align with existing codebase patterns.

**Total Tasks:** 3
**Estimated Hours:** 1-2 hours

## Task Dependency Graph

```
┌──────────────────────────────┐   ┌──────────────────────────────┐
│  01-fix-mutex-poison         │   │  02-fix-unwrap-selected-id   │
│  (handler/update.rs,         │   │  (handler/session_lifecycle, │
│   handler/devtools/network)  │   │   handler/devtools/mod.rs)   │
└──────────────────────────────┘   └──────────────────────────────┘

┌──────────────────────────────┐
│  03-fix-launch-device        │
│  (example/app3 launch.toml,  │
│   example/TESTING.md)        │
└──────────────────────────────┘
```

All three tasks are independent — no shared write files.

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-fix-mutex-poison](tasks/01-fix-mutex-poison.md) | Not started | — | 0.5h | `handler/update.rs`, `handler/devtools/network.rs` |
| 2 | [02-fix-unwrap-selected-id](tasks/02-fix-unwrap-selected-id.md) | Not started | — | 0.5h | `handler/session_lifecycle.rs`, `handler/devtools/mod.rs` |
| 3 | [03-fix-launch-device-mismatch](tasks/03-fix-launch-device-mismatch.md) | Not started | — | 0.5h | `example/app3/.fdemon/launch.toml`, `example/TESTING.md` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|----------------------|--------------------------|
| 01-fix-mutex-poison | `crates/fdemon-app/src/handler/update.rs`, `crates/fdemon-app/src/handler/devtools/network.rs` | `crates/fdemon-app/src/actions/mod.rs` (reference pattern) |
| 02-fix-unwrap-selected-id | `crates/fdemon-app/src/handler/session_lifecycle.rs`, `crates/fdemon-app/src/handler/devtools/mod.rs` | `crates/fdemon-app/src/handler/devtools/network.rs` (reference pattern at line 290) |
| 03-fix-launch-device-mismatch | `example/app3/.fdemon/launch.toml`, `example/TESTING.md` | — |

### Overlap Matrix

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|-------------------|-------------------|
| 01 + 02 | None | Parallel (worktree) |
| 01 + 03 | None | Parallel (worktree) |
| 02 + 03 | None | Parallel (worktree) |

**All three tasks have zero write-file overlap and can run in parallel.**

## Success Criteria

Phase 4 is complete when:

- [ ] Both `.lock().ok()` transfer-slot sites log on poison and recover via `into_inner()` instead of silently producing `None`
- [ ] Both `selected_id().unwrap()` sites use `let-else` with early return instead of bare unwrap
- [ ] `example/app3/.fdemon/launch.toml` uses `device = "auto"` for the "Profile (Issue #25)" config
- [ ] `example/TESTING.md` Test I snippet matches the actual launch.toml
- [ ] All existing tests pass (`cargo test --workspace`)
- [ ] `cargo clippy --workspace -- -D warnings` passes
