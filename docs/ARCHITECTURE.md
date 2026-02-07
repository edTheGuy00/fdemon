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
```rust
// Initialization
Engine::new(project_path)           // Creates engine with full initialization

// Message processing
engine.process_message(msg)         // Process single message through TEA
engine.drain_pending_messages()     // Process all pending messages
engine.flush_pending_logs()         // Flush batched logs and sync SharedState

// Service accessors
engine.flutter_controller()         // Get controller for current session
engine.log_service()                // Get log buffer access
engine.state_service()              // Get app state access

// Event broadcasting
engine.subscribe()                  // Subscribe to EngineEvents

// Lifecycle
engine.shutdown().await             // Stop watcher, cleanup sessions
```

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

**Usage Example:**
```rust
let mut rx = engine.subscribe();

tokio::spawn(async move {
    while let Ok(event) = rx.recv().await {
        match event {
            EngineEvent::ReloadStarted { session_id } => {
                // Track reload start time
            }
            EngineEvent::ReloadCompleted { session_id, time_ms } => {
                // Report reload performance
            }
            EngineEvent::LogBatch { session_id, entries } => {
                // Forward logs to MCP server
            }
            _ => {}
        }
    }
});
```

### Runner Implementations

Both runners create an Engine and use it as the single source of truth.

**TUI Runner** (`tui/runner.rs`):
```rust
pub async fn run_with_project(project_path: &Path) -> Result<()> {
    let mut engine = Engine::new(project_path.to_path_buf());
    let mut term = ratatui::init();

    // TUI-specific startup
    startup::startup_flutter(&mut engine.state, &engine.settings, &engine.project_path);

    // Main loop
    while !engine.should_quit() {
        engine.drain_pending_messages();
        engine.flush_pending_logs();
        term.draw(|frame| render::view(frame, &mut engine.state))?;
        if let Some(message) = event::poll()? {
            engine.process_message(message);
        }
    }

    engine.shutdown().await;
    ratatui::restore();
    Ok(())
}
```

**Headless Runner** (`headless/runner.rs`):
```rust
pub async fn run_headless(project_path: &Path) -> Result<()> {
    let mut engine = Engine::new(project_path.to_path_buf());

    // Headless-specific stdin reader
    spawn_stdin_reader(engine.msg_sender());

    // Auto-start Flutter session
    headless_auto_start(&mut engine).await;

    // Main loop
    loop {
        if engine.should_quit() { break; }
        match engine.msg_rx.recv().await {
            Some(msg) => {
                engine.process_message(msg);
                engine.flush_pending_logs();
                emit_headless_events(&engine.state);
            }
            None => break,
        }
    }

    engine.shutdown().await;
    Ok(())
}
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
│   │       ├── protocol.rs       # DaemonMessage::parse() implementation
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
| `types.rs` | `AppPhase`, `LogEntry`, `LogLevel`, `LogSource` — core domain types. |
| `events.rs` | `DaemonMessage`, `DaemonEvent`, and all 9 event structs (`AppStart`, `AppLog`, `DeviceInfo`, etc.) — events from the Flutter process. |
| `discovery.rs` | Flutter project detection: `is_runnable_flutter_project()`, `discover_flutter_projects()`, `ProjectType` enum. |
| `stack_trace.rs` | Stack trace parsing and rendering. |
| `ansi.rs` | ANSI escape sequence handling. |
| `error.rs` | Custom `Error` enum with variants for each error category. Includes `Result<T>` alias and `ResultExt` trait for error context. |
| `logging.rs` | Sets up file-based logging via `tracing` (stdout is owned by TUI). |
| `prelude.rs` | Re-exports common types (`Result`, `Error`, tracing macros). |

### `fdemon-daemon` — Flutter Process Infrastructure

**Location**: `crates/fdemon-daemon/`
**Dependencies**: `fdemon-core`
**Purpose**: Manages Flutter child processes and JSON-RPC communication

| File | Purpose |
|------|---------|
| `process.rs` | `FlutterProcess` — spawns `flutter run --machine`, manages stdin/stdout/stderr streams. |
| `protocol.rs` | `DaemonMessage::parse()` — converts JSON-RPC to typed events (event types in `fdemon-core`). |
| `commands.rs` | `CommandSender`, `DaemonCommand`, `RequestTracker` — send commands with request ID tracking. |
| `devices.rs` | `Device` type, `discover_devices()` — finds connected devices. |
| `emulators.rs` | `Emulator` type, `discover_emulators()`, `launch_emulator()`. |
| `avds.rs` | Android AVD utilities. |
| `simulators.rs` | iOS simulator utilities. |
| `tool_availability.rs` | Tool detection (Android SDK, iOS simulators). |
| `test_utils.rs` | Test helpers for device/emulator testing. |

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
| `engine.rs` | `Engine` struct — shared orchestration core for TUI and headless runners. |
| `engine_event.rs` | `EngineEvent` enum — domain events broadcast to external consumers. |
| `state.rs` | `AppState` — complete application state (the Model). |
| `message.rs` | `Message` enum — all possible events/actions. |
| `signals.rs` | Signal handling for SIGINT/SIGTERM. |
| `handler/` | `update()` function and handler helpers (TEA). |
| `session.rs` | `Session`, `SessionHandle` — per-device session state. |
| `session_manager.rs` | `SessionManager` — manages up to 9 concurrent sessions. |
| `watcher.rs` | `FileWatcher` — watches `lib/` for `.dart` changes, debounces, emits `WatcherEvent`. |

**Configuration (`config/`):**

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

**Services (`services/`):**

The services layer provides trait-based abstractions for Flutter control operations, managed by the Engine.

| File | Purpose |
|------|---------|
| `flutter_controller.rs` | `FlutterController` trait — `reload()`, `restart()`, `stop()`, `is_running()`. |
| `log_service.rs` | `LogService` trait — log buffer access and filtering. |
| `state_service.rs` | `SharedState` — thread-safe state with `Arc<RwLock<>>`. |

**UI State:**

| File | Purpose |
|------|---------|
| `editor.rs` | `open_in_editor()` function for file navigation. |
| `settings_items.rs` | Setting item generators for settings panel. |
| `log_view_state.rs` | `LogViewState` — scroll/viewport state. |
| `hyperlinks.rs` | `LinkHighlightState` — link detection and navigation. |
| `confirm_dialog.rs` | `ConfirmDialogState` — confirmation dialog state. |
| `new_session_dialog/` | New session dialog state (fuzzy filtering, target selector, device groups). |

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
| `runner.rs` | Main entry point, Engine creation, event loop. |
| `startup.rs` | TUI-specific startup logic. |
| `render/mod.rs` | State → UI rendering. |
| `render/tests.rs` | Full-screen snapshot and transition tests. |
| `layout.rs` | Layout calculations for different UI modes. |
| `event.rs` | Terminal event polling (keyboard, resize). |
| `terminal.rs` | Terminal initialization, cleanup, panic hook. |
| `selector.rs` | Interactive project selection (when multiple found). |
| `test_utils.rs` | TestTerminal wrapper and test helpers. |

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
| `mod.rs` | `HeadlessEvent` enum and NDJSON serialization. |
| `runner.rs` | Headless runner, Engine creation, stdin reader, event loop. |

**HeadlessEvent Types:**
- `DaemonConnected`, `DaemonDisconnected`
- `AppStarted`, `AppStopped`
- `HotReloadStarted`, `HotReloadCompleted`, `HotReloadFailed`
- `Log`, `Error`
- `SessionCreated`, `SessionRemoved`

**Usage:**
```bash
# Run in headless mode
cargo run -- --headless /path/to/flutter/project > events.ndjson

# Send commands via stdin
echo "r" | cargo run -- --headless /path/to/flutter/project
```

### Restructuring Notes (Phases 1-3)

The project was incrementally restructured from a single-crate architecture to a workspace with 4 library crates:

**Phase 1 (Clean Dependencies):**
- **Event types** (`DaemonMessage`, event structs) moved from `daemon/` to `core/` — core became a true leaf module with no dependencies
- **State types** (`LogViewState`, `LinkHighlightState`, `ConfirmDialogState`) moved from `tui/` to `app/` — removed tui → app dependency inversion
- **Logic functions** (`process_message`, `handle_action`, `open_in_editor`, `fuzzy_filter`, setting item generators) moved from `tui/` to `app/` — enabled headless mode without tui dependency
- **Signal handler** moved from `common/` to `app/` — common became a true leaf module
- **File watcher** emits its own `WatcherEvent` instead of constructing `Message` — watcher became independent of app

**Phase 2 (Engine Abstraction):**
- **Engine struct** (`app/engine.rs`) — encapsulates all shared state between TUI and headless runners
- **EngineEvent enum** (`app/engine_event.rs`) — domain events for external consumers (future MCP server)
- **TUI refactor** (`tui/runner.rs`) — uses Engine for all state management
- **Headless refactor** (`headless/runner.rs`) — uses Engine for all state management
- **Services wiring** — `FlutterController`, `LogService`, `StateService` accessible via Engine

**Phase 3 (Workspace Split):**
- **fdemon-core** — Created from `src/core/` + `src/common/` (243 unit tests)
- **fdemon-daemon** — Created from `src/daemon/` (136 unit tests)
- **fdemon-app** — Created from `src/app/`, `src/config/`, `src/services/`, `src/watcher/` (726 unit tests)
- **fdemon-tui** — Created from `src/tui/` (427 unit tests)
- **Binary** — `src/main.rs` + `src/headless/` depends on all 4 crates

**Benefits:**
- **Compile-time enforcement**: Cargo prevents circular dependencies and layer violations
- **Independent testing**: Each crate can be tested in isolation (1,532 total unit tests)
- **Clear boundaries**: Module structure matches crate boundaries
- **Future extensibility**: Crates can be published, reused, or replaced independently
- **Parallel compilation**: Cargo can build independent crates concurrently
- **MCP readiness**: Event broadcasting and service layer ready for MCP server integration

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
| `app/engine` | inline | Engine initialization, message processing, event broadcasting |
| `app/engine_event` | inline | Event type labels, serialization, all event variants |
| `app/handler` | `tests.rs` | Message handling, state transitions |
| `app/session` | `tests.rs` | Session lifecycle, log management |
| `core/discovery` | inline | Project detection logic |
| `core/ansi` | inline | ANSI escape handling |
| `daemon/protocol` | inline | JSON-RPC parsing |
| `tui/render` | `render/tests.rs` | Full-screen snapshots, UI transitions |
| `tui/widgets/log_view` | `tests.rs` | Widget rendering, scrolling |
| `tui/widgets/status_bar` | inline | Widget rendering, phase display |
| `headless` | inline | NDJSON serialization, event constructors |

---

## Future Considerations

1. **MCP Server** — Services layer and EngineEvent broadcasting designed for MCP (Model Context Protocol) integration. External consumers can subscribe to `engine.subscribe()` and use `engine.flutter_controller()`, `engine.log_service()`, and `engine.state_service()` for control operations. The workspace structure enables a future `fdemon-mcp` crate that depends only on `fdemon-app`.

2. **Crate Publishing** — The workspace structure enables independent crate publishing:
   - `fdemon-core`: Reusable domain types for Flutter tooling
   - `fdemon-daemon`: Reusable Flutter process management
   - `fdemon-app`: Reusable Engine for custom Flutter frontends
   - `fdemon-tui`: Reference TUI implementation

3. **Plugin System** — Core/service separation enables plugin extensions. Plugins can subscribe to EngineEvents and access services via the Engine.

4. **Remote Devices** — Device abstraction supports remote device connections. Future work could add SSH transport layer in `fdemon-daemon`.

5. **Themes** — UI settings include theme configuration placeholder in `fdemon-app`.
