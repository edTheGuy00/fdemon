# Phase 8: Integration & Cleanup - Task Index

## Overview

Final integration phase: wire up the NewSessionDialog to the app, remove old dialogs (DeviceSelector, StartupDialog), update tests, and ensure everything works together.

**Total Tasks:** 5
**Estimated Time:** 3 hours

## Prerequisites

**Depends on:** Phase 6.1 (File Splitting Refactoring), Phase 7

Phase 6.1 restructures the handler files that are modified/removed in this phase:
- `app/handler/update.rs` → `app/handler/` modules (including `startup_dialog.rs`, `device_selector.rs`)
- `new_session_dialog/state.rs` → `new_session_dialog/state/` module

## Task Dependency Graph

```
┌─────────────────────────────────────┐
│  01-ui-mode-integration             │
└────────────────┬────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────┐
│  02-startup-flow                    │
└────────────────┬────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────┐
│  03-remove-old-dialogs              │
└────────────────┬────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────┐
│  04-update-tests                    │
└────────────────┬────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────┐
│  05-documentation                   │
└─────────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. | Modules |
|---|------|--------|------------|------|---------|
| 1 | [01-ui-mode-integration](tasks/01-ui-mode-integration.md) | Done | Phase 7 | 35m | `app/state.rs`, `tui/render/mod.rs` |
| 2 | [02-startup-flow](tasks/02-startup-flow.md) | Done | 1 | 30m | `app/handler/session.rs`, `main.rs` |
| 3 | [03-remove-old-dialogs](tasks/03-remove-old-dialogs.md) | Done | 2 | 35m | `app/handler/startup_dialog.rs`, `app/handler/device_selector.rs`, widgets |
| 4 | [04-update-tests](tasks/04-update-tests.md) | Done | 3 | 40m | Test files |
| 5 | [05-documentation](tasks/05-documentation.md) | Done | 4 | 20m | `docs/KEYBINDINGS.md` |

---

## Review Follow-up Tasks

Based on the [Phase 8 Review](../../../../reviews/features/new-session-dialog-phase-8/REVIEW.md), the following critical and major issues must be addressed:

| # | Task | Status | Depends On | Est. | Modules | Priority |
|---|------|--------|------------|------|---------|----------|
| 6 | [06-fix-key-handlers](tasks/06-fix-key-handlers.md) | Not Started | 5 | 15m | `app/handler/keys.rs` | 🔴 Critical |
| 7 | [07-remove-deprecated-messages](tasks/07-remove-deprecated-messages.md) | Not Started | 6 | 20m | `app/message.rs` | 🔴 Critical |
| 8 | [08-remove-deprecated-handlers](tasks/08-remove-deprecated-handlers.md) | Not Started | 7 | 15m | `app/handler/update.rs` | 🔴 Critical |
| 9 | [09-fix-handler-tests](tasks/09-fix-handler-tests.md) | Not Started | 8 | 25m | `app/handler/tests.rs`, `app/handler/keys.rs` | 🟠 Major |
| 10 | [10-fix-render-tests](tasks/10-fix-render-tests.md) | Not Started | 8 | 30m | `tui/render/tests.rs` | 🟠 Major |
| 11 | [11-update-e2e-snapshots](tasks/11-update-e2e-snapshots.md) | Not Started | 10 | 20m | `tests/e2e/` | 🟠 Major |
| 12 | [12-minor-cleanup](tasks/12-minor-cleanup.md) | Not Started | 11 | 25m | Various | 🟡 Minor |

### Follow-up Dependency Graph

```
┌─────────────────────────────────────┐
│  06-fix-key-handlers               │  ← Critical: '+' and 'd' keys broken
└────────────────┬────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────┐
│  07-remove-deprecated-messages     │  ← Critical: Clean message.rs
└────────────────┬────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────┐
│  08-remove-deprecated-handlers     │  ← Critical: Clean update.rs
└────────────────┬────────────────────┘
                 │
         ┌───────┴───────┐
         ▼               ▼
┌─────────────────┐  ┌─────────────────┐
│09-fix-handler   │  │10-fix-render    │  ← Major: Fix test compilation
│    tests        │  │    tests        │
└────────┬────────┘  └────────┬────────┘
         │                    │
         └────────┬───────────┘
                  ▼
┌─────────────────────────────────────┐
│  11-update-e2e-snapshots           │  ← Major: E2E coverage
└────────────────┬────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────┐
│  12-minor-cleanup                  │  ← Minor: Polish
└─────────────────────────────────────┘
```

## Success Criteria

Phase 8 is complete when:

### Original Tasks (1-5)
- [x] `UiMode::NewSessionDialog` replaces `StartupDialog` and `DeviceSelector`
- [x] App startup shows NewSessionDialog when no sessions exist
- [x] 'd' key opens NewSessionDialog to add session
- [x] Old `DeviceSelectorState` and `StartupDialogState` removed
- [x] Old widget files deleted
- [x] All references updated
- [x] Documentation updated

### Review Follow-up Tasks (6-12)
- [ ] `+` and `d` keys work correctly without sessions (Task 6)
- [ ] All deprecated message variants removed from `message.rs` (Task 7)
- [ ] All deprecated handlers removed from `update.rs` (Task 8)
- [ ] Handler tests compile and pass (Task 9)
- [ ] Render tests compile and pass (Task 10)
- [ ] E2E snapshots updated (Task 11)
- [ ] Minor cleanup completed (Task 12)
- [ ] `cargo fmt && cargo check && cargo test && cargo clippy -- -D warnings` passes

## Files to Delete

- `src/tui/widgets/device_selector.rs`
- `src/tui/widgets/startup_dialog/mod.rs`
- `src/tui/widgets/startup_dialog/styles.rs`

## Files to Modify

After Phase 6.1, the handler structure is modular:

- `src/app/state.rs` - Remove old state types
- `src/app/message.rs` - Remove old message types
- `src/app/handler/startup_dialog.rs` - **DELETE** (replaces old code in update.rs)
- `src/app/handler/device_selector.rs` - **DELETE** (replaces old code in update.rs)
- `src/app/handler/keys.rs` - Remove old key handlers
- `src/app/handler/mod.rs` - Update exports (remove deleted modules)
- `src/tui/render/mod.rs` - Update rendering
- `src/tui/widgets/mod.rs` - Update exports

## Migration Notes

### State Migration
```
Before:
  AppState.startup_dialog: Option<StartupDialogState>
  AppState.device_selector: DeviceSelectorState

After:
  AppState.new_session_dialog: Option<NewSessionDialogState>
```

### UiMode Migration
```
Before:
  UiMode::StartupDialog
  UiMode::DeviceSelector

After:
  UiMode::NewSessionDialog
```

### Message Migration
```
Before:
  Message::StartupDialogXxx
  Message::DeviceSelectorXxx

After:
  Message::NewSessionDialogXxx
```

## Notes

- Run tests frequently during cleanup
- Keep old code until new code is fully working
- Update one file at a time to catch issues early
- Manual testing: startup flow, add device flow, all keyboard shortcuts
