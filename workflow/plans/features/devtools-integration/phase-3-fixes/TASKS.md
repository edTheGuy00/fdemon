# Phase 3 Fixes: Review Action Items — Task Index

## Overview

Address all issues identified in the Phase 3 code review (NEEDS WORK verdict). Fixes cover a blocking resource leak, three critical bugs, five major code quality issues, two minor issues, and a structural refactor of `session.rs` into a module directory.

**Total Tasks:** 6
**Estimated Hours:** 10-14 hours

## Task Dependency Graph

```
┌─────────────────────────┐  ┌──────────────────────────┐  ┌───────────────────────────┐
│ 01-perf-polling-        │  │ 02-isolate-cache-        │  │ 04-import-path-           │
│ lifecycle               │  │ invalidation             │  │ fixes                     │
│ (fdemon-app)            │  │ (fdemon-daemon,          │  │ (fdemon-app)              │
│ BLOCKING + CRITICAL     │  │  fdemon-app)             │  │ MAJOR + MINOR             │
└───────────┬─────────────┘  │ CRITICAL                 │  └──────────┬────────────────┘
            │                └──────────┬───────────────┘             │
            │                           │                             │
            │  ┌────────────────────────┘                             │
            │  │  ┌───────────────────────────┐                      │
            │  │  │ 03-stats-computation-     │                      │
            │  │  │ fixes                     │                      │
            │  │  │ (fdemon-app, fdemon-core) │                      │
            │  │  │ CRITICAL + MAJOR          │                      │
            │  │  └──────────┬────────────────┘                      │
            │  │             │                                        │
            └──┼─────────────┼────────────────────────────────────────┘
               │             │
               ▼             ▼
       ┌──────────────────────────┐
       │ 05-session-module-split  │
       │ (fdemon-app)             │
       │ STRUCTURAL               │
       └──────────┬───────────────┘
                  │
                  ▼
       ┌──────────────────────────┐
       │ 06-gc-event-filtering    │
       │ (fdemon-app)             │
       │ MINOR                    │
       └──────────────────────────┘
```

## Waves (Parallelizable Groups)

### Wave 1 (Bug Fixes — All Independent)
- **01-perf-polling-lifecycle** — Fix perf_shutdown_tx not signaled + JoinHandle not tracked (BLOCKING #1, CRITICAL #2)
- **02-isolate-cache-invalidation** — Fix stale isolate ID after hot restart (CRITICAL #3)
- **03-stats-computation-fixes** — Fix unused param, FPS calc, dead branch, total_frames, iter().count() (CRITICAL #4, MAJOR #5-#8)
- **04-import-path-fixes** — Fix submodule path access + broken doc comment (MAJOR #9, MINOR #10)

### Wave 2 (Structural Refactor)
- **05-session-module-split** — Split session.rs (2,731 lines) into session/ module directory (MINOR #12)

### Wave 3 (Enhancement)
- **06-gc-event-filtering** — Filter GC events to prevent Scavenge drowning (MINOR #11)

## Tasks

| # | Task | Status | Depends On | Est. Hours | Crate | Key Files |
|---|------|--------|------------|------------|-------|-----------|
| 1 | [01-perf-polling-lifecycle](tasks/01-perf-polling-lifecycle.md) | [x] Done | - | 2-3h | `fdemon-app` | `handler/session_lifecycle.rs`, `handler/session.rs`, `actions.rs`, `session.rs` |
| 2 | [02-isolate-cache-invalidation](tasks/02-isolate-cache-invalidation.md) | [x] Done | - | 1-2h | `fdemon-daemon`, `fdemon-app` | `vm_service/client.rs`, `handler/update.rs` |
| 3 | [03-stats-computation-fixes](tasks/03-stats-computation-fixes.md) | [x] Done | - | 2-3h | `fdemon-app`, `fdemon-core` | `session.rs`, `performance.rs` |
| 4 | [04-import-path-fixes](tasks/04-import-path-fixes.md) | [x] Done | - | 0.5h | `fdemon-app` | `actions.rs` |
| 5 | [05-session-module-split](tasks/05-session-module-split.md) | [x] Done | 1, 2, 3, 4 | 3-4h | `fdemon-app` | `session.rs` → `session/` directory |
| 6 | [06-gc-event-filtering](tasks/06-gc-event-filtering.md) | [x] Done | 5 | 1-2h | `fdemon-app` | `session/performance.rs`, `handler/update.rs` |

## Success Criteria

Phase 3 fixes are complete when:

- [ ] `perf_shutdown_tx` signaled on all session close paths (lifecycle, exited, AppStop)
- [ ] Performance polling JoinHandle tracked and cleaned up on session close
- [ ] Isolate ID cache invalidated on hot restart
- [ ] `compute_stats` has no unused parameters
- [ ] `calculate_fps` returns actual FPS rate using consistent clock domain
- [ ] `frames.iter().count()` replaced with `frames.len()`
- [ ] Dead branch in `compute_stats` removed
- [ ] `total_frames` semantics clarified (renamed or made cumulative)
- [ ] Import paths use re-exported surface, not submodule paths
- [ ] `session.rs` split into `session/` module directory (all files < 500 lines)
- [ ] GC events filtered to prevent Scavenge drowning
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace -- -D warnings` passes
- [ ] `cargo fmt --all -- --check` passes
- [ ] No new test failures

## Notes

- **Wave 1 tasks are fully independent** — they touch different files and can be dispatched in parallel.
- **Task 05 (session split) depends on all Wave 1 tasks** — we want to split clean code, not code that's about to change.
- **Task 06 (GC filtering) depends on Task 05** — it modifies `session/performance.rs` which is created by the split.
- **Review reference:** `workflow/reviews/features/devtools-integration-phase-3/ACTION_ITEMS.md`
