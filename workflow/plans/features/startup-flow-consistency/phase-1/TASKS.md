# Phase 1: Add StartAutoLaunch Message and Handler - Task Index

## Overview

Create the message infrastructure for triggering auto-start from the event loop. This phase establishes the foundation for moving auto-start logic from the synchronous pre-loop phase into the TEA message loop.

**Total Tasks:** 4
**Estimated Hours:** 4-6 hours

## Task Dependency Graph

```
┌─────────────────────────────────┐
│  01-add-message-variants        │
└───────────────┬─────────────────┘
                │
                ▼
┌─────────────────────────────────┐
│  02-add-update-action           │
└───────────────┬─────────────────┘
                │
                ▼
┌─────────────────────────────────┐
│  03-add-handler-scaffolding     │
└───────────────┬─────────────────┘
                │
                ▼
┌─────────────────────────────────┐
│  04-add-spawn-function          │
└─────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. Hours | Modules |
|---|------|--------|------------|------------|---------|
| 1 | [01-add-message-variants](tasks/01-add-message-variants.md) | Not Started | - | 0.5h | `message.rs` |
| 2 | [02-add-update-action](tasks/02-add-update-action.md) | Not Started | 1 | 0.5h | `handler/mod.rs` |
| 3 | [03-add-handler-scaffolding](tasks/03-add-handler-scaffolding.md) | Not Started | 2 | 1-2h | `handler/update.rs` |
| 4 | [04-add-spawn-function](tasks/04-add-spawn-function.md) | Not Started | 3 | 2-3h | `tui/spawn.rs`, `tui/actions.rs` |

## Success Criteria

Phase 1 is complete when:

- [ ] `Message::StartAutoLaunch` exists and compiles
- [ ] `Message::AutoLaunchProgress` exists and compiles
- [ ] `Message::AutoLaunchResult` exists and compiles
- [ ] `UpdateAction::DiscoverDevicesAndAutoLaunch` exists
- [ ] Handler dispatches to scaffolding functions (can log/no-op initially)
- [ ] Spawn function structure is in place
- [ ] `cargo fmt && cargo check && cargo clippy -- -D warnings` passes

## Notes

- This phase focuses on infrastructure only; actual behavior change happens in Phase 2
- Handlers can be minimal/no-op initially - full logic comes in Phase 3
- Keep existing startup flow working during this phase (no breaking changes)
- All new code should have doc comments explaining purpose
