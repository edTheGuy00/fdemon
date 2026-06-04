# Phase 4 Followup — Review Remediation — Task Index

## Overview

These tasks remediate the verified findings from the Phase 4 (OS prerequisites)
code review (`workflow/reviews/features/phase-4-prereq/REVIEW.md`). The review
fanned out 8 specialized reviewers across the `129e66e..HEAD` diff and
adversarially verified every finding (2 false positives were rejected). Result:
**2 MAJOR, 10 MINOR, 4 NITPICK** — no CRITICAL, no security issues, all core
detection/navigation logic confirmed correct.

These are remediation tasks, not new feature work. The two MAJOR findings (M1
render clipping, M2 Yum command) are genuine correctness/UX defects but neither
panics nor corrupts data; the rest are consistency, purity, test-quality, and
documentation polish.

**Total Tasks:** 6
**Estimated Hours:** 11–17 hours

## Finding → Task Map

| Finding | Sev | Task |
|---------|-----|------|
| M1 multi-command guided section clips all but first command | MAJOR | 01 |
| m5 `GUIDED_COMMAND_MIN_HEIGHT` doc misdescribes rows | MINOR | 01 |
| M2 Yum arm emits a `dnf` command that fails on yum-only systems | MAJOR | 02 |
| n2 community package names lack best-effort caveat | NITPICK | 02 |
| m6 `PREREQ_KEY_GIT` via full path instead of import | MINOR | 02 |
| m2 Linux detail uses `Missing:` not `MISSING_PREFIX` | MINOR | 03 |
| m3 Linux `Partial` vs macOS/Windows `Missing` for absent tools | MINOR | 03 |
| m4 Windows reports `Ok` without VS C++ workload (false-Ok) | MINOR | 03 |
| n1 GTK absence double-reported when `pkg-config` missing | NITPICK | 03 |
| m1 `which::which` filesystem I/O inside TEA `update()` | MINOR | 04 |
| n3 stringly-typed detail cross-crate contract (deferred note) | NITPICK | 04 |
| n4 `which` added as `fdemon-app` dependency | NITPICK | 04 |
| m7 missing `[`/`]` key-mapping tests | MINOR | 05 |
| m8 `test_non_android_steps_have_no_guided_commands` false invariant | MINOR | 05 |
| m9 `test_package_manager_precedence_apt_before_dnf` no assertion | MINOR | 05 |
| m10 ARCHITECTURE.md module-table stale for Phase 4 | MINOR | 06 |

## Task Dependency Graph

```
Wave 1 (parallel, disjoint files)      Wave 2        Wave 3        Wave 4
┌────────────────────────────┐         ┌─────────┐   ┌─────────┐   ┌──────────────┐
│ 01 step_detail.rs (clip)   │         │ 04 TEA  │   │ 05 test │   │ 06 ARCH docs │
│ 02 state.rs (guided cmds)  │────────▶│ purity  │──▶│ quality │──▶│ (doc_maint.) │
│ 03 prerequisites.rs (detect)│────────▶│         │   │         │   │              │
└────────────────────────────┘         └─────────┘   └─────────┘   └──────────────┘
                                        (needs 02,03  (needs 04 —   (needs 04,05 —
                                         shared files) shared files) final shape)
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 01 | [01-fix-multicommand-guided-clipping](tasks/01-fix-multicommand-guided-clipping.md) | Not Started | - | 3-5h | `widgets/install_wizard/step_detail.rs` |
| 02 | [02-polish-prerequisites-guided-commands](tasks/02-polish-prerequisites-guided-commands.md) | Not Started | - | 2-3h | `install_wizard/state.rs` |
| 03 | [03-refine-prereq-detection-status](tasks/03-refine-prereq-detection-status.md) | Not Started | - | 2-3h | `toolchain/checks/prerequisites.rs` |
| 04 | [04-pure-guided-commands-tea](tasks/04-pure-guided-commands-tea.md) | Not Started | 02, 03 | 3-4h | `toolchain/types.rs`, `toolchain/mod.rs`, `toolchain/checks/prerequisites.rs`, `install_wizard/state.rs`, `fdemon-app/Cargo.toml` |
| 05 | [05-test-quality-fixes](tasks/05-test-quality-fixes.md) | Not Started | 04 | 1-2h | `handler/keys.rs`, `install_wizard/state.rs`, `toolchain/checks/prerequisites.rs` |
| 06 | [06-update-architecture-docs](tasks/06-update-architecture-docs.md) | Not Started | 04, 05 | 1h | `docs/ARCHITECTURE.md` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|---------------------------|
| 01 | `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs` | `fdemon-app::install_wizard` (`selected_command_index`, `GuidedCommand`) |
| 02 | `crates/fdemon-app/src/install_wizard/state.rs` | `fdemon-daemon` `LinuxPackageManager`, `PREREQ_KEY_*` |
| 03 | `crates/fdemon-daemon/src/toolchain/checks/prerequisites.rs` | `toolchain/types.rs` (`ComponentStatus`) |
| 04 | `crates/fdemon-daemon/src/toolchain/types.rs`, `crates/fdemon-daemon/src/toolchain/mod.rs`, `crates/fdemon-daemon/src/toolchain/checks/prerequisites.rs`, `crates/fdemon-app/src/install_wizard/state.rs`, `crates/fdemon-app/Cargo.toml` | `toolchain/checks/mod.rs`, `fdemon-daemon/src/lib.rs` |
| 05 | `crates/fdemon-app/src/handler/keys.rs`, `crates/fdemon-app/src/install_wizard/state.rs`, `crates/fdemon-daemon/src/toolchain/checks/prerequisites.rs` | task 04 changes |
| 06 | `docs/ARCHITECTURE.md` | tasks 01–05 (final module shape) |

### Overlap Matrix

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|-------------------|--------------------|
| 01 + 02 | None (`step_detail.rs` vs `state.rs`) | **Parallel (worktree)** |
| 01 + 03 | None (`step_detail.rs` vs `prerequisites.rs`) | **Parallel (worktree)** |
| 02 + 03 | None (`state.rs` vs `prerequisites.rs`) | **Parallel (worktree)** |
| 04 vs 02 | `install_wizard/state.rs` | **Sequential** — enforced by 04→02 dep edge |
| 04 vs 03 | `prerequisites.rs` | **Sequential** — enforced by 04→03 dep edge |
| 05 vs 04 | `prerequisites.rs`, `state.rs` | **Sequential** — enforced by 05→04 dep edge |

**Isolation note:** Wave 1 (01, 02, 03) writes three disjoint files and is the
only safe parallel set. Tasks 04 and 05 are serialized by dependency edges
because they re-touch `state.rs` and `prerequisites.rs`. Task 06 (docs) runs last
so it captures the final module shape after the 04 TEA refactor.

## Suggested Wave Schedule

- **Wave 1 (parallel):** 01, 02, 03
- **Wave 2:** 04 (after 02, 03)
- **Wave 3:** 05 (after 04)
- **Wave 4:** 06 (after 04, 05)

## Success Criteria

Followup is complete when:

- [ ] **M1:** On the real macOS Prerequisites path (a `Prerequisites` component
      present + 2–3 guided commands), pressing `]`/`[` keeps the *selected* command,
      its highlight, and its `[c]` copy hint visible; `c` copies a command the user
      can see. A test mirrors `make_state_prerequisites_macos_three_commands` but
      **with** a `Prerequisites` component and `selected_command_index = 2`.
- [ ] **M2:** The `LinuxPackageManager::Yum` arm emits a command that runs on a
      yum-only system (a real `yum install …`), or carries an explicit substitution
      note; the one platform class that reaches this arm gets a working command.
- [ ] Linux detail/status semantics are consistent with macOS/Windows (or the
      divergence is documented at the source); the GTK probe no longer asserts
      definitive absence when `pkg-config` itself is missing.
- [ ] Windows `Prerequisites = Ok` no longer overstates readiness (the Ok detail
      flags the unverified VS C++ workload).
- [ ] `prerequisites_guided_commands` is a pure function of the report (no
      `which::which` PATH I/O in the `update()` path), **or** the I/O call sites
      carry an explicit `// EXCEPTION:` annotation referencing `docs/REVIEW_FOCUS.md`.
- [ ] `[`/`]` key mappings have tests; misleadingly-named tests are renamed and
      strengthened; package-manager precedence is testable.
- [ ] ARCHITECTURE.md module-table reflects the Phase 4 (and any 04-refactor) additions.
- [ ] `cargo fmt --all -- --check`, `cargo check --workspace --all-targets`,
      `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`
      all pass; no regressions.

## Notes

- **Optional/NITPICK items** (n1 GTK double-report, n2 caveat note, n3 typed
  missing-keys field) are folded into their nearest task as explicitly-optional
  sub-items — implementors may defer them without blocking the task. n3 in
  particular is tracked as future hardening, not a blocker.
- The two findings the adversarial verifier **rejected** (the `bottom_area`
  saturating-arithmetic "simplification" and the `is_jdk_actionable` doc comment)
  are NOT in scope — they were confirmed non-issues. Do not "fix" them.
