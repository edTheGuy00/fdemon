# Phase 4 — Polish + Documentation — Task Index

## Overview

Live-edge drift tests, key-bindings doc, architecture doc. Last gate before merge.

**Total Tasks:** 3
**Estimated Hours:** 1.5-2.5 hours

## Task Dependency Graph

```
08-live-edge-drift-tests
09-update-keybindings-doc      (parallel)
10-update-architecture-doc     (parallel, doc_maintainer)
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 8 | [08-live-edge-drift-tests](tasks/08-live-edge-drift-tests.md) | Not Started | Phase 3 | 1h | `widgets/devtools/performance/` tests |
| 9 | [09-update-keybindings-doc](tasks/09-update-keybindings-doc.md) | Not Started | Phase 3 | 0.5h | `docs/KEYBINDINGS.md` |
| 10 | [10-update-architecture-doc](tasks/10-update-architecture-doc.md) | Not Started | Phase 3 | 0.5h | `docs/ARCHITECTURE.md` (doc_maintainer) |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read |
|------|----------------------|-----------|
| 08 | `widgets/devtools/performance/frame_chart/tests.rs`, `widgets/devtools/performance/memory_chart/chart.rs` tests | All Phase 3 outputs |
| 09 | `docs/KEYBINDINGS.md` | — |
| 10 | `docs/ARCHITECTURE.md` | — |

### Overlap Matrix

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|-------------------|-------------------|
| 08 + 09 | None | Parallel (worktree) |
| 08 + 10 | None | Parallel (worktree) |
| 09 + 10 | None (different docs) | Parallel (worktree) |

### Wave Plan

All three in parallel.

## Success Criteria

- [ ] Live-edge drift tests pass and assert Model A semantics.
- [ ] `docs/KEYBINDINGS.md` lists all new Performance bindings.
- [ ] `docs/ARCHITECTURE.md` describes the focused-section model + render-hint cells.
- [ ] All four CI quality gates pass.
- [ ] Manual smoke verification documented in task 08 completion summary.
