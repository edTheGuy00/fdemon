# Phase 4.5: Mouse Support Follow-up — Task Index

## Overview

Phase 4 of mouse-support shipped left-click handling for log view, DevTools sub-tabs, Inspector tree, Performance frame chart, and Network table. The implementation review (`workflow/reviews/features/mouse-support-phase-4/REVIEW.md`) returned **NEEDS WORK** with one critical correctness bug, six major findings, and nineteen minor findings. Phase 4.5 closes all of them so Phase 5 (modal dialogs / overlays at `z_index = 1`) starts from a clean baseline.

The critical defect is a wrap-mode misalignment: when `state.offset` lands inside a multi-row entry, log-view click regions sit in `all_lines` space rather than screen space, so clicking the visible row of entry B can resolve to entry A's region. The other major findings are duplicated logic (inspector layout-fetch, three sister functions duplicating `Widget::render` bodies), test gaps (no wrap-mode tests, missing 80×24 baselines, manual smoke test deferred), and a lint-suppression anti-pattern. Minor findings consolidate into hygiene tasks across `mouse_regions.rs`, the log-view handler, the DevTools mouse handler, two widgets, and PLAN/MOUSE doc updates.

**Total Tasks:** 10
**Estimated Hours:** ~11.75 hours

## Prerequisites

- Phase 4 must be merged on `feat/mouse-support`. All current Phase-4 production code is the baseline for these fixes.
- No new external dependencies. No new crate-level Cargo.toml changes.

## Task Dependency Graph

```
                ┌──────────────────────────────────────────────────────┐
                │                  No internal dependencies            │
                │  (all 10 tasks run in parallel — single wave)        │
                └──────────────────────────────────────────────────────┘

   ┌────┬────┬────┬────┬────┬────┬────┬────┬────┬────┐
   ▼    ▼    ▼    ▼    ▼    ▼    ▼    ▼    ▼    ▼
┌────┐┌────┐┌────┐┌────┐┌────┐┌────┐┌────┐┌────┐┌────┐┌────┐
│ 01 ││ 02 ││ 03 ││ 04 ││ 05 ││ 06 ││ 07 ││ 08 ││ 09 ││ 10 │
│wrap││ins-││sis-││80x ││man-││reg-││log-││dev-││tree││doc-│
│-fix││pec-││ter-││24  ││ual ││ist-││view││tools-│gly││drift│
│    ││tor-││fns ││base││smk-││ry  ││hand││handler│ph+││ +  │
│    ││re- ││re- ││line││test││doc-││+ st││polish│cst││MSE.│
│    ││fac-││fac-││+   ││    ││pol-││ate ││      │   ││md  │
│    ││tor ││tor ││fix ││    ││ish ││pol-││      │   ││    │
│    ││    ││    ││asrt││    ││    ││ish ││      │   ││    │
└────┘└────┘└────┘└────┘└────┘└────┘└────┘└────┘└────┘└────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Crate / Area |
|---|------|--------|------------|------------|--------------|
| 1 | [01-wrap-mode-click-region-fix](tasks/01-wrap-mode-click-region-fix.md) | Done | — | 2.0h | `fdemon-tui` |
| 2 | [02-inspector-handler-refactor](tasks/02-inspector-handler-refactor.md) | Done | — | 1.5h | `fdemon-app` |
| 3 | [03-sister-function-render-impl-refactor](tasks/03-sister-function-render-impl-refactor.md) | Done | — | 2.0h | `fdemon-tui` |
| 4 | [04-render-tests-baselines-and-tightening](tasks/04-render-tests-baselines-and-tightening.md) | Done | — | 1.0h | `fdemon-tui` |
| 5 | [05-manual-smoke-test](tasks/05-manual-smoke-test.md) | Blocked (manual) | — | 1.0h | docs |
| 6 | [06-mouse-regions-doc-polish](tasks/06-mouse-regions-doc-polish.md) | Done | — | 1.0h | `fdemon-app` |
| 7 | [07-log-view-handler-and-state-polish](tasks/07-log-view-handler-and-state-polish.md) | Done | — | 1.0h | `fdemon-app` |
| 8 | [08-devtools-mouse-handler-polish](tasks/08-devtools-mouse-handler-polish.md) | Done (concern) | — | 0.75h | `fdemon-app` |
| 9 | [09-tree-glyph-and-network-details-polish](tasks/09-tree-glyph-and-network-details-polish.md) | Done | — | 0.75h | `fdemon-tui` |
| 10 | [10-plan-drift-and-mouse-doc-updates](tasks/10-plan-drift-and-mouse-doc-updates.md) | Done | — | 0.75h | docs |

### Orchestration Notes

- **T05 (manual smoke test)** is **Blocked**: it requires interactive macOS terminal access with a live Flutter device. The implementor confirmed `cargo build` succeeds, pre-filled the 15-step results table with `NOT RUN` markers, and committed the Blocked status. A human must execute the smoke test manually and update the task file with results before phase merge to `main`.
- **T08 validator returned CONCERN (proceed-recommended)**: `crates/fdemon-app/src/handler/mouse/mod.rs` was widened from `&AppState` to `&mut AppState` to enable in-dispatcher mutation of `network.filter_input_active` without modifying `update.rs`. This file was not in T08's declared write scope, but the cascade was structurally necessary and the task's plan text incorrectly assumed `handle_press` already took `&mut`. Documented in T08's Completion Summary.

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|---------------------------|
| 01-wrap-mode-click-region-fix | `crates/fdemon-tui/src/widgets/log_view/mod.rs`, `crates/fdemon-tui/src/widgets/log_view/tests.rs` | `crates/fdemon-app/src/state.rs` (`LogViewState::offset`), `crates/fdemon-app/src/message.rs` (`Message::ClickLogRow`) |
| 02-inspector-handler-refactor | `crates/fdemon-app/src/handler/devtools/inspector.rs` | `crates/fdemon-app/src/state.rs` (`InspectorState`) |
| 03-sister-function-render-impl-refactor | `crates/fdemon-tui/src/widgets/devtools/mod.rs`, `crates/fdemon-tui/src/widgets/devtools/performance/mod.rs`, `crates/fdemon-tui/src/widgets/devtools/inspector/mod.rs`, plus the matching `tests.rs` for each | `crates/fdemon-tui/src/widgets/devtools/network/{mod.rs,request_table.rs,request_details.rs}` (reference for the desired `render_impl` pattern) |
| 04-render-tests-baselines-and-tightening | `crates/fdemon-tui/src/render/tests.rs` | `crates/fdemon-tui/src/widgets/devtools/{performance,network}/mod.rs` (compact-mode predicates) |
| 05-manual-smoke-test | `workflow/plans/features/mouse-support/phase-4.5-followup/tasks/05-manual-smoke-test.md` (Completion Summary section only) | n/a — runs the binary against a live Flutter project |
| 06-mouse-regions-doc-polish | `crates/fdemon-app/src/mouse_regions.rs` | n/a |
| 07-log-view-handler-and-state-polish | `crates/fdemon-app/src/handler/log_view.rs`, `crates/fdemon-app/src/handler/update.rs`, `crates/fdemon-app/src/state.rs` | `crates/fdemon-app/src/handler/session.rs` (session-switch arms reference) |
| 08-devtools-mouse-handler-polish | `crates/fdemon-app/src/handler/mouse/devtools.rs` | `crates/fdemon-tui/src/widgets/devtools/mod.rs` (sub-tab bar rect reference) |
| 09-tree-glyph-and-network-details-polish | `crates/fdemon-tui/src/widgets/devtools/inspector/tree_panel.rs`, `crates/fdemon-tui/src/widgets/devtools/network/request_details.rs` | n/a |
| 10-plan-drift-and-mouse-doc-updates | `workflow/plans/features/mouse-support/PLAN.md`, `docs/MOUSE.md` | `workflow/reviews/features/mouse-support-phase-4/REVIEW.md` (drift summary) |

### Overlap Matrix

Wave 1 (no internal dependencies): all 10 tasks.

| Task Pair | Wave | Shared Write Files | Isolation Strategy |
|-----------|------|--------------------|---------------------|
| 01 + 02 | 1 | None | **Parallel (worktree)** |
| 01 + 03 | 1 | None — T01 writes `log_view/`, T03 writes `devtools/` | **Parallel (worktree)** |
| 01 + 04 | 1 | None — T01 writes `log_view/tests.rs`, T04 writes `render/tests.rs` | **Parallel (worktree)** |
| 01 + 05 | 1 | None — T05 only writes its own task file | **Parallel (worktree)** |
| 01 + 06 | 1 | None | **Parallel (worktree)** |
| 01 + 07 | 1 | None | **Parallel (worktree)** |
| 01 + 08 | 1 | None | **Parallel (worktree)** |
| 01 + 09 | 1 | None — T01 writes `log_view/`, T09 writes `devtools/inspector/tree_panel.rs` and `devtools/network/request_details.rs` | **Parallel (worktree)** |
| 01 + 10 | 1 | None | **Parallel (worktree)** |
| 02 + 03 | 1 | None — T02 writes `fdemon-app`, T03 writes `fdemon-tui` | **Parallel (worktree)** |
| 02 + 04 | 1 | None | **Parallel (worktree)** |
| 02 + 05 | 1 | None | **Parallel (worktree)** |
| 02 + 06 | 1 | None — T02 writes `handler/devtools/inspector.rs`, T06 writes `mouse_regions.rs` | **Parallel (worktree)** |
| 02 + 07 | 1 | None — T02 writes `handler/devtools/inspector.rs`, T07 writes `handler/log_view.rs` + `handler/update.rs` + `state.rs` | **Parallel (worktree)** |
| 02 + 08 | 1 | None — T02 writes `handler/devtools/inspector.rs`, T08 writes `handler/mouse/devtools.rs` | **Parallel (worktree)** |
| 02 + 09 | 1 | None — same crate boundary as 01 + 09 reasoning | **Parallel (worktree)** |
| 02 + 10 | 1 | None | **Parallel (worktree)** |
| 03 + 04 | 1 | None — T03 writes per-widget `tests.rs` (devtools/, performance/, inspector/), T04 writes `render/tests.rs` | **Parallel (worktree)** |
| 03 + 05 | 1 | None | **Parallel (worktree)** |
| 03 + 06 | 1 | None | **Parallel (worktree)** |
| 03 + 07 | 1 | None | **Parallel (worktree)** |
| 03 + 08 | 1 | None | **Parallel (worktree)** |
| 03 + 09 | 1 | None — T03 writes `inspector/mod.rs` and `inspector/tests.rs`; T09 writes `inspector/tree_panel.rs`. Same dir, disjoint files. | **Parallel (worktree)** |
| 03 + 10 | 1 | None | **Parallel (worktree)** |
| 04 + 05 | 1 | None | **Parallel (worktree)** |
| 04 + 06 | 1 | None | **Parallel (worktree)** |
| 04 + 07 | 1 | None | **Parallel (worktree)** |
| 04 + 08 | 1 | None | **Parallel (worktree)** |
| 04 + 09 | 1 | None | **Parallel (worktree)** |
| 04 + 10 | 1 | None | **Parallel (worktree)** |
| 05 + 06 | 1 | None | **Parallel (worktree)** |
| 05 + 07 | 1 | None | **Parallel (worktree)** |
| 05 + 08 | 1 | None | **Parallel (worktree)** |
| 05 + 09 | 1 | None | **Parallel (worktree)** |
| 05 + 10 | 1 | None | **Parallel (worktree)** |
| 06 + 07 | 1 | None — T06 writes `mouse_regions.rs`, T07 writes `handler/log_view.rs` + `handler/update.rs` + `state.rs` | **Parallel (worktree)** |
| 06 + 08 | 1 | None | **Parallel (worktree)** |
| 06 + 09 | 1 | None | **Parallel (worktree)** |
| 06 + 10 | 1 | None | **Parallel (worktree)** |
| 07 + 08 | 1 | None — T07 writes `handler/log_view.rs` + `handler/update.rs` + `state.rs`, T08 writes `handler/mouse/devtools.rs` | **Parallel (worktree)** |
| 07 + 09 | 1 | None | **Parallel (worktree)** |
| 07 + 10 | 1 | None | **Parallel (worktree)** |
| 08 + 09 | 1 | None — T08 writes `handler/mouse/devtools.rs` (fdemon-app), T09 writes widgets in fdemon-tui | **Parallel (worktree)** |
| 08 + 10 | 1 | None | **Parallel (worktree)** |
| 09 + 10 | 1 | None | **Parallel (worktree)** |

All 45 task pairs have zero shared write files — Wave 1 is fully parallelizable across ten worktrees.

### Notes on Overlap Analysis

- **`handler/update.rs` overlap (T07 only)**: only Task 07 modifies `handler/update.rs` (to clear `last_log_click` on session-switch arms). No other task touches it. Task 02 writes a sibling file (`handler/devtools/inspector.rs`) that shares the parent module but disjoint file path.
- **`state.rs` overlap (T07 only)**: only Task 07 modifies `state.rs` (to optionally adjust `LogClickStamp` semantics if needed for cross-session stamp clearing).
- **Inspector subdirectory**: T03 writes `widgets/devtools/inspector/{mod.rs, tests.rs}`; T09 writes `widgets/devtools/inspector/tree_panel.rs`. Disjoint files, no merge conflict — even within the same logical module.
- **Test-file isolation**: `render/tests.rs` (T04) and `log_view/tests.rs` (T01) and per-panel `tests.rs` (T03) are three distinct files. The orchestrator may run them in parallel.

## Success Criteria

Phase 4.5 is complete when:

- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo check --workspace --all-targets` passes
- [ ] `cargo test --workspace` passes (no regressions; baseline grows by ~10 new tests across the 10 tasks)
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [ ] **CRITICAL FIX (T01):** `wrap_mode = true` + `state.offset` causing `wrap_intra_offset > 0` correctly aligns click regions with visible rows. Regression test in `widgets/log_view/tests.rs` exercises this path.
- [ ] **MAJOR (T02):** `maybe_fetch_layout(&mut InspectorState) -> Option<String>` helper extracted; both `handle_inspector_navigate` and `handle_inspector_select_row` call it. No `let _ = (old_index, new_index)` lint suppressions remain.
- [ ] **MAJOR (T03):** `widgets/devtools/{mod, performance/mod, inspector/mod}.rs` each share an internal `render_impl(area, buf, ctx: Option<&mut MouseCtx<'_>>)` with their `Widget::render`. Per-panel test asserts `Widget::render` and `render_with_regions(... None)` produce byte-identical buffers.
- [ ] **MAJOR (T04):** `render/tests.rs` includes 80×24 baseline tests for performance + network compact-mode (asserting no Phase-4 regions are pushed). The log-view region-count assertion uses exact equality (`==`), not `>=`.
- [ ] **MAJOR (T05):** Manual smoke test executed against a live Flutter session on macOS; results recorded in T05's Completion Summary.
- [ ] **HYGIENE (T06–T10):** all minor findings from `ACTION_ITEMS.md` items #8–#26 addressed.
- [ ] **Re-review:** dispatch the `reviewer` skill on `feat/mouse-support` head; verdict must be **APPROVED** (or APPROVED WITH CONCERNS for items explicitly tracked as out-of-scope follow-ups in `IDEAS.md`).

## Notes

- **Why Phase 4.5 instead of folding into Phase 5:** Phase 5 introduces a new layer concern (`z_index = 1` overlays for dialogs). Mixing the Phase-4 cleanup with new-layer work would conflate two distinct waves of risk. Following the precedent set by Phases 1.5, 2.5, and 3.5 keeps each phase boundary clean.

- **Why a single wave:** the file-overlap analysis shows zero conflicts across all 10 tasks. The orchestrator can dispatch all 10 implementors in parallel worktrees in a single dispatch call, with one validation pass per task. Total wall-clock time is bounded by the slowest task (~2h for T01 or T03), not the sum.

- **Why T05 is in Wave 1:** the manual smoke test does not need to run after the code fixes — running it on the current `feat/mouse-support` head exercises the as-shipped Phase 4 behavior, which is exactly what the review asked us to verify before merging to `main`. If the smoke test surfaces new issues, they get added as follow-up tasks rather than blocking the existing Wave 1.

- **Why some minors are not addressed:** ACTION_ITEMS.md item #10 (`MouseAction::Emit(Box<Message>)` allocation churn) is explicitly deferred — defer until measured pain. Item #13 (glyph-after-row push order test guard) is partially addressed by T06 (doc note on `MouseRegionsBuilder::click`); a stricter type-system enforcement would require a `RegionAwareWidget` redesign and is tracked for Phase 6 in `IDEAS.md`.

- **Re-review trigger:** after all 10 tasks merge, the user should run `/reviewer` again. The verdict should be APPROVED (or APPROVED WITH CONCERNS for items in IDEAS.md). If NEEDS WORK or REJECTED returns, a Phase 4.6 may be needed — but this is unlikely since the high-severity findings are all directly addressed by T01–T07.

- **`MouseAction::as_emit()` placement decision:** kept as-is (public method on `MouseAction` in `mouse_regions.rs`). T06 adds a `// Used by cross-crate tests; kept public for that reason.` doc comment. Cross-crate `#[cfg(test)]` would require Cargo feature-flag machinery for marginal benefit.

- **PLAN.md drift rationale:** the original PLAN.md sketched two design choices that the implementation diverged from for good reasons: (a) per-row `Emit` instead of `EmitWithCoord` (Phase 4 task 06 docs explain), and (b) double-click without spatial constraint (entry_id matching is more robust to scrolling). T10 records both deviations in PLAN.md so future readers don't get confused; PLAN.md is an internal planning doc, not a stable contract, so in-place updates are appropriate.
