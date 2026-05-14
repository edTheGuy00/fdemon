# Phase 1 — State + Messages — Task Index

## Overview

Add `PerfSection` enum, scroll/focus/render-hint fields on `PerformanceState`, and 7 new `Message` variants. No behavior change yet.

**Total Tasks:** 2
**Estimated Hours:** 2-3 hours

## Task Dependency Graph

```
01-perf-section-enum-and-state-fields
02-perf-message-variants   (parallel)
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-perf-section-enum-and-state-fields](tasks/01-perf-section-enum-and-state-fields.md) | Done | — | 1-2h | `session/performance.rs` |
| 2 | [02-perf-message-variants](tasks/02-perf-message-variants.md) | Done | — | 1h | `message.rs` |

## File Overlap Analysis

| Task | Files Modified (Write) | Files Read |
|------|----------------------|-----------|
| 01 | `crates/fdemon-app/src/session/performance.rs` | — |
| 02 | `crates/fdemon-app/src/message.rs` | `session/performance.rs` (for `PerfSection` import) |

### Overlap Matrix

| Task Pair | Shared Write Files | Isolation Strategy |
|-----------|-------------------|-------------------|
| 01 + 02 | None (different crates' modules) | **Parallel (worktree)**, but 02 imports `PerfSection` from 01 — must merge 01 first or block 02 |

### Wave Plan

In practice, 02 needs the `PerfSection` enum from 01 to compile. Run sequentially: 01 → 02. Or run in parallel-worktree where 02's prep stub-defines `PerfSection` locally until 01 merges.

**Pragmatic plan**: Sequential (01 → 02) — the gain from parallelizing two ≤ 1.5 h tasks is small.

## Success Criteria

- [ ] `PerfSection` enum exists with three variants.
- [ ] `PerformanceState` has 5 new behavioral fields + 3 render-hint cells.
- [ ] 7 new `Message` variants exist.
- [ ] `cargo check --workspace --all-targets` and `cargo test --workspace` pass.
- [ ] No widget or handler change yet (this phase is structural only).
