# Phase 3: Device & Launch Management (Cockpit UI) - Task Index

## Overview

**Goal**: Add multi-instance support with tabs, device selection before launch, and configuration file parsing for both `.vscode/launch.json` and `.fdemon/launch.toml`.

**Duration**: 3-4 weeks

**Total Tasks**: 10

This phase transforms Flutter Demon from a single-instance tool into a full "cockpit" for Flutter development with:
- Multiple simultaneous app instances displayed as tabs
- Device discovery and selection UI before launching
- Configuration file support (both VSCode compatibility and native TOML)
- Emulator management (list and launch)
- Refined UI with tabs for multi-session management

---

## Research Findings

### 1. Multi-Instance Support with Ratatui Tabs

The Ratatui `Tabs` widget provides an excellent foundation for multi-instance support:

```rust
use ratatui::widgets::Tabs;
use ratatui::text::Line;

// Create tab titles from session names
let titles: Vec<Line> = sessions.iter()
    .map(|s| format!(" {} ({}) ", s.device_name, s.status_icon()).into())
    .collect();

Tabs::new(titles)
    .select(selected_tab_index)
    .highlight_style(Style::default().fg(Color::Yellow))
    .divider(" │ ")
    .render(area, buf);
```

**Key architectural changes needed:**
- Current `AppState` assumes single instance → refactor to `Session` per tab
- Each session tracks its own: `app_id`, `device_id`, log buffer, phase/state
- `SessionManager` to coordinate multiple sessions
- Tab switching via keyboard shortcuts (`1-9`, `Tab`/`Shift+Tab`, or `H`/`L`)

### 2. Device Discovery Protocol

Based on Flutter daemon protocol v3.38.5 (protocol version 0.6.1).
See: https://github.com/flutter/flutter/blob/main/packages/flutter_tools/doc/daemon.md

**Protocol Changelog (relevant):**
- v0.6.1: Added `coldBoot` option to `emulator.launch` command
- v0.6.0: Added `debounce` option to `app.restart` command
- v0.5.3: Added `emulatorId` field to device
- v0.5.2: Added `platformType` and `category` fields to emulator
- v0.5.1: Added `platformType`, `ephemeral`, and `category` fields to device

| Command | Description | Response |
|---------|-------------|----------|
| `device.getDevices` | Get all connected devices | `[{id, name, platform, category, platformType, ephemeral, emulator, emulatorId}]` |
| `device.enable` | Start device polling | Enables `device.added`/`device.removed` events |
| `device.disable` | Stop device polling | - |
| `emulator.getEmulators` | List available emulators | `[{id, name, category, platformType}]` |
| `emulator.launch` | Start an emulator | `{emulatorId, coldBoot?}` → starts emulator |
| `emulator.create` | Create Android emulator | `{name?}` → `{success, emulatorName, error?}` |

**Device object fields (v3.38.5):**
```json
{
  "id": "702ABC1F-5EA5-4F83-84AB-6380CA91D39A",
  "name": "iPhone 15 Pro",
  "platform": "ios",
  "category": "mobile",
  "platformType": "ios",
  "ephemeral": false,
  "emulator": true,
  "emulatorId": "apple_ios_simulator"
}
```

**Field descriptions:**
- `category`: "mobile", "web", "desktop", or null
- `platformType`: "android", "ios", "linux", "macos", "fuchsia", "windows", "web"
- `ephemeral`: true if device needs manual connection (e.g., physical Android device)
- `emulatorId`: Matches ID from `emulator.getEmulators` (may be null even for emulators if connection failed)

**Two approaches for device discovery:**
1. **`flutter daemon` mode** - Long-running process with event-based updates
2. **`flutter devices --machine`** - One-shot query (simpler, used for initial implementation)

We will use **approach 2** initially for simplicity, with option to upgrade to daemon mode later.

### 3. VSCode launch.json Format for Flutter

The Dart/Flutter VSCode extension uses this format:

```json
{
  "version": "0.2.0",
  "configurations": [
    {
      "name": "My App (development)",
      "type": "dart",
      "request": "launch",
      "program": "lib/main.dart",
      "deviceId": "iphone",
      "flutterMode": "debug",
      "toolArgs": [
        "--dart-define", "API_URL=https://dev.api.com",
        "--flavor", "development"
      ],
      "args": []
    },
    {
      "name": "My App (production)",
      "type": "dart",
      "request": "launch",
      "flutterMode": "release",
      "toolArgs": ["--flavor", "production"]
    }
  ]
}
```

**Relevant fields for Flutter Demon:**
| Field | Type | Description |
|-------|------|-------------|
| `name` | String | Display name for configuration |
| `type` | String | Must be "dart" to be relevant |
| `request` | String | "launch" or "attach" |
| `program` | String | Entry point (default: lib/main.dart) |
| `deviceId` | String | Target device ID |
| `flutterMode` | String | debug/profile/release |
| `toolArgs` | String[] | Args passed to `flutter run` |
| `args` | String[] | Args passed to app's main() |
| `cwd` | String | Working directory |

### 4. Proposed .fdemon/launch.toml Format

Native Flutter Demon configuration with better ergonomics than JSON:

```toml
# .fdemon/launch.toml

[[configurations]]
name = "Development"
device = "auto"                    # "auto" = first available, or specific device ID
mode = "debug"                     # debug | profile | release
flavor = "development"
entry_point = "lib/main.dart"      # Optional, defaults to lib/main.dart
auto_start = true                  # Start this config automatically

[configurations.dart_defines]
API_URL = "https://dev.api.com"
DEBUG_MODE = "true"
SENTRY_DSN = ""

[[configurations]]
name = "Production iOS"
device = "ios"                     # Platform prefix matches any iOS device
mode = "release"
flavor = "production"
extra_args = ["--obfuscate", "--split-debug-info=build/symbols"]

[[configurations]]
name = "Web Debug"
device = "chrome"
mode = "debug"
```

### 5. Proposed .fdemon/config.toml Format

Global settings for Flutter Demon behavior:

```toml
# .fdemon/config.toml

[behavior]
auto_start = false                 # false = show device selector, true = use launch.toml auto_start
confirm_quit = true                # Ask before quitting with running apps

[watcher]
paths = ["lib"]
debounce_ms = 500
auto_reload = true
extensions = ["dart"]

[ui]
log_buffer_size = 10000
show_timestamps = true
compact_logs = false               # Collapse similar consecutive logs
theme = "default"                  # Future: support custom themes

[devtools]
auto_open = false
browser = ""                       # Empty = system default
```

### 6. Configuration Priority Order

When configurations conflict, use this priority (highest to lowest):
1. Command-line arguments
2. `.fdemon/launch.toml`
3. `.vscode/launch.json` (compatibility import)
4. Built-in defaults

---

## Task Dependency Graph

```
                    ┌─────────────────────────────┐
                    │  01-config-module           │
                    │  (types, toml parsing)      │
                    └──────────────┬──────────────┘
                                   │
              ┌────────────────────┼────────────────────┐
              │                    │                    │
              ▼                    ▼                    ▼
┌─────────────────────┐  ┌─────────────────┐  ┌─────────────────────┐
│  02-vscode-import   │  │  03-device-     │  │  04-session-manager │
│  (launch.json)      │  │  discovery      │  │  (multi-instance)   │
└─────────────────────┘  └────────┬────────┘  └──────────┬──────────┘
                                  │                      │
                                  ▼                      │
                    ┌─────────────────────────┐          │
                    │  05-device-selector-ui  │          │
                    │  (modal device picker)  │          │
                    └──────────────┬──────────┘          │
                                   │                     │
                                   ▼                     │
                    ┌─────────────────────────┐          │
                    │  06-delayed-start       │          │
                    │  (wait for selection)   │◄─────────┘
                    └──────────────┬──────────┘
                                   │
                    ┌──────────────┴──────────────┐
                    │                             │
                    ▼                             ▼
     ┌─────────────────────────┐   ┌─────────────────────────┐
     │  07-tabs-widget         │   │  08-emulator-management │
     │  (multi-session tabs)   │   │  (list & launch)        │
     └──────────────┬──────────┘   └─────────────────────────┘
                    │
                    ▼
     ┌─────────────────────────┐
     │  09-refined-layout      │
     │  (cockpit UI polish)    │
     └──────────────┬──────────┘
                    │
                    ▼
     ┌─────────────────────────┐
     │  10-keyboard-shortcuts  │
     │  (tab nav, stop app)    │
     └─────────────────────────┘
```

**Parallelization opportunities:**
- Tasks 02, 03, 04 can be done in parallel after 01
- Tasks 07, 08 can be done in parallel after 06

---

## Tasks

| # | Task | Status | Depends On | Effort | Key Modules |
|---|------|--------|------------|--------|-------------|
| 1 | [01-config-module](tasks/01-config-module.md) | ✅ Done | - | 4-5 hrs | `config/mod.rs`, `config/types.rs`, `config/settings.rs` |
| 2 | [02-vscode-import](tasks/02-vscode-import.md) | ✅ Done | 01 | 2-3 hrs | `config/vscode.rs` |
| 3 | [03-device-discovery](tasks/03-device-discovery.md) | ✅ Done | 01 | 3-4 hrs | `daemon/devices.rs` |
| 4 | [04-session-manager](tasks/04-session-manager.md) | ✅ Done | 01 | 5-6 hrs | `app/session.rs`, `app/session_manager.rs` |
| 5 | [05-device-selector-ui](tasks/05-device-selector-ui.md) | ✅ Done | 03 | 3-4 hrs | `tui/widgets/device_selector.rs` |
| 6 | [06-delayed-start](tasks/06-delayed-start.md) | ✅ Done | 04, 05 | 4-5 hrs | `tui/mod.rs`, `app/mod.rs`, `app/handler.rs`, `app/state.rs` |
| 7 | [07-tabs-widget](tasks/07-tabs-widget.md) | ✅ Done | 06 | 3-4 hrs | `tui/widgets/tabs.rs`, `tui/render.rs` |
| 8 | [08-emulator-management](tasks/08-emulator-management.md) | ✅ Done | 06 | 2-3 hrs | `daemon/emulators.rs` |
| 9 | [09-refined-layout](tasks/09-refined-layout.md) | ✅ Done | 07 | 3-4 hrs | `tui/layout.rs`, `tui/render.rs` |
| 10 | [10-keyboard-shortcuts](tasks/10-keyboard-shortcuts.md) | ✅ Done | 09 | 2-3 hrs | `tui/event.rs`, `app/handler.rs` |

**Total Estimated Effort**: 32-41 hours

---

## Task Summaries

| Task | Description |
|------|-------------|
| **01-config-module** | Create config module with types for launch configurations and settings, TOML parsing for `.fdemon/` directory |
| **02-vscode-import** | Parse `.vscode/launch.json` for Flutter/Dart configurations, convert to internal format |
| **03-device-discovery** | Run `flutter devices --machine` to get device list, parse JSON output into typed structs |
| **04-session-manager** | Create `Session` struct for per-instance state, `SessionManager` to coordinate multiple running apps |
| **05-device-selector-ui** | Modal/popup device list widget with arrow key navigation and Enter to select |
| **06-delayed-start** | Refactor startup flow to show device selector first (unless auto_start configured) |
| **07-tabs-widget** | Add `Tabs` widget to header showing all running sessions with device names |
| **08-emulator-management** | Query `flutter emulators --machine`, add option to launch emulator from device selector |
| **09-refined-layout** | Polish UI layout with tabs, proper spacing, and responsive design |
| **10-keyboard-shortcuts** | Add 's' for stop, '1-9' for tab switching, Tab/Shift+Tab navigation |

---

## New Dependencies Required

```toml
[dependencies]
# TOML parsing (already have serde)
toml = "0.8"

# Optional: For strum derive macros (tab enums)
strum = { version = "0.26", features = ["derive"] }
```

---

## New Module Structure

After Phase 3, the `src/` directory will include:

```
src/
├── config/                         # NEW - Configuration (Tasks 01, 02)
│   ├── mod.rs                      # Re-exports
│   ├── types.rs                    # LaunchConfig, Settings structs
│   ├── settings.rs                 # Parse .fdemon/config.toml
│   ├── launch.rs                   # Parse .fdemon/launch.toml
│   └── vscode.rs                   # Parse .vscode/launch.json
│
├── app/
│   ├── mod.rs                      # Updated entry points
│   ├── handler.rs                  # Extended for multi-session
│   ├── message.rs                  # New session-related messages
│   ├── state.rs                    # Existing (per-session state)
│   ├── session.rs                  # NEW - Session struct (Task 04)
│   └── session_manager.rs          # NEW - SessionManager (Task 04)
│
├── daemon/
│   ├── mod.rs                      # Updated re-exports
│   ├── process.rs                  # Updated for device ID param
│   ├── devices.rs                  # NEW - Device discovery (Task 03)
│   └── emulators.rs                # NEW - Emulator management (Task 08)
│
├── tui/
│   ├── mod.rs                      # Updated event loop
│   ├── render.rs                   # Updated for tabs
│   ├── layout.rs                   # Extended for tabs
│   └── widgets/
│       ├── mod.rs                  # Re-exports
│       ├── header.rs               # Updated for tabs
│       ├── tabs.rs                 # NEW - Session tabs (Task 07)
│       ├── device_selector.rs      # NEW - Device picker (Task 05)
│       ├── log_view.rs             # Existing
│       └── status_bar.rs           # Updated for selected session
│
└── ... (existing modules)
```

---

## Configuration File Locations

```
my_flutter_app/
├── .fdemon/                        # Flutter Demon native config
│   ├── config.toml                 # Global settings
│   └── launch.toml                 # Launch configurations
│
├── .vscode/                        # VSCode (read for compatibility)
│   └── launch.json                 # Import Dart/Flutter configs
│
├── lib/
│   └── main.dart
└── pubspec.yaml
```

**Directory creation**: Flutter Demon should create `.fdemon/` if it doesn't exist, with sensible defaults.

---

## UI Layout After Phase 3

```
┌─────────────────────────────────────────────────────────────────────┐
│  Flutter Demon  │ iPhone 15 │ Pixel 8 │ macOS │    [r] [R] [d] [q]  │
├─────────────────┴───────────┴─────────┴───────┴─────────────────────┤
│                                                                     │
│  [12:34:56] ● flutter: App started                                  │
│  [12:34:57] ○ flutter: Building widget tree...                      │
│  [12:35:01] ● Reloaded 1 of 423 libraries in 234ms                  │
│  [12:35:15] ○ flutter: Button pressed                               │
│  [12:35:16] ✗ flutter: Error: Widget overflow by 42 pixels          │
│                                                                     │
├─────────────────────────────────────────────────────────────────────┤
│  ● Running on iPhone 15 Pro (ios_simulator) │ Reloads: 3 │ 00:05:23 │
└─────────────────────────────────────────────────────────────────────┘
```

**Tab indicators:**
- Active tab: highlighted/underlined
- Running: `●` green
- Stopped: `○` gray
- Error: `✗` red

---

## Device Selector Modal

```
┌─────────────────────────────────────────┐
│           Select Target Device          │
├─────────────────────────────────────────┤
│                                         │
│  ▶ iPhone 15 Pro           (simulator)  │
│    iPhone 15               (simulator)  │
│    Pixel 8                 (emulator)   │
│    macOS                   (desktop)    │
│    Chrome                  (web)        │
│  ──────────────────────────────────     │
│    + Launch Android Emulator...         │
│    + Launch iOS Simulator...            │
│                                         │
├─────────────────────────────────────────┤
│  ↑↓ Navigate  Enter Select  Esc Cancel  │
└─────────────────────────────────────────┘
```

---

## Edge Cases & Risks

| Risk | Mitigation |
|------|------------|
| No devices available | Show helpful message, offer to launch emulator |
| `flutter devices` hangs | 10s timeout, show error, allow retry |
| Invalid launch.json | Log warning, skip invalid entries, continue |
| Invalid TOML config | Show parse error with line number, use defaults |
| Device disconnects during run | Detect via daemon events, update session state |
| Multiple auto_start configs | Start all marked auto_start, or first if only one allowed |
| Tab overflow (many sessions) | Truncate names, add `<` `>` scroll indicators |
| Session cleanup on crash | Ensure FlutterProcess kill_on_drop works |
| Config file permissions | Handle read errors gracefully |
| Mixed platform configs | Allow, but show platform mismatch warning |

---

## Success Criteria

Phase 3 is complete when:

- [ ] `.fdemon/config.toml` is parsed and applied on startup
- [ ] `.fdemon/launch.toml` configurations are loaded
- [ ] `.vscode/launch.json` Dart/Flutter configs are imported
- [ ] Device list is fetched via `flutter devices --machine`
- [ ] Device selector modal is displayed before first launch
- [ ] Multiple app instances can run simultaneously
- [ ] Tabs widget shows all running sessions
- [ ] Tab switching works with keyboard shortcuts
- [ ] Emulator list is available in device selector
- [ ] Emulator can be launched from device selector
- [ ] 's' key stops the currently selected app
- [ ] Status bar reflects selected session's state
- [ ] All new code has unit tests
- [ ] `cargo test` passes
- [ ] `cargo clippy` has no warnings

---

## Clarifying Questions (Resolved)

| Question | Decision |
|----------|----------|
| Run `flutter daemon` or `flutter devices`? | Use `flutter devices --machine` initially (simpler) |
| Multi-instance: same app or different apps? | Same app on multiple devices (for now) |
| Write back to `.vscode/launch.json`? | No, read-only import for compatibility |
| Multiple auto_start configs? | Start all configs marked `auto_start = true` |
| Tab switching keys? | `1-9` for direct access, `Tab`/`Shift+Tab` for next/prev |

---

## Milestone Deliverable

A "cockpit" TUI for Flutter development with:
- Device selection before launch
- Multiple app instances running simultaneously with tabs
- Configuration files for reproducible launch settings
- Emulator management
- VSCode launch.json compatibility

This milestone enables efficient multi-device testing workflows and integrates with existing VSCode configurations.