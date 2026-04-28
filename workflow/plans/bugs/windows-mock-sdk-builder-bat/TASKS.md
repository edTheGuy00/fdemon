# Windows CI Portability — Final Sweep — Task Index

## Overview

Four parallel tasks to bundle every remaining and known-latent Windows portability issue. After this plan lands, no further Windows-only fix rounds should be required for the surface area covered (see [BUG.md](BUG.md) for the full audit).

**Total Tasks:** 4
**Estimated Hours:** 1–1.5h total (parallel ~30 minutes wall time)

## Task Dependency Graph

```
Wave 1 (parallel — disjoint write-file sets)
├── 01-mock-sdk-builder-windows-bat       (tests/sdk_detection/fixtures.rs)
├── 02-ready-check-windows-fixes          (crates/fdemon-app/src/actions/ready_check.rs)
├── 03-custom-log-capture-windows-gates   (crates/fdemon-daemon/src/native_logs/custom.rs)
└── 04-e2e-pty-test-gating-consistency    (tests/e2e/settings_page.rs, tests/e2e/debug_settings.rs)
```

No cross-task dependencies. Wave 1 is the only wave.

## Tasks

| # | Task | Status | Depends On | Est. Hours | Severity |
|---|------|--------|------------|------------|----------|
| 1 | [01-mock-sdk-builder-windows-bat](tasks/01-mock-sdk-builder-windows-bat.md) | Not Started | - | 0.25h | BLOCKER |
| 2 | [02-ready-check-windows-fixes](tasks/02-ready-check-windows-fixes.md) | Not Started | - | 0.5h | BLOCKER + LATENT |
| 3 | [03-custom-log-capture-windows-gates](tasks/03-custom-log-capture-windows-gates.md) | Not Started | - | 0.25h | BLOCKER |
| 4 | [04-e2e-pty-test-gating-consistency](tasks/04-e2e-pty-test-gating-consistency.md) | Not Started | - | 0.25h | LATENT |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|---------------------------|
| 01-mock-sdk-builder-windows-bat | `tests/sdk_detection/fixtures.rs` | - |
| 02-ready-check-windows-fixes | `crates/fdemon-app/src/actions/ready_check.rs` | - |
| 03-custom-log-capture-windows-gates | `crates/fdemon-daemon/src/native_logs/custom.rs` | - |
| 04-e2e-pty-test-gating-consistency | `tests/e2e/settings_page.rs`, `tests/e2e/debug_settings.rs` | (`tests/e2e/tui_interaction.rs` for the `cfg_attr` pattern reference) |

### Overlap Matrix

All 4 tasks write disjoint files. Every pair is parallel-safe.

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|--------------------|-------------------|
| 01 + 02 | None | Parallel (worktree) |
| 01 + 03 | None | Parallel (worktree) |
| 01 + 04 | None | Parallel (worktree) |
| 02 + 03 | None | Parallel (worktree) |
| 02 + 04 | None | Parallel (worktree) |
| 03 + 04 | None | Parallel (worktree) |

## Strategy

All four tasks follow the same recipe:

1. Apply the change described in the task file.
2. Verify per-crate (or workspace where the task touches multiple crates):
   - `cargo clippy -p <crate> --all-targets -- -D warnings` exits 0
   - `cargo test -p <crate>` passes locally (macOS)
   - `cargo fmt --all -- --check` clean
3. Windows-specific fixes (Tasks 01, 02-cfg-gate, 03) cannot be tested on macOS directly. Reliance is on (a) targeted code review against the audit findings, (b) the post-merge CI matrix run on `windows-latest`, which is the definitive validation.

## Cross-Cutting Constraints

- **No production behaviour changes** beyond the path-already-fixed normalisations and the L1 mock-server drain (which is test-only). All other changes are `#[cfg(...)]` gates on tests, or one fixture-builder behavioural change keyed on `cfg!(target_os = "windows")`.
- **No new public API.** All fixes are internal (test gates, fixture-builder internal logic, mock-server logic).
- **Existing patterns preferred.** Task 03's gating mirrors the already-applied pattern in `test_custom_capture_working_dir`. Task 04 mirrors the per-test `cfg_attr` already applied to `tests/e2e/tui_interaction.rs` and `tui_workflows.rs`. Task 02's L1 fix copies the drain/shutdown pattern already applied to `test_http_check_success`.
- **Quality gate per-crate before merge.** The orchestrator validates each task with `task_validator` after completion, and the post-merge CI matrix is the final gate.

## Success Criteria

This bug is resolved when:

- [ ] All 48 currently-failing Windows tests pass (Task 01).
- [ ] All 12 currently-latent-but-will-fail-next Windows tests pass (Tasks 02, 03).
- [ ] No further Windows-only test failures surface in CI for the rest of the branch's lifetime within the surface area audited.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` exits 0 on all 3 OSes.
- [ ] `cargo test --workspace` passes on the developer's macOS.
- [ ] `cargo fmt --all -- --check` clean.
- [ ] PR #38's CI matrix is green.

## Notes

- **Source of bugs:** Identified by aggregating distinct failure modes across CI runs `24998544748`, `25001441575`, `25003302149`, `25009502657`, `25044658233`, `25045192202`, `25046094078`, `25047058461` plus a forward-looking audit by `codebase_researcher` of test fixtures and production code.
- **Two LATENT items NOT in scope:** `fs2::FileExt::lock_exclusive()` on Windows and `escape_toml_string(entry_point)` not slash-normalising. Both are real but neither breaks CI nor is associated with a failing test. Track separately if user wants to address cross-host config portability.
- After this followup lands and CI is green, archive the original `ci-3os-portability-fixes` plan directory and this directory to `workflow/reviews/bugs/` per repo convention.
