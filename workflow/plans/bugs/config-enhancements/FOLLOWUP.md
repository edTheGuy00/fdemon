# Config Enhancements - Review Follow-up Tasks

## Overview

Follow-up items from the [code review](../../../reviews/bugs/config-enhancements/REVIEW.md) of the config-enhancements implementation (tasks 01-05). No critical issues; 3 must-fix items blocking merge, 2 recommended fixes, and 3 optional improvements.

**Total Tasks:** 6
**Source:** Review verdict "Approved with Concerns" — 5 agent reviewers, 0 critical, 9 items total

## Task Dependency Graph

```
Wave 1 (must fix before merge — all independent):
┌────────────────────────────────────┐
│  06-review-code-quality-fixes      │
└────────────────────────────────────┘

Wave 2 (recommended — all independent):
┌────────────────────────────────┐   ┌────────────────────────────────┐
│  07-empty-watcher-paths-warning│   │  08-testing-md-path-fixes      │
└────────────────────────────────┘   └────────────────────────────────┘

Wave 3 (optional improvements — all independent):
┌──────────────────────────────┐  ┌──────────────────────────────┐  ┌──────────────────────────────┐
│  09-extract-startup-dispatch │  │  10-cap-pending-watcher-errs │  │  11-doc-sync-process-message │
└──────────────────────────────┘  └──────────────────────────────┘  └──────────────────────────────┘
```

## Tasks

| # | Task | Priority | Status | Depends On | Modules |
|---|------|----------|--------|------------|---------|
| 6 | [06-review-code-quality-fixes](tasks/06-review-code-quality-fixes.md) | Must Fix | Done | - | `startup.rs`, `watcher/mod.rs` |
| 7 | [07-empty-watcher-paths-warning](tasks/07-empty-watcher-paths-warning.md) | Should Fix | Done | - | `watcher/mod.rs` |
| 8 | [08-testing-md-path-fixes](tasks/08-testing-md-path-fixes.md) | Should Fix | Done | - | `example/TESTING.md`, `example/app4/.fdemon/config.toml` |
| 9 | [09-extract-startup-dispatch](tasks/09-extract-startup-dispatch.md) | Consider | Done | - | `runner.rs`, `startup.rs` |
| 10 | [10-cap-pending-watcher-errors](tasks/10-cap-pending-watcher-errors.md) | Consider | Done | - | `state.rs`, `handler/update.rs` |
| 11 | [11-doc-sync-process-message](tasks/11-doc-sync-process-message.md) | Consider | Done | - | `runner.rs` |

## Success Criteria

Follow-up is complete when:

- [x] All "Must Fix" items resolved (task 06)
- [x] All "Should Fix" items resolved (tasks 07-08)
- [x] `cargo test --workspace` passes (4 pre-existing snapshot failures unrelated to these changes)
- [x] `cargo clippy --workspace -- -D warnings` clean

## Notes

- Wave 1 (task 06) blocks the merge PR — all 3 items are trivial single-line fixes
- Wave 2 tasks are independent and can be parallelized
- Wave 3 tasks are optional improvements flagged for tracking; can be deferred to a later PR
- The "Consider" tasks address tech debt identified during review, not regressions
