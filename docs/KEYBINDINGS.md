# Keyboard Bindings

This document provides a comprehensive reference of all keyboard controls available in Flutter Demon, organized by context and functionality.

> **Mouse interactions** are documented separately in [MOUSE.md](MOUSE.md), which covers
> wheel-scroll routing, click-to-activate semantics for the header / tabs / dialogs / DevTools,
> and the `[ui] enable_mouse` opt-out.

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
  - [DAP Server](#dap-server)
- [New Session Dialog](#new-session-dialog)
  - [General Navigation](#general-navigation)
  - [Target Selector (Left Pane)](#target-selector-left-pane)
  - [Launch Context (Right Pane)](#launch-context-right-pane)
  - [Fuzzy Search Modal](#fuzzy-search-modal)
  - [Dart Defines Modal](#dart-defines-modal)
- [Search Input Mode](#search-input-mode)
- [Link Highlight Mode](#link-highlight-mode)
- [Settings Panel Mode](#settings-panel-mode)
- [DevTools Mode](#devtools-mode)
  - [Panel Navigation](#panel-navigation)
  - [Debug Overlays](#debug-overlays)
  - [Widget Inspector Panel](#widget-inspector-panel)
  - [Performance Panel](#performance-panel)
  - [Memory Panel](#memory-panel)
  - [Network Panel](#network-panel)
- [Flutter Version Mode](#flutter-version-mode)
  - [General Controls](#general-controls-4)
  - [Pane Navigation](#pane-navigation)
  - [Version List Controls](#version-list-controls-when-installed-versions-pane-is-focused)
- [Install Wizard Mode](#install-wizard-mode)
  - [General Controls](#general-controls-5)
  - [Pane Navigation](#pane-navigation-1)
  - [Step List Controls](#step-list-controls-when-step-list-pane-is-focused)
  - [Detail Pane Controls](#detail-pane-controls-when-detail-pane-is-focused)
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
| `Alt+m` | Toggle mouse capture | Suspend or resume mouse capture (runtime, in-process — see [MOUSE.md](MOUSE.md)). Status bar shows `[mouse]` / `[mouse-off]`. |

### Startup State

When Flutter Demon starts without auto-start configured, you'll see:
- Status bar: "○ Not Connected"
- Log area: "Press + to start a new session"

Press `+` to open the Startup Dialog and configure your first session.

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
| `d` | DevTools Mode | Enter DevTools mode (Inspector/Performance/Network panels) |
| `D` | Toggle DAP Server | Start or stop the DAP debug adapter server |

### App Control

These commands control the Flutter app running in the current session. They are disabled while a reload/restart is in progress.

| Key | Action | Description |
|-----|--------|-------------|
| `r` | Hot Reload | Trigger a hot reload (disabled when busy) |
| `R` | Hot Restart | Trigger a hot restart (disabled when busy). **Context-dependent:** In DevTools Performance with the Rebuild Stats tab focused, `R` instead toggles `ext.flutter.profileWidgetBuilds` (rebuild tracking) — the hot-restart binding is shadowed in that specific context. |
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
| `T` or `t` | Open/close native tag filter overlay | Toggle visibility of individual native platform log tags (Android/iOS/macOS) |
| `w` | Toggle Wrap Mode | Toggle line wrap on/off for the log view |

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

### DevTools

| Key | Action | Description |
|-----|--------|-------------|
| `d` | DevTools Mode | Enter DevTools mode (Inspector/Performance/Network panels) |

Once in DevTools mode, see [DevTools Mode](#devtools-mode) for detailed controls.

### Flutter SDK

| Key | Action | Description |
|-----|--------|-------------|
| `V` | Open Flutter Version Panel | Open the Flutter SDK version manager panel |
| `I` | Open Install Wizard | Open the toolchain install wizard (runs a preflight check) |

Once in Flutter Version mode, see [Flutter Version Mode](#flutter-version-mode) for detailed controls.

Once in Install Wizard mode, see [Install Wizard Mode](#install-wizard-mode) for detailed controls.

### DAP Server

| Key | Action | Description |
|-----|--------|-------------|
| `D` | Toggle DAP Server | Start or stop the DAP debug adapter server. When active, `[DAP :PORT]` appears in the status bar. Connect your IDE's debugger to this port. |

---

## New Session Dialog

The New Session Dialog is the central interface for launching Flutter sessions. It appears when starting Flutter Demon (if `auto_start = false`) or when pressing `+` to add a new session.

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
| `↑` | Navigate Up | Move up in device list |
| `↓` | Navigate Down | Move down in device list |
| `Enter` | Select/Boot | Acknowledge selection (Connected tab) or Boot device (Bootable tab); use Launch Context to launch |
| `Space` | Toggle Selection | Toggle multi-launch checked state for cursor device (Connected tab only) |
| `a` | Select All / Clear | Check all connected devices for multi-launch; clears if all are already checked |
| `r` | Refresh | Refresh device list |

> **Multi-launch resource note:** Confirming with multiple devices checked starts
> one Flutter session per checked device (up to the 9-session limit), each running
> its own `flutter run` build, VM Service connection, and native-log capture.
> Launching many cold-build targets at once can spike CPU/memory and contend for
> build tools (Gradle, Xcode). Check only the devices you need. Sessions launch
> concurrently — there is currently no staggering. Devices already running a
> session are skipped; a toast reports "Launched X of Y" when some are skipped.

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

## DevTools Mode

Enter DevTools mode by pressing `d` in Normal mode (requires VM Service connection).

> **Migration note (2026-05):** the previous Performance panel's memory chart and
> allocation table moved to a new Memory panel (`m`). The `s` (sort) binding
> moved with them.

### Panel Navigation

| Key | Action | Description |
|-----|--------|-------------|
| `Esc` | Exit DevTools | Return to Normal mode (log view). In Performance panel, deselects frame first. In Memory panel, deselects alloc row first. In Network panel, deselects request first. |
| `i` | Inspector Panel | Switch to Widget Inspector panel |
| `p` | Performance Panel | Switch to Performance monitoring panel |
| `m` | Memory Panel | Switch to Memory panel |
| `n` | Network Panel | Switch to Network monitor panel |
| `b` | Browser DevTools | Open Flutter DevTools in system browser |
| `q` | Quit | Quit the application |

> **`b` — Browser DevTools behavior:** On Flutter SDK ≥ 1.22, the Flutter daemon registers
> DevTools with DDS and provides a stable served URL; `b` opens that URL directly in your
> system browser. On older SDKs that do not support the `devtools.serve` daemon command, `b`
> falls back to the legacy DDS-served URL and shows a recovery toast in the status bar.

### Debug Overlays

| Key | Action | Description |
|-----|--------|-------------|
| `Ctrl+r` | Repaint Rainbow | Toggle repaint rainbow overlay on device |
| `Ctrl+p` | Performance Overlay | Toggle performance overlay on device |
| `Ctrl+d` | Debug Paint | Toggle debug paint overlay on device |

### Widget Inspector Panel

The Inspector panel has two modes: **tree mode** (default) and **details mode**
(after pressing `Enter` on a selected widget). Key bindings differ between
modes.

#### Tree mode

| Key | Action |
|-----|--------|
| `Up` / `k` | Move selection up |
| `Down` / `j` | Move selection down |
| `Right` / `l` | Expand node (or expand collapsed group) |
| `Left` / `h` | Collapse node |
| `Enter` | Open Details view for selected widget |
| `Shift+H` | Toggle "Hide implementation widgets" (chain collapsing) |
| `r` | Refresh widget tree |
| `b` | Open Flutter DevTools in browser |
| `Esc` | Exit DevTools → Logs |

#### Details mode

| Key | Action |
|-----|--------|
| `Tab` / `Right` / `l` | Cycle to next visible tab (wraps; skips hidden tabs; no-op when only one tab is visible) |
| `Shift+Tab` / `Left` / `h` | Cycle to previous visible tab |
| `Esc` | Close Details (return to tree mode) |
| `r` | Refresh details |
| `b` | Open Flutter DevTools in browser |
| `Up` / `Down` / `j` / `k` | **No-op** — selection frozen while details is open |

**Details tab visibility:**
- Widget properties: always shown.
- Render object: shown when the selected widget has a render object (e.g. `Padding`, `Column`, `Stack` — not `Container`).
- Flex explorer: shown when the selected widget or its parent is `Row`, `Column`, or `Flex`.

Press `Esc` from Details to return to tree mode; press `Esc` again to exit
DevTools to the log view.

Chain collapsing: when "Hide implementation widgets" is on (default,
`[devtools] hide_implementation_widgets = true` in `.fdemon/config.toml`),
long single-child chains of non-local-project wrapper widgets (e.g. nested
`BlocProvider`s) fold into a single `+ N more widgets` row. Press `Right` on
the leader to expand the chain in place.

The Inspector panel shows a 50/50 split: widget tree on one side, layout explorer on the other. Layout data auto-fetches when a tree node is selected.

### Performance Panel

When the Performance panel is active:

| Key | Action |
|-----|--------|
| `Tab` / `Shift+Tab` | Cycle focus between Frame Chart and Details pane |
| `←` / `→` | Select previous / next frame |
| `]` | Cycle details tab forward (Frame Analysis → Rebuild Stats → Timeline Events) |
| `[` | Cycle details tab backward |
| `↑` / `k` | Scroll focused section up |
| `↓` / `j` | Scroll focused section down |
| `PageUp` / `PageDown` | Page-scroll focused section |
| `Home` / `End` | Jump to oldest / live edge |
| `Esc` | Deselect frame; or, if no frame selected, return to Logs |
| `Ctrl+p` | Toggle performance overlay on device |
| `b` | Open DevTools in browser |
| `f` | Performance, Details, TimelineEvents tab — Cycle filter All → UI → Raster |
| `R` (Shift+r) | Performance, Details, RebuildStats tab — Toggle widget rebuild tracking |

> The `]`/`[` cycle only fires when the Details pane has focus (press `Tab` from the Frame Chart).
>
> The `f` key only fires when the Details pane has focus **and** the Timeline Events tab is active.
>
> The `R` (Shift+r) key only fires when the Details pane has focus **and** the Rebuild Stats tab is active. In all other contexts (Logs panel, Frame Chart focus, Frame Analysis tab, Memory panel, etc.) `R` performs a hot restart as usual.

### Memory Panel

When the Memory panel is active:

| Key | Action |
|-----|--------|
| `Tab` / `Shift+Tab` | Cycle focus between Memory Chart and Allocation List |
| `↑` / `k` | Scroll focused section up |
| `↓` / `j` | Scroll focused section down |
| `PageUp` / `PageDown` | Page-scroll focused section |
| `Home` / `End` | Jump to oldest / live edge of chart, or first / last alloc row |
| `s` | Toggle allocation sort (By Size ↔ By Instances) |
| `Esc` | Deselect alloc row; or, if no row selected, return to Logs |
| `b` | Open DevTools in browser |

### Network Panel

When the Network panel is active:

| Key | Action | Description |
|-----|--------|-------------|
| `Up` / `k` | Navigate Up | Move up in request list |
| `Down` / `j` | Navigate Down | Move down in request list |
| `Page Up` | Page Up | Scroll request list up one page |
| `Page Down` | Page Down | Scroll request list down one page |
| `Enter` | Select / Refetch | Select request and fetch details (or refetch if already selected) |
| `Esc` | Deselect | Clear request selection |
| `Space` | Toggle Recording | Toggle network recording on/off |
| `Ctrl+x` | Clear Requests | Clear all recorded network requests |
| `/` | Filter | Enter filter mode to filter requests by text |
| `g` | General Tab | Switch to General detail sub-tab |
| `h` | Headers Tab | Switch to Headers detail sub-tab |
| `q` | Request Body Tab | Switch to Request Body detail sub-tab |
| `s` | Response Body Tab | Switch to Response Body detail sub-tab |
| `t` | Timing Tab | Switch to Timing detail sub-tab |

The Network panel shows HTTP/HTTPS requests in a scrollable table with detailed inspection.

#### Network Filter Mode

When filter input is active (after pressing `/`):

| Key | Action | Description |
|-----|--------|-------------|
| Type | Input | Type characters to build filter query |
| `Enter` | Apply Filter | Apply the filter and return to normal Network panel |
| `Esc` | Cancel | Discard filter input and return to normal Network panel |
| `Backspace` | Delete | Remove last character from filter |

---

## Flutter Version Mode

Enter Flutter Version mode by pressing `V` in Normal mode. This panel shows the current Flutter SDK info and the list of versions installed in the FVM cache.

The panel has a two-pane layout:
- **SDK Info** (left): Current version, channel, source, SDK path, and bundled Dart version
- **Installed Versions** (right): FVM cache entries with the active version highlighted

### General Controls

| Key | Action | Description |
|-----|--------|-------------|
| `Esc` | Close Panel | Close the Flutter Version panel and return to Normal mode |
| `Ctrl+C` | Force Quit | Emergency exit from Flutter Demon |

### Pane Navigation

| Key | Action | Description |
|-----|--------|-------------|
| `Tab` | Switch Pane | Toggle focus between SDK Info and Installed Versions |

### Version List Controls (when Installed Versions pane is focused)

| Key | Action | Description |
|-----|--------|-------------|
| `k` / `↑` | Navigate Up | Move selection up in the version list |
| `j` / `↓` | Navigate Down | Move selection down in the version list |
| `Enter` | Switch Version | Switch to the selected Flutter SDK version (writes `.fvmrc` in project root) |
| `d` | Remove Version | Delete the selected SDK version from the FVM cache |
| `i` | Install Version | Install the selected Flutter SDK version |
| `u` | Update Version | Update the selected Flutter SDK version |

> **Note:** Switching to the active version or removing the active version are both blocked — the status bar will show an error message.

---

## Install Wizard Mode

Enter Install Wizard mode by pressing `I` in Normal mode. This panel shows the results of a toolchain preflight check, grouped into five ordered steps.

The panel has a two-pane layout:
- **Step List** (left): Five ordered steps with roll-up status indicators
- **Detail** (right): Per-step detail — component checks and embedded doctor output

Preflight runs automatically when the wizard opens. Press `r` to re-run at any time.

> **Guided vs executable steps:** Some steps (such as FlutterSdk and PathConfig) can be run directly by pressing `Enter`. The **Prerequisites step is guided-only** — `Enter` has no effect there. Instead, the wizard shows per-OS install commands (e.g. the Linux package-manager command, macOS Xcode CLT / CocoaPods / Rosetta commands, or Windows Git for Windows). Use `[` / `]` to cycle between commands when a step offers multiple options, copy the selected command with `c`, run it in your terminal, then press `r` to re-check.

### General Controls

| Key | Action | Description |
|-----|--------|-------------|
| `Esc` | Cancel Step / Close Panel | **Context-dependent:** when an install step is currently Running, cancels the in-flight step (signals the `CancellationToken`, resets execution to Idle) and stays in the wizard. When no step is running (idle, completed, or failed), closes the Install Wizard and returns to Normal mode. If Flutter is live when closing, the wizard also triggers device discovery and routes to the startup flow. |
| `Ctrl+C` | Force Quit | Emergency exit from Flutter Demon |

### Pane Navigation

| Key | Action | Description |
|-----|--------|-------------|
| `Tab` | Switch Pane | Toggle focus between Step List and Detail pane |

### Step List Controls (when Step List pane is focused)

| Key | Action | Description |
|-----|--------|-------------|
| `k` / `↑` | Navigate Up | Move selection up in the step list |
| `j` / `↓` | Navigate Down | Move selection down in the step list |
| `Enter` | Run / Retry Step | Run or retry the selected step (Flutter SDK install, Android Tools install — gated on a present JDK 17 — or PATH config write). No-op on guided-only steps (e.g. Prerequisites). |
| `[` | Previous Command | Select the previous guided command on the current step (e.g. cycle backward through macOS Prerequisites: Xcode CLT → Rosetta). No-op when only one command is available. |
| `]` | Next Command | Select the next guided command on the current step (e.g. cycle forward through macOS Prerequisites: Xcode CLT → CocoaPods → Rosetta). No-op when only one command is available. |
| `c` | Copy Selected Guided Command | Copy the currently selected guided command to the clipboard (e.g. the JDK install command or a per-OS prerequisite install command). No-op when the step has no guided command. |
| `r` | Re-run Preflight | Re-run the toolchain preflight check (useful after completing a guided step such as installing JDK 17 or OS prerequisites outside fdemon) |

### Detail Pane Controls (when Detail pane is focused)

| Key | Action | Description |
|-----|--------|-------------|
| `k` / `↑` | Scroll Up | Scroll the detail pane up |
| `j` / `↓` | Scroll Down | Scroll the detail pane down |
| `r` | Re-run Preflight | Re-run the toolchain preflight check |

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