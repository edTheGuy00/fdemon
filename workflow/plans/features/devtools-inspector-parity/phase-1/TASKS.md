# Phase 1 — Tree Rendering, Hide-Impl Toggle, Details Scaffold — Task Index

## Overview

Phase 1 of the DevTools Inspector parity feature. Replaces the current right-shifting widget tree with a DevTools-style guideline tree (vertical lines + branch ticks + per-type icons + collapsed implementation chains), adds a `Shift+H` toggle for hiding implementation widgets persisted to `.fdemon/config.toml`, and introduces a tabbed Details view (Widget properties tab populated; Render object and Flex explorer tabs stubbed) opened with `Enter` from the tree. Cf. parent plan at `workflow/plans/features/devtools-inspector-parity/PLAN.md`.

**Total Tasks:** 11
**Estimated Hours:** 27–41 hours

## Task Dependency Graph

```
                  ┌─────────────────────────────────────────┐
                  │ 01-core-diagnostics-and-row-builder     │
                  │ (fdemon-core/widget_tree.rs)            │
                  └────────────────┬────────────────────────┘
                                   ▼
                  ┌─────────────────────────────────────────┐
                  │ 02-state-inspector-extensions           │
                  │ (fdemon-app/state.rs)                   │
                  └──┬──────────────────┬─────────────────┬─┘
                     ▼                  ▼                 ▼
       ┌──────────────────┐  ┌────────────────────┐  ┌────────────────────┐
       │ 03-settings-     │  │ 04-message-        │  │ 07-tui-tree-       │
       │  hide-impl       │  │  variants          │  │  rendering         │
       │ (types.rs +      │  │ (message.rs)       │  │ (tree_panel.rs +   │
       │  settings.rs +   │  │                    │  │  tests.rs)         │
       │  engine wire-up) │  │                    │  │                    │
       └──────────────────┘  └─┬──────────────┬───┘  └────────────────────┘
                               ▼              ▼
              ┌──────────────────────────┐  ┌──────────────────────────┐
              │ 05-handlers-details-     │  │ 06-key-bindings          │
              │  and-toggle              │  │ (handler/keys.rs)        │
              │ (handler/devtools/       │  │                          │
              │  inspector.rs +          │  │                          │
              │  handler/devtools/mod.rs)│  │                          │
              └──────────────────────────┘  └──────────────────────────┘

         (depends on 02)        (depends on 02)
              ▼                      ▼
       ┌─────────────────────┐  ┌─────────────────────┐
       │ 08-tui-details-tabs │  │ 10-tui-footer-hints │
       │ (NEW details/* )    │  │ (devtools/mod.rs)   │
       └──────────┬──────────┘  └─────────────────────┘
                  ▼
       ┌──────────────────────────┐
       │ 09-tui-inspector-mode-   │
       │  switch                  │
       │ (inspector/mod.rs)       │
       └──────────────────────────┘

                  ▼ (depends on 01-10)
       ┌──────────────────────────┐
       │ 11-docs-update           │
       │ (Agent: doc_maintainer)  │
       │ ARCHITECTURE.md +        │
       │ KEYBINDINGS.md           │
       └──────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 01 | [01-core-diagnostics-and-row-builder](tasks/01-core-diagnostics-and-row-builder.md) | Not Started | — | 4–6h | `crates/fdemon-core/src/widget_tree.rs` |
| 02 | [02-state-inspector-extensions](tasks/02-state-inspector-extensions.md) | Not Started | 01 | 3–4h | `crates/fdemon-app/src/state.rs` |
| 03 | [03-settings-hide-implementation](tasks/03-settings-hide-implementation.md) | Not Started | 02 | 1–2h | `crates/fdemon-app/src/config/types.rs`, `crates/fdemon-app/src/config/settings.rs`, one engine init site |
| 04 | [04-message-variants](tasks/04-message-variants.md) | Not Started | 02 | 1h | `crates/fdemon-app/src/message.rs` |
| 05 | [05-handlers-details-and-toggle](tasks/05-handlers-details-and-toggle.md) | Not Started | 02, 04 | 3–4h | `crates/fdemon-app/src/handler/devtools/inspector.rs`, `crates/fdemon-app/src/handler/devtools/mod.rs` |
| 06 | [06-key-bindings](tasks/06-key-bindings.md) | Not Started | 02, 04 | 2–3h | `crates/fdemon-app/src/handler/keys.rs` |
| 07 | [07-tui-tree-rendering](tasks/07-tui-tree-rendering.md) | Not Started | 01, 02 | 5–7h | `crates/fdemon-tui/src/widgets/devtools/inspector/tree_panel.rs`, `crates/fdemon-tui/src/widgets/devtools/inspector/tests.rs` |
| 08 | [08-tui-details-tabs](tasks/08-tui-details-tabs.md) | Not Started | 02 | 4–5h | `crates/fdemon-tui/src/widgets/devtools/inspector/details/mod.rs` **NEW**, `details/properties_tab.rs` **NEW**, `details/render_object_tab.rs` **NEW**, `details/flex_explorer_tab.rs` **NEW** |
| 09 | [09-tui-inspector-mode-switch](tasks/09-tui-inspector-mode-switch.md) | Not Started | 08 | 2h | `crates/fdemon-tui/src/widgets/devtools/inspector/mod.rs` |
| 10 | [10-tui-footer-hints](tasks/10-tui-footer-hints.md) | Not Started | 02 | 1h | `crates/fdemon-tui/src/widgets/devtools/mod.rs` |
| 11 | [11-docs-update](tasks/11-docs-update.md) | Not Started | 01–10 | 1–2h | `docs/ARCHITECTURE.md`, `docs/KEYBINDINGS.md` |

## Wave Schedule

| Wave | Tasks | Notes |
|------|-------|-------|
| W1 | 01 | Pure-domain additions to `fdemon-core`. |
| W2 | 02 | State extensions; depends on the new types added in 01. |
| W3 | 03, 04, 07 | Independent of each other; all depend on 02. 07 also depends on 01. |
| W4 | 05, 06 | Independent of each other; both depend on 02 + 04. |
| W5 | 08, 10 | Independent of each other; both depend on 02 only. |
| W6 | 09 | Branches `inspector/mod.rs` on `details_open`; requires the details renderer from 08. |
| W7 | 11 | Documentation update; runs after all implementation is merged. |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read (Dependencies) |
|------|------------------------|--------------------------|
| 01-core-diagnostics-and-row-builder | `crates/fdemon-core/src/widget_tree.rs` | `tmp/devtools/packages/devtools_app/lib/src/shared/diagnostics/diagnostics_node.dart` (reference for `_alwaysVisible` predicate) |
| 02-state-inspector-extensions | `crates/fdemon-app/src/state.rs` | `crates/fdemon-core/src/widget_tree.rs` (types from task 01) |
| 03-settings-hide-implementation | `crates/fdemon-app/src/config/types.rs`, `crates/fdemon-app/src/config/settings.rs`, one engine init site (likely `crates/fdemon-app/src/engine.rs` or `crates/fdemon-app/src/state.rs::AppState::new` — implementor to verify with grep) | `crates/fdemon-app/src/state.rs` (InspectorState field from task 02) |
| 04-message-variants | `crates/fdemon-app/src/message.rs` | `crates/fdemon-app/src/state.rs` (DetailsTab from task 02 if referenced) |
| 05-handlers-details-and-toggle | `crates/fdemon-app/src/handler/devtools/inspector.rs`, `crates/fdemon-app/src/handler/devtools/mod.rs` | `crates/fdemon-app/src/message.rs`, `crates/fdemon-app/src/state.rs`, `crates/fdemon-app/src/config/types.rs` |
| 06-key-bindings | `crates/fdemon-app/src/handler/keys.rs` | `crates/fdemon-app/src/message.rs`, `crates/fdemon-app/src/state.rs` |
| 07-tui-tree-rendering | `crates/fdemon-tui/src/widgets/devtools/inspector/tree_panel.rs`, `crates/fdemon-tui/src/widgets/devtools/inspector/tests.rs`, `crates/fdemon-tui/src/theme/palette.rs` | `crates/fdemon-core/src/widget_tree.rs`, `crates/fdemon-app/src/state.rs`, `crates/fdemon-tui/src/widgets/devtools/inspector/mod.rs` |
| 08-tui-details-tabs | `crates/fdemon-tui/src/widgets/devtools/inspector/details/mod.rs` **NEW**, `crates/fdemon-tui/src/widgets/devtools/inspector/details/properties_tab.rs` **NEW**, `crates/fdemon-tui/src/widgets/devtools/inspector/details/render_object_tab.rs` **NEW**, `crates/fdemon-tui/src/widgets/devtools/inspector/details/flex_explorer_tab.rs` **NEW** | `crates/fdemon-tui/src/widgets/devtools/inspector/layout_panel.rs` (existing helpers to lift/reuse), `crates/fdemon-app/src/state.rs` |
| 09-tui-inspector-mode-switch | `crates/fdemon-tui/src/widgets/devtools/inspector/mod.rs` | `details/mod.rs` (task 08), `state.rs`, `layout_panel.rs` |
| 10-tui-footer-hints | `crates/fdemon-tui/src/widgets/devtools/mod.rs` | `crates/fdemon-app/src/state.rs` (read `details_open` field) |
| 11-docs-update | `docs/ARCHITECTURE.md`, `docs/KEYBINDINGS.md` | All implementation task files; `~/.claude/skills/doc-standards/schemas.md` |

### Overlap Matrix

(Pairs evaluated only between tasks in the same wave — i.e. tasks with no dependency between them.)

| Wave | Task Pair | Shared Write Files | Isolation Strategy |
|------|-----------|--------------------|--------------------|
| W3 | 03 + 04 | None | Parallel (worktree) |
| W3 | 03 + 07 | None | Parallel (worktree) |
| W3 | 04 + 07 | None | Parallel (worktree) |
| W4 | 05 + 06 | None | Parallel (worktree) |
| W5 | 08 + 10 | None | Parallel (worktree) |

No write-file collisions detected within any wave. All wave-peer tasks can be dispatched concurrently in isolated worktrees.

## Cross-Cutting Constraints

1. **`visible_nodes()` is kept as a backwards-compatible shim.** Task 02 keeps the existing signature `pub fn visible_nodes(&self) -> Vec<(&DiagnosticsNode, usize)>` working by re-implementing it as a thin flatten of the new `inspector_rows()`. This is the chosen path because `grep -rn "visible_nodes()" crates/` showed **6 production call sites + 2 test references** — too many for inline migration. Production callers that don't need ticks/group info (selection counting, mouse hit-testing, click index translation) keep using `visible_nodes()`. Only the tree renderer (task 07) migrates to `inspector_rows()`.

2. **`InspectorState::reset()` must preserve `hide_implementation_widgets`.** Task 02 must explicitly carry over the field across resets so a user's toggle preference survives hot-restart, session switch, and refresh.

3. **Mouse-region tests cannot regress.** Task 07 must keep the click invariants documented in `crates/fdemon-tui/src/widgets/devtools/inspector/tests.rs` (whole-row select + glyph-cell toggle, with last-pushed-wins-at-same-z). The migration to `inspector_rows()` changes the glyph X-position math; the row-region width and the layered push order must remain identical.

4. **Frozen selection in details mode.** Task 05 must make `handle_inspector_navigate` a no-op when `state.devtools_view_state.inspector.details_open == true` (return `UpdateResult::none()` at the top of the function).

5. **Tiered Esc.** Task 05 must route Esc through a new check in `handler/devtools/mod.rs`: if `details_open`, dispatch `DevToolsInspectorCloseDetails`; otherwise, fall through to the existing "exit DevTools → Logs" path.

6. **Tab-cycling must skip hidden tabs.** Although the Render Object and Flex Explorer tabs are stubbed in this phase (showing "Coming soon"), the per-widget-type visibility logic comes in Phase 3. For Phase 1, tabs are all visible; `Tab` cycles Properties → Render object → Flex explorer → Properties.

7. **Settings persistence.** Task 03 plus Task 05 together must write the toggled `hide_implementation_widgets` value back to `.fdemon/config.toml` on flip. If the project does not yet have a write-back path for runtime-edited settings, the implementor should follow the same pattern used by other persisted toggles (search for existing `save_settings()` or `persist_settings()` helpers). If none exists, an in-memory toggle for this phase + a TODO is acceptable; flag this in the task's completion summary.

## Success Criteria

Phase 1 is complete when:

- [ ] The deep `BlocProvider` chain demonstrated in the user's screenshot (~25 levels) collapses into a single "+ N more widgets" leader row directly under `MultiBlocProvider`. Expanding the leader reveals the chain inline at the leader's indent + 1.
- [ ] The widget tree renders vertical guideline `│` columns for every ancestor that still has more siblings below the current row, and `├─` / `└─` branch ticks at each child entry. Visual parity with DevTools screenshot 2.
- [ ] Each widget row is preceded by a 1-cell type-icon glyph; widgets without a specific mapping fall back to a circle/letter glyph.
- [ ] Pressing `Shift+H` while the Inspector tab is active toggles chain collapsing. The toggle state persists across `r` refreshes and is written back to `.fdemon/config.toml` under `[devtools] hide_implementation_widgets`.
- [ ] Pressing `Enter` on a selected widget opens a tabbed Details view in the right pane: tabs `Widget properties` (populated from existing layout data + a stub property list), `Render object` (stub "Coming soon"), `Flex explorer` (stub "Coming soon").
- [ ] `Tab` / `Shift+Tab` cycle the active tab while Details is open.
- [ ] `Esc` while Details is open closes Details back to tree mode (without exiting DevTools). A second `Esc` in tree mode exits DevTools as today.
- [ ] `Up` / `Down` are no-ops while Details is open.
- [ ] Footer hint string updates per mode (tree mode shows `[Enter] Details`, details mode shows `[Esc] Close [Tab] Next Tab`).
- [ ] `cargo fmt --all -- --check && cargo check --workspace --all-targets && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings` all pass on the implementor's branch.
- [ ] No regression in existing inspector tests (`tests.rs` mouse-region invariants, layout-panel snapshot tests, `handler/devtools/inspector.rs` navigation tests).
- [ ] `docs/ARCHITECTURE.md` and `docs/KEYBINDINGS.md` reflect the new state model and the new key bindings.

## Keyboard Shortcuts (added or changed by Phase 1)

| Key | Mode | Action |
|-----|------|--------|
| `Enter` | Inspector tab, tree mode | Open Details view for the selected widget |
| `Esc` | Inspector tab, details mode | Close Details (return to tree mode) |
| `Esc` | Inspector tab, tree mode | Exit DevTools → Logs (unchanged behavior) |
| `Tab` | Inspector tab, details mode | Cycle to next tab |
| `Shift+Tab` | Inspector tab, details mode | Cycle to previous tab |
| `Shift+H` | Inspector tab | Toggle "Hide implementation widgets" |
| `h` (lowercase) | Inspector tab | Collapse node (unchanged) |
| `Up` / `Down` / `j` / `k` | Inspector tab, details mode | **No-op** (selection frozen) |

## Notes

- Phase 1 ships as a single PR — all 11 tasks are merged together.
- Phase 2 (Render object tab via `getProperties` RPC + Flex explorer ASCII visualization) and Phase 3 (conditional tab visibility) will reuse the scaffold built here.
- The two stub tabs in this phase (`render_object_tab.rs`, `flex_explorer_tab.rs`) are intentionally minimal — they render a centered "Coming soon" message. Task 08 must structure them so Phase 2 can fill the bodies without rewriting the modules.
- Reference DevTools source files (read-only) are under `tmp/devtools/packages/devtools_app/lib/src/`. Key locations are linked from each task file's "Details" section as needed.
