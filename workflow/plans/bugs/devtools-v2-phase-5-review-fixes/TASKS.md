# DevTools v2 Phase 5 Review Fixes — Task Index

## Overview

Fix 3 blocking bugs, 2 major issues, and 4 minor quality issues found during Phase 5 code review.

**Total Tasks:** 6
**Source:** [REVIEW.md](../../../reviews/features/devtools-v2-phase-5/REVIEW.md), [ACTION_ITEMS.md](../../../reviews/features/devtools-v2-phase-5/ACTION_ITEMS.md)

## Task Dependency Graph

```
Wave 1 (all independent — can dispatch in parallel)
┌──────────────────────────────┐
│  01-fix-settings-item-count  │
├──────────────────────────────┤
│  02-fix-default-panel-options│
├──────────────────────────────┤
│  03-fix-filter-bar-cursor    │
├──────────────────────────────┤
│  04-deduplicate-session-api  │
├──────────────────────────────┤
│  05-fix-network-state-reset  │
└──────────────────────────────┘

Wave 2 (after Wave 1)
┌──────────────────────────────┐
│  06-minor-quality-fixes      │
└──────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Crate(s) | Severity |
|---|------|--------|------------|----------|----------|
| 1 | [01-fix-settings-item-count](tasks/01-fix-settings-item-count.md) | Done | - | fdemon-app | CRITICAL |
| 2 | [02-fix-default-panel-options](tasks/02-fix-default-panel-options.md) | Done | - | fdemon-app | HIGH |
| 3 | [03-fix-filter-bar-cursor](tasks/03-fix-filter-bar-cursor.md) | Done | - | fdemon-tui | MAJOR |
| 4 | [04-deduplicate-session-api](tasks/04-deduplicate-session-api.md) | Done | - | fdemon-app | MEDIUM |
| 5 | [05-fix-network-state-reset](tasks/05-fix-network-state-reset.md) | Done | - | fdemon-app | MEDIUM |
| 6 | [06-minor-quality-fixes](tasks/06-minor-quality-fixes.md) | Done | 1-5 | fdemon-core, fdemon-app, fdemon-tui | LOW |

## Success Criteria

Phase 5 review fixes are complete when:

- [ ] All 27 Project tab settings items reachable via keyboard navigation
- [ ] Dynamic tabs (LaunchConfig, VSCodeConfig) navigate correct item counts
- [ ] `default_panel` shows inspector/performance/network — no "layout"
- [ ] Filter bar cursor at correct column for ASCII and multi-byte input
- [ ] No duplicated insertion blocks in session_manager.rs
- [ ] `NetworkState::reset()` preserves `recording` config
- [ ] Minor quality issues (visibility, sort helper, cell loops) resolved
- [ ] `cargo fmt --all && cargo check --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings` passes

## Notes

- Wave 1 tasks are all independent and safe to dispatch simultaneously
- Task 01 is the most complex — requires changing `get_item_count_for_tab()` signature to accept more state
- Task 04 has ~150 test call sites for `create_session` — keep that method's signature unchanged
- Task 06 batches 4 minor issues that are individually trivial
