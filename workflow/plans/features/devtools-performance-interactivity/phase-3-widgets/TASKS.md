# Phase 3 — Widgets — Task Index

## Overview

Make widgets honor focus, scroll offsets, and selection. Register mouse regions for section focus and row selection. Write `Cell<usize>` render-hints every frame.

**Total Tasks:** 3
**Estimated Hours:** 8-12 hours

## Task Dependency Graph

```
05-frame-chart-scroll-and-focus
06-memory-chart-scroll-and-focus  (parallel)
07-alloc-table-scroll-and-selection  (parallel)
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 5 | [05-frame-chart-scroll-and-focus](tasks/05-frame-chart-scroll-and-focus.md) | Not Started | Phase 2 | 3-4h | `widgets/devtools/performance/frame_chart/`, `widgets/devtools/performance/mod.rs` |
| 6 | [06-memory-chart-scroll-and-focus](tasks/06-memory-chart-scroll-and-focus.md) | Not Started | Phase 2 | 3-4h | `widgets/devtools/performance/memory_chart/mod.rs`, `chart.rs`, `widgets/devtools/performance/mod.rs` |
| 7 | [07-alloc-table-scroll-and-selection](tasks/07-alloc-table-scroll-and-selection.md) | Not Started | Phase 2 | 2-4h | `widgets/devtools/performance/memory_chart/table.rs` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read |
|------|----------------------|-----------|
| 05 | `widgets/devtools/performance/frame_chart/mod.rs`, `frame_chart/bars.rs`, `widgets/devtools/performance/mod.rs` | `session/performance.rs` |
| 06 | `widgets/devtools/performance/memory_chart/mod.rs`, `memory_chart/chart.rs`, `widgets/devtools/performance/mod.rs` | `session/performance.rs` |
| 07 | `widgets/devtools/performance/memory_chart/table.rs` | `session/performance.rs` |

### Overlap Matrix

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|-------------------|-------------------|
| 05 + 06 | `widgets/devtools/performance/mod.rs` | **Sequential (same branch)** — both modify the top-level performance widget mod |
| 05 + 07 | None | Parallel (worktree) |
| 06 + 07 | None | Parallel (worktree) |

### Wave Plan

- **Wave 1**: 05 + 07 in parallel (different files entirely after the mod.rs touch in 05).
- **Wave 2**: 06 (rebase onto 05's mod.rs change).

Alternative single-branch sequence: 05 → 06 → 07 (no parallelism, but no merge work).

## Success Criteria

- [ ] Focused section has visible focus highlight (border color/style).
- [ ] Each chart respects its scroll offset; live-edge drift is correct.
- [ ] Allocation table scrolls and shows selected row highlight.
- [ ] Render-hint cells written every frame.
- [ ] Mouse clicks on each section emit the correct `Message`s.
- [ ] `cargo test --workspace` passes; new render tests cover bound conditions.
