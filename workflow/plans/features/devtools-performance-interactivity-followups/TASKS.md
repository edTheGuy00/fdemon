# DevTools Performance Interactivity Follow-ups — Task Index

## Overview

Two small independent follow-ups surfaced during phase-4 smoke verification.

**Total Tasks:** 2
**Estimated Hours:** 0.75-1.25 hours

## Task Dependency Graph

```
01-clear-scroll-offset-on-frame-select       (parallel)
02-mouse-wheel-scroll-in-perf-panel          (parallel)
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-clear-scroll-offset-on-frame-select](tasks/01-clear-scroll-offset-on-frame-select.md) | Not Started | — | 0.25-0.5h | `handler/devtools/performance.rs` |
| 2 | [02-mouse-wheel-scroll-in-perf-panel](tasks/02-mouse-wheel-scroll-in-perf-panel.md) | Not Started | — | 0.5-0.75h | `handler/mouse/devtools.rs` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read |
|------|----------------------|-----------|
| 01 | `crates/fdemon-app/src/handler/devtools/performance.rs` (handler body + existing test inversion) | — |
| 02 | `crates/fdemon-app/src/handler/mouse/devtools.rs` (new `handle_performance_scroll` + tests) | `crates/fdemon-app/src/handler/mouse/{inspector_scroll,network_scroll}` patterns |

### Overlap Matrix

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|-------------------|-------------------|
| 01 + 02 | None | Parallel (worktree) |

### Wave Plan

Both tasks in parallel (Wave 1).

## Success Criteria

- [ ] Left/Right frame select clears `frame_chart_scroll_offset` (test 4 from phase-4 task 08 inverts to forward assertion).
- [ ] Wheel up/down inside Performance scrolls the focused section consistently with keyboard `↑`/`↓`/`k`/`j`.
- [ ] Shift+wheel maps to `PerfPageUp`/`PerfPageDown` for parity with `PageUp`/`PageDown` keys.
- [ ] All four CI quality gates pass.
