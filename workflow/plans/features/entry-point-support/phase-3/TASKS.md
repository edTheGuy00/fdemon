# Phase 3: Entry Point UI Support - Task Index

## Overview

Add UI support for entry point selection in the NewSessionDialog Launch Context pane. Users can select from discovered entry points via fuzzy modal, with auto-save support for FDemon configurations.

**Total Tasks:** 11

## Task Dependency Graph

```
┌───────────────────────┐     ┌───────────────────────┐
│  01-add-entry-point-  │     │  02-add-entry-point-  │
│  to-field-enum        │     │  to-fuzzy-modal-type  │
└───────────┬───────────┘     └───────────┬───────────┘
            │                             │
            └──────────┬──────────────────┘
                       │
                       ▼
            ┌───────────────────────┐
            │  03-add-state-helper- │
            │  methods              │
            └───────────┬───────────┘
                        │
            ┌───────────┴───────────┐
            │                       │
            ▼                       ▼
┌───────────────────────┐  ┌───────────────────────┐
│  04-add-render-       │  │  06-add-field-        │
│  entry-point-field    │  │  activation-handler   │
└───────────┬───────────┘  └───────────┬───────────┘
            │                          │
            ▼                          ▼
┌───────────────────────┐  ┌───────────────────────┐
│  05-update-widget-    │  │  07-add-entry-point-  │
│  layout               │  │  selected-handler     │
└───────────────────────┘  └───────────────────────┘

--- Review Follow-up Tasks ---

┌───────────────────────┐     ┌───────────────────────┐
│  08-add-file-size-    │     │  11-reorder-          │
│  guard                │     │  editability-check    │
└───────────┬───────────┘     └───────────────────────┘
            │                 (independent, low priority)
            ▼
┌───────────────────────┐
│  09-move-discovery-   │
│  to-async             │
└───────────┬───────────┘
            │
            ▼
┌───────────────────────┐
│  10-add-loading-      │
│  indicator            │
└───────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 1 | [01-add-entry-point-to-field-enum](tasks/01-add-entry-point-to-field-enum.md) | Done | - | `types.rs` |
| 2 | [02-add-entry-point-to-fuzzy-modal-type](tasks/02-add-entry-point-to-fuzzy-modal-type.md) | Done | - | `types.rs` |
| 3 | [03-add-state-helper-methods](tasks/03-add-state-helper-methods.md) | Done | 1, 2 | `state.rs` |
| 4 | [04-add-render-entry-point-field](tasks/04-add-render-entry-point-field.md) | Done | 3 | `tui/widgets/.../launch_context.rs` |
| 5 | [05-update-widget-layout](tasks/05-update-widget-layout.md) | Done | 4 | `tui/widgets/.../launch_context.rs` |
| 6 | [06-add-field-activation-handler](tasks/06-add-field-activation-handler.md) | Done | 3 | `app/handler/new_session/launch_context.rs` |
| 7 | [07-add-entry-point-selected-handler](tasks/07-add-entry-point-selected-handler.md) | Done | 6 | `app/handler/new_session/launch_context.rs` |

### Review Follow-up Tasks

| # | Task | Status | Depends On | Priority | Modules |
|---|------|--------|------------|----------|---------|
| 8 | [08-add-file-size-guard](tasks/08-add-file-size-guard.md) | Not Started | - | Medium | `core/discovery.rs` |
| 9 | [09-move-discovery-to-async](tasks/09-move-discovery-to-async.md) | Not Started | 8 | High | `handler/mod.rs`, `message.rs`, `spawn.rs`, `actions.rs`, `fuzzy_modal.rs` |
| 10 | [10-add-loading-indicator](tasks/10-add-loading-indicator.md) | Not Started | 9 | Medium | `tui/widgets/.../fuzzy_modal.rs` |
| 11 | [11-reorder-editability-check](tasks/11-reorder-editability-check.md) | Not Started | - | Low | `handler/new_session/launch_context.rs` |

## Success Criteria

Phase 3 is complete when:

- [x] `LaunchContextField::EntryPoint` variant added
- [x] `next()` and `prev()` navigation updated for new field
- [x] `FuzzyModalType::EntryPoint` variant added with `allows_custom() = true`
- [x] `LaunchContextState.available_entry_points` field added
- [x] `entry_point_display()` method returns "(default)" or path
- [x] `is_entry_point_editable()` method respects config source
- [x] Entry Point field renders in Launch Context pane
- [x] Field shows "(from config)" suffix for VSCode configs
- [x] Enter key opens fuzzy modal with discovered entry points
- [x] Modal includes "(default)" option to clear selection
- [x] Selection updates `LaunchContextState.entry_point`
- [x] FDemon configs trigger auto-save on selection
- [x] VSCode configs show entry point as read-only
- [x] Compact layout handles new field gracefully
- [x] All unit tests pass
- [x] `cargo clippy` passes with no warnings

### Review Follow-up Criteria (Tasks 8-11)

- [ ] Large files (>1MB) skipped during discovery
- [ ] Entry point discovery runs asynchronously
- [ ] UI remains responsive during discovery on large projects
- [ ] Loading indicator shown while discovering
- [ ] Editability check precedes selection parsing

## Verification Commands

```bash
cargo test --lib new_session_dialog
cargo test --lib launch_context
cargo test --lib entry_point
cargo clippy -- -D warnings
```

## Notes

- Tasks 1 and 2 can be done in parallel (both modify types.rs but different enums)
- Tasks 4-5 (UI) and 6-7 (handlers) can be done in parallel after Task 3
- Follows existing patterns from Flavor field implementation
- Entry point field placed between Flavor and DartDefines in the UI
- Phase 2's `discover_entry_points()` is used in Task 6 to populate the modal

## Parallelization Strategy

```
Wave 1: Tasks 1, 2 (parallel - different enums)
Wave 2: Task 3 (depends on 1, 2)
Wave 3: Tasks 4, 6 (parallel - different modules)
Wave 4: Tasks 5, 7 (parallel - finalize UI and handlers)

--- Review Follow-up ---
Wave 5: Tasks 8, 11 (parallel - independent changes)
Wave 6: Task 9 (depends on 8)
Wave 7: Task 10 (depends on 9)
```

---

## Review Follow-up Summary

These tasks address issues identified during Phase 3 code review (see `workflow/reviews/features/entry-point-support-phase-3/`).

### Issue Sources

| Task | Source | Priority | Issue |
|------|--------|----------|-------|
| 08 | Risks Analyzer | Medium | No file size limit when reading Dart files |
| 09 | Risks Analyzer | High | Blocking I/O in TEA update handler |
| 10 | Risks Analyzer | Medium | No user feedback during discovery |
| 11 | Logic Checker | Low | Selection parsed before editability check |

### Deferred to Backlog

The following issues were identified but deferred as low priority:

1. **Cache entry points with invalidation** - Discovery runs on every modal open; could cache with file watcher invalidation
2. **Parallel file discovery** - Files scanned sequentially; could use rayon for concurrency
3. **Streaming regex search** - Entire file read into memory; could use streaming reader

These optimizations would improve performance on very large projects but are not blocking issues.
