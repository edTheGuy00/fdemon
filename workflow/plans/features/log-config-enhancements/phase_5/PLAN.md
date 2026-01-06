# Phase 5: Launch Configuration Enhancements

## Overview
Extend launch configuration system with full editing support for launch.toml, a new comprehensive startup dialog, and proper VSCode launch.json compatibility.

## User Decisions
- **Startup dialog**: Centered modal (floating dialog over dimmed background)
- **launch.json**: Read-only (no editing to avoid VSCode conflicts)
- **Preferences**: Auto-save last selected config/device to settings.local.toml

---

## Goals

1. **Launch Config Editing**: Make "n" key functional to create/edit launch.toml configurations
2. **Startup Priority**: launch.toml → launch.json (with visual divider if both exist)
3. **Auto-start Logic**: Check settings.local.toml preferences first, then configs
4. **Startup Dialog**: Replace device_selector with comprehensive modal dialog
5. **VSCode Compatibility**: Full support for launch.json without requiring launch.toml

---

## Requirements

### 1. Launch Config Editing (launch.toml only)
- Create new launch configurations in `.fdemon/launch.toml`
- Edit existing configuration values
- Delete configurations
- **launch.json stays read-only**

### 2. Startup Priority Order
When loading configurations:
1. **First**: `.fdemon/launch.toml` configurations
2. **Second**: `.vscode/launch.json` configurations

Display behavior:
- If both exist: show launch.toml on top with visual divider, then launch.json below
- If only one exists: show that one
- If none exist: allow launching with default `flutter run` (no config)

### 3. Auto-start Logic
When `auto_start = true`:
1. Check `settings.local.toml` for `last_config` + `last_device`
2. If found and both exist: use those preferences
3. If not found: check `launch.toml` for first config with `auto_start = true`
4. If no auto_start config: use first config from `launch.toml`
5. If no `launch.toml`: use first config from `launch.json`
6. If no configs at all: run `flutter run` with first available device

### 4. Startup Dialog (when auto_start = OFF)
Replace `device_selector` with comprehensive startup dialog:

**UI Components** (centered modal):
- Launch config dropdown (launch.toml + launch.json configs with divider)
- Mode selector: Debug / Profile / Release
- Flavor input (optional, pre-filled if defined in config)
- Dart-define args (optional text input)
- Device list (scrollable, with emulator launch options)
- "Remember selection" auto-enabled (saves to settings.local.toml)

**Key bindings**:
- `j/k` or `↑/↓`: Navigate items
- `Tab`: Move between sections
- `Enter`: Select/confirm
- `Esc`: Cancel (if sessions running) or quit
- `r`: Refresh devices

### 5. VSCode Compatibility
- No `launch.toml` required
- All features available via launch.json configs
- Can mix both if user wants

---

## Module Structure

```
src/tui/widgets/startup_dialog/
├── mod.rs      # Widget impl, render, layout
├── state.rs    # StartupDialogState
└── styles.rs   # Visual styling
```

---

## Critical Files

| File | Purpose |
|------|---------|
| `src/tui/widgets/startup_dialog/mod.rs` | New startup dialog widget |
| `src/tui/startup.rs` | Startup flow logic |
| `src/app/state.rs` | StartupDialogState |
| `src/app/message.rs` | New messages |
| `src/app/handler/keys.rs` | Key bindings |
| `src/config/launch.rs` | Config save/create |
| `src/config/settings.rs` | Preferences save/load |
| `src/config/priority.rs` | Config priority/loading |

---

## Testing Checklist

- [ ] Startup with only launch.toml → shows launch.toml configs
- [ ] Startup with only launch.json → shows launch.json configs
- [ ] Startup with both → shows launch.toml first, divider, then launch.json
- [ ] Startup with neither → allows bare `flutter run` with device selection
- [ ] Auto-start respects settings.local.toml preferences
- [ ] Creating new launch.toml config works
- [ ] Editing launch.toml config works
- [ ] launch.json remains read-only
- [ ] Last selection auto-saves to settings.local.toml
- [ ] Startup dialog navigation works (j/k, Tab, Enter, Esc)
- [ ] Device refresh works in startup dialog
