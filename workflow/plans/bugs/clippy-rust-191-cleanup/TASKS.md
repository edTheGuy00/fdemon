# Clippy Rust 1.91 Cleanup — Task Index

## Overview

Mechanical cleanup of ~193 clippy warnings introduced by Rust 1.91, split one task per crate so they can run in parallel worktrees with zero write-file overlap. A final task restores `-D warnings` to `.github/workflows/ci.yml`.

**Total Tasks:** 7
**Estimated Hours:** 6–10 hours (mostly parallelizable)

## Task Dependency Graph

```
Wave 1 (parallel — one per crate)
├── 01-fix-fdemon-core         (2 warnings)
├── 02-fix-fdemon-daemon       (10 warnings)
├── 03-fix-fdemon-dap          (35 warnings)
├── 04-fix-fdemon-tui          (57 warnings)
├── 05-fix-fdemon-app          (79 warnings)
└── 06-fix-integration-tests   (4 warnings)
                │
                ▼
Wave 2 (sequential — depends on all of Wave 1)
└── 07-restore-d-warnings-ci   (.github/workflows/ci.yml)
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-fix-fdemon-core](tasks/01-fix-fdemon-core.md) | Not Started | - | 0.25h | `crates/fdemon-core/` |
| 2 | [02-fix-fdemon-daemon](tasks/02-fix-fdemon-daemon.md) | Not Started | - | 0.5h | `crates/fdemon-daemon/` |
| 3 | [03-fix-fdemon-dap](tasks/03-fix-fdemon-dap.md) | Not Started | - | 1–1.5h | `crates/fdemon-dap/` |
| 4 | [04-fix-fdemon-tui](tasks/04-fix-fdemon-tui.md) | Not Started | - | 1.5–2h | `crates/fdemon-tui/` |
| 5 | [05-fix-fdemon-app](tasks/05-fix-fdemon-app.md) | Not Started | - | 2–3h | `crates/fdemon-app/` |
| 6 | [06-fix-integration-tests](tasks/06-fix-integration-tests.md) | Not Started | - | 0.25h | `tests/sdk_detection/` |
| 7 | [07-restore-d-warnings-ci](tasks/07-restore-d-warnings-ci.md) | Not Started | 1, 2, 3, 4, 5, 6 | 0.25h | `.github/workflows/ci.yml` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|--------------------------|
| 01-fix-fdemon-core | `crates/fdemon-core/src/ansi.rs` | - |
| 02-fix-fdemon-daemon | `crates/fdemon-daemon/src/devices.rs`, `crates/fdemon-daemon/src/native_logs/custom.rs`, `crates/fdemon-daemon/src/vm_service/extensions/inspector.rs`, `crates/fdemon-daemon/src/vm_service/extensions/mod.rs`, `crates/fdemon-daemon/src/vm_service/extensions/overlays.rs` | - |
| 03-fix-fdemon-dap | `crates/fdemon-dap/src/adapter/threads.rs`, `crates/fdemon-dap/src/adapter/tests/call_service.rs`, `crates/fdemon-dap/src/adapter/tests/restart_frame.rs`, `crates/fdemon-dap/src/adapter/tests/request_timeouts_events.rs`, `crates/fdemon-dap/src/adapter/tests/stack_scopes_variables.rs`, `crates/fdemon-dap/src/adapter/tests/update_debug_options.rs` | - |
| 04-fix-fdemon-tui | `crates/fdemon-tui/src/test_utils.rs`, `crates/fdemon-tui/src/widgets/devtools/mod.rs`, `crates/fdemon-tui/src/widgets/devtools/network/tests.rs`, `crates/fdemon-tui/src/widgets/devtools/performance/tests.rs`, `crates/fdemon-tui/src/widgets/header.rs`, `crates/fdemon-tui/src/widgets/new_session_dialog/launch_context.rs`, `crates/fdemon-tui/src/widgets/new_session_dialog/target_selector.rs`, `crates/fdemon-tui/src/widgets/search_input.rs`, `crates/fdemon-tui/src/widgets/settings_panel/tests.rs` | - |
| 05-fix-fdemon-app | `crates/fdemon-app/src/actions/native_logs.rs`, `crates/fdemon-app/src/actions/network.rs`, `crates/fdemon-app/src/actions/performance.rs`, `crates/fdemon-app/src/actions/vm_service.rs`, `crates/fdemon-app/src/config/settings.rs`, `crates/fdemon-app/src/handler/devtools/debug.rs`, `crates/fdemon-app/src/handler/helpers.rs`, `crates/fdemon-app/src/handler/new_session/launch_context.rs`, `crates/fdemon-app/src/handler/settings_dart_defines.rs`, `crates/fdemon-app/src/handler/tests.rs`, `crates/fdemon-app/src/new_session_dialog/target_selector_state.rs`, `crates/fdemon-app/src/session/debug_state.rs`, `crates/fdemon-app/src/session/network.rs`, `crates/fdemon-app/src/session/performance.rs`, `crates/fdemon-app/src/session/tests.rs`, `crates/fdemon-app/src/settings_items.rs`, `crates/fdemon-app/src/spawn.rs`, `crates/fdemon-app/src/state.rs`, `crates/fdemon-app/src/watcher/mod.rs` | - |
| 06-fix-integration-tests | `tests/sdk_detection/docker_helpers.rs`, `tests/sdk_detection/tier1_detection_chain.rs`, `tests/sdk_detection/tier2_headless.rs` | - |
| 07-restore-d-warnings-ci | `.github/workflows/ci.yml` | all task files (verification context) |

### Overlap Matrix

Wave-1 peer pairs (01–06): each task owns a distinct directory subtree (one crate's `src/` or the workspace `tests/` dir). Zero shared write files across all 15 pairs.

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|--------------------|-------------------|
| 01 + 02 | None | Parallel (worktree) |
| 01 + 03 | None | Parallel (worktree) |
| 01 + 04 | None | Parallel (worktree) |
| 01 + 05 | None | Parallel (worktree) |
| 01 + 06 | None | Parallel (worktree) |
| 02 + 03 | None | Parallel (worktree) |
| 02 + 04 | None | Parallel (worktree) |
| 02 + 05 | None | Parallel (worktree) |
| 02 + 06 | None | Parallel (worktree) |
| 03 + 04 | None | Parallel (worktree) |
| 03 + 05 | None | Parallel (worktree) |
| 03 + 06 | None | Parallel (worktree) |
| 04 + 05 | None | Parallel (worktree) |
| 04 + 06 | None | Parallel (worktree) |
| 05 + 06 | None | Parallel (worktree) |
| 07 + any | n/a — depends on 01–06 | Sequential (after merge) |

## Strategy (per crate task)

Each Wave-1 task follows the same recipe:

1. Run `cargo clippy --fix -p <crate> --all-targets --allow-dirty` to apply mechanical suggestions.
2. Hand-fix the remaining warnings clippy can't auto-resolve (type aliases, MSRV-incompatible suggestions, `module_inception`, doc fixes, etc.).
3. Verify with `cargo clippy -p <crate> --all-targets -- -D warnings` (must exit 0).
4. Run `cargo test -p <crate>` to confirm no regressions.
5. Run `cargo fmt --all` before committing.

## Cross-Cutting Constraints

These apply to all tasks:

- **MSRV is `1.77.2`** (`Cargo.toml`). Do **not** apply suggestions that require newer methods:
  - `clippy::manual_is_multiple_of` → `i.is_multiple_of(2)` was stabilized in 1.87. Use `#[allow(clippy::manual_is_multiple_of)]` on the offending function/module instead of accepting the auto-fix.
  - All other lint fixes inventoried (`is_some_and`, `slice::from_ref`, `(a..b).contains(&x)`, etc.) are MSRV-safe.
- **No behavior changes** — this is lint cleanup only. No test renames, no API changes, no refactors beyond what the lint requires.
- **Type aliases** for `clippy::type_complexity` may stay private (file-local `type` declarations are fine).
- **Test-only dead code** (`HangingGetVmBackend` in `fdemon-dap`) should be marked `#[allow(dead_code)]` rather than deleted — preserve test scaffolding intent.
- **`module_inception`** (`mod tests` inside `tests.rs`) is a pre-existing convention in this repo. Use `#[allow(clippy::module_inception)]` on the inner `mod tests` rather than renaming.

## Success Criteria

The bug is resolved when:

- [ ] `cargo clippy --workspace --all-targets -- -D warnings` exits 0 (locally and on all 3 CI runners).
- [ ] `cargo test --workspace` continues to pass (no test regressions).
- [ ] `.github/workflows/ci.yml` has `-D warnings` restored on the clippy step, and the `# NOTE: -D warnings is temporarily dropped …` comment block is removed.
- [ ] No public API changes; no behavior changes; only lint-driven edits.

## Notes

- Source of warning inventory: `cargo clippy --workspace --all-targets 2>&1` on branch `fix/detect-windows-bat`. Per-crate counts: core=2, daemon=10, dap=35, tui=57, app=79, integration-tests=4 (total 187 distinct warnings; `--all-targets` can re-emit the same site for lib + test targets, hence the 193 line count).
- BUG.md predates discovery of `fdemon-dap` warnings; this index covers all 6 work units.
- Each task verifies its own crate independently. The full-workspace gate is exercised in task 07 only, after all crate fixes have merged.
- After all tasks land, delete this plan directory or move it to `workflow/reviews/bugs/clippy-rust-191-cleanup/` per repo convention.
