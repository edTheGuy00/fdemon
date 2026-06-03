# Phase 1 — Review Fixes — Task Index

## Overview

Follow-up tasks addressing the code review of Toolchain Bootstrap Phase 1
(`workflow/reviews/features/toolchain-bootstrap-phase-1/REVIEW.md` +
`ACTION_ITEMS.md`). The review verdict was **NEEDS WORK** — no blockers, but three MAJOR
should-fix-before-merge defects (orphaned `flutter doctor` process on timeout, auto-launch +
missing-SDK dead-end, missing re-run guard) plus a cluster of MINOR/nitpick cleanups.

This batch covers **all findings** (MAJOR + MINOR + nitpicks). Finding **m4** (the new
`fdemon-tui → fdemon-daemon` runtime dependency) is resolved by **re-exporting** the four
display types through `fdemon-app` and repointing the TUI imports — removing the layer-boundary
deviation rather than documenting an exception.

**Total Tasks:** 5
**Estimated Hours:** 14–19 hours

## Finding → Task Map

| Finding | Severity | Task |
|---------|----------|------|
| M1 — kill timed-out `flutter doctor` child + cap output reads + fix comment | MAJOR | 01 |
| n11 — cap `DoctorLine::indent` allocation | NITPICK | 01 |
| n13 — consolidate the duplicated `strip_ansi` | NITPICK | 01 |
| m5 — JDK unparseable-major classifies as `Ok` (should be `Partial`) | MINOR | 02 |
| m8 — flaky env-mutation test (serialize / isolate) | MINOR | 02 |
| n12 — strip ANSI from `ComponentCheck::detail` | NITPICK | 02 |
| m10 — remove duplicate `test_host_platform_detect_matches_cfg` | MINOR | 02 |
| m7 — split `checks.rs` (962 LOC) into `checks/android.rs` | MINOR | 03 |
| M3 — re-entrancy guard on `r` re-run | MAJOR | 04 |
| m6 — remove dead `_effective` binding | MINOR | 04 |
| m4 (app side) — re-export daemon display types from `fdemon-app` | MINOR | 04 |
| m9 — register the new `Cell` render-hint in `REVIEW_FOCUS.md` | MINOR | 04 |
| M2 — open the wizard on auto-launch + missing-SDK path | MAJOR | 05 |
| m4 (tui side) — repoint imports to `fdemon_app`, drop runtime dep | MINOR | 05 |
| n14 — remove unused `_selected_index` param | NITPICK | 05 |
| n15 — simplify redundant Doctor-step clamp math | NITPICK | 05 |

## Task Dependency Graph

```
        ┌─────────────────────────────────┐     ┌─────────────────────────────────┐
        │ 01-doctor-process-memory-        │     │ 04-app-handler-fixes-and-        │
        │    hardening (fdemon-daemon)     │     │    reexports (fdemon-app)        │
        └────────────────┬─────────────────┘     └────────────────┬─────────────────┘
                         ▼                                         ▼
        ┌─────────────────────────────────┐     ┌─────────────────────────────────┐
        │ 02-checks-correctness-ansi-test- │     │ 05-tui-startup-hook-and-cleanup  │
        │    isolation (fdemon-daemon)     │     │    (fdemon-tui)                  │
        └────────────────┬─────────────────┘     └─────────────────────────────────┘
                         ▼
        ┌─────────────────────────────────┐
        │ 03-split-checks-android          │
        │    (fdemon-daemon)               │
        └─────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Crate |
|---|------|--------|------------|------------|-------|
| 1 | [01-doctor-process-memory-hardening](tasks/01-doctor-process-memory-hardening.md) | Not Started | - | 3-4h | `fdemon-daemon` |
| 2 | [02-checks-correctness-ansi-test-isolation](tasks/02-checks-correctness-ansi-test-isolation.md) | Not Started | 1 | 3-4h | `fdemon-daemon` |
| 3 | [03-split-checks-android](tasks/03-split-checks-android.md) | Not Started | 2 | 2-3h | `fdemon-daemon` |
| 4 | [04-app-handler-fixes-and-reexports](tasks/04-app-handler-fixes-and-reexports.md) | Not Started | - | 3-4h | `fdemon-app` |
| 5 | [05-tui-startup-hook-and-cleanup](tasks/05-tui-startup-hook-and-cleanup.md) | Not Started | 4 | 3-4h | `fdemon-tui` |

## Execution Waves

| Wave | Tasks | Notes |
|------|-------|-------|
| 1 | 01 ∥ 04 | **Parallel** — `fdemon-daemon` doctor/diagnostics vs `fdemon-app` handlers/re-export. Disjoint files. |
| 2 | 02 ∥ 05 | **Parallel** — 02 (`fdemon-daemon` checks) depends on 01's shared `strip_ansi`; 05 (`fdemon-tui`) depends on 04's re-export. Disjoint files. |
| 3 | 03 | `checks.rs` split — depends on 02 (same file). |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|----------------------|--------------------------|
| 01 | `crates/fdemon-daemon/src/toolchain/doctor.rs`, `crates/fdemon-daemon/src/flutter_sdk/diagnostics.rs` | `toolchain/types.rs`, `toolchain/mod.rs`, `flutter_sdk/types.rs` |
| 02 | `crates/fdemon-daemon/src/toolchain/checks.rs`, `crates/fdemon-daemon/src/toolchain/mod.rs` | `flutter_sdk/diagnostics.rs` (shared `strip_ansi` from task 01), `toolchain/types.rs` |
| 03 | `crates/fdemon-daemon/src/toolchain/checks.rs`, `crates/fdemon-daemon/src/toolchain/checks/android.rs` (NEW), `crates/fdemon-daemon/src/toolchain/checks/mod.rs` (NEW, if a directory module is introduced) | `toolchain/mod.rs`, `toolchain/types.rs` |
| 04 | `crates/fdemon-app/src/handler/install_wizard/actions.rs`, `crates/fdemon-app/src/handler/install_wizard/navigation.rs`, `crates/fdemon-app/src/install_wizard/mod.rs`, `docs/REVIEW_FOCUS.md` | `crates/fdemon-app/src/state.rs`, `fdemon_daemon::toolchain` types |
| 05 | `crates/fdemon-tui/src/runner.rs`, `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs`, `crates/fdemon-tui/src/widgets/install_wizard/doctor_view.rs`, `crates/fdemon-tui/Cargo.toml` | task 04 re-exports (`fdemon_app::install_wizard::{DoctorLine, DoctorMarker, ComponentCheck, ComponentStatus}`), `crates/fdemon-tui/src/startup.rs`, `crates/fdemon-app/src/handler/update.rs` (StartAutoLaunch context) |

### Overlap Matrix

Only wave-peers matter. Concurrent pairs are **Wave 1 (01 + 04)** and **Wave 2 (02 + 05)**.

| Task Pair | Same Wave? | Shared Write Files | Isolation Strategy |
|-----------|-----------|-------------------|-------------------|
| 01 + 04 | Yes (Wave 1) | None (01 = `fdemon-daemon`, 04 = `fdemon-app`/docs) | **Parallel (worktree)** |
| 02 + 05 | Yes (Wave 2) | None (02 = `fdemon-daemon`, 05 = `fdemon-tui`) | **Parallel (worktree)** |
| 01 + 02 | No (dep chain) | `flutter_sdk/diagnostics.rs` read by 02, written by 01 — read-only overlap | Sequential (dependency) |
| 02 + 03 | No (dep chain) | `checks.rs`, `toolchain/mod.rs` — written by both | Sequential (dependency) |
| 04 + 05 | No (dep chain) | None — but 05 needs 04's re-export to compile | Sequential (dependency) |

> **Standalone-compile guarantee:** Each task leaves the workspace compiling green. Task 04's
> re-export is purely additive (compiles alone). Task 05 repoints the TUI imports to the new
> re-export **and** drops the `fdemon-daemon` runtime dep in the same task, so the TUI crate is
> never left referencing a removed dependency. Task 02 consumes the `strip_ansi` helper exposed by
> task 01; task 03 splits the file only after 02's correctness edits land.

## Success Criteria

This batch is complete when:

- [ ] **M1:** A timed-out `flutter doctor` leaves no lingering process; doctor stdout/stderr reads
      are byte-capped; the misleading "Kill the lingering process" comment is corrected.
- [ ] **M2:** Launching with `auto_launch` configured **and** no resolvable Flutter SDK opens
      `UiMode::InstallWizard` (not a silent no-op). Covered by a handler/runner test.
- [ ] **M3:** Pressing `r` while a preflight is already in flight does **not** spawn a second
      preflight (`handle_rerun_preflight` early-returns when `loading`).
- [ ] **m4:** `fdemon-tui` no longer has `fdemon-daemon` as a **runtime** dependency; the wizard
      widgets import the four display types from `fdemon_app::install_wizard::*`.
- [ ] **m5:** A JDK version string that yields no parseable major (e.g. bare `"1"`) classifies as
      `Partial`/`Error`, never `Ok`.
- [ ] **m7:** `toolchain/checks.rs` and the new Android submodule are each under the 500-line
      standard.
- [ ] **m8:** `cargo test --workspace` is deterministic under default (parallel) execution.
- [ ] **m9:** `InstallWizardState::last_known_visible_height` is listed in `REVIEW_FOCUS.md`
      "Current usage".
- [ ] **m10 / n11–n15:** addressed per their task files.
- [ ] Full quality gate green: `cargo fmt --all -- --check`,
      `cargo check --workspace --all-targets`, `cargo test --workspace`,
      `cargo clippy --workspace --all-targets -- -D warnings`.

## Notes

- **Scope discipline still applies:** this is still Phase 1 (read-only diagnostics). Do not add
  install/download/network code, new crate dependencies (beyond moving `fdemon-daemon` to a
  dev-dep in `fdemon-tui`), `[toolchain]` config keys, or step-execution bindings.
- **No `doc_maintainer` task needed:** with m4 resolved via re-export, `ARCHITECTURE.md`'s
  dependency matrix (tui → core + app) stays accurate. `REVIEW_FOCUS.md` (m9) is
  implementor-editable. If an implementor judges that the re-export pattern warrants a one-line
  note in `ARCHITECTURE.md`, flag it in the completion summary for a follow-up `doc_maintainer`
  pass rather than editing the managed doc directly.
