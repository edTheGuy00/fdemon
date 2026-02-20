# Phase 2: Merged Inspector + Layout Tab - Task Index

## Overview

Combine the Inspector and Layout tabs into a single unified Inspector tab. The widget tree occupies one half, a new Layout Explorer panel occupies the other half. The separate Layout tab is removed entirely.

**Total Tasks:** 7
**Waves:** 4 (tasks 01-02 parallel, then 03-05 parallel, then 06, then 07)

## Task Dependency Graph

```
Wave 1 (parallel — different crates)
┌──────────────────────────────┐   ┌─────────────────────────────────────┐
│ 01-add-edge-insets-core-types│   │ 02-merge-layout-state-into-inspector│
│ (fdemon-core)                │   │ (fdemon-app)                        │
└──────────────┬───────────────┘   └──────────┬──────────────────────────┘
               │                              │
Wave 2 (parallel — different crates/files)    │
               │    ┌─────────────────────────┤
               │    │                         │
               ▼    │                         ▼
┌──────────────────────────┐   ┌───────────────────────────────┐
│ 04-extract-padding-vm    │   │ 03-remove-layout-panel-variant│
│ (fdemon-daemon)          │   │ (fdemon-app + fdemon-tui)     │
│ depends: 01              │   │ depends: 02                   │
└──────────────┬───────────┘   └───────────┬───────────────────┘
               │                           │
               │   ┌───────────────────────────────────┐
               │   │ 05-create-layout-panel-widget     │
               │   │ (fdemon-tui inspector/)            │
               │   │ depends: 01, 02                   │
               │   └───────────┬───────────────────────┘
               │               │
Wave 3         └───────┬───────┘
                       ▼
          ┌─────────────────────────────────┐
          │ 06-wire-merged-inspector-layout │
          │ (fdemon-app + fdemon-tui)       │
          │ depends: 03, 04, 05            │
          └──────────────┬──────────────────┘
                         │
Wave 4                   ▼
          ┌─────────────────────────────────┐
          │ 07-final-test-and-cleanup       │
          │ (workspace-wide)                │
          │ depends: 06                     │
          └─────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Crate | Modules |
|---|------|--------|------------|-------|---------|
| 1 | [01-add-edge-insets-core-types](tasks/01-add-edge-insets-core-types.md) | Not Started | - | `fdemon-core` | `widget_tree.rs` |
| 2 | [02-merge-layout-state-into-inspector](tasks/02-merge-layout-state-into-inspector.md) | Not Started | - | `fdemon-app` | `state.rs`, `handler/devtools/{mod,inspector,layout}.rs` |
| 3 | [03-remove-layout-panel-variant](tasks/03-remove-layout-panel-variant.md) | Not Started | 2 | `fdemon-app`, `fdemon-tui` | `state.rs`, `handler/keys.rs`, `handler/devtools/mod.rs`, `widgets/devtools/mod.rs`, `widgets/devtools/layout_explorer.rs` |
| 4 | [04-extract-padding-from-vm-service](tasks/04-extract-padding-from-vm-service.md) | Not Started | 1 | `fdemon-daemon` | `vm_service/extensions/layout.rs` |
| 5 | [05-create-layout-panel-widget](tasks/05-create-layout-panel-widget.md) | Not Started | 1, 2 | `fdemon-tui` | `widgets/devtools/inspector/layout_panel.rs` |
| 6 | [06-wire-merged-inspector-layout](tasks/06-wire-merged-inspector-layout.md) | Not Started | 3, 4, 5 | `fdemon-app`, `fdemon-tui` | `widgets/devtools/inspector/{mod,details_panel}.rs`, `handler/devtools/inspector.rs`, `state.rs` |
| 7 | [07-final-test-and-cleanup](tasks/07-final-test-and-cleanup.md) | Not Started | 6 | workspace | All devtools modules |

## Dispatch Plan

**Wave 1** (parallel — no file conflicts):
- Task 01: Add EdgeInsets type (fdemon-core only)
- Task 02: Merge layout state into inspector (fdemon-app only)

**Wave 2** (parallel — no file conflicts between tasks):
- Task 03: Remove DevToolsPanel::Layout variant (fdemon-app state/handler + fdemon-tui devtools/mod.rs)
- Task 04: Extract padding from VM Service (fdemon-daemon only)
- Task 05: Create layout panel widget (fdemon-tui inspector/ only — new file, no conflicts with 03)

**Wave 3** (sequential — depends on all Wave 2 tasks):
- Task 06: Wire merged inspector layout + auto-fetch + 50/50 split

**Wave 4** (sequential — final verification):
- Task 07: Full test and cleanup pass

## Success Criteria

Phase 2 is complete when:

- [ ] Inspector and Layout tabs merged into single Inspector tab
- [ ] `DevToolsPanel::Layout` variant removed
- [ ] `'l'` keybinding removed from DevTools mode
- [ ] Widget tree and Layout Explorer shown in 50/50 split
- [ ] Responsive: horizontal split (wide >= 100 cols) / vertical split (narrow < 100 cols)
- [ ] Layout Explorer shows box model visualization with dimensions, padding, constraints
- [ ] Layout auto-fetches on tree node selection with 500ms debounce
- [ ] Source location (file:line) displayed in layout panel
- [ ] All new code has unit tests (20+ new tests)
- [ ] Old `layout_explorer.rs` file deleted
- [ ] All existing tests pass (with updates for new layout)
- [ ] `cargo fmt && cargo check && cargo test && cargo clippy` clean

## Notes

- **State merge is a pure refactor**: Task 02 moves `LayoutExplorerState` fields into `InspectorState` and updates all handler references. The Layout tab still works after this task — it just reads from `inspector.*` instead of `layout_explorer.*`. This makes Task 03 (actual removal) a clean deletion step.
- **Cross-crate isolation**: Wave 1 tasks touch different crates. Wave 2 tasks touch different files within their respective crates.
- **layout_panel.rs replaces details_panel.rs**: The existing details panel (widget name, properties list, source location) is replaced by a full layout explorer. The widget name and source location are preserved at the top of the new layout panel.
- **Debounce difference**: Inspector tree fetch uses a 2-second cooldown (`is_fetch_debounced`). Layout auto-fetch uses a 500ms cooldown — shorter because it fires on every navigation step and shouldn't feel sluggish.
- **Object groups**: Inspector uses `"fdemon-inspector-{n}"` (managed by ObjectGroupManager). Layout uses `"devtools-layout"`. These remain separate — no change to object group management.
