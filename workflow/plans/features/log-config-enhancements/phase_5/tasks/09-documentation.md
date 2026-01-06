# Task: Documentation Updates

**Objective**: Update documentation to reflect the new startup dialog, config priority, and auto-save behavior.

**Depends on**: All other tasks (Wave 5)

## Scope

- `docs/KEYBINDINGS.md` — Add startup dialog keybindings
- `docs/CONFIGURATION.md` — Document config priority, auto-save, preferences

## Details

### KEYBINDINGS.md Updates

Add new section for Startup Dialog:

```markdown
## Startup Dialog

The startup dialog appears when `auto_start = false` or when pressing `d` without running sessions.

| Key | Action |
|-----|--------|
| `j` / `↓` | Navigate down in current section |
| `k` / `↑` | Navigate up in current section |
| `Tab` | Move to next section |
| `Shift+Tab` | Move to previous section |
| `Enter` | Confirm and launch session |
| `Esc` | Cancel dialog |
| `r` | Refresh device list |
| `1` | Jump to Configuration section |
| `2` | Jump to Mode section |
| `3` | Jump to Flavor input (edit mode) |
| `4` | Jump to Dart Defines input (edit mode) |
| `5` | Jump to Device section |

### Text Input (Flavor / Dart Defines)

When editing Flavor or Dart Defines fields:

| Key | Action |
|-----|--------|
| Any character | Add to input |
| `Backspace` | Delete last character |
| `Delete` / `Ctrl+U` | Clear entire field |
| `Enter` | Exit edit mode |
| `Esc` | Exit edit mode |
| `Tab` | Exit edit mode and move to next section |
```

Update 'd' key description:

```markdown
## Normal Mode

| Key | Action |
|-----|--------|
| ... |  |
| `d` | Add device session (shows Startup Dialog if no sessions, Device Selector if sessions running) |
| ... |  |
```

### CONFIGURATION.md Updates

Add new section for Launch Configuration:

```markdown
## Launch Configuration

Flutter Demon supports two sources for launch configurations:

1. **`.fdemon/launch.toml`** - Flutter Demon native format (recommended)
2. **`.vscode/launch.json`** - VSCode Dart/Flutter format (read-only)

### Priority Order

When both files exist, configurations are loaded in this order:
1. `.fdemon/launch.toml` configurations (first)
2. `.vscode/launch.json` configurations (second)

The startup dialog displays them with a visual divider between sources.

### launch.toml Format

```toml
# .fdemon/launch.toml

[[configurations]]
name = "Development"
device = "auto"              # "auto" or specific device ID
mode = "debug"               # debug, profile, or release
flavor = "development"       # optional
auto_start = true            # optional, default false

[configurations.dart_defines]
API_URL = "https://dev.api.com"
DEBUG = "true"

[[configurations]]
name = "Production"
device = "auto"
mode = "release"
flavor = "production"
```

### launch.json Compatibility

Flutter Demon reads `.vscode/launch.json` for VSCode users who want to use their existing configurations. These configurations are **read-only** in Flutter Demon - edit them in VSCode.

Supported launch.json fields:
- `name` - Configuration name
- `type` - Must be "dart"
- `request` - Must be "launch"
- `flutterMode` - Maps to mode (debug/profile/release)
- `deviceId` - Target device
- `args` - Additional flutter run arguments

### Auto-Start Behavior

When `behavior.auto_start = true` in `config.toml`:

1. Check `settings.local.toml` for last used config/device
2. If found and valid, use that selection
3. If not found, look for first config with `auto_start = true`
4. If no auto_start config, use first config from launch.toml
5. If no launch.toml, use first config from launch.json
6. If no configs at all, run bare `flutter run` with first available device

### User Preferences (settings.local.toml)

Your last selection is automatically saved to `.fdemon/settings.local.toml`:

```toml
# .fdemon/settings.local.toml (auto-generated, gitignored)

last_config = "Development"
last_device = "iPhone-15-Pro"
```

This file is automatically added to `.gitignore` as it contains user-specific preferences.

### Creating Configurations

In the Settings panel (`S` key), navigate to the "Launch Config" tab:

| Key | Action |
|-----|--------|
| `n` | Create new configuration |
| `d` | Delete selected configuration |
| `Enter` | Edit selected field |

Note: Only `.fdemon/launch.toml` configurations can be edited. VSCode configurations are read-only.
```

### README.md Update (Optional)

If README mentions device selection, update to mention startup dialog:

```markdown
## Quick Start

1. Run `fdemon` in your Flutter project directory
2. If `auto_start` is disabled, the startup dialog will appear
3. Select a configuration (or use none for bare `flutter run`)
4. Choose build mode (Debug/Profile/Release)
5. Optionally set flavor and dart-define arguments
6. Select a target device
7. Press Enter to launch
```

## Acceptance Criteria

1. Startup Dialog keybindings documented in KEYBINDINGS.md
2. Config priority order explained in CONFIGURATION.md
3. launch.toml format documented with examples
4. launch.json compatibility explained
5. Auto-start behavior documented
6. settings.local.toml auto-save explained
7. Config editing keybindings documented

## Testing

Documentation testing (manual):
- [ ] Read through KEYBINDINGS.md - all keys accurate
- [ ] Read through CONFIGURATION.md - all options accurate
- [ ] Try following the documented config format - works
- [ ] Launch.json compatibility as described

## Notes

- Keep documentation concise but complete
- Include code examples where helpful
- Link between related sections
- Update version/changelog if applicable

---

## Completion Summary

**Status:** Not Started

**Files Modified:**
- (none yet)

**Implementation Details:**
(to be filled after implementation)

**Testing Performed:**
- Manual review of documentation accuracy
