# Phase 2.5: Scroll-Wheel Follow-up — Task Index

## Overview

Phase 2 of mouse-support shipped working per-`UiMode` scroll routing. The implementation review (`workflow/reviews/features/mouse-support-phase-2-scroll-wheel/REVIEW.md`) surfaced one real logic asymmetry (Inspector accepts `Shift+Ctrl/Alt` combos that every other mode rejects), one cross-handler UX inconsistency that needs documentation (Settings vs NewSession dart-defines Edit-pane behavior), one missing user-facing doc (`docs/MOUSE.md` is load-bearing for Win11 Shift caveat / modifier asymmetry / coordinate-free routing), plus a handful of small code-quality polish items. Phase 2.5 closes all of these so Phase 3 (region registry + clickable hit-testing) starts from a clean baseline.

The dart-defines mouse-vs-keyboard reconciliation is intentionally deferred: this phase only adds a cross-reference comment documenting the existing divergence. Changing the keyboard handler at `keys.rs:851-855` is a real product decision and lives in a separate bug task if pursued.

**Total Tasks:** 6
**Estimated Hours:** ~2.5 hours

## Task Dependency Graph

```
                ┌──────────────────────────────────────────────────────┐
                │                  No dependencies                     │
                │  (all 6 tasks run in parallel — single wave)         │
                └──────────────────────────────────────────────────────┘

       ┌─────────────────┬─────────────────┬─────────────────┬─────────────────┬─────────────────┐
       ▼                 ▼                 ▼                 ▼                 ▼                 ▼
┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐
│ 01-fix-      │ │ 02-document- │ │ 03-stub-     │ │ 04-mod-rs-   │ │ 05-comment-  │ │ 06-strengthen│
│ inspector-   │ │ dart-defines-│ │ mouse-docs   │ │ doc-and-     │ │ ignored-mods │ │ mouse-tests  │
│ modifier-    │ │ divergence   │ │ (docs/       │ │ coverage     │ │ (flutter_    │ │ (handler/    │
│ rule         │ │ (settings.rs)│ │  MOUSE.md +  │ │ (mouse/      │ │  version.rs +│ │  tests.rs)   │
│ (devtools.rs)│ │              │ │ CONFIGURATION│ │  mod.rs)     │ │  new_session │ │              │
│              │ │              │ │ .md)         │ │              │ │  .rs)        │ │              │
└──────────────┘ └──────────────┘ └──────────────┘ └──────────────┘ └──────────────┘ └──────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Crate / Area |
|---|------|--------|------------|------------|--------------|
| 1 | [01-fix-inspector-modifier-rule](tasks/01-fix-inspector-modifier-rule.md) | Not Started | — | 0.5h | `fdemon-app` |
| 2 | [02-document-dart-defines-divergence](tasks/02-document-dart-defines-divergence.md) | Not Started | — | 0.25h | `fdemon-app` |
| 3 | [03-stub-mouse-docs](tasks/03-stub-mouse-docs.md) | Not Started | — | 0.75h | docs |
| 4 | [04-mod-rs-doc-and-coverage](tasks/04-mod-rs-doc-and-coverage.md) | Not Started | — | 0.5h | `fdemon-app` |
| 5 | [05-comment-ignored-mods](tasks/05-comment-ignored-mods.md) | Not Started | — | 0.25h | `fdemon-app` |
| 6 | [06-strengthen-mouse-tests](tasks/06-strengthen-mouse-tests.md) | Not Started | — | 0.5h | `fdemon-app` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|---------------------------|
| 01-fix-inspector-modifier-rule | `crates/fdemon-app/src/handler/mouse/devtools.rs` | `crates/fdemon-app/src/handler/keys.rs` (reference for parity), `crates/fdemon-app/src/handler/mouse/normal.rs` (reference for `is_shift_only` pattern) |
| 02-document-dart-defines-divergence | `crates/fdemon-app/src/handler/mouse/settings.rs` | `crates/fdemon-app/src/handler/mouse/new_session.rs` (rationale source), `crates/fdemon-app/src/handler/keys.rs` (`keys.rs:733-770` and `:851-855` reference) |
| 03-stub-mouse-docs | `docs/MOUSE.md` (NEW), `docs/CONFIGURATION.md` (add link to MOUSE.md) | `crates/fdemon-app/src/handler/mouse/*.rs` (per-mode behavior), `workflow/plans/features/mouse-support/PLAN.md` (Edge Cases section), `workflow/reviews/features/mouse-support-phase-2-scroll-wheel/REVIEW.md` (risks summary) |
| 04-mod-rs-doc-and-coverage | `crates/fdemon-app/src/handler/mouse/mod.rs` | `crates/fdemon-app/src/handler/mouse/settings.rs`, `crates/fdemon-app/src/handler/mouse/new_session.rs` (positive-assertion targets) |
| 05-comment-ignored-mods | `crates/fdemon-app/src/handler/mouse/flutter_version.rs`, `crates/fdemon-app/src/handler/mouse/new_session.rs` | — |
| 06-strengthen-mouse-tests | `crates/fdemon-app/src/handler/tests.rs` | `crates/fdemon-app/src/handler/mouse/mod.rs` (no-op contract), `crates/fdemon-app/src/handler/scroll.rs` (`is_busy` semantics) |

### Overlap Matrix

Wave 1 (no dependencies): 01, 02, 03, 04, 05, 06

| Task Pair | Wave | Shared Write Files | Isolation Strategy |
|-----------|------|--------------------|---------------------|
| 01 + 02 | Wave 1 | None | **Parallel (worktree)** |
| 01 + 03 | Wave 1 | None | **Parallel (worktree)** |
| 01 + 04 | Wave 1 | None | **Parallel (worktree)** |
| 01 + 05 | Wave 1 | None | **Parallel (worktree)** |
| 01 + 06 | Wave 1 | None | **Parallel (worktree)** |
| 02 + 03 | Wave 1 | None | **Parallel (worktree)** |
| 02 + 04 | Wave 1 | None | **Parallel (worktree)** |
| 02 + 05 | Wave 1 | None | **Parallel (worktree)** |
| 02 + 06 | Wave 1 | None | **Parallel (worktree)** |
| 03 + 04 | Wave 1 | None | **Parallel (worktree)** |
| 03 + 05 | Wave 1 | None | **Parallel (worktree)** |
| 03 + 06 | Wave 1 | None | **Parallel (worktree)** |
| 04 + 05 | Wave 1 | None | **Parallel (worktree)** |
| 04 + 06 | Wave 1 | None | **Parallel (worktree)** |
| 05 + 06 | Wave 1 | None | **Parallel (worktree)** |

All 15 task pairs have zero shared write files — Wave 1 is fully parallelizable across six worktrees.

**Lesson applied from Phase 2:** every Phase 2 wave-2 task incidentally edited `mouse/mod.rs` to update a shared no-op test array, producing 4 merge conflicts. Phase 2.5 isolates `mod.rs` writes to a single task (Task 04) and explicitly instructs other tasks to leave `mod.rs` untouched — see each task's "Notes" section.

## Success Criteria

Phase 2.5 is complete when:

- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo check --workspace --all-targets` passes
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [ ] `crates/fdemon-app/src/handler/mouse/devtools.rs::handle_inspector_scroll` rejects `Shift+Ctrl+wheel` and `Shift+Alt+wheel` with `None`, matching `normal.rs` / `link_highlight.rs` / `handle_network_scroll`. A test asserts the new behavior.
- [ ] `crates/fdemon-app/src/handler/mouse/devtools.rs` module doc no longer claims "Inspector → tree row navigation (Up/Down only; no page step)" without acknowledging that Shift falls through to single-step (or, since the new rule rejects all modifier combos for Inspector, the doc accurately reflects that).
- [ ] `crates/fdemon-app/src/handler/mouse/settings.rs:21` (the `DartDefinesPane::Edit => None` arm) carries an inline comment explaining that the divergence from `new_session.rs` is intentional and pointing to the rationale.
- [ ] `docs/MOUSE.md` exists, documents per-mode modifier behavior, the coordinate-free routing decision, and the Win11 Shift-mod drop caveat.
- [ ] `docs/CONFIGURATION.md`'s `enable_mouse` row links to `docs/MOUSE.md`.
- [ ] `crates/fdemon-app/src/handler/mouse/mod.rs::handle_scroll` has a `///` doc comment describing per-mode dispatch.
- [ ] `crates/fdemon-app/src/handler/mouse/mod.rs` test module includes positive-assertion tests for `UiMode::Settings` and `UiMode::NewSessionDialog` scroll routing.
- [ ] `crates/fdemon-app/src/handler/mouse/mod.rs` `test_scroll_no_op_in_non_scrollable_modes` array includes `UiMode::EmulatorSelector`.
- [ ] `crates/fdemon-app/src/handler/mouse/flutter_version.rs` and `new_session.rs` carry inline comments explaining why `_mods` is unused.
- [ ] `crates/fdemon-app/src/handler/tests.rs::mouse_scroll::assert_scroll_routes_to` carries a doc comment warning future callers that it compares discriminants only (data-carrying variants must use `matches!` directly).
- [ ] `crates/fdemon-app/src/handler/tests.rs` contains a `scroll_during_reload_does_not_block_or_corrupt` (or similarly named) test that drives `update(state_with_busy_session, Message::Mouse(Scroll{..}))` and asserts the scroll message still fires.

Out of scope (handled separately or deferred):

- **Reconciling NewSession dart-defines Edit-pane scroll to Settings' policy.** The current asymmetry is keyboard-driven and changing it requires updating `keys.rs:851-855`, which is a product decision, not a polish fix. If desired, file as a separate bug task: `workflow/plans/bugs/dart-defines-edit-scroll-asymmetry/`.
- **Renaming non-conforming tests** in `normal.rs` / `link_highlight.rs` to follow `test_<function>_<scenario>_<expected_result>`. Style-only churn; the project test suite has mixed conventions already.
- **Extracting a shared `log_scroll_message` helper** between `normal.rs` and `link_highlight.rs`. Premature refactor for 12 lines.
- **Hoisting `test_device()` helper** between `devtools.rs` and `handler/tests.rs`. Over-engineering for a 10-line dup.
- **Strengthening Network filter-inactive integration test** to attach a session. Already covered at unit level by `network_filter_active_swallows_scroll` in `devtools.rs`.
- **Process feedback for orchestrator/planner workflow** (shared test array surface). Tracked in `ACTION_ITEMS.md` as planning improvement; not a code task.

## Notes

- **No new external dependencies.** All work is in existing source files plus one new doc.
- **No `Message` variants added.** Phase 2.5 changes routing logic and tests only.
- **No new `pub` items.** All changes are doc/comment polish, internal logic, internal tests, or net-new docs.
- **`docs/MOUSE.md` is the new owner of the user-facing mouse story.** Phase 6 of the parent feature plan can expand it; Phase 2.5 ships the stub now so users on `main` between Phase 2 and Phase 6 have a reference for the modifier asymmetry, Win11 Shift caveat, and coordinate-free routing.
- **Worktree strategy:** all six tasks dispatch in parallel as worktrees. `mod.rs` is exclusively owned by Task 04; other tasks must NOT update `mod.rs` even if they think a sibling test needs adjusting (the lesson from Phase 2's 4 merge conflicts).
