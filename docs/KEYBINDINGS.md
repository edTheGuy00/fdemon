# Keyboard Bindings

This document provides a comprehensive reference of all keyboard controls available in Flutter Demon, organized by context and functionality.

---

## Table of Contents

- [Normal Mode](#normal-mode)
  - [General Controls](#general-controls)
  - [Startup State](#startup-state)
  - [Session Management](#session-management)
  - [App Control](#app-control)
  - [Log Navigation](#log-navigation)
  - [Log Filtering](#log-filtering)
  - [Log Search](#log-search)
  - [Error Navigation](#error-navigation)
  - [Stack Trace Interaction](#stack-trace-interaction)
  - [Link Navigation](#link-navigation)
- [New Session Dialog](#new-session-dialog)
  - [General Navigation](#general-navigation)
  - [Target Selector (Left Pane)](#target-selector-left-pane)
  - [Launch Context (Right Pane)](#launch-context-right-pane)
  - [Fuzzy Search Modal](#fuzzy-search-modal)
  - [Dart Defines Modal](#dart-defines-modal)
- [Search Input Mode](#search-input-mode)
- [Link Highlight Mode](#link-highlight-mode)
- [Settings Panel Mode](#settings-panel-mode)
- [Confirm Dialog Mode](#confirm-dialog-mode)
- [Loading Mode](#loading-mode)

---

## Normal Mode

Normal mode is the default mode when viewing logs and managing your Flutter app sessions.

### General Controls

| Key | Action | Description |
|-----|--------|-------------|
| `q` | Quit | Request to quit (may show confirmation dialog if sessions are running) |
| `qq` | Quick Quit | Quick quit shortcut - second `q` confirms the quit dialog |
| `Esc` | Quit | Same as `q` |
| `Ctrl+C` | Force Quit | Emergency exit, bypasses confirmation dialog |
| `c` | Clear Logs | Clear all logs in the current session |

### Startup State

When Flutter Demon starts without auto-start configured, you'll see:
- Status bar: "○ Not Connected"
- Log area: "Press + to start a new session"

Press `+` or `d` to open the Startup Dialog and configure your first session.

### Session Management

Flutter Demon supports running up to 9 simultaneous device sessions.

| Key | Action | Description |
|-----|--------|-------------|
| `1`-`9` | Switch Session | Switch to session 1-9 by index |
| `Tab` | Next Session | Cycle to the next session |
| `Shift+Tab` | Previous Session | Cycle to the previous session |
| `x` | Close Session | Close the current session |
| `Ctrl+W` | Close Session | Alternative binding to close current session |
| `+` | Start New Session | Start a new session (shows Startup Dialog if no sessions, Device Selector if sessions exist) |
| `d` | Start New Session | Alternative binding for starting new session |

### App Control

These commands control the Flutter app running in the current session. They are disabled while a reload/restart is in progress.

| Key | Action | Description |
|-----|--------|-------------|
| `r` | Hot Reload | Trigger a hot reload (disabled when busy) |
| `R` | Hot Restart | Trigger a hot restart (disabled when busy) |
| `s` | Stop App | Stop the running app (disabled when busy) |

### Log Navigation

#### Vertical Scrolling

| Key | Action | Description |
|-----|--------|-------------|
| `j` | Scroll Down | Move down one line (vim-style) |
| `↓` | Scroll Down | Move down one line |
| `k` | Scroll Up | Move up one line (vim-style) |
| `↑` | Scroll Up | Move up one line |
| `g` | Go to Top | Jump to the beginning of logs |
| `G` | Go to Bottom | Jump to the end of logs |
| `Home` | Go to Top | Alternative binding |
| `End` | Go to Bottom | Alternative binding |
| `Page Up` | Page Up | Scroll up one page |
| `Page Down` | Page Down | Scroll down one page |

#### Horizontal Scrolling

| Key | Action | Description |
|-----|--------|-------------|
| `h` | Scroll Left | Move left 10 characters (vim-style) |
| `←` | Scroll Left | Move left 10 characters |
| `l` | Scroll Right | Move right 10 characters (vim-style) |
| `→` | Scroll Right | Move right 10 characters |
| `0` | Line Start | Jump to the start of the line |
| `$` | Line End | Jump to the end of the line |

### Log Filtering

Filter logs by level or source to focus on relevant information.

| Key | Action | Description |
|-----|--------|-------------|
| `f` | Cycle Level Filter | Cycle through: All → Errors → Warnings → Info → Debug |
| `F` | Cycle Source Filter | Cycle through: All → App → Daemon → Flutter → Watcher |
| `Ctrl+F` | Reset Filters | Clear all active filters |

### Log Search

Search for patterns in logs using regex (vim-style search).

| Key | Action | Description |
|-----|--------|-------------|
| `/` | Start Search | Enter search input mode to type a query |
| `n` | Next Match | Jump to the next search match (only when search active) |
| `N` | Previous Match | Jump to the previous search match |

### Error Navigation

Quickly jump between error messages in the logs.

| Key | Action | Description |
|-----|--------|-------------|
| `e` | Next Error | Jump to the next error log entry |
| `E` | Previous Error | Jump to the previous error log entry |

### Stack Trace Interaction

Expand or collapse stack traces for error log entries.

| Key | Action | Description |
|-----|--------|-------------|
| `Enter` | Toggle Stack Trace | Expand/collapse the stack trace of the focused entry (if available) |

### Link Navigation

Open file references from logs in your configured editor.

| Key | Action | Description |
|-----|--------|-------------|
| `L` | Enter Link Mode | Highlight all file references with shortcut badges |

Once in link highlight mode, see [Link Highlight Mode](#link-highlight-mode) for selection controls.

### Settings

Access the settings panel to configure Flutter Demon and manage launch configurations.

| Key | Action | Description |
|-----|--------|-------------|
| `,` | Open Settings Panel | Open the full-screen settings panel |

Once in settings panel mode, see [Settings Panel Mode](#settings-panel-mode) for detailed controls.

---

## New Session Dialog

The New Session Dialog is the central interface for launching Flutter sessions. It appears when starting Flutter Demon (if `auto_start = false`) or when pressing `d` to add a new session.

The dialog has a two-pane layout:
- **Target Selector** (left): Choose a device or boot an emulator
- **Launch Context** (right): Configure launch settings (configuration, mode, flavor, dart-defines)

### General Navigation

| Key | Action | Description |
|-----|--------|-------------|
| `Tab` | Switch Pane | Switch focus between Target Selector and Launch Context |
| `1` | Connected Tab | Switch to Connected devices tab |
| `2` | Bootable Tab | Switch to Bootable devices tab |
| `Esc` | Close | Close modal (if open), or close dialog (if sessions exist) |
| `Ctrl+C` | Force Quit | Emergency exit |

### Target Selector (Left Pane)

When the Target Selector pane is focused:

| Key | Action | Description |
|-----|--------|-------------|
| `↑` / `k` | Navigate Up | Move up in device list |
| `↓` / `j` | Navigate Down | Move down in device list |
| `Enter` | Select/Boot | Select device (Connected tab) or Boot device (Bootable tab) |
| `r` | Refresh | Refresh device list |

### Launch Context (Right Pane)

When the Launch Context pane is focused:

| Key | Action | Description |
|-----|--------|-------------|
| `↑` / `k` | Previous Field | Navigate to previous field |
| `↓` / `j` | Next Field | Navigate to next field |
| `Enter` | Activate/Launch | Open selector modal for current field, or Launch if on Launch button |
| `←` | Previous Mode | Change to previous mode (when Mode field focused) |
| `→` | Next Mode | Change to next mode (when Mode field focused) |

**Fields:**
- **Configuration**: Opens fuzzy search modal to select or create a launch configuration
- **Mode**: Cycles through Debug → Profile → Release
- **Flavor**: Opens fuzzy search modal to select or enter custom flavor
- **Dart Defines**: Opens Dart Defines modal for key-value editing

### Fuzzy Search Modal

The fuzzy search modal appears when selecting Configuration or Flavor. Type to filter items or enter a custom value.

| Key | Action | Description |
|-----|--------|-------------|
| Type | Filter/Input | Filter existing items or enter custom value |
| `↑` | Previous Item | Navigate to previous filtered result |
| `↓` | Next Item | Navigate to next filtered result |
| `Enter` | Confirm | Select highlighted item or use custom text |
| `Esc` | Cancel | Close modal without changes |
| `Backspace` | Delete Char | Delete last character from query |

### Dart Defines Modal

The Dart Defines modal appears when editing Dart Defines. It has two panes: List (left) and Edit (right).

| Key | Action | Description |
|-----|--------|-------------|
| `Tab` | Switch Pane | Switch between List and Edit panes |
| `↑` | Previous Item | Navigate up in list (List pane) |
| `↓` | Next Item | Navigate down in list (List pane) |
| `Enter` | Action | Load item for editing (List) / Save (Edit) / Delete (Edit) |
| `Esc` | Save & Close | Save all changes and close modal |

**In Edit Pane:**

The Edit pane has a focus cycle: Key field → Value field → Save button → Delete button

| Key | Action | Description |
|-----|--------|-------------|
| `Tab` | Next Focus | Cycle through: Key → Value → Save → Delete |
| Type | Input | Edit Key or Value field (when focused) |
| `Enter` | Next/Activate | Move to next field or activate button |
| `Backspace` | Delete Char | Delete last character (when editing field) |

### Config Editability

The editability of fields depends on the configuration source:

| Config Source | Mode | Flavor | Dart Defines |
|---------------|------|--------|--------------|
| **VSCode** | Read-only | Read-only | Read-only |
| **FDemon** | Editable (auto-saves) | Editable (auto-saves) | Editable (auto-saves) |
| **None** | Editable (transient) | Editable (transient) | Editable (transient) |

When a VSCode config is selected, fields show "(from config)" and cannot be modified. When an FDemon config is selected, changes are automatically saved to `.fdemon/launch.toml`.

---

## Search Input Mode

When you press `/` in normal mode, you enter search input mode to type your query.

| Key | Action | Description |
|-----|--------|-------------|
| `Esc` | Cancel Search | Exit search input mode, keep the current query |
| `Enter` | Submit Search | Exit search input mode, keep the query active |
| `Backspace` | Delete Character | Remove the last character from the query |
| `Ctrl+U` | Clear Input | Clear the entire search query |
| `a`-`z`, `A`-`Z`, `0`-`9` | Type Character | Add character to the search query |
| `Ctrl+C` | Force Quit | Emergency exit from Flutter Demon |

---

## Link Highlight Mode

When you press `L` in normal mode, all file references in the visible viewport are highlighted with shortcut badges.

| Key | Action | Description |
|-----|--------|-------------|
| `Esc` | Exit Link Mode | Return to normal mode |
| `L` | Exit Link Mode | Toggle off link highlight mode |
| `1`-`9` | Open Link | Open the file reference labeled 1-9 |
| `a`-`z` | Open Link | Open the file reference labeled 10-35 (a=10, b=11, etc.) |
| `j` / `↓` | Scroll Down | Scroll down while in link mode |
| `k` / `↑` | Scroll Up | Scroll up while in link mode |
| `Page Up` | Page Up | Scroll up one page |
| `Page Down` | Page Down | Scroll down one page |
| `Ctrl+C` | Force Quit | Emergency exit from Flutter Demon |

> **Note:** The `j` and `k` keys are used for scrolling, not for selecting links.

---

## Settings Panel Mode

The settings panel provides a tabbed interface for managing project settings, user preferences, launch configurations, and viewing VSCode configurations.

### General Controls

| Key | Action | Description |
|-----|--------|-------------|
| `Esc` | Close Settings | Close the settings panel and return to normal mode |
| `q` | Close Settings | Same as `Esc` |
| `Ctrl+C` | Force Quit | Emergency exit from Flutter Demon |
| `Ctrl+S` | Save Settings | Save changes to the current tab's configuration file |

### Tab Navigation

| Key | Action | Description |
|-----|--------|-------------|
| `Tab` | Next Tab | Move to the next settings tab |
| `Shift+Tab` | Previous Tab | Move to the previous settings tab |
| `1` | Jump to Project | Jump to Project Settings tab (config.toml) |
| `2` | Jump to User | Jump to User Preferences tab (settings.local.toml) |
| `3` | Jump to Launch | Jump to Launch Config tab (launch.toml) |
| `4` | Jump to VSCode | Jump to VSCode Config tab (launch.json, read-only) |

### Item Navigation

| Key | Action | Description |
|-----|--------|-------------|
| `j` / `↓` | Next Setting | Move to the next setting in the current tab |
| `k` / `↑` | Previous Setting | Move to the previous setting in the current tab |

### Editing Values

| Key | Action | Description |
|-----|--------|-------------|
| `Enter` | Edit / Toggle | Edit the selected setting (or toggle for booleans/enums) |
| `Space` | Edit / Toggle | Same as `Enter` |
| `Esc` | Cancel Edit | Cancel editing and discard changes (when editing) |
| `Enter` | Commit Edit | Save the edited value (when editing strings/numbers) |

### Value-Specific Controls

#### Boolean Values
| Key | Action | Description |
|-----|--------|-------------|
| `Enter` / `Space` | Toggle | Toggle between true and false |

#### Number Values
| Key | Action | Description |
|-----|--------|-------------|
| `+` / `=` | Increment | Increase the number by 1 |
| `-` | Decrement | Decrease the number by 1 |
| `0`-`9` | Type Digit | Type a number directly |
| `Backspace` | Delete Character | Remove the last digit |

#### String Values
| Key | Action | Description |
|-----|--------|-------------|
| `a`-`z`, etc. | Type Character | Add character to the string |
| `Backspace` | Delete Character | Remove the last character |
| `Delete` | Clear Buffer | Clear the entire edit buffer |

#### Enum Values
| Key | Action | Description |
|-----|--------|-------------|
| `Enter` / `Space` | Cycle Next | Move to the next enum option |
| `→` | Cycle Next | Same as `Enter` |
| `←` | Cycle Previous | Move to the previous enum option |

#### List Values
| Key | Action | Description |
|-----|--------|-------------|
| `Enter` | Add Item | Add a new item to the list (after typing) |
| `d` | Remove Item | Remove the last item from the list |
| `Backspace` | Delete Character | Remove the last character while typing |

---

## Confirm Dialog Mode

When quitting with active sessions, a confirmation dialog appears.

| Key | Action | Description |
|-----|--------|-------------|
| `y` / `Y` | Confirm | Confirm and quit Flutter Demon |
| `q` | Confirm | Confirm quit (enables "qq" quick quit pattern) |
| `Enter` | Confirm | Same as `y` |
| `n` / `N` | Cancel | Cancel quit and return to normal mode |
| `Esc` | Cancel | Same as `n` |
| `Ctrl+C` | Force Quit | Emergency exit, bypasses confirmation |

---

## Loading Mode

While Flutter Demon is initializing or loading.

| Key | Action | Description |
|-----|--------|-------------|
| `q` | Quit | Quit Flutter Demon |
| `Esc` | Quit | Same as `q` |
| `Ctrl+C` | Force Quit | Emergency exit |

---

## Tips

- **Vim-style Navigation**: Flutter Demon uses vim-style keybindings (`hjkl`, `gg`, `G`, etc.) for efficient keyboard-only navigation.
- **Emergency Exit**: `Ctrl+C` always forces an immediate quit in any mode.
- **Multi-Device Workflow**: Use number keys `1`-`9` for quick switching between up to 9 simultaneous sessions.
- **File Opening**: Link mode automatically detects your editor from environment variables (`$VISUAL`, `$EDITOR`) or common IDEs in your terminal.

---

## Configuration

Keyboard behavior can be customized via `.fdemon/config.toml`. See the main [README](../README.md#configuration) for configuration options.

For editor integration and file opening patterns, configure the `[editor]` section in your config file.