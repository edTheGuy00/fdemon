# Phase 1: Entry Point Core Fix - Task Index

## Overview

Fix the broken entry_point flow so that VSCode `program` field and FDemon `entry_point` config are correctly passed to the Flutter process.

**Total Tasks:** 6

## Task Dependency Graph

```
┌─────────────────────────┐     ┌─────────────────────────┐
│  01-add-entry-point-    │     │  02-add-entry-point-    │
│  to-launch-params       │     │  to-launch-context-     │
│                         │     │  state                  │
└───────────┬─────────────┘     └───────────┬─────────────┘
            │                               │
            └───────────┬───────────────────┘
                        ▼
            ┌─────────────────────────┐
            │  03-update-select-      │
            │  config                 │
            └───────────┬─────────────┘
                        │
                        ▼
            ┌─────────────────────────┐
            │  04-update-build-       │
            │  launch-params          │
            └───────────┬─────────────┘
                        │
                        ▼
            ┌─────────────────────────┐
            │  05-update-handle-      │
            │  launch                 │
            └───────────┬─────────────┘
                        │
                        ▼
            ┌─────────────────────────┐
            │  06-add-update-field-   │
            │  support                │
            └─────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 1 | [01-add-entry-point-to-launch-params](tasks/01-add-entry-point-to-launch-params.md) | Not Started | - | `types.rs` |
| 2 | [02-add-entry-point-to-launch-context-state](tasks/02-add-entry-point-to-launch-context-state.md) | Not Started | - | `state.rs` |
| 3 | [03-update-select-config](tasks/03-update-select-config.md) | Not Started | 2 | `state.rs` |
| 4 | [04-update-build-launch-params](tasks/04-update-build-launch-params.md) | Not Started | 1, 2 | `state.rs` |
| 5 | [05-update-handle-launch](tasks/05-update-handle-launch.md) | Not Started | 1, 4 | `launch_context.rs` |
| 6 | [06-add-update-field-support](tasks/06-add-update-field-support.md) | Not Started | - | `config/launch.rs` |

## Success Criteria

Phase 1 is complete when:

- [ ] `LaunchParams` includes `entry_point` field
- [ ] `LaunchContextState` includes `entry_point` field
- [ ] `select_config()` applies `entry_point` from selected config
- [ ] `build_launch_params()` extracts `entry_point` from state
- [ ] `handle_launch()` passes `entry_point` to `LaunchConfig`
- [ ] `update_launch_config_field()` handles `entry_point` field
- [ ] VSCode configs with `program` field result in correct `-t` argument
- [ ] FDemon configs with `entry_point` field load and save correctly
- [ ] All unit tests pass
- [ ] `cargo clippy` passes with no warnings

## Verification Commands

```bash
cargo test --lib entry_point
cargo test --lib launch_params
cargo test --lib select_config
cargo clippy -- -D warnings
```

## Notes

- Tasks 1 and 2 can be done in parallel
- Task 6 is independent and can be done in parallel with tasks 1-5
- All changes are in the app layer except task 6 (config layer)
