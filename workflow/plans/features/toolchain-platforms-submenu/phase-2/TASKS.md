# Phase 2 — Platforms parent + expand/collapse + Android leaf — Task Index

## Overview

Replace the single `AndroidTools` step with an expandable **Platforms** submenu. Add
`WizardStepKind::Platforms` (non-executable parent) + per-platform leaf kinds
(`PlatformAndroid`, `PlatformIos`, `PlatformMacos`, `PlatformWeb`, `PlatformWindows`), rename the
existing Android behaviour to `PlatformAndroid`, and add inline expand/collapse. In Phase 2 only the
**Android** leaf is functional; the others are host-gated **placeholder** rows (`Pending`, inert).

**Data model (decided):** `state.steps` stays a flat `Vec<WizardStep>`; add `platforms_expanded: bool`
to `InstallWizardState` and `indent: u8` to `WizardStep`. `build_steps(report, expanded)` returns the
**already-projected visible list**: collapsed → parent only (`[Prerequisites, Platforms, FlutterSdk,
PathConfig, Doctor]` = 5 rows); expanded → parent + host-applicable leaves inserted after it. The
parent's status rolls up its leaves' statuses (placeholders' `Pending` is neutral). Host gating uses
`report.platform` (never `cfg!`) so `build_steps` stays a pure, testable function.

**Why these task boundaries:** the new `WizardStepKind` variants hard-error at two exhaustive `match`
sites (`handle_run_selected_step`, the `RunWizardStep` executor), so the enum + rename + all forced
arms must land as **one compiling unit** (Task 01). Interactivity (Task 02, fdemon-app) and rendering
(Task 03, fdemon-tui) touch disjoint files and parallelize after 01.

**Total Tasks:** 4
**Estimated Hours:** 8–11 hours

## Task Dependency Graph

```
                ┌──────────────────────────────────────┐
                │ 01-enum-datamodel-rename (foundation) │   Wave 1
                │  types + state + build_steps + all    │
                │  forced match arms + test fixups      │
                └───────────────┬──────────────────────┘
                                │  (must compile + tests green)
              ┌─────────────────┴──────────────────┐
              ▼                                     ▼            Wave 2 (parallel)
 ┌──────────────────────────────┐   ┌────────────────────────────┐
 │ 02-expand-collapse-nav        │   │ 03-tui-indent-caret-height │
 │ (fdemon-app: msg/keys/nav)    │   │ (fdemon-tui: step_list/mod)│
 └───────────────┬──────────────┘   └──────────────┬─────────────┘
                 └──────────────┬───────────────────┘
                                ▼                                  Wave 3
                ┌──────────────────────────────────────┐
                │ 04-update-architecture-docs           │
                │ (doc_maintainer)                      │
                └──────────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-enum-datamodel-rename](tasks/01-enum-datamodel-rename.md) | ✅ Done (validated, committed `a5255f4`) | - | 3–4h | `install_wizard/{types,state}.rs`, `handler/install_wizard/actions.rs`, `actions/mod.rs`, `widgets/install_wizard/{step_detail,step_list}.rs` |
| 2 | [02-expand-collapse-nav](tasks/02-expand-collapse-nav.md) | ✅ Done (validated, merged `af86f55`) | 1 | 2–3h | `message.rs`, `handler/mod.rs`, `handler/install_wizard/navigation.rs`, `handler/keys.rs` |
| 3 | [03-tui-indent-caret-height](tasks/03-tui-indent-caret-height.md) | ✅ Done (validated, merged `0999100`) | 1 | 2–3h | `widgets/install_wizard/step_list.rs`, `widgets/install_wizard/mod.rs` |
| 4 | [04-update-architecture-docs](tasks/04-update-architecture-docs.md) | ✅ Done (validated, committed `ebc9f5c`) | 1, 2, 3 | 1h | `docs/ARCHITECTURE.md` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|----------------------|--------------------------|
| 01 | `crates/fdemon-app/src/install_wizard/types.rs`, `crates/fdemon-app/src/install_wizard/state.rs`, `crates/fdemon-app/src/handler/install_wizard/actions.rs`, `crates/fdemon-app/src/actions/mod.rs`, `crates/fdemon-tui/src/widgets/install_wizard/step_detail.rs`, `crates/fdemon-tui/src/widgets/install_wizard/step_list.rs` (test helper only) | — |
| 02 | `crates/fdemon-app/src/message.rs`, `crates/fdemon-app/src/handler/mod.rs`, `crates/fdemon-app/src/handler/install_wizard/navigation.rs`, `crates/fdemon-app/src/handler/keys.rs` | `install_wizard/{types,state}.rs` |
| 03 | `crates/fdemon-tui/src/widgets/install_wizard/step_list.rs`, `crates/fdemon-tui/src/widgets/install_wizard/mod.rs` | `install_wizard/{types,state}.rs` |
| 04 | `docs/ARCHITECTURE.md` | task 01–03 files, `~/.claude/skills/doc-standards/schemas.md` |

### Overlap Matrix

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|-------------------|-------------------|
| 01 + 02 | none (different files; 02 depends on 01) | Sequential (01 → 02) |
| 01 + 03 | `…/install_wizard/step_list.rs` | Sequential (01 → 03) |
| 02 + 03 | **none** | **Parallel (worktree)** after 01 |
| 04 vs all | none | Sequential (after 01–03) |

> 01 edits only the `make_steps()` **test helper** in `step_list.rs` (to add `indent: 0` + the
> `PlatformAndroid` rename so the crate compiles); 03 edits the render code + render tests in the same
> file. They share the file, so 03 is sequenced after 01 (not parallel with it). 02 and 03 write
> disjoint files and run in parallel once 01 is merged.

## Success Criteria

Phase 2 is complete when:

- [ ] `WizardStepKind` has `Platforms` + 5 `Platform*` leaves; `AndroidTools` is fully renamed to `PlatformAndroid`.
- [ ] `build_steps(report, expanded=false)` returns `[Prerequisites, Platforms, FlutterSdk, PathConfig, Doctor]`;
      `expanded=true` inserts host-applicable leaves after the parent (Android + Web on all hosts; iOS + macOS
      on macOS; Windows on Windows).
- [ ] The Platforms parent's rolled-up status reflects its leaves (placeholders' `Pending` is neutral).
- [ ] `Enter` on the Platforms parent expands/collapses; leaves are reachable only when expanded.
- [ ] `Esc` collapses an expanded submenu before closing the wizard; collapsing clamps `selected_index`
      back onto a visible row.
- [ ] The Android leaf retains the existing managed install + JDK gate (now on `PlatformAndroid`); the
      placeholder leaves show "Available in a later phase" and never hit the `WizardStepFailed` path incorrectly.
- [ ] The step list renders the parent with an expand/collapse caret and indents leaf rows; the step-list
      pane height is dynamic; the footer hints expand/collapse on the parent.
- [ ] `cargo test --workspace --lib` green; `cargo fmt --all` + `cargo clippy --workspace -- -D warnings` clean.
- [ ] `docs/ARCHITECTURE.md` documents the Platforms submenu + new `WizardStepKind` variants.

## Notes

- **Phase 2 keeps only Android functional.** iOS/macOS/Web/Windows leaves are inert placeholders here;
  their detection + guided commands arrive in Phases 3–5.
- **Host gating** in `build_steps` must read `report.platform` (the `ToolchainReport` field), never
  `cfg!(target_os=…)` — the function is pure and tested across simulated hosts via `make_report_for_platform`.
- **Compiler is your safety net:** the two exhaustive matches (`handle_run_selected_step` in `actions.rs`,
  the `RunWizardStep` executor in `actions/mod.rs`) will fail to build until every new variant has an arm.
- **Website docs deferred:** `website/src/pages/docs/toolchain.rs` still says "2. Android Tools". It will be
  rewritten for the full Platforms submenu once the platform leaves carry real content (Phases 3–5), to
  avoid editing it twice. (Tracked as a Phase-5 / wrap-up docs task.)
- Snapshot line numbers in the task files **will drift** (Phase 1 already shifted some) — locate by
  symbol/test-name/variant, not absolute line.
- **Retire the literal `selected_index = N` test pattern (Phase 1 review LOW-1).** The Platforms reshuffle
  invalidates the ~20 hardcoded index literals renumbered in Phase 1. Task 01 must migrate every test it
  touches to `position(|s| s.kind == …)` kind-lookup rather than hand-renumbering literals again — this
  removes the silent-mis-target trap where two adjacent steps both satisfy a weak assertion. See
  `workflow/reviews/features/toolchain-platforms-submenu-phase-1/REVIEW.md` and Task 01 §6 + acceptance #6.
