# Phase 4 Followup-2 — Review Remediation (round 2) — Task Index

## Overview

These tasks remediate the confirmed findings from the **re-review of the first
Phase 4 followup** (`workflow/reviews/features/phase-4-prereq-followup/REVIEW.md`
+ `ACTION_ITEMS.md`). Five specialized reviewers fanned out over the first-round
remediation diff (`004f4a2..8f5b950`); the pivotal finding (F1, partial M1 fix) and
the two rejected "clippy" claims were adversarially re-verified against live code.

Result: **3 MINOR, 3 NITPICK** confirmed + **1 deferred** (D1, out of scope). No
CRITICAL, no security findings, no regressions; the first-round quality gate is green.

The headline item is **F1**: the first-round M1 fix sized the guided section to the
command count and clamped it, but `render_guided_commands` has **no scroll-to-selected**
logic — so on a terminal shorter than the full section, the *selected* command and its
`[c] copy` hint can still clip while `c` copies it. This re-opens the original M1
visible/copied divergence in the short-terminal regime.

**Total Tasks:** 7 (4 required, 2 optional NITPICK, 1 doc)
**Estimated Hours:** 8–13 hours

## Finding → Task Map

| Finding | Sev | Task |
|---------|-----|------|
| F3 duplicated caption derivation (drift hazard) | MINOR | 01 |
| F1 M1 fix partial — no scroll-to-selected; selected cmd clips on short terminals | MINOR | 02 |
| F2 `LinuxPackageManager` reached via `fdemon_daemon::` in TUI test fixtures | MINOR | 03 |
| N1 redundant `missing_binaries.clone()` | NITPICK (optional) | 04 |
| N2 best-effort caveat asymmetry across PM arms | NITPICK (optional) | 05 |
| N3 `winget_available: bool` vs `Option<…>` asymmetry | NITPICK (optional) | 06 |
| F2 doc note: ARCHITECTURE.md "four toolchain display types" stale (now five) | MINOR | 07 |
| D1 Windows `status==Ok` real `vswhere.exe` probe | (deferred) | — (not planned) |

## Task Dependency Graph

```
Wave 1 (parallel, disjoint files)        Wave 2        Wave 3        Wave 4
┌──────────────────────────────┐         ┌─────────┐   ┌─────────┐   ┌─────────────────┐
│ 01 F3 step_detail (helper)   │────────▶│ 02 F1   │──▶│ 03 F2   │──▶│ 07 ARCH doc     │
│ 04 N1 prerequisites.rs       │         │ scroll  │   │ reexport│   │ (doc_maint.)    │
│ 05 N2 state.rs (caveats)     │         │ window  │   │ +fixtures│  │ 06 N3 (optional)│
└──────────────────────────────┘         └─────────┘   └─────────┘   └─────────────────┘
   (01,04,05 disjoint files)             (needs 01 —   (needs 02 —   (07 needs 03; 06
                                          same file)    same file)    cross-cutting, last)
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Required? | Modules |
|---|------|--------|------------|------------|-----------|---------|
| 01 | [01-extract-step-caption-helper](tasks/01-extract-step-caption-helper.md) | Done ✅ | - | 1h | Yes | `widgets/install_wizard/step_detail.rs` |
| 02 | [02-scroll-window-selected-command](tasks/02-scroll-window-selected-command.md) | Done ✅ | 01 | 3-4h | Yes | `widgets/install_wizard/step_detail.rs` |
| 03 | [03-reexport-linux-package-manager](tasks/03-reexport-linux-package-manager.md) | Done ✅ | 02 | 1-2h | Yes | `install_wizard/mod.rs`, `widgets/install_wizard/step_detail.rs` (tests), `widgets/install_wizard/mod.rs` (tests) |
| 04 | [04-prereq-clone-to-move](tasks/04-prereq-clone-to-move.md) | Done ✅ | - | 0.5h | Optional | `toolchain/checks/prerequisites.rs` |
| 05 | [05-pm-caveat-symmetry](tasks/05-pm-caveat-symmetry.md) | Done ✅ | - | 0.5-1h | Optional | `install_wizard/state.rs` |
| 06 | [06-winget-available-option](tasks/06-winget-available-option.md) | Deferred ⏸️ | 02, 03, 05 | 2-3h | Optional (deferrable) | `toolchain/types.rs`, `toolchain/mod.rs`, `install_wizard/state.rs`, app+tui fixtures |
| 07 | [07-update-architecture-note](tasks/07-update-architecture-note.md) | Done ✅ | 03 | 0.5h | Yes | `docs/ARCHITECTURE.md` |

> **06 deferred (not dropped):** N3 (`winget_available: bool` → `Option<bool>`) is the
> explicitly-deferrable cross-cutting NITPICK. It touches every `ToolchainReport`
> construction site for a documentation-grade asymmetry the risks reviewer rated
> "acceptable" as-is. Deferred by orchestrator scope decision; tracked here for a future
> followup. The `false`-conflation is already documented on the field.

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|---------------------------|
| 01 | `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs` | — |
| 02 | `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs` | task 01's `step_caption` helper; `fdemon-app::install_wizard` (`selected_command_index`, `GuidedCommand`) |
| 03 | `crates/fdemon-app/src/install_wizard/mod.rs`, `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs` (test module only), `crates/fdemon-tui/src/widgets/install_wizard/mod.rs` (test module only) | `fdemon_daemon::toolchain::LinuxPackageManager` |
| 04 | `crates/fdemon-daemon/src/toolchain/checks/prerequisites.rs` | — |
| 05 | `crates/fdemon-app/src/install_wizard/state.rs` | `fdemon-daemon` `LinuxPackageManager` arms |
| 06 | `crates/fdemon-daemon/src/toolchain/types.rs`, `crates/fdemon-daemon/src/toolchain/mod.rs`, `crates/fdemon-app/src/install_wizard/state.rs`, `crates/fdemon-app/src/handler/install_wizard/{actions.rs,navigation.rs}` (fixtures), `crates/fdemon-tui/src/widgets/install_wizard/{step_detail.rs,mod.rs}` (fixtures) | every `ToolchainReport` construction site |
| 07 | `docs/ARCHITECTURE.md` | final state of task 03 |

### Overlap Matrix

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|-------------------|--------------------|
| 01 + 04 | None (`step_detail.rs` vs `prerequisites.rs`) | **Parallel (worktree)** |
| 01 + 05 | None (`step_detail.rs` vs `state.rs`) | **Parallel (worktree)** |
| 04 + 05 | None (`prerequisites.rs` vs `state.rs`) | **Parallel (worktree)** |
| 02 vs 01 | `step_detail.rs` | **Sequential** — enforced by 02→01 dep edge |
| 03 vs 02 | `step_detail.rs` (test module) | **Sequential** — enforced by 03→02 dep edge |
| 06 vs 02 | `step_detail.rs` (test fixtures) | **Sequential** — enforced by 06→02 dep edge |
| 06 vs 03 | `step_detail.rs` + tui `mod.rs` (fixtures) | **Sequential** — enforced by 06→03 dep edge |
| 06 vs 05 | `state.rs` | **Sequential** — enforced by 06→05 dep edge |
| 07 vs 06 | None (`ARCHITECTURE.md` vs code) | **Parallel (worktree)** — both in final wave |

**Isolation note:** The only safe parallel set is **Wave 1 (01, 04, 05)** — three
disjoint files across the three crates (tui / daemon / app). Tasks 02 and 03 re-touch
`step_detail.rs` and are serialized by dependency edges. **Task 06 (N3) is
cross-cutting** — changing the `winget_available` field type touches every
`ToolchainReport` construction site (run_preflight + all app/tui test fixtures) and the
`state.rs` consumer, so it overlaps 02, 03, and 05 and must run last. It is an **optional
NITPICK** and may be deferred entirely without blocking. Task 07 (docs) runs in the final
wave; it is disjoint from 06 so the two may run in parallel.

## Suggested Wave Schedule

- **Wave 1 (parallel):** 01, 04, 05
- **Wave 2:** 02 (after 01)
- **Wave 3:** 03 (after 02)
- **Wave 4 (parallel):** 07 (after 03) + 06 (optional; after 02, 03, 05)

## Success Criteria

Followup-2 is complete when:

- [ ] **F1:** On a short detail pane (≈10–12 rows) with a `Prerequisites` component and
      3 guided commands at `selected_command_index = 2`, the selected command's row, its
      highlight, and its `[c] copy` hint are all visible (a scroll window anchored to the
      selected index), and `c` copies a command the user can see. The saturating clamp is
      preserved; tall-terminal behavior is unchanged.
- [ ] **F3:** A single caption-deriving function (e.g. `step_caption(kind)`) is the sole
      source of truth, called by both `guided_section_full_height` and
      `render_guided_commands`; no duplicated `matches!(kind, AndroidTools | Prerequisites)`.
- [ ] **F2:** `LinuxPackageManager` is re-exported through `fdemon-app::install_wizard`;
      no `fdemon_daemon::` path for it remains anywhere in `fdemon-tui` (production or test);
      ARCHITECTURE.md's display-types note is accurate (task 07).
- [ ] **N1/N2/N3 (optional):** if taken — the redundant clone is removed; the best-effort
      caveat is symmetric across non-apt PM arms; `winget_available` is symmetric with
      `linux_package_manager`. Any deferred NITPICK is explicitly noted, not silently dropped.
- [ ] `cargo fmt --all -- --check`, `cargo check --workspace --all-targets`,
      `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`
      all pass; no regressions.

## Notes

- **D1 (Windows `status==Ok` real `vswhere.exe` probe) is intentionally NOT planned here.**
  It is the agreed Phase 4 interim (note-only mitigation already shipped). Track it as a
  separate future task; label it "mitigated/deferred", not "resolved". See
  `workflow/reviews/features/phase-4-prereq-followup/ACTION_ITEMS.md` D1.
- The two reviewer claims **rejected** by verification (clippy `redundant_clone` /
  `collapsible_else_if` as gate-failing) are NOT in scope. `redundant_clone` survives only
  as NITPICK N1 (style); `collapsible_else_if` does not fire. Do not "fix" the else-if.
- Optional NITPICK tasks (04, 05, 06) may be deferred by the implementor/orchestrator
  without blocking the required tasks (01, 02, 03, 07).
