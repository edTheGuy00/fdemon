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
| `scopes` / `variables` | `getObject(isolateId, objectId)`, `evaluate` (for getters/toString) |
| `evaluate` | `evaluateInFrame(isolateId, frameIndex, expression)` |
| `exceptionInfo` | Read stored exception `InstanceRef` + `toString()` |
| `restartFrame` | `resume(isolateId, step: kRewind, frameIndex: N)` |
| `loadedSources` | `getScripts(isolateId)` → list all loaded Dart scripts |
| `breakpointLocations` | `getSourceReport(isolateId, scriptId, [PossibleBreakpoints])` |
| `completions` | Scope variable names + library names (no VM RPC needed) |
| `restart` | Hot restart — `FlutterController::restart()` + re-apply breakpoints |
| `disconnect` | Cleanup, optionally `resume` paused isolates |

Custom DAP requests (Flutter-specific):
- `hotReload` → `FlutterController::reload()`
- `hotRestart` → `FlutterController::restart()`
- `callService` → `vmService.callMethod(method, params)` (forward arbitrary VM Service RPCs)
- `updateDebugOptions` → `setLibraryDebuggable()` per library (toggle SDK/package debugging)

Custom DAP events (IDE-consumed):
- `dart.debuggerUris` → VM Service URI for DevTools
- `flutter.appStarted` → session startup complete
- `dart.hotReloadComplete` / `dart.hotRestartComplete` → operation completion notification
- `dart.serviceExtensionAdded` → VM service extension registered on isolate
- Progress events (`progressStart`/`progressEnd`) → hot reload/restart progress

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

6. **Variables and scopes** (`adapter/variables.rs`, `adapter/stack.rs`)

   **6a. Scope model — three scopes per frame:**

   | Scope | Source | `expensive` | When present |
   |---|---|---|---|
   | Locals | `frame.vars` (`BoundVariable[]`) from `getStack` | `false` | Always |
   | Globals | Library static fields via `getObject(libraryId)` on the frame's script library | `true` | Always |
   | Exceptions | The `exception` `InstanceRef` from the `PauseException` event | `false` | Only when paused at exception (`PauseException` / `PauseExit`) |

   - Locals scope: Extract `BoundVariable[]` from the stack frame at the given `frameIndex`. Each `BoundVariable` has `{ name, value: InstanceRef }`. Convert each to a DAP `Variable`.
   - Globals scope: Obtain the frame's `code.owner` (a `LibraryRef`), call `getObject(isolateId, libraryId)` to get the full `Library` object, then read its `variables` field (list of `FieldRef`). For each field, call `getObject(isolateId, fieldId)` to get the `Field` object with its `staticValue: InstanceRef`. Convert to DAP `Variable`.
   - Exceptions scope: When `PauseException` event fires, store the `exception: InstanceRef` on the thread state. Create an "Exceptions" scope with a single variable representing the exception object (expandable via its `variablesReference`). Name it by its class name (e.g., `"FormatException"`).

   **6b. Variable reference management (`VariableStore`):**

   - Monotonically increasing `i64` IDs starting at 1, allocated per `variables` / `scopes` response
   - `HashMap<i64, VariableRef>` where `VariableRef` is:
     - `Scope(ScopeKind, frame_index)` — for scope expansion
     - `Object(isolate_id, object_id)` — for instance field/collection expansion
     - `MapEntry(isolate_id, key_object_id, value_object_id)` — for map key/value pairs
     - `GetterEval(isolate_id, instance_id, getter_name)` — for lazy getter evaluation
   - **Cleared on every `Resume` event** — all references become invalid when execution continues. The `on_resume()` handler calls `var_store.reset()` and `frame_store.reset()`.
   - **Cleared on `HotRestart`** — old isolate IDs and object IDs become stale. The `on_hot_restart()` handler invalidates all stores.

   **6c. `InstanceRef` → DAP `Variable` conversion (`instance_ref_to_variable`):**

   The VM Service returns `InstanceRef` objects with a polymorphic `kind` field. Each kind maps to a specific display strategy:

   | `InstanceKind` | Display value | `variablesReference` | `type` field |
   |---|---|---|---|
   | `Null` | `"null"` | 0 | `"Null"` |
   | `Bool` | `"true"` / `"false"` | 0 | `"bool"` |
   | `Int` | `valueAsString` (e.g., `"42"`) | 0 | `"int"` |
   | `Double` | `valueAsString` (e.g., `"3.14"`) | 0 | `"double"` |
   | `String` | `"\"hello\""` (quoted). If `valueAsStringIsTruncated`, append `…` | ref (for full string) | `"String"` |
   | `List` | `"List (3 items)"` using `length` field | ref → expand via indexed children | `classRef.name` (e.g., `"List<int>"`) |
   | `Map` | `"Map (5 items)"` using `length` field | ref → expand via `associations` | `classRef.name` |
   | `Set` | `"Set (2 items)"` using `length` field | ref → expand via `elements` | `classRef.name` |
   | `PlainInstance` | `classRef.name` + optional `toString()` result in parens | ref → expand fields | `classRef.name` |
   | `Closure` | `"Closure (functionName)"` using `closureFunction.name` | 0 | `"Closure"` |
   | `Record` | `"Record (N fields)"` | ref → expand named/positional fields | `"Record"` |
   | `Type` | `"Type (ClassName)"` using `name` field | 0 | `"Type"` |
   | `TypeParameter` | Type parameter name | 0 | `"TypeParameter"` |
   | `RegExp` | `classRef.name` | ref → expand fields | `"RegExp"` |
   | `StackTrace` | `"StackTrace"` | ref → expand `valueAsString` | `"StackTrace"` |
   | `Sentinel` | `valueAsString` or `"<optimized out>"` | 0 | `"Sentinel"` |
   | `WeakReference` | `"WeakReference"` | ref → expand `target` | `"WeakReference"` |
   | Other/unknown | `valueAsString` or `kind` | 0 | `kind` |

   **Important serialization note:** When locals are fetched via `backend.get_stack()`, the typed `Stack` struct is round-tripped through `serde_json::to_value()` which applies `#[serde(rename_all = "camelCase")]`. This means `class_ref` serializes as `"classRef"` in the JSON. The variable converter must read `"classRef"` (not `"class"`) for locals. When objects are fetched via `backend.get_object()`, the raw VM wire JSON is returned directly, which uses `"class"` as the key. The converter must handle both field names: `.get("classRef").or_else(|| instance_ref.get("class"))`.

   **6d. Collection expansion with pagination:**

   - `variables` request supports `filter: "indexed" | "named"`, `start`, `count` parameters
   - For `List`: report `indexedVariables: length` in parent variable. Client requests children with `filter: "indexed"`, `start: N`, `count: M`. Call `getObject(isolateId, listId, offset: start, count: count)` to get paginated elements. Name each `"[N]"`.
   - For `Map`: report `namedVariables: length`. Each entry expands to a child variable showing `"key_display → value_display"`. For primitive keys, use `valueAsString`. For complex keys, call `toString()` on the key object (with 1s timeout). Assign each key-value pair its own `variablesReference` (type `MapEntry`) for further expansion of key and value objects.
   - For `Set`: same as List — use `elements` with indexed access.
   - Cap per-request items with `MAX_VARIABLES_PER_REQUEST` constant (e.g., 100).

   **6e. Object field expansion (`expand_object`):**

   When the IDE expands a `PlainInstance` variable (clicks the `▶` arrow):
   1. Call `getObject(isolateId, objectId)` → returns full `Instance` object
   2. Read `fields` array: each `BoundField` has `{ decl: FieldRef, value: InstanceRef }`
   3. Convert each field's `value` to a DAP `Variable` with `name = decl.name`
   4. If `evaluateGettersInDebugViews` setting is `true`:
      - Traverse class hierarchy: `classRef` → `getObject(classId)` → `superClass` → repeat
      - For each class in hierarchy, collect getter names (filter `functions` where `kind == "Getter"` and not `static` and not `_identityHashCode`)
      - For each getter, call `evaluate(isolateId, objectId, getterName, disableBreakpoints: true)` with 1s timeout
      - Convert result to DAP `Variable`. On error, show `"<error: message>"`. On timeout, show `"<timed out>"`
      - Set `presentationHint.attributes: ["hasSideEffects"]` on getter variables
   5. If `evaluateGettersInDebugViews` is `false`: show getters as expandable lazy variables with `presentationHint.lazy: true` and `presentationHint.attributes: ["readOnly", "hasSideEffects"]`. Evaluate only when user explicitly expands.
   6. Filter out `@TypeArguments` from displayed variables (internal VM implementation detail)
   7. For Record types: expand positional fields as `$1`, `$2`, etc., and named fields by name

   **6f. `toString()` display for instances:**

   When `evaluateToStringInDebugViews` setting is `true`:
   - For `PlainInstance` and other non-primitive kinds, call `invoke(isolateId, objectId, "toString", [], disableBreakpoints: true)` with 1s timeout
   - If successful, append result in parentheses: `"MyClass (custom string repr)"`
   - If it fails or times out, show class name only (no error displayed — silent fallback)
   - Never call `toString()` on primitives, collections, closures, types, or sentinels

   **6g. `evaluateName` construction:**

   Each DAP `Variable` should include an `evaluateName` field that allows the IDE to construct watch expressions for drilling into objects:
   - Locals: `evaluateName = variableName` (e.g., `"myVar"`)
   - Fields: `evaluateName = parentEvaluateName + "." + fieldName` (e.g., `"myVar.name"`)
   - Nullable fields: `evaluateName = parentEvaluateName + "?." + fieldName` (when parent type is nullable)
   - Indexed: `evaluateName = parentEvaluateName + "[" + index + "]"` (e.g., `"myList[0]"`)
   - Map entries: `evaluateName = parentEvaluateName + "[" + keyExpression + "]"` (e.g., `'myMap["key"]'`)
   - Exception: `evaluateName = "$_threadException"`

   **6h. `VariablePresentationHint`:**

   | Scenario | `kind` | `attributes` | `visibility` |
   |---|---|---|---|
   | Object field | `"property"` | `[]` | `"public"` or `"private"` |
   | Getter (evaluated) | `"property"` | `["hasSideEffects"]` | `"public"` or `"private"` |
   | Getter (lazy) | `"property"` | `["readOnly", "hasSideEffects"]` | visibility from decl |
   | Const value | `"property"` | `["readOnly", "constant"]` | `"public"` |
   | Static field (global) | `"property"` | `["static"]` | `"public"` or `"private"` |
   | Closure | `"method"` | `[]` | `"public"` |
   | Type parameter | `"class"` | `["readOnly"]` | `"public"` |
   | Local variable | `"data"` | `[]` | `"public"` |

   **6i. Format specifiers:**

   The `evaluate` request `format` argument and trailing comma syntax in expressions support:
   - `nq` — no quotes (strip surrounding quotes from strings)
   - `h` — hex format for integers
   - `d` — decimal format (default)
   Parse format specifiers from the expression: if expression ends with `,nq` or `,h` or `,d`, strip the suffix and apply formatting to the result display.

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

### Phase 6: Tier-1 Feature Completion & Variable System Overhaul

**Goal**: Fix critical bugs in the existing implementation (issue #24 — empty variables panel), implement all missing features required for tier-1 parity with the official Dart DAP adapter, and add differentiating capabilities that go beyond what dart-code provides. After this phase, fdemon's DAP server can fully replace the built-in debugger for Flutter development in any IDE.

#### Post-Implementation Gap Analysis

The following issues and gaps were identified by comparing the current implementation against the official Dart DDS adapter (`pkg/dds`), the Dart-Code VS Code extension, and the full DAP specification:

**Critical Bugs:**

| Bug | Location | Impact |
|---|---|---|
| `"class"` vs `"classRef"` field name mismatch | `variables.rs:365-370` | Locals display wrong type names. `get_scope_variables` round-trips through typed `Stack` serialization (`serde(rename_all = "camelCase")`) producing `"classRef"`, but `instance_ref_to_variable` reads `"class"` (raw VM wire format). Class names are always `None` for locals. |
| `extract_source` used instead of `extract_source_with_store` in stack traces | `variables.rs:97` calls `extract_source` | SDK/package source frames in stack trace lack `sourceReference`, so `source` request fails for them. `extract_source_with_store` exists in `stack.rs:444` but is unused in the live code path. |
| `supportsRestartRequest: true` advertised but no handler | `types.rs:869` vs `handlers.rs:36-55` | IDE sends `restart` and gets `"unsupported command"` error. The capability claims support that doesn't exist. |

**Missing Tier-1 Features (must-have for parity with dart-code):**

| Feature | DAP Request/Event | VM Service Mapping | Current State |
|---|---|---|---|
| Globals scope | `variables` for Globals scope | `getObject(libraryId)` → `Library.variables` | Returns `Vec::new()` always (stub) |
| Exception scope | `scopes` with Exceptions scope | Store `exception` from `PauseException` event | Not created — only Locals + Globals scopes returned |
| `exceptionInfo` request | `exceptionInfo` | Read stored exception `InstanceRef` → structured data | Not implemented, capability not advertised |
| `restartFrame` | `restartFrame` | `resume(isolateId, step: kRewind, frameIndex: N)` | Not implemented, `kRewind` step mode not modeled |
| `callService` custom request | `callService` | `vmService.callMethod(method, params)` | Returns "unsupported command" |
| `updateDebugOptions` custom request | `updateDebugOptions` | `setLibraryDebuggable` per library | Returns "unsupported command" |
| `setLibraryDebuggable` integration | Implicit (via `updateDebugOptions`) | `setLibraryDebuggable(isolateId, libraryId, isDebuggable)` | Not implemented — SDK frames always deemphasized statically |
| `loadedSources` request | `loadedSources` | `getScripts(isolateId)` | Backend method exists (`get_scripts()`) but no handler calls it |
| `dart.hotReloadComplete` event | Custom event | After `reloadSources` completes | `hotReload` handler returns immediately, no completion event |
| `dart.hotRestartComplete` event | Custom event | After restart completes | Same — fire-and-forget, no completion event |
| `dart.serviceExtensionAdded` event | Custom event | `ServiceExtensionAdded` isolate event | Not forwarded to DAP client |
| Getter evaluation in variables | Implicit (variables panel) | `evaluate(isolateId, objectId, getterName)` per getter | Not implemented — only raw fields shown |
| `toString()` in variables panel | Implicit (variable value display) | `invoke(isolateId, objectId, "toString", [])` | Not implemented (only in hover evaluate context) |
| `evaluateName` construction | `Variable.evaluateName` field | N/A (client-side watch expression aid) | Not set on any variables |
| Record type expansion | `variables` for Record instances | `getObject` → positional + named fields | Falls to default `_` arm — flat string, no expansion |
| String truncation indicator | `Variable.value` display | `valueAsStringIsTruncated` field on `InstanceRef` | Not checked — truncated strings show without `…` |
| Complex map key display | `variables` for Map entries | `toString()` on key object | Shows `"?"` for non-primitive keys |
| Request timeout | All backend calls | N/A | `REQUEST_TIMEOUT` constant defined but never applied — hung VM Service blocks session |

**Should-have features (competitive differentiation):**

| Feature | DAP Request/Event | VM Service Mapping | Current State |
|---|---|---|---|
| Progress reporting | `progressStart` / `progressUpdate` / `progressEnd` | N/A (timed around hot reload/restart) | Not implemented |
| `breakpointLocations` request | `breakpointLocations` | `getSourceReport(PossibleBreakpoints)` | Not implemented |
| `completions` request | `completions` | Partial expression evaluation via `evaluate` | Not implemented, field not in Capabilities struct |
| `restart` request (session-level) | `restart` | Re-launch Flutter process | Capability advertised, no handler |
| `dart.log` event | Custom event | Adapter diagnostic forwarding | Not implemented |
| `dart.toolEvent` event | Custom event | Extension stream → ToolEvent | Not implemented |

**Features NOT feasible with Dart VM (do not implement):**

| Feature | Why |
|---|---|
| `setVariable` / `setExpression` | No VM Service API for mutating variable values |
| `stepBack` / `reverseContinue` | No time-travel debug support in Dart VM |
| `dataBreakpoints` | No watchpoint support in Dart VM |
| `instructionBreakpoints` | No instruction-level access in Dart VM |
| `readMemory` / `writeMemory` | No raw memory API in Dart VM |
| `disassemble` | No disassembly API (Dart is a managed runtime) |
| `gotoTargets` / `goto` | No arbitrary PC jump support |

#### Steps

1. **Fix critical variable display bugs** (`variables.rs`, `stack.rs`)
   - Fix `"class"` vs `"classRef"` mismatch: change `instance_ref_to_variable` to read `.get("classRef").or_else(|| instance_ref.get("class"))` so it handles both typed-serialized and raw VM wire formats
   - Switch `handle_stack_trace` to call `extract_source_with_store` instead of `extract_source`, enabling source references for SDK/package frames
   - Remove `supportsRestartRequest: true` from `fdemon_defaults()` until the `restart` handler is implemented
   - Wire `REQUEST_TIMEOUT` (10s) around all `backend.*()` calls using `tokio::time::timeout`

2. **Implement globals scope** (`variables.rs`, `backend.rs`)
   - Add `get_isolate(isolate_id) -> Result<Value>` to `DebugBackend` trait
   - In `get_scope_variables(Globals)`:
     1. Call `backend.get_stack()` to get the frame at `frame_index`
     2. Extract the `code.owner` library reference from the frame
     3. Call `backend.get_object(isolateId, libraryId)` to get the full `Library` object
     4. Read `Library.variables` (list of `FieldRef`)
     5. For each field, call `backend.get_object(isolateId, fieldId)` to get `Field.staticValue`
     6. Convert each to a DAP `Variable` with `presentationHint.attributes: ["static"]`
   - Mark globals scope as `expensive: true` (already done)
   - Handle libraries with many fields by supporting `start`/`count` pagination

3. **Implement exception scope** (`variables.rs`, `events.rs`, `stack.rs`)
   - Add `exception_reference: Option<(String, String)>` (isolate_id, object_id) to per-thread state in `VariableStore`
   - On `PauseException` event: store the exception's `InstanceRef` details
   - In `handle_scopes`: when `exception_reference.is_some()`, add a third scope "Exceptions" with the exception object
   - In `get_scope_variables(Exceptions)`: return a single variable named by the exception's class, with a `variablesReference` pointing to the exception object for field expansion
   - Support `$_threadException` magic expression in `handle_evaluate` — return the stored exception object

4. **Implement `exceptionInfo` request** (`handlers.rs`, `types.rs`)
   - Add `supportsExceptionInfoRequest: true` to `fdemon_defaults()` capabilities
   - Add `exceptionInfo` to the request dispatch table
   - Handler: read the stored exception `InstanceRef` for the given thread, call `getObject` to get full exception details, call `toString()` for the description, return `ExceptionInfoResponse { exceptionId, description, breakMode, details: { message, typeName, stackTrace } }`
   - `breakMode` maps from the current `ExceptionPauseMode`: `All` → `"always"`, `Unhandled` → `"unhandled"`, `None` → `"never"`

5. **Implement `restartFrame` request** (`handlers.rs`, `types.rs`)
   - Add `kRewind` variant to `StepMode` enum in `types.rs`
   - Add `supportsRestartFrame` field to `Capabilities` struct, set `true` in `fdemon_defaults()`
   - Add `restartFrame` to the request dispatch table
   - Handler: validate `frameId`, look up the frame's isolate and index, call `backend.resume(isolateId, step: Rewind, frameIndex: frameIndex)`
   - Reject frames above the first async suspension marker (cannot rewind async boundaries) — return error response with clear message
   - On success, emit `StoppedEvent(reason: "restart")` when the isolate pauses at the rewound frame

6. **Implement `loadedSources` request** (`handlers.rs`, `types.rs`)
   - Add `supportsLoadedSourcesRequest: true` to `fdemon_defaults()`
   - Add `loadedSources` to the request dispatch table
   - Handler: call `backend.get_scripts(isolateId)` (already in `DebugBackend` trait), convert each `ScriptRef` to a DAP `Source` using `extract_source_with_store`, return all sources
   - Filter out internal/generated scripts (those with `dart:_internal`, `eval:source`, etc.)

7. **Implement `callService` custom request** (`handlers.rs`)
   - Add `callService` to the request dispatch table
   - Handler: read `method` and `params` from the request arguments, call `backend.call_service(method, params)` → forwards to `VmRequestHandle::request(method, params)`
   - Add `call_service(method, params) -> Result<Value>` to `DebugBackend` trait
   - Return the raw VM Service response as the DAP response body
   - This enables DevTools integration — VS Code calls `callService("ext.flutter.debugDumpApp", {})` etc.

8. **Implement `updateDebugOptions` custom request** (`handlers.rs`, `backend.rs`)
   - Add `updateDebugOptions` to the request dispatch table
   - Handler: read `debugSdkLibraries: bool` and `debugExternalPackageLibraries: bool` from arguments
   - Store the settings on the adapter state
   - Call `set_library_debuggable(isolateId, libraryId, isDebuggable)` for each library in each isolate:
     - SDK libraries (`dart:*`): debuggable if `debugSdkLibraries == true`
     - External package libraries: debuggable if `debugExternalPackageLibraries == true`
     - App libraries: always debuggable
   - Add `set_library_debuggable(isolate_id, library_id, is_debuggable) -> Result<()>` to `DebugBackend` trait
   - Add `get_isolate(isolate_id) -> Result<Value>` to `DebugBackend` trait (needed to enumerate libraries)
   - Re-apply on every `IsolateRunnable` event (new isolates must be configured)

9. **Implement getter evaluation and `toString()` in variables** (`variables.rs`)
   - Add `evaluateGettersInDebugViews: bool` and `evaluateToStringInDebugViews: bool` settings (from attach/launch args, default both `true`)
   - In `expand_object` for `PlainInstance`:
     - After fields, traverse class hierarchy to collect getter names
     - For each getter (excluding `_identityHashCode`, `hashCode`, `runtimeType` for primitives):
       - Call `backend.evaluate(isolateId, objectId, getterName)` with `disableBreakpoints: true` and 1s timeout
       - On success: convert to DAP `Variable` with `presentationHint.attributes: ["hasSideEffects"]`
       - On error/timeout: show `"<error: message>"` or `"<timed out>"` as value
     - When `evaluateGettersInDebugViews == false`: show getters with `presentationHint.lazy: true` (expandable on demand)
   - In `instance_ref_to_variable` for `PlainInstance`:
     - If `evaluateToStringInDebugViews == true`: call `backend.evaluate(isolateId, objectId, "toString()")` with 1s timeout, append result in parentheses: `"MyClass (custom string repr)"`
     - On failure: show class name only (silent fallback)

10. **Implement `evaluateName` construction** (`variables.rs`)
    - Thread an `evaluate_name: Option<String>` through the variable conversion pipeline
    - Set `evaluateName` on every DAP `Variable` returned:
      - Locals: `evaluateName = name`
      - Fields: `evaluateName = parent_evaluate_name + "." + fieldName`
      - Indexed: `evaluateName = parent_evaluate_name + "[" + index + "]"`
      - Map entries: `evaluateName = parent_evaluate_name + "[" + keyExpression + "]"`
      - Exception: `evaluateName = "$_threadException"`
    - This enables watch expressions to drill into nested objects

11. **Implement Record and additional type expansion** (`variables.rs`)
    - Add `Record` to the match arms in `instance_ref_to_variable`:
      - Display: `"Record (N fields)"`
      - Expansion: positional fields as `$1`, `$2`, ..., named fields by name
      - Call `getObject(objectId)` → read `fields` array
    - Add `WeakReference`:
      - Display: `"WeakReference"`
      - Expansion: show `target` field (which may be `null` if collected)
    - Add `Sentinel`:
      - Display: `valueAsString` or `"<optimized out>"` / `"<not initialized>"`
      - No expansion (`variablesReference: 0`)
    - Fix string truncation: check `valueAsStringIsTruncated` field, append `…` when true

12. **Implement progress reporting for hot reload/restart** (`handlers.rs`, `events.rs`)
    - Check `client_capabilities.supports_progress_reporting` from the `initialize` request
    - On `hotReload` request: emit `progressStart(title: "Hot Reload")`, then on completion emit `progressEnd`
    - On `hotRestart` request: emit `progressStart(title: "Hot Restart")`, then on completion emit `progressEnd`
    - Add `hot_reload` / `hot_restart` completion events:
      - Modify `DebugBackend::hot_reload()` and `hot_restart()` to return a future that resolves when the operation completes (not fire-and-forget)
      - On completion, emit `dart.hotReloadComplete` / `dart.hotRestartComplete` custom events
    - Use a `DapProgressReporter` helper that tracks progress IDs and emits start/update/end events

13. **Implement `restart` request (session-level)** (`handlers.rs`)
    - Add `restart` to the request dispatch table (capability already advertised)
    - Handler: perform a hot restart — `backend.hot_restart()`, re-apply breakpoints, invalidate variable stores
    - This is distinct from `restartFrame` (single-frame rewind) — `restart` re-runs the entire Flutter app

14. **Implement missing custom DAP events** (`events.rs`)
    - `dart.serviceExtensionAdded`: on `ServiceExtensionAdded` isolate event, forward as custom DAP event with `{ extensionRPC, method }`
    - `dart.log`: when `sendLogsToClient` setting is true, forward adapter diagnostic messages as `dart.log` events
    - `flutter.forwardedEvent`: forward relevant `flutter run --machine` daemon events (e.g., `app.webLaunchUrl`, `app.warning`)

15. **Implement `breakpointLocations` request** (`handlers.rs`, `backend.rs`)
    - Add `supportsBreakpointLocationsRequest: true` to `fdemon_defaults()`
    - Add `breakpointLocations` to the request dispatch table
    - Handler: call `backend.get_source_report(isolateId, scriptId, ["PossibleBreakpoints"], tokenPos, endTokenPos)`
    - Convert the source report ranges to DAP `BreakpointLocation` objects
    - This enables the IDE to show valid breakpoint positions when the user hovers over the gutter

16. **Implement `completions` request** (`handlers.rs`, `types.rs`)
    - Add `supportsCompletionsRequest: true` to `fdemon_defaults()`
    - Add `completions` to the request dispatch table
    - Handler: use partial expression evaluation — attempt `evaluateInFrame` with the text up to the cursor, catch compilation errors, parse error messages for available identifiers
    - Alternative simpler approach: enumerate `frame.vars` names + library top-level names + class field names for the current scope
    - Return `CompletionItem[]` with `label`, `type` ("variable", "method", "property"), and `sortText`
    - This is a differentiator — neither the Dart DDS adapter nor Dart-Code implement `completions`

17. **Wire request timeouts** (`handlers.rs` or adapter layer)
    - Wrap all `backend.*()` calls with `tokio::time::timeout(REQUEST_TIMEOUT, ...)` (10s)
    - On timeout: return DAP error response `"Request timed out after 10s"`
    - Prevents a hung VM Service from blocking the entire DAP session indefinitely
    - Special case: `toString()` and getter evaluation use a shorter 1s timeout (they're side-effect calls that should be fast)

18. **Fix map key display for complex objects** (`variables.rs`)
    - In `expand_object` for Map entries:
      - For primitive keys (`Int`, `Bool`, `String`, `Double`): use `valueAsString` directly
      - For complex keys: call `toString()` on the key object with 1s timeout
      - On failure: show the class name (e.g., `"MyKey instance"`) instead of `"?"`
    - Set `evaluateName` for map entries using the key expression

**Milestone**: Variables panel shows locals, globals, and exception values correctly. Getters and `toString()` display rich object representations. `restartFrame` enables frame rewind. `callService` and `updateDebugOptions` enable full DevTools integration. Progress reporting shows hot reload/restart status in IDE. `breakpointLocations` and `completions` differentiate fdemon above the official Dart adapter. The empty variables panel (issue #24) is fully resolved.

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

### Typed vs Raw JSON Serialization Mismatch (Phase 6)

- **Risk:** The `VmServiceBackend::get_stack()` round-trips through `serde_json::to_value(&stack)` which applies `#[serde(rename_all = "camelCase")]`, producing `"classRef"`, `"valueAsString"`, etc. But `get_object()` returns raw VM wire JSON which uses `"class"`, `"valueAsString"`, etc. Any code consuming both paths must handle both field name conventions.
- **Mitigation:** Use `.get("classRef").or_else(|| .get("class"))` for all polymorphic field lookups. Long-term: either always use raw JSON (skip typed deserialization for stack responses) or always use typed deserialization (add `getObject` typed parsing). Phase 6 Step 1 fixes the immediate bug.

### Getter Evaluation Performance (Phase 6)

- **Risk:** Evaluating all getters on a class hierarchy can trigger hundreds of `evaluate` RPCs per variable expansion, each potentially executing user code. A single variable click could cause multi-second delays.
- **Mitigation:** (1) 1s timeout per getter evaluation. (2) Default to `evaluateGettersInDebugViews: false` — show getters as lazy expandable items. (3) Filter out `_identityHashCode` and `hashCode` (frequently expensive, rarely useful). (4) Show a maximum of 50 getter results per object. (5) Evaluate getters sequentially (not in parallel) to avoid overwhelming the VM.

### `toString()` Side Effects in Variables Panel (Phase 6)

- **Risk:** `toString()` calls execute arbitrary user code. A buggy `toString()` implementation could throw an exception, mutate state, or infinite-loop — all while the user is just looking at variables.
- **Mitigation:** (1) Call with `disableBreakpoints: true` to prevent recursive pause. (2) 1s timeout. (3) Silent fallback on error — show class name only, never show error in the variable value. (4) Skip `toString()` for framework-internal types where the default `Instance of 'ClassName'` is adequate.

### `restartFrame` Across Async Boundaries (Phase 6)

- **Risk:** VM Service's `kRewind` step mode only works for synchronous frames. Attempting to rewind past an async suspension marker crashes the isolate or returns an error.
- **Mitigation:** Track the index of the first async suspension marker in the stack. Reject `restartFrame` requests for frames at or above this index with a clear error: `"Cannot restart frame above an async suspension boundary"`. The IDE will gray out the restart option for those frames.

### `callService` Security (Phase 6)

- **Risk:** `callService` allows the IDE to invoke arbitrary VM Service RPCs, including `kill`, `requestHeapSnapshot`, or any service extension. A malicious or buggy IDE extension could disrupt the debug session.
- **Mitigation:** (1) Only accept `callService` from authenticated DAP clients (localhost-only by default). (2) Log all `callService` invocations at `debug` level for auditability. (3) Reject `callService` calls to destructive RPCs (`kill`, `enableProfiler`) unless the `allow_destructive_call_service` config is `true` (default: `false`).

### Variable Store Memory Growth (Phase 6)

- **Risk:** Every `variablesReference` allocation adds an entry to the `HashMap`. Deep object graphs (e.g., widget trees with thousands of children) could grow the store unboundedly during a single pause.
- **Mitigation:** (1) Cap `VariableStore` at 10,000 entries. Once exceeded, new allocations return `variablesReference: 0` (non-expandable) with a warning in the value string. (2) The store is already cleared on every resume, so growth is bounded to a single pause session.

### `completions` Accuracy (Phase 6)

- **Risk:** Debug console auto-completion based on frame variable names may suggest identifiers that don't exist in the current scope, or miss imported names.
- **Mitigation:** Start with a conservative approach: only suggest `frame.vars` names + top-level library names. This guarantees accuracy at the cost of completeness. More sophisticated approaches (partial evaluation, AST parsing) can be added later if needed.

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

### Phase 6 Complete When:

**Critical bug fixes (must pass before any new features):**
- [ ] `"classRef"` / `"class"` field name mismatch fixed — locals display correct type names (class name, not `kind`)
- [ ] `extract_source_with_store` used in `handle_stack_trace` — SDK/package source frames have `sourceReference`, IDE can view source
- [ ] `supportsRestartRequest` removed from capabilities until `restart` handler exists (or `restart` handler implemented)
- [ ] `REQUEST_TIMEOUT` applied to all backend calls via `tokio::time::timeout` — hung VM Service no longer blocks session

**Variable system (issue #24 resolution):**
- [ ] Globals scope returns library static fields (not empty `Vec::new()`)
- [ ] Exception scope appears when paused at exception with the exception object expandable
- [ ] `$_threadException` magic expression works in evaluate/watch contexts
- [ ] String truncation indicator (`…`) appended when `valueAsStringIsTruncated` is true
- [ ] Record types expand with positional (`$1`, `$2`) and named fields
- [ ] Map entries with complex keys show `toString()` result (not `"?"`)
- [ ] `evaluateName` set on all variables (enables watch expression drill-down)
- [ ] Getter evaluation works when `evaluateGettersInDebugViews` is true (with 1s timeout per getter)
- [ ] `toString()` display appended to `PlainInstance` values when `evaluateToStringInDebugViews` is true
- [ ] `VariablePresentationHint` set correctly for fields, getters, statics, closures, consts

**New DAP requests:**
- [ ] `exceptionInfo` — returns structured exception data with description, type, stack trace
- [ ] `restartFrame` — rewinds to selected frame via VM Service `kRewind` step mode; rejects async frames
- [ ] `loadedSources` — returns all scripts via `get_scripts()` backend method
- [ ] `callService` — forwards arbitrary VM Service RPCs (for DevTools integration)
- [ ] `updateDebugOptions` — toggles `debugSdkLibraries` and `debugExternalPackageLibraries` via `setLibraryDebuggable`
- [ ] `breakpointLocations` — returns valid breakpoint positions via `getSourceReport(PossibleBreakpoints)`
- [ ] `completions` — debug console auto-complete from scope variables and library names
- [ ] `restart` — session-level restart (hot restart) with handler matching advertised capability

**Custom events:**
- [ ] `dart.hotReloadComplete` and `dart.hotRestartComplete` emitted after operations complete
- [ ] `dart.serviceExtensionAdded` forwarded from `ServiceExtensionAdded` isolate events
- [ ] Progress events (`progressStart` / `progressEnd`) emitted during hot reload/restart when client supports them

**Quality gates:**
- [ ] 200+ new unit tests for variable system (type rendering, pagination, getters, toString, edge cases)
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace` clean
- [ ] Tested end-to-end with VS Code Dart extension: variables panel shows locals, globals, exception values, getters, and collections correctly
- [ ] Tested with Neovim nvim-dap: same variable inspection works
- [ ] Performance tested: expanding a 10,000-element List doesn't hang (pagination works)
- [ ] Performance tested: `toString()` timeout prevents infinite-loop hang

---

## Future Enhancements

- **Stdio transport**: Support DAP over stdin/stdout (in addition to TCP) for editors that prefer spawning the adapter as a subprocess
- **Launch mode**: Support `launch` request (not just `attach`) — fdemon spawns the Flutter process when the IDE starts debugging
- **Inline values**: Support DAP's `supportsInlineValues` capability for showing variable values inline in the editor. Requires `evaluateInFrame` for every visible variable on every step — expensive, needs careful caching. Neither Dart DDS adapter nor Dart-Code implement this as of 2026.
- **Dedicated IDE extensions**: Publish VS Code extension and Neovim plugin that auto-discover fdemon's DAP port and provide richer integration (Phase 5 handles config generation; dedicated extensions would add live port discovery, status bar indicators, and custom DAP views)
- **Zed Dart debugger extension**: Write a Zed extension implementing `get_dap_binary` for Dart/Flutter so `.zed/debug.json` auto-config actually works (currently Dart is not in Zed's supported debugger languages)
- **Remote debugging**: Support non-localhost bind addresses for remote Flutter device debugging
- **Profiling integration**: Expose fdemon's performance data through DAP events for integrated perf views
- **IntelliJ DAP via LSP4IJ**: If the LSP4IJ plugin gains wider adoption, add `.idea/runConfigurations/` XML generation for IntelliJ/Android Studio DAP support
- **`functionBreakpoints`**: Map to `addBreakpointAtEntry(functionId)` VM Service RPC — requires resolving function names to IDs via library/class traversal. Not in the Dart DDS adapter.
- **`modules` request**: Map Dart libraries/packages to DAP `Module` objects for navigating loaded code. Low priority — `loadedSources` covers the primary use case.
- **`dart.toolEvent` forwarding**: Forward VM Service Extension stream `ToolEvent` data to IDE for Flutter DevTools integration. Enables embedded DevTools views.
- **Format specifiers**: Support trailing comma format syntax in evaluate expressions (`expr,nq` for no quotes, `expr,h` for hex, `expr,d` for decimal). Matches Dart DDS adapter behavior.
