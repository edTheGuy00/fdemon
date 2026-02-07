# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Flutter Demon (`fdemon`) is a high-performance terminal user interface (TUI) for Flutter development, written in Rust. It provides real-time log viewing, hot reload on file changes, and multi-device session management.

## Build Commands

```bash
# Workspace commands
cargo build --workspace        # Build all crates
cargo test --workspace         # Test all crates
cargo test --lib               # Unit tests only
cargo fmt --all                # Format all crates
cargo clippy --workspace       # Lint all crates

# Per-crate commands
cargo check -p fdemon-core     # Check specific crate
cargo test -p fdemon-app       # Test specific crate
```

Run the binary: `cargo run -- /path/to/flutter/project` or `cargo run` from a Flutter project directory.

## Architecture

The project follows **The Elm Architecture (TEA)** pattern with a **Cargo workspace** structure:

```
┌─────────────────────────────────────────────────┐
│         flutter-demon (binary crate)            │
│         CLI + headless mode                     │
└──────────────┬──────────────────────────────────┘
               │
       ┌───────┴────────┐
       ▼                ▼
┌─────────────┐  ┌─────────────┐
│ fdemon-tui  │  │ fdemon-app  │
│ Terminal UI │  │ Engine, TEA │
└──────┬──────┘  └──────┬──────┘
       │                │
       │         ┌──────┴──────┐
       │         ▼             ▼
       │  ┌─────────────┐ ┌─────────────┐
       │  │fdemon-daemon│ │ fdemon-core │
       │  │Flutter I/O  │ │Domain types │
       │  └──────┬──────┘ └─────────────┘
       │         │
       └─────────┘
```

### Workspace Crates

- **`fdemon-core`** (`crates/fdemon-core/`): Domain types (`LogEntry`, `LogLevel`, `AppPhase`), project discovery, error handling. **Zero internal dependencies.**
- **`fdemon-daemon`** (`crates/fdemon-daemon/`): Flutter process management, JSON-RPC protocol parsing (`--machine` mode), device/emulator discovery. Depends on `fdemon-core`.
- **`fdemon-app`** (`crates/fdemon-app/`): TEA implementation - `AppState` (model), `Message` (events), `handler::update()` (state transitions), Engine orchestration, services, config, watcher. Depends on `fdemon-core` + `fdemon-daemon`.
- **`fdemon-tui`** (`crates/fdemon-tui/`): Ratatui-based terminal UI with widgets. Depends on `fdemon-core` + `fdemon-app`.
- **`flutter-demon`** (binary): CLI parsing, project discovery, headless mode. Depends on all 4 crates.

### Data Flow

1. Events (keyboard, daemon, file watcher) → `Message` enum
2. `handler::update(state, message)` → returns `(new_state, Option<UpdateAction>)`
3. `tui::render(state)` → draws to terminal
4. `UpdateAction` triggers async tasks (reload, spawn process, etc.)

### Multi-Session Architecture

`SessionManager` holds up to 9 concurrent `SessionHandle` instances, each with its own `FlutterProcess`, logs, and state. Sessions are identified by UUID and ordered for tab display.

## Testing

Unit tests use inline `#[cfg(test)] mod tests` or separate `tests.rs` files for larger suites:

- `crates/fdemon-core/src/` - 243 unit tests
- `crates/fdemon-daemon/src/` - 136 unit tests
- `crates/fdemon-app/src/handler/tests.rs` - 726 unit tests (state transitions)
- `crates/fdemon-tui/src/widgets/` - 427 unit tests (rendering)
- `tests/` directory - Integration tests (binary crate)

Total: 1,532 unit tests across 4 crates

## Configuration

- `.fdemon/config.toml` - Global settings (watcher paths, debounce, UI options, editor)
- `.fdemon/launch.toml` - Launch configurations (device, mode, flavor, dart-defines)
- `.vscode/launch.json` - Auto-imported VSCode Dart configurations

## Key Patterns

- **Workspace structure**: 4 library crates with compile-time enforced layer boundaries
- **Custom errors**: `fdemon-core/error.rs` defines `Error` enum with `fatal` vs `recoverable` classification
- **Request tracking**: `fdemon-daemon/commands.rs` tracks JSON-RPC request/response pairs via `RequestTracker`
- **Log parsing**: `fdemon-daemon/protocol.rs` parses Flutter's `--machine` JSON-RPC output
- **Stack trace detection**: `fdemon-core/stack_trace.rs` parses and renders collapsible stack traces
- **Engine abstraction**: `fdemon-app/engine.rs` provides shared orchestration for TUI and headless modes
- **Service layer**: `fdemon-app/services/` provides trait-based abstractions for external consumers
