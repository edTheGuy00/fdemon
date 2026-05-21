# Phase 1.5 — Inspector Parity Fixes — Task Index

## Overview

Bundled remediation phase resolving the 4 critical correctness bugs, 8 major cleanups, and selected minor items found in the Phase 1 code review (see `workflow/reviews/features/devtools-inspector-parity-phase-1/`). 10 tasks across 6 waves. Mixed parallel + sequential by file overlap.

**Total Tasks:** 10
**Estimated Hours:** 10–14 hours

## Task Dependency Graph

```
Wave 1 (parallel — no overlap)
 ┌──────────────────────┐  ┌──────────────────────┐  ┌──────────────────────┐  ┌──────────────────────┐
 │ 01-inspector-state-  │  │ 02-persist-settings- │  │ 03-strip-dead-allows-│  │ 04-tree-rendering-   │
 │   helpers            │  │   action             │  │   and-cosmetics      │  │   correctness        │
 │ (state.rs)           │  │ (handler/mod.rs +    │  │ (details/*.rs)       │  │ (widget_tree.rs +    │
 │                      │  │  actions/mod.rs)     │  │                      │  │  tree_panel.rs +     │
 │                      │  │                      │  │                      │  │  tests.rs + lib.rs)  │
 └──────────┬───────────┘  └──────────┬───────────┘  └──────────────────────┘  └──────────┬───────────┘
            │                         │                                                    │
            │                         │                                                    │
Wave 2      │                         │                                                    ▼
            │                         │                                            ┌──────────────────────┐
            │                         │                                            │ 05-sanitize-vm-      │
            │                         │                                            │   service-strings    │
            │                         │                                            │ (widget_tree.rs)     │
            │                         │                                            └──────────┬───────────┘
            ▼                         │                                                       │
 ┌──────────────────────┐             │                                                       │
 │ 06-wire-expanded-    │             │                                                       │
 │   groups-and-cleanup │             │                                                       │
 │ (handler/devtools/   │             │                                                       │
 │  inspector.rs)       │             │                                                       │
 └──────────┬───────────┘             │                                                       │
            ▼                         │                                                       │
Wave 3                                │                                                       │
 ┌──────────────────────┐             │                                                       │
 │ 07-reset-state-on-   │             │                                                       │
 │   refresh-restart    │             │                                                       │
 │ (inspector.rs +      │             │                                                       │
 │  update.rs)          │             │                                                       │
 └──────────┬───────────┘             │                                                       │
            ▼                         ▼                                                       │
Wave 4                ┌────────────────────────────────────────┐                              │
                      │ 08-async-settings-persistence          │                              │
                      │ (inspector.rs + settings_handlers.rs + │                              │
                      │  config/settings.rs)                   │                              │
                      └────────────────┬───────────────────────┘                              │
                                       │                                                      │
Wave 5                                 │                                                      ▼
                                       │      ┌────────────────────────────────────────────────────┐
                                       │      │ 09-remove-visible-and-rename-misc                  │
                                       │      │ (tree_panel.rs + inspector/mod.rs + tests.rs +     │
                                       │      │  message.rs + handler/keys.rs + handler/update.rs +│
                                       │      │  details/mod.rs)                                   │
                                       │      └────────────────┬───────────────────────────────────┘
                                       │                       │
Wave 6 (doc_maintainer)                ▼                       ▼
                                       ┌────────────────────────────────┐
                                       │ 10-docs-update                 │
                                       │ (Agent: doc_maintainer)        │
                                       │ docs/ARCHITECTURE.md +         │
                                       │ docs/KEYBINDINGS.md            │
                                       └────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 01 | [01-inspector-state-helpers](tasks/01-inspector-state-helpers.md) | Done ✅ | — | 0.5–1h | `crates/fdemon-app/src/state.rs` |
| 02 | [02-persist-settings-action](tasks/02-persist-settings-action.md) | Done ✅ | — | 1–1.5h | `crates/fdemon-app/src/handler/mod.rs`, `crates/fdemon-app/src/actions/mod.rs`, `crates/fdemon-app/src/message.rs` |
| 03 | [03-strip-dead-allows-and-cosmetics](tasks/03-strip-dead-allows-and-cosmetics.md) | Done ✅ | — | 0.5h | `crates/fdemon-tui/src/widgets/devtools/inspector/details/{flex_explorer_tab,render_object_tab,properties_tab,mod}.rs` |
| 04 | [04-tree-rendering-correctness](tasks/04-tree-rendering-correctness.md) | Done ⚠️ CONCERN | — | 2–3h | `crates/fdemon-core/src/widget_tree.rs`, `crates/fdemon-core/src/lib.rs`, `crates/fdemon-tui/src/widgets/devtools/inspector/tree_panel.rs`, `crates/fdemon-tui/src/widgets/devtools/inspector/tests.rs` |
| 05 | [05-sanitize-vm-service-strings](tasks/05-sanitize-vm-service-strings.md) | Done ✅ | 04 | 1h | `crates/fdemon-core/src/widget_tree.rs` |
| 06 | [06-wire-expanded-groups-and-cleanup](tasks/06-wire-expanded-groups-and-cleanup.md) | Done ✅ | 01, 04 | 2h | `crates/fdemon-app/src/handler/devtools/inspector.rs` |
| 07 | [07-reset-state-on-refresh-restart](tasks/07-reset-state-on-refresh-restart.md) | Done ✅ | 06 | 1h | `crates/fdemon-app/src/handler/devtools/inspector.rs`, `crates/fdemon-app/src/handler/update.rs` |
| 08 | [08-async-settings-persistence](tasks/08-async-settings-persistence.md) | Done ✅ | 02, 07 | 1–1.5h | `crates/fdemon-app/src/handler/devtools/inspector.rs`, `crates/fdemon-app/src/handler/settings_handlers.rs`, `crates/fdemon-app/src/config/settings.rs` |
| 09 | [09-remove-visible-and-rename-misc](tasks/09-remove-visible-and-rename-misc.md) | Done ✅ | 04, 06 | 1.5–2h | `crates/fdemon-tui/src/widgets/devtools/inspector/tree_panel.rs`, `crates/fdemon-tui/src/widgets/devtools/inspector/mod.rs`, `crates/fdemon-tui/src/widgets/devtools/inspector/tests.rs`, `crates/fdemon-app/src/message.rs`, `crates/fdemon-app/src/handler/keys.rs`, `crates/fdemon-app/src/handler/update.rs`, `crates/fdemon-tui/src/widgets/devtools/inspector/details/mod.rs` |
| 10 | [10-docs-update](tasks/10-docs-update.md) | Done ✅ | 01–09 | 0.5–1h | `docs/ARCHITECTURE.md`, `docs/KEYBINDINGS.md` |

## Wave Schedule

| Wave | Tasks | Notes |
|------|-------|-------|
| W1 | 01, 02, 03, 04 | 4 parallel; no write-file overlap. |
| W2 | 05, 06 | 2 parallel; 05 depends on 04 (same file widget_tree.rs ran first); 06 depends on 01. |
| W3 | 07 | Sequential — same file as 06 (`handler/devtools/inspector.rs`). |
| W4 | 08 | Sequential — same file as 07 (`handler/devtools/inspector.rs`). Also depends on 02. |
| W5 | 09 | Sequential w.r.t. 04 (same files `tree_panel.rs` + `tests.rs`) and 06 (consumes the new `selected_row()` ergonomics). |
| W6 | 10 | Documentation update via `doc_maintainer`. |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|--------------------------|
| 01-inspector-state-helpers | `crates/fdemon-app/src/state.rs` | `crates/fdemon-core/src/widget_tree.rs` (InspectorRow / RowGroup types) |
| 02-persist-settings-action | `crates/fdemon-app/src/handler/mod.rs`, `crates/fdemon-app/src/actions/mod.rs`, `crates/fdemon-app/src/message.rs` | `crates/fdemon-app/src/config/settings.rs` (save_settings signature), existing `UpdateAction::AutoSaveConfig` pattern |
| 03-strip-dead-allows-and-cosmetics | `crates/fdemon-tui/src/widgets/devtools/inspector/details/flex_explorer_tab.rs`, `crates/fdemon-tui/src/widgets/devtools/inspector/details/render_object_tab.rs`, `crates/fdemon-tui/src/widgets/devtools/inspector/details/properties_tab.rs`, `crates/fdemon-tui/src/widgets/devtools/inspector/details/mod.rs` | — |
| 04-tree-rendering-correctness | `crates/fdemon-core/src/widget_tree.rs`, `crates/fdemon-core/src/lib.rs`, `crates/fdemon-tui/src/widgets/devtools/inspector/tree_panel.rs`, `crates/fdemon-tui/src/widgets/devtools/inspector/tests.rs` | — |
| 05-sanitize-vm-service-strings | `crates/fdemon-core/src/widget_tree.rs` | `crates/fdemon-core/src/ansi.rs` (`strip_ansi_codes` API), `crates/fdemon-daemon/src/protocol.rs:380` (existing usage pattern) |
| 06-wire-expanded-groups-and-cleanup | `crates/fdemon-app/src/handler/devtools/inspector.rs` | `crates/fdemon-app/src/state.rs` (`selected_row()` from task 01), `crates/fdemon-core/src/widget_tree.rs` (`RowGroup` variants from task 04) |
| 07-reset-state-on-refresh-restart | `crates/fdemon-app/src/handler/devtools/inspector.rs`, `crates/fdemon-app/src/handler/update.rs` | `crates/fdemon-app/src/state.rs` (`InspectorState` field list) |
| 08-async-settings-persistence | `crates/fdemon-app/src/handler/devtools/inspector.rs`, `crates/fdemon-app/src/handler/settings_handlers.rs`, `crates/fdemon-app/src/config/settings.rs` | `crates/fdemon-app/src/handler/mod.rs` (`UpdateAction::PersistSettings` from task 02), `crates/fdemon-app/src/actions/mod.rs` |
| 09-remove-visible-and-rename-misc | `crates/fdemon-tui/src/widgets/devtools/inspector/tree_panel.rs`, `crates/fdemon-tui/src/widgets/devtools/inspector/mod.rs`, `crates/fdemon-tui/src/widgets/devtools/inspector/tests.rs`, `crates/fdemon-app/src/message.rs`, `crates/fdemon-app/src/handler/keys.rs`, `crates/fdemon-app/src/handler/update.rs`, `crates/fdemon-tui/src/widgets/devtools/inspector/details/mod.rs` | — |
| 10-docs-update | `docs/ARCHITECTURE.md`, `docs/KEYBINDINGS.md` | All implementation task files, `~/.claude/skills/doc-standards/schemas.md` |

### Overlap Matrix

Pairs evaluated only between tasks scheduled in the same wave (i.e. no dependency between them).

| Wave | Task Pair | Shared Write Files | Isolation Strategy |
|------|-----------|--------------------|--------------------|
| W1 | 01 + 02 | None | Parallel (worktree) |
| W1 | 01 + 03 | None | Parallel (worktree) |
| W1 | 01 + 04 | None | Parallel (worktree) |
| W1 | 02 + 03 | None | Parallel (worktree) |
| W1 | 02 + 04 | `crates/fdemon-app/src/message.rs` ⚠️ (02 adds a Message variant; 04 does NOT touch message.rs) | **None — 04 does not write message.rs.** Re-checked: 04 writes only widget_tree.rs + lib.rs + tree_panel.rs + tests.rs. Parallel (worktree). |
| W1 | 03 + 04 | None | Parallel (worktree) |
| W2 | 05 + 06 | None | Parallel (worktree) |

No within-wave write-file collisions. All wave-peer tasks may be dispatched concurrently in isolated worktrees.

Sequential pairs (cross-wave, by file overlap):
- 04 → 05: same file `widget_tree.rs`
- 04 → 09: same files `tree_panel.rs` + `tests.rs`
- 06 → 07 → 08: same file `handler/devtools/inspector.rs`
- 02 → 08: 02 defines the `UpdateAction::PersistSettings` variant that 08 consumes
- 06 → 09: 06 introduces the new `selected_row()` consumer pattern; 09's per-frame consolidation builds on it

## Cross-Cutting Constraints

1. **No new `Cell<usize>` render-hint fields.** The review noted M5 (per-frame `inspector_rows()` duplication) is a performance concern, not a layout-feedback concern. The fix in task 09 is to build the row list once at the top of `render_impl` and thread it down — not to add new `Cell`-wrapped state. See `docs/REVIEW_FOCUS.md` "Approved TEA Exception: Render-Hint Feedback" for the exception scope.

2. **Variant rename `ExitDevToolsMode → DevToolsEscape` is mechanical.** Task 09 owns the rename across all call sites. After the rename, `cargo check` must be green before moving on within the same task — no partial rename.

3. **`InspectorState::reset_details_and_groups()` helper.** Task 07 introduces a single helper method that clears `details_open`, `details_node_id`, `details_tab` (back to `DetailsTab::Properties`), `expanded_groups`, `properties`, `properties_loading`, `properties_error`. Both `handle_widget_tree_fetched` and `SessionRestartCompleted` call it. Defining it on `InspectorState` (in state.rs) is acceptable for task 07 even though state.rs is not in its primary write list — the alternative (duplicating the field-clear code in both call sites) is worse. Task 07 may write to state.rs IF it adds the helper method there; declare it explicitly in the task's "Files Modified (Write)" before starting.

4. **`Right` / `Enter` semantics on `LeaderCollapsed`.** Task 06 must preserve the existing behaviour that pressing `Enter` on a non-leader row opens Details. The leader-expand branch fires on `RowGroup::LeaderCollapsed` only; non-leader rows fall through to the existing handler.

5. **`hide_implementation_widgets` persistence path.** After task 08, the toggle handler in `handle_toggle_hide_implementation` no longer calls `save_settings` synchronously — it returns `UpdateAction::PersistSettings { settings: state.settings.clone(), project_path: state.project_path.clone() }`. Subsequent re-toggles before the prior write completes are acceptable; the latest write wins (last-write semantics matches user intent).

6. **Quality gate after each wave merge.** The orchestrator should run `cargo fmt --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings` after merging each wave's worktree branches, mirroring the Phase 1 cadence. A red gate must pause orchestration until resolved.

## Success Criteria

Phase 1.5 is complete when:

- [ ] All 4 CRITICAL items (C1–C4) from `ACTION_ITEMS.md` are resolved with new wired tests.
- [ ] All 8 MAJOR items (M1–M8) from `ACTION_ITEMS.md` are resolved.
- [ ] The 14 bundled MINOR items + 2 nitpicks (m2, m4, m5, m6, m7, m8, m9, m10, m11, m12, m13, m14, n1, n2) are resolved.
- [ ] Final workspace quality gate green:
  - [ ] `cargo fmt --all -- --check`
  - [ ] `cargo check --workspace --all-targets`
  - [ ] `cargo test --workspace`
  - [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] Manual smoke test: open fdemon on a Flutter app with a `MultiBlocProvider` chain, see the folded leader row, press `Right`, see the chain unfold; press `Esc` from Details to return to tree mode; press `r` to refresh and confirm no stale Details panel.
- [ ] `docs/ARCHITECTURE.md` and `docs/KEYBINDINGS.md` reflect the Phase 1.5 changes (Up/Down comment fix, `selected_row()` helper note if added).

## Out of Scope (explicitly deferred)

| Item | Reason |
|------|--------|
| m1 — split `widget_tree.rs` (1,650 lines) | Standalone follow-up. Splitting now compounds merge risk. |
| m3 — narrow-terminal details fallback | UX decision needed before implementing. |
| n3, n4, n5 | Pure style / coverage / Phase 2 supersedes. |

## Notes

- Phase 1.5 ships as a single PR — all 10 tasks merged together onto `feat/devtools-inspector-parity`.
- The reviewer skill should be re-run after Phase 1.5 closes, before Phase 2 begins.
- Implementors should read both `REVIEW.md` and `ACTION_ITEMS.md` before starting their task — each task here points back to specific review items by identifier (C1, M3, m9, etc.).
