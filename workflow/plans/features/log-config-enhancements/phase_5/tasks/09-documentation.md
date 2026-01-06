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

**Status:** Done

**Files Modified:**

| File | Changes |
|------|---------|
| `docs/KEYBINDINGS.md` | Added Startup Dialog section with navigation, text input, and section jump keys; Updated 'd' key description; Added to TOC |
| `docs/CONFIGURATION.md` | Added Launch Configuration section covering priority order, launch.toml format, launch.json compatibility, auto-start behavior, user preferences, and config editing |

**Implementation Details:**

1. **KEYBINDINGS.md Updates:**
   - Added new "Startup Dialog" section with comprehensive keybindings table
   - Documented navigation keys (j/k, arrows, Tab, Shift+Tab)
   - Documented action keys (Enter, Esc, r for refresh)
   - Documented section jump keys (1-5 for Config/Mode/Flavor/DartDefines/Device)
   - Added "Text Input" subsection for Flavor/Dart Defines editing
   - Updated 'd' key description to mention Startup Dialog behavior
   - Added section to Table of Contents

2. **CONFIGURATION.md Updates:**
   - Added comprehensive "Launch Configuration" section before Global Settings Reference
   - Documented two-source priority system (launch.toml first, then launch.json)
   - Provided launch.toml format with TOML code examples
   - Explained launch.json compatibility with supported fields list
   - Documented 6-step auto-start behavior priority chain
   - Explained settings.local.toml auto-save with example
   - Added "Creating Configurations" subsection with Settings panel keybindings
   - Added subsections to Table of Contents

**Notable Decisions/Tradeoffs:**

1. **Placement of Launch Configuration Section:** Placed before "Global Settings Reference" to appear early in the document since it's a fundamental concept
2. **Code Examples:** Used simple, clear examples focusing on common use cases (Development/Production)
3. **Structured Format:** Used tables and numbered lists for easy scanning and reference

**Testing Performed:**

- `cargo fmt` - Passed (no code changes)
- `cargo check` - Passed
- `cargo test --lib` - Passed (1197 tests passed, 0 failed)
- Manual review of documentation structure and accuracy
- Verified all acceptance criteria met:
  1. Startup Dialog keybindings documented in KEYBINDINGS.md - YES
  2. Config priority order explained in CONFIGURATION.md - YES
  3. launch.toml format documented with examples - YES
  4. launch.json compatibility explained - YES
  5. Auto-start behavior documented - YES
  6. settings.local.toml auto-save explained - YES
  7. Config editing keybindings documented - YES

**Risks/Limitations:**

1. **Documentation Accuracy:** Documentation reflects implementation at task completion time; future changes may require updates
2. **User Testing Needed:** Real users should verify documentation clarity and completeness
