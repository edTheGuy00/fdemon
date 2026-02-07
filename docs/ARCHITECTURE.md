# Flutter Demon Architecture

This document describes the internal architecture of Flutter Demon, a high-performance TUI for Flutter development written in Rust.

## Table of Contents

- [Overview](#overview)
- [Engine Architecture](#engine-architecture)
- [Design Principles](#design-principles)
- [Project Structure](#project-structure)
- [Module Reference](#module-reference)
- [Key Patterns](#key-patterns)
- [Data Flow](#data-flow)
- [Key Types](#key-types)
- [Future Considerations](#future-considerations)

---

## Overview

Flutter Demon is a terminal-based Flutter development environment that manages Flutter processes, provides real-time log viewing, and supports multi-device sessions. The application is built with a layered architecture separating concerns between domain logic, infrastructure, and presentation.

The core of the application is the **Engine** (`app/engine.rs`), which provides shared orchestration for both TUI and headless runners. The Engine encapsulates all state management, message processing, session tracking, and event broadcasting.

```
┌─────────────────────────────────────────────────────────────────┐
│                        Binary (main.rs)                         │
│                   CLI parsing, project discovery                │
└─────────────────────────────────────────────────────────────────┘
                                 │
                   ┌─────────────┴─────────────┐
                   ▼                           ▼
           ┌───────────────┐           ┌───────────────┐
           │  TUI Runner   │           │    Headless   │
           │ (tui/runner)  │           │   (headless)  │
           │ Terminal I/O  │           │  NDJSON out   │
           └───────┬───────┘           └───────┬───────┘
                   │                           │
                   └─────────────┬─────────────┘
                                 ▼
                    ┌─────────────────────────┐
                    │       Engine            │◄──── signal handler
                    │   (app/engine.rs)       │◄──── file watcher
                    │                         │
                    │ • AppState (TEA model)  │
                    │ • Message channel       │
                    │ • Session tasks         │
                    │ • SharedState           │
                    │ • Event broadcast       │
                    └────────┬────────────────┘
                             │
              ┌──────────────┼──────────────┐
              ▼              ▼              ▼
    ┌───────────────┐ ┌──────────┐ ┌──────────────┐
    │  Services     │ │ Daemon   │ │    Core      │
    │ (controllers) │ │(process) │ │ (domain)     │
    └───────────────┘ └──────────┘ └──────────────┘
                             │
                             ▼
                  ┌───────────────────────┐
                  │   Flutter Process     │
                  │   (flutter run)       │
                  └───────────────────────┘
```

---

## Engine Architecture

### Engine (`app/engine.rs`)

The Engine is the shared orchestration core used by both TUI and headless runners. It encapsulates all application state and coordination logic in a single, testable struct.

**Core Responsibilities:**
- **State Management**: Owns the `AppState` (TEA model)
- **Message Channel**: Unified message channel for all events (keyboard, daemon, watcher, signals)
- **Session Task Tracking**: Manages background tasks for each Flutter session
- **Signal Handling**: SIGINT/SIGTERM handling via `shutdown_tx`/`shutdown_rx`
- **File Watcher**: Integrates file watcher with message bridge
- **Shared State**: Provides `SharedState` for service layer consumers
- **Event Broadcasting**: Emits `EngineEvent` to external subscribers (future MCP server)

**Key Methods:**

| Method | Purpose |
|--------|---------|
| `Engine::new(project_path)` | Creates engine with full initialization |
| `process_message(msg)` | Process single message through TEA |
| `drain_pending_messages()` | Process all pending messages |
| `flush_pending_logs()` | Flush batched logs and sync SharedState |
| `flutter_controller()` | Get controller for current session |
| `log_service()` | Get log buffer access |
| `state_service()` | Get app state access |
| `subscribe()` | Subscribe to EngineEvents |
| `shutdown().await` | Stop watcher, cleanup sessions |

**Event Flow:**
```
Input Sources → Message Channel → Engine.process_message() → handler::update()
                                                          ↓
Signal Handler ──────────────────────────────────────────┘
File Watcher   ──────────────────────────────────────────┘
Daemon Tasks   ──────────────────────────────────────────┘
TUI/Headless   ──────────────────────────────────────────┘
                                                          ↓
                                        ┌─────────────────┴─────────────────┐
                                        ▼                                   ▼
                                  handle_action()                  emit_events()
                                  (side effects)                   (EngineEvent)
                                        │                                   │
                                        ▼                                   ▼
                            Spawn session tasks                     Broadcast to
                            Update SharedState                      subscribers
```

### EngineEvent (`app/engine_event.rs`)

Domain events emitted by the Engine after each message processing cycle. This is the primary extension point for pro features.

**Event Categories:**
- **Session Lifecycle**: `SessionCreated`, `SessionStarted`, `SessionStopped`, `SessionRemoved`
- **Phase Changes**: `PhaseChanged` (Initializing → Running → Reloading, etc.)
- **Hot Reload/Restart**: `ReloadStarted`, `ReloadCompleted`, `ReloadFailed`, `RestartStarted`, `RestartCompleted`
- **Logging**: `LogEntry`, `LogBatch` (for high-volume logging)
- **Device Discovery**: `DevicesDiscovered`
- **File Watcher**: `FilesChanged`
- **Engine Lifecycle**: `Shutdown`

### Runner Implementations

Both runners create an Engine and use it as the single source of truth.

**TUI Runner** (`tui/runner.rs`):
- Creates Engine and initializes the terminal
- Runs TUI-specific startup (device selection, Flutter process spawning)
- Main loop: drains pending messages, flushes logs, renders frame, polls for input
- On quit: shuts down Engine, restores terminal

**Headless Runner** (`headless/runner.rs`):
- Creates Engine and spawns a stdin reader for commands
- Auto-starts a Flutter session
- Main loop: receives messages, processes through Engine, emits NDJSON events
- On quit: shuts down Engine

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

The workspace crates enforce clean layer boundaries with **compile-time guarantees**:

| Crate | Responsibility | Dependencies |
|-------|----------------|--------------|
| **flutter-demon (binary)** | CLI, entry point, headless mode | fdemon-core, fdemon-daemon, fdemon-app, fdemon-tui |
| **fdemon-tui** | Terminal UI presentation | fdemon-core, fdemon-app |
| **fdemon-app** | State, orchestration, TEA, Engine, services, config, watcher | fdemon-core, fdemon-daemon |
| **fdemon-daemon** | Flutter process I/O, device/emulator management | fdemon-core |
| **fdemon-core** | Domain types, events, discovery, error handling | **None** (zero internal deps) |

**Dependency Flow:**
```
fdemon-core (foundation)
    ↓
fdemon-daemon (Flutter I/O)
    ↓
fdemon-app (orchestration)
    ↓
fdemon-tui (presentation)
    ↓
flutter-demon (binary)
```

### Layer Dependencies Note

The TUI crate depends on App because of the TEA pattern:
- **View** (`tui::render`) must receive **Model** (`AppState`) to render it
- This is the fundamental TEA contract: `View: State → UI`
- The dependency is intentional and necessary, not a violation

**Workspace Benefits:**
- **Compile-time enforcement**: Cargo prevents circular dependencies and violations
- **Independent testing**: Each crate can be tested in isolation
- **Clear boundaries**: Module structure matches crate boundaries
- **Future extensibility**: Crates can be published, reused, or replaced independently
- **Parallel compilation**: Cargo can build independent crates concurrently

### Error Handling

- Custom `Error` enum with domain-specific variants
- `Result<T>` type alias throughout
- Errors are categorized as `fatal` vs `recoverable`
- Rich error context via `ResultExt` trait

---

## Project Structure

Flutter Demon is organized as a **Cargo workspace** with 4 library crates and 1 binary:

```
flutter-demon/
├── Cargo.toml                    # Workspace root + binary configuration
├── src/
│   ├── main.rs                   # Binary entry point, CLI handling
│   └── headless/                 # Headless NDJSON mode
│       ├── mod.rs                # HeadlessEvent types
│       └── runner.rs             # Headless runner (uses Engine)
│
├── crates/
│   ├── fdemon-core/              # Domain types (zero internal deps)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── types.rs          # LogEntry, LogLevel, AppPhase
│   │       ├── events.rs         # DaemonMessage, DaemonEvent + 9 event structs
│   │       ├── discovery.rs      # Flutter project detection
│   │       ├── stack_trace.rs    # Stack trace parsing
│   │       ├── ansi.rs           # ANSI escape sequence handling
│   │       ├── error.rs          # Error types and Result alias
│   │       ├── logging.rs        # File-based logging setup
│   │       └── prelude.rs        # Common imports
│   │
│   ├── fdemon-daemon/            # Flutter process management
│   │   ├── Cargo.toml            # depends: fdemon-core
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── process.rs        # FlutterProcess spawning/lifecycle
│   │       ├── protocol.rs       # parse_daemon_message() and conversion functions
│   │       ├── commands.rs       # Command sending with request tracking
│   │       ├── devices.rs        # Device discovery
│   │       ├── emulators.rs      # Emulator discovery and launch
│   │       ├── avds.rs           # Android AVD utilities
│   │       ├── simulators.rs     # iOS simulator utilities
│   │       ├── tool_availability.rs  # Tool detection
│   │       └── test_utils.rs     # Test helpers
│   │
│   ├── fdemon-app/               # Application state and orchestration
│   │   ├── Cargo.toml            # depends: fdemon-core, fdemon-daemon
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── engine.rs         # Engine - shared orchestration core
│   │       ├── engine_event.rs   # EngineEvent - domain events
│   │       ├── state.rs          # AppState (the Model)
│   │       ├── message.rs        # Message enum (all events)
│   │       ├── signals.rs        # SIGINT/SIGTERM handling
│   │       ├── handler/          # TEA update function + helpers
│   │       ├── session.rs        # Per-device session state
│   │       ├── session_manager.rs  # Multi-session coordination
│   │       ├── watcher.rs        # File system watching
│   │       ├── config/           # Configuration parsing
│   │       │   ├── types.rs      # LaunchConfig, Settings types
│   │       │   ├── settings.rs   # .fdemon/config.toml loader
│   │       │   ├── launch.rs     # .fdemon/launch.toml loader
│   │       │   └── vscode.rs     # .vscode/launch.json compatibility
│   │       ├── services/         # Reusable service layer
│   │       │   ├── flutter_controller.rs  # Reload/restart operations
│   │       │   ├── log_service.rs         # Log buffer access
│   │       │   └── state_service.rs       # Shared state management
│   │       ├── editor.rs         # Editor integration
│   │       ├── settings_items.rs # Setting item generators
│   │       ├── log_view_state.rs # Scroll/viewport state
│   │       ├── hyperlinks.rs     # Link detection and state
│   │       ├── confirm_dialog.rs # Dialog state
│   │       └── new_session_dialog/  # New session dialog state
│   │           ├── state.rs
│   │           ├── fuzzy.rs
│   │           ├── target_selector_state.rs
│   │           └── device_groups.rs
│   │
│   └── fdemon-tui/               # Terminal UI (Ratatui)
│       ├── Cargo.toml            # depends: fdemon-core, fdemon-app
│       └── src/
│           ├── lib.rs
│           ├── runner.rs         # TUI runner (creates Engine)
│           ├── startup.rs        # TUI-specific startup
│           ├── render/           # State → UI rendering
│           │   ├── mod.rs
│           │   └── tests.rs
│           ├── layout.rs         # Layout calculations
│           ├── event.rs          # Terminal event handling
│           ├── terminal.rs       # Terminal setup/restore
│           ├── selector.rs       # Project selection UI
│           ├── test_utils.rs     # TestTerminal wrapper
│           └── widgets/          # Reusable UI components
│               ├── header.rs
│               ├── tabs.rs
│               ├── log_view/     # Scrollable log display
│               │   ├── mod.rs
│               │   ├── styles.rs
│               │   └── tests.rs
│               ├── status_bar.rs
│               ├── device_selector.rs
│               ├── settings_panel/
│               │   ├── mod.rs
│               │   └── styles.rs
│               ├── confirm_dialog.rs
│               └── new_session_dialog/
│                   ├── mod.rs
│                   └── target_selector.rs
│
└── tests/                        # Integration tests (binary crate)
    ├── common/
    └── e2e/
```

---

## Module Reference

### `fdemon-core` — Domain Types (Foundation Crate)

**Location**: `crates/fdemon-core/`
**Dependencies**: Zero internal dependencies (only external crates)
**Purpose**: Pure business logic types with no infrastructure dependencies

| File | Purpose |
|------|---------|
| `types.rs` | `AppPhase`, `LogEntry`, `LogLevel`, `LogSource` — core domain types |
| `events.rs` | `DaemonMessage`, `DaemonEvent`, and all 9 event structs (`AppStart`, `AppLog`, `DeviceInfo`, etc.) — events from the Flutter process |
| `discovery.rs` | Flutter project detection: `is_runnable_flutter_project()`, `discover_flutter_projects()`, `ProjectType` enum |
| `stack_trace.rs` | Stack trace parsing and rendering |
| `ansi.rs` | ANSI escape sequence handling |
| `error.rs` | Custom `Error` enum with variants for each error category. Includes `Result<T>` alias and `ResultExt` trait for error context |
| `logging.rs` | Sets up file-based logging via `tracing` (stdout is owned by TUI) |
| `prelude.rs` | Re-exports common types (`Result`, `Error`, tracing macros) |

### `fdemon-daemon` — Flutter Process Infrastructure

**Location**: `crates/fdemon-daemon/`
**Dependencies**: `fdemon-core`
**Purpose**: Manages Flutter child processes and JSON-RPC communication

| File | Purpose |
|------|---------|
| `process.rs` | `FlutterProcess` — spawns `flutter run --machine`, manages stdin/stdout/stderr streams |
| `protocol.rs` | `parse_daemon_message()`, `to_log_entry()`, `parse_flutter_log()`, `detect_log_level()` — converts JSON-RPC to typed events (event types in `fdemon-core`) |
| `commands.rs` | `CommandSender`, `DaemonCommand`, `RequestTracker` — send commands with request ID tracking |
| `devices.rs` | `Device` type, `discover_devices()` — finds connected devices |
| `emulators.rs` | `Emulator` type, `discover_emulators()`, `launch_emulator()` |
| `avds.rs` | Android AVD utilities |
| `simulators.rs` | iOS simulator utilities |
| `tool_availability.rs` | Tool detection (Android SDK, iOS simulators) |
| `test_utils.rs` | Test helpers for device/emulator testing |

**Key Protocol:**
- Flutter's `--machine` flag outputs JSON-RPC over stdout
- Messages wrapped in `[...]` brackets
- Events: `daemon.connected`, `app.start`, `app.log`, `device.added`, etc.
- Commands: `app.restart`, `app.stop`, `daemon.shutdown`, etc.

### `fdemon-app` — Application State and Orchestration

**Location**: `crates/fdemon-app/`
**Dependencies**: `fdemon-core`, `fdemon-daemon`
**Purpose**: TEA pattern implementation, Engine orchestration, services, config, watcher

**Core Modules:**

| File | Purpose |
|------|---------|
| `engine.rs` | `Engine` struct — shared orchestration core for TUI and headless runners |
| `engine_event.rs` | `EngineEvent` enum — domain events broadcast to external consumers |
| `state.rs` | `AppState` — complete application state (the Model) |
| `message.rs` | `Message` enum — all possible events/actions |
| `signals.rs` | Signal handling for SIGINT/SIGTERM |
| `handler/` | `update()` function and handler helpers (TEA) |
| `session.rs` | `Session`, `SessionHandle` — per-device session state |
| `session_manager.rs` | `SessionManager` — manages up to 9 concurrent sessions |
| `watcher.rs` | `FileWatcher` — watches `lib/` for `.dart` changes, debounces, emits `WatcherEvent` |

**Configuration (`config/`):**

| File | Purpose |
|------|---------|
| `types.rs` | `LaunchConfig`, `Settings`, `FlutterMode`, and related types |
| `settings.rs` | Loads `.fdemon/config.toml` for global settings |
| `launch.rs` | Loads `.fdemon/launch.toml` for launch configurations |
| `vscode.rs` | Parses `.vscode/launch.json` for VSCode compatibility |

**Configuration Files:**
- `.fdemon/config.toml` — Behavior, watcher, UI settings
- `.fdemon/launch.toml` — Launch configurations (device, mode, flavor, etc.)
- `.vscode/launch.json` — VSCode Dart launch configs (auto-converted)

**Services (`services/`):**

The services layer provides trait-based abstractions for Flutter control operations, managed by the Engine.

| File | Purpose |
|------|---------|
| `flutter_controller.rs` | `FlutterController` trait — `reload()`, `restart()`, `stop()`, `is_running()` |
| `log_service.rs` | `LogService` trait — log buffer access and filtering |
| `state_service.rs` | `SharedState` — thread-safe state with `Arc<RwLock<>>` |

**UI State:**

| File | Purpose |
|------|---------|
| `editor.rs` | `open_in_editor()` function for file navigation |
| `settings_items.rs` | Setting item generators for settings panel |
| `log_view_state.rs` | `LogViewState` — scroll/viewport state |
| `hyperlinks.rs` | `LinkHighlightState` — link detection and navigation |
| `confirm_dialog.rs` | `ConfirmDialogState` — confirmation dialog state |
| `new_session_dialog/` | New session dialog state (fuzzy filtering, target selector, device groups) |

**Message Categories:**
- Keyboard events (`Key`)
- Daemon events (`Daemon`)
- Scroll commands (`ScrollUp`, `ScrollDown`, etc.)
- Control commands (`HotReload`, `HotRestart`, `StopApp`)
- Session management (`NextSession`, `CloseCurrentSession`)
- Device/emulator management (`ShowDeviceSelector`, `LaunchEmulator`)

### `fdemon-tui` — Terminal UI (Presentation Layer)

**Location**: `crates/fdemon-tui/`
**Dependencies**: `fdemon-core`, `fdemon-app`
**Purpose**: Presentation layer using `ratatui`. The TUI runner creates an Engine and uses it for all state management.

**Key Architecture:**
- **Runner** (`runner.rs`): Main entry point, creates Engine, runs event loop
- **Event Polling** (`event.rs`): Polls terminal for keyboard/resize events, converts to `Message`
- **Rendering** (`render/`): Renders `AppState` to terminal using ratatui widgets
- **Widgets** (`widgets/`): Reusable UI components (header, tabs, log view, status bar, dialogs)

| File | Purpose |
|------|---------|
| `runner.rs` | Main entry point, Engine creation, event loop |
| `startup.rs` | TUI-specific startup logic |
| `render/mod.rs` | State → UI rendering |
| `render/tests.rs` | Full-screen snapshot and transition tests |
| `layout.rs` | Layout calculations for different UI modes |
| `event.rs` | Terminal event polling (keyboard, resize) |
| `terminal.rs` | Terminal initialization, cleanup, panic hook |
| `selector.rs` | Interactive project selection (when multiple found) |
| `test_utils.rs` | TestTerminal wrapper and test helpers |

**Widgets (`widgets/`):**

| Widget | Purpose |
|--------|---------|
| `header.rs` | Application title bar with project name |
| `tabs.rs` | Tab bar for multi-session navigation (1-9) |
| `log_view/` | Scrollable log display with syntax highlighting |
| `status_bar.rs` | Bottom bar showing phase, device, reload count |
| `device_selector.rs` | Modal for device/emulator selection |
| `settings_panel/` | Settings editor (project, user prefs, launch configs, VSCode) |
| `confirm_dialog.rs` | Confirmation dialog widget |
| `new_session_dialog/` | New session creation dialog |

### `flutter-demon` (Binary) — Headless Mode

**Location**: `src/headless/`
**Dependencies**: `fdemon-core`, `fdemon-daemon`, `fdemon-app`, `fdemon-tui`
**Purpose**: Binary entry point, CLI parsing, headless NDJSON mode

**Headless Mode:**

Headless mode provides a non-TUI interface for E2E testing and automation. It creates an Engine and outputs structured NDJSON events to stdout.

| File | Purpose |
|------|---------|
| `mod.rs` | `HeadlessEvent` enum and NDJSON serialization |
| `runner.rs` | Headless runner, Engine creation, stdin reader, event loop |

**HeadlessEvent Types:**
- `DaemonConnected`, `DaemonDisconnected`
- `AppStarted`, `AppStopped`
- `HotReloadStarted`, `HotReloadCompleted`, `HotReloadFailed`
- `Log`, `Error`
- `SessionCreated`, `SessionRemoved`

---

## Key Patterns

### TEA Message Flow (via Engine)

The Engine acts as the central hub for all message processing. Both TUI and headless runners send messages to the Engine, which processes them through the TEA update cycle.

```
┌──────────────────────────────────────────────────────────────────┐
│                          Event Loop                              │
│                                                                  │
│  Input Sources                     Engine                        │
│  ┌─────────┐                  ┌──────────────┐                  │
│  │ Terminal│─────┐            │ msg_channel  │                  │
│  │  Event  │     │            │      ↓       │                  │
│  └─────────┘     │            │ process_msg  │                  │
│                  ├───Message──▶│      ↓       │                  │
│  ┌─────────┐     │            │  update()    │───Action────┐    │
│  │ Daemon  │─────┤            │      ↓       │             │    │
│  │  Event  │     │            │  AppState    │             ▼    │
│  └─────────┘     │            │      ↓       │      handle_action() │
│                  │            │emit_events() │      sync_shared_state() │
│  ┌─────────┐     │            └──────┬───────┘             │    │
│  │ Watcher │─────┤                   │                     │    │
│  │  Event  │     │                   ▼                     ▼    │
│  └─────────┘     │            EngineEvent            UpdateAction│
│                  │            (broadcast)            (side effects)│
│  ┌─────────┐     │                                                │
│  │ Signal  │─────┘                                                │
│  │ Handler │                                                      │
│  └─────────┘                                                      │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │ TUI Runner: Render after drain_pending_messages()       │    │
│  │ Headless Runner: Emit NDJSON events after process_msg() │    │
│  └─────────────────────────────────────────────────────────┘    │
└──────────────────────────────────────────────────────────────────┘
```

**Message Processing Steps:**
1. Input source (terminal, daemon, watcher, signal) sends `Message` to Engine's channel
2. Engine calls `process_message(msg)`:
   - Captures state snapshot (pre)
   - Calls `handler::update(state, msg)` → returns `(new_state, action)`
   - Calls `handle_action(action)` → spawns tasks, updates SharedState
   - Captures state snapshot (post)
   - Calls `emit_events(pre, post)` → broadcasts `EngineEvent` to subscribers
3. Runner-specific handling:
   - **TUI**: Drains all messages, flushes logs, renders frame
   - **Headless**: Processes one message, flushes logs, emits NDJSON

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
    │                         protocol::parse_daemon_message()
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

The complete application state, owned by the Engine. Contains:
- **UI mode** (`UiMode`) — Normal, DeviceSelector, Loading, etc.
- **Session manager** — Multi-session coordination with up to 9 sessions
- **Device selector state** — Device/emulator selection UI state
- **Configuration** — Settings, project path, project name
- **Active session state** — Phase, logs, log view state, app ID, device info, reload count

### Message (Events)

All possible events that can affect application state:
- **Input**: `Key(KeyEvent)`, `Daemon(DaemonEvent)`, `Tick`
- **Navigation**: `ScrollUp`, `ScrollDown`, `PageUp`, `PageDown`
- **Control**: `HotReload`, `HotRestart`, `StopApp`
- **Reload lifecycle**: `ReloadStarted`, `ReloadCompleted { time_ms }`, `ReloadFailed { reason }`
- **File watcher**: `FilesChanged { count }`, `AutoReloadTriggered`
- **Session management**: `ShowDeviceSelector`, `DeviceSelected { device }`, `NextSession`, `CloseCurrentSession`
- **Lifecycle**: `Quit`

### UpdateResult (Update Output)

The return type from `handler::update()`:
- **message** — Optional follow-up `Message` to process
- **action** — Optional `UpdateAction` side effect for the event loop

**UpdateAction variants:**
- `SpawnTask(Task)` — Spawn an async task (reload, restart, etc.)
- `DiscoverDevices` — Trigger device discovery
- `DiscoverEmulators` — Trigger emulator discovery
- `LaunchEmulator { emulator_id }` — Launch a specific emulator
- `SpawnSession { device, config }` — Create a new Flutter session
