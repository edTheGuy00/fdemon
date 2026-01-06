# Phase 5: Launch Configuration Enhancements - Task Index

## Overview

Complete launch configuration system with startup dialog, config editing, and VSCode launch.json compatibility. Replaces device selector with comprehensive startup dialog for session launching.

**Total Tasks:** 18

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
└─────────┬─────────┘ └───────────────┘ └─────────┬─────────┘
          │                                       │
          │                 ┌─────────────────────┘
          │                 ▼
          │     ┌─────────────────────────┐
          │     │ 07 Launch Config Edit   │ (independent)
          │     └─────────────────────────┘
          │                 │
          │                 ▼
          │     ┌─────────────────────────┐
          │     │ 08 Deprecate DeviceSel  │
          │     └───────────┬─────────────┘
          │                 │
          ▼                 ▼
┌─────────────────────────────────────────────────────────┐
│              BUGFIX WAVE (8a, 8b, 8c) - Parallel        │
├─────────────────┬─────────────────┬─────────────────────┤
│ 08a Section     │ 08b Text Field  │ 08c Device          │
│ Jump Keys       │ Editing         │ Discovery           │
│ (1-5 shortcuts) │ (Flavor/Dart)   │ Integration         │
└─────────────────┴─────────────────┴─────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────┐
│           POLISH WAVE (8d, 8e, 8f) - Parallel           │
├─────────────────┬─────────────────┬─────────────────────┤
│ 08d Loading     │ 08e Device      │ 08f VSCode args     │
│ Screen          │ Cache Sharing   │ Parsing             │
│ (auto_start UX) │ (instant show)  │ (--flavor in args)  │
└─────────────────┴─────────────────┴─────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────┐
│         CRITICAL BUGFIX WAVE (9a, 9b, 9c) - Parallel   │
├─────────────────┬─────────────────┬─────────────────────┤
│ 09a JSONC       │ 09b Manual      │ 09c Loading         │
│ Trailing Commas │ Flavor Pass     │ Animation Fix       │
│ (vscode.rs)     │ (update.rs)     │ (runner/startup)    │
└─────────────────┴─────────────────┴─────────────────────┘
                            │
                            ▼
            ┌─────────────────────────┐
            │ 09 Documentation        │
            └─────────────────────────┘
```

## Tasks

| # | Task | Status | Depends On | Modules |
|---|------|--------|------------|---------|
| 1 | [01-config-priority](tasks/01-config-priority.md) | Done | - | `config/priority.rs` (new) |
| 2 | [02-dialog-state-messages](tasks/02-dialog-state-messages.md) | Done | - | `app/state.rs`, `app/message.rs` |
| 3 | [03-startup-dialog-widget](tasks/03-startup-dialog-widget.md) | Done | 1, 2 | `tui/widgets/startup_dialog/` |
| 4 | [04-dialog-key-handler](tasks/04-dialog-key-handler.md) | Done | 2, 3 | `app/handler/keys.rs` |
| 5 | [05-preferences-autosave](tasks/05-preferences-autosave.md) | Done | - | `config/settings.rs` |
| 6 | [06-startup-flow-refactor](tasks/06-startup-flow-refactor.md) | Done | 1, 2, 3, 5 | `tui/startup.rs` |
| 7 | [07-launch-config-editing](tasks/07-launch-config-editing.md) | Done | - | `config/launch.rs`, `settings_panel/` |
| 8 | [08-deprecate-device-selector](tasks/08-deprecate-device-selector.md) | Done | 3, 6 | `widgets/device_selector.rs` |
| 8a | [08a-section-jump-keys](tasks/08a-section-jump-keys.md) | Done | 4 | `app/message.rs`, `handler/keys.rs`, `handler/update.rs` |
| 8b | [08b-text-field-editing](tasks/08b-text-field-editing.md) | Done | 4 | `app/message.rs`, `handler/keys.rs`, `handler/update.rs`, `startup_dialog/` |
| 8c | [08c-device-discovery-integration](tasks/08c-device-discovery-integration.md) | Done | 3, 6 | `handler/update.rs` |
| 8d | [08d-startup-loading-screen](tasks/08d-startup-loading-screen.md) | Done | 6 | `tui/runner.rs`, `tui/render.rs`, `app/state.rs` |
| 8e | [08e-device-cache-sharing](tasks/08e-device-cache-sharing.md) | Done | 8c | `app/state.rs`, `handler/update.rs`, `tui/startup.rs` |
| 8f | [08f-vscode-args-parsing](tasks/08f-vscode-args-parsing.md) | Done | 1 | `config/vscode.rs` |
| 9a | [09a-jsonc-trailing-commas](tasks/09a-jsonc-trailing-commas.md) | Done | 8f | `config/vscode.rs` |
| 9b | [09b-manual-flavor-passthrough](tasks/09b-manual-flavor-passthrough.md) | Done | 8b | `handler/update.rs` |
| 9c | [09c-loading-animation-fix](tasks/09c-loading-animation-fix.md) | Done | 8d | `tui/runner.rs`, `tui/startup.rs` |
| 9 | [09-documentation](tasks/09-documentation.md) | Done | All | `docs/*.md` |

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

### Wave 4.5 (Bugfixes) - After Wave 4, Parallel
- Task 08a: Section Jump Keys (1-5 shortcuts)
- Task 08b: Text Field Editing (Flavor/DartDefines)
- Task 08c: Device Discovery Integration

### Wave 4.6 (Polish) - After Wave 4.5, Parallel
- Task 08d: Startup Loading Screen (no black screen on auto_start)
- Task 08e: Device Cache Sharing (instant device list on 'n')
- Task 08f: VSCode args Parsing (--flavor from args field)

### Wave 4.7 (Critical Bugfixes) - After Wave 4.6, Parallel
- Task 09a: JSONC Trailing Comma Support (fix launch.json parsing)
- Task 09b: Manual Flavor Passthrough (fix flavor not passed to flutter)
- Task 09c: Loading Animation Fix (spinner not animating)

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
- [ ] **No black screen on auto_start** (loading indicator shown)
- [ ] **Device list cached** (instant display on 'n' for new session)
- [ ] **VSCode launch.json with args[] flavor parsed correctly**
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
