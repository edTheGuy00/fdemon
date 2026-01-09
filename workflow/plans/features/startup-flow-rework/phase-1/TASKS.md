# Phase 1: Core Flow Changes - Task Index

## Overview

Enable the app to start in Normal mode without sessions and show appropriate "Not Connected" state instead of immediately showing the StartupDialog.

**Total Tasks:** 3

## Task Dependency Graph

```
┌─────────────────────────────────────┐
│  01-modify-startup-logic            │
│  (Enter Normal mode on startup)     │
└─────────────────┬───────────────────┘
                  │
                  ▼
┌─────────────────────────────────────┐     ┌─────────────────────────────────┐
│  02-update-empty-state-display      │     │  03-update-status-bar           │
│  (Change centered message)          │     │  (Show "Not Connected")         │
└─────────────────────────────────────┘     └─────────────────────────────────┘
         │                                           │
         └───────────────────┬───────────────────────┘
                             │
                    (Can run in parallel)
```

## Tasks

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 1 | [01-modify-startup-logic](tasks/01-modify-startup-logic.md) | Not Started | - | `tui/startup.rs` |
| 2 | [02-update-empty-state-display](tasks/02-update-empty-state-display.md) | Not Started | 1 | `tui/widgets/log_view/mod.rs` |
| 3 | [03-update-status-bar](tasks/03-update-status-bar.md) | Not Started | 1 | `tui/widgets/status_bar/mod.rs` |

## Success Criteria

Phase 1 is complete when:

- [ ] App starts in `UiMode::Normal` when `auto_start = false`
- [ ] No StartupDialog appears automatically on startup
- [ ] Status bar shows "○ Not Connected" when no sessions exist
- [ ] Log area shows "Press + to start a new session" (centered)
- [ ] Auto-start flow still works correctly (when configured)
- [ ] `cargo check` passes
- [ ] `cargo clippy -- -D warnings` passes
- [ ] `cargo test --lib` passes

## Notes

- The '+' keybinding will be added in Phase 2; for now the 'n' and 'd' keys still work
- Snapshot tests will fail after these changes - they'll be updated in Phase 3
- E2E tests should start passing more easily after this phase
