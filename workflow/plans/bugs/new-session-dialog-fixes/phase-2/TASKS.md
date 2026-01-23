# Phase 2: Portrait Layout Styling Fixes - Task Index

## Overview

Fix visual inconsistencies in portrait layout mode. Add section titles and borders to match horizontal layout styling, and implement responsive mode labels that show full text when space allows.

**Total Tasks:** 2
**Bugs Addressed:** Bug 3 (Missing borders/titles), Bug 4 (Abbreviated mode labels)

## Task Dependency Graph

```
┌─────────────────────────────────┐
│  01-compact-borders-titles      │  (Bug 3)
│  Add borders to compact mode    │
└─────────────────────────────────┘

┌─────────────────────────────────┐
│  02-responsive-mode-labels      │  (Bug 4)
│  Full labels when width allows  │
└─────────────────────────────────┘

(Tasks are independent - can run in parallel)
```

## Tasks

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 1 | [01-compact-borders-titles](tasks/01-compact-borders-titles.md) | Done | - | `target_selector.rs`, `launch_context.rs` |
| 2 | [02-responsive-mode-labels](tasks/02-responsive-mode-labels.md) | Done | - | `launch_context.rs` |

## Success Criteria

Phase 2 is complete when:

- [x] Portrait layout shows "Target Selector" title with border
- [x] Portrait layout shows "Launch Context" title with border
- [x] Mode buttons show full labels ("Debug", "Profile", "Release") when width >= 48
- [x] Mode buttons show abbreviated labels ("Dbg", "Prof", "Rel") when width < 48
- [x] No visual regression in horizontal layout
- [x] Borders don't consume excessive vertical space
- [x] All new code has unit tests
- [x] `cargo test` passes
- [x] `cargo clippy` passes

## Notes

- Portrait layout triggers at width 40-69 columns (horizontal at >= 70)
- Dialog uses 90% of terminal width, so 50-column terminal = 45-column dialog
- Current compact mode intentionally omits borders to save space - need to find balance
- Consider using minimal borders (PLAIN style or top-only) to minimize vertical impact
- The `area.width` parameter is available in render methods but not currently used for adaptive rendering
