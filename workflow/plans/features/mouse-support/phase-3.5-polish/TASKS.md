# Phase 3.5: Region Registry Polish & Phase-4 Prep — Task Index

## Overview

Phase 3 of mouse-support shipped a working per-frame region registry, clickable header shortcuts, session tabs with select/close, and the single-session device pill. The implementation review (`workflow/reviews/features/mouse-support-phase-3-region-registry/REVIEW.md`) raised 0 critical, 4 major, and 14 minor findings — none are correctness regressions, but several represent polish debt and pre-Phase-4 hardening that becomes more expensive to fix once Phase 4 multiplies the registry's surface area.

Phase 3.5 discharges all 18 ACTION_ITEMS in three thematic groups:

1. **Mechanical cleanup (Wave 1)** — stale TODO referencing already-shipped Task 02, dead `to_mouse_rect` helper, REVIEW_FOCUS.md exception entry missing, TASKS.md narrative drift on Settings-mode regions, Task 07 reconciliation audit trail. Includes a baseline ARCHITECTURE.md update covering the new `mouse_regions` module + `MouseCtx` threading.

2. **Polish (Wave 2)** — extract `SHORTCUT_SEGMENT_PREFIX` constant, replace bare `u16` arithmetic with `saturating_add`, drop redundant `padded_area.height.max(1)` guard, colocate Phase-5 update notes inline next to the assertions they affect.

3. **Phase-4 prep (Waves 3 & 4)** — lift `tag_filter_visible` early-return from `normal::handle_press` into the `mouse::handle_press` dispatcher so future per-mode handlers inherit it; add a `///` doc comment on `handle_scroll`; introduce a `MouseRegionGuard<'_>` RAII wrapper around `Cell::take`/`set` so a widget panic can no longer silently disable the registry; backfill three `SessionManager::remove_session` tests covering the new `selected_index`-decrement branch.

A second ARCHITECTURE.md update (Wave 5) documents the new `MouseRegionGuard` once Wave 4 lands.

**Total Tasks:** 11
**Estimated Hours:** ~4.5 hours

## Prerequisites

- Phase 3 must be merged on `feat/mouse-support`.
- No new external dependencies.
- All five reviewer agents' findings are aggregated in `workflow/reviews/features/mouse-support-phase-3-region-registry/ACTION_ITEMS.md`.

## Task Dependency Graph

```
Wave 1 (no internal deps — 4 tasks parallel):
┌────────────────────────┐ ┌────────────────────────┐ ┌────────────────────────┐ ┌────────────────────────┐
│ 01 - polish-mouse-     │ │ 02 - delete-to-mouse-  │ │ 03 - docs-and-plan-    │ │ 04 - architecture-     │
│      regions           │ │      rect-helper       │ │      reconciliation    │ │      doc-baseline      │
│ (mouse_regions.rs)     │ │ (widgets/mod.rs)       │ │ (REVIEW_FOCUS.md,      │ │ (ARCHITECTURE.md)      │
│                        │ │                        │ │  Phase-3 TASKS.md,     │ │ Agent: doc_maintainer  │
│                        │ │                        │ │  task-07.md)           │ │                        │
└────────────────────────┘ └────────────────────────┘ └────────────────────────┘ └────────────────────────┘

Wave 2 (3 tasks parallel — different widget files):
┌────────────────────────┐ ┌────────────────────────┐ ┌────────────────────────┐
│ 05 - header-constants- │ │ 06 - tabs-empty-rect-  │ │ 07 - render-tests-     │
│      and-overflow      │ │      cleanup           │ │      todo-ergonomics   │
│ (widgets/header.rs)    │ │ (widgets/tabs.rs)      │ │ (render/tests.rs)      │
└────────────────────────┘ └────────────────────────┘ └────────────────────────┘

Wave 3 (single task — bundles two small edits in mouse handler):
┌────────────────────────┐
│ 08 - mouse-handler-    │
│      hygiene           │
│ (handler/mouse/mod.rs +│
│  handler/mouse/        │
│  normal.rs)            │
└────────────────────────┘

Wave 4 (2 tasks parallel — Phase-4 hardening, disjoint files):
┌────────────────────────┐ ┌────────────────────────┐
│ 09 - mouse-region-     │ │ 10 - remove-session-   │
│      guard-raii        │ │      tests             │
│ (mouse_regions.rs +    │ │ (session_manager.rs)   │
│  render/mod.rs +       │ │                        │
│  handler/mouse/        │ │                        │
│  normal.rs)            │ │                        │
└────────────────────────┘ └────────────────────────┘

Wave 5 (single — doc_maintainer follow-up after Wave 4):
┌────────────────────────┐
│ 11 - architecture-     │
│      doc-raii-update   │
│ (ARCHITECTURE.md)      │
│ Agent: doc_maintainer  │
└────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Crate / Area |
|---|------|--------|------------|------------|--------------|
| 1 | [01-polish-mouse-regions](tasks/01-polish-mouse-regions.md) | Not Started | — | 0.5h | `fdemon-app` |
| 2 | [02-delete-to-mouse-rect-helper](tasks/02-delete-to-mouse-rect-helper.md) | Not Started | — | 0.1h | `fdemon-tui` |
| 3 | [03-docs-and-plan-reconciliation](tasks/03-docs-and-plan-reconciliation.md) | Not Started | — | 0.25h | docs / plan |
| 4 | [04-architecture-doc-baseline](tasks/04-architecture-doc-baseline.md) | Not Started | — | 0.5h | docs (`doc_maintainer`) |
| 5 | [05-header-constants-and-overflow](tasks/05-header-constants-and-overflow.md) | Not Started | — | 0.25h | `fdemon-tui` |
| 6 | [06-tabs-empty-rect-cleanup](tasks/06-tabs-empty-rect-cleanup.md) | Not Started | — | 0.1h | `fdemon-tui` |
| 7 | [07-render-tests-todo-ergonomics](tasks/07-render-tests-todo-ergonomics.md) | Not Started | — | 0.1h | `fdemon-tui` |
| 8 | [08-mouse-handler-hygiene](tasks/08-mouse-handler-hygiene.md) | Not Started | — | 0.5h | `fdemon-app` |
| 9 | [09-mouse-region-guard-raii](tasks/09-mouse-region-guard-raii.md) | Not Started | 1, 8 | 1.5h | `fdemon-app`, `fdemon-tui` |
| 10 | [10-remove-session-tests](tasks/10-remove-session-tests.md) | Not Started | — | 0.5h | `fdemon-app` |
| 11 | [11-architecture-doc-raii-update](tasks/11-architecture-doc-raii-update.md) | Not Started | 9 | 0.25h | docs (`doc_maintainer`) |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|---------------------------|
| 01-polish-mouse-regions | `crates/fdemon-app/src/mouse_regions.rs` | `crates/fdemon-app/src/message.rs` (ensure `Message::CloseSessionAt` exists) |
| 02-delete-to-mouse-rect-helper | `crates/fdemon-tui/src/widgets/mod.rs` | — |
| 03-docs-and-plan-reconciliation | `docs/REVIEW_FOCUS.md`, `workflow/plans/features/mouse-support/phase-3-region-registry/TASKS.md`, `workflow/plans/features/mouse-support/phase-3-region-registry/tasks/07-tabs-and-device-pill-regions.md` | `crates/fdemon-app/src/handler/mouse/mod.rs` (cite dispatcher gate location); `crates/fdemon-tui/src/render/tests.rs` (cite probe test name) |
| 04-architecture-doc-baseline | `docs/ARCHITECTURE.md` | `crates/fdemon-app/src/mouse_regions.rs`, `crates/fdemon-app/src/state.rs`, `crates/fdemon-tui/src/render/mod.rs`, `crates/fdemon-tui/src/widgets/mod.rs` (read for module/type descriptions) |
| 05-header-constants-and-overflow | `crates/fdemon-tui/src/widgets/header.rs` | — |
| 06-tabs-empty-rect-cleanup | `crates/fdemon-tui/src/widgets/tabs.rs` | `crates/fdemon-app/src/mouse_regions.rs` (verify `click_left_middle` empty-rect guard) |
| 07-render-tests-todo-ergonomics | `crates/fdemon-tui/src/render/tests.rs` | — |
| 08-mouse-handler-hygiene | `crates/fdemon-app/src/handler/mouse/mod.rs`, `crates/fdemon-app/src/handler/mouse/normal.rs` | `crates/fdemon-app/src/handler/keys.rs` (cite parallel `tag_filter_visible` intercept) |
| 09-mouse-region-guard-raii | `crates/fdemon-app/src/mouse_regions.rs`, `crates/fdemon-tui/src/render/mod.rs`, `crates/fdemon-app/src/handler/mouse/normal.rs` | `crates/fdemon-app/src/state.rs` (`MouseRegionsCell` API) |
| 10-remove-session-tests | `crates/fdemon-app/src/session_manager.rs` | — |
| 11-architecture-doc-raii-update | `docs/ARCHITECTURE.md` | `crates/fdemon-app/src/mouse_regions.rs` (read post-Wave-4 state for `MouseRegionGuard` description) |

### Overlap Matrix

| Task Pair | Wave | Shared Write Files | Isolation Strategy |
|-----------|------|--------------------|---------------------|
| 01 + 02 + 03 + 04 | Wave 1 | None — four disjoint write surfaces | **Parallel (worktree)** |
| 05 + 06 + 07 | Wave 2 | None — three disjoint widget files | **Parallel (worktree)** |
| 08 alone | Wave 3 | n/a — single task | **Single task on current branch** |
| 09 + 10 | Wave 4 | None — `mouse_regions.rs`/`render/mod.rs`/`normal.rs` (T09) vs `session_manager.rs` (T10) | **Parallel (worktree)** |
| 11 alone | Wave 5 | n/a — single task | **Single task on current branch** |

Notes on overlap analysis:

- **`mouse_regions.rs` write overlap (01 ↔ 09)** is dependency-ordered: Task 01 lands in Wave 1, Task 09 in Wave 4. Sequential by wave structure, no conflict.
- **`handler/mouse/normal.rs` overlap (08 ↔ 09)** is dependency-ordered: Task 08 in Wave 3 lifts the `tag_filter_visible` check; Task 09 in Wave 4 replaces the manual take/set with the RAII guard. Task 09 explicitly depends on Task 08 (`Depends On: 1, 8`).
- **`docs/ARCHITECTURE.md` overlap (04 ↔ 11)** is dependency-ordered: Task 04 in Wave 1 documents the Phase-3 baseline; Task 11 in Wave 5 augments with `MouseRegionGuard`. Sequential by wave structure.
- **`docs/ARCHITECTURE.md`-only tasks (04, 11)** are routed to `doc_maintainer` per the planner-skill rules for managed docs.
- **`docs/REVIEW_FOCUS.md` (Task 03)** is unmanaged per the planner-skill rules — implementor edits it directly.

## Success Criteria

Phase 3.5 is complete when:

- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo check --workspace --all-targets` passes
- [ ] `cargo test --workspace` passes (no regressions; existing 5,131 unit-test count grows by ≥3 from Task 10)
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes **without** `#[allow(dead_code)]` on `to_mouse_rect` (the helper is gone after Task 02)
- [ ] All four "Required for Approval" review items from `ACTION_ITEMS.md` are discharged (TODO removed, dead helper deleted, REVIEW_FOCUS.md updated, Phase 3 TASKS.md narrative reconciled)
- [ ] `MouseRegionGuard<'_>` exists in `mouse_regions.rs` and replaces both manual `Cell::take`/`set` pairs (`render::view`, `normal::handle_press`)
- [ ] `mouse::handle_press` (the dispatcher) returns `None` early when `state.tag_filter_visible == true`, regardless of `UiMode`
- [ ] `SessionManager::remove_session` test coverage extended to cover (a) remove-pre-selected (id-preserving), (b) `evict_oldest_stopped` interaction, (c) failed-spawn removal preserving user selection
- [ ] `docs/ARCHITECTURE.md` describes the `mouse_regions` module, the `MouseCtx` threading pattern, and `MouseRegionGuard`
- [ ] `docs/REVIEW_FOCUS.md` "Approved TEA Exception → Current usage" lists `AppState::mouse_regions: MouseRegionsCell`
- [ ] Phase 3 TASKS.md note (line 172) is reconciled — Settings-mode gating is correctly attributed to the dispatcher, not registration
- [ ] Task 07's completion summary in Phase 3 includes a reconciliation audit trail (kept vs discarded)

## Notes

- **Why split the ARCHITECTURE.md update across Waves 1 and 5.** The user requested docs land in lockstep with code. Wave 1 documents what already exists on `feat/mouse-support` post-Phase-3 (`mouse_regions` module, `MouseCtx`, `MouseRegionsCell` exception); Wave 5 adds the `MouseRegionGuard` once Wave 4 has implemented it. This keeps `doc_maintainer` writing about reality at all times, never about pending work.

- **Why Task 08 bundles two edits.** The `tag_filter_visible` lift (item 10) and the `handle_scroll` doc-comment addition (item 9) both touch `handler/mouse/normal.rs`. Splitting them creates a same-file sequential pair with no parallelism win. Bundling keeps Wave 3 to a single task with under-30-minute estimated effort.

- **Why Task 09 (RAII guard) depends on Task 08.** Task 08 changes `normal::handle_press`'s control flow (lifting `tag_filter_visible` to the dispatcher leaves `handle_press` shorter). Task 09 then replaces the now-cleaner take/set pair with `MouseRegionGuard`. Doing them in the other order means Task 09 fights with Task 08's later refactor.

- **Why the `MouseRegionGuard<'_>` over a closure-based `with_regions(|builder| ...)` API.** The borrowing guard with `Deref{Mut}` keeps call-site syntax identical to the current `take`/`set` pattern (a single mutable binding, no closure indentation), supports early-return via `?` cleanly, and gives `Drop` panic-safety for free. A closure API would require all call sites to nest one level deeper and would fight Rust's `?`-via-closure ergonomics. The guard is ~15 lines of code and adds no allocations.

- **Manual smoke test deferred.** The Phase 3 smoke-test bullets (start a Flutter session → click `[r]`, etc.) remain valid for Phase 3.5 — none of these tasks change observable user behavior except for the `MouseRegionGuard` (which is mechanical refactoring) and the dispatcher gate lift (which makes per-mode handlers receive cleaner `state` — no behavioral change since `tag_filter_visible` already short-circuited inside `normal::handle_press`).

- **Task 10's three new tests are independent of any other Phase 3.5 task** — they exercise existing `SessionManager::remove_session` semantics that landed in Phase 3 Task 02. The reason this lives in Phase 3.5 is that Task 02's review noted only one test (`test_remove_session`) exercised the new branch, and call sites in `evict_oldest_stopped`/`handle_session_spawn_failed` are not specifically validated.
