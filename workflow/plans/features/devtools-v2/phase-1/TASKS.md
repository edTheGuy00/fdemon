# Phase 1: Widget Component Decomposition - Task Index

## Overview

Break existing oversized widget and handler files into smaller, modular sub-components without changing any visible behavior. Pure refactor — same visual output, same test assertions, but code organized into ~200-400 line files instead of 800-1500 line monoliths.

**Total Tasks:** 4
**Waves:** 2 (tasks 01-03 parallel, then task 04)

## Task Dependency Graph

```
┌──────────────────────────┐  ┌───────────────────────────┐  ┌───────────────────────────┐
│ 01-split-inspector       │  │ 02-split-performance      │  │ 03-split-handler-devtools │
│ (fdemon-tui)             │  │ (fdemon-tui)              │  │ (fdemon-app)              │
└────────────┬─────────────┘  └─────────────┬─────────────┘  └─────────────┬─────────────┘
             │                              │                              │
             └──────────────┬───────────────┘──────────────────────────────┘
                            ▼
               ┌─────────────────────────┐
               │ 04-verify-no-regressions│
               │ (workspace-wide)        │
               └─────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Crate | Modules |
|---|------|--------|------------|-------|---------|
| 1 | [01-split-inspector-widget](tasks/01-split-inspector-widget.md) | Done | - | `fdemon-tui` | `widgets/devtools/inspector/{mod,tree_panel,details_panel,tests}.rs` |
| 2 | [02-split-performance-widget](tasks/02-split-performance-widget.md) | Done | - | `fdemon-tui` | `widgets/devtools/performance/{mod,frame_section,memory_section,stats_section,styles}.rs` |
| 3 | [03-split-handler-devtools](tasks/03-split-handler-devtools.md) | Done | - | `fdemon-app` | `handler/devtools/{mod,inspector,layout}.rs` |
| 4 | [04-verify-no-regressions](tasks/04-verify-no-regressions.md) | Done | 1, 2, 3 | workspace | All devtools modules |

## Dispatch Plan

**Wave 1** (parallel — no file conflicts between tasks):
- Task 01: Split inspector widget (fdemon-tui crate, `inspector.rs` only)
- Task 02: Split performance widget (fdemon-tui crate, `performance.rs` only)
- Task 03: Split handler devtools (fdemon-app crate, `handler/devtools.rs` only)

**Wave 2** (sequential — depends on all Wave 1 tasks):
- Task 04: Full verification pass

## Success Criteria

Phase 1 is complete when:

- [x] `inspector.rs` split into `inspector/{mod,tree_panel,details_panel,tests}.rs` (each < 400 lines)
- [x] `performance.rs` split into `performance/{mod,frame_section,memory_section,stats_section,styles}.rs`
- [x] `handler/devtools.rs` split into `handler/devtools/{mod,inspector,layout}.rs`
- [x] All 27 inspector widget tests pass unchanged
- [x] All 20 performance widget tests pass unchanged
- [x] All 42+ handler/devtools tests pass unchanged
- [x] `cargo clippy --workspace` clean
- [x] Visual output identical to pre-refactor

## Notes

- **No `mod.rs` conflicts**: Rust resolves `pub mod inspector;` to either `inspector.rs` or `inspector/mod.rs` automatically. The parent `devtools/mod.rs` declarations and re-exports remain unchanged, so tasks 01 and 02 do not conflict.
- **Cross-crate isolation**: Tasks 01/02 touch `fdemon-tui` while task 03 touches `fdemon-app`. No shared file edits.
- **Handler split naming**: The plan originally proposed `handler/devtools/performance.rs`, but research shows there are zero performance-specific handlers — the file holds layout explorer handlers. The task uses `layout.rs` instead, which accurately reflects the current content and provides a natural home for layout handlers that will be merged into inspector handlers in Phase 2.
