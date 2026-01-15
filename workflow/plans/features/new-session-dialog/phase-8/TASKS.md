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
| 1 | [01-ui-mode-integration](tasks/01-ui-mode-integration.md) | Not Started | Phase 7 | 35m | `app/state.rs`, `tui/render/mod.rs` |
| 2 | [02-startup-flow](tasks/02-startup-flow.md) | Not Started | 1 | 30m | `app/handler/session.rs`, `main.rs` |
| 3 | [03-remove-old-dialogs](tasks/03-remove-old-dialogs.md) | Not Started | 2 | 35m | `app/handler/startup_dialog.rs`, `app/handler/device_selector.rs`, widgets |
| 4 | [04-update-tests](tasks/04-update-tests.md) | Not Started | 3 | 40m | Test files |
| 5 | [05-documentation](tasks/05-documentation.md) | Not Started | 4 | 20m | `docs/KEYBINDINGS.md` |

## Success Criteria

Phase 8 is complete when:

- [ ] `UiMode::NewSessionDialog` replaces `StartupDialog` and `DeviceSelector`
- [ ] App startup shows NewSessionDialog when no sessions exist
- [ ] 'd' key opens NewSessionDialog to add session
- [ ] Old `DeviceSelectorState` and `StartupDialogState` removed
- [ ] Old widget files deleted
- [ ] All references updated
- [ ] All tests pass
- [ ] Documentation updated
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
