# Phase 4 Fixes: Network Monitor Review Issues

## Overview

Address all issues identified in the [Phase 4 Review](../../../../reviews/features/devtools-v2-phase-4/REVIEW.md). Four blocking issues, four major issues, and several minor issues across all 4 crates.

**Total Tasks:** 7
**Waves:** 4

## Task Dependency Graph

```
Wave 1 (parallel — independent fixes across crates)
┌──────────────────────────────┐  ┌──────────────────────────────────┐
│ 01-fix-duplicate-polling     │  │ 02-fix-recording-toggle          │
│ (fdemon-app handler)         │  │ (fdemon-app handler)             │
└──────────────┬───────────────┘  └──────────────┬───────────────────┘
               │                                  │
┌──────────────┴───────────────┐  ┌──────────────┴───────────────────┐
│ 03-fix-session-close-leak    │  │ 04-fix-truncate-utf8-panic       │
│ (fdemon-app handler)         │  │ (fdemon-tui widget)              │
└──────────────┬───────────────┘  └──────────────┬───────────────────┘
               │                                  │
Wave 2 (parallel — independent fixes)             │
               │    ┌─────────────────────────────┘
               │    │
┌──────────────┴────┴──────────┐  ┌──────────────────────────────────┐
│ 05-fix-major-issues          │  │ 06-fix-minor-issues              │
│ (color, hydration, index)    │  │ (bool, VecDeque, alloc, etc.)    │
│ depends: none                │  │ depends: none                    │
└──────────────┬───────────────┘  └──────────────┬───────────────────┘
               │                                  │
Wave 3         │                                  │
               │    ┌─────────────────────────────┘
               ▼    ▼
┌──────────────────────────────────────────────────┐
│ 07-narrow-layout-vertical-split                  │
│ (fdemon-tui widget — larger UX change)           │
│ depends: 04, 05                                  │
└──────────────────────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Crate | Severity |
|---|------|--------|------------|-------|----------|
| 1 | [01-fix-duplicate-polling-tasks](tasks/01-fix-duplicate-polling-tasks.md) | Done | - | `fdemon-app` | CRITICAL |
| 2 | [02-fix-recording-toggle](tasks/02-fix-recording-toggle.md) | Done | - | `fdemon-app` | CRITICAL |
| 3 | [03-fix-session-close-leak](tasks/03-fix-session-close-leak.md) | Done | - | `fdemon-app` | HIGH |
| 4 | [04-fix-truncate-utf8-panic](tasks/04-fix-truncate-utf8-panic.md) | Done | - | `fdemon-tui` | HIGH |
| 5 | [05-fix-major-issues](tasks/05-fix-major-issues.md) | Done | - | multi-crate | MAJOR |
| 6 | [06-fix-minor-issues](tasks/06-fix-minor-issues.md) | Done | - | multi-crate | MINOR |
| 7 | [07-narrow-layout-vertical-split](tasks/07-narrow-layout-vertical-split.md) | Done | 4, 5 | `fdemon-tui` | MAJOR (UX) |

## Dispatch Plan

**Wave 1** (parallel — all four blocking fixes are independent):
- Task 01: Fix duplicate polling tasks (handler guard + defensive signal)
- Task 02: Fix recording toggle (TEA-side guard in handler)
- Task 03: Fix session close network task leak (add cleanup parity)
- Task 04: Fix truncate UTF-8 panic (use existing `truncate_str`)

**Wave 2** (parallel — independent bundles):
- Task 05: Fix major issues (color consistency, hydration failure, selected_index semantics)
- Task 06: Fix minor issues (bool-as-string, VecDeque, filtered_count alloc, magic numbers, type alias, NetworkDetailTab location, Clear widget, duplicate VM Service code)

**Wave 3** (depends on widget fixes being stable):
- Task 07: Narrow layout vertical split (UX overhaul for narrow terminals)

## Success Criteria

Phase 4 fixes are complete when:

- [ ] No duplicate polling tasks on repeated panel switches (`n` → `i` → `n`)
- [ ] Recording toggle actually pauses/resumes data collection
- [ ] Session close cleans up network monitoring task (no zombies)
- [ ] `truncate()` handles multi-byte UTF-8 without panicking
- [ ] HTTP method colors are consistent between table and details
- [ ] `FetchHttpRequestDetail` hydration failure clears loading state
- [ ] `selected_index` is correct when filters are active during eviction
- [ ] Narrow terminals show vertical split (table top, details bottom)
- [ ] Boolean parameters sent as JSON booleans to VM Service
- [ ] Entries stored in `VecDeque` for O(1) eviction
- [ ] `filtered_count()` uses iterator `.count()` (no allocation)
- [ ] Magic numbers replaced with named constants
- [ ] All existing tests pass (no regressions)
- [ ] `cargo fmt && cargo check && cargo test && cargo clippy` clean
