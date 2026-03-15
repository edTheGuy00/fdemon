# Phase 2 Followup — Review Fixes - Task Index

## Overview

Address issues identified in the Phase 2 code review (`workflow/reviews/features/pre-app-custom-sources-phase-2/REVIEW.md`). One MAJOR race condition fix, two MINOR correctness/consistency fixes, one documentation fix, and two nitpick cleanups.

**Total Tasks:** 4
**Review:** [REVIEW.md](../../../reviews/features/pre-app-custom-sources-phase-2/REVIEW.md) | [ACTION_ITEMS.md](../../../reviews/features/pre-app-custom-sources-phase-2/ACTION_ITEMS.md)

## Task Dependency Graph

```
┌───────────────────────────────┐
│  01-dedup-shared-started      │  MAJOR — race condition fix
│  (independent)                │
└───────────────────────────────┘

┌───────────────────────────────┐
│  02-flush-shared-stopped      │  MINOR — consistency fix
│  (independent)                │
└───────────────────────────────┘

┌───────────────────────────────┐
│  03-guard-shared-post-app     │  MINOR — spurious action fix
│  (independent)                │
└───────────────────────────────┘

┌───────────────────────────────┐
│  04-doc-and-nitpicks          │  MINOR + NITPICK — docs & cleanup
│  (independent)                │
└───────────────────────────────┘
```

All four tasks are independent and can be dispatched in parallel.

## Tasks

| # | Task | Status | Depends On | Severity | Modules |
|---|------|--------|------------|----------|---------|
| 1 | [01-dedup-shared-started](tasks/01-dedup-shared-started.md) | Done | - | MAJOR | `handler/update.rs`, `handler/tests.rs` |
| 2 | [02-flush-shared-stopped](tasks/02-flush-shared-stopped.md) | Done | - | MINOR | `handler/update.rs`, `handler/tests.rs` |
| 3 | [03-guard-shared-post-app](tasks/03-guard-shared-post-app.md) | Done | - | MINOR | `handler/session.rs`, `handler/tests.rs` |
| 4 | [04-doc-and-nitpicks](tasks/04-doc-and-nitpicks.md) | Done | - | MINOR/NITPICK | `docs/ARCHITECTURE.md`, `config/types.rs`, `session/handle.rs` |

## Success Criteria

Followup is complete when:

- [x] Issue 1: Dedup guard prevents duplicate `SharedSourceHandle` entries
- [x] Issue 2: `SharedSourceStopped` flushes batched logs consistently
- [x] Issue 3: `has_unstarted_post_app` accounts for shared source handles
- [x] Issue 4: ARCHITECTURE.md accurately describes shared source modes
- [x] Issues 5-6: Unused helpers demoted, `CustomSourceHandle` gets `Debug`
- [x] `cargo fmt --all && cargo check --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings` passes
