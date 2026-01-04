# Phase 1: Log Filtering & Search - Task Index

## Overview

Phase 1 implements log filtering and search functionality for Flutter Demon. Users will be able to filter logs by level (Error/Warning/Info/Debug) and source (App/Daemon/Watcher), as well as perform regex-based searches with match highlighting and navigation.

**Estimated Duration:** 1-1.5 weeks  
**Total Tasks:** 7  
**Estimated Hours:** 26-32 hours

## Task Dependency Graph

```
┌─────────────────────┐     ┌─────────────────────┐
│  01-add-filter-     │     │  02-add-search-     │
│      types          │     │      types          │
└─────────┬───────────┘     └──────────┬──────────┘
          │                            │
          └──────────┬─────────────────┘
                     ▼
          ┌─────────────────────┐
          │  03-integrate-      │
          │  filter-search-     │
          │      state          │
          └─────────┬───────────┘
                    │
       ┌────────────┼────────────┐
       ▼            │            ▼
┌─────────────┐     │     ┌─────────────┐
│ 04-implement│     │     │ 05-implement│
│ -filter-    │     │     │ -search-    │
│ handlers-   │     │     │ mode        │
│ logic       │     │     └──────┬──────┘
└──────┬──────┘     │            │
       │            │            ▼
       │            │     ┌─────────────┐
       │            │     │ 06-implement│
       │            │     │ -search-    │
       │            │     │ logic-      │
       │            │     │ highlighting│
       │            │     └──────┬──────┘
       │            │            │
       └────────────┼────────────┘
                    ▼
          ┌─────────────────────┐
          │  07-add-error-      │
          │  navigation         │
          └─────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-add-filter-types](tasks/01-add-filter-types.md) | Not Started | - | 3-4h | `core/types.rs` |
| 2 | [02-add-search-types](tasks/02-add-search-types.md) | Not Started | - | 2-3h | `core/types.rs` |
| 3 | [03-integrate-filter-search-state](tasks/03-integrate-filter-search-state.md) | Not Started | 1, 2 | 3-4h | `app/session.rs`, `app/message.rs` |
| 4 | [04-implement-filter-handlers-logic](tasks/04-implement-filter-handlers-logic.md) | Not Started | 3 | 5-6h | `app/handler/keys.rs`, `app/handler/update.rs`, `tui/widgets/log_view.rs` |
| 5 | [05-implement-search-mode](tasks/05-implement-search-mode.md) | Not Started | 3 | 4-5h | `app/handler/keys.rs`, `tui/widgets/log_view.rs`, `tui/widgets/search_input.rs` |
| 6 | [06-implement-search-logic-highlighting](tasks/06-implement-search-logic-highlighting.md) | Not Started | 5 | 5-6h | `tui/widgets/log_view.rs`, `app/handler/update.rs` |
| 7 | [07-add-error-navigation](tasks/07-add-error-navigation.md) | Not Started | 4, 6 | 3-4h | `app/handler/keys.rs`, `tui/widgets/log_view.rs` |

## Success Criteria

Phase 1 is complete when:

- [ ] Log filtering by level works (All/Errors/Warnings/Info/Debug)
- [ ] Log filtering by source works (All/App/Daemon/Watcher)
- [ ] Filter indicator shown in log panel header
- [ ] Search with regex highlights matches
- [ ] Next/previous match navigation works (n/N)
- [ ] Match count displayed: "[3/47 matches]"
- [ ] Quick jump to errors (e/E) works
- [ ] All new code has unit tests
- [ ] No regressions in existing functionality

## Keyboard Shortcuts (Phase 1)

| Key | Action |
|-----|--------|
| `f` | Cycle log level filter |
| `F` | Cycle log source filter |
| `Shift+f` | Reset all filters |
| `/` | Open search prompt |
| `n` | Next search match |
| `N` | Previous search match |
| `Escape` | Clear search / close search prompt |
| `e` | Jump to next error |
| `E` | Jump to previous error |

## Notes

- Filters are applied on display, not on storage (logs are always fully preserved)
- Search uses the `regex` crate (already a transitive dependency)
- Filter and search state is per-session (each session has its own)
- Performance: Filter results should be cached when filter unchanged
- Optional: Persist filter preference to config (deferred to future enhancement)