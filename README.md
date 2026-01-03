# Flutter Demon

A high-performance TUI (Terminal User Interface) for Flutter development.

## Installation

```bash
cargo install --path .
```

Or run directly:

```bash
cargo run -- /path/to/flutter/project
```

## Usage

### Running Flutter Demon

You can run Flutter Demon in several ways:

#### From a Flutter App Directory

```bash
cd /path/to/my_flutter_app
fdemon
```

#### With an Explicit Path

```bash
fdemon /path/to/my_flutter_app
```

#### Auto-Discovery Mode

If you run Flutter Demon from a directory that isn't a runnable Flutter app,
it will automatically search subdirectories for runnable Flutter projects:

```bash
cd /path/to/workspace  # Contains multiple Flutter projects
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

Press the number key to select a project, or 'q' to cancel.

### Project Type Detection

Flutter Demon intelligently detects different project types:

| Type | Runnable? | What Happens |
|------|-----------|--------------|
| **Flutter App** | Yes | Runs directly |
| **Flutter Plugin** | No | Searches `example/` subdirectory |
| **Flutter Package** | No | Skipped (no platform directories) |
| **Dart Package** | No | Skipped (no Flutter dependency) |

A **runnable** Flutter project must have:
- `pubspec.yaml` with `sdk: flutter` dependency
- At least one platform directory (`android/`, `ios/`, `macos/`, `web/`, `linux/`, `windows/`)
- NOT be a plugin (no `flutter: plugin:` section)

#### Working with Plugins

If you're developing a Flutter plugin, run Flutter Demon from the plugin directory
and it will automatically find and use the `example/` project:

```bash
cd /path/to/my_plugin
fdemon

# Output:
# Detected Flutter plugin at: /path/to/my_plugin
#    Plugins cannot be run directly. Searching for runnable examples...
#
# Found Flutter project: /path/to/my_plugin/example
```

### Keyboard Controls

While in the TUI:

| Key | Action |
|-----|--------|
| `q` / `Esc` | Quit |
| `j` / `Down` | Scroll down |
| `k` / `Up` | Scroll up |
| `g` | Go to top |
| `G` | Go to bottom |
| `Page Up` | Page up |
| `Page Down` | Page down |

## Development

### Building

```bash
cargo build
```

### Running Tests

```bash
cargo test
```

### Formatting

```bash
cargo fmt
```

## Architecture

Flutter Demon follows a layered architecture:

```
src/
├── main.rs        # Binary entry point
├── lib.rs         # Library public API
├── common/        # Shared utilities (errors, logging, signals)
├── core/          # Domain types (log entries, discovery)
├── daemon/        # Flutter process + JSON-RPC protocol
├── app/           # Application state (TEA pattern)
└── tui/           # Terminal UI (ratatui)
```

## License

MIT
