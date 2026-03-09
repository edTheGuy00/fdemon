# IDE Setup Guide for Flutter Demon DAP Server

This guide explains how to connect your IDE to Flutter Demon's Debug Adapter
Protocol (DAP) server for Flutter debugging.

Flutter Demon exposes a DAP interface so that editors with DAP support can set
breakpoints, step through code, inspect variables, and evaluate expressions
while `fdemon` manages the Flutter process.

## Transport Modes

Flutter Demon supports two DAP transport modes. **TCP is the recommended mode
for real debugging.** Stdio mode is available for protocol testing and IDE
integration validation.

### TCP Mode — Recommended

TCP mode connects your IDE to a running `fdemon` TUI session that manages the
Flutter process. This is the production-ready path for debugging.

1. Run `fdemon` in your Flutter project directory.
2. Press `D` to start the DAP server (or pass `--dap-port <PORT>` at startup).
3. Note the port shown in the status bar: `[DAP :4711]`.
4. Connect your IDE to `127.0.0.1:<port>`.

### Stdio Mode — Protocol Testing Only

> **Important Limitation**: Stdio mode (`--dap-stdio`) is a transport-only
> implementation for protocol validation and IDE integration testing. It does
> **not** start a Flutter Engine or Flutter process, and does **not** route
> `attach` commands to the Dart VM Service. Real debugging (breakpoints,
> stepping, variables) requires **TCP mode** with a running `fdemon` TUI
> session.

When you need stdio transport for IDE integration testing:

1. Configure your IDE to launch `fdemon --dap-stdio` as an adapter subprocess.
2. The IDE manages the adapter lifecycle automatically — no manual `fdemon`
   instance is needed.
3. DAP protocol messages (initialize, configurationDone, disconnect) are
   processed correctly. `attach` and debug commands return errors because no
   VM Service backend is connected.

All non-DAP output (tracing, logs) goes to stderr in stdio mode.

---

## Automatic IDE Configuration

When fdemon's DAP server starts (press `D` or pass `--dap-port`), it
auto-detects whether it is running inside an IDE's integrated terminal and
generates the appropriate debug configuration file. No manual config is needed
in most cases.

### Detection Table

| IDE | Detected Via | Config File Generated | Merge Strategy |
|-----|-------------|-----------------------|----------------|
| VS Code / Cursor | `$TERM_PROGRAM`, `$VSCODE_IPC_HOOK_CLI` | `.vscode/launch.json` | Merge by `"name"` field; `"fdemon-managed": true` marker |
| Zed | `$ZED_TERM` | `.zed/debug.json` | Merge by `"label"` field |
| Neovim | `$NVIM` | `.vscode/launch.json` + `.nvim-dap.lua` | VS Code merge + Lua snippet overwrite |
| Helix | `$HELIX_RUNTIME` | `.helix/languages.toml` | TOML merge: replaces `[language.debugger]` in dart entry |
| Emacs | `$INSIDE_EMACS` | `.fdemon/dap-emacs.el` | Always overwritten (fdemon-owned) |
| IntelliJ / Android Studio | `$TERMINAL_EMULATOR` | None | Auto-config not supported; use manual setup |

> **Helix note:** The auto-generated `.helix/languages.toml` uses `port-arg` so
> Helix spawns a new fdemon instance to pick the port. This differs from TCP
> attach to an already-running fdemon — use Option A (`:debug-remote`) when you
> want to connect to an existing TUI session.

> **IntelliJ / Android Studio note:** These IDEs are detected via
> `$TERMINAL_EMULATOR` but `supports_dap_config()` returns `false`. Auto-config
> is not generated; follow the manual setup path for your IDE.

### Merge Safety

fdemon reads existing config files and merges its entry without clobbering other
configurations. If the generated content is identical to what is already in the
file, the file is not touched (mtime preserved). This prevents editor
file-watcher noise.

### Status Bar

After config generation, the DAP badge in the status bar shows which IDE was
configured, for example: `[DAP :4711 · VS Code]`.

### CLI Standalone Mode

```bash
# Generate config and exit (useful for CI/scripts)
fdemon --dap-config vscode --dap-port 4711

# Override IDE detection in combined mode
fdemon --dap-config zed
```

### Disabling Auto-Configuration

```toml
# .fdemon/config.toml
[dap]
auto_configure_ide = false
```

Or toggle in the Settings panel: `,` → Project → DAP Server → Auto-Configure IDE.

---

## Zed IDE

> **Automatic setup:** If you run fdemon from Zed's integrated terminal,
> `.zed/debug.json` is generated automatically when you press `D`. The
> instructions below are for manual setup or troubleshooting.

Zed's Dart/Flutter debugging is not built-in (as of early 2026). Flutter Demon
fills this gap.

### Option A: TCP — Recommended (Connect to a Running fdemon)

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

To trace the raw DAP wire protocol for troubleshooting, enable:

```json
{
  "debugger": {
    "log_dap_communications": true
  }
}
```

DAP messages will appear in Zed's log panel (`View → Debug → DAP Log`).

#### Zed DAP Client Behaviour

- Zed's play/pause button state depends on receiving `stopped` and `continued`
  DAP events — fdemon emits both correctly.
- The Zed debug UI does not yet support all DAP capabilities (e.g., logpoints
  may not have a dedicated UI). Use Neovim or VS Code for full feature coverage.
- Hot reload and hot restart must be triggered from fdemon's TUI (`r` / `R`),
  or via custom DAP requests if your IDE supports them.

### Option B: Stdio — Protocol Testing Only (Zed Launches fdemon)

> **Limitation**: This option is for protocol validation and IDE integration
> testing only. Real debugging requires Option A (TCP). See the
> [Transport Modes](#transport-modes) section for details.

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
    "label": "Flutter Demon (stdio — protocol testing only)",
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

### Option C: Zed Extension (Future)

Phase 5 delivered automatic config file generation (see
[Automatic IDE Configuration](#automatic-ide-configuration) above). A full Zed
WASM extension that registers the `FlutterDemon` adapter in the `DapRegistry`,
provides `get_dap_binary`, and auto-detects Flutter projects via `pubspec.yaml`
remains future work beyond Phase 5. Until then, use Option A (TCP) or rely on
the auto-generated `.zed/debug.json`.

---

## Helix

> **Automatic setup:** If you run fdemon from a Helix terminal session
> (`$HELIX_RUNTIME` detected), `.helix/languages.toml` is generated
> automatically when you press `D`. The instructions below are for manual setup
> or troubleshooting.

Helix marks DAP support as experimental. Known limitations:

- Variable expansion shows a flat popup (no tree view).
- Hover values are not supported.
- The `:debug-remote` command connects over TCP; stdio requires a
  `languages.toml` entry.

### Option A: TCP — Recommended (Connect to a Running fdemon)

Start `fdemon` in your Flutter project, press `D` to activate the DAP server,
then connect from Helix with:

```
:debug-remote 127.0.0.1:4711
```

No configuration file changes are needed for TCP mode.

### Option B: TCP with Port Argument (Helix-managed port)

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

### Option C: Stdio — Protocol Testing Only (Helix Launches fdemon)

> **Limitation**: This option is for protocol validation and IDE integration
> testing only. Real debugging requires Option A or B (TCP). See the
> [Transport Modes](#transport-modes) section for details.

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
name = "attach (protocol testing only)"
request = "attach"
completion = []
args = {}
```

**Usage:**

1. Open a Dart file in Helix.
2. Run `<space>Gl` (dap launch) and select the `attach (protocol testing only)` template.
3. Helix starts `fdemon --dap-stdio` as a subprocess and connects.

Note: DAP handshake messages (initialize, configurationDone, disconnect) will
succeed. The `attach` command will return an error because no VM Service backend
is wired up in stdio mode.

> **Requirement:** `fdemon` must be on your `PATH`.

---

## Neovim (nvim-dap)

> **Automatic setup:** If you run fdemon from Neovim's integrated terminal
> (`$NVIM` detected), `.vscode/launch.json` and `.nvim-dap.lua` are generated
> automatically when you press `D`. The instructions below are for manual setup
> or troubleshooting.

Install [nvim-dap](https://github.com/mfussenegger/nvim-dap) and add the
following to your Neovim configuration (e.g.,
`~/.config/nvim/lua/dap-config.lua`):

```lua
local dap = require('dap')

-- Recommended: TCP — connect to an already-running fdemon instance
dap.adapters.fdemon_tcp = {
  type = 'server',
  host = '127.0.0.1',
  port = 4711,
}

-- Protocol testing only: Stdio — nvim-dap launches fdemon as a subprocess.
-- NOTE: Real debugging (breakpoints, stepping) is NOT supported in stdio mode.
-- Use fdemon_tcp above for actual debugging workflows.
dap.adapters.fdemon = {
  type = 'executable',
  command = 'fdemon',
  args = { '--dap-stdio' },
}

-- Debug configurations for Dart/Flutter files
dap.configurations.dart = {
  {
    type = 'fdemon_tcp',   -- recommended: TCP mode for real debugging
    request = 'attach',
    name = 'Flutter Demon (TCP)',
  },
  {
    type = 'fdemon',       -- protocol testing only
    request = 'attach',
    name = 'Flutter Demon stdio (protocol testing only)',
  },
}
```

**Usage (TCP mode — recommended):**

1. Run `fdemon` in your Flutter project and press `D` to start the DAP server.
2. Open a Dart file in Neovim.
3. Set a breakpoint with `:lua require('dap').toggle_breakpoint()`.
4. Start debugging with `:lua require('dap').continue()` and select
   `Flutter Demon (TCP)`.

---

## VS Code

> **Automatic setup:** If you run fdemon from VS Code's integrated terminal
> (`$TERM_PROGRAM` or `$VSCODE_IPC_HOOK_CLI` detected), `.vscode/launch.json`
> is generated automatically when you press `D`. The instructions below are for
> manual setup or troubleshooting.

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

## Emacs (dap-mode)

> **Automatic setup:** If you run fdemon from an Emacs terminal session
> (`$INSIDE_EMACS` detected), `.fdemon/dap-emacs.el` is generated automatically
> when you press `D`. This file is always overwritten by fdemon (it is
> fdemon-owned). The instructions below show how to load the generated file or
> configure dap-mode manually.

When running fdemon from an Emacs terminal, fdemon auto-generates
`.fdemon/dap-emacs.el` containing `dap-register-debug-provider` and
`dap-register-debug-template` forms ready to connect to fdemon's DAP TCP server.

### Loading the Auto-Generated Config

Add the following to your Emacs init file (e.g., `~/.emacs.d/init.el` or your
`use-package` block for dap-mode):

```emacs-lisp
(load-file (expand-file-name ".fdemon/dap-emacs.el"
                             (project-root (project-current))))
```

This loads the generated provider and template each time you open the project.
Because fdemon regenerates the file on every DAP server start, this always
reflects the current port.

### Manual Configuration

If you prefer to configure dap-mode directly without the auto-generated file,
add:

```emacs-lisp
(require 'dap-mode)

(dap-register-debug-provider
 "fdemon"
 (lambda (conf)
   (plist-put conf :host "127.0.0.1")
   (plist-put conf :port 4711)
   conf))

(dap-register-debug-template
 "Flutter Demon (TCP)"
 (list :type "fdemon"
       :request "attach"
       :name "Flutter Demon (TCP)"))
```

Adjust the port to match what fdemon reports in the status bar (`[DAP :PORT]`).

**Usage:**

1. Run `fdemon` in your Flutter project and press `D` to start the DAP server.
2. In Emacs, run `M-x dap-debug` and select `Flutter Demon (TCP)`.
3. dap-mode connects over TCP to fdemon's DAP server.

> **Requirement:** `dap-mode` must be installed. See the
> [dap-mode README](https://github.com/emacs-lsp/dap-mode) for installation
> instructions.

---

## Phase 4 Debugging Features

The following features are implemented and available when connecting over TCP
mode to a running fdemon session.

### Debug Event Flow

fdemon correctly emits `stopped`, `continued`, and `thread` DAP events when the
Dart VM pauses, resumes, or creates/destroys isolates. IDEs that drive their
play/pause button state from these events (such as Zed) will reflect the correct
debugger state.

### Hot Reload and Hot Restart via DAP

The DAP server exposes two custom requests that trigger fdemon's existing reload
and restart lifecycle:

| Custom Request | Effect |
|---|---|
| `hotReload` | Triggers a Flutter hot reload (same as pressing `r` in the TUI) |
| `hotRestart` | Triggers a Flutter hot restart (same as pressing `R` in the TUI) |

These go through the TEA pipeline — reload suppression, phase tracking, and
EngineEvent broadcasting all work as normal. IDEs can send them via the DAP
`customRequest` mechanism. For example in VS Code (with a Dart extension that
supports custom requests):

```json
{ "command": "hotReload" }
```

### Auto-Reload Suppression While Paused

When the debugger pauses an isolate (breakpoint, exception, step), fdemon
automatically suspends file-watcher triggered auto-reloads. This prevents hot
reload from invalidating the paused stack frame mid-inspection.

- File changes that arrive during a pause are queued.
- When the debugger resumes, fdemon flushes the queue and performs a single
  reload if any files changed.
- If the DAP client disconnects while paused, the watcher is automatically
  re-enabled.

This behaviour is controlled by `settings.dap.suppress_reload_on_pause`
(default: `true`) in `.fdemon/config.toml`.

### Conditional Breakpoints

fdemon supports the full DAP conditional breakpoint model:

| Breakpoint Property | Behaviour |
|---|---|
| `condition` | A Dart expression; the breakpoint fires only when the expression evaluates to truthy |
| `hitCondition` | A hit-count expression (e.g., `">= 3"`, `"% 2 == 0"`); checked before `condition` |

Both properties are set from your IDE's breakpoint UI. No special configuration
is required on the fdemon side.

### Logpoints

A breakpoint with a `logMessage` is treated as a *logpoint*. When it triggers:

1. All applicable conditions are evaluated (hit condition, then expression
   condition).
2. `{expression}` placeholders in the message template are evaluated via the
   Dart VM's `evaluateInFrame` RPC.
3. The interpolated message is emitted as a DAP `output` event in the IDE's
   debug console.
4. Execution is **not** paused — the isolate auto-resumes.

Example log message template: `"x = {x}, counter = {counter.value}"`

### Expression Evaluation

The `evaluate` DAP request supports all standard contexts:

| Context | Behaviour |
|---|---|
| `hover` | Evaluates an expression hovered in the editor. Long strings are truncated to 100 chars; no expandable references. |
| `watch` | Full evaluation with expandable object references for the watch panel. |
| `variables` | Sub-expression evaluation from the variable tree; same as `watch`. |
| `repl` | Debug console evaluation; full output, side effects allowed. |
| `clipboard` | Full representation with no truncation; suitable for copy-to-clipboard. |

Evaluation requires the isolate to be paused. If a `frameId` is provided, the
expression is evaluated in that stack frame's scope. Without a `frameId`, it
evaluates in the root library context.

### Source References (SDK and Package Sources)

When stepping into Dart SDK code (`dart:core`, `dart:async`, etc.) or into
package sources that are not present on the local filesystem, fdemon assigns a
`sourceReference` integer to the source. The IDE can then request the source
text via the `source` DAP request, and fdemon fetches it from the Dart VM.

This allows you to step into and read SDK source code directly in your editor
without any extra setup. Source references persist across pause/resume
transitions but are invalidated on hot restart.

### Custom DAP Events

fdemon emits the following custom events after a successful `attach`:

| Event | When | Body |
|---|---|---|
| `dart.debuggerUris` | Immediately after attach | `{ "vmServiceUri": "ws://127.0.0.1:PORT/..." }` |
| `flutter.appStart` | Immediately after attach | `{ "deviceId": "...", "mode": "debug", "supportsRestart": true }` |
| `flutter.appStarted` | When the VM signals the app is fully started | `{}` |

IDEs can consume `dart.debuggerUris` to connect supplementary tooling (such as
Dart DevTools) to the same VM Service connection that fdemon is using.

### Multi-Session Debugging

When multiple Flutter sessions are running simultaneously (up to 9), fdemon
namespaces DAP thread IDs so isolates from different sessions cannot collide:

| Session Index | Thread ID Range |
|---|---|
| 0 | 1000–1999 |
| 1 | 2000–2999 |
| … | … |
| 8 | 9000–9999 |

The IDE sees a flat list of threads. The session that owns each thread can be
determined from `thread_id / 1000 - 1`. All standard DAP requests
(`stackTrace`, `scopes`, `variables`, `evaluate`) are routed to the correct
session based on the thread ID.

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

### Breakpoints not hitting after hot restart

Hot restart creates a new Dart isolate with new internal IDs. Re-set your
breakpoints in the IDE after a hot restart to ensure they are registered against
the new isolate. (Automatic breakpoint re-application across hot restart is
planned for a future release.)

### IDE shows "Debugger paused" but fdemon's TUI shows "Running"

This can happen if an isolate paused at startup (PauseStart). Press the
"Continue" button in your IDE to let the isolate proceed past the initial pause.
fdemon's phase reflects the Flutter app phase, not the debugger state, so the
TUI can show "Running" while an isolate is paused for debugging.

### Auto-reload not triggering after saving files

If you are actively debugging (isolate paused at a breakpoint), auto-reload is
suppressed to protect the paused stack frame. Resume or disconnect the debugger
to re-enable file-watcher triggered reloads.

### Zed: play/pause button stuck

This typically means Zed did not receive a `stopped` or `continued` DAP event.
Enable `debugger.log_dap_communications` in Zed's `settings.json` to inspect
the raw message stream and confirm fdemon is sending the events.

### Stale `.zed/debug.json` from Phase 3

If you have an existing `.zed/debug.json` with `"adapter": "custom"`, replace
`"custom"` with `"Delve"`. The `"custom"` adapter name is not valid in Zed and
will launch CodeLLDB instead of connecting to fdemon.

---

## Implemented DAP Capabilities

Flutter Demon's DAP adapter supports the following capabilities in TCP mode:

| Capability | TCP Mode |
|---|---|
| Initialize | Supported |
| Attach | Supported |
| Set breakpoints | Supported |
| Conditional breakpoints (`condition`, `hitCondition`) | Supported |
| Logpoints (`logMessage` with `{expression}`) | Supported |
| Set exception breakpoints | Supported |
| Continue / pause | Supported |
| Step over / in / out | Supported |
| Stack traces | Supported |
| Scopes and variables | Supported |
| Variable expansion (objects, lists) | Supported |
| Evaluate expression (hover, watch, repl, clipboard) | Supported |
| Source references (SDK / unresolvable package sources) | Supported |
| Output events (stdout, stderr) | Supported |
| Custom request: `hotReload` | Supported |
| Custom request: `hotRestart` | Supported |
| Custom event: `dart.debuggerUris` | Supported |
| Custom event: `flutter.appStart` | Supported |
| Custom event: `flutter.appStarted` | Supported |
| Multi-session thread ID namespacing | Supported |
| Auto-reload suppression while paused | Supported |
| Configuration done | Supported |
| Disconnect | Supported |
| Launch request | Not supported (attach only) |
| Breakpoint persistence across hot restart | Planned |

All configurations should use `"request": "attach"` — fdemon attaches to an
already-running Flutter process rather than launching one itself.
