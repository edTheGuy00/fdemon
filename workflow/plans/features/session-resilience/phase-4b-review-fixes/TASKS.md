# Phase 4b: Review Fixes — Task Index

## Overview

Address concerns and observations from the [phase-4 review](../../../../reviews/features/session-resilience-phase-4/REVIEW.md). Three concerns (stopped session accumulation, dead code removal, missing test coverage) and three non-blocking observations (comment fix, doc improvement, inaccurate task notes).

**Total Tasks:** 5

## Task Dependency Graph

```
┌──────────────────────────────┐
│ 01-remove-find-by-device-id  │  (wave 1 — independent)
└──────────────────────────────┘

┌──────────────────────────────┐
│ 02-auto-evict-stopped        │  (wave 1 — independent)
└──────────────────────────────┘

┌──────────────────────────────┐
│ 03-missing-phase-tests       │  (wave 1 — independent, but depends on 01 for test cleanup)
└──────────┬───────────────────┘
           │
           ▼
┌──────────────────────────────┐
│ 04-doc-comment-fixes         │  (wave 2 — after 01, updates doc that references deleted method)
└──────────────────────────────┘

┌──────────────────────────────┐
│ 05-fix-task-03-notes         │  (wave 1 — independent, docs only)
└──────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 1 | [01-remove-find-by-device-id](tasks/01-remove-find-by-device-id.md) | Done | - | `session_manager.rs` |
| 2 | [02-auto-evict-stopped](tasks/02-auto-evict-stopped.md) | Done | - | `session_manager.rs` |
| 3 | [03-missing-phase-tests](tasks/03-missing-phase-tests.md) | Done | 1 | `session_manager.rs`, `launch_context.rs` |
| 4 | [04-doc-comment-fixes](tasks/04-doc-comment-fixes.md) | Done | 1 | `session_manager.rs`, `launch_context.rs` |
| 5 | [05-fix-task-03-notes](tasks/05-fix-task-03-notes.md) | Done | - | `workflow/` docs only |

## Success Criteria

Phase 4b is complete when:

- [x] `find_by_device_id` is removed from production code and all tests
- [x] `create_session*` methods auto-evict the oldest stopped session when `MAX_SESSIONS` is reached
- [x] `find_active_by_device_id` has tests for `Quitting` and `Reloading` phases at SessionManager level
- [x] `handle_launch` integration tests cover `Quitting` and `Reloading` phases
- [x] `find_active_by_device_id` doc comment states positive contract
- [x] Task reference comment in `launch_context.rs` replaced with descriptive comment
- [x] Inaccurate `app_id = None` claim in task-03 notes is corrected
- [x] `cargo clippy --workspace -- -D warnings` clean
- [x] All existing tests pass

## Notes

- Review reference: `workflow/reviews/features/session-resilience-phase-4/REVIEW.md`
- Concern 1 (MAX_SESSIONS) is the highest-impact item — it prevents a reachable UX dead-end
- Concern 2 (dead code) is explicitly requested by user as DELETE, not "add a doc comment"
- Task 03 depends on task 01 because deleting `find_by_device_id` removes the cross-check assertion in `test_find_active_by_device_id_skips_stopped_session` (line 590)
