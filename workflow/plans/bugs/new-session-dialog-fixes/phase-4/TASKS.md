# Phase 4: Auto-Config Creation - Task Index

## Overview

Automatically create and save a default configuration when the user sets flavor or dart-defines without having a config selected. This ensures user preferences are persisted to `.fdemon/launch.toml` instead of being lost after launch.

**Total Tasks:** 3
**Bugs Addressed:** Bug 6 (No auto-creation of default config)

## Task Dependency Graph

```
┌─────────────────────────────────────┐
│  01-auto-config-helper              │
│  Add state helper methods           │
└──────────────────┬──────────────────┘
                   │
        ┌──────────┴──────────┐
        ▼                     ▼
┌───────────────────┐  ┌───────────────────┐
│  02-flavor-auto   │  │  03-dart-defines  │
│  Flavor handler   │  │  Dart-defines     │
└───────────────────┘  └───────────────────┘

(Tasks 02 and 03 depend on 01, but can run in parallel with each other)
```

## Tasks

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 1 | [01-auto-config-helper](tasks/01-auto-config-helper.md) | Not Started | - | `state.rs`, `launch_context.rs` (state) |
| 2 | [02-flavor-auto-config](tasks/02-flavor-auto-config.md) | Not Started | 01 | `launch_context.rs` (handler) |
| 3 | [03-dart-defines-auto-config](tasks/03-dart-defines-auto-config.md) | Not Started | 01 | `launch_context.rs` (handler) |

## Success Criteria

Phase 4 is complete when:

- [ ] Setting flavor without config selected creates new "Default" config
- [ ] Setting dart-defines without config selected creates new "Default" config
- [ ] Created config is automatically selected in the dialog
- [ ] Config is saved to `.fdemon/launch.toml` via auto-save
- [ ] Unique naming works ("Default", "Default 2", etc.) if "Default" already exists
- [ ] Next dialog open shows the created config in the list
- [ ] No regression in existing config editing behavior
- [ ] All new code has unit tests
- [ ] `cargo test` passes
- [ ] `cargo clippy` passes

## Data Flow

```
User sets flavor/dart-defines with no config selected
                    │
                    ▼
┌─────────────────────────────────────────────────┐
│ Handler detects: selected_config_index == None  │
│                  AND values are being set       │
└──────────────────────┬──────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────┐
│ Create default config via helper:               │
│ - create_default_launch_config()                │
│ - Set flavor/dart-defines on new config         │
│ - add_launch_config() with unique name          │
└──────────────────────┬──────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────┐
│ Update dialog state:                            │
│ - Add config to LaunchContextState.configs      │
│ - Set selected_config_index to new config       │
└──────────────────────┬──────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────┐
│ Trigger auto-save:                              │
│ - Return UpdateAction::AutoSaveConfig           │
│ - save_fdemon_configs() writes to disk          │
└─────────────────────────────────────────────────┘
```

## Notes

- Existing `create_default_launch_config()` at `config/launch.rs:151-163` creates a config template
- Existing `add_launch_config()` at `config/launch.rs:165-186` handles unique naming
- Auto-save mechanism already exists via `UpdateAction::AutoSaveConfig`
- Only create config for FDemon source (not VSCode configs which are read-only)
- Config should inherit current mode from dialog state
