# Phase 3: Entry Point UI Support - Task Index

## Overview

Add UI support for entry point selection in the NewSessionDialog Launch Context pane. Users can select from discovered entry points via fuzzy modal, with auto-save support for FDemon configurations.

**Total Tasks:** 7

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
```

## Tasks

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 1 | [01-add-entry-point-to-field-enum](tasks/01-add-entry-point-to-field-enum.md) | Open | - | `types.rs` |
| 2 | [02-add-entry-point-to-fuzzy-modal-type](tasks/02-add-entry-point-to-fuzzy-modal-type.md) | Open | - | `types.rs` |
| 3 | [03-add-state-helper-methods](tasks/03-add-state-helper-methods.md) | Open | 1, 2 | `state.rs` |
| 4 | [04-add-render-entry-point-field](tasks/04-add-render-entry-point-field.md) | Open | 3 | `tui/widgets/.../launch_context.rs` |
| 5 | [05-update-widget-layout](tasks/05-update-widget-layout.md) | Open | 4 | `tui/widgets/.../launch_context.rs` |
| 6 | [06-add-field-activation-handler](tasks/06-add-field-activation-handler.md) | Open | 3 | `app/handler/new_session/launch_context.rs` |
| 7 | [07-add-entry-point-selected-handler](tasks/07-add-entry-point-selected-handler.md) | Open | 6 | `app/handler/new_session/launch_context.rs` |

## Success Criteria

Phase 3 is complete when:

- [ ] `LaunchContextField::EntryPoint` variant added
- [ ] `next()` and `prev()` navigation updated for new field
- [ ] `FuzzyModalType::EntryPoint` variant added with `allows_custom() = true`
- [ ] `LaunchContextState.available_entry_points` field added
- [ ] `entry_point_display()` method returns "(default)" or path
- [ ] `is_entry_point_editable()` method respects config source
- [ ] Entry Point field renders in Launch Context pane
- [ ] Field shows "(from config)" suffix for VSCode configs
- [ ] Enter key opens fuzzy modal with discovered entry points
- [ ] Modal includes "(default)" option to clear selection
- [ ] Selection updates `LaunchContextState.entry_point`
- [ ] FDemon configs trigger auto-save on selection
- [ ] VSCode configs show entry point as read-only
- [ ] Compact layout handles new field gracefully
- [ ] All unit tests pass
- [ ] `cargo clippy` passes with no warnings

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
```
