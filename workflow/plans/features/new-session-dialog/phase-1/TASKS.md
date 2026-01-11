# Phase 1: State Foundation - Task Index

## Overview

Create the core state structures and message types for the NewSessionDialog. This phase establishes the foundation for the dual-pane dialog with tabbed device selection.

**Total Tasks:** 5
**Estimated Time:** 2 hours

## Task Dependency Graph

```
┌─────────────────────────────────────┐
│  01-bootable-device-type            │
└────────────────┬────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────┐
│  02-dialog-state-struct             │
└────────────────┬────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────┐
│  03-message-types                   │
└────────────────┬────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────┐
│  04-state-transitions               │
└────────────────┬────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────┐
│  05-ui-mode-integration             │
└─────────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. | Modules |
|---|------|--------|------------|------|---------|
| 1 | [01-bootable-device-type](tasks/01-bootable-device-type.md) | Not Started | - | 15m | `core/types.rs` |
| 2 | [02-dialog-state-struct](tasks/02-dialog-state-struct.md) | Not Started | 1 | 30m | `app/state.rs`, `tui/widgets/new_session_dialog/state.rs` |
| 3 | [03-message-types](tasks/03-message-types.md) | Not Started | 2 | 20m | `app/message.rs` |
| 4 | [04-state-transitions](tasks/04-state-transitions.md) | Not Started | 3 | 30m | `app/state.rs` |
| 5 | [05-ui-mode-integration](tasks/05-ui-mode-integration.md) | Not Started | 4 | 25m | `app/state.rs`, `tui/render/mod.rs` |

## Success Criteria

Phase 1 is complete when:

- [ ] `BootableDevice` type defined with platform, runtime, state fields
- [ ] `NewSessionDialogState` struct with dual-pane and tabbed structure
- [ ] All new message types defined in `Message` enum
- [ ] State navigation methods implemented (pane switch, tab switch, up/down)
- [ ] `UiMode::NewSessionDialog` variant added and recognized
- [ ] `cargo fmt && cargo check && cargo test && cargo clippy -- -D warnings` passes

## Notes

- Keep old `StartupDialogState` and `DeviceSelectorState` during this phase (remove in Phase 7)
- New state should be able to coexist with old state until integration
- Focus on structure, not rendering (that comes in Phase 3-4)
