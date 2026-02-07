# Post-Restructure Cleanup - Task Index

## Overview

Address all actionable findings from the workspace restructure phase 3 code review. 3 major fixes (Wave 1) and 4 minor improvements (Wave 2).

**Total Tasks:** 7
**Review Reference:** `workflow/reviews/features/workspace-restructure-phase-3/REVIEW.md`

## Task Dependency Graph

```
Wave 1 (parallel):
┌─────────────────────────┐  ┌─────────────────────────┐  ┌─────────────────────────┐
│ 01-move-parse-to-daemon │  │ 02-fix-headless-log-dup │  │ 03-replace-eprintln     │
└────────────┬────────────┘  └─────────────────────────┘  └─────────────────────────┘
             │
             ▼
Wave 2 (parallel, 06 depends on 01):
┌─────────────────────────┐  ┌─────────────────────────┐  ┌─────────────────────────┐  ┌─────────────────────────┐
│ 04-consolidate-spawn    │  │ 05-fix-try-lock-race    │  │ 06-standardize-imports  │  │ 07-abstract-key-event   │
└─────────────────────────┘  └─────────────────────────┘  └─────────────────────────┘  └─────────────────────────┘
```

## Tasks

| # | Task | Review Issue | Severity | Status | Depends On | Modules |
|---|------|-------------|----------|--------|------------|---------|
| 1 | [01-move-parse-to-daemon](tasks/01-move-parse-to-daemon.md) | #1 | MAJOR | Not Started | - | `fdemon-core/events.rs`, `fdemon-daemon/protocol.rs` |
| 2 | [02-fix-headless-log-dup](tasks/02-fix-headless-log-dup.md) | #2 | MAJOR | Not Started | - | `src/headless/runner.rs` |
| 3 | [03-replace-eprintln](tasks/03-replace-eprintln.md) | #3 | MAJOR | Not Started | - | `src/headless/mod.rs` |
| 4 | [04-consolidate-spawn](tasks/04-consolidate-spawn.md) | #6 | MINOR | Not Started | - | `fdemon-daemon/process.rs` |
| 5 | [05-fix-try-lock-race](tasks/05-fix-try-lock-race.md) | #7 | MINOR | Not Started | - | `fdemon-app/actions.rs` |
| 6 | [06-standardize-imports](tasks/06-standardize-imports.md) | #8, #10 | MINOR | Not Started | 01 | Multiple files |
| 7 | [07-abstract-key-event](tasks/07-abstract-key-event.md) | #4 | MINOR | Not Started | - | `fdemon-app/message.rs`, `fdemon-app/handler/keys.rs`, `fdemon-tui/event.rs` |

## Accepted / No Action

| Review Issue | Title | Reason |
|-------------|-------|--------|
| #5 | `view()` takes `&mut AppState` | Framework limitation (ratatui `StatefulWidget`). Documented. |
| #9 | Large files exceeding 500-line standard | Pre-existing tech debt, not introduced by restructure. Track separately. |

## Success Criteria

Post-restructure cleanup is complete when:

- [ ] All 7 tasks are Done
- [ ] `cargo fmt --all` passes
- [ ] `cargo check --workspace` passes
- [ ] `cargo test --workspace --lib` passes (1,532+ tests, 0 failures)
- [ ] `cargo clippy --workspace --lib -- -D warnings` passes (0 warnings)
- [ ] No regressions in TUI or headless mode

## Notes

- Wave 1 tasks (01-03) are the review's "should fix before merge" items
- Wave 2 tasks (04-07) are "track for future" items batched for efficiency
- Task 06 depends on task 01 because import standardization should happen after parse logic moves
- Task 07 is the largest minor task (~895 lines of key handler refactoring)
