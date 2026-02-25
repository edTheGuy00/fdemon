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
- `crates/fdemon-app/src/message.rs` — Add DAP Message variants
- `crates/fdemon-app/src/handler/mod.rs` — Add DAP UpdateAction variants
- `crates/fdemon-app/src/session/handle.rs` — Add DAP shutdown/task fields to `SessionHandle`
- `crates/fdemon-app/src/engine_event.rs` — Wire up unimplemented `EngineEvent` variants needed by DAP
- `crates/fdemon-app/Cargo.toml` — Depend on `fdemon-dap` (or feature-gated)
- `Cargo.toml` (workspace) — Add `fdemon-dap` member, binary deps
- `src/main.rs` — Add `--dap` / `--dap-port` CLI flags

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
- `crates/fdemon-app/src/handler/dap.rs` — **NEW**: DAP message handler in TEA
- `crates/fdemon-app/src/session/debug_state.rs` — **NEW**: Per-session debug state
- `src/dap/mod.rs` — **NEW**: DAP runner (binary crate, like `src/headless/`)
- `src/dap/runner.rs` — **NEW**: DAP server event loop

---

## Architecture

### Crate Dependency Graph (with fdemon-dap)

```
┌─────────────────────────────────────────────────────┐
│           flutter-demon (binary crate)               │
│     CLI + TUI runner + headless + DAP runner         │
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

DAP server task (tokio::spawn)
  ├── TcpListener on configured port
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

6. **DAP runner** (`src/dap/runner.rs` in binary crate)
   - `run_dap_server(project_path, port) -> Result<()>`
   - Creates `Engine`, subscribes to events
   - Spawns DAP TCP server task with `engine.msg_sender()` + `VmRequestHandle`
   - Runs Engine event loop (reuses headless pattern)
   - CLI: `fdemon --dap [--dap-port 4711] [project_path]`

7. **Integration with Engine lifecycle**
   - Add `Message::DapClientConnected { session_id }` and `Message::DapClientDisconnected { session_id }` variants
   - Add DAP shutdown fields to `SessionHandle`
   - Wire up cleanup in all session teardown paths

**Milestone**: `fdemon --dap` starts a TCP server. VS Code (or `dap-client` test tool) can connect, complete initialization, and receive capabilities response. No debugging yet, but the transport works end-to-end.

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

---

## Configuration Additions

```toml
# .fdemon/config.toml

[dap]
# Enable DAP server (can also use --dap CLI flag)
enabled = false

# TCP port for DAP connections (default: 0 = auto-assign)
port = 0

# Bind address (default: 127.0.0.1 for security)
bind_address = "127.0.0.1"

# Suppress auto-reload while debugger is paused at breakpoint
suppress_reload_on_pause = true

# Auto-attach debugger when session starts (vs waiting for IDE to attach)
auto_attach = false
```

---

## CLI Additions

```
fdemon --dap [--dap-port PORT] [--dap-bind ADDRESS] [project_path]

Options:
  --dap              Start with DAP server enabled
  --dap-port PORT    Port for DAP server (default: 0 = auto-assign, printed to stdout)
  --dap-bind ADDR    Bind address (default: 127.0.0.1)
```

When `--dap-port 0` is used, the actual port is printed to stdout as JSON for IDE integration:
```json
{"dapPort": 54321}
```

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
- [ ] `fdemon --dap` CLI flag works
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
- [ ] Hot reload/restart work from debug toolbar
- [ ] Breakpoints persist across hot restart
- [ ] Auto-reload suppressed while paused
- [ ] Expression evaluation works in debug console and hover
- [ ] Conditional breakpoints and logpoints work
- [ ] SDK/package sources viewable in IDE
- [ ] Multi-session debugging works (multiple Flutter sessions)
- [ ] Custom DAP events received by IDE (dart.debuggerUris, flutter.appStarted)
- [ ] Tested against VS Code, Neovim, and Helix
- [ ] Performance tested: variable expansion doesn't hang on large objects
- [ ] Documentation updated

---

## Future Enhancements

- **Stdio transport**: Support DAP over stdin/stdout (in addition to TCP) for editors that prefer spawning the adapter as a subprocess
- **Launch mode**: Support `launch` request (not just `attach`) — fdemon spawns the Flutter process when the IDE starts debugging
- **Data breakpoints**: Break on field value changes (requires VM Service `addBreakpointOnActivation` or equivalent)
- **Memory inspection**: Expose fdemon's memory profiling data through DAP's `readMemory` request
- **Inline values**: Support DAP's `inlineValues` capability for showing variable values inline in the editor
- **IDE extensions**: Publish VS Code extension and Neovim plugin that auto-discover fdemon's DAP port
- **Remote debugging**: Support non-localhost bind addresses for remote Flutter device debugging
- **Profiling integration**: Expose fdemon's performance data through DAP events for integrated perf views
