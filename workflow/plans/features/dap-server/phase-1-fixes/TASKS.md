# Phase 1 Fixes: Review Follow-up — Task Index

## Overview

Address all issues identified in the [Phase 1 code review](../../reviews/features/dap-server-phase-1/REVIEW.md). One critical bug (serde flatten), five major issues, and several minor cleanups.

**Total Tasks:** 5
**Source:** [ACTION_ITEMS.md](../../../reviews/features/dap-server-phase-1/ACTION_ITEMS.md)

## Task Dependency Graph

```
Wave 1 (parallel — no dependencies):
┌──────────────────────────────────────┐
│  01-fix-serde-flatten-bug            │
│  (CRITICAL: debugger_types.rs,       │
│   vm_service.rs, integration test)   │
└──────────────────┬───────────────────┘
                   │
                   │  ┌──────────────────────────────────────┐
                   │  │  04-handler-module-fixes              │
                   │  │  (devtools/mod.rs, debug.rs)          │
                   │  └──────────────────────────────────────┘
                   │
                   │  ┌──────────────────────────────────────┐
                   │  │  05-minor-cleanups                    │
                   │  │  (debug_state.rs, debugger.rs,        │
                   │  │   actions/mod.rs)                     │
                   │  └──────────────────────────────────────┘
                   │
Wave 2 (after 01):
                   │
          ┌────────┴────────────────────────────┐
          ▼                                     ▼
┌──────────────────────────────┐  ┌──────────────────────────────┐
│  02-parse-failure-logging    │  │  03-fix-service-extension    │
│  (vm_service.rs imports +    │  │  (debugger_types.rs          │
│   logging on None branches)  │  │   ServiceExtensionAdded)     │
│  depends: 01                 │  │  depends: 01                 │
└──────────────────────────────┘  └──────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Review Issues | Modules |
|---|------|--------|------------|---------------|---------|
| 1 | [01-fix-serde-flatten-bug](tasks/01-fix-serde-flatten-bug.md) | Done | - | #1 (critical), #7, #13 | `debugger_types.rs`, `vm_service.rs` |
| 2 | [02-parse-failure-logging](tasks/02-parse-failure-logging.md) | Done | 1 | #2, #3 | `vm_service.rs` |
| 3 | [03-fix-service-extension](tasks/03-fix-service-extension.md) | Done | 1 | #5 | `debugger_types.rs` |
| 4 | [04-handler-module-fixes](tasks/04-handler-module-fixes.md) | Done | - | #4, #6, #8 | `devtools/mod.rs`, `devtools/debug.rs` |
| 5 | [05-minor-cleanups](tasks/05-minor-cleanups.md) | Done | - | #9, #11, #12 | `debug_state.rs`, `debugger.rs`, `actions/mod.rs` |

## Success Criteria

Phase 1 fixes are complete when:

- [x] Critical serde flatten bug is fixed — `parse_debug_event` returns `Some(DebugEvent)` when called with a `StreamEvent` deserialized from real VM Service JSON
- [x] Integration test added exercising the full `StreamEvent` deserialization → parse path
- [x] All major issues (#2–#6) resolved
- [x] All minor issues (#7–#13) resolved
- [x] `cargo fmt --all` passes
- [x] `cargo check --workspace` passes
- [x] `cargo test --workspace` passes (no regressions)
- [x] `cargo clippy --workspace -- -D warnings` passes

## Notes

- Task 01 is the **critical blocker** — it changes function signatures in `debugger_types.rs` that Tasks 02 and 03 depend on
- Tasks 04 and 05 are independent of 01 and can run in Wave 1
- Issue #7 (clone-per-event) is resolved as a side-effect of Task 01 (switching to `&StreamEvent` eliminates `parse_isolate_ref` entirely)
- Issue #10 (no-op breakpoint comments) is bundled into Task 04
