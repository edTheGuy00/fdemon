# Plan: Native DAP Server

## TL;DR

Build a Debug Adapter Protocol (DAP) server directly into Flutter Demon, enabling any IDE (VS Code, IntelliJ, Neovim, Helix) to attach a debugger to a running fdemon session. The server translates DAP requests into Dart VM Service RPCs via the existing `VmRequestHandle`, coordinates hot reload/restart with debugger state, and exposes multi-session debugging through the standard DAP thread model. This avoids the architectural conflicts of spawning `dart debug_adapter` or `flutter debug-adapter` as subprocesses (dual VM Service connections, lifecycle races, pause/resume conflicts).

---

## Background

### The Problem

Flutter Demon manages Flutter processes, hot reload, log viewing, and multi-device sessions. But when developers need to set breakpoints, inspect variables, or step through code, they must switch to their IDE's built-in debugger — which launches a _separate_ Flutter process outside fdemon's control. This means:

- Two Flutter processes competing for the same device
- No coordination between hot reload and debug state
- Lost context when switching between fdemon and IDE debugging
- No way to debug multi-session setups that fdemon uniquely enables

### Why Build It Natively (Option 2)

The Dart SDK ships `dart debug_adapter` and Flutter ships `flutter debug-adapter`. Both are designed to **own** the Flutter process lifecycle:

- `flutter debug-adapter` always spawns `flutter run --machine` or `flutter attach --machine` as a subprocess — directly conflicting with fdemon's existing process management
- `dart debug_adapter` in attach mode connects directly to the VM Service, creating **two clients** (fdemon + adapter) on the same WebSocket — causing state coherence problems with pause/resume, hot restart breakpoint persistence, and reconnection races
- Neither adapter can coordinate hot reload suppression during stepping, or expose fdemon's multi-session model

Building the DAP server natively means:
- **One process, one VM Service connection, one source of truth**
- Hot reload/restart coordinated with breakpoint state (suppress file watcher during stepping, re-apply breakpoints after hot restart)
- Multi-session debugging through DAP's thread model
- Log/profiling data forwarded via DAP output events

### Existing Infrastructure

fdemon already has the hard parts:

| Capability | Status | Location |
|---|---|---|
| VM Service WebSocket client | Production (375 tests) | `fdemon-daemon/src/vm_service/client.rs` |
| Clonable async RPC handle | Done | `VmRequestHandle` (Clone, Send, thread-safe) |
| Raw JSON-RPC `request()` | Done | Can call any VM Service RPC today |
| Stream subscription | Done | Extension, Logging, GC streams |
| Multi-session management | Done (up to 9) | `SessionManager` + `SessionHandle` |
| Hot reload/restart coordination | Done | Daemon protocol + isolate cache invalidation |
| Extension API for integrations | Done | `Engine::subscribe()`, `EnginePlugin`, service traits |
| Headless runner pattern | Done | `src/headless/runner.rs` — template for DAP runner |

**What's missing:**
- VM Service debugging RPCs (breakpoints, stepping, stack, variables, expression eval)
- Debug/Isolate stream subscription and event parsing
- DAP protocol types and wire format (Content-Length framed JSON)
- TCP listener for DAP clients
- Per-session debug state tracking

### DAP Protocol Overview

The Debug Adapter Protocol is a JSON-based protocol with a `Content-Length` header, communicating over TCP or stdio. Key request/response pairs:

| DAP Request | VM Service RPC(s) |
|---|---|
| `initialize` | Capability negotiation (no RPC) |
| `attach` | `streamListen("Debug")`, `streamListen("Isolate")`, `setExceptionPauseMode` |
| `setBreakpoints` | `addBreakpointWithScriptUri` / `removeBreakpoint` |
| `continue` | `resume(isolateId)` |
| `next` / `stepIn` / `stepOut` | `resume(isolateId, step: Over/Into/Out)` |
| `pause` | `pause(isolateId)` |
| `threads` | `getVM()` → map isolates to threads |
| `stackTrace` | `getStack(isolateId)` |
| `scopes` / `variables` | `getObject(isolateId, objectId)` |
| `evaluate` | `evaluateInFrame(isolateId, frameIndex, expression)` |
| `disconnect` | Cleanup, optionally `resume` paused isolates |

Custom DAP requests (Flutter-specific):
- `hotReload` → `FlutterController::reload()`
- `hotRestart` → `FlutterController::restart()`

Custom DAP events (IDE-consumed):
- `dart.debuggerUris` → VM Service URI for DevTools
- `flutter.appStarted` → session startup complete

### References

- [DAP Specification](https://microsoft.github.io/debug-adapter-protocol/specification)
- [Dart VM Service Protocol](https://github.com/dart-lang/sdk/blob/main/runtime/vm/service/service.md)
- [Flutter debug-adapter README](https://github.com/flutter/flutter/blob/master/packages/flutter_tools/lib/src/debug_adapters/README.md)
- [Dart-Code DAP integration](https://github.com/Dart-Code/Dart-Code)
- [`dapts` crate (Rust DAP types)](https://lib.rs/crates/dapts)

---

## Affected Modules

### Modified Modules

- `crates/fdemon-daemon/src/vm_service/client.rs` — Subscribe to `Debug` + `Isolate` streams on connect
- `crates/fdemon-daemon/src/vm_service/extensions/mod.rs` — Re-export new debugger module
- `crates/fdemon-daemon/src/vm_service/protocol.rs` — Add debug event types (`PauseBreakpoint`, `PauseException`, etc.)
- `crates/fdemon-daemon/Cargo.toml` — No new deps needed (raw `request()` suffices)
- `crates/fdemon-app/src/config/types.rs` — Add `DapSettings` struct, `ParentIde::Emacs`/`Helix` variants, `supports_dap_config()` method
- `crates/fdemon-app/src/message.rs` — Add DAP Message variants (`StartDapServer`, `StopDapServer`, `ToggleDap`, `DapServerStarted`, etc.)
- `crates/fdemon-app/src/handler/mod.rs` — Add DAP UpdateAction variants (`SpawnDapServer`, `StopDapServer`)
- `crates/fdemon-app/src/handler/keys.rs` — Add `D` keybinding in Normal mode for `Message::ToggleDap`
- `crates/fdemon-app/src/settings_items.rs` — Add DAP settings section (`dap.enabled`, `dap.auto_start_in_ide`, `dap.port`)
- `crates/fdemon-app/src/handler/settings.rs` — Add `apply_project_setting` cases for `dap.*` keys
- `crates/fdemon-app/src/session/handle.rs` — Add DAP shutdown/task fields to `SessionHandle`
- `crates/fdemon-app/src/engine_event.rs` — Wire up unimplemented `EngineEvent` variants needed by DAP
- `crates/fdemon-app/Cargo.toml` — Depend on `fdemon-dap` (or feature-gated)
- `crates/fdemon-tui/src/widgets/log_view/mod.rs` — Add `dap_status` field to `StatusInfo`, render `[DAP :PORT]` badge
- `crates/fdemon-tui/src/widgets/header.rs` — Add `[D] DAP` to keybinding hints
- `Cargo.toml` (workspace) — Add `fdemon-dap` member, binary deps
- `src/main.rs` — Add `--dap-port` / `--dap-config` CLI flags
- `src/tui/runner.rs` — Handle `UpdateAction::SpawnDapServer` / `StopDapServer` in TUI event loop
- `src/headless/runner.rs` — Handle DAP startup in headless mode event loop

### New Modules

- `crates/fdemon-dap/` — **NEW CRATE**: DAP protocol, server, adapter
  - `src/lib.rs` — Public API
  - `src/protocol/mod.rs` — DAP message types (using `dapts` crate or hand-rolled subset)
  - `src/protocol/codec.rs` — Content-Length framed JSON encoder/decoder
  - `src/protocol/types.rs` — DAP request/response/event type aliases
  - `src/server/mod.rs` — TCP listener, client session management
  - `src/server/session.rs` — Per-DAP-client state machine
  - `src/adapter/mod.rs` — DAP request → VM Service RPC translation
  - `src/adapter/breakpoints.rs` — Breakpoint management (set, resolve, persist across restarts)
  - `src/adapter/threads.rs` — Isolate-to-thread mapping
  - `src/adapter/stack.rs` — Stack frames, scopes, variables
  - `src/adapter/evaluate.rs` — Expression evaluation
  - `src/adapter/sources.rs` — Source reference resolution
- `crates/fdemon-daemon/src/vm_service/debugger.rs` — **NEW**: VM Service debugging RPC wrappers
- `crates/fdemon-daemon/src/vm_service/debugger_types.rs` — **NEW**: Typed structs for VM Service debug protocol
- `crates/fdemon-app/src/handler/dap.rs` — **NEW**: DAP message handler in TEA (StartDapServer, StopDapServer, ToggleDap, DapServerStarted, etc.)
- `crates/fdemon-app/src/session/debug_state.rs` — **NEW**: Per-session debug state
- `crates/fdemon-dap/src/ide_config/mod.rs` — **NEW**: IDE DAP config auto-generation dispatch
- `crates/fdemon-dap/src/ide_config/vscode.rs` — **NEW**: VS Code `.vscode/launch.json` generator
- `crates/fdemon-dap/src/ide_config/neovim.rs` — **NEW**: Neovim nvim-dap config generator
- `crates/fdemon-dap/src/ide_config/helix.rs` — **NEW**: Helix `.helix/languages.toml` generator
- `crates/fdemon-dap/src/ide_config/zed.rs` — **NEW**: Zed `.zed/debug.json` generator
- `crates/fdemon-dap/src/ide_config/emacs.rs` — **NEW**: Emacs dap-mode snippet generator

---

## Architecture

### Crate Dependency Graph (with fdemon-dap)

```
┌─────────────────────────────────────────────────────┐
│           flutter-demon (binary crate)               │
│       CLI + TUI runner + headless runner             │
└──────────────┬──────────────────────────────────────┘
               │
       ┌───────┼────────┬──────────────┐
       ▼       ▼        ▼              ▼
┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐
│fdemon-tui│ │fdemon-dap│ │fdemon-app│ │          │
│Terminal  │ │DAP Server│ │Engine,TEA│ │          │
└────┬─────┘ └────┬─────┘ └────┬─────┘ │          │
     │            │             │       │          │
     │            │      ┌──────┴───┐   │          │
     │            │      ▼          ▼   │          │
     │            │ ┌──────────┐ ┌──────────┐      │
     │            └─│fdemon-   │ │fdemon-   │      │
     │              │daemon    │ │core      │      │
     └──────────────│Flutter IO│ │Domain    │──────┘
                    └──────────┘ └──────────┘
```

`fdemon-dap` depends on:
- `fdemon-core` — Domain types (`SessionId`, `LogEntry`, `AppPhase`)
- `fdemon-daemon` — `VmRequestHandle` for debugging RPCs
- `fdemon-app` — `Engine`, `EngineEvent`, `Message`, service traits
- `tokio` — Async TCP server
- `serde` / `serde_json` — DAP JSON serialization

### Data Flow

```
IDE (VS Code / IntelliJ / Neovim)
  │
  │  DAP over TCP (Content-Length framed JSON)
  ▼
┌──────────────────────────────────────────────────┐
│  fdemon-dap: DapServer (TcpListener)             │
│    └── DapClientSession (per-connection)          │
│         ├── receives DAP requests                 │
│         ├── sends DAP responses + events          │
│         └── holds DapAdapter                      │
│              ├── VmRequestHandle (clone)           │
│              │   └── debugging RPCs → VM Service   │
│              ├── msg_tx: Sender<Message>           │
│              │   └── hot reload/restart → Engine   │
│              └── event_rx: Receiver<EngineEvent>   │
│                  └── log/phase events → DAP events │
└──────────────────────────────────────────────────┘
                    │           │
         ┌──────────┘           └──────────┐
         ▼                                 ▼
  ┌──────────────┐                ┌──────────────┐
  │  VM Service  │                │    Engine     │
  │  (WebSocket) │                │  (TEA loop)   │
  │  breakpoints │                │  hot reload   │
  │  stepping    │                │  session mgmt │
  │  variables   │                │  log pipeline  │
  └──────────────┘                └──────────────┘
```

### Thread/Task Model

```
Main thread (Engine event loop)
  ├── TUI rendering (if TUI mode) or headless (if headless mode)
  ├── processes Message::Dap* variants from DAP server
  └── emits EngineEvent to all subscribers

DAP server task (tokio::spawn — started/stopped at runtime)
  ├── TcpListener on configured port
  ├── shutdown_rx: watch::Receiver<bool> for clean stop
  └── per-client tasks (tokio::spawn per connection)
       ├── reads DAP requests from TCP stream
       ├── translates to VM Service RPCs via VmRequestHandle
       ├── translates Flutter commands to Message via msg_tx
       ├── listens to EngineEvent broadcast for state changes
       └── sends DAP events/responses to TCP stream

VM Service background task (existing)
  ├── WebSocket I/O loop
  ├── forwards Debug/Isolate stream events → Message pipeline
  └── routes RPC responses to waiting oneshot channels
```

### DAP Server Startup Flow (Smart Auto-Start)

The DAP server is an **integrated service** within TUI mode (and headless mode), not a separate runner. It can be started/stopped at runtime like other Engine services.

```
fdemon launch
  │
  ├── Parse CLI args: --dap-port (if provided, sets dap.enabled = true)
  ├── Load settings: dap.enabled, dap.auto_start_in_ide
  ├── detect_parent_ide() → Option<ParentIde>
  │
  ├── Should DAP auto-start?
  │   ├── dap.enabled = true in config (or --dap-port)?  → YES, start
  │   ├── IDE detected AND dap.auto_start_in_ide?        → YES, start
  │   └── None of the above?                             → NO, stay off
  │
  ├── If starting:
  │   ├── Bind TCP port (configured or auto-assign)
  │   ├── Emit Message::DapServerStarted { port }
  │   ├── Generate IDE config (Phase 5) if IDE detected
  │   └── Log: "DAP server listening on 127.0.0.1:{port}"
  │
  └── At runtime (keybinding 'D' in Normal mode):
      ├── If DAP off → Message::StartDapServer → bind + start + config gen
      └── If DAP on  → Message::StopDapServer  → disconnect clients + unbind
```

**Key design decision**: The DAP server is NOT a separate mode — it's an integrated service within TUI or headless mode. There is no `--dap` CLI flag. Instead, the server starts automatically when an IDE terminal is detected (zero-config experience), or can be toggled at runtime with `D`. For CI/scripting, `--dap-port PORT` forces a specific port and implies DAP enabled. This means the TUI keeps running — the developer sees their logs while the IDE connects for debugging.

**On fdemon exit**: Generated IDE config entries are **left in place** (not cleaned up). The next fdemon run updates the port in the config. This avoids surprising the user by removing their IDE setup.

---

## Development Phases

### Phase 1: VM Service Debugging Foundation

**Goal**: Extend the VM Service client with all debugging RPCs and stream event parsing needed by DAP. This is pure infrastructure with no user-facing changes — everything is internal and fully testable.

#### Steps

1. **Add debug event types** (`debugger_types.rs`)
   - `Breakpoint { id, resolved, location: SourceLocation }`
   - `SourceLocation { script: ScriptRef, line, column, token_pos }`
   - `ScriptRef { id, uri }`
   - `Frame { index, kind, code, location, vars: Vec<BoundVariable> }`
   - `BoundVariable { name, value: InstanceRef }`
   - `InstanceRef { id, kind, class_ref, value_as_string, length }`
   - `Stack { frames: Vec<Frame>, messages: Vec<Message>, truncated }`
   - `ScriptList { scripts: Vec<ScriptRef> }`
   - `PauseEvent { kind, breakpoint, exception, top_frame }`
   - `StepOption` enum: `Into`, `Over`, `Out`, `OverAsyncSuspension`
   - `ExceptionPauseMode` enum: `None`, `Unhandled`, `All`

2. **Add debugging RPC wrappers** (`debugger.rs`)
   - All wrappers call `handle.request(method, params)` — no new transport code needed
   - `pause(isolate_id) -> Result<()>`
   - `resume(isolate_id, step: Option<StepOption>) -> Result<()>`
   - `add_breakpoint_with_script_uri(isolate_id, script_uri, line, column?) -> Result<Breakpoint>`
   - `remove_breakpoint(isolate_id, breakpoint_id) -> Result<()>`
   - `get_stack(isolate_id, limit?) -> Result<Stack>`
   - `get_object(isolate_id, object_id) -> Result<Value>` (returns raw JSON — objects are highly polymorphic)
   - `evaluate(isolate_id, target_id, expression) -> Result<InstanceRef>`
   - `evaluate_in_frame(isolate_id, frame_index, expression) -> Result<InstanceRef>`
   - `set_exception_pause_mode(isolate_id, mode: ExceptionPauseMode) -> Result<()>`
   - `get_scripts(isolate_id) -> Result<ScriptList>`
   - `get_source_report(isolate_id, script_id, reports: Vec<String>) -> Result<Value>`

3. **Subscribe to Debug + Isolate streams**
   - Update `RESUBSCRIBE_STREAMS` constant in `client.rs` to include `"Debug"` and `"Isolate"`
   - Add `VmServiceEvent` parsing for Debug stream events: `PauseBreakpoint`, `PauseException`, `PauseExit`, `PauseStart`, `PausePostRequest`, `Resume`, `BreakpointAdded`, `BreakpointRemoved`, `BreakpointResolved`
   - Add parsing for Isolate stream events: `IsolateStart`, `IsolateRunnable`, `IsolateExit`, `IsolateReload`

4. **Add per-session debug state** (`session/debug_state.rs`)
   - `DebugState { paused: bool, pause_reason: Option<PauseReason>, breakpoints: HashMap<String, Vec<TrackedBreakpoint>>, exception_mode: ExceptionPauseMode }`
   - `TrackedBreakpoint { dap_id: i64, vm_id: String, uri: String, line: i32, verified: bool }`
   - `PauseReason` enum: `Breakpoint`, `Exception`, `Step`, `Pause`, `Entry`
   - Methods: `track_breakpoint()`, `untrack_breakpoint()`, `mark_paused()`, `mark_resumed()`, `breakpoints_for_uri()`

**Milestone**: All VM Service debugging RPCs callable via `VmRequestHandle`, Debug/Isolate stream events parsed and forwarded through the Message pipeline. 100% unit test coverage on new code.

---

### Phase 2: DAP Protocol & Server Infrastructure

**Goal**: Implement the DAP wire protocol, TCP server, and client session management. A DAP client can connect, complete initialization handshake, and receive capability negotiation — but no debugging features yet.

#### Steps

1. **Create `fdemon-dap` crate**
   - Workspace member in root `Cargo.toml`
   - Dependencies: `fdemon-core`, `fdemon-daemon`, `fdemon-app`, `tokio`, `serde`, `serde_json`, `tracing`
   - Evaluate `dapts` crate (v0.0.6) for DAP type definitions — if stable enough, use it; otherwise hand-roll the subset needed (initialize, attach, disconnect, setBreakpoints, continue, next, stepIn, stepOut, pause, threads, stackTrace, scopes, variables, evaluate, output, stopped, terminated, custom requests)

2. **DAP protocol codec** (`protocol/codec.rs`)
   - `DapCodec` implementing tokio's `Encoder`/`Decoder` traits (or manual framing)
   - Parse `Content-Length: N\r\n\r\n<JSON>` frames from TCP stream
   - Serialize responses with `Content-Length` header
   - Handle partial reads, malformed frames, oversized messages

3. **DAP message types** (`protocol/types.rs`)
   - `DapMessage` enum: `Request`, `Response`, `Event`
   - `DapRequest { seq, command, arguments }`
   - `DapResponse { request_seq, success, command, body, message }`
   - `DapEvent { event, body }`
   - Capability structs for `InitializeResponse`

4. **TCP server** (`server/mod.rs`)
   - `DapServer::new(port, engine_handle)` — binds `TcpListener`
   - Accepts connections, spawns per-client `DapClientSession`
   - Configurable bind address (default `127.0.0.1` — localhost only for security)
   - Graceful shutdown via `watch::Receiver<bool>` from Engine

5. **Client session state machine** (`server/session.rs`)
   - States: `Uninitialized` → `Initializing` → `Configured` → `Attached` → `Debugging` → `Disconnecting`
   - `initialize` → negotiate capabilities, respond with supported features
   - `configurationDone` → transition to ready state
   - `disconnect` → cleanup, optionally resume paused isolates
   - Request/response tracking (match `seq` numbers)

6. **DAP service integration** (Engine-level, not a separate runner)
   - `DapService` struct in `fdemon-dap` — manages server lifecycle (start/stop/status)
   - `DapService::start(port, bind_addr, msg_tx, event_rx) -> Result<DapHandle>`
   - `DapHandle { port: u16, shutdown_tx: watch::Sender<bool>, task: JoinHandle }` — returned to Engine for lifecycle management
   - `DapService::stop(handle) -> Result<()>` — signals shutdown, waits for task completion, disconnects all clients
   - Engine holds `Option<DapHandle>` — `Some` when DAP is running, `None` when off
   - CLI: `fdemon [--dap-port PORT] [project_path]`
   - `--dap-port PORT` sets a fixed port and forces `dap.enabled = true` (for CI/scripting)
   - No `--dap` flag — auto-start via IDE detection is the primary path; `D` keybinding for runtime toggle
   - Both TUI and headless runners evaluate `should_auto_start_dap()` and call `DapService::start()` if needed

7. **Smart auto-start logic** (startup + runtime toggle)
   - **Startup**: On Engine init, evaluate startup conditions (see Architecture → DAP Server Startup Flow)
   - **Message variants**:
     - `Message::StartDapServer` — keybinding or auto-start trigger
     - `Message::StopDapServer` — keybinding or shutdown cleanup
     - `Message::DapServerStarted { port: u16 }` — response after successful bind
     - `Message::DapServerStopped` — response after shutdown
     - `Message::DapClientConnected { client_id: String }` — per-connection tracking
     - `Message::DapClientDisconnected { client_id: String }` — per-connection tracking
   - **Keybinding**: `D` (uppercase) in Normal mode → `Message::ToggleDap`
     - If DAP off: transitions to `StartDapServer`, auto-picks port, generates IDE config
     - If DAP on: transitions to `StopDapServer`, disconnects clients
   - **UpdateAction variants**:
     - `UpdateAction::SpawnDapServer { port, bind_addr }` — handled by TUI/headless runner event loops
     - `UpdateAction::StopDapServer` — triggers graceful shutdown
   - **Status tracking**: Add to `AppState`:
     - `dap_status: DapStatus` where `DapStatus` is `Off`, `Starting`, `Running { port: u16, client_count: usize }`, `Stopping`
   - **Cleanup**: On Engine shutdown (`engine.shutdown()`), stop DAP server if running. On session close, notify connected DAP clients

8. **Status bar integration**
   - Add `dap_status` field to TUI's `StatusInfo` struct
   - When `DapStatus::Running { port, .. }`: render `[DAP :4711]` badge next to existing `[VM]` badge
   - When `DapStatus::Running { client_count > 0, .. }`: render `[DAP :4711 ●]` with a connected indicator
   - When `DapStatus::Off`: no badge shown (default state)
   - In header keybinding hints: add `[D] DAP` when in Normal mode

**Milestone**: Running `fdemon` inside VS Code auto-starts the DAP server with zero configuration. Pressing `D` toggles the server on/off at runtime. Status bar shows `[DAP :PORT]` when active. VS Code (or `dap-client` test tool) can connect, complete initialization, and receive capabilities response. `--dap-port PORT` available for CI/scripting. No debugging yet, but the transport and UX work end-to-end.

---

### Phase 3: Core Debugging Features

**Goal**: Full debugging support — breakpoints, stepping, stack traces, variables, threads, exception handling. An IDE can attach to a running Flutter session and debug with the standard feature set.

#### Steps

1. **Adapter layer** (`adapter/mod.rs`)
   - `DapAdapter` struct holding `VmRequestHandle`, `msg_tx: Sender<Message>`, `event_rx: Receiver<EngineEvent>`, per-session `DebugState`
   - Request dispatch: match on `command` string, delegate to specialized handlers
   - Response construction: wrap VM Service results into DAP response format

2. **Thread management** (`adapter/threads.rs`)
   - `attach` request → connect to session's VM Service via `ws_uri`
   - Map isolates to DAP thread IDs (monotonic integer, keyed by isolate ID string)
   - `threads` request → `getVM()`, filter non-system isolates, return as DAP Thread objects
   - Track thread lifecycle: `IsolateStart` → add, `IsolateExit` → remove, emit `thread` events

3. **Breakpoint management** (`adapter/breakpoints.rs`)
   - `setBreakpoints` request → per-file breakpoint diff:
     - Remove breakpoints no longer in the request (`removeBreakpoint`)
     - Add new breakpoints (`addBreakpointWithScriptUri`)
     - Track mapping: DAP breakpoint ID ↔ VM Service breakpoint ID
   - Handle `BreakpointResolved` events → update `verified` status, send `breakpoint` event to IDE
   - `setExceptionBreakpoints` → `setExceptionPauseMode` on all isolates

4. **Execution control** (`adapter/mod.rs`)
   - `continue` → `resume(isolateId)`, emit `continued` event
   - `next` → `resume(isolateId, step: Over)`
   - `stepIn` → `resume(isolateId, step: Into)`
   - `stepOut` → `resume(isolateId, step: Out)`
   - `pause` → `pause(isolateId)`, wait for `PauseStart` event
   - Handle `PauseBreakpoint` / `PauseException` / `PauseExit` → emit `stopped` event with reason

5. **Stack traces** (`adapter/stack.rs`)
   - `stackTrace` → `getStack(isolateId, limit)` → map VM `Frame` objects to DAP `StackFrame`
   - Include source location: `Source { name, path, sourceReference }`
   - Handle async suspension markers (virtual frames)
   - Assign monotonic frame IDs per request

6. **Variables and scopes** (`adapter/stack.rs`)
   - `scopes` → derive from frame: "Locals" scope + "Globals" scope
   - `variables` → `getObject(isolateId, objectId)` → expand instance fields, list elements, map entries
   - Handle primitive types (int, double, string, bool, null) inline
   - Handle collection types (List, Map, Set) with lazy expansion
   - Assign monotonic `variablesReference` IDs, maintain lookup table per stopped state

7. **Output events** (via `EngineEvent` subscription)
   - `EngineEvent::LogEntry` / `EngineEvent::LogBatch` → DAP `output` events
   - Category mapping: `LogLevel::Error` → `"stderr"`, others → `"stdout"`
   - Include source location if available (from log entry metadata)

**Milestone**: Full breakpoint-based debugging works. Developer can set breakpoints in VS Code, hit them, inspect variables, step through code, see log output in debug console. Tested with VS Code Dart extension and Neovim nvim-dap.

---

### Phase 4: Flutter Integration & Polish

**Goal**: Flutter-specific debugging features that differentiate fdemon from generic DAP adapters. Tight coordination between hot reload and debug state. Multi-session support. Production hardening.

#### Steps

1. **Hot reload/restart via custom DAP requests**
   - `hotReload` custom request → `FlutterController::reload()` via `msg_tx`
   - `hotRestart` custom request → `FlutterController::restart()` via `msg_tx`
   - Send `ReloadCompleted` / `RestartCompleted` as custom DAP events
   - Handle `hotRestart` breakpoint persistence:
     - On restart trigger: save breakpoint list from `DebugState`
     - On new isolate runnable: re-apply all breakpoints via `addBreakpointWithScriptUri`
     - Invalidate variable references (old object IDs no longer valid)

2. **Coordinated pause during stepping**
   - When debugger pauses at a breakpoint/step:
     - Suppress file watcher auto-reload (send `Message::SuspendFileWatcher`)
     - Queue file change events instead of triggering reload
   - When debugger resumes:
     - Re-enable file watcher (send `Message::ResumeFileWatcher`)
     - If files changed while paused, trigger hot reload after resume

3. **Custom DAP events for IDE integration**
   - `dart.debuggerUris` event → `{ vmServiceUri: "ws://..." }` sent on attach
   - `flutter.appStarted` event → sent when session phase reaches `Running`
   - `flutter.appStart` event → device/mode metadata on session creation
   - `dart.serviceExtensionAdded` → when Flutter extensions register

4. **Multi-session thread grouping**
   - Thread IDs namespaced per session: session 0 → threads 1000-1999, session 1 → 2000-2999
   - `threads` request returns isolates from all active sessions with session name prefix
   - Breakpoints apply to all sessions (same codebase)
   - Stepping/pause operates on the specific thread (isolate) the IDE selects

5. **Expression evaluation** (`adapter/evaluate.rs`)
   - `evaluate` request with `frameId` → `evaluateInFrame(isolateId, frameIndex, expression)`
   - `evaluate` request without `frameId` → `evaluate(isolateId, targetId, expression)` on root library
   - Handle evaluation errors gracefully (compile errors, runtime exceptions)
   - Support `context: "hover"` for tooltip evaluation (auto-toString)
   - Support `context: "repl"` for debug console evaluation

6. **Conditional breakpoints & logpoints**
   - Conditional: set breakpoint, but only emit `stopped` if condition evaluates to truthy
   - Logpoints: set breakpoint, evaluate expression, emit `output` event, auto-resume
   - Both require `evaluateInFrame` at the pause point before deciding to stop or continue

7. **Source references** (`adapter/sources.rs`)
   - SDK sources (dart:core, etc.) → fetch via `getObject(scriptId)` → return as `Source` with `sourceReference`
   - Package sources → resolve from `.dart_tool/package_config.json` paths
   - `source` request → return source text for a `sourceReference` ID

8. **Production hardening**
   - Connection timeout handling
   - Graceful degradation when VM Service disconnects mid-debug
   - Rate limiting on variable expansion (prevent IDE from fetching entire object graph)
   - Comprehensive error responses for malformed requests
   - `disconnect` with `terminateDebuggee: false` (default) vs `true`

**Milestone**: Full Flutter debugging experience. Hot reload from IDE preserves breakpoints. Debug console evaluates expressions. Multi-session debugging works. Tested against VS Code Dart extension, Neovim nvim-dap, and Helix DAP.

---

### Phase 5: IDE DAP Auto-Configuration

**Goal**: Automatically detect which IDE the terminal is running inside and generate the appropriate DAP client configuration so the user can start debugging with zero manual setup. Leverages the existing `ParentIde` detection from the hyperlinks/editor feature (`fdemon-app/src/config/settings.rs`).

#### Background

fdemon already detects the parent IDE via environment variables in `detect_parent_ide()` (`crates/fdemon-app/src/config/settings.rs:73-123`). The existing `ParentIde` enum covers 7 IDEs: VS Code, VS Code Insiders, Cursor, Zed, IntelliJ, Android Studio, and Neovim. Each IDE has a distinct mechanism for connecting a DAP client to an external TCP server:

| IDE | Config File | DAP TCP Mechanism | Auto-Gen Feasibility |
|-----|------------|-------------------|---------------------|
| VS Code / Insiders / Cursor | `.vscode/launch.json` | `debugServer: PORT` field | High — fully auto-generatable |
| Neovim (nvim-dap) | `.vscode/launch.json` (via `load_launchjs()`) | `type = "server"` adapter | Medium — piggybacks on VS Code format |
| Helix | `.helix/languages.toml` | `transport = "tcp"` | High — project-local config supported |
| Zed | `.zed/debug.json` | `tcp_connection: {host, port}` | High — project-local config supported |
| Emacs (dap-mode) | Project-local snippet | `:debugPort` / `:host` plist | Low — requires user to source snippet |
| IntelliJ / Android Studio | N/A | No standard DAP path | Not supported — uses proprietary debugging |

#### Steps

1. **IDE config generation trait** (`ide_config/mod.rs`)
   - `IdeConfigGenerator` trait with methods:
     - `config_path(project_root: &Path) -> PathBuf` — where to write the config
     - `generate(port: u16, project_root: &Path) -> Result<String>` — generate config content
     - `config_exists(project_root: &Path) -> bool` — check if config already exists
     - `merge_config(existing: &str, port: u16) -> Result<String>` — merge into existing config without clobbering
   - `generate_ide_config(ide: Option<ParentIde>, port: u16, project_root: &Path) -> Result<IdeConfigResult>`
   - `IdeConfigResult { path: PathBuf, action: ConfigAction }` where `ConfigAction` is `Created`, `Updated`, `Skipped(reason)`
   - Dispatch to IDE-specific generators based on `ParentIde` variant

2. **VS Code config generator** (`ide_config/vscode.rs`)
   - Generates/merges into `.vscode/launch.json`
   - Uses `debugServer` field to redirect DAP transport to fdemon's TCP port:
     ```json
     {
       "name": "Flutter (fdemon)",
       "type": "dart",
       "request": "attach",
       "debugServer": 4711
     }
     ```
   - `debugServer` is a VS Code internal mechanism that tells VS Code to connect to an already-running DAP server on the given port instead of spawning a debug adapter process. The Dart extension must be installed (it provides `"type": "dart"`)
   - **Merge logic**: If `.vscode/launch.json` exists, parse it, check for existing fdemon configuration (match by `"name"` field), update the port if found, append if not. Preserve all other configurations and the `version` field
   - Handles VS Code, VS Code Insiders, and Cursor (all use the same `.vscode/launch.json` format)
   - Include `"cwd": "${workspaceFolder}"` for correct path resolution

3. **Neovim config generator** (`ide_config/neovim.rs`)
   - **Primary strategy**: Generate `.vscode/launch.json` (same as VS Code generator) because nvim-dap supports loading VS Code launch configs via `require("dap.ext.vscode").load_launchjs()`
   - **Secondary strategy**: Also generate a project-local `.nvim-dap.lua` snippet that users can source:
     ```lua
     -- fdemon DAP configuration (auto-generated)
     -- Source this file or add to your nvim-dap config:
     --   require("dap.ext.vscode").load_launchjs()
     local dap = require('dap')
     dap.adapters.fdemon = {
       type = 'server',
       host = '127.0.0.1',
       port = 4711,
     }
     dap.configurations.dart = dap.configurations.dart or {}
     table.insert(dap.configurations.dart, {
       type = 'fdemon',
       request = 'attach',
       name = 'Flutter (fdemon)',
       cwd = vim.fn.getcwd(),
     })
     ```
   - The `.nvim-dap.lua` file is informational — the `.vscode/launch.json` is the primary auto-config path since nvim-dap's `load_launchjs()` is widely used
   - Detect if `$NVIM` socket is available for status feedback via `nvim --server $NVIM --remote-send`

4. **Helix config generator** (`ide_config/helix.rs`)
   - Generates `.helix/languages.toml` in the project root (Helix merges project-local with user config)
   - Helix's TCP transport spawns the adapter and picks the port via `port-arg`. For fdemon (already-running server), the config points to the fdemon binary with `--dap-port`:
     ```toml
     [[language]]
     name = "dart"

     [language.debugger]
     name = "fdemon-dap"
     transport = "tcp"
     command = "fdemon"
     args = []
     port-arg = "--dap-port {}"

     [[language.debugger.templates]]
     name = "Flutter: Attach (fdemon)"
     request = "attach"
     completion = []
     args = {}
     ```
   - **Alternative for already-running fdemon**: Generate a helper script that simply connects to the fixed port (since Helix always wants to spawn + pick port). Document this limitation
   - **Merge logic**: If `.helix/languages.toml` exists, parse TOML, find or create the `[[language]]` entry for `name = "dart"`, update the debugger section. Preserve all other language configurations

5. **Zed config generator** (`ide_config/zed.rs`)
   - Generates `.zed/debug.json` in the project root:
     ```json
     [
       {
         "label": "Flutter (fdemon DAP)",
         "adapter": "fdemon-dap",
         "request": "attach",
         "tcp_connection": {
           "host": "127.0.0.1",
           "port": 4711
         },
         "cwd": "$ZED_WORKTREE_ROOT"
       }
     ]
     ```
   - `tcp_connection` tells Zed to connect to an existing TCP server rather than spawning a new adapter process
   - **Caveat**: Dart/Flutter are not natively supported by Zed's debugger as of March 2026. A community Zed debugger extension for Dart would need to exist for this config to work. Generate the config anyway (forward-compatible) but log a warning if Dart support is not detected
   - **Merge logic**: If `.zed/debug.json` exists, parse JSON array, find existing fdemon entry by `"label"` field, update port if found, append if not

6. **Emacs config generator** (`ide_config/emacs.rs`)
   - Cannot auto-write to user's Emacs config — Emacs does not support project-local DAP configuration in a standard way
   - Generate a `.dir-locals.el` snippet file at `.fdemon/dap-emacs.el`:
     ```elisp
     ;; fdemon DAP configuration for Emacs dap-mode
     ;; Add this to your Emacs config or eval-buffer this file:
     ;;
     ;; (load-file "/path/to/project/.fdemon/dap-emacs.el")

     (require 'dap-mode)

     (dap-register-debug-provider
       "fdemon"
       (lambda (conf)
         (plist-put conf :debugPort 4711)
         (plist-put conf :host "localhost")
         conf))

     (dap-register-debug-template
       "Flutter :: fdemon"
       (list :type "fdemon"
             :request "attach"
             :name "Flutter (fdemon DAP)"))
     ```
   - This is a "generate and instruct" approach — fdemon generates the snippet and prints instructions on how to load it
   - Emacs detection: Not currently in `ParentIde` enum. Add `ParentIde::Emacs` variant with detection via `$INSIDE_EMACS` environment variable (set by Emacs shell/vterm/eshell)

7. **Auto-generation trigger and lifecycle**
   - When DAP server starts (handles `Message::DapServerStarted { port }`), call `generate_ide_config(detect_parent_ide(), port, project_root)` if `dap.auto_configure_ide` is true
   - Print result to tracing log: `"Generated DAP config for {ide} at {path}"` or `"Skipped DAP config: {reason}"`
   - Add `Message::DapConfigGenerated { path, action }` variant for TUI status display
   - On port change (server toggled off/on with different port), regenerate config with updated port
   - On fdemon exit: **leave generated config in place** (not cleaned up). Next run updates the port if needed
   - **When no IDE detected**: Skip auto-generation, log info message suggesting manual config. Print the port number and a generic connection instruction
   - **Manual generation**: `fdemon --dap-config <ide> --dap-port <port>` generates config for a specific IDE without starting the full TUI

8. **Extend `ParentIde` enum**
   - Add `ParentIde::Emacs` variant:
     - Detection: `$INSIDE_EMACS` environment variable (set by Emacs `shell-mode`, `vterm`, `eshell`, `term-mode`)
     - `display_name()` → `"Emacs"`
     - `url_scheme()` → `"file"` (Emacs doesn't have a URL scheme for file opening)
     - `reuse_flag()` → `None`
   - Add `ParentIde::Helix` variant:
     - Detection: `$HELIX_RUNTIME` environment variable (set when running inside Helix's `:sh` command)
     - `display_name()` → `"Helix"`
     - `url_scheme()` → `"file"`
     - `reuse_flag()` → `None`
   - Add methods to `ParentIde`:
     - `supports_dap_config(&self) -> bool` — returns `true` for IDEs where auto-generation is meaningful (all except IntelliJ/AndroidStudio)
     - `dap_config_path(&self, project_root: &Path) -> Option<PathBuf>` — returns the target config file path

9. **Safe file writing with merge semantics**
   - **Critical**: Never overwrite existing IDE config files. Always merge
   - For JSON files (`.vscode/launch.json`, `.zed/debug.json`): parse existing, find/update fdemon entries, serialize back preserving formatting and comments where possible
   - For TOML files (`.helix/languages.toml`): parse existing, find/update dart language section, serialize back
   - For Elisp files (`.fdemon/dap-emacs.el`): always overwrite (fdemon-owned file)
   - Use a marker comment/field (e.g., `"fdemon-managed": true` in JSON configs) to identify auto-generated entries for safe updates and cleanup
   - Create parent directories if they don't exist (e.g., `.vscode/`, `.helix/`, `.zed/`)
   - Respect `.gitignore` — add a note in generated files that they can be committed or gitignored

10. **TUI integration**
    - When DAP config is generated, show a brief status message: `"DAP config generated for {IDE} at {path}"`
    - Add a DAP status indicator to the status bar (Phase 2 already adds the server status; extend with config status)
    - In settings UI, add a `[dap] auto_configure_ide` boolean option (default: `true`)

**Milestone**: When a developer runs `fdemon` from inside VS Code's terminal, the DAP server auto-starts and fdemon auto-generates a `launch.json` configuration. The developer opens the Run & Debug panel, selects "Flutter (fdemon)", and is immediately connected to the debugger — zero manual configuration required. Same for Neovim, Helix, and Zed (with their respective config formats).

---

## Edge Cases & Risks

### Protocol Compatibility

- **Risk:** VS Code Dart extension sends non-standard custom requests that we don't support
- **Mitigation:** Phase 4 implements all documented custom requests (`hotReload`, `hotRestart`, `callService`). Return `ErrorResponse` for unknown custom requests with clear message. Test against multiple IDEs.

### Dual VM Service Subscription

- **Risk:** Adding `Debug` and `Isolate` stream subscriptions may conflict with existing `Extension` stream handling or increase message volume
- **Mitigation:** The VM Service supports multiple simultaneous stream subscriptions. Debug events are only emitted when isolates are paused/resumed, which is low-frequency. Parse and route events through dedicated handlers, not the existing log pipeline.

### Hot Restart Breakpoint Loss

- **Risk:** Hot restart creates a new isolate — all VM Service breakpoint IDs become invalid
- **Mitigation:** Phase 4 explicitly handles this: save DAP breakpoint state, detect new isolate via `IsolateRunnable` event, re-apply all breakpoints, emit updated `breakpoint` events with new `verified` status.

### Multi-Client Conflicts

- **Risk:** Two IDEs connect simultaneously and send conflicting commands (one pauses, other resumes)
- **Mitigation:** Phase 2 server supports multiple connections but routes debugging commands per-session. Each DAP client session is independent. Document that only one debugger should control a given Flutter session at a time. Optionally: single-client mode per session as a configuration option.

### Performance Under Variable Expansion

- **Risk:** IDE requests variables for large objects (e.g., a 10,000-element list), causing thousands of `getObject` RPCs
- **Mitigation:** Implement pagination via DAP's `start`/`count` parameters on `variables` requests. Limit default expansion depth. Use `variablesReference` for lazy evaluation.

### Debug Stream Events vs Existing Event Pipeline

- **Risk:** `PauseBreakpoint` events from Debug stream need to reach the DAP adapter without being dropped or delayed by the Engine's 256-capacity message channel
- **Mitigation:** The DAP adapter holds its own `VmRequestHandle` and can listen to the VM Service event stream directly (separate from the Engine's forwarding loop). This bypasses the Engine channel for latency-critical debug events while still using the Engine for coordination messages.

### Isolate Identification

- **Risk:** Dart VM Service isolate IDs are strings like `"isolates/1234567890"`. After hot restart, new isolate gets a different ID. Cached references become stale.
- **Mitigation:** fdemon already handles this via `VmRequestHandle::invalidate_isolate_cache()`. The DAP adapter must invalidate its own thread-ID-to-isolate mapping on hot restart and re-discover isolates.

### IDE Config Overwrite (Phase 5)

- **Risk:** Auto-generating IDE config files could overwrite user's existing debug configurations
- **Mitigation:** Always merge, never overwrite. Parse existing config files, find fdemon-managed entries by marker field (`"fdemon-managed": true`), update only those entries. If no fdemon entry exists, append. If the file can't be parsed (malformed JSON/TOML), skip generation and log a warning rather than clobbering the file.

### Stale Port in Generated Config (Phase 5)

- **Risk:** fdemon generates config with port 54321, user restarts fdemon on port 54322 — IDE still tries the old port
- **Mitigation:** Regenerate config on every DAP server startup, updating the port in the existing config entry. For stable ports across restarts, recommend users set a fixed `dap.port` in config or `--dap-port` on CLI. The auto-generated config always reflects the current run's port.

### Helix Port-Arg Incompatibility (Phase 5)

- **Risk:** Helix's `transport = "tcp"` always spawns the adapter binary and passes a port via `port-arg`. This conflicts with fdemon's model where the DAP server is already running as part of the fdemon process
- **Mitigation:** The Helix config uses `fdemon` as the command with `--dap-port {}` as the port arg. Helix spawns a *new* fdemon instance that listens on the Helix-chosen port. This is acceptable for single-session use. For the already-running fdemon scenario, document that users should use `hx --health` to verify DAP support and manually set the port. Alternative: provide a thin wrapper script that connects stdin/stdout to an existing TCP socket.

### IDE Not Detected (Phase 5)

- **Risk:** fdemon is run from a terminal that isn't inside any IDE (plain terminal, tmux, etc.) — no config to generate
- **Mitigation:** This is a normal case, not an error. Skip auto-generation silently (debug-level log only). Print the DAP port to stdout/logs so the user can manually configure their IDE. Include a `fdemon --dap-config <ide>` CLI flag for manual config generation targeting a specific IDE.

### Emacs/Helix Detection Gaps (Phase 5)

- **Risk:** `$INSIDE_EMACS` is not set in all Emacs terminal modes (e.g., some custom shell setups). `$HELIX_RUNTIME` may not be set in Helix's `:sh` command in all versions
- **Mitigation:** These are best-effort detections. Document the expected environment variables. Users can always fall back to `fdemon --dap-config emacs` or `fdemon --dap-config helix` for manual generation.

---

## Configuration Additions

```toml
# .fdemon/config.toml

[dap]
# Always enable DAP server on startup (overrides auto-detection)
# Can also use --dap-port CLI flag for a fixed port
enabled = false

# Auto-start DAP server when running inside a detected IDE terminal
# (VS Code, Neovim, Helix, Zed, Emacs). No effect if enabled = true.
# Default: true — zero-setup experience for IDE users
auto_start_in_ide = true

# TCP port for DAP connections (default: 0 = auto-assign)
# Use a fixed port for stable IDE configs across restarts
port = 0

# Bind address (default: 127.0.0.1 for security)
bind_address = "127.0.0.1"

# Suppress auto-reload while debugger is paused at breakpoint
suppress_reload_on_pause = true

# Auto-attach debugger when session starts (vs waiting for IDE to attach)
auto_attach = false

# Automatically generate IDE DAP config when server starts (Phase 5)
auto_configure_ide = true
```

---

## CLI Additions

```
fdemon [--dap-port PORT] [--dap-config IDE] [project_path]

Options:
  --dap-port PORT    Start DAP server on a fixed port (implies DAP enabled).
                     Use a fixed port for stable IDE configs across restarts.
                     Without this flag, DAP auto-starts when an IDE terminal is
                     detected, using an auto-assigned port.
  --dap-config IDE   Generate DAP config for a specific IDE without auto-detection
                     Values: vscode, neovim, helix, zed, emacs
                     Can be used standalone: fdemon --dap-config vscode --dap-port 4711
```

The auto-assigned port is logged and emitted via `Message::DapServerStarted { port }`. In headless mode, it is also printed to stdout as JSON:
```json
{"dapPort": 54321}
```

**How DAP starts (no `--dap` flag needed):**
- `fdemon` inside VS Code/Neovim/etc. — DAP auto-starts (IDE detected, zero config)
- `fdemon` in plain terminal — DAP stays off; press `D` to toggle on
- `fdemon --dap-port 4711` — DAP on fixed port (for CI/scripting)
- `fdemon --headless --dap-port 4711` — Headless mode with DAP on fixed port
- `dap.enabled = true` in `.fdemon/config.toml` — DAP always on regardless of IDE

---

## Success Criteria

### Phase 1 Complete When:
- [ ] All VM Service debugging RPCs (pause, resume, breakpoints, stack, variables, evaluate) are implemented and unit tested
- [ ] Debug and Isolate stream events are parsed with typed structs
- [ ] `DebugState` tracks per-session breakpoints and pause status
- [ ] 100+ new unit tests for debugging RPCs and event parsing
- [ ] `cargo test -p fdemon-daemon` passes
- [ ] `cargo clippy --workspace` clean

### Phase 2 Complete When:
- [ ] `fdemon-dap` crate compiles and passes tests
- [ ] DAP protocol codec handles Content-Length framing correctly (including edge cases: partial reads, zero-length, oversized)
- [ ] TCP server accepts connections and completes DAP initialization handshake
- [ ] Smart auto-start works: running inside VS Code terminal auto-starts DAP (zero config, `auto_start_in_ide = true` by default)
- [ ] `--dap-port PORT` CLI flag starts DAP server on a fixed port
- [ ] `D` keybinding toggles DAP server on/off in Normal mode
- [ ] Status bar shows `[DAP :PORT]` badge when server is running
- [ ] `DapSettings` struct with `enabled`, `auto_start_in_ide`, `port`, `bind_address` fields in config
- [ ] DAP settings appear in settings panel (`,` → DAP section)
- [ ] Tested with VS Code DAP client (can connect and see capabilities)
- [ ] `cargo test -p fdemon-dap` passes
- [ ] `cargo clippy --workspace` clean

### Phase 3 Complete When:
- [ ] Breakpoints can be set, hit, and removed from VS Code
- [ ] Stepping (next, stepIn, stepOut) works correctly
- [ ] Stack trace shows correct frames with source locations
- [ ] Variables can be inspected (primitives, objects, collections)
- [ ] Exception breakpoints work (uncaught, all)
- [ ] Log output appears in debug console
- [ ] Threads list shows Flutter isolates
- [ ] Tested with VS Code Dart extension AND Neovim nvim-dap
- [ ] 200+ new unit tests
- [ ] `cargo test --workspace` passes

### Phase 4 Complete When:
- [x] Hot reload/restart work from debug toolbar (Task 02)
- [ ] Breakpoints persist across hot restart (Task 10 — planned)
- [x] Auto-reload suppressed while paused (Task 03)
- [x] Expression evaluation works in debug console and hover (Task 06)
- [x] Conditional breakpoints and logpoints work (Tasks 04, 05)
- [x] SDK/package sources viewable in IDE (Task 07)
- [x] Multi-session debugging works (multiple Flutter sessions) (Task 09)
- [x] Custom DAP events received by IDE (dart.debuggerUris, flutter.appStarted) (Task 08)
- [ ] Tested against VS Code, Neovim, and Helix
- [ ] Performance tested: variable expansion doesn't hang on large objects (Task 11 — planned)
- [x] Documentation updated (Task 12)

### Phase 5 Complete When:
- [ ] `ParentIde` enum extended with `Emacs` and `Helix` variants (with env var detection)
- [ ] `ParentIde::supports_dap_config()` and `dap_config_path()` methods implemented
- [ ] VS Code config generator creates valid `launch.json` with `debugServer` field
- [ ] VS Code config generator merges into existing `launch.json` without clobbering other configs
- [ ] Neovim config generator produces `.vscode/launch.json` + `.nvim-dap.lua` snippet
- [ ] Helix config generator produces valid `.helix/languages.toml` with `transport = "tcp"`
- [ ] Zed config generator produces valid `.zed/debug.json` with `tcp_connection`
- [ ] Emacs config generator produces `.fdemon/dap-emacs.el` with `dap-register-debug-provider`
- [ ] Auto-generation triggers on DAP server bind, skips gracefully when no IDE detected
- [ ] `--dap-config <ide>` CLI flag works for manual generation
- [ ] Config merge logic handles malformed files without data loss (skip + warn)
- [ ] 50+ unit tests covering generation, merging, and edge cases
- [ ] `cargo test -p fdemon-dap` passes
- [ ] `cargo clippy --workspace` clean
- [ ] Tested end-to-end: run `fdemon` inside VS Code terminal → DAP auto-starts → launch.json generated → Debug panel shows config → connection succeeds

---

## Future Enhancements

- **Stdio transport**: Support DAP over stdin/stdout (in addition to TCP) for editors that prefer spawning the adapter as a subprocess
- **Launch mode**: Support `launch` request (not just `attach`) — fdemon spawns the Flutter process when the IDE starts debugging
- **Data breakpoints**: Break on field value changes (requires VM Service `addBreakpointOnActivation` or equivalent)
- **Memory inspection**: Expose fdemon's memory profiling data through DAP's `readMemory` request
- **Inline values**: Support DAP's `inlineValues` capability for showing variable values inline in the editor
- **Dedicated IDE extensions**: Publish VS Code extension and Neovim plugin that auto-discover fdemon's DAP port and provide richer integration (Phase 5 handles config generation; dedicated extensions would add live port discovery, status bar indicators, and custom DAP views)
- **Zed Dart debugger extension**: Write a Zed extension implementing `get_dap_binary` for Dart/Flutter so `.zed/debug.json` auto-config actually works (currently Dart is not in Zed's supported debugger languages)
- **Remote debugging**: Support non-localhost bind addresses for remote Flutter device debugging
- **Profiling integration**: Expose fdemon's performance data through DAP events for integrated perf views
- **IntelliJ DAP via LSP4IJ**: If the LSP4IJ plugin gains wider adoption, add `.idea/runConfigurations/` XML generation for IntelliJ/Android Studio DAP support
