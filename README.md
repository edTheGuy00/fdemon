<p align="center">
    <img src="docs/images/logo.png" width="200">
    <br>
    <br>
    <strong>Flutter Demon</strong>
    <br>
    <em>A blazingly fast TUI for Flutter development</em>
    <br>
    <br>
    <a href="https://github.com/edTheGuy00/fdemon/releases">
        <img src="https://img.shields.io/github/v/release/edTheGuy00/fdemon?style=flat&labelColor=1d1d1d&color=54c5f8&logo=GitHub&logoColor=white" alt="GitHub Release"></a>
    <a href="https://github.com/edTheGuy00/fdemon/actions">
        <img src="https://img.shields.io/github/actions/workflow/status/edTheGuy00/fdemon/ci.yml?style=flat&labelColor=1d1d1d&color=white&logo=GitHub%20Actions&logoColor=white" alt="CI"></a>
    <a href="https://github.com/edTheGuy00/fdemon/blob/main/LICENSE">
        <img src="https://img.shields.io/badge/license-BSL%201.1-white?style=flat&labelColor=1d1d1d" alt="License"></a>
    <br>
    <br>
    <a href="https://fdemon.dev">Website</a> &middot;
    <a href="https://fdemon.dev/docs">Documentation</a> &middot;
    <a href="https://fdemon.dev/docs/keybindings">Keybindings</a> &middot;
    <a href="https://fdemon.dev/docs/configuration">Configuration</a>
</p>

---

**Flutter Demon** (`fdemon`) is a high-performance terminal UI for Flutter development. Run apps, view logs in real-time, hot reload on file changes, and manage multiple device sessions — all from your terminal.

<p align="center">
    <img src="docs/images/log-view.png" width="600" alt="Log view with real-time logs, hot reload, and file watcher">
</p>

## Installation

```bash
curl -fsSL https://raw.githubusercontent.com/edTheGuy00/fdemon/main/install.sh | bash
```

This downloads the latest release binary for your platform and installs it to `$HOME/.local/bin`.

See the [installation guide](https://fdemon.dev/docs/installation) for version-specific installs, custom directories, Windows, and building from source.

## Features

- **Real-time log viewing** with level/source filtering, regex search, and error navigation
- **Auto hot reload** on file save with smart debouncing
- **Multi-device sessions** — run on up to 9 devices simultaneously
- **Built-in DevTools** — widget inspector, performance monitor, network monitor
- **New Session Dialog** — device selection, launch configs, dart defines
- **Link Highlight Mode** — open files from log traces directly in your editor
- **Smart project discovery** — auto-detects Flutter apps, plugins, and workspaces

## Quick Start

```bash
cd /path/to/my_flutter_app
fdemon
```

Select a device, configure launch settings, and press `Enter` to launch.

<p align="center">
    <img src="docs/images/new-session.png" width="500" alt="New Session dialog with device selection and launch configuration">
</p>

## DevTools

Press `d` to enter DevTools mode. Three panels are available:

| Key | Panel | Description |
|-----|-------|-------------|
| `i` | **Widget Inspector** | Browse the widget tree, view layout details and source locations |
| `p` | **Performance** | Real-time FPS, memory usage, jank detection, allocation table |
| `n` | **Network** | HTTP request capture, detail tabs, filtering, recording controls |

<p align="center">
    <img src="docs/images/widget-inspector.png" width="420" alt="Widget Inspector with tree view and Layout Explorer">
    &nbsp;&nbsp;
    <img src="docs/images/performance-monitor.png" width="420" alt="Performance Monitor with FPS, memory, and allocations">
</p>

Debug overlays (`Ctrl+r` repaint rainbow, `Ctrl+p` performance, `Ctrl+d` debug paint) and browser DevTools (`b`) are also available.

## Key Bindings

| Key | Action |
|-----|--------|
| `r` / `R` | Hot reload / Hot restart |
| `d` | Enter DevTools |
| `+` | New session |
| `1-9` / `Tab` | Switch session |
| `f` / `F` | Cycle level/source filter |
| `/` | Search logs |
| `e` / `E` | Next/previous error |
| `L` | Link highlight mode |
| `q` `q` | Quit |

Full reference: [fdemon.dev/docs/keybindings](https://fdemon.dev/docs/keybindings)

## Configuration

Flutter Demon works out-of-the-box with sensible defaults. Optionally configure via:

- **`.fdemon/config.toml`** — Behavior, watcher, UI, editor settings
- **`.fdemon/launch.toml`** — Launch configurations (device, mode, flavor, dart defines)
- **`.vscode/launch.json`** — Auto-imported (read-only)

Full reference: [fdemon.dev/docs/configuration](https://fdemon.dev/docs/configuration)

## Requirements

- **Flutter SDK** in your PATH
- A terminal with Unicode support

## Contributing

Contributions are welcome! See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

```bash
cargo build && cargo test && cargo clippy
```

## License

[Business Source License 1.1](LICENSE) — free for all use except providing a commercial hosted Flutter development service. Converts to AGPL-3.0 after four years.

---

<p align="center">
    Made with 🔥 for Flutter developers
</p>
