# Configuration

This document provides a complete reference for all configuration options available in Flutter Demon.

---

## Table of Contents

- [Overview](#overview)
- [Configuration Files](#configuration-files)
  - [`.fdemon/config.toml`](#fdemonconfig.toml) - Global settings
  - [`.fdemon/launch.toml`](#fdemonlaunch.toml) - Launch configurations
  - [`.fdemon/settings.local.toml`](#fdemonsettingslocal.toml) - User preferences
  - [`.vscode/launch.json`](#vscodelaunch.json) - VSCode compatibility
- [Launch Configuration](#launch-configuration)
  - [Priority Order](#priority-order)
  - [launch.toml Format](#launchtoml-format)
  - [launch.json Compatibility](#launchjson-compatibility)
  - [Auto-Start Behavior](#auto-start-behavior-1)
  - [User Preferences](#user-preferences-settingslocaltoml)
  - [Creating Configurations](#creating-configurations)
- [Global Settings Reference](#global-settings-reference)
  - [Behavior Settings](#behavior-settings)
  - [Watcher Settings](#watcher-settings)
  - [UI Settings](#ui-settings)
  - [DevTools Settings](#devtools-settings)
  - [Editor Settings](#editor-settings)
- [Launch Configuration Reference](#launch-configuration-reference)
  - [Configuration Properties](#configuration-properties)
  - [Flutter Modes](#flutter-modes)
  - [Device Selection](#device-selection)
  - [Dart Defines](#dart-defines)
- [VSCode Integration](#vscode-integration)
- [Editor Detection](#editor-detection)
- [Examples](#examples)
- [Settings Panel](#settings-panel)
  - [Overview](#overview-1)
  - [Tab Navigation](#tab-navigation)
  - [Editing Settings](#editing-settings)
  - [Saving Changes](#saving-changes)
  - [User Preferences vs Project Settings](#user-preferences-vs-project-settings)
- [Best Practices](#best-practices)

---

## Overview

Flutter Demon uses a hierarchical configuration system:

1. **Global Settings** (`.fdemon/config.toml`) - Application-wide behavior, UI preferences, and file watcher settings
2. **Launch Configurations** (`.fdemon/launch.toml`) - Predefined run configurations with device targets, build modes, and dart-defines
3. **VSCode Compatibility** (`.vscode/launch.json`) - Automatically imports existing VSCode launch configurations

All configuration files are optional. Flutter Demon works out-of-the-box with sensible defaults.

---

## Configuration Files

Flutter Demon uses three configuration files in the `.fdemon/` directory:

### File Overview

| File | Purpose | Tracked in Git? | Editable in Settings Panel? |
|------|---------|-----------------|------------------------------|
| `.fdemon/config.toml` | Project settings (shared with team) | Yes | Yes (Project tab) |
| `.fdemon/launch.toml` | Launch configurations | Yes | Yes (Launch Config tab) |
| `.fdemon/settings.local.toml` | User preferences (local overrides) | No (gitignored) | Yes (User Preferences tab) |
| `.vscode/launch.json` | VSCode launch configurations | Yes | No (read-only view) |

### `.fdemon/config.toml`

Global settings file for Flutter Demon. Create this file in your project root to customize behavior:

```bash
# Initialize with default config
mkdir -p .fdemon
touch .fdemon/config.toml
```

**Location:** `<project_root>/.fdemon/config.toml`

**Team Sharing:** This file should be committed to version control and shared across the team for consistent behavior.

### `.fdemon/launch.toml`

Launch configurations for different build targets and environments. Similar to VSCode's launch.json but TOML-based.

```bash
# Initialize with default launch config
mkdir -p .fdemon
touch .fdemon/launch.toml
```

**Location:** `<project_root>/.fdemon/launch.toml`

**Team Sharing:** This file should be committed to version control to share launch configurations across the team.

### `.fdemon/settings.local.toml`

User-specific preferences that override project settings. This file is automatically added to `.gitignore` when created.

**Location:** `<project_root>/.fdemon/settings.local.toml`

**Privacy:** This file is gitignored and should NOT be committed. It's for your personal preferences only.

**Example:**
```toml
# User-specific preferences (not tracked in git)

[editor]
command = "nvim"
open_pattern = "nvim +$LINE $FILE"

# Theme override
theme = "dark"
```

> **Note:** Only specific settings can be overridden locally. Not all project settings are available in user preferences. The settings panel (User Preferences tab) shows which settings can be overridden.

### `.vscode/launch.json`

Flutter Demon automatically reads VSCode launch configurations for seamless integration. No migration needed!

**Location:** `<project_root>/.vscode/launch.json`

> **Note:** Only configurations with `"type": "dart"` are imported. View these in the settings panel's VSCode Config tab (read-only).

---

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

---

## Global Settings Reference

### Behavior Settings

Control general application behavior.

```toml
[behavior]
auto_start = false      # If true, skip device selector and use first available device
confirm_quit = true     # Show confirmation dialog when quitting with active sessions
```

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `auto_start` | `boolean` | `false` | If `false`, shows device selector on startup. If `true`, automatically starts on the first available device. |
| `confirm_quit` | `boolean` | `true` | If `true`, shows confirmation dialog when quitting with running apps. If `false`, quits immediately. |

### Watcher Settings

Configure the file watcher for automatic hot reload.

```toml
[watcher]
paths = ["lib"]              # Directories to watch (relative to project root)
debounce_ms = 500            # Milliseconds to wait before triggering reload
auto_reload = true           # Enable automatic hot reload on file changes
extensions = ["dart"]        # File extensions to monitor
```

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `paths` | `array<string>` | `["lib"]` | List of directories to watch for changes, relative to project root. |
| `debounce_ms` | `integer` | `500` | Debounce delay in milliseconds. Prevents reload spam when multiple files change rapidly. |
| `auto_reload` | `boolean` | `true` | If `true`, automatically triggers hot reload when watched files change. |
| `extensions` | `array<string>` | `["dart"]` | File extensions to monitor. Only files with these extensions trigger reload. |

**Example:** Watch both `lib` and `test` directories with 1-second debounce:

```toml
[watcher]
paths = ["lib", "test"]
debounce_ms = 1000
auto_reload = true
extensions = ["dart"]
```

### UI Settings

Customize the terminal user interface.

```toml
[ui]
icons = "nerd_fonts"            # Icon style: "nerd_fonts" (default) or "unicode"
log_buffer_size = 10000         # Maximum log entries to keep in memory
show_timestamps = true          # Display timestamps in log entries
compact_logs = false            # Collapse similar consecutive log entries
theme = "default"               # Color theme name
stack_trace_collapsed = true    # Start stack traces collapsed by default
stack_trace_max_frames = 3     # Number of frames to show when collapsed
```

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `icons` | `string` | `"nerd_fonts"` | Icon style for the TUI. `"nerd_fonts"` uses rich Nerd Font glyphs (requires a [Nerd Font](https://www.nerdfonts.com/) installed in your terminal). `"unicode"` uses safe characters that work in all terminals. |
| `log_buffer_size` | `integer` | `10000` | Maximum number of log entries to retain. Older entries are discarded when limit is reached. |
| `show_timestamps` | `boolean` | `true` | If `true`, displays timestamps for each log entry. |
| `compact_logs` | `boolean` | `false` | If `true`, collapses similar consecutive log entries to reduce noise. |
| `theme` | `string` | `"default"` | Color theme name. Currently only `"default"` is supported. |
| `stack_trace_collapsed` | `boolean` | `true` | If `true`, stack traces start collapsed showing only the first few frames. |
| `stack_trace_max_frames` | `integer` | `3` | Number of stack trace frames to show when collapsed. Press `Enter` to expand. |

> **Environment variable override:** Set `FDEMON_ICONS=unicode` or `FDEMON_ICONS=nerd_fonts` to override the config file setting for the current session.

> **No Nerd Font?** If icons appear as missing characters or boxes, your terminal font does not include Nerd Font glyphs. Add `icons = "unicode"` to your `[ui]` section in `.fdemon/config.toml`, or run with `FDEMON_ICONS=unicode` to switch to safe Unicode characters that work in all terminals. See [nerdfonts.com](https://www.nerdfonts.com/) to install a patched font.

### DevTools Settings

Configure Flutter DevTools integration.

```toml
[devtools]
auto_open = false          # Automatically open DevTools when app starts
browser = ""               # Browser command (empty = system default)
```

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `auto_open` | `boolean` | `false` | If `true`, automatically opens DevTools in a browser when the app starts. |
| `browser` | `string` | `""` | Browser command to use (e.g., `"chrome"`, `"firefox"`). Empty string uses system default. |

### Editor Settings

Configure editor integration for opening files from stack traces.

```toml
[editor]
command = ""                                    # Editor command (empty = auto-detect)
open_pattern = "$EDITOR $FILE:$LINE"          # Pattern for opening files
```

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `command` | `string` | `""` (auto-detect) | Editor command name or path. Leave empty for automatic detection. |
| `open_pattern` | `string` | `"$EDITOR $FILE:$LINE"` | Pattern for opening files at specific lines. Available variables: `$EDITOR`, `$FILE`, `$LINE`, `$COLUMN`. |

#### Auto-Detection

When `command` is empty, Flutter Demon automatically detects your editor in this order:

1. **Parent IDE** - If running inside an IDE's integrated terminal (VS Code, Cursor, Zed, IntelliJ, Android Studio), uses that IDE with `--reuse-window` flag
2. **`$VISUAL`** environment variable
3. **`$EDITOR`** environment variable
4. **Common editors in PATH** - Checks for: `code`, `cursor`, `zed`, `nvim`, `vim`, `emacs`, `subl`, `idea`

#### Supported Editors

| Editor | Command | Default Pattern |
|--------|---------|-----------------|
| **Visual Studio Code** | `code` | `code --reuse-window --goto $FILE:$LINE:$COLUMN` |
| **Cursor** | `cursor` | `cursor --reuse-window --goto $FILE:$LINE:$COLUMN` |
| **Zed** | `zed` | `zed $FILE:$LINE` |
| **Neovim** | `nvim` | `nvim +$LINE $FILE` |
| **Vim** | `vim` | `vim +$LINE $FILE` |
| **Emacs** | `emacs` | `emacs +$LINE:$COLUMN $FILE` |
| **Sublime Text** | `subl` | `subl $FILE:$LINE:$COLUMN` |
| **IntelliJ IDEA** | `idea` | `idea --line $LINE $FILE` |

#### Custom Editor Example

```toml
[editor]
command = "code"
open_pattern = "code --goto $FILE:$LINE:$COLUMN"
```

For Neovim with remote editing:

```toml
[editor]
command = "nvim"
open_pattern = "nvim --server $NVIM --remote-send '<Esc>:e +$LINE $FILE<CR>'"
```

---

## Launch Configuration Reference

Launch configurations define how to run your Flutter app with specific settings.

### Basic Structure

```toml
[[configurations]]
name = "Configuration Name"
device = "auto"
mode = "debug"
flavor = "development"
entry_point = "lib/main_dev.dart"
auto_start = false

[configurations.dart_defines]
API_URL = "https://dev.example.com"
DEBUG_MODE = "true"

[[configurations]]
name = "Another Configuration"
# ... more configs
```

### Configuration Properties

| Property | Type | Default | Required | Description |
|----------|------|---------|----------|-------------|
| `name` | `string` | - | ✅ Yes | Display name for this configuration |
| `device` | `string` | `"auto"` | No | Target device (see [Device Selection](#device-selection)) |
| `mode` | `string` | `"debug"` | No | Flutter build mode: `"debug"`, `"profile"`, or `"release"` |
| `flavor` | `string` | `null` | No | Build flavor (e.g., `"development"`, `"production"`) |
| `entry_point` | `string` | `null` | No | Entry point file path (defaults to `lib/main.dart`) |
| `dart_defines` | `table` | `{}` | No | Key-value pairs passed as `--dart-define` flags |
| `extra_args` | `array<string>` | `[]` | No | Additional arguments passed to `flutter run` |
| `auto_start` | `boolean` | `false` | No | If `true`, starts automatically when Flutter Demon launches |

### Flutter Modes

The `mode` property controls the build optimization level:

| Mode | Flag | Description | Use Case |
|------|------|-------------|----------|
| `debug` | `--debug` | Full debugging support, assertions enabled, slower performance | Development, debugging |
| `profile` | `--profile` | Some optimizations, performance profiling enabled | Performance testing |
| `release` | `--release` | Full optimizations, no debugging, fastest performance | Production builds |

### Device Selection

The `device` property accepts:

- **`"auto"`** - Use first available device
- **Platform prefix** - Match by platform (e.g., `"ios"`, `"android"`, `"macos"`, `"windows"`, `"linux"`)
- **Partial device ID** - Match device by partial ID (e.g., `"iphone"`, `"pixel"`, `"emulator"`)
- **Exact device ID** - Match specific device (e.g., `"00008020-001A3422367A002E"`)

**Examples:**

```toml
device = "auto"              # First available
device = "ios"               # Any iOS device/simulator
device = "android"           # Any Android device/emulator
device = "iphone"            # Matches "iPhone 15 Pro"
device = "pixel"             # Matches "Pixel 8"
device = "chrome"            # Web on Chrome
```

### Dart Defines

Pass compile-time constants to your Dart code:

```toml
[configurations.dart_defines]
API_URL = "https://api.example.com"
API_KEY = "sk_test_12345"
FEATURE_FLAG_X = "true"
DEBUG_MODE = "false"
```

Access in Dart:

```dart
const apiUrl = String.fromEnvironment('API_URL', defaultValue: 'https://default.com');
const apiKey = String.fromEnvironment('API_KEY');
const featureEnabled = bool.fromEnvironment('FEATURE_FLAG_X', defaultValue: false);
```

### Extra Arguments

Pass additional arguments to `flutter run`:

```toml
extra_args = [
    "--verbose",
    "--no-sound-null-safety",
    "--enable-experiment=macros",
    "--dart-define-from-file=config.json"
]
```

Common arguments:
- `--verbose` - Verbose logging
- `--no-sound-null-safety` - Disable sound null safety
- `--obfuscate` - Obfuscate code (release mode)
- `--split-debug-info=<dir>` - Extract debug symbols
- `--enable-experiment=<name>` - Enable Dart experiments
- `--dart-define-from-file=<path>` - Load dart-defines from JSON file

---

## VSCode Integration

Flutter Demon automatically imports `.vscode/launch.json` configurations.

### Supported Properties

Only configurations with `"type": "dart"` are imported:

```json
{
  "version": "0.2.0",
  "configurations": [
    {
      "name": "Development",
      "type": "dart",
      "request": "launch",
      "program": "lib/main_dev.dart",
      "deviceId": "iphone",
      "flutterMode": "debug",
      "toolArgs": [
        "--dart-define=API_URL=https://dev.example.com",
        "--flavor=development"
      ]
    }
  ]
}
```

### Mapping Table

| VSCode Property | Flutter Demon Property | Notes |
|-----------------|------------------------|-------|
| `name` | `name` | Configuration display name |
| `program` | `entry_point` | Entry point file (default: `lib/main.dart`) |
| `deviceId` | `device` | Target device (default: `"auto"`) |
| `flutterMode` | `mode` | Build mode: `"debug"`, `"profile"`, `"release"` |
| `toolArgs` | Parsed into `dart_defines`, `flavor`, `extra_args` | See [Tool Args Parsing](#tool-args-parsing) |

### Tool Args Parsing

The `toolArgs` array is parsed to extract:

**Dart Defines:**
```json
"toolArgs": ["--dart-define=KEY=value"]
```
→ `dart_defines.KEY = "value"`

**Flavor:**
```json
"toolArgs": ["--flavor=production"]
```
→ `flavor = "production"`

**Other Arguments:**
```json
"toolArgs": ["--verbose", "--no-sound-null-safety"]
```
→ `extra_args = ["--verbose", "--no-sound-null-safety"]`

### Comments in JSON

Flutter Demon supports JSONC (JSON with Comments) just like VSCode:

```json
{
  "version": "0.2.0",
  "configurations": [
    // Development configuration
    {
      "name": "Dev",
      /* 
       * Multi-line comment
       * for configuration details
       */
      "type": "dart",
      "request": "launch"
    }
  ]
}
```

### Auto-Start Behavior

VSCode-imported configurations **never** auto-start, even if you set `auto_start: true` in VSCode. This prevents unexpected launches when switching between VSCode and Flutter Demon.

---

## Editor Detection

Flutter Demon intelligently detects your editor to open files from stack traces.

### Detection Priority

1. **Parent IDE Detection** - Most reliable method
   - Checks `$TERM_PROGRAM` environment variable
   - Checks `$ZED_TERM` for Zed
   - Checks `$VSCODE_IPC_HOOK_CLI` for VSCode/Cursor
   - Checks `$TERMINAL_EMULATOR` for JetBrains IDEs
   - Checks `$NVIM` for Neovim terminal

2. **Environment Variables**
   - `$VISUAL` - Preferred editor
   - `$EDITOR` - Fallback editor

3. **PATH Search**
   - Searches for known editors: `code`, `cursor`, `zed`, `nvim`, `vim`, `emacs`, `subl`, `idea`

### IDE Instance Reuse

When running inside an IDE's integrated terminal, Flutter Demon automatically:

- Detects the parent IDE
- Uses the correct command with `--reuse-window` flag (VSCode, Cursor)
- Opens files in the **current** IDE instance, not a new window

**Detected IDEs:**
- Visual Studio Code (`TERM_PROGRAM=vscode`)
- VS Code Insiders (`TERM_PROGRAM=vscode-insiders`)
- Cursor (`TERM_PROGRAM=cursor`)
- Zed (`TERM_PROGRAM=Zed` or `ZED_TERM` set)
- IntelliJ IDEA (`TERMINAL_EMULATOR=JetBrains-*`)
- Android Studio (JetBrains terminal with `IDEA_INITIAL_DIRECTORY` containing `AndroidStudio`)
- Neovim terminal (`NVIM` environment variable set)

---

## Examples

### Complete `.fdemon/config.toml` Example

```toml
[behavior]
auto_start = false
confirm_quit = true

[watcher]
paths = ["lib", "packages/core/lib"]
debounce_ms = 500
auto_reload = true
extensions = ["dart"]

[ui]
icons = "nerd_fonts"
log_buffer_size = 15000
show_timestamps = true
compact_logs = false
theme = "default"
stack_trace_collapsed = true
stack_trace_max_frames = 3

[devtools]
auto_open = false
browser = ""

[editor]
command = ""  # Auto-detect
open_pattern = "$EDITOR $FILE:$LINE"
```

### Multi-Environment `.fdemon/launch.toml` Example

```toml
# Development environment
[[configurations]]
name = "Dev (iOS)"
device = "iphone"
mode = "debug"
flavor = "development"
entry_point = "lib/main_dev.dart"
auto_start = false

[configurations.dart_defines]
API_URL = "https://dev.api.example.com"
API_KEY = "sk_dev_12345"
DEBUG_MODE = "true"
ENABLE_LOGGING = "true"

# Development environment - Android
[[configurations]]
name = "Dev (Android)"
device = "android"
mode = "debug"
flavor = "development"
entry_point = "lib/main_dev.dart"

[configurations.dart_defines]
API_URL = "https://dev.api.example.com"
API_KEY = "sk_dev_12345"
DEBUG_MODE = "true"

# Staging environment
[[configurations]]
name = "Staging"
device = "auto"
mode = "profile"
flavor = "staging"
entry_point = "lib/main_staging.dart"

[configurations.dart_defines]
API_URL = "https://staging.api.example.com"
API_KEY = "sk_staging_12345"
DEBUG_MODE = "false"

# Production environment
[[configurations]]
name = "Production"
device = "auto"
mode = "release"
flavor = "production"
entry_point = "lib/main_prod.dart"
extra_args = [
    "--obfuscate",
    "--split-debug-info=build/symbols"
]

[configurations.dart_defines]
API_URL = "https://api.example.com"
API_KEY = "sk_prod_12345"
DEBUG_MODE = "false"
ANALYTICS_ENABLED = "true"
```

### Monorepo Configuration

For Flutter apps in a monorepo:

```toml
[watcher]
paths = [
    "lib",
    "../shared/lib",
    "../../packages/core/lib"
]
debounce_ms = 750
auto_reload = true
extensions = ["dart"]

[ui]
log_buffer_size = 20000

[[configurations]]
name = "Mobile App"
device = "auto"
mode = "debug"

[[configurations]]
name = "Admin Dashboard"
device = "chrome"
mode = "debug"
entry_point = "lib/main_admin.dart"
```

### VSCode Compatibility Example

Existing `.vscode/launch.json`:

```json
{
  "version": "0.2.0",
  "configurations": [
    {
      "name": "flutter-demon (development)",
      "type": "dart",
      "request": "launch",
      "program": "lib/main.dart",
      "deviceId": "iphone",
      "flutterMode": "debug",
      "toolArgs": [
        "--flavor=development",
        "--dart-define=API_URL=https://dev.example.com",
        "--dart-define=FEATURE_X=true"
      ]
    },
    {
      "name": "flutter-demon (release)",
      "type": "dart",
      "request": "launch",
      "deviceId": "android",
      "flutterMode": "release",
      "toolArgs": [
        "--flavor=production",
        "--dart-define=API_URL=https://api.example.com",
        "--obfuscate"
      ]
    }
  ]
}
```

Flutter Demon automatically imports these as launch configurations!

### Custom Editor Integration

For a custom editor or terminal-based workflow:

```toml
[editor]
command = "nvim"
open_pattern = "nvim --server /tmp/nvim.sock --remote-send '<Esc>:e +$LINE $FILE<CR>'"
```

For IntelliJ IDEA with custom project:

```toml
[editor]
command = "idea"
open_pattern = "idea --line $LINE /path/to/project/$FILE"
```

---

## Settings Panel

Flutter Demon provides a built-in settings panel for managing all configuration options without editing TOML files directly. Access it by pressing `,` (comma) from normal mode.

### Overview

The settings panel provides four tabs:

1. **Project Settings** - Edit `.fdemon/config.toml` (shared with team)
2. **User Preferences** - Edit `.fdemon/settings.local.toml` (personal overrides)
3. **Launch Config** - Manage `.fdemon/launch.toml` configurations
4. **VSCode Config** - View `.vscode/launch.json` (read-only)

### Tab Navigation

- **Tab / Shift+Tab**: Cycle through tabs
- **1-4**: Jump directly to a specific tab
- **j/k or arrow keys**: Navigate settings within a tab
- **Enter/Space**: Edit the selected setting

### Editing Settings

Different setting types have different editing behaviors:

#### Booleans
- **Enter/Space**: Toggle between `true` and `false`
- Example: `auto_start`, `confirm_quit`, `show_timestamps`

#### Numbers
- **+/=**: Increment by 1
- **-**: Decrement by 1
- **0-9**: Type a number directly
- **Backspace**: Delete last digit
- Example: `debounce_ms`, `log_buffer_size`

#### Strings
- **Type normally**: Add characters to the string
- **Backspace**: Delete the last character
- **Delete**: Clear the entire buffer
- Example: `editor.command`, `editor.open_pattern`

#### Enums
- **Enter/Space** or **→**: Cycle to next option
- **←**: Cycle to previous option
- Example: `mode` (debug/profile/release), `theme`, `icons` (unicode/nerd_fonts)

#### Lists
- **Enter**: Add a new item (after typing)
- **d**: Remove the last item
- Example: `watcher.paths`, `watcher.extensions`

### Saving Changes

- **Ctrl+S**: Save changes to the current tab's configuration file
- **Esc**: Close settings panel (prompts if unsaved changes)
- Changes are written to the appropriate file:
  - Project tab → `.fdemon/config.toml`
  - User Preferences tab → `.fdemon/settings.local.toml`
  - Launch Config tab → `.fdemon/launch.toml`

### User Preferences vs Project Settings

The **User Preferences** tab allows you to override specific project settings locally:

**Available Overrides:**
- **Editor command**: Your preferred editor (e.g., `nvim`, `code`)
- **Editor open pattern**: Custom file opening pattern
- **Theme**: UI color theme override
- **Icons**: Icon style override (useful if your terminal has Nerd Font support but teammates don't)

**How Overrides Work:**
- Overrides are stored in `.fdemon/settings.local.toml`
- This file is automatically gitignored
- Overridden settings are marked with a ⚡ indicator
- Project defaults are shown as dimmed fallbacks

**Example:**
If the project sets `editor.command = ""` (auto-detect), but you prefer Neovim, set it in User Preferences. Your local override takes precedence without affecting the team's configuration.

### Launch Configuration Management

The **Launch Config** tab displays all launch configurations with their properties:

- **name**: Configuration display name
- **device**: Target device or platform
- **mode**: Build mode (debug/profile/release)
- **flavor**: Optional build flavor
- **auto_start**: Whether to start automatically

Each configuration is visually separated with a header. Navigate between configurations using j/k.

### VSCode Config (Read-Only)

The **VSCode Config** tab shows Dart configurations from `.vscode/launch.json`. This is a read-only view for reference.

To edit VSCode configurations, modify `.vscode/launch.json` directly in your editor. Changes will be reflected when you reopen the settings panel.

---

## Best Practices

### 1. Use Launch Configurations for Environments

Instead of manually passing arguments, create launch configurations:

```toml
[[configurations]]
name = "Dev"
mode = "debug"
[configurations.dart_defines]
ENV = "development"

[[configurations]]
name = "Prod"
mode = "release"
[configurations.dart_defines]
ENV = "production"
```

### 2. Keep Sensitive Data Out of Config Files

Don't commit API keys or secrets. Use environment variables:

```toml
[configurations.dart_defines]
API_URL = "https://api.example.com"
# Don't do this: API_KEY = "sk_secret_12345"
```

Instead, pass secrets via command-line or load from a separate file:

```toml
extra_args = ["--dart-define-from-file=secrets.json"]
```

### 3. Adjust Debounce for Your Workflow

- **Fast iterations:** Lower debounce (300ms)
- **Large projects:** Higher debounce (1000ms) to avoid reload spam during batch file changes

### 4. Use `.vscode/launch.json` for Team Sharing

If your team uses VSCode, maintain `.vscode/launch.json` for compatibility. Flutter Demon will automatically import it.

### 5. Set `auto_start` for Common Configurations

For your primary development configuration:

```toml
[[configurations]]
name = "Main Dev"
device = "iphone"
mode = "debug"
auto_start = true  # Starts automatically
```

---

## Troubleshooting

### Configuration Not Loading

**Check file location:**
```bash
ls -la .fdemon/
```

Should show `config.toml` and/or `launch.toml`.

**Check TOML syntax:**
```bash
# Install toml-cli
cargo install toml-cli

# Validate syntax
toml get .fdemon/config.toml
```

### Editor Not Auto-Detected

**Check environment variables:**
```bash
echo $VISUAL
echo $EDITOR
echo $TERM_PROGRAM
```

**Check editor in PATH:**
```bash
which code    # VSCode
which cursor  # Cursor
which zed     # Zed
which nvim    # Neovim
```

**Manually configure:**
```toml
[editor]
command = "code"
open_pattern = "code --goto $FILE:$LINE:$COLUMN"
```

### VSCode Configs Not Importing

**Verify configuration type:**

Ensure `"type": "dart"` is set:
```json
{
  "name": "My Config",
  "type": "dart",  // Must be "dart"
  "request": "launch"
}
```

**Check for JSON syntax errors:**

VSCode allows comments, but invalid JSON structure will fail parsing.

---

## Related Documentation

- [Keyboard Bindings](KEYBINDINGS.md) - All keyboard shortcuts
- [Architecture](ARCHITECTURE.md) - Internal architecture and design patterns
- [README](../README.md) - Getting started and usage guide

---

## Feedback

Have suggestions for configuration options? [Open an issue](https://github.com/edTheGuy00/fdemon/issues) on GitHub!