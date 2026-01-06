# Phase 5: Launch Configuration Enhancements - Task Index

## Overview

Complete launch configuration system with startup dialog, config editing, and VSCode launch.json compatibility. Replaces device selector with comprehensive startup dialog for session launching.

**Total Tasks:** 9

## Task Dependency Graph

```
┌─────────────────────────┐     ┌─────────────────────────┐
│ 01 Config Priority      │     │ 02 Dialog State/Msgs    │
│ (config/priority.rs)    │     │ (app/state, message)    │
└───────────┬─────────────┘     └───────────┬─────────────┘
            │                               │
            └───────────┬───────────────────┘
                        ▼
┌─────────────────────────────────────────────────────────┐
│                  03 Startup Dialog Widget               │
│               (tui/widgets/startup_dialog/)             │
└───────────────────────────┬─────────────────────────────┘
                            │
            ┌───────────────┼───────────────┐
            ▼               ▼               ▼
┌───────────────────┐ ┌───────────────┐ ┌───────────────────┐
│ 04 Key Handler    │ │ 05 User Prefs │ │ 06 Startup Flow   │
│ (handler/keys.rs) │ │ (settings.rs) │ │ (startup.rs)      │
└───────────────────┘ └───────────────┘ └─────────┬─────────┘
                                                  │
                        ┌─────────────────────────┘
                        ▼
            ┌─────────────────────────┐
            │ 07 Launch Config Edit   │ (independent)
            └─────────────────────────┘
                        │
                        ▼
            ┌─────────────────────────┐
            │ 08 Deprecate DeviceSel  │
            └─────────────────────────┘
                        │
                        ▼
            ┌─────────────────────────┐
            │ 09 Documentation        │
            └─────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 1 | [01-config-priority](tasks/01-config-priority.md) | Not Started | - | `config/priority.rs` (new) |
| 2 | [02-dialog-state-messages](tasks/02-dialog-state-messages.md) | Not Started | - | `app/state.rs`, `app/message.rs` |
| 3 | [03-startup-dialog-widget](tasks/03-startup-dialog-widget.md) | Not Started | 1, 2 | `tui/widgets/startup_dialog/` |
| 4 | [04-dialog-key-handler](tasks/04-dialog-key-handler.md) | Not Started | 2, 3 | `app/handler/keys.rs` |
| 5 | [05-preferences-autosave](tasks/05-preferences-autosave.md) | Not Started | - | `config/settings.rs` |
| 6 | [06-startup-flow-refactor](tasks/06-startup-flow-refactor.md) | Not Started | 1, 2, 3, 5 | `tui/startup.rs` |
| 7 | [07-launch-config-editing](tasks/07-launch-config-editing.md) | Not Started | - | `config/launch.rs`, `settings_panel/` |
| 8 | [08-deprecate-device-selector](tasks/08-deprecate-device-selector.md) | Not Started | 3, 6 | `widgets/device_selector.rs` |
| 9 | [09-documentation](tasks/09-documentation.md) | Not Started | All | `docs/*.md` |

## Execution Waves

### Wave 1 (Foundation) - Parallel
- Task 01: Config Priority & Display
- Task 02: Dialog State & Messages

### Wave 2 (Core Widget) - Parallel after Wave 1
- Task 03: Startup Dialog Widget
- Task 04: Key Handler for Startup Dialog

### Wave 3 (Integration) - After Wave 2
- Task 05: User Preferences Auto-save (independent)
- Task 06: Startup Flow Refactor

### Wave 4 (Editing & Cleanup) - After Wave 3
- Task 07: Launch Config Editing (independent)
- Task 08: Deprecate Device Selector

### Wave 5 (Documentation) - After All
- Task 09: Documentation Updates

## Success Criteria

Phase 5 is complete when:

- [ ] Startup dialog shows combined launch.toml + launch.json configs with divider
- [ ] Config priority: launch.toml first, then launch.json
- [ ] Auto-start respects `settings.local.toml` preferences
- [ ] Startup dialog allows mode/flavor/dart-define selection
- [ ] Last selection auto-saves to `settings.local.toml`
- [ ] Creating/editing launch.toml configs works
- [ ] launch.json remains read-only
- [ ] Device selector deprecated for startup (may keep for add-session)
- [ ] All keybindings documented in `docs/KEYBINDINGS.md`
- [ ] All new code has unit tests
- [ ] `cargo test` passes
- [ ] `cargo clippy -- -D warnings` passes

## Keyboard Shortcuts (New)

### Startup Dialog
| Key | Action |
|-----|--------|
| `j` / `↓` | Navigate down in current section |
| `k` / `↑` | Navigate up in current section |
| `Tab` | Next section |
| `Shift+Tab` | Previous section |
| `Enter` | Confirm (launch session) |
| `Esc` | Cancel |
| `r` | Refresh devices |
| Character | Text input for flavor/dart-defines |
| `Backspace` | Delete character in input |

## Notes

- **User Decision**: Startup dialog is centered modal, not full-screen
- **User Decision**: launch.json is read-only (users edit in VSCode)
- **User Decision**: Selections auto-save to settings.local.toml
- **Flutter Note**: Always use `-d <device_id>` to avoid interactive prompts
