# IDE Setup Guide for Flutter Demon DAP Server

This guide explains how to connect your IDE to Flutter Demon's Debug Adapter
Protocol (DAP) server for Flutter debugging.

Flutter Demon exposes a DAP interface so that editors with DAP support can set
breakpoints, step through code, inspect variables, and evaluate expressions
while `fdemon` manages the Flutter process.

## Quick Start

### TCP Mode (Any IDE)

1. Run `fdemon` in your Flutter project directory.
2. Press `D` to start the DAP server (or pass `--dap-port <PORT>` at startup).
3. Note the port shown in the status bar: `[DAP :4711]`.
4. Connect your IDE to `127.0.0.1:<port>`.

### Stdio Mode (Recommended for Zed and Helix)

1. Configure your IDE to launch `fdemon --dap-stdio` as an adapter subprocess.
2. The IDE manages the adapter lifecycle automatically — no manual `fdemon`
   instance is needed.

> **Note:** In stdio mode the TUI is not started. `fdemon` acts purely as a
> DAP adapter: it attaches to the Flutter VM Service and routes DAP messages
> between the IDE and the Dart VM. All log output goes to stderr.

---

## Zed IDE

Zed's Dart/Flutter debugging is not built-in (as of early 2026). Flutter Demon
fills this gap.

### Option A: TCP (Connect to a Running fdemon)

Start `fdemon` in your Flutter project and press `D` to activate the DAP
server, then add a debug configuration in `.zed/debug.json` at the root of
your Flutter project:

```json
[
  {
    "label": "Flutter Demon (TCP)",
    "adapter": "Delve",
    "request": "attach",
    "tcp_connection": {
      "host": "127.0.0.1",
      "port": 4711
    }
  }
]
```

> **Why `"Delve"`?** Zed does not support registering custom DAP adapter names
> without a WASM extension. `Delve` is the only built-in adapter that supports
> pure TCP connect mode (it sets `command = None` when `tcp_connection` is
> present, so no Go debugger is spawned). Zed simply opens a TCP socket to
> fdemon's DAP server. This workaround will be replaced by a proper Zed
> extension in Phase 5.

> **Important:** Do NOT use `"adapter": "custom"` — it is not a valid Zed
> adapter name and will cause Zed to fall back to CodeLLDB (native debugger),
> showing a process picker instead of connecting to the DAP server.

Increase the connection timeout in Zed's `settings.json` if needed (TCP
connections may take a moment on first attach):

```json
{
  "debugger": {
    "timeout": 10000
  }
}
```

### Option B: Stdio (Zed Launches fdemon)

Override an existing adapter's binary in Zed's `settings.json` to point to
`fdemon`:

```json
{
  "dap": {
    "Delve": {
      "binary": "fdemon",
      "args": ["--dap-stdio"]
    }
  }
}
```

Then add a debug configuration in `.zed/debug.json`:

```json
[
  {
    "label": "Flutter Demon",
    "adapter": "Delve",
    "request": "attach"
  }
]
```

> **Requirement:** `fdemon` must be on the system `PATH`. If you installed via
> `cargo install flutter-demon`, the binary is at `~/.cargo/bin/fdemon`. Add
> `~/.cargo/bin` to your shell `PATH` if it is not already there.
>
> **Note:** The `"dap"` key in `settings.json` can only override binaries for
> adapters already in Zed's built-in registry — it cannot register new adapter
> names. Using `"dap": { "fdemon": { ... } }` will **not** work.
>
> **Caveat:** This overrides the Delve adapter globally. If you also use Go
> debugging with Delve, use Option A (TCP) instead, or use project-level
> `.zed/settings.json` to scope the override.

### Option C: Zed Extension (Future — Phase 5)

A proper Zed WASM extension that registers the `FlutterDemon` adapter in the
`DapRegistry`, provides `get_dap_binary`, and auto-detects Flutter projects
via `pubspec.yaml` is planned for Phase 5. Phase 3 covers manual configuration
only.

---

## Helix

Helix marks DAP support as experimental. Known limitations:

- Variable expansion shows a flat popup (no tree view).
- Hover values are not supported.
- The `:debug-remote` command connects over TCP; stdio requires a
  `languages.toml` entry.

### Option A: TCP (Connect to a Running fdemon)

Start `fdemon` in your Flutter project, press `D` to activate the DAP server,
then connect from Helix with:

```
:debug-remote 127.0.0.1:4711
```

No configuration file changes are needed for TCP mode.

### Option B: Stdio (Helix Launches fdemon)

Add a debugger configuration for Dart/Flutter in
`~/.config/helix/languages.toml`:

```toml
[[language]]
name = "dart"

[language.debugger]
name = "fdemon-dap"
transport = "stdio"
command = "fdemon"
args = ["--dap-stdio"]

[[language.debugger.templates]]
name = "attach"
request = "attach"
completion = []
args = {}

[[language.debugger.templates]]
name = "attach-uri"
request = "attach"
completion = [{ name = "VM Service URI", completion = "text" }]
args = { vmServiceUri = "{0}" }
```

**Usage:**

1. Open a Dart file in Helix.
2. Run `<space>Gl` (dap launch) and select the `attach` template.
3. Helix starts `fdemon --dap-stdio` as a subprocess and connects.

> **Requirement:** `fdemon` must be on your `PATH`.

### Option C: TCP with Port Argument (Helix-managed port)

If you want Helix to pick the port and pass it to fdemon:

```toml
[language.debugger]
name = "fdemon-dap"
transport = "tcp"
command = "fdemon"
args = ["--dap-port"]
port-arg = "{}"

[[language.debugger.templates]]
name = "attach"
request = "attach"
completion = []
args = {}
```

Helix picks a free port, calls `fdemon --dap-port <PORT>`, then connects.

---

## Neovim (nvim-dap)

Install [nvim-dap](https://github.com/mfussenegger/nvim-dap) and add the
following to your Neovim configuration (e.g.,
`~/.config/nvim/lua/dap-config.lua`):

```lua
local dap = require('dap')

-- Option A: Stdio — nvim-dap launches fdemon as a subprocess
dap.adapters.fdemon = {
  type = 'executable',
  command = 'fdemon',
  args = { '--dap-stdio' },
}

-- Option B: TCP — connect to an already-running fdemon instance
dap.adapters.fdemon_tcp = {
  type = 'server',
  host = '127.0.0.1',
  port = 4711,
}

-- Debug configurations for Dart/Flutter files
dap.configurations.dart = {
  {
    type = 'fdemon',       -- use 'fdemon_tcp' for TCP mode
    request = 'attach',
    name = 'Flutter Demon (attach)',
  },
}
```

**Usage:**

1. Open a Dart file.
2. Set a breakpoint with `:lua require('dap').toggle_breakpoint()`.
3. Start debugging with `:lua require('dap').continue()`.

---

## VS Code

VS Code users typically use the official Dart extension for Flutter debugging.
If you need to connect VS Code to Flutter Demon's DAP server (for example, to
use fdemon's TUI alongside VS Code's debug UI), add a launch configuration to
`.vscode/launch.json`:

```json
{
  "version": "0.2.0",
  "configurations": [
    {
      "name": "Flutter Demon (TCP)",
      "type": "node",
      "request": "attach",
      "debugServer": 4711
    }
  ]
}
```

> VS Code's built-in `debugServer` field connects to a running DAP TCP server.
> Adjust the port to match what fdemon is listening on.

---

## Troubleshooting

### Zed: TCP connection times out

Increase the `debugger.timeout` in Zed's `settings.json` (see Option A above).
Default is often 5 000 ms; try 10 000 ms for slow startup scenarios.

### Zed / Helix / nvim: "fdemon: command not found"

Ensure `fdemon` is on your `PATH`. If you installed with `cargo install`:

```sh
export PATH="$HOME/.cargo/bin:$PATH"
```

Add this to your shell profile (`.bashrc`, `.zshrc`, etc.) to make it
permanent.

### Helix: adapter exits immediately

Helix stdio transport requires `fdemon` to stay alive waiting for DAP messages.
Make sure you are using `--dap-stdio` (not `--headless` or a bare `fdemon`).

### Port already in use

If fdemon reports the DAP port is already in use, either:

- Stop the conflicting process, or
- Use a different port: `fdemon --dap-port 0` lets the OS pick a free port.
  The assigned port is shown in the status bar `[DAP :PORT]` and printed to
  stderr.

### All IDEs: verify fdemon is running with DAP active

- **TUI mode:** The status bar shows `[DAP :PORT]` when the DAP server is
  active.
- **Headless mode:** fdemon prints a JSON event:
  `{"event":"dap_server_started","port":4711,...}`.
- **Stdio mode:** The IDE manages the lifecycle; check the IDE's debug console
  for adapter errors.

---

## Implemented DAP Capabilities

Flutter Demon's DAP adapter currently supports:

| Capability | Status |
|---|---|
| Initialize / attach | Supported |
| Set breakpoints | Supported |
| Set exception breakpoints | Supported |
| Continue / pause | Supported |
| Step over / in / out | Supported |
| Stack traces | Supported |
| Scopes and variables | Supported |
| Variable expansion (objects, lists) | Supported |
| Evaluate expression | Supported |
| Output events (stdout, stderr) | Supported |
| Configuration done | Supported |
| Disconnect | Supported |
| Launch request | Not supported (attach only) |
| Restart | Not supported |
| Hot reload via DAP | Not supported (use `r` in fdemon TUI) |

All configurations should use `"request": "attach"` — fdemon attaches to an
already-running Flutter process rather than launching one itself.
