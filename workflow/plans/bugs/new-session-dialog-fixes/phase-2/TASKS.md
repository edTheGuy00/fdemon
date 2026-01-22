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
| 1 | [01-compact-borders-titles](tasks/01-compact-borders-titles.md) | Not Started | - | `target_selector.rs`, `launch_context.rs` |
| 2 | [02-responsive-mode-labels](tasks/02-responsive-mode-labels.md) | Not Started | - | `launch_context.rs` |

## Success Criteria

Phase 2 is complete when:

- [ ] Portrait layout shows "Target Selector" title with border
- [ ] Portrait layout shows "Launch Context" title with border
- [ ] Mode buttons show full labels ("Debug", "Profile", "Release") when width >= 45
- [ ] Mode buttons show abbreviated labels ("Dbg", "Prof", "Rel") when width < 45
- [ ] No visual regression in horizontal layout
- [ ] Borders don't consume excessive vertical space
- [ ] All new code has unit tests
- [ ] `cargo test` passes
- [ ] `cargo clippy` passes

## Notes

- Portrait layout triggers at width 40-69 columns (horizontal at >= 70)
- Dialog uses 90% of terminal width, so 50-column terminal = 45-column dialog
- Current compact mode intentionally omits borders to save space - need to find balance
- Consider using minimal borders (PLAIN style or top-only) to minimize vertical impact
- The `area.width` parameter is available in render methods but not currently used for adaptive rendering
