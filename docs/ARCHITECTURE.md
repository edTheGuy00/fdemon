# Flutter Demon Architecture

This document describes the internal architecture of Flutter Demon, a high-performance TUI for Flutter development written in Rust.

## Table of Contents

- [Overview](#overview)
- [Design Principles](#design-principles)
- [Project Structure](#project-structure)
- [Module Reference](#module-reference)
- [Key Patterns](#key-patterns)
- [Data Flow](#data-flow)
- [Key Types](#key-types)
- [Testing Strategy](#testing-strategy)

---

## Overview

Flutter Demon is a terminal-based Flutter development environment that manages Flutter processes, provides real-time log viewing, and supports multi-device sessions. The application is built with a layered architecture separating concerns between domain logic, infrastructure, and presentation.

```
┌─────────────────────────────────────────────────────────────────┐
│                        Binary (main.rs)                         │
│                   CLI parsing, project discovery                │
└─────────────────────────────────────────────────────────────────┘
                                 │
                                 ▼
┌─────────────────────────────────────────────────────────────────┐
│                       Application Layer                         │
│              State management, message handling (TEA)           │
│                         (app/, services/)                       │
└─────────────────────────────────────────────────────────────────┘
                                 │
              ┌──────────────────┼──────────────────┐
              ▼                  ▼                  ▼
┌───────────────────┐ ┌───────────────────┐ ┌───────────────────┐
│   Presentation    │ │   Infrastructure  │ │      Domain       │
│   (tui/)          │ │   (daemon/)       │ │      (core/)      │
│   Terminal UI     │ │   Process mgmt    │ │   Business types  │
│   Widgets         │ │   JSON-RPC        │ │   Discovery       │
└───────────────────┘ └───────────────────┘ └───────────────────┘
                                 │
                                 ▼
                    ┌───────────────────────┐
                    │    Flutter Process    │
                    │   (flutter run)       │
                    └───────────────────────┘
```

---

## Design Principles

### The Elm Architecture (TEA)

Flutter Demon follows the **TEA pattern** (Model-View-Update) for state management:

1. **Model** (`AppState`) - The complete application state
2. **Messages** (`Message`) - All possible events/actions
3. **Update** (`handler::update`) - Pure function: `(State, Message) → (State, Action)`
4. **View** (`tui::render`) - Renders state to the terminal

This provides:
- Predictable state transitions
- Easy testing (update is pure)
- Clear separation of concerns
- Time-travel debugging potential

### Layered Architecture

Each layer has clear responsibilities and dependencies flow downward:

| Layer | Responsibility | Dependencies |
|-------|----------------|--------------|
| **Binary** | CLI, entry point | All |
| **App** | State, orchestration | Core, Daemon, TUI |
| **Services** | Reusable controllers | Core, Daemon |
| **TUI** | Presentation | Core, App (TEA View pattern) |
| **Daemon** | Flutter process I/O | Core |
| **Core** | Domain types | None |
| **Common** | Utilities | None |

### Layer Dependencies Note

The TUI layer depends on App because of the TEA pattern:
- **View** (`tui::render`) must receive **Model** (`AppState`) to render it
- This is the fundamental TEA contract: `View: State → UI`
- The dependency is intentional and necessary, not a violation

### Error Handling

- Custom `Error` enum with domain-specific variants
- `Result<T>` type alias throughout
- Errors are categorized as `fatal` vs `recoverable`
- Rich error context via `ResultExt` trait

---

## Project Structure

```
src/
├── main.rs              # Binary entry point, CLI handling
├── lib.rs               # Library public API
│
├── common/              # Shared utilities (no dependencies)
│   ├── error.rs         # Error types and Result alias
│   ├── logging.rs       # File-based logging setup
│   ├── signals.rs       # SIGINT/SIGTERM handling
│   └── prelude.rs       # Common imports
│
├── core/                # Domain types (pure business logic)
│   ├── types.rs         # LogEntry, LogLevel, AppPhase
│   ├── events.rs        # DaemonEvent enum
│   └── discovery.rs     # Flutter project detection
│
├── config/              # Configuration parsing
│   ├── types.rs         # LaunchConfig, Settings types
│   ├── settings.rs      # .fdemon/config.toml loader
│   ├── launch.rs        # .fdemon/launch.toml loader
│   └── vscode.rs        # .vscode/launch.json compatibility
│
├── daemon/              # Flutter process management
│   ├── process.rs       # FlutterProcess spawning/lifecycle
│   ├── protocol.rs      # JSON-RPC message parsing
│   ├── commands.rs      # Command sending with request tracking
│   ├── devices.rs       # Device discovery
│   ├── emulators.rs     # Emulator discovery and launch
│   └── events.rs        # Daemon event type definitions
│
├── watcher/             # File system watching
│   └── mod.rs           # FileWatcher for auto-reload
│
├── services/            # Reusable service layer
│   ├── flutter_controller.rs  # Reload/restart operations
│   ├── log_service.rs         # Log buffer access
│   └── state_service.rs       # Shared state management
│
├── app/                 # Application layer (TEA)
│   ├── state.rs         # AppState (the Model)
│   ├── message.rs       # Message enum (all events)
│   ├── handler.rs       # update() function
│   ├── session.rs       # Per-device session state
│   └── session_manager.rs  # Multi-session coordination
│
└── tui/                 # Terminal UI (ratatui)
    ├── mod.rs           # Main event loop
    ├── render.rs        # State → UI rendering
    ├── layout.rs        # Layout calculations
    ├── event.rs         # Terminal event handling
    ├── terminal.rs      # Terminal setup/restore
    ├── selector.rs      # Project selection UI
    └── widgets/         # Reusable UI components
        ├── header.rs       # App header bar
        ├── tabs.rs         # Session tab bar
        ├── log_view/       # Scrollable log display (module)
        │   ├── mod.rs         # Widget implementation
        │   ├── state.rs       # LogViewState, FocusInfo
        │   ├── styles.rs      # Stack trace styling
        │   └── tests.rs       # Unit tests
        ├── status_bar.rs   # Bottom status bar
        └── device_selector.rs  # Device selection modal
```

---

## Module Reference

### `common/` — Shared Utilities

Infrastructure code with no domain dependencies.

| File | Purpose |
|------|---------|
| `error.rs` | Custom `Error` enum with variants for each error category. Includes `Result<T>` alias and `ResultExt` trait for error context. |
| `logging.rs` | Sets up file-based logging via `tracing` (stdout is owned by TUI). |
| `signals.rs` | Spawns async handler for SIGINT/SIGTERM, sends `Message::Quit`. |
| `prelude.rs` | Re-exports common types (`Result`, `Error`, tracing macros). |

### `core/` — Domain Types

Pure business logic types with no external dependencies.

| File | Purpose |
|------|---------|
| `types.rs` | `AppPhase`, `LogEntry`, `LogLevel`, `LogSource` — core domain types. |
| `events.rs` | `DaemonEvent` — events from the Flutter process (stdout, stderr, exit). |
| `discovery.rs` | Flutter project detection: `is_runnable_flutter_project()`, `discover_flutter_projects()`, `ProjectType` enum. |

### `config/` — Configuration

Handles loading and parsing configuration from multiple sources.

| File | Purpose |
|------|---------|
| `types.rs` | `LaunchConfig`, `Settings`, `FlutterMode`, and related types. |
| `settings.rs` | Loads `.fdemon/config.toml` for global settings. |
| `launch.rs` | Loads `.fdemon/launch.toml` for launch configurations. |
| `vscode.rs` | Parses `.vscode/launch.json` for VSCode compatibility. |

**Configuration Files:**
- `.fdemon/config.toml` — Behavior, watcher, UI settings
- `.fdemon/launch.toml` — Launch configurations (device, mode, flavor, etc.)
- `.vscode/launch.json` — VSCode Dart launch configs (auto-converted)

### `daemon/` — Flutter Process Infrastructure

Manages Flutter child processes and JSON-RPC communication.

| File | Purpose |
|------|---------|
| `process.rs` | `FlutterProcess` — spawns `flutter run --machine`, manages stdin/stdout/stderr streams. |
| `protocol.rs` | `DaemonMessage` parsing — converts JSON-RPC to typed events. |
| `commands.rs` | `CommandSender`, `DaemonCommand`, `RequestTracker` — send commands with request ID tracking. |
| `devices.rs` | `Device` type, `discover_devices()` — finds connected devices. |
| `emulators.rs` | `Emulator` type, `discover_emulators()`, `launch_emulator()`. |
| `events.rs` | Daemon-specific event types (`AppStart`, `AppLog`, `DeviceInfo`, etc.). |

**Key Protocol:**
- Flutter's `--machine` flag outputs JSON-RPC over stdout
- Messages wrapped in `[...]` brackets
- Events: `daemon.connected`, `app.start`, `app.log`, `device.added`, etc.
- Commands: `app.restart`, `app.stop`, `daemon.shutdown`, etc.

### `watcher/` — File System Watching

Watches for Dart file changes to trigger auto-reload.

| File | Purpose |
|------|---------|
| `mod.rs` | `FileWatcher` — watches `lib/` for `.dart` changes, debounces, sends `Message::AutoReloadTriggered`. |

**Configuration:**
- Default watch path: `lib/`
- Default debounce: 500ms
- Default extensions: `.dart`

### `services/` — Service Layer

Abstractions for Flutter control operations, usable by TUI and future MCP server.

| File | Purpose |
|------|---------|
| `flutter_controller.rs` | `FlutterController` trait — `reload()`, `restart()`, `stop()`, `is_running()`. |
| `log_service.rs` | `LogService` trait — log buffer access and filtering. |
| `state_service.rs` | `SharedState` — thread-safe state with `Arc<RwLock<>>`. |

**Architecture:**
```
┌─────────────┐     ┌─────────────┐
│     TUI     │     │  MCP Server │  (future)
└──────┬──────┘     └──────┬──────┘
       │                   │
       └─────────┬─────────┘
                 │
          ┌──────▼──────┐
          │  Services   │
          │  (traits)   │
          └──────┬──────┘
                 │
          ┌──────▼──────┐
          │ SharedState │
          └─────────────┘
```

### `app/` — Application Layer

TEA pattern implementation — state management and orchestration.

| File | Purpose |
|------|---------|
| `state.rs` | `AppState` — complete application state (the Model). |
| `message.rs` | `Message` enum — all possible events/actions. |
| `handler.rs` | `update()` function — processes messages, returns new state + actions. |
| `session.rs` | `Session`, `SessionHandle` — per-device session state. |
| `session_manager.rs` | `SessionManager` — manages up to 9 concurrent sessions. |
| `mod.rs` | `run()`, `run_with_project()` — entry points. |

**Message Categories:**
- Keyboard events (`Key`)
- Daemon events (`Daemon`)
- Scroll commands (`ScrollUp`, `ScrollDown`, etc.)
- Control commands (`HotReload`, `HotRestart`, `StopApp`)
- Session management (`NextSession`, `CloseCurrentSession`)
- Device/emulator management (`ShowDeviceSelector`, `LaunchEmulator`)

### `tui/` — Terminal UI

Presentation layer using `ratatui` for rendering.

| File | Purpose |
|------|---------|
| `mod.rs` | Main event loop, message channel setup, task spawning. |
| `render/mod.rs` | State → UI rendering (was render.rs). |
| `render/tests.rs` | Full-screen snapshot and transition tests. |
| `layout.rs` | Layout calculations for different UI modes. |
| `event.rs` | Terminal event polling (keyboard, resize). |
| `terminal.rs` | Terminal initialization, cleanup, panic hook. |
| `selector.rs` | Interactive project selection (when multiple found). |
| `test_utils.rs` | TestTerminal wrapper and test helpers. |

**Widgets (`widgets/`):**

| Widget | Purpose |
|--------|---------|
| `Header` | Application title bar with project name |
| `SessionTabs` | Tab bar for multi-session navigation (1-9) |
| `LogView` | Scrollable log display with syntax highlighting |
| `StatusBar` | Bottom bar showing phase, device, reload count |
| `DeviceSelector` | Modal for device/emulator selection |

---

## Key Patterns

### TEA Message Flow

```
┌──────────────────────────────────────────────────────────────┐
│                        Event Loop                            │
│                                                              │
│   ┌─────────┐    ┌─────────┐    ┌─────────┐    ┌─────────┐  │
│   │ Terminal│───▶│ Message │───▶│ Update  │───▶│ Render  │  │
│   │  Event  │    │         │    │(handler)│    │  (view) │  │
│   └─────────┘    └─────────┘    └────┬────┘    └─────────┘  │
│                                      │                       │
│   ┌─────────┐                   ┌────▼────┐                  │
│   │ Daemon  │───▶Message───────▶│  State  │                  │
│   │  Event  │                   │(AppState)│                  │
│   └─────────┘                   └─────────┘                  │
│                                                              │
│   ┌─────────┐                                                │
│   │  Timer  │───▶Message::Tick                               │
│   └─────────┘                                                │
└──────────────────────────────────────────────────────────────┘
```

### Multi-Session Architecture

```
SessionManager
├── sessions: HashMap<SessionId, SessionHandle>
├── session_order: Vec<SessionId>  (for tab ordering)
└── selected_index: usize

SessionHandle
├── session: Session  (state)
├── process: Option<FlutterProcess>
├── cmd_sender: Option<CommandSender>
└── request_tracker: Arc<RequestTracker>

Session
├── id, name, phase
├── device_id, device_name, platform
├── logs: Vec<LogEntry>
├── log_view_state: LogViewState
├── app_id: Option<String>
└── reload_count, timing data
```

### Request/Response Tracking

```
CommandSender
    │
    ▼
DaemonCommand ──┬──▶ RequestTracker.register(id)
    │           │
    ▼           │
stdin.write()   │
    │           │
    ▼           │
FlutterProcess  │
    │           │
    ▼           │
stdout ─────────┴──▶ DaemonMessage::Response
                         │
                         ▼
                    RequestTracker.complete(id)
```

---

## Data Flow

### Startup Sequence

```
1. main.rs: Parse CLI args
2. main.rs: Check if path is runnable Flutter project
3. main.rs: If not, discover projects in subdirectories
4. main.rs: If multiple, show project selector
5. app::run_with_project(): Initialize logging
6. tui::run_with_project(): Initialize terminal
7. tui::run_with_project(): Load settings
8. tui::run_with_project(): Show device selector (if auto_start=false)
9. tui::run_with_project(): Spawn Flutter process
10. tui::run_loop(): Enter main event loop
```

### Hot Reload Flow

```
1. User presses 'r' OR FileWatcher detects change
2. Message::HotReload sent to channel
3. handler::update() processes message:
   - Validates app_id exists
   - Sets phase to Reloading
   - Returns UpdateAction::SpawnTask(Task::Reload)
4. Event loop spawns reload task
5. CommandSender sends app.restart JSON-RPC
6. Flutter process performs reload
7. DaemonEvent::Message(AppProgress{finished:true}) received
8. handler::update() sets phase back to Running
9. tui::render() shows updated status
```

### Log Processing Flow

```
FlutterProcess
    │
    ├── stdout reader task ──▶ DaemonEvent::Stdout(line)
    │                              │
    │                              ▼
    │                         protocol::DaemonMessage::parse()
    │                              │
    │                              ▼
    │                         DaemonEvent::Message(parsed)
    │                              │
    └── stderr reader task ──▶ DaemonEvent::Stderr(line)
                                   │
                                   ▼
                              Message::Daemon(event)
                                   │
                                   ▼
                              handler::update()
                                   │
                                   ▼
                              state.add_log(LogEntry)
                                   │
                                   ▼
                              tui::render() → LogView widget
```

---

## Key Types

### AppState (Model)

```rust
pub struct AppState {
    // UI mode
    pub ui_mode: UiMode,  // Normal, DeviceSelector, Loading, etc.

    // Multi-session support
    pub session_manager: SessionManager,
    pub device_selector: DeviceSelectorState,

    // Configuration
    pub settings: Settings,
    pub project_path: PathBuf,
    pub project_name: Option<String>,

    // Legacy single-session (backward compat)
    pub phase: AppPhase,
    pub logs: Vec<LogEntry>,
    pub log_view_state: LogViewState,
    pub current_app_id: Option<String>,
    pub device_name: Option<String>,
    pub reload_count: u32,
    // ...
}
```

### Message (Events)

```rust
pub enum Message {
    // Input
    Key(KeyEvent),
    Daemon(DaemonEvent),
    Tick,

    // Navigation
    ScrollUp, ScrollDown, PageUp, PageDown,
    
    // Control
    HotReload, HotRestart, StopApp,
    ReloadStarted, ReloadCompleted { time_ms: u64 }, ReloadFailed { reason: String },

    // File watcher
    FilesChanged { count: usize },
    AutoReloadTriggered,

    // Device/session management
    ShowDeviceSelector, HideDeviceSelector,
    DeviceSelected { device: Device },
    SelectSessionByIndex(usize),
    NextSession, PreviousSession,
    CloseCurrentSession,

    // Lifecycle
    Quit,
}
```

### UpdateResult (Update Output)

```rust
pub struct UpdateResult {
    pub message: Option<Message>,  // Follow-up message
    pub action: Option<UpdateAction>,  // Side effect for event loop
}

pub enum UpdateAction {
    SpawnTask(Task),
    DiscoverDevices,
    DiscoverEmulators,
    LaunchEmulator { emulator_id: String },
    SpawnSession { device: Device, config: Option<Box<LaunchConfig>> },
}
```

---

## Dependencies

| Crate | Purpose |
|-------|---------|
| `ratatui` | Terminal UI framework |
| `crossterm` | Cross-platform terminal manipulation |
| `tokio` | Async runtime |
| `serde` / `serde_json` | JSON serialization |
| `toml` | TOML config parsing |
| `notify` | File system watching |
| `tracing` | Structured logging |
| `thiserror` | Error derive macros |
| `color-eyre` | Enhanced error reporting |
| `chrono` | Date/time handling |

---

## Testing Strategy

Flutter Demon follows Rust's conventional test organization with unit tests alongside source code and integration tests in a separate directory.

### Unit Tests

Unit tests live in `src/` alongside the code they test. There are two patterns:

**Inline module (for small test suites):**
```rust
// src/some_module.rs
pub fn add(a: i32, b: i32) -> i32 { a + b }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        assert_eq!(add(2, 2), 4);
    }
}
```

**Separate file (for large test suites, 100+ lines):**
```rust
// src/some_module/mod.rs
pub fn add(a: i32, b: i32) -> i32 { a + b }

#[cfg(test)]
mod tests;

// src/some_module/tests.rs
use super::*;

#[test]
fn test_add() {
    assert_eq!(add(2, 2), 4);
}
```

**Key points:**
- Unit tests can access private items via `use super::*`
- Use `#[cfg(test)]` to exclude test code from release builds
- Prefer separate `tests.rs` file when tests exceed ~100 lines

**Examples in this project:**
| File | Tests | Description |
|------|-------|-------------|
| `src/app/handler/tests.rs` | 150+ | Handler unit tests |
| `src/app/session/tests.rs` | 80+ | Session state tests |
| `src/tui/widgets/log_view/tests.rs` | 77 | Log view widget tests |

### Integration Tests

Integration tests live in the `tests/` directory at the project root:

```
tests/
└── discovery_integration.rs   # Flutter project discovery tests
```

**Key points:**
- Integration tests can only access the public API
- Each file in `tests/` is compiled as a separate crate
- Use `tests/common/mod.rs` for shared helpers (not `tests/common.rs`)
- Run with `cargo test --test <name>` for specific test files

### Running Tests

```bash
# Run all tests
cargo test

# Run unit tests only
cargo test --lib

# Run integration tests only
cargo test --test '*'

# Run specific test file
cargo test --test discovery_integration

# Run tests matching a pattern
cargo test log_view

# Run with output visible
cargo test -- --nocapture

# Run specific test
cargo test test_hot_reload_flow
```

### Test Coverage by Module

| Module | Test File | Coverage |
|--------|-----------|----------|
| `app/handler` | `tests.rs` | Message handling, state transitions |
| `app/session` | `tests.rs` | Session lifecycle, log management |
| `core/discovery` | inline | Project detection logic |
| `core/ansi` | inline | ANSI escape handling |
| `daemon/protocol` | inline | JSON-RPC parsing |
| `tui/render` | `render/tests.rs` | Full-screen snapshots, UI transitions |
| `tui/widgets/log_view` | `tests.rs` | Widget rendering, scrolling |
| `tui/widgets/status_bar` | inline | Widget rendering, phase display |

---

## Future Considerations

1. **MCP Server** — Services layer designed for MCP (Model Context Protocol) integration
2. **Plugin System** — Core/service separation enables plugin extensions
3. **Remote Devices** — Device abstraction supports remote device connections
4. **Themes** — UI settings include theme configuration placeholder
