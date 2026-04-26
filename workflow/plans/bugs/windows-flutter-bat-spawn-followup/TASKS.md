# Windows `flutter devices` Spawn Failure — Follow-up Task Index

## Overview

Address the four blockers and assorted quality issues raised by the multi-agent review of the original Wave-1 fix. See:

- `BUG.md` (this directory) — full root-cause analysis and strategy
- `workflow/reviews/bugs/windows-flutter-bat-spawn/REVIEW.md` — original review
- `workflow/reviews/bugs/windows-flutter-bat-spawn/ACTION_ITEMS.md` — per-issue remediation steps

User-confirmed strategy decisions:

- **Clippy:** Strategy A — relax CI to `-W warnings` (no `-D`); file a separate `workflow/plans/bugs/clippy-rust-191-cleanup/` bug for the workspace-wide cleanup.
- **Shim-installer:** Option B — implement the real fix (binary-only fallback) so scoop/winget work without manual `[flutter] sdk_path` configuration.
- **Scope:** All Wave A + Wave B + Wave C tasks ship in this single follow-up plan (no second polish PR).

**Total Tasks:** 9 (8 implementor + 1 doc_maintainer)

## Task Dependency Graph

```
Wave A (parallel, no deps)
├── 01-emulators-diagnostic-pattern    (emulators.rs + new diagnostics.rs + devices.rs minor)
├── 02-demote-args-logging             (process.rs)
├── 03-relax-clippy-ci                 (ci.yml + new clippy-cleanup BUG.md)
├── 04-shim-installer-real-fix         (locator.rs)
└── 07-windowsbatch-doc-honesty        (types.rs)

Wave B (depends on Wave A)
├── 05-windows-tests-cleanup           (windows_tests.rs) — depends on 01, 04
└── 06-diagnostic-surface-polish       (devices.rs, locator.rs, process.rs, diagnostics.rs) — depends on 01, 02, 04

Wave C (depends on Wave A)
└── 08-pin-actions-shas                (.github/workflows/*.yml) — depends on 03

Wave D (depends on Waves A-C; doc_maintainer)
└── 09-update-architecture-doc         (docs/ARCHITECTURE.md) — depends on 01, 04, 06
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules | Agent |
|---|------|--------|------------|------------|---------|-------|
| 1 | [01-emulators-diagnostic-pattern](tasks/01-emulators-diagnostic-pattern.md) | Not Started | — | 1.5h | `crates/fdemon-daemon/src/emulators.rs`, `crates/fdemon-daemon/src/flutter_sdk/diagnostics.rs` (NEW), `crates/fdemon-daemon/src/devices.rs`, `crates/fdemon-daemon/src/flutter_sdk/mod.rs` | implementor |
| 2 | [02-demote-args-logging](tasks/02-demote-args-logging.md) | Not Started | — | 0.5h | `crates/fdemon-daemon/src/process.rs` | implementor |
| 3 | [03-relax-clippy-ci](tasks/03-relax-clippy-ci.md) | Not Started | — | 0.5h | `.github/workflows/ci.yml`, `workflow/plans/bugs/clippy-rust-191-cleanup/BUG.md` (NEW) | implementor |
| 4 | [04-shim-installer-real-fix](tasks/04-shim-installer-real-fix.md) | Not Started | — | 2.5-3h | `crates/fdemon-daemon/src/flutter_sdk/locator.rs` | implementor |
| 5 | [05-windows-tests-cleanup](tasks/05-windows-tests-cleanup.md) | Not Started | 1, 4 | 1.5h | `crates/fdemon-daemon/src/flutter_sdk/windows_tests.rs` | implementor |
| 6 | [06-diagnostic-surface-polish](tasks/06-diagnostic-surface-polish.md) | Not Started | 1, 2, 4 | 1.5-2h | `crates/fdemon-daemon/src/devices.rs`, `crates/fdemon-daemon/src/flutter_sdk/locator.rs`, `crates/fdemon-daemon/src/flutter_sdk/diagnostics.rs`, `crates/fdemon-daemon/src/process.rs` | implementor |
| 7 | [07-windowsbatch-doc-honesty](tasks/07-windowsbatch-doc-honesty.md) | Not Started | — | 0.25h | `crates/fdemon-daemon/src/flutter_sdk/types.rs` | implementor |
| 8 | [08-pin-actions-shas](tasks/08-pin-actions-shas.md) | Not Started | 3 | 0.5h | `.github/workflows/ci.yml`, `.github/workflows/e2e.yml`, `.github/workflows/release.yml` | implementor |
| 9 | [09-update-architecture-doc](tasks/09-update-architecture-doc.md) | Not Started | 1, 4, 6 | 0.5h | `docs/ARCHITECTURE.md` | doc_maintainer |

**Estimated total:** 8.75-10h

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|---------------------------|
| 01-emulators-diagnostic-pattern | `crates/fdemon-daemon/src/emulators.rs`, `crates/fdemon-daemon/src/flutter_sdk/diagnostics.rs` (new), `crates/fdemon-daemon/src/devices.rs`, `crates/fdemon-daemon/src/flutter_sdk/mod.rs` | — |
| 02-demote-args-logging | `crates/fdemon-daemon/src/process.rs` | — |
| 03-relax-clippy-ci | `.github/workflows/ci.yml`, `workflow/plans/bugs/clippy-rust-191-cleanup/BUG.md` (new) | — |
| 04-shim-installer-real-fix | `crates/fdemon-daemon/src/flutter_sdk/locator.rs` | `crates/fdemon-daemon/src/flutter_sdk/types.rs` |
| 05-windows-tests-cleanup | `crates/fdemon-daemon/src/flutter_sdk/windows_tests.rs` | `crates/fdemon-daemon/src/flutter_sdk/locator.rs`, `crates/fdemon-daemon/src/flutter_sdk/diagnostics.rs` |
| 06-diagnostic-surface-polish | `crates/fdemon-daemon/src/devices.rs`, `crates/fdemon-daemon/src/flutter_sdk/locator.rs`, `crates/fdemon-daemon/src/flutter_sdk/diagnostics.rs`, `crates/fdemon-daemon/src/process.rs` | — |
| 07-windowsbatch-doc-honesty | `crates/fdemon-daemon/src/flutter_sdk/types.rs` | — |
| 08-pin-actions-shas | `.github/workflows/ci.yml`, `.github/workflows/e2e.yml`, `.github/workflows/release.yml` | — |
| 09-update-architecture-doc | `docs/ARCHITECTURE.md` | All implementation files (read for change context) |

### Overlap Matrix

Compared pairwise within each wave (tasks that have no dependency between them).

**Wave A — `01`, `02`, `03`, `04`, `07`:**

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|-------------------|-------------------|
| 01 + 02 | None (`emulators.rs`/`diagnostics.rs`/`devices.rs`/`mod.rs` vs `process.rs`) | Parallel (worktree) |
| 01 + 03 | None (`emulators.rs`/`diagnostics.rs`/`devices.rs`/`mod.rs` vs `ci.yml`/new BUG.md) | Parallel (worktree) |
| 01 + 04 | None (`emulators.rs`/`diagnostics.rs`/`devices.rs`/`mod.rs` vs `locator.rs`) | Parallel (worktree) |
| 01 + 07 | None (`emulators.rs`/`diagnostics.rs`/`devices.rs`/`mod.rs` vs `types.rs`) | Parallel (worktree) |
| 02 + 03 | None (`process.rs` vs `ci.yml`/new BUG.md) | Parallel (worktree) |
| 02 + 04 | None (`process.rs` vs `locator.rs`) | Parallel (worktree) |
| 02 + 07 | None (`process.rs` vs `types.rs`) | Parallel (worktree) |
| 03 + 04 | None (`ci.yml`/new BUG.md vs `locator.rs`) | Parallel (worktree) |
| 03 + 07 | None (`ci.yml`/new BUG.md vs `types.rs`) | Parallel (worktree) |
| 04 + 07 | None (`locator.rs` vs `types.rs`) | Parallel (worktree) |

**Wave B — `05` and `06`:**

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|-------------------|-------------------|
| 05 + 06 | None (`windows_tests.rs` vs `devices.rs`/`locator.rs`/`diagnostics.rs`/`process.rs`) | Parallel (worktree) |

**Wave C — `08` (single task):** No worktree; runs alone on the working branch (it modifies `ci.yml` already touched by `03` in Wave A — but Wave A is merged before Wave C starts, so no live conflict).

**Wave D — `09` (single doc_maintainer task):** No worktree.

**Cross-wave:** Tasks in later waves depend on earlier waves and run after them — no overlap concerns.

**Note on Wave B isolation:** Both tasks in Wave B (`05` and `06`) write to entirely disjoint files. Wave B can run as a fully-parallel worktree wave once Wave A is merged.

**Note on `06`'s overlap with Wave-A files:** Task 06 modifies `devices.rs` (also touched by `01` to extract `windows_hint()`), `locator.rs` (also touched by `04` for the shim fix), `diagnostics.rs` (created by `01`), and `process.rs` (also touched by `02` to demote args logging). These are not concurrent overlaps — Wave A merges first, then Wave B starts on a branch that already has Wave A's changes. Standard wave sequencing handles this cleanly.

## Success Criteria

The follow-up is complete when:

- [ ] `emulators.rs` non-zero-exit and spawn-error branches include the binary path and (on Windows) the hint, mirroring `devices.rs`.
- [ ] `process.rs` no longer logs `args = ?args` at `info!` level. `binary` and `cwd` remain at `info!`.
- [ ] `cargo clippy --workspace --all-targets` exits 0 (without `-D warnings`) on the new CI matrix; the dedicated cleanup bug-plan exists at `workflow/plans/bugs/clippy-rust-191-cleanup/BUG.md`.
- [ ] On Windows hosts with scoop or winget Flutter installations, `find_flutter_sdk` succeeds via the new binary-only fallback (Strategy 12). Two new Windows-only unit tests cover the scoop and winget shim layouts.
- [ ] All tests in `windows_tests.rs` follow the `test_<function>_<scenario>_<expected_result>` convention. The spaces-in-path test asserts the argument was actually passed (not just that spawn succeeded). All `unwrap()` calls have `.expect("...")` messages. `#[serial]` annotations have rationale comments.
- [ ] `windows_hint()` is appended only when stderr matches a path-resolution-error pattern. `try_system_path()` is called once per `find_flutter_sdk` invocation. `process.rs` emits a Windows-targeted message for `ErrorKind::InvalidInput`. ANSI sequences are stripped from stderr before embedding in error strings. `try_system_path()` doc comment mentions the explicit `[flutter] sdk_path` mitigation.
- [ ] `FlutterExecutable::WindowsBatch` doc-comment is honest about its current operationally-identical-to-`Direct` semantics.
- [ ] All third-party GitHub Actions in `.github/workflows/*.yml` are pinned to commit SHAs.
- [ ] `docs/ARCHITECTURE.md` reflects the new `flutter_sdk/diagnostics.rs` module, the new Strategy 12 (binary-only fallback), and the stderr-gated hint behavior.
- [ ] `cargo fmt --all && cargo check --workspace && cargo test --workspace` passes on all three CI platforms.
- [ ] Reporters of #32 and #34 are pinged with a verification build (manual, post-merge).

## Notes

- **Workspace clippy cleanup is intentionally out of scope.** It will be tracked in `workflow/plans/bugs/clippy-rust-191-cleanup/` (created as part of Task 03). Restoring `-D warnings` to the CI clippy step happens when that bug ships.
- **No Windows machine is available locally.** Scoop and winget shim layouts are simulated via `tempfile::TempDir` + a `flutter.bat` shim, then `which::which` + the new Strategy 12 are exercised. Real-host validation depends on the Windows CI runner.
- **The `WindowsBatch` enum variant remains.** Task 07 only updates the doc-comment to reflect reality; the variant itself is preserved per the user's earlier decision to avoid API churn.
- **Doc updates for `docs/DEVELOPMENT.md`** are not required by this follow-up — the original Wave-1 task already updated MSRV and the CI section. Only `docs/ARCHITECTURE.md` needs a refresh.
- **Issue follow-up (commenting on #32 and #34)** remains a manual human task once the binary is built and uploaded.
