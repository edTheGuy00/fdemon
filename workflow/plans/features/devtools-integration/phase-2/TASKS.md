# Phase 2: Flutter Service Extensions - Task Index

## Overview

Implement typed wrappers for all Flutter-specific VM Service extensions in `fdemon-daemon`, plus domain data models in `fdemon-core`. This phase builds the complete data/RPC layer that Phase 4 (TUI DevTools Mode) will consume. No TEA integration or UI changes in this phase.

**Total Tasks:** 6
**Estimated Hours:** 14-20 hours

## Task Dependency Graph

```
┌──────────────────────────┐     ┌──────────────────────────┐
│ 01-extension-framework   │     │ 02-widget-tree-types     │
│ (fdemon-daemon)          │     │ (fdemon-core)            │
└───────────┬──────────────┘     └──────────┬───────────────┘
            │                               │
    ┌───────┼──────────┬────────────────────┤
    │       │          │                    │
    ▼       ▼          ▼                    ▼
┌────────┐ ┌─────────────────┐ ┌──────────────────────────┐
│03-debug│ │04-widget-tree   │ │05-layout-explorer        │
│-overlay│ │-extensions      │ │-extension                │
│-toggles│ │(depends: 01,02) │ │(depends: 01, 02)         │
└────────┘ └─────────────────┘ └──────────────────────────┘
    │
    ▼
┌──────────────────────────┐
│ 06-debug-dump-extensions │
│ (depends: 01)            │
└──────────────────────────┘
```

## Waves (Parallelizable Groups)

### Wave 1 (Foundation)
- **01-extension-framework** — Generic extension call infrastructure
- **02-widget-tree-types** — Domain data models (independent crate)

### Wave 2 (Extension Wrappers)
- **03-debug-overlay-toggles** — 4 boolean toggle extensions
- **04-widget-tree-extensions** — Inspector tree RPCs + DiagnosticsNode parsing
- **05-layout-explorer-extension** — Layout explorer RPC + layout property parsing
- **06-debug-dump-extensions** — 3 debug dump text RPCs

## Tasks

| # | Task | Status | Depends On | Est. Hours | Crate | Key Modules |
|---|------|--------|------------|------------|-------|-------------|
| 1 | [01-extension-framework](tasks/01-extension-framework.md) | Done | - | 3-4h | `fdemon-daemon` | `vm_service/extensions.rs` |
| 2 | [02-widget-tree-types](tasks/02-widget-tree-types.md) | Done | - | 2-3h | `fdemon-core` | `widget_tree.rs` |
| 3 | [03-debug-overlay-toggles](tasks/03-debug-overlay-toggles.md) | Done | 1 | 2-3h | `fdemon-daemon` | `vm_service/extensions.rs` |
| 4 | [04-widget-tree-extensions](tasks/04-widget-tree-extensions.md) | Done | 1, 2 | 3-4h | `fdemon-daemon` | `vm_service/extensions.rs` |
| 5 | [05-layout-explorer-extension](tasks/05-layout-explorer-extension.md) | Done | 1, 2 | 2-3h | `fdemon-daemon` | `vm_service/extensions.rs` |
| 6 | [06-debug-dump-extensions](tasks/06-debug-dump-extensions.md) | Done | 1 | 1-2h | `fdemon-daemon` | `vm_service/extensions.rs` |

## Success Criteria

Phase 2 is complete when:

- [x] All debug overlay toggles callable via typed API with state tracking
- [x] Widget summary tree retrieved and parsed into `DiagnosticsNode` structs
- [x] Detail subtree and selected widget fetched for any node by `valueId`
- [x] Layout explorer data parsed with constraints, size, and flex properties
- [x] Debug dump commands return valid text output
- [x] Object group lifecycle managed (create, dispose)
- [x] Extension-not-available errors handled gracefully (profile mode, release mode)
- [x] All new code has unit tests with mock JSON responses
- [x] No regressions in existing functionality (`cargo test --workspace`)

## New Module Structure

```
crates/fdemon-core/src/
├── ...existing files...
└── widget_tree.rs              # NEW: DiagnosticsNode, WidgetNode, LayoutInfo, CreationLocation

crates/fdemon-daemon/src/vm_service/
├── mod.rs                      # MODIFIED: add extensions module export
├── client.rs                   # MODIFIED: add isolate ID caching, call_extension() convenience
├── protocol.rs                 # existing
├── errors.rs                   # existing
├── logging.rs                  # existing
└── extensions.rs               # NEW: all extension wrappers (debug overlays, inspector, layout, dumps)
```

## Notes

- **No TEA integration in Phase 2.** New `Message` variants, `UpdateAction` variants, and handler code for triggering extensions from the UI belong to Phase 4.
- **All extension params are strings** at the wire level — booleans are `"true"`/`"false"`, integers are `"2"`. The typed wrappers handle conversion.
- **`isolateId` is always required.** The extension framework caches the main isolate ID discovered during connection.
- **Inspector extensions are debug-only.** The framework must handle `ExtensionNotAvailable` errors gracefully for profile/release builds.
- **Object group management is critical** for inspector tree calls — references are only valid while their group is alive.
- **Parameter key inconsistency:** Inspector tree calls use `objectGroup` + `arg`, but layout explorer uses `groupName` + `id`. The typed wrappers must use the correct keys for each.
- Phase 1 established the VM Service client with `request(method, params)` — Phase 2 builds typed wrappers on top of this existing infrastructure.
