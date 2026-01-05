# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Flutter Demon (`fdemon`) is a high-performance terminal user interface (TUI) for Flutter development, written in Rust. It provides real-time log viewing, hot reload on file changes, and multi-device session management.

## Build Commands

```bash
cargo build            # Build the project
cargo test             # Run all tests
cargo test --lib       # Run unit tests only
cargo test log_view    # Run tests matching pattern
cargo fmt              # Format code
cargo clippy           # Run lints
```

Run the binary: `cargo run -- /path/to/flutter/project` or `cargo run` from a Flutter project directory.

## Architecture

The project follows **The Elm Architecture (TEA)** pattern with a layered architecture:

```
Binary (main.rs) → CLI, project discovery
       ↓
App Layer (app/) → TEA state management, Message handling
       ↓
┌──────────────┬──────────────┬──────────────┐
│ TUI (tui/)   │ Daemon       │ Core (core/) │
│ Terminal UI  │ (daemon/)    │ Domain types │
│ Widgets      │ Flutter I/O  │ Discovery    │
└──────────────┴──────────────┴──────────────┘
```

### Key Modules

- **`app/`**: TEA implementation - `AppState` (model), `Message` (events), `handler::update()` (state transitions)
- **`tui/`**: Ratatui-based terminal UI with widgets in `tui/widgets/`
- **`daemon/`**: Flutter process management, JSON-RPC protocol parsing (`--machine` mode)
- **`core/`**: Domain types (`LogEntry`, `LogLevel`, `AppPhase`), project discovery
- **`config/`**: Configuration loading from `.fdemon/config.toml`, `.fdemon/launch.toml`, and `.vscode/launch.json`
- **`services/`**: Abstraction layer (`FlutterController`, `LogService`) for future MCP server integration
- **`watcher/`**: File system monitoring for auto hot reload

### Data Flow

1. Events (keyboard, daemon, file watcher) → `Message` enum
2. `handler::update(state, message)` → returns `(new_state, Option<UpdateAction>)`
3. `tui::render(state)` → draws to terminal
4. `UpdateAction` triggers async tasks (reload, spawn process, etc.)

### Multi-Session Architecture

`SessionManager` holds up to 9 concurrent `SessionHandle` instances, each with its own `FlutterProcess`, logs, and state. Sessions are identified by UUID and ordered for tab display.

## Testing

Unit tests use inline `#[cfg(test)] mod tests` or separate `tests.rs` files for larger suites:

- `src/app/handler/tests.rs` - Handler/state transition tests
- `src/tui/widgets/log_view/tests.rs` - Widget rendering tests
- `tests/` directory - Integration tests

## Configuration

- `.fdemon/config.toml` - Global settings (watcher paths, debounce, UI options, editor)
- `.fdemon/launch.toml` - Launch configurations (device, mode, flavor, dart-defines)
- `.vscode/launch.json` - Auto-imported VSCode Dart configurations

## Key Patterns

- **Custom errors**: `common/error.rs` defines `Error` enum with `fatal` vs `recoverable` classification
- **Request tracking**: `daemon/commands.rs` tracks JSON-RPC request/response pairs via `RequestTracker`
- **Log parsing**: `daemon/protocol.rs` parses Flutter's `--machine` JSON-RPC output
- **Stack trace detection**: `core/stack_trace.rs` parses and renders collapsible stack traces
