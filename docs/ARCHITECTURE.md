# Flutter Demon Architecture

This document describes the internal architecture of Flutter Demon, a high-performance TUI for Flutter development written in Rust.

## Table of Contents

- [Overview](#overview)
- [Engine Architecture](#engine-architecture)
- [Design Principles](#design-principles)
- [Project Structure](#project-structure)
- [Module Reference](#module-reference)
- [Key Patterns](#key-patterns)
- [DevTools Subsystem](#devtools-subsystem)
- [DAP Server Subsystem](#dap-server-subsystem)
- [Native Log Capture Subsystem](#native-log-capture-subsystem)
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
| **flutter-demon (binary)** | CLI, entry point, headless mode | fdemon-core, fdemon-daemon, fdemon-app, fdemon-tui, fdemon-dap |
| **fdemon-tui** | Terminal UI presentation | fdemon-core, fdemon-app |
| **fdemon-app** | State, orchestration, TEA, Engine, services, config, watcher, DAP bridge | fdemon-core, fdemon-daemon, fdemon-dap |
| **fdemon-dap** | DAP protocol types, adapter logic, TCP server, stdio transport | fdemon-core |
| **fdemon-daemon** | Flutter process I/O, device/emulator management | fdemon-core |
| **fdemon-core** | Domain types, events, discovery, error handling | **None** (zero internal deps) |

**Dependency Flow:**
```
fdemon-core (foundation)
    ↓               ↓
fdemon-daemon    fdemon-dap (DAP protocol)
    ↓               ↓
fdemon-app (orchestration + DAP bridge)
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

Flutter Demon is organized as a **Cargo workspace** with 5 library crates and 1 binary:

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
│   │       ├── prelude.rs        # Common imports
│   │       ├── network.rs        # Network domain types (HttpProfileEntry, NetworkTiming, etc.)
│   │       ├── performance.rs    # Performance domain types (FrameTiming, MemorySample, RingBuffer, etc.)
│   │       └── widget_tree.rs    # Widget tree types (DiagnosticsNode, LayoutInfo, EdgeInsets)
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
│   │       ├── tool_availability.rs  # Tool detection (adb, xcrun simctl, idevicesyslog)
│   │       ├── test_utils.rs     # Test helpers
│   │       ├── native_logs/      # Native platform log capture
│   │       │   ├── mod.rs        # NativeLogCapture trait, shared types, platform dispatch
│   │       │   ├── android.rs    # adb logcat capture
│   │       │   ├── macos.rs      # macOS log stream capture
│   │       │   └── ios.rs        # iOS simulator (xcrun simctl) + physical (idevicesyslog)
│   │       └── vm_service/       # VM Service WebSocket client
│   │           ├── mod.rs        # VmServiceHandle, connection management
│   │           ├── client.rs     # WebSocket client transport
│   │           ├── protocol.rs   # JSON-RPC protocol types
│   │           ├── errors.rs     # VM Service error types
│   │           ├── logging.rs    # VM Service logging utilities
│   │           ├── network.rs    # ext.dart.io.* HTTP/socket profiling
│   │           ├── performance.rs # Memory usage, allocation profiling
│   │           ├── timeline.rs   # Frame timing from extension stream
│   │           └── extensions/   # Inspector, layout, overlays, dumps
│   │               ├── mod.rs
│   │               ├── inspector.rs
│   │               ├── layout.rs
│   │               ├── overlays.rs
│   │               └── dumps.rs
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
│   │       │   └── devtools/     # DevTools mode handlers
│   │       │       ├── mod.rs    # Panel switching, enter/exit, overlays
│   │       │       ├── inspector.rs  # Widget tree fetch, layout data fetch
│   │       │       ├── performance.rs # Frame selection, memory samples, allocations
│   │       │       └── network.rs    # Network navigation, recording, filter, polling
│   │       ├── session/          # Per-device session state
│   │       │   ├── mod.rs
│   │       │   ├── session.rs    # Session struct and core state
│   │       │   ├── handle.rs     # SessionHandle
│   │       │   ├── network.rs    # NetworkState — per-session network monitoring
│   │       │   ├── performance.rs # PerformanceState — per-session perf monitoring
│   │       │   └── native_tags.rs # NativeTagState — per-session tag discovery/filtering
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
│               ├── tag_filter.rs     # Native tag filter overlay (toggle visibility per tag)
│               ├── new_session_dialog/
│               │   ├── mod.rs
│               │   └── target_selector.rs
│               └── devtools/         # DevTools panels
│                   ├── mod.rs        # Tab bar + panel dispatch
│                   ├── inspector/    # Widget Inspector (tree + layout explorer)
│                   │   ├── mod.rs
│                   │   ├── tree_panel.rs
│                   │   └── layout_panel.rs
│                   ├── performance/  # Performance monitoring
│                   │   ├── mod.rs
│                   │   ├── styles.rs
│                   │   ├── frame_chart/  # Frame timing bar chart
│                   │   │   ├── mod.rs
│                   │   │   ├── bars.rs
│                   │   │   └── detail.rs
│                   │   └── memory_chart/ # Memory time-series + allocation table
│                   │       ├── mod.rs
│                   │       ├── chart.rs
│                   │       ├── table.rs
│                   │       └── braille_canvas.rs
│                   └── network/      # Network monitor
│                       ├── mod.rs
│                       ├── request_table.rs
│                       └── request_details.rs
│
├── crates/fdemon-dap/            # DAP server (protocol + adapter + transport)
│   ├── Cargo.toml                # depends: fdemon-core (no daemon/app deps)
│   └── src/
│       ├── lib.rs
│       ├── protocol/             # DAP wire protocol
│       │   ├── mod.rs
│       │   ├── types.rs          # All DAP request/response/event types
│       │   └── codec.rs          # Content-Length framing encode/decode
│       ├── adapter/              # DAP ↔ VM Service translation
│       │   ├── mod.rs            # DapAdapter, DebugBackend trait, DebugEvent
│       │   ├── breakpoints.rs    # BreakpointState, conditions, logpoints
│       │   ├── evaluate.rs       # Expression evaluation, EvalContext
│       │   ├── stack.rs          # FrameStore, VariableStore, SourceReferenceStore
│       │   └── threads.rs        # ThreadMap, MultiSessionThreadMap, ID namespacing
│       ├── server/               # TCP listener + session lifecycle
│       │   ├── mod.rs            # DapServer, TCP accept loop
│       │   └── session.rs        # DapClientSession, NoopBackend (test helper)
│       └── transport/            # Stdio transport
│           ├── mod.rs
│           └── stdio.rs          # Stdio DAP transport for IDE integration testing
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
| `tool_availability.rs` | Tool detection (`adb`, `xcrun simctl`, `idevicesyslog`, `log`). `IosLogTool` enum selects the iOS capture backend at runtime. |
| `test_utils.rs` | Test helpers for device/emulator testing |
| `native_logs/mod.rs` | `NativeLogCapture` trait, `NativeLogHandle`, shared types (`NativeLogEvent`, `AndroidLogConfig`, `MacOsLogConfig`, `IosLogConfig`), and `create_native_log_capture()` platform dispatch |
| `native_logs/android.rs` | `AndroidLogCapture` — spawns `adb logcat`, parses logcat output |
| `native_logs/macos.rs` | `MacOsLogCapture` — spawns `log stream`, parses macOS unified log output |
| `native_logs/ios.rs` | `IosLogCapture` — simulator via `xcrun simctl log stream`, physical via `idevicesyslog` (macOS-only, `#[cfg(target_os = "macos")]`) |
| `native_logs/custom.rs` | `CustomLogCapture` — spawns user-defined commands, reads stdout through format parsers; `CustomSourceConfig` — config for a single custom source; `create_custom_log_capture()` factory |
| `native_logs/formats.rs` | `parse_line()` dispatch — routes raw output lines to `parse_raw()`, `parse_json()`, `parse_logcat_threadtime()`, or `parse_syslog()` based on `OutputFormat` |

**Platform Support:**

| Platform | Mechanism          | Module        |
|----------|--------------------|---------------|
| Android  | `adb logcat`       | `android.rs`  |
| macOS    | `log stream`       | `macos.rs`    |
| iOS (sim)| `simctl log stream`| `ios.rs`      |
| iOS (phy)| `idevicesyslog`    | `ios.rs`      |
| Others   | Not needed (pipe)  | —             |

**Tool Dependencies:**
- `adb` — Android Debug Bridge, required for Android logcat capture
- `log` — macOS unified logging tool, required for macOS native log capture
- `xcrun simctl` — Xcode CLI tools, required for iOS simulator log capture
- `idevicesyslog` — part of the `libimobiledevice` suite, required for physical iOS device log capture (optional; graceful degradation if absent)

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
| `session/` | `Session`, `SessionHandle`, per-session state: `PerformanceState`, `NetworkState`, `NativeTagState` |
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
| `tag_filter.rs` | Native tag filter overlay — toggle per-tag visibility, shows tag counts |
| `new_session_dialog/` | New session creation dialog |

### `fdemon-dap` — DAP Server

**Location**: `crates/fdemon-dap/`
**Dependencies**: `fdemon-core` only
**Purpose**: Debug Adapter Protocol implementation — TCP server, protocol types, adapter logic, stdio transport

**Key Design Constraint**: `fdemon-dap` has no dependency on `fdemon-daemon` or
`fdemon-app`. The `DebugBackend` trait abstracts all VM Service operations;
`fdemon-app` provides the concrete `VmServiceBackend` implementation.

| Module | Purpose |
|--------|---------|
| `protocol/types.rs` | All DAP request, response, and event types |
| `protocol/codec.rs` | Content-Length framing encoder/decoder |
| `adapter/mod.rs` | `DapAdapter` struct, `DebugBackend` trait, `DebugEvent` enum |
| `adapter/breakpoints.rs` | `BreakpointState` — DAP ID ↔ VM ID mapping, conditional breakpoints, logpoints |
| `adapter/evaluate.rs` | Expression evaluation handler, `EvalContext` (hover/watch/repl/clipboard) |
| `adapter/stack.rs` | `FrameStore`, `VariableStore`, `SourceReferenceStore` |
| `adapter/threads.rs` | `ThreadMap`, `MultiSessionThreadMap`, session ID namespacing |
| `server/mod.rs` | `DapServer` — TCP accept loop, client session spawning |
| `server/session.rs` | `DapClientSession`, `NoopBackend` (test-only backend) |
| `transport/stdio.rs` | Stdio transport for IDE integration testing |

### `flutter-demon` (Binary) — Headless Mode

**Location**: `src/headless/`
**Dependencies**: `fdemon-core`, `fdemon-daemon`, `fdemon-app`, `fdemon-tui`, `fdemon-dap`
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
├── request_tracker: Arc<RequestTracker>
├── vm_shutdown_tx / vm_request_handle  (VM Service connection)
├── perf_shutdown_tx / perf_task_handle  (performance monitoring task)
├── network_shutdown_tx / network_task_handle  (network monitoring task)
├── debug_shutdown_tx / debug_task_handle  (DAP debug event task)
├── native_log_shutdown_tx / native_log_task_handle  (platform capture task)
├── native_tag_state: NativeTagState  (discovered tags + visibility)
└── custom_source_handles: Vec<CustomSourceHandle>  (per-source handles)

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

## DevTools Subsystem

The DevTools mode provides three inspection panels — Inspector, Performance, and Network — accessible by pressing `d` when a Flutter session has a VM Service connection.

### Architecture Overview

```
┌──────────────────────────────────────────────────────────┐
│                    DevTools View                          │
│           (fdemon-tui/widgets/devtools/)                  │
│  ┌────────────┐  ┌────────────────┐  ┌────────────────┐  │
│  │ Inspector  │  │  Performance   │  │   Network      │  │
│  │ tree_panel │  │  frame_chart   │  │ request_table  │  │
│  │layout_panel│  │  memory_chart  │  │request_details │  │
│  └──────┬─────┘  └──────┬─────────┘  └──────┬─────────┘  │
└─────────┼───────────────┼───────────────────┼────────────┘
          │               │                   │
          ▼               ▼                   ▼
┌──────────────────────────────────────────────────────────┐
│               DevTools Handlers                          │
│         (fdemon-app/handler/devtools/)                    │
│  inspector.rs   performance.rs   network.rs   mod.rs     │
└─────────┬───────────────┬───────────────────┬────────────┘
          │               │                   │
          ▼               ▼                   ▼
┌──────────────────────────────────────────────────────────┐
│              Per-Session State                            │
│         (fdemon-app/session/)                             │
│  InspectorState    PerformanceState    NetworkState       │
│  (in state.rs)     (performance.rs)    (network.rs)      │
└─────────┬───────────────┬───────────────────┬────────────┘
          │               │                   │
          ▼               ▼                   ▼
┌──────────────────────────────────────────────────────────┐
│              VM Service Client                           │
│        (fdemon-daemon/vm_service/)                        │
│  extensions/    performance.rs    network.rs   timeline  │
└─────────┬───────────────┬───────────────────┬────────────┘
          │               │                   │
          ▼               ▼                   ▼
┌──────────────────────────────────────────────────────────┐
│              Domain Types                                │
│            (fdemon-core/)                                 │
│  widget_tree.rs    performance.rs    network.rs           │
└──────────────────────────────────────────────────────────┘
```

### Panel State Model

DevTools state lives at two levels:

- **View state** (`DevToolsViewState` in `state.rs`): UI-level state shared across sessions — active panel, overlay toggles, VM connection status. Reset when exiting DevTools mode.
- **Session state** (`PerformanceState`, `NetworkState` on `Session`): Per-session data (frame history, memory samples, network entries). Persists across tab switches and survives DevTools mode exit.

### VM Service Data Flow

1. Engine spawns background polling tasks (performance monitor, network monitor) when a session connects
2. Polling tasks call VM Service extensions via `VmServiceHandle`
3. Responses are parsed into domain types (`MemorySample`, `HttpProfileEntry`, etc.)
4. Results sent as `Message` variants to the Engine message channel
5. Handler functions update per-session state
6. TUI renders the updated state on the next frame

---

## DAP Server Subsystem

The DAP server enables IDE debuggers (VS Code, Zed, Neovim, Helix) to attach to
Flutter sessions managed by fdemon via the Debug Adapter Protocol.

### Architecture Overview

```
┌──────────────────────────────────────────────────────────────┐
│                      IDE (DAP client)                        │
│              VS Code / Zed / Neovim / Helix                  │
└────────────────────────┬─────────────────────────────────────┘
                         │ TCP (DAP wire protocol)
                         ▼
┌──────────────────────────────────────────────────────────────┐
│                    fdemon-dap crate                          │
│  ┌────────────────┐  ┌──────────────────────────────────┐   │
│  │   DapServer    │  │         DapClientSession         │   │
│  │ (TCP listener) │──│  (per-connection state machine)  │   │
│  └────────────────┘  └──────────────┬───────────────────┘   │
│                                     │                        │
│                          ┌──────────▼──────────┐            │
│                          │      DapAdapter      │            │
│                          │  (protocol handler)  │            │
│                          └──────────┬───────────┘            │
│                                     │ DebugBackend trait     │
└─────────────────────────────────────┼──────────────────────┘
                                      │
┌─────────────────────────────────────┼──────────────────────┐
│               fdemon-app crate      │                       │
│                                     ▼                       │
│                          ┌──────────────────────┐          │
│                          │  VmServiceBackend    │          │
│                          │ (DebugBackend impl)  │          │
│                          └──────────┬───────────┘          │
│                                     │                       │
│          ┌──────────────────────────┼──────────┐           │
│          ▼                          ▼           ▼           │
│  dap_debug_senders          TEA Engine    VmRequestHandle  │
│  (DebugEvent channel)      (hot reload)   (VM Service RPC) │
└──────────────────────────────────────────────────────────┘
```

### Debug Event Flow

VM Service debug events (breakpoint hit, resume, isolate created) are translated
into DAP events and forwarded to connected IDE clients:

```
Dart VM Service
    │
    ├── "Debug" stream events ──────────────────────┐
    │   (PauseBreakpoint, Resume, PauseException)   │
    │                                               ▼
    │                                  actions/vm_service.rs
    │                                  (VM event forwarding loop)
    │                                               │
    │                                               ▼
    │                                  Message::VmServiceDebugEvent
    │                                               │
    │                                               ▼
    │                                  handler/devtools/debug.rs
    │                                  handle_debug_event()
    │                                               │
    │                               ┌───────────────┴─────────────────┐
    │                               ▼                                 ▼
    │                    Mutate per-session DebugState        Translate to DapDebugEvent
    │                    (paused/resumed/isolate state)       (Paused/Resumed/ThreadExited)
    │                                                                  │
    └── "Isolate" stream events                                        ▼
        (IsolateStart, IsolateExit)                     dap_debug_senders registry
                │                                       (one mpsc::Sender per DAP client)
                ▼                                                       │
        handler/devtools/debug.rs                                       ▼
        handle_isolate_event()                             DapAdapter.process_debug_event()
                                                                        │
                                                                        ▼
                                                           IDE receives stopped/continued/
                                                           thread DAP events
```

### Channel Architecture: `dap_debug_senders`

The `dap_debug_senders` registry is the bridge between the TEA message loop and
the per-connection DAP adapters:

```
AppState
└── dap_debug_senders: Arc<Mutex<Vec<mpsc::Sender<DebugEvent>>>>
          │
          │ (one entry per connected DAP client)
          │
          ├── Sender → DapClientSession 1 (IDE window 1)
          ├── Sender → DapClientSession 2 (IDE window 2)
          └── ...
```

- When a DAP client attaches, the Engine creates an `mpsc` channel and registers
  the `Sender` in `dap_debug_senders`.
- The TEA handler calls `try_send` on each sender when a debug event arrives.
- Stale senders (where the DAP client disconnected) are pruned automatically:
  `try_send` returns `Err` for a closed channel, and the handler uses `retain`
  to remove them.

### Breakpoint State Model

Each `DapAdapter` instance holds a `BreakpointState` that tracks the mapping
between DAP breakpoint IDs (integers) and VM Service breakpoint IDs (strings):

```
setBreakpoints (IDE request)
    │
    ▼
BreakpointState
├── by_dap_id: HashMap<i64, BreakpointEntry>
│   └── BreakpointEntry {
│       dap_id, vm_id, uri, line, column, verified,
│       condition, hit_condition, hit_count, log_message
│   }
└── vm_id_to_dap_id: HashMap<String, i64>

When VM emits PauseBreakpoint (vm_id):
  1. Look up dap_id via vm_id_to_dap_id
  2. Increment hit_count
  3. Evaluate hit_condition (if any) — cheap, no VM RPC
  4. Evaluate condition via evaluateInFrame (if any)
  5. If logpoint: interpolate {expression} and emit output event, auto-resume
  6. If all conditions pass: emit stopped event to IDE
```

### Multi-Session Thread ID Namespacing

Each Flutter session is assigned a dedicated thread ID range so that isolates
from different sessions cannot collide:

```
Session index  │  Thread ID range  │  Formula
───────────────┼───────────────────┼─────────────────────────────
0              │  1000–1999        │  (0+1) × 1000 = 1000
1              │  2000–2999        │  (1+1) × 1000 = 2000
2              │  3000–3999        │  (2+1) × 1000 = 3000
…              │  …                │  …
8              │  9000–9999        │  (8+1) × 1000 = 9000
```

Given a thread ID, the session index is recovered as: `(thread_id / 1000) - 1`.
The `ThreadMap` inside each session converts between Dart isolate IDs (strings
like `"isolates/12345"`) and namespaced DAP thread IDs (integers).

### Coordinated Pause: Auto-Reload Suppression

When the Dart VM pauses an isolate (breakpoint, exception, step), file-watcher
triggered hot reloads are suppressed to avoid invalidating the paused stack
frame:

```
PauseBreakpoint event received
    │
    ▼
handle_debug_event()
    ├── Update DebugState (paused = true)
    ├── Forward DapDebugEvent::Paused to IDE clients
    └── Emit Message::SuspendFileWatcher (follow-up)
            │
            ▼
    AppState.file_watcher_suspended = true
    (file changes queued in pending_watcher_changes)

Resume event received (or DAP client disconnects)
    │
    ▼
    AppState.file_watcher_suspended = false
    If pending_watcher_changes > 0: trigger single hot reload
```

### Custom DAP Events

On successful `attach`, fdemon emits three custom events to the IDE:

```
dart.debuggerUris
  body: { "vmServiceUri": "ws://127.0.0.1:PORT/..." }
  → Allows IDE to connect supplementary tooling (Dart DevTools) to the same
    VM Service connection

flutter.appStart
  body: { "deviceId": "...", "mode": "debug", "supportsRestart": true }
  → Signals session metadata to the IDE debugger extension

flutter.appStarted
  body: {}
  → Emitted when the VM signals the app is fully started (IsolateRunnable /
    AppStarted VM event)
```

### `DebugBackend` Trait

`fdemon-dap` defines the `DebugBackend` trait so it does not depend on
`fdemon-daemon` or `fdemon-app`. The concrete implementation,
`VmServiceBackend`, lives in `fdemon-app/src/handler/dap_backend.rs`:

```
fdemon-dap (defines trait)              fdemon-app (implements trait)
┌───────────────────────────┐          ┌──────────────────────────┐
│ pub trait DebugBackend {  │          │ pub struct VmServiceBackend {
│   pause(isolate_id)       │◄─────────│   handle: VmRequestHandle │
│   resume(isolate_id, step)│          │   msg_tx: mpsc::Sender     │
│   add_breakpoint(...)     │          │   ws_uri: Option<String>   │
│   evaluate_in_frame(...)  │          │   device_id: Option<String>│
│   hot_reload()            │          │   build_mode: String       │
│   hot_restart()           │          │ }                          │
│   ws_uri()                │          │                            │
│   get_source(...)         │          │ // hot_reload / hot_restart│
│   ...                     │          │ // send Message::HotReload │
│ }                         │          │ // into TEA pipeline       │
└───────────────────────────┘          └──────────────────────────┘
```

`hot_reload()` and `hot_restart()` on `VmServiceBackend` send
`Message::HotReload` / `Message::HotRestart` into the TEA pipeline rather than
calling VM Service RPCs directly. This ensures reload lifecycle, phase tracking,
and EngineEvent broadcasting all work consistently whether reload is triggered
from the TUI, file watcher, or IDE.

---

## Native Log Capture Subsystem

Flutter apps on Android and iOS/macOS emit native platform logs (e.g., Go plugin logs, OkHttp network logs) that do not appear on Flutter's stdout/stderr pipe. The native log capture subsystem bridges these platform-specific log streams into the fdemon log view.

### Architecture

```
FlutterProcess starts
    │
    ▼
fdemon-daemon: create_native_log_capture(platform, …)
    │
    ├── "android" ──► AndroidLogCapture
    │                 spawns: adb logcat --pid <pid>
    │
    ├── "macos"   ──► MacOsLogCapture
    │                 spawns: log stream --process <name>
    │
    └── "ios"     ──► IosLogCapture
                      ├── is_simulator=true → xcrun simctl spawn <udid> log stream
                      └── is_simulator=false → idevicesyslog -u <udid> -p <process>
```

Each backend implements `NativeLogCapture::spawn()` which returns a `NativeLogHandle` with:
- `event_rx`: `mpsc::Receiver<NativeLogEvent>` — parsed log events
- `shutdown_tx`: `watch::Sender<bool>` — graceful stop signal
- `task_handle`: `JoinHandle<()>` — background task (abortable as fallback)

### Tag Filtering

All native log events include a `tag` field (e.g., `"GoLog"`, `"OkHttp"`). Per-session tag state is tracked in `NativeTagState` (in `fdemon-app/session/native_tags.rs`):

- Tags are discovered as events arrive and added to `discovered_tags` (a `BTreeMap<String, usize>` tracking count per tag)
- Users can hide individual tags via the tag filter overlay (press `T` in normal mode)
- Hidden tags are stored in `hidden_tags` (`BTreeSet<String>`)
- Filtering is applied at the handler level: entries for hidden tags are not added to the session log buffer
- Un-hiding a tag only applies to future entries (consistent with `LogSourceFilter` behaviour)

### Per-Tag Configuration

Individual tags can be configured in `.fdemon/config.toml` under `[native_logs.tags.<TagName>]`:

```toml
[native_logs.tags.GoLog]
min_level = "debug"   # per-tag minimum level override

[native_logs.tags.OkHttp]
min_level = "info"
```

### Tool Dependencies

| Tool | Platform | Purpose | Availability |
|------|----------|---------|--------------|
| `adb` | Android | logcat log capture | Required for Android native logs |
| `log` | macOS | unified log stream capture | Required for macOS native logs |
| `xcrun simctl` | macOS (iOS sim) | iOS simulator log stream | Requires Xcode CLI tools |
| `idevicesyslog` | macOS (iOS phy) | Physical iOS device syslog relay | Optional; part of `libimobiledevice`. Graceful degradation if absent. |

### Custom Log Sources

Users can define arbitrary log source processes via `[[native_logs.custom_sources]]` configuration. Each custom source implements the same `NativeLogCapture` trait as platform backends.

#### Architecture

```
                     NativeLogCapture trait
┌──────────────────────────────────────────────────────────────────┐
│ AndroidLogCapture │ MacOsLogCapture │ IosLogCapture │ CustomLogCapture │
│ (adb logcat)      │ (log stream)    │ (xcrun simctl/│ (user-defined    │
│                   │                 │  idevicesyslog)│  command)        │
└──────────────────────────────────────────────────────────────────┘
          │                  │               │               │
          └──────────────────┴───────────────┴───────────────┘
                                     │
                              NativeLogEvent
                                     │
                         ┌───────────┴───────────┐
                         │   Format Parser        │
                         │   (formats.rs)         │
                         │   Raw│Json│Logcat│Syslog│
                         └───────────────────────┘
                                     │
                         Message::NativeLog
                                     │
                         handler::update()
                                     │
                         NativeTagState + log buffer
```

`CustomLogCapture` is separate from `create_native_log_capture()` (which dispatches by platform string). Multiple custom sources can be active concurrently within a single session.

**Key design decisions:**

- **No shell expansion**: Commands are spawned directly via `tokio::process::Command::new` with explicit args — never `sh -c`. This avoids injection risks.
- **No auto-restart**: If the process exits, a warning is logged and the capture stops. Users must fix their command configuration.
- **stderr not parsed**: stderr is piped to avoid orphaned pipe errors but its output is not forwarded as log events.
- **Tag filtering**: Reuses `should_include_tag()` from `native_logs/mod.rs` with the per-source `include_tags`/`exclude_tags` lists.

#### Format Parser Dispatch (`native_logs/formats.rs`)

The `formats` module provides pluggable output parsing for custom sources via the `parse_line()` dispatch function:

| Format | `OutputFormat` variant | Parser | Behavior |
|--------|------------------------|--------|----------|
| `raw` | `OutputFormat::Raw` | `parse_raw()` | Each non-empty line → `NativeLogEvent` (Info level, tag = source name) |
| `json` | `OutputFormat::Json` | `parse_json()` | JSON objects with flexible field aliases: message/msg/text, tag/source/logger, level/severity/priority, timestamp/time/ts |
| `logcat-threadtime` | `OutputFormat::LogcatThreadtime` | delegates to `android::parse_threadtime_line()` + `android::logcat_line_to_event()` | Android logcat threadtime format |
| `syslog` | `OutputFormat::Syslog` | delegates to `macos::parse_syslog_line()` + `macos::syslog_line_to_event()` | macOS/iOS unified logging compact format (macOS-only; returns `None` on other platforms) |

Custom sources integrate with the existing pipeline identically to platform backends:
- Events flow through `NativeLogEvent` → `Message::NativeLog` → handler path
- Tags are tracked in `NativeTagState` and appear in the tag filter overlay (`T` key)
- `should_include_tag()` filtering applies identically to platform backends
- `min_level` filtering uses the same `effective_min_level()` logic

#### Custom Source Lifecycle Messages

Two `Message` variants manage custom source lifecycle:

| Message | When sent | Purpose |
|---------|-----------|---------|
| `CustomSourceStarted { session_id, name, shutdown_tx, task_handle }` | After `CustomLogCapture::spawn()` succeeds | TEA handler stores `shutdown_tx` and `task_handle` in `SessionHandle::custom_source_handles` |
| `CustomSourceStopped { session_id, name }` | When the source's event channel closes (process exited) | TEA handler removes the named handle from `custom_source_handles` |

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

---

## API Surface

### Public API Boundaries

Each crate in the workspace has a clearly defined public API. Only items exported from `lib.rs` are considered public. Items marked `pub(crate)` are internal implementation details.

#### `fdemon-core` — Domain Types

**Public API** (exported from `lib.rs`):
- `LogEntry`, `LogLevel`, `LogSource` — Log entries and metadata
- `AppPhase` — Application lifecycle phases
- `DaemonMessage`, `DaemonEvent` — Events from Flutter daemon
- `Error`, `Result<T>` — Error handling types
- `is_runnable_flutter_project()`, `discover_flutter_projects()` — Project discovery
- `prelude` module — Common imports

**Internal** (`pub(crate)`):
- Protocol parsing helpers
- Stack trace implementation details

#### `fdemon-daemon` — Flutter Process Management

**Public API** (exported from `lib.rs`):
- `Device`, `Emulator`, `AndroidAvd`, `IosSimulator` — Device types
- `discover_devices()`, `discover_emulators()`, `launch_emulator()` — Discovery functions
- `FlutterProcess` — Process spawning and lifecycle
- `CommandSender`, `DaemonCommand` — Command dispatch
- `ToolAvailability` — Tool detection

**Internal** (`pub(crate)`):
- JSON-RPC protocol parsing (`protocol.rs`)
- Request tracking implementation
- AVD/simulator utilities

#### `fdemon-app` — Application State and Orchestration

**Public API** (exported from `lib.rs`):
- `Engine` — Orchestration core
- `EngineEvent` — Domain events for external consumers
- `EnginePlugin` — Extension trait for plugins
- `AppState` — TEA model (read-only access recommended)
- `Message` — TEA messages
- `UpdateAction`, `UpdateResult` — TEA update outputs
- `Session`, `SessionHandle`, `SessionManager` — Session types
- `services::FlutterController` — Reload/restart operations
- `services::LogService` — Log buffer access
- `services::StateService` — App state queries
- `config::Settings`, `config::LaunchConfig` — Configuration types

**Internal** (`pub(crate)`):
- TEA handler implementation (`handler/`)
- Process spawning logic (`process.rs`, `spawn.rs`)
- Signal handling (`signals.rs`)
- Action dispatching (`actions/` — modular directory with `mod.rs`, `session.rs`, `vm_service.rs`, `performance.rs`, `inspector/`, `network.rs`)

#### `fdemon-dap` — DAP Server

**Public API** (exported from `lib.rs`):
- `DapServer`, `DapServerHandle` — TCP server lifecycle
- `DapClientSession`, `NoopBackend` — Session and test backend
- `DapMessage`, `DapRequest`, `DapResponse` — Protocol message types
- `DebugBackend`, `DebugEvent`, `StepMode`, `BackendError` — Backend trait and types
- `BreakpointState`, `BreakpointCondition`, `BreakpointResult` — Breakpoint tracking
- `FrameStore`, `VariableStore`, `SourceReferenceStore` — Reference stores
- `ThreadMap`, `MultiSessionThreadMap` — Thread ID mapping
- `parse_log_message`, `LogSegment` — Logpoint interpolation
- `run_dap_stdio()` — Stdio transport entry point

**Internal** (`pub(crate)`):
- Protocol codec (Content-Length framing)
- Adapter handler methods

#### `fdemon-tui` — Terminal UI

**Public API** (exported from `lib.rs`):
- `run_with_project()` — Main TUI entry point
- Widget types are not exported (TUI-specific)

**Internal** (`pub(crate)`):
- All rendering logic
- Terminal setup/cleanup
- Event polling

### Visibility Conventions

| Visibility | Meaning | External Access |
|------------|---------|-----------------|
| `pub` (in `lib.rs`) | Public API | ✅ Stable, documented, supported |
| `pub` (in submodule) | Crate-public | ⚠️ Internal, may change |
| `pub(crate)` | Crate-internal | ❌ Private implementation detail |
| `pub(super)` | Parent module only | ❌ Private implementation detail |
| (no visibility) | Module-private | ❌ Private implementation detail |

**Rule:** External consumers should only use items exported from `lib.rs`. Importing from submodules (e.g., `use fdemon_app::handler::update`) is unsupported and may break.

---

## Extension Points

The Engine provides two extension mechanisms for pro features (MCP server, remote SSH, desktop apps):

### 1. Event Subscription (`Engine::subscribe()`)

Async broadcast channel for observing domain events. Best for read-only consumers that need async processing.

```rust
let mut rx = engine.subscribe();

tokio::spawn(async move {
    while let Ok(event) = rx.recv().await {
        match event {
            EngineEvent::ReloadCompleted { session_id, time_ms } => {
                // Forward to remote client
            }
            EngineEvent::LogBatch { session_id, entries } => {
                // Stream logs
            }
            _ => {}
        }
    }
});
```

**Key Properties:**
- **Non-blocking**: Subscribers receive events via async channel
- **Multiple subscribers**: Each call to `subscribe()` creates a new receiver
- **Lagging policy**: If a subscriber falls behind, older events are dropped
- **Event types**: 15 event types covering sessions, phases, reloads, logs, devices, files

See `engine_event.rs` for the full `EngineEvent` enum.

### 2. Plugin Trait (`EnginePlugin`)

Synchronous lifecycle callbacks for tighter integration. Best for features that need to react to every message or participate in the Engine lifecycle.

```rust
#[derive(Debug)]
struct MetricsPlugin {
    reload_count: AtomicUsize,
}

impl EnginePlugin for MetricsPlugin {
    fn name(&self) -> &str { "metrics" }

    fn on_start(&self, state: &AppState) -> Result<()> {
        // Called when Engine starts
        Ok(())
    }

    fn on_message(&self, msg: &Message, state: &AppState) -> Result<()> {
        // Called after each message is processed
        Ok(())
    }

    fn on_event(&self, event: &EngineEvent) -> Result<()> {
        // Called for each EngineEvent
        if matches!(event, EngineEvent::ReloadCompleted { .. }) {
            self.reload_count.fetch_add(1, Ordering::SeqCst);
        }
        Ok(())
    }

    fn on_shutdown(&self) -> Result<()> {
        // Called during shutdown
        Ok(())
    }
}

// Registration
engine.register_plugin(Box::new(MetricsPlugin { reload_count: AtomicUsize::new(0) }));
engine.notify_plugins_start();
```

**Key Properties:**
- **Synchronous**: Hooks are called inline with message processing
- **Lifecycle**: Covers start, per-message, per-event, shutdown
- **Thread-safe**: Must be `Send + Sync`
- **Error handling**: Plugin errors are logged but don't crash the Engine

### 3. Service Traits

Programmatic access to Flutter operations via trait-based abstractions.

**`FlutterController`** (`services/flutter_controller.rs`):
```rust
if let Some(controller) = engine.flutter_controller() {
    controller.reload().await?;
    controller.restart().await?;
    controller.stop().await?;
    let running = controller.is_running().await;
}
```

**`LogService`** (`services/log_service.rs`):
```rust
let log_service = engine.log_service();
let logs = log_service.get_logs(100).await;
let count = log_service.count().await;
```

**`StateService`** (`services/state_service.rs`):
```rust
let state_service = engine.state_service();
let phase = state_service.phase().await;
let info = state_service.project_info().await;
let running = state_service.is_running().await;
```

**Key Properties:**
- **Trait-based**: Abstracts daemon implementation details
- **Async**: All operations return `async` futures
- **Testable**: Traits can be mocked for testing
- **Thread-safe**: Uses `Arc<SharedState>` internally

### Extension Point Comparison

| Feature | Event Subscription | Plugin Trait | Service Traits |
|---------|-------------------|--------------|----------------|
| **Async** | ✅ Yes | ❌ No | ✅ Yes |
| **Multiple consumers** | ✅ Yes | ✅ Yes | ✅ Yes |
| **Read state** | ✅ Events only | ✅ Full state | ✅ Via services |
| **Write state** | ❌ No | ❌ No | ✅ Commands only |
| **Lifecycle hooks** | ❌ No | ✅ Yes | ❌ No |
| **Best for** | Remote forwarding | Metrics, logging | Control operations |

For detailed examples and usage patterns, see [Extension API Documentation](./EXTENSION_API.md).

---

## Future Considerations

- **Remote MCP Server**: The Engine's event broadcasting and service traits are designed to support an MCP server that can control Flutter Demon from Claude Desktop or other AI tools
- **SSH Remote Development**: The headless mode and shared state architecture enable remote Flutter development workflows
- **Multi-Project Workspaces**: The single-session architecture could be extended to support multiple concurrent projects in a workspace view
- **Time-Travel Debugging**: The TEA pattern (pure update function) enables recording and replaying state transitions for debugging
