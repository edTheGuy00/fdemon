<p align="center">
    <img src="assets/logo.png" width="200">
    <br>
    <br>
    <strong>Flutter Demon</strong>
    <br>
    <br>
    <em>A blazingly fast TUI for Flutter development</em>
    <br>
    <br>
    <a href="https://github.com/edTheGuy00/flutter-demon/releases">
        <img src="https://img.shields.io/github/v/release/edTheGuy00/flutter-demon?style=flat&labelColor=1d1d1d&color=54c5f8&logo=GitHub&logoColor=white" alt="GitHub Release"></a>
    <br>
    <a href="https://github.com/edTheGuy00/flutter-demon/actions">
        <img src="https://img.shields.io/github/actions/workflow/status/edTheGuy00/flutter-demon/ci.yml?style=flat&labelColor=1d1d1d&color=white&logo=GitHub%20Actions&logoColor=white" alt="CI"></a>
    <a href="https://opensource.org/licenses/MIT">
        <img src="https://img.shields.io/badge/license-MIT-white?style=flat&labelColor=1d1d1d" alt="License"></a>
</p>

<h4 align="center">
  <a href="docs/ARCHITECTURE.md">Architecture</a> |
  <a href="#installation">Installation</a> |
  <a href="#usage">Usage</a>
</h4>

---

😈🔥 **Flutter Demon** is a high-performance terminal user interface for Flutter development. Run your Flutter apps, view logs in real-time, hot reload on file changes, and manage multiple device sessions — all from the comfort of your terminal!

## Quickstart

Install with `cargo`:

```bash
cargo install flutter-demon
```

Then run it from your Flutter project directory:

```bash
fdemon
```

That's it! Flutter Demon will automatically detect your Flutter project and start the app. 🚀

## Features

### 🔍 Smart Project Discovery

Flutter Demon intelligently detects different Flutter project types and finds runnable apps:

| Type | Runnable? | Behavior |
|------|-----------|----------|
| **Flutter App** | ✅ Yes | Runs directly |
| **Flutter Plugin** | ❌ No | Auto-discovers `example/` |
| **Flutter Package** | ❌ No | Skipped (no platform dirs) |
| **Dart Package** | ❌ No | Skipped (no Flutter SDK) |

### 📱 Multi-Device Sessions

Run your app on up to 9 devices simultaneously! Switch between sessions with number keys or Tab.

### ⚡ Auto Hot Reload

File watcher monitors your `lib/` directory and triggers hot reload automatically when you save — with smart debouncing to avoid reload spam.

### 🎨 Beautiful TUI

Clean, responsive terminal interface built with [ratatui](https://github.com/ratatui/ratatui):

- Scrollable log view with syntax highlighting
- Session tabs for multi-device development
- Status bar with reload count and timing
- Device/emulator selection modal

## Installation

### From crates.io

```bash
cargo install flutter-demon
```

### From source

```bash
git clone https://github.com/nickmeinhold/flutter-demon
cd flutter-demon
cargo install --path .
```

### Run directly

```bash
cargo run -- /path/to/flutter/project
```

## Usage

### Basic Usage

```bash
# From a Flutter app directory
cd /path/to/my_flutter_app
fdemon

# Or with an explicit path
fdemon /path/to/my_flutter_app
```

### Auto-Discovery Mode

Run from a workspace with multiple Flutter projects:

```bash
cd /path/to/workspace
fdemon
```

If multiple runnable projects are found, you'll see a selection menu:

```
Flutter Demon

Multiple Flutter projects found in:
/path/to/workspace

Select a project:

  [1] app_one
  [2] app_two
  [3] my_plugin/example

Enter number (1-3) or 'q' to quit:
```

### Working with Plugins

Flutter Demon automatically finds the `example/` app in plugin directories:

```bash
cd /path/to/my_plugin
fdemon

# Output:
# 📦 Detected Flutter plugin at: /path/to/my_plugin
#    Plugins cannot be run directly. Searching for runnable examples...
#
# ✅ Found Flutter project: /path/to/my_plugin/example
```

## Keyboard Controls

| Key | Action |
|-----|--------|
| `q` / `Esc` | Quit |
| `r` | Hot reload |
| `R` | Hot restart |
| `d` / `n` | Open device selector |
| `c` | Clear logs |
| `1-9` | Switch to session |
| `Tab` / `Shift+Tab` | Next/previous session |
| `x` / `Ctrl+W` | Close current session |
| `j` / `↓` | Scroll down |
| `k` / `↑` | Scroll up |
| `g` | Go to top |
| `G` | Go to bottom |
| `Page Up/Down` | Page scroll |

## Configuration

Flutter Demon supports configuration via `.fdemon/config.toml`:

```toml
[behavior]
auto_start = false      # Show device selector on startup
confirm_quit = true     # Confirm before quitting with running apps

[watcher]
paths = ["lib"]         # Directories to watch
debounce_ms = 500       # Debounce delay for file changes
auto_reload = true      # Trigger hot reload on file change
extensions = ["dart"]   # File extensions to watch

[ui]
log_buffer_size = 10000
show_timestamps = true
```

### Launch Configurations

Define launch configurations in `.fdemon/launch.toml`:

```toml
[[configurations]]
name = "Development"
device = "iphone"
mode = "debug"
auto_start = true

[configurations.dart_defines]
API_URL = "https://dev.api.example.com"

[[configurations]]
name = "Production"
device = "android"
mode = "release"
flavor = "production"
```

> [!TIP]
> Flutter Demon also reads `.vscode/launch.json` for compatibility with existing VSCode configurations!

## Architecture

For developers interested in contributing or understanding the internals, see the [Architecture Documentation](docs/ARCHITECTURE.md).

**TL;DR:** Flutter Demon follows the **Elm Architecture (TEA)** pattern with a layered design:

```
src/
├── main.rs        # Binary entry point
├── lib.rs         # Library public API
├── common/        # Shared utilities (errors, logging, signals)
├── core/          # Domain types (log entries, discovery)
├── config/        # Configuration parsing (.fdemon/, .vscode/)
├── daemon/        # Flutter process + JSON-RPC protocol
├── watcher/       # File system watching for auto-reload
├── services/      # Reusable service layer
├── app/           # Application state (TEA pattern)
└── tui/           # Terminal UI (ratatui widgets)
```

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

```bash
# Build
cargo build

# Run tests
cargo test

# Format code
cargo fmt

# Run lints
cargo clippy
```

## Requirements

- **Rust** 1.70+ (for building)
- **Flutter SDK** in your PATH
- A terminal with Unicode support

## License

Licensed under the [MIT License](LICENSE).

---

<p align="center">
    Made with 🔥 for Flutter developers
</p>
