# Phase 6: Launch Context Widget - Task Index

## Overview

Create the Launch Context widget - the right pane of the NewSessionDialog. Contains configuration selection, mode selector, flavor, dart-defines, and launch button.

**Total Tasks:** 5
**Estimated Time:** 2 hours

## UI Design

```
┌── ⚙️ Launch Context ─────────────────┐
│                                       │
│  Configuration:                       │
│  [ Development (Default)          ▼]  │  ← Opens fuzzy modal
│                                       │
│  Mode:                                │
│  (●) Debug  (○) Profile  (○) Release  │
│                                       │
│  Flavor:                              │
│  [ dev____________________        ▼]  │  ← Opens fuzzy modal (if editable)
│                                       │
│  Dart Defines:                        │
│  [ 3 items                        ▶]  │  ← Opens dart defines modal
│                                       │
│  [          🚀 LAUNCH (Enter)       ] │
│                                       │
└───────────────────────────────────────┘
```

## Config Editability Rules

| Config Source | Mode | Flavor | Dart Defines | Behavior |
|---------------|------|--------|--------------|----------|
| VSCode | Read-only | Read-only | Read-only | All fields disabled, show "(from config)" |
| FDemon | Editable | Editable | Editable | Changes auto-save to `.fdemon/launch.toml` |
| None selected | Editable | Editable | Editable | Transient values, not persisted |

## Task Dependency Graph

```
┌─────────────────────────────────────┐
│  01-launch-context-state            │
└────────────────┬────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────┐
│  02-field-widgets                   │
└────────────────┬────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────┐
│  03-config-auto-save                │
└────────────────┬────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────┐
│  04-launch-context-widget           │
└────────────────┬────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────┐
│  05-launch-context-messages         │
└─────────────────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Est. | Modules |
|---|------|--------|------------|------|---------|
| 1 | [01-launch-context-state](tasks/01-launch-context-state.md) | Not Started | Phase 1 | 25m | `new_session_dialog/state.rs` |
| 2 | [02-field-widgets](tasks/02-field-widgets.md) | Not Started | 1 | 30m | `new_session_dialog/launch_context.rs` |
| 3 | [03-config-auto-save](tasks/03-config-auto-save.md) | Not Started | 2 | 20m | `config/writer.rs` |
| 4 | [04-launch-context-widget](tasks/04-launch-context-widget.md) | Not Started | 3 | 25m | `new_session_dialog/launch_context.rs` |
| 5 | [05-launch-context-messages](tasks/05-launch-context-messages.md) | Not Started | 4 | 15m | `app/message.rs`, `app/handler/update.rs` |

## Success Criteria

Phase 6 is complete when:

- [ ] `LaunchContextState` struct with config, mode, flavor, dart_defines
- [ ] Configuration dropdown opens fuzzy modal
- [ ] Mode radio buttons work (Debug/Profile/Release)
- [ ] Flavor field opens fuzzy modal (when editable)
- [ ] Dart Defines field opens dart defines modal (when editable)
- [ ] Fields show disabled state for VSCode configs
- [ ] FDemon config changes auto-save to file
- [ ] Launch button renders with focus state
- [ ] Up/Down navigation between fields
- [ ] `cargo fmt && cargo check && cargo test && cargo clippy -- -D warnings` passes

## Field Navigation

- Up/Down moves between fields: Config → Mode → Flavor → Dart Defines → Launch
- Enter on Config/Flavor → opens fuzzy modal
- Enter on Dart Defines → opens dart defines modal
- Enter on Launch → triggers launch action
- Left/Right on Mode → changes mode selection

## Notes

- Field editability depends on selected config source
- VSCode configs show "(from config)" suffix
- FDemon configs auto-save on change
- No config selected → transient values
- Consider visual indication of which fields are editable
