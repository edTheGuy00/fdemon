# CI 3-OS Portability Fixes — Task Index

## Overview

PR #38's new 3-OS CI matrix surfaced 10 pre-existing test failures across `fdemon-daemon` and `fdemon-app`. This plan groups them into 8 disjoint-write-file tasks so they can dispatch in parallel under worktree isolation. Tasks 01–07 are `implementor` work; Task 08 is `doc_maintainer` work (DEVELOPMENT.md is in the doc_maintainer-managed set).

**Total Tasks:** 8
**Estimated Hours:** 3.5–4.5 hours total (parallelizable down to ~45 minutes wall time)

## Task Dependency Graph

```
Wave 1 (parallel — disjoint write-file sets)
├── 01-fix-ios-tool-availability-tests       (tool_availability.rs)
├── 02-fix-flutter-wrapper-env-leak          (locator.rs)
├── 03-fix-emacs-path-separator              (emacs.rs)
├── 04-fix-vscode-path-separator             (vscode.rs)
├── 05-fix-instant-subtraction               (state.rs)
├── 06-fix-http-mock-server-loop             (ready_check.rs)
├── 07-fix-review-feedback-code              (ci.yml + process.rs)
└── 08-fix-development-doc-commands          (DEVELOPMENT.md — doc_maintainer)
```

No cross-task dependencies. Wave 1 is the only wave.

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-fix-ios-tool-availability-tests](tasks/01-fix-ios-tool-availability-tests.md) | Done | - | 0.25h | `crates/fdemon-daemon/` |
| 2 | [02-fix-flutter-wrapper-env-leak](tasks/02-fix-flutter-wrapper-env-leak.md) | Done | - | 0.25h | `crates/fdemon-daemon/` |
| 3 | [03-fix-emacs-path-separator](tasks/03-fix-emacs-path-separator.md) | Done | - | 0.5–1h | `crates/fdemon-app/` |
| 4 | [04-fix-vscode-path-separator](tasks/04-fix-vscode-path-separator.md) | Done | - | 0.5h | `crates/fdemon-app/` |
| 5 | [05-fix-instant-subtraction](tasks/05-fix-instant-subtraction.md) | Done | - | 0.25h | `crates/fdemon-app/` |
| 6 | [06-fix-http-mock-server-loop](tasks/06-fix-http-mock-server-loop.md) | Done | - | 0.5h | `crates/fdemon-app/` |
| 7 | [07-fix-review-feedback-code](tasks/07-fix-review-feedback-code.md) | Done | - | 0.5h | `.github/workflows/`, `crates/fdemon-daemon/` |
| 8 | [08-fix-development-doc-commands](tasks/08-fix-development-doc-commands.md) | Done | - | 0.25h | `docs/` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|--------------------------|
| 01-fix-ios-tool-availability-tests | `crates/fdemon-daemon/src/tool_availability.rs` | - |
| 02-fix-flutter-wrapper-env-leak | `crates/fdemon-daemon/src/flutter_sdk/locator.rs` | (sibling tests in same file as patterns to match) |
| 03-fix-emacs-path-separator | `crates/fdemon-app/src/ide_config/emacs.rs` | - |
| 04-fix-vscode-path-separator | `crates/fdemon-app/src/ide_config/vscode.rs` | - |
| 05-fix-instant-subtraction | `crates/fdemon-app/src/state.rs` | - |
| 06-fix-http-mock-server-loop | `crates/fdemon-app/src/actions/ready_check.rs` | (sibling test `test_http_check_non_200_retries` in same file as the loop pattern) |
| 07-fix-review-feedback-code | `.github/workflows/ci.yml`, `crates/fdemon-daemon/src/process.rs` | - |
| 08-fix-development-doc-commands | `docs/DEVELOPMENT.md` | `.github/workflows/ci.yml` (read for source of truth) |

### Overlap Matrix

All 8 tasks write to disjoint files. Every pair is parallel-safe.

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|--------------------|-------------------|
| 01 + 02 | None — different files within `fdemon-daemon` | Parallel (worktree) |
| 01 + 03 | None | Parallel (worktree) |
| 01 + 04 | None | Parallel (worktree) |
| 01 + 05 | None | Parallel (worktree) |
| 01 + 06 | None | Parallel (worktree) |
| 01 + 07 | None — Task 07 writes a different `fdemon-daemon` file (`process.rs`) | Parallel (worktree) |
| 01 + 08 | None | Parallel (worktree) |
| 02 + 03 | None | Parallel (worktree) |
| 02 + 04 | None | Parallel (worktree) |
| 02 + 05 | None | Parallel (worktree) |
| 02 + 06 | None | Parallel (worktree) |
| 02 + 07 | None — Task 07 writes `process.rs`, Task 02 writes `flutter_sdk/locator.rs` | Parallel (worktree) |
| 02 + 08 | None | Parallel (worktree) |
| 03 + 04 | None — different files within `fdemon-app/ide_config/` | Parallel (worktree) |
| 03 + 05 | None | Parallel (worktree) |
| 03 + 06 | None | Parallel (worktree) |
| 03 + 07 | None | Parallel (worktree) |
| 03 + 08 | None | Parallel (worktree) |
| 04 + 05 | None | Parallel (worktree) |
| 04 + 06 | None | Parallel (worktree) |
| 04 + 07 | None | Parallel (worktree) |
| 04 + 08 | None | Parallel (worktree) |
| 05 + 06 | None — different files within `fdemon-app` | Parallel (worktree) |
| 05 + 07 | None | Parallel (worktree) |
| 05 + 08 | None | Parallel (worktree) |
| 06 + 07 | None | Parallel (worktree) |
| 06 + 08 | None | Parallel (worktree) |
| 07 + 08 | None — Task 08 reads `ci.yml` but does not write it | Parallel (worktree) |

**Note on Task 07 vs Task 08 ci.yml read/write:** Task 07 *writes* `.github/workflows/ci.yml` (comment fix). Task 08 only *reads* `ci.yml` to determine the canonical command list to mirror in `DEVELOPMENT.md`. Read-only overlap is safe.

## Strategy

All 8 tasks follow the same recipe:

1. Apply the fix described in the task file.
2. Verify per-crate (or workspace for cross-crate tasks):
   - `cargo clippy -p <crate> --all-targets -- -D warnings` exits 0 (or `--workspace --all-targets` for tasks 07).
   - `cargo test -p <crate>` passes (or workspace).
   - `cargo fmt --all -- --check` is clean.
3. For Windows-specific fixes (Tasks 03, 04, 05, 06), the implementor cannot test on Windows directly. Reliance is on (a) targeted code review of the fix, (b) the post-merge CI matrix run, which is the definitive validation.

Task 08 (`doc_maintainer`) follows the same content-boundary rules as other doc work — DEVELOPMENT.md is bounded to build/run/test commands and environment setup. The doc fix is: replace bare `cargo {check,test,clippy} --workspace` with `cargo {check,test,clippy} --workspace --all-targets [-- -D warnings]` to mirror the CI matrix.

## Cross-Cutting Constraints

These apply to all tasks:

- **No production behavior changes** beyond the path-separator normalisations (Tasks 03, 04). Path normalisation is observably correct on every platform — forward-slash paths in Lisp/JSON config files are universally accepted by Emacs and VS Code.
- **No new public API.** All fixes are internal (private functions, test helpers, error message wording).
- **Existing test patterns preferred.** Tasks 02 and 06 should mirror sibling tests in the same file rather than introduce new patterns.
- **Quality gate per-crate before merge.** The orchestrator validates each task with `task_validator` after completion, and the post-merge CI matrix is the final gate.

## Success Criteria

This bug is resolved when:

- [ ] All 10 originally-failing tests pass on all 3 OS runners (verified by a green CI run on the merged branch).
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` exits 0.
- [ ] `cargo test --workspace --all-targets` passes on the developer's macOS.
- [ ] `cargo fmt --all` is clean.
- [ ] The 3 Copilot review comments on PR #38 are resolved by the corresponding edits and the conversation threads marked resolved.
- [ ] No new test or production regressions introduced.

## Notes

- **Source of bug:** Identified by the post-merge CI matrix on PR #38 (run `24998544748`). All failures are pre-existing portability issues, not regressions from PR #38's intent.
- **Why these were missed previously:** No prior CI on Linux or Windows. The developer's macOS environment masks (a) the iOS-cfg-gate test bugs (`#[cfg(target_os = "macos")]` arms always present), (b) `FLUTTER_ROOT` env-var leakage (developer machine may or may not have it set, but the test happens to pass when it isn't set), (c) backslash separators (macOS `Path::display()` emits `/`), (d) `Instant` subtraction (macOS uptime is large enough), (e) HTTP mock server race (Tokio scheduler timing differs from Windows).
- **CI does enforce `-D warnings` post-`clippy-rust-191-cleanup` Wave 7** (commit `1dd8b59`). Tasks 07 and 08 keep that gate consistent.
- After this followup lands and CI is green, archive this plan directory to `workflow/reviews/bugs/ci-3os-portability-fixes/` per repo convention.
