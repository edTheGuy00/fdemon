# Phase 1: Fix Dependency Violations - Task Index

## Overview

Eliminate all 6 dependency violations so module boundaries flow downward, without changing the single-crate structure. This is the prerequisite for the Cargo workspace split in Phase 3.

**Total Tasks:** 8
**Estimated Hours:** 18-26 hours

## Task Dependency Graph

```
┌──────────────────────────┐  ┌──────────────────────────┐  ┌──────────────────────────┐
│ 01-move-daemon-message   │  │ 02-move-signal-handler   │  │ 03-make-watcher-generic  │
│ (core -> daemon fix)     │  │ (common -> app fix)      │  │ (watcher -> app fix)     │
└────────────┬─────────────┘  └──────────────────────────┘  └──────────────────────────┘
             │
             │  (DaemonMessage must be in core/ first)
             ▼
┌──────────────────────────────────────────────────────────────────────────────────────┐
│ 04-move-state-types-to-app   (LogViewState, LinkHighlightState, ConfirmDialogState)  │
│ (app -> tui fix: state types)                                                        │
└────────────┬─────────────────────────────────────────────────────────────────────────┘
             │
             │  (state types must be moved first)
             ▼
┌──────────────────────────────────────────────────────────────────────────────────────┐
│ 05-move-handler-deps-from-tui   (editor, fuzzy_filter, SettingsPanel logic,          │
│                                  TargetSelectorState, GroupedBootableDevice)          │
│ (app/handler -> tui fix)                                                             │
└────────────┬─────────────────────────────────────────────────────────────────────────┘
             │
             │  (handler imports cleaned up first)
             ▼
┌──────────────────────────────────────────────────────────────────────────────────────┐
│ 06-move-process-and-actions   (process_message, handle_action, SessionTaskMap)        │
│ (headless -> tui fix)                                                                │
└────────────┬─────────────────────────────────────────────────────────────────────────┘
             │
             ▼
┌──────────────────────────────────────────────────────────────────────────────────────┐
│ 07-fix-test-imports   (update all #[cfg(test)] imports that reach into tui/)          │
└────────────┬─────────────────────────────────────────────────────────────────────────┘
             │
             ▼
┌──────────────────────────────────────────────────────────────────────────────────────┐
│ 08-verify-and-document   (verify clean deps, update ARCHITECTURE.md)                 │
└──────────────────────────────────────────────────────────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Key Modules |
|---|------|--------|------------|------------|-------------|
| 1 | [01-move-daemon-message](tasks/01-move-daemon-message.md) | Done | - | 2-3h | `core/events.rs`, `daemon/protocol.rs`, `daemon/events.rs` |
| 2 | [02-move-signal-handler](tasks/02-move-signal-handler.md) | Done | - | 1h | `common/signals.rs`, `app/signals.rs` |
| 3 | [03-make-watcher-generic](tasks/03-make-watcher-generic.md) | Done | - | 1-2h | `watcher/mod.rs` |
| 4 | [04-move-state-types-to-app](tasks/04-move-state-types-to-app.md) | Done | 1 | 4-5h | `app/session.rs`, `app/state.rs`, `tui/widgets/log_view/state.rs`, `tui/hyperlinks.rs`, `tui/widgets/confirm_dialog.rs` |
| 5 | [05-move-handler-deps-from-tui](tasks/05-move-handler-deps-from-tui.md) | Done | 4 | 4-6h | `app/handler/*.rs`, `tui/editor.rs`, `tui/widgets/settings_panel/`, `tui/widgets/new_session_dialog/` |
| 6 | [06-move-process-and-actions](tasks/06-move-process-and-actions.md) | Done | 5 | 3-4h | `tui/process.rs`, `tui/actions.rs`, `headless/runner.rs` |
| 7 | [07-fix-test-imports](tasks/07-fix-test-imports.md) | Done | 6 | 2-3h | `app/handler/tests.rs`, `app/handler/new_session/*.rs` |
| 8 | [08-verify-and-document](tasks/08-verify-and-document.md) | Done | 7 | 1-2h | `docs/ARCHITECTURE.md` |

## Success Criteria

Phase 1 is complete when:

- [x] All module imports flow downward (no upward/circular dependencies)
- [x] `cargo test` passes with no regressions
- [x] `cargo clippy` is clean
- [x] `DaemonMessage` and event structs live in `core/`, not `daemon/`
- [x] `AppState` contains no types imported from `tui/`
- [x] `Session` contains no types imported from `tui/`
- [x] `app/handler/*.rs` has no imports from `tui/`
- [x] `headless/` does not import from `tui/`
- [x] `common/` does not import from `app/`
- [x] `watcher/` does not import from `app/`
- [x] `docs/ARCHITECTURE.md` accurately reflects the new structure

## Notes

- **Tasks 1, 2, 3 are independent** and can be done in parallel (Wave 1)
- **Task 4** depends on Task 1 because `DaemonMessage` needs to already be in `core/` when we reorganize event types there
- **Tasks 5, 6, 7** are sequential -- each depends on the prior task's type relocations
- **Task 8** is the final verification pass
- All tasks operate within the single-crate structure. No Cargo.toml changes needed (Phase 3 does that).
- Re-exports at old locations may be used temporarily to reduce churn, but the authoritative definitions must move.
