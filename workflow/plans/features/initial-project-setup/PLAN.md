# Flutter Demon - Comprehensive Development Plan

> **The High-Performance, Headless "Sidecar" for Flutter Development**

A standalone Terminal User Interface (TUI) for managing Flutter development sessions, designed to run alongside lightweight editors (Zed, Neovim, Helix) to provide "Grade A" Flutter tooling without the bloat of a heavy IDE.

---

## TL;DR

Flutter Demon is a Rust-based TUI application that wraps the `flutter run --machine` command, providing a rich terminal interface for Flutter development. It communicates with the Flutter daemon via JSON-RPC over stdin/stdout, enabling hot reload, device selection, log viewing, and DevTools integration—all from a dedicated terminal window that's independent of your code editor.

---

## Research Findings

### Flutter Daemon Protocol

The Flutter daemon operates using **JSON-RPC 2.0** over **stdin/stdout**. When running `flutter run --machine` or `flutter attach --machine`, a subset of daemon functionality is exposed for programmatic control.

**Protocol Reference:** Flutter daemon v3.38.5 (protocol version 0.6.1)
See: https://github.com/flutter/flutter/blob/main/packages/flutter_tools/doc/daemon.md

**Protocol Changelog (relevant):**
- v0.6.1: Added `coldBoot` option to `emulator.launch` command
- v0.6.0: Added `debounce` option to `app.restart` command
- v0.5.3: Added `emulatorId` field to device
- v0.5.2: Added `platformType` and `category` fields to emulator
- v0.5.1: Added `platformType`, `ephemeral`, and `category` fields to device

#### Protocol Format
- All messages wrapped in square brackets `[]`
- Single-line JSON-RPC format
- Requests have `method`, `id`, and optional `params`
- Responses have `id` and `result` or `error`
- Events have `event` and `params` fields

#### Available Domains (for `--machine` mode)

**daemon domain:**
- Commands: `version()`, `shutdown()`
- Events: `connected`, `log`, `logMessage`

**app domain:**
- Commands: `restart()`, `callServiceExtension()`, `detach()`, `stop()`
- Events: `start`, `debugPort`, `started`, `log`, `progress`, `stop`, `webLaunchUrl`, `devTools`, `dtd`

**device domain:**
- Commands: `getDevices()`, `enable()`, `disable()`, `forward()`, `unforward()`
- Events: `added`, `removed`

**emulator domain:**
- Commands: `getEmulators()`, `launch(emulatorId, coldBoot?)`, `create(name?)`

**devtools domain:**
- Commands: `serve()`

#### Device Object Fields (v3.38.5)

| Field | Type | Description |
|-------|------|-------------|
| `id` | String | Unique device identifier |
| `name` | String | Human-readable device name |
| `platform` | String | Platform identifier (e.g., "ios", "android") |
| `category` | String? | "mobile", "web", "desktop", or null (v0.5.1+) |
| `platformType` | String? | "android", "ios", "linux", "macos", "fuchsia", "windows", "web" (v0.5.1+) |
| `ephemeral` | bool | True if device needs manual connection (v0.5.1+) |
| `emulator` | bool | Whether this is an emulator/simulator |
| `emulatorId` | String? | Matches ID from `emulator.getEmulators` (v0.5.3+) |

#### Key Events for TUI Implementation

| Event | Description | Use Case |
|-------|-------------|----------|
| `app.start` | App is starting | Show "Launching..." status |
| `app.started` | App launched successfully | Enable hot reload buttons |
| `app.log` | Application output | Display in log console |
| `app.progress` | Operation progress | Show loading indicators |
| `app.debugPort` | VM service available | Store for DevTools |
| `app.devTools` | DevTools URI available | Enable DevTools button |
| `app.warning` | App-specific warning | Show warning notifications |
| `device.added` | Device connected | Update device list |
| `device.removed` | Device disconnected | Update device list |

---

### Technology Stack & Crate Versions

#### Core Dependencies

| Crate | Latest Version | Purpose |
|-------|----------------|---------|
| `ratatui` | `0.30.0` | Terminal UI framework |
| `crossterm` | `0.29.x` | Terminal manipulation backend |
| `tokio` | `1.x` | Async runtime for process/IO |
| `serde` | `1.x` | Serialization framework |
| `serde_json` | `1.0.x` | JSON parsing for daemon protocol |
| `notify` | `8.2.0` | File system watching |
| `notify-debouncer-full` | `0.6.0` | Debounced file events |

#### Error Handling & Logging

| Crate | Latest Version | Purpose |
|-------|----------------|---------|
| `color-eyre` | `0.6.3` | Rich error reports with colors |
| `thiserror` | `2.x` | Derive macro for custom errors |
| `tracing` | `0.1.x` | Structured logging |
| `tracing-subscriber` | `0.3.22` | Log subscriber with filtering |

#### Additional Utilities

| Crate | Version | Purpose |
|-------|---------|---------|
| `clap` | `4.x` | CLI argument parsing |
| `toml` | `0.8.x` | Config file parsing |
| `dirs` | `5.x` | Platform-specific directories |
| `open` | `5.x` | Open URLs in default browser |
| `regex` | `1.x` | Stack trace parsing |
| `chrono` | `0.4.x` | Timestamp formatting |

---

### Ratatui Architecture Patterns

#### Application Loop Pattern
```rust
fn main() -> Result<()> {
    color_eyre::install()?;
    let terminal = ratatui::init();
    let result = App::default().run(terminal);
    ratatui::restore();
    result
}

impl App {
    fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        while !self.should_quit {
            terminal.draw(|frame| frame.render_widget(&mut self, frame.area()))?;
            self.handle_events()?;
        }
        Ok(())
    }
}
```

#### Key Widgets for Flutter Demon
- **Block**: Bordered containers with titles
- **Paragraph**: Log output with scrolling
- **List**: Device/emulator selection
- **Tabs**: Multiple app sessions
- **Gauge**: Progress indicators
- **Scrollbar**: Log navigation

#### Layout System
- `Layout::vertical()` and `Layout::horizontal()` for splitting areas
- `Constraint::Percentage`, `Constraint::Min`, `Constraint::Length` for sizing
- `Flex::Center` for centering elements

---

### Async Architecture Considerations

Since `ratatui` is not inherently async but we need to:
1. Read Flutter daemon stdout (async)
2. Write to Flutter daemon stdin (async)
3. Watch files for changes (async)
4. Handle terminal events (sync/async)

**Recommended Pattern**: Use `tokio` with channels to bridge async operations to the main TUI loop.

```
┌─────────────────────────────────────────────────────────────┐
│                      Main TUI Thread                        │
│  ┌──────────┐   ┌──────────┐   ┌──────────┐                │
│  │ Render   │───│ Events   │───│ State    │                │
│  │ Loop     │   │ Handler  │   │ Update   │                │
│  └──────────┘   └──────────┘   └──────────┘                │
│        ▲              ▲              ▲                      │
│        │              │              │                      │
│   ┌────┴──────────────┴──────────────┴────┐                │
│   │           mpsc::channel               │                │
│   └────┬──────────────┬──────────────┬────┘                │
└────────┼──────────────┼──────────────┼──────────────────────┘
         │              │              │
    ┌────┴────┐    ┌────┴────┐    ┌────┴────┐
    │ Flutter │    │ File    │    │ Terminal│
    │ Process │    │ Watcher │    │ Events  │
    │ Task    │    │ Task    │    │ Task    │
    └─────────┘    └─────────┘    └─────────┘
```

---

## Project Architecture

This project follows **Clean Architecture** principles adapted for Rust, combined with **The Elm Architecture (TEA)** pattern recommended by Ratatui for TUI applications.

### Architectural Principles

1. **Layered Architecture**: Clear separation between domain, application, infrastructure, and presentation layers
2. **The Elm Architecture (TEA)**: Model-Update-View pattern for predictable state management
3. **Trait-based Abstractions**: Dependency injection via traits for testability
4. **Library + Binary Split**: Core logic in `lib.rs`, thin entry point in `main.rs`
5. **Feature-based Organization**: Modules organized by domain/feature, not by type

### Layer Responsibilities

```
┌─────────────────────────────────────────────────────────────────┐
│                    Presentation Layer (tui/)                    │
│         Terminal handling, widgets, rendering (View)            │
├─────────────────────────────────────────────────────────────────┤
│                    Application Layer (app/)                     │
│         State management, event handling (Model + Update)       │
├─────────────────────────────────────────────────────────────────┤
│                   Infrastructure Layer                          │
│    ┌─────────────┐  ┌─────────────┐  ┌─────────────┐           │
│    │   daemon/   │  │   watcher/  │  │   config/   │           │
│    │   Flutter   │  │    File     │  │   Config    │           │
│    │   Process   │  │   System    │  │   Files     │           │
│    └─────────────┘  └─────────────┘  └─────────────┘           │
├─────────────────────────────────────────────────────────────────┤
│                      Core Layer (core/)                         │
│         Domain types, events, commands (pure, no deps)          │
└─────────────────────────────────────────────────────────────────┘
```

### The Elm Architecture (TEA) Implementation

```rust
// Message: All possible events/actions in the application
enum Message {
    TerminalEvent(crossterm::event::Event),
    DaemonEvent(DaemonEvent),
    FileChanged(PathBuf),
    Tick,
    Quit,
}

// Model: Complete application state
struct Model {
    app_state: AppState,
    logs: LogBuffer,
    devices: Vec<Device>,
    // ...
}

// Update: Pure state transitions
fn update(model: &mut Model, message: Message) -> Option<Message> {
    match message {
        Message::Quit => { model.app_state = AppState::Quitting; None }
        // ...
    }
}

// View: Pure rendering (no side effects)
fn view(frame: &mut Frame, model: &Model) {
    // Render widgets based on model state
}
```

### Module Structure

```
src/
├── lib.rs                      # Library crate root - exports public API
├── main.rs                     # Binary entry point (thin wrapper)
│
├── core/                       # Domain Layer - Pure business logic
│   ├── mod.rs                  #   Module exports
│   ├── events.rs               #   Domain event definitions
│   ├── commands.rs             #   Command type definitions
│   ├── device.rs               #   Device/emulator types
│   └── types.rs                #   Shared domain types (AppId, etc.)
│
├── app/                        # Application Layer - State & Logic
│   ├── mod.rs                  #   Module exports + App struct
│   ├── state.rs                #   Model (application state)
│   ├── message.rs              #   Message enum (all events)
│   ├── handler.rs              #   Update function (state transitions)
│   └── runner.rs               #   Main application loop
│
├── tui/                        # Presentation Layer - Terminal UI
│   ├── mod.rs                  #   Module exports
│   ├── terminal.rs             #   Terminal setup/restore/panic hook
│   ├── event.rs                #   Terminal event polling
│   ├── render.rs               #   Main view function
│   ├── layout.rs               #   Screen layout definitions
│   ├── theme.rs                #   Colors and styling
│   └── widgets/                #   Custom widget components
│       ├── mod.rs              #     Widget exports
│       ├── header.rs           #     Header bar widget
│       ├── log_view.rs         #     Scrollable log widget
│       ├── status_bar.rs       #     Status bar widget
│       └── device_list.rs      #     Device selector (Phase 3)
│
├── daemon/                     # Infrastructure - Flutter Process
│   ├── mod.rs                  #   Module exports
│   ├── process.rs              #   Process spawning & management
│   ├── protocol.rs             #   JSON-RPC message parsing
│   ├── session.rs              #   FlutterSession trait (extensible)
│   └── client.rs               #   High-level daemon client API
│
├── flutter/                    # Infrastructure - Flutter SDK
│   ├── mod.rs                  #   Module exports
│   └── sdk.rs                  #   SDK detection & version info
│
├── watcher/                    # Infrastructure - File Watching
│   ├── mod.rs                  #   Module exports
│   └── file_watcher.rs         #   Debounced file change events
│
├── config/                     # Infrastructure - Configuration
│   ├── mod.rs                  #   Module exports
│   ├── settings.rs             #   Configuration struct
│   └── loader.rs               #   Config file loading
│
├── editor/                     # Infrastructure - Editor Integration
│   ├── mod.rs                  #   EditorLauncher trait + registry
│   ├── zed.rs                  #   Zed editor support
│   └── generic.rs              #   Generic $EDITOR fallback
│
└── common/                     # Shared Utilities
    ├── mod.rs                  #   Module exports + prelude
    ├── error.rs                #   Error types (thiserror)
    ├── logging.rs              #   Tracing setup
    └── result.rs               #   Result type alias
```

### Key Design Patterns

#### 1. Prelude Pattern for Common Imports

```rust
// src/common/mod.rs
pub mod error;
pub mod logging;
pub mod result;

/// Prelude for common imports
pub mod prelude {
    pub use super::error::{Error, Result};
    pub use tracing::{debug, error, info, trace, warn};
}
```

Usage in other modules:
```rust
use crate::common::prelude::*;
```

#### 2. Trait-based Dependency Injection

```rust
// src/daemon/session.rs
pub trait FlutterSession: Send + Sync {
    async fn start(&mut self) -> Result<()>;
    async fn stop(&mut self) -> Result<()>;
    async fn reload(&mut self, full: bool) -> Result<()>;
}

// Allows mocking for tests and future AttachSession
```

#### 3. Message-driven Architecture

```rust
// All async tasks communicate via channels
pub enum AppEvent {
    Terminal(TerminalEvent),
    Daemon(DaemonEvent),
    FileChange(PathBuf),
    Signal(Signal),
}

// Single receiver in main loop processes all events
```

#### 4. Widget as Stateless + External State

```rust
// Widget configuration is separate from state
pub struct LogView<'a> {
    logs: &'a [LogEntry],
    title: &'a str,
}

pub struct LogViewState {
    scroll_offset: usize,
    auto_scroll: bool,
}

// Render with: frame.render_stateful_widget(widget, area, &mut state);
```

### Module Visibility Rules

| Layer | Can Depend On |
|-------|---------------|
| `main.rs` | `lib.rs` public API only |
| `tui/` | `app/`, `core/`, `common/` |
| `app/` | `core/`, `daemon/`, `watcher/`, `config/`, `common/` |
| `daemon/` | `core/`, `common/` |
| `watcher/` | `core/`, `common/` |
| `config/` | `core/`, `common/` |
| `core/` | `common/` only (or std) |
| `common/` | External crates only |

### Testing Strategy

```
tests/
├── integration/                # Integration tests
│   ├── mod.rs
│   └── daemon_test.rs
│
src/
├── core/
│   └── events.rs              # Unit tests in #[cfg(test)] mod tests
├── daemon/
│   └── protocol.rs            # Unit tests for parsing
└── app/
    └── handler.rs             # Unit tests for state transitions
```

- **Unit tests**: In-module with `#[cfg(test)]`
- **Integration tests**: In `tests/` directory
- **Mock traits**: Use trait objects or generics for testability

---

## Development Phases

### Phase 1: Foundation (Proof of Concept)

**Goal**: Prove we can spawn Flutter, communicate via JSON-RPC, and display output in a TUI.

**Duration**: 1-2 weeks

#### Steps

1. **Project Setup**
   - Initialize Cargo.toml with all dependencies
   - Set up project structure with module stubs
   - Configure `color-eyre` for error handling
   - Set up `tracing` with file output (TUI conflicts with stdout logging)

2. **Basic TUI Shell**
   - Implement ratatui initialization/restoration
   - Create main application loop with quit handling (Ctrl+C, 'q')
   - Display static "Flutter Demon" header
   - Create bordered log area (empty for now)

3. **Flutter Process Spawning**
   - Use `tokio::process::Command` to spawn `flutter run --machine`
   - Pipe stdout/stderr for reading
   - Pipe stdin for command injection
   - Implement graceful process termination

4. **Raw Output Display**
   - Read daemon stdout line by line
   - Strip `[]` wrappers and display raw JSON in log area
   - Handle process exit gracefully

**Milestone Deliverable**: A TUI that spawns Flutter, shows raw JSON-RPC output, and cleanly exits.

---

### Phase 1.1: Flutter Project Discovery

**Goal**: Automatically discover **runnable** Flutter projects when running from a parent directory, filtering out plugins/packages that cannot be run directly, and allow users to select from multiple discovered projects.

**Duration**: 0.5-1 week

**Status**: Added post-Phase 1 to address usability gap

#### Problem Addressed

When running `flutter-demon` from a directory that doesn't directly contain a Flutter project (but has Flutter projects in subdirectories), the app should intelligently search for and offer project selection instead of failing.

Additionally, users working on **Flutter plugins** may have a `pubspec.yaml` at the root, but it's not a runnable target—the runnable example is typically in `example/` or `sample/` subdirectories.

#### Project Type Classification

| Type | Runnable? | Detection |
|------|-----------|-----------|
| **Flutter Application** | ✅ Yes | Has `flutter: sdk` dependency, has platform dirs, NO `flutter: plugin:` section |
| **Flutter Plugin** | ❌ No | Has `flutter: plugin: platforms:` section in pubspec.yaml |
| **Flutter Package** | ❌ No | Has `flutter: sdk` dependency but no platform directories |
| **Dart Package** | ❌ No | No `flutter: sdk` dependency, pure Dart code |

For **plugins**, we recursively check `example/` and `sample/` subdirectories for runnable targets.

#### Steps

1. **Discovery Module** (`core/discovery.rs`)
   - Implement recursive search for `pubspec.yaml` files
   - **Parse pubspec.yaml** to detect project type (app, plugin, package)
   - **Check for platform directories** (android/, ios/, macos/, web/, linux/, windows/)
   - **Filter out non-runnable projects** (plugins, packages, Dart-only)
   - **For plugins**: Check `example/` and `sample/` subdirectories for runnable targets
   - Configurable max depth (default: 3 levels)
   - Skip hidden directories (`.git`, `.dart_tool`, etc.)
   - Skip build/dependency directories (`build/`, `node_modules/`)
   - Return sorted list of discovered **runnable** project paths

2. **Project Selector** (`tui/selector.rs`)
   - Simple numbered menu using crossterm (pre-TUI)
   - Single-keypress selection (1-9)
   - Cancel with 'q', Escape, or Ctrl+C
   - Display paths relative to searched directory
   - Plugin examples shown as `my_plugin/example`

3. **Integration Flow** (`main.rs`)
   - Priority 1: Check if PWD is a **runnable** Flutter project
   - Priority 2: If plugin/package detected at PWD, explain and search for runnable targets
   - Priority 3: Search subdirectories for runnable projects
   - Auto-select if exactly one runnable project found
   - Show selector if multiple runnable projects found
   - Helpful error if no runnable projects found (with explanation of requirements)

4. **Testing & Documentation**
   - Unit tests for discovery including plugin/package detection
   - Integration tests for monorepo scenarios with mixed project types
   - Update README with discovery usage and project type table

**Milestone Deliverable**: Running `flutter-demon` from any directory intelligently finds runnable Flutter projects, handles plugins by finding their examples, and skips non-runnable packages.

**Task Details**: See [phase_1_1/TASKS.md](phase_1_1/TASKS.md)

---

### Phase 2: Protocol Integration (Basic Control)

**Goal**: Parse daemon protocol and implement hot reload/restart commands.

**Duration**: 2-3 weeks

#### Steps

1. **JSON-RPC Message Types**
   - Define `DaemonMessage` enum (Request, Response, Event)
   - Implement serde deserialization with `#[serde(untagged)]`
   - Create typed structures for each event type
   - Handle `app.log`, `daemon.logMessage`, `app.progress` events

2. **Parsed Log Display**
   - Filter and format `app.log` messages
   - Color-code error messages (red)
   - Add timestamps to log entries
   - Implement log scrolling with keyboard (j/k or arrows)

3. **Command Injection**
   - Implement `send_command()` function
   - Track request IDs and match responses
   - Bind 'r' key to `app.restart` (hot reload)
   - Bind 'R' key to `app.restart { fullRestart: true }` (hot restart)

4. **Status Bar**
   - Extract app state from events
   - Display: "Launching...", "Running", "Reloading...", "Stopped"
   - Show device name and app ID
   - Show last reload time

5. **File Watcher Integration**
   - Use `notify-debouncer-full` to watch `lib/` folder
   - Debounce with 500ms delay
   - Trigger hot reload on file save
   - Show "Auto-reloading..." status briefly

**Milestone Deliverable**: A functional development TUI with hot reload, status display, and auto-reload on save.

---

### Phase 3: Device & Launch Management (Cockpit UI)

**Goal**: Add device selection, launch configuration, and refined UI layout.

**Duration**: 2-3 weeks

#### Steps

1. **Device Discovery**
   - Query `device.getDevices` on startup
   - Enable device polling with `device.enable`
   - Handle `device.added` and `device.removed` events
   - Store device list in application state

2. **Device Selector UI**
   - Create modal/popup device list using `List` widget
   - Show device name, platform, and emulator status
   - Allow selection with arrow keys and Enter
   - Store selected device ID for launch

3. **Emulator Management**
   - Query `emulator.getEmulators`
   - Add option to launch emulators
   - Cold boot option for Android emulators

4. **Launch Configuration**
   - Parse `fdemon.toml` configuration file
   - Support: device ID, flavor, dart-defines, additional args
   - Optionally parse `.vscode/launch.json` for compatibility
   - Command-line argument support via `clap`

5. **Refined UI Layout**
   ```
   ┌─────────────────────────────────────────────────────────┐
   │  Flutter Demon    [r] Reload  [R] Restart  [d] DevTools │
   ├─────────────────────────────────────────────────────────┤
   │                                                         │
   │  [12:34:56] App started                                 │
   │  [12:34:57] flutter: Hello from main()                  │
   │  [12:35:01] Reloaded 1 of 423 libraries                │
   │  [12:35:15] flutter: Button pressed                     │
   │                                                         │
   │                                                         │
   ├─────────────────────────────────────────────────────────┤
   │  ● Running on iPhone 15 Pro (ios_simulator)    00:05:23 │
   └─────────────────────────────────────────────────────────┘
   ```

6. **Keyboard Shortcuts**
   - 'q' - Quit (confirm if app running)
   - 'r' - Hot reload
   - 'R' - Hot restart
   - 'd' - Open DevTools
   - 's' - Stop app
   - '/' - Filter logs
   - 'c' - Clear logs

**Milestone Deliverable**: Full device selection, configurable launch, and polished UI layout.

---

### Phase 4: Advanced Features (Polish)

**Goal**: Feature parity with VS Code Flutter extension.

**Duration**: 2-3 weeks

#### Steps

1. **DevTools Integration**
   - Capture DevTools URL from `app.devTools` event
   - Open in default browser using `open` crate
   - Display URL in status bar for manual copy

2. **Log Filtering & Search**
   - Filter modes: All, Errors, Network, Print statements
   - Regex search within logs
   - Highlight matched terms
   - Persist filter preference

3. **Error Highlighting**
   - Parse Dart stack traces with regex
   - Highlight file:line references
   - Color-code error severity
   - Collapsible stack trace sections

4. **Pubspec Watcher**
   - Watch `pubspec.yaml` and `pubspec.lock`
   - Detect dependency changes
   - Prompt to run `flutter pub get`
   - Show pub get output in log area

5. **Session Persistence**
   - Remember last used device
   - Remember window position/size (if terminal supports)
   - Log history saved to file (optional)

6. **Mouse Support**
   - Clickable header buttons
   - Scrollable log area with mouse wheel
   - Log entry selection

**Milestone Deliverable**: Production-ready TUI with all essential Flutter development features.

---

### Phase 5: Multi-Session & Extensibility (Future)

**Goal**: Support advanced workflows and extensibility.

**Duration**: 3-4 weeks (future roadmap)

#### Steps

1. **Multiple App Sessions**
   - Tab-based UI for multiple running apps
   - Keyboard shortcuts to switch tabs
   - Independent log buffers per session

2. **Terminal Hyperlinks**
   - OSC 8 escape sequences for clickable links
   - Click file:line to open in editor
   - Configurable editor command

3. **Plugin System**
   - Lua or WASM-based plugins
   - Custom commands and widgets
   - Theme customization

4. **Remote Development**
   - SSH tunnel support
   - Remote Flutter daemon connection
   - Cloud device support

---

## Edge Cases & Risks

### Process Management
- **Risk**: Flutter process becomes orphaned on crash
- **Mitigation**: Register signal handlers (SIGTERM, SIGINT), use process groups, implement cleanup on panic

### Terminal Compatibility
- **Risk**: Different terminals have varying feature support
- **Mitigation**: Use crossterm's capability detection, graceful degradation for unsupported features

### JSON-RPC Parsing
- **Risk**: Malformed or unexpected daemon output
- **Mitigation**: Robust parsing with `serde`, log unparseable lines, don't crash on parse errors

### File Watcher Performance
- **Risk**: Large projects may generate many file events
- **Mitigation**: Use debouncer, watch only `lib/` by default, configurable watch paths

### Cross-Platform Compatibility
- **Risk**: Different behavior on Windows/macOS/Linux
- **Mitigation**: Use cross-platform crates (crossterm, notify), test on all platforms, conditional compilation where needed

### Stdin/Stdout Conflicts
- **Risk**: TUI and daemon both use terminal
- **Mitigation**: Run daemon as child process with piped IO, TUI owns the terminal exclusively

---

## Configuration File Format

```toml
# fdemon.toml

[app]
# Target device ID (from `flutter devices`)
device = "emulator-5554"

# Build flavor
flavor = "development"

# Additional dart-defines
dart_defines = ["API_URL=https://api.dev.example.com"]

# Additional flutter run arguments
extra_args = ["--no-sound-null-safety"]

[watcher]
# Paths to watch for hot reload (relative to project root)
paths = ["lib/", "assets/"]

# Debounce delay in milliseconds
debounce_ms = 500

# Enable auto-reload on save
auto_reload = true

[ui]
# Log buffer size (number of lines)
log_buffer_size = 10000

# Show timestamps in logs
show_timestamps = true

# Theme: "dark", "light", or "auto"
theme = "dark"

[devtools]
# Auto-open DevTools on app start
auto_open = false

[mcp]
# Enable MCP server for AI agent control (future feature)
enabled = true

# Port for MCP server (Streamable HTTP transport)
port = 3939

# Bind address (localhost only for security)
bind = "127.0.0.1"

# Maximum concurrent MCP sessions
max_sessions = 5

# Session timeout in seconds (0 = no timeout)
session_timeout = 3600
```

---

## Initial Cargo.toml

```toml
[package]
name = "flutter-demon"
version = "0.1.0"
edition = "2024"
description = "A high-performance TUI for Flutter development"
license = "MIT"
repository = "https://github.com/user/flutter-demon"

[dependencies]
# TUI Framework
ratatui = { version = "0.30", features = ["all-widgets"] }
crossterm = "0.29"

# Async Runtime
tokio = { version = "1", features = ["full"] }

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# File Watching
notify = "8.2"
notify-debouncer-full = "0.6"

# Error Handling
color-eyre = "0.6"
thiserror = "2"

# Logging (to file, since TUI uses stdout)
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
tracing-appender = "0.2"

# CLI & Config
clap = { version = "4", features = ["derive"] }
toml = "0.8"
dirs = "5"

# Utilities
open = "5"
regex = "1"
chrono = { version = "0.4", features = ["serde"] }
which = "7"
```

---

## Design Decisions

The following decisions have been made for the initial implementation:

### 1. Flutter Attach Support
**Decision**: Not now, but structure for future support.

We will create an abstraction layer for Flutter session management:

```rust
// src/daemon/session.rs
pub trait FlutterSession: Send + Sync {
    async fn start(&mut self) -> Result<()>;
    async fn stop(&mut self) -> Result<()>;
    async fn reload(&mut self, full_restart: bool) -> Result<()>;
    async fn send_command(&mut self, command: DaemonCommand) -> Result<DaemonResponse>;
}

pub struct RunSession { /* flutter run --machine */ }
pub struct AttachSession { /* flutter attach --machine - future */ }

impl FlutterSession for RunSession { ... }
// impl FlutterSession for AttachSession { ... } // Future
```

### 2. Editor Integration
**Decision**: Prioritize Zed, structure for easy extension.

We will implement an `EditorLauncher` trait for extensible editor support:

```rust
// src/editor/mod.rs
pub mod zed;      // Primary support
// pub mod vscode;  // Future
// pub mod neovim;  // Future

pub trait EditorLauncher: Send + Sync {
    fn name(&self) -> &str;
    fn open_file(&self, path: &Path, line: Option<u32>, column: Option<u32>) -> Result<()>;
    fn is_available(&self) -> bool;
}

// src/editor/zed.rs
pub struct ZedLauncher;

impl EditorLauncher for ZedLauncher {
    fn name(&self) -> &str { "Zed" }
    
    fn open_file(&self, path: &Path, line: Option<u32>, column: Option<u32>) -> Result<()> {
        // zed path/to/file:line:column
        let mut arg = path.display().to_string();
        if let Some(l) = line {
            arg.push_str(&format!(":{}", l));
            if let Some(c) = column {
                arg.push_str(&format!(":{}", c));
            }
        }
        std::process::Command::new("zed").arg(&arg).spawn()?;
        Ok(())
    }
    
    fn is_available(&self) -> bool {
        which::which("zed").is_ok()
    }
}
```

Configuration support in `fdemon.toml`:
```toml
[editor]
# Editor to use for opening files: "zed", "vscode", "neovim", "custom"
name = "zed"

# Custom command template (only used when name = "custom")
# Placeholders: {file}, {line}, {column}
# custom_command = "code -g {file}:{line}:{column}"
```

### 3. Log Persistence
**Decision**: Not for initial release.

Logs will be kept in memory only with a configurable buffer size. This simplifies the initial implementation and avoids privacy/security concerns.

### 4. Flutter SDK Version Display
**Decision**: Show SDK version in status bar.

Implementation approach:
- Run `flutter --version --machine` on startup to get JSON output
- Parse and extract: Flutter version, Dart version, channel
- Display in status bar: `Flutter 3.19.0 (stable) • Dart 3.3.0`
- Detect FVM by checking for `.fvm/flutter_sdk` symlink
- Support configurable Flutter path via `fdemon.toml` or `FLUTTER_ROOT` env var

```rust
// src/flutter/sdk.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlutterSdkInfo {
    pub flutter_version: String,
    pub dart_version: String,
    pub channel: String,
    pub flutter_root: PathBuf,
    pub is_fvm: bool,
}

impl FlutterSdkInfo {
    pub async fn detect() -> Result<Self> {
        // 1. Check config for explicit path
        // 2. Check FLUTTER_ROOT env var
        // 3. Check for .fvm/flutter_sdk symlink
        // 4. Fall back to `which flutter`
        // 5. Run `flutter --version --machine` and parse
    }
    
    pub fn display_string(&self) -> String {
        format!("Flutter {} ({}) • Dart {}", 
            self.flutter_version, 
            self.channel, 
            self.dart_version
        )
    }
}
```

### 5. Daemon/Headless Mode
**Decision**: Not for initial release.

The initial focus is on providing an excellent standalone TUI experience. Headless/daemon mode would require significant additional complexity (IPC protocol, client library, security considerations) and is deferred to a future version.

### 6. MCP Server Integration (Future Groundwork)
**Decision**: Plan now, implement after MVP, adopt patterns during MVP.

Flutter Demon will expose an **MCP (Model Context Protocol) server** to allow AI agents to control Flutter development workflows. This is a priority future feature with its own detailed plan.

**Full Plan**: See [../mcp-server/PLAN.md](../mcp-server/PLAN.md)

**Key Architectural Decisions**:

1. **Transport**: Use **Streamable HTTP** (not stdio) because the TUI owns terminal I/O. MCP server binds to `localhost:3939` alongside the TUI.

2. **SDK**: Use `rmcp` crate (official Rust MCP SDK) with `axum` for HTTP server.

3. **Service Layer Pattern**: Extract shared business logic into `services/` module during Phase 2-3. Both TUI and future MCP handlers will use these services.

```rust
// services/flutter_controller.rs
pub trait FlutterController: Send + Sync {
    async fn reload(&self) -> Result<ReloadResult>;
    async fn restart(&self) -> Result<RestartResult>;
    async fn stop(&self) -> Result<()>;
    async fn get_state(&self) -> AppRunState;
}
```

4. **Shared State**: Use `Arc<RwLock<T>>` for app state, log buffer, devices to enable concurrent access.

5. **Event Broadcasting**: Use `tokio::sync::broadcast` channels so multiple consumers (TUI + future MCP) can subscribe to events.

**MCP Capabilities Planned**:
- **Tools**: `flutter.reload`, `flutter.restart`, `flutter.stop`, `flutter.start`, `flutter.pub_get`, `flutter.clean`, `flutter.open_devtools`
- **Resources**: `flutter://logs`, `flutter://state`, `flutter://devices`, `flutter://project`, `flutter://widget-tree`

---

## UI Layout

With SDK version display in the status bar:

```
┌─────────────────────────────────────────────────────────────────┐
│  🔥 Flutter Demon   [r] Reload  [R] Restart  [s] Stop  [d] Dev  │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  12:34:56  App started on iPhone 15 Pro                         │
│  12:34:57  flutter: Hello from main()                           │
│  12:35:01  Reloaded 1 of 423 libraries in 245ms                │
│  12:35:15  flutter: Button pressed                              │
│  12:35:22  flutter: API response received                       │
│                                                                 │
│                                                                 │
├─────────────────────────────────────────────────────────────────┤
│  ● Running │ iPhone 15 Pro (ios) │ Flutter 3.19.0 │ ⏱ 00:05:23 │
└─────────────────────────────────────────────────────────────────┘
```

Status bar segments:
1. **State indicator**: ● Running (green), ○ Stopped (gray), ↻ Reloading (yellow)
2. **Device info**: Device name and platform
3. **SDK version**: Flutter version (from `flutter --version --machine`)
4. **Session timer**: Time since app started

---

## Future Considerations (Deferred)

These features are explicitly deferred for future versions:

1. **Flutter Attach Mode** - Connect to already-running apps
2. **Additional Editor Support** - VS Code, Neovim, Emacs, Sublime
3. **Log Persistence** - Save logs to disk for post-mortem analysis
4. **Daemon/Headless Mode** - IPC-based communication for editor extensions
5. **Multiple Sessions** - Tab-based UI for running multiple apps
6. **Terminal Hyperlinks** - OSC 8 clickable links in log output
7. **Plugin System** - Lua/WASM extensibility
8. **MCP Server Integration** - Expose Flutter Demon as an MCP (Model Context Protocol) server for AI agent control

### MCP Server Integration (Priority Feature)

**Full Plan**: See [../mcp-server/PLAN.md](../mcp-server/PLAN.md)

Flutter Demon will expose an **MCP server** (using Streamable HTTP transport on localhost) that enables AI agents like Claude, Cursor, and Zed AI to:

- **Tools**: `flutter.reload`, `flutter.restart`, `flutter.stop`, `flutter.start`, `flutter.pub_get`, `flutter.clean`, `flutter.open_devtools`
- **Resources**: `flutter://logs`, `flutter://state`, `flutter://devices`, `flutter://project`, `flutter://widget-tree`

**Architectural Groundwork for MCP-Readiness**:

To minimize future refactoring, the following patterns should be adopted during MVP development:

1. **Service Layer Pattern** (Phase 2+): Create `services/` module with `FlutterController`, `StateService`, `LogService` traits. TUI handlers should use services, not direct daemon access.

2. **Shared State with Arc<RwLock>**: App state, log buffer, and device list should be wrapped in `Arc<RwLock<T>>` for concurrent access by future MCP handlers.

3. **Command/Query Separation**: Define `FlutterCommand` enum (Reload, Restart, Stop, Start) for mutations and queries for reads. Enables audit logging and rate limiting.

4. **Event Broadcasting**: Use `tokio::sync::broadcast` for events so multiple subscribers (TUI + future MCP) can receive notifications.

**Why Streamable HTTP**: The TUI owns stdin/stdout, so MCP cannot use stdio transport. Streamable HTTP binds to `localhost:3939` alongside the TUI.

**Crate**: `rmcp` (official Rust MCP SDK) with `axum` for HTTP server.

---

## Success Criteria

### Phase 1 Complete When:
- [ ] TUI initializes and displays without errors
- [ ] Flutter process spawns and output appears
- [ ] Clean exit with Ctrl+C or 'q'

### Phase 2 Complete When:
- [ ] Parsed, formatted logs display correctly
- [ ] Hot reload (r) and restart (R) work
- [ ] File save triggers auto-reload
- [ ] Status bar shows current app state

### Phase 3 Complete When:
- [ ] Device selection works at startup
- [ ] Configuration file is parsed and applied
- [ ] UI layout matches design specification
- [ ] All keyboard shortcuts functional

### Phase 4 Complete When:
- [ ] DevTools opens in browser
- [ ] Log filtering works with multiple modes
- [ ] Error stack traces are highlighted
- [ ] Pubspec changes trigger pub get prompt

### Project Complete When:
- [ ] Works on macOS, Linux, and Windows
- [ ] Documentation complete (README, man page)
- [ ] CI/CD pipeline with releases
- [ ] Community feedback incorporated