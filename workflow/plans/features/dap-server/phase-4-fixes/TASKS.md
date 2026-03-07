# DAP Server Phase 4 Review Fixes — Task Index

## Overview

Address 2 critical, 6 major, and 8 minor issues identified in the Phase 4 code review. The critical issues block merge; major issues are correctness concerns that could cause debugging failures in production.

**Review:** `workflow/reviews/features/dap-server-phase-4/REVIEW.md`
**Total Tasks:** 15
**Waves:** 4 (dependency-ordered)

## Task Dependency Graph

```
Wave 1 — Critical (blocking merge)
├── 01-fix-isolate-runnable-translation
└── 02-split-adapter-mod

Wave 2 — Major correctness (depends on 02)
├── 03-tea-purity-event-forwarding          (depends on 01)
├── 04-remove-expect-panic                  (depends on 02)
├── 05-log-resume-failures                  (depends on 02)
├── 06-prune-paused-isolates                (depends on 02)
└── 07-update-breakpoint-conditions         (depends on 02)

Wave 3 — Minor correctness (depends on 02)
├── 08-defer-on-resume-clearing             (depends on 02)
└── 09-fix-all-threads-stopped              (depends on 02)

Wave 4 — Cleanup (depends on 02)
├── 10-wire-or-remove-dead-code             (depends on 02)
├── 11-clean-dead-update-actions
├── 12-move-dap-senders-to-engine           (depends on 03)
├── 13-remove-empty-globals-scope           (depends on 02)
├── 14-source-ref-reverse-index             (depends on 02)
└── 15-consolidate-mock-backends            (depends on 02)
```

## Tasks

| # | Task | Severity | Status | Depends On | Modules |
|---|------|----------|--------|------------|---------|
| 1 | [01-fix-isolate-runnable-translation](tasks/01-fix-isolate-runnable-translation.md) | Critical | Not Started | - | `app/handler/devtools/debug.rs` |
| 2 | [02-split-adapter-mod](tasks/02-split-adapter-mod.md) | Critical | Not Started | - | `dap/adapter/mod.rs` → 6 files |
| 3 | [03-tea-purity-event-forwarding](tasks/03-tea-purity-event-forwarding.md) | Major | Not Started | 01 | `app/handler/devtools/debug.rs`, `app/actions/mod.rs`, `app/engine.rs` |
| 4 | [04-remove-expect-panic](tasks/04-remove-expect-panic.md) | Major | Not Started | 02 | `dap/adapter/handlers.rs` |
| 5 | [05-log-resume-failures](tasks/05-log-resume-failures.md) | Major | Not Started | 02 | `dap/adapter/events.rs` |
| 6 | [06-prune-paused-isolates](tasks/06-prune-paused-isolates.md) | Major | Not Started | 02 | `dap/adapter/events.rs` |
| 7 | [07-update-breakpoint-conditions](tasks/07-update-breakpoint-conditions.md) | Major | Not Started | 02 | `dap/adapter/handlers.rs` |
| 8 | [08-defer-on-resume-clearing](tasks/08-defer-on-resume-clearing.md) | Minor | Not Started | 02 | `dap/adapter/handlers.rs` |
| 9 | [09-fix-all-threads-stopped](tasks/09-fix-all-threads-stopped.md) | Minor | Not Started | 02 | `dap/adapter/events.rs` |
| 10 | [10-wire-or-remove-dead-code](tasks/10-wire-or-remove-dead-code.md) | Minor | Not Started | 02 | `dap/adapter/types.rs`, `app/handler/dap_backend.rs` |
| 11 | [11-clean-dead-update-actions](tasks/11-clean-dead-update-actions.md) | Minor | Not Started | - | `app/actions/mod.rs`, `app/message.rs` |
| 12 | [12-move-dap-senders-to-engine](tasks/12-move-dap-senders-to-engine.md) | Minor | Not Started | 03 | `app/state.rs`, `app/engine.rs`, `app/handler/devtools/debug.rs` |
| 13 | [13-remove-empty-globals-scope](tasks/13-remove-empty-globals-scope.md) | Minor | Not Started | 02 | `dap/adapter/variables.rs` |
| 14 | [14-source-ref-reverse-index](tasks/14-source-ref-reverse-index.md) | Minor | Not Started | 02 | `dap/adapter/stack.rs` |
| 15 | [15-consolidate-mock-backends](tasks/15-consolidate-mock-backends.md) | Minor | Not Started | 02 | `dap/adapter/` test modules |

## Success Criteria

Phase 4 fixes are complete when:

- [ ] `IsolateRunnable` events forwarded correctly — breakpoints re-apply after hot restart
- [ ] No file in `crates/fdemon-dap/src/adapter/` exceeds 800 lines (excluding tests)
- [ ] No `expect()` in library code paths
- [ ] No silent `resume()` failures — all logged at `warn!`
- [ ] TEA `update()` is side-effect-free — channel sends moved to `handle_action()`
- [ ] `cargo fmt --all` — Pass
- [ ] `cargo check --workspace` — Pass
- [ ] `cargo test --workspace` — Pass (all existing + new tests green)
- [ ] `cargo clippy --workspace -- -D warnings` — Pass

## Notes

- Wave 1 tasks are independent and can run in parallel
- Wave 2 tasks 04–07 all touch adapter submodules created by task 02; they can run in parallel with each other
- Task 03 depends on task 01 because the IsolateRunnable fix must be correct before refactoring the forwarding path
- Task 12 depends on task 03 because moving senders to Engine requires the UpdateAction-based forwarding to be in place first
- Task 15 (mock consolidation) is the largest cleanup task; consider a `mock_backend!` macro approach
