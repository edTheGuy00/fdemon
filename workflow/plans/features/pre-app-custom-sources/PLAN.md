# Plan: Pre-App Custom Sources with Readiness Checks

## TL;DR

Add `start_before_app` and `ready_check` options to `[[native_logs.custom_sources]]` so that processes like backend servers can be spawned and verified healthy before the Flutter app launches. This bridges the gap for projects that require companion processes (API servers, sidecar services) to be ready before the Flutter frontend connects.

---

## Background

### The Problem

fdemon's `[[native_logs.custom_sources]]` currently spawn **after** the Flutter app emits its `AppStarted` event. This works for log-only sources but fails for companion processes that the Flutter app depends on — like a backend API server.

In the user's workflow (zabin-app project), they run both a Flutter desktop app and `cargo run -p server` in separate terminals. The server **must be healthy** before the Flutter app tries to connect, or the app hits connection errors on startup. Today, users must manually start the server in a separate terminal — defeating the "single terminal" benefit of custom sources.

### Current Flow

```
handle_launch() → UpdateAction::SpawnSession
  → FlutterProcess::spawn() → flutter run --machine
  → daemon stdout events...
  → DaemonMessage::AppStart ← THE GATE
    → maybe_start_native_log_capture()
      → spawn_custom_sources()  ← ALL custom sources spawn HERE
```

Every custom source starts **after** `AppStarted`, which itself fires after Gradle/Xcode build + Dart compilation. There is no pre-launch hook for custom sources.

### Desired Flow

```
handle_launch()
  → IF has pre-app sources:
      UpdateAction::SpawnPreAppSources { ... }
        → spawn pre-app custom sources
        → run readiness checks (HTTP, TCP, command, stdout pattern, delay)
        → on ready: Message::PreAppSourcesReady
          → UpdateAction::SpawnSession (normal flow continues)
        → on timeout: Message::PreAppSourcesTimedOut
          → proceed anyway with warning (configurable)
  → ELSE:
      UpdateAction::SpawnSession (unchanged — zero behavioral change)
```

---

## Affected Modules

### Core Types (`fdemon-core`)
- `crates/fdemon-core/src/types.rs` — Add `ReadyCheck` enum (config type, serde-enabled)

### App Config (`fdemon-app`)
- `crates/fdemon-app/src/config/types.rs` — Extend `CustomSourceConfig` with `start_before_app: bool` and `ready_check: Option<ReadyCheck>`. Add validation.

### App Messages (`fdemon-app`)
- `crates/fdemon-app/src/message.rs` — Add `PreAppSourcesReady`, `PreAppSourcesTimedOut`, `PreAppSourceProgress` variants

### App Handler (`fdemon-app`)
- `crates/fdemon-app/src/handler/mod.rs` — Add `UpdateAction::SpawnPreAppSources` variant
- `crates/fdemon-app/src/handler/update.rs` — Handle new message variants
- `crates/fdemon-app/src/handler/new_session/launch_context.rs` — Conditional dispatch: `SpawnPreAppSources` vs `SpawnSession`

### App Actions (`fdemon-app`)
- `crates/fdemon-app/src/actions/native_logs.rs` — Add `spawn_pre_app_sources()` function with readiness check logic
- `crates/fdemon-app/src/actions/ready_check.rs` — **NEW** Ready check execution (HTTP, TCP, command, stdout, delay)
- `crates/fdemon-app/src/actions/mod.rs` — Dispatch `SpawnPreAppSources` action

### App Session (`fdemon-app`)
- `crates/fdemon-app/src/session/handle.rs` — Track pre-app source handles alongside existing `custom_source_handles`

### Daemon Layer (`fdemon-daemon`)
- `crates/fdemon-daemon/src/native_logs/custom.rs` — Extend `CustomLogCapture` to optionally accept a stdout ready pattern and provide a `oneshot::Receiver<()>` for readiness signaling

### TUI Layer (`fdemon-tui`)
- `crates/fdemon-tui/src/widgets/log_view/` — Display pre-app source progress messages in log view (uses existing `LogSource::Daemon` or new `LogSource::System`)

---

## Design Decisions

### Decision 1: Readiness Check Types

**Chosen: Five strategies — HTTP, TCP, command, stdout pattern, fixed delay.**

| Type | Use Case | Success Condition |
|------|----------|-------------------|
| `http` | Backend API with REST health endpoint | HTTP GET returns 2xx status |
| `tcp` | Database, Redis, generic TCP service | TCP connection established |
| `command` | gRPC servers, custom protocols, databases | Command exits with code 0 |
| `stdout` | Process prints "ready" message | Regex pattern matched in stdout |
| `delay` | Simple wait, no check needed | Timer elapsed |

Rationale: These five cover virtually all real-world readiness scenarios. HTTP and TCP are the most common for REST backend services. Command is the most flexible — it covers gRPC (via `grpcurl`), databases (via `pg_isready`), and any protocol fdemon doesn't need to understand natively. Stdout matching handles processes without network endpoints. Delay is a simple fallback.

### Decision 2: Where to Gate the Flutter Launch

**Chosen: New `UpdateAction::SpawnPreAppSources` dispatched from `handle_launch()`, which sends `Message::PreAppSourcesReady` on completion to trigger `UpdateAction::SpawnSession`.**

Rationale: This follows the TEA pattern cleanly — actions produce messages, messages trigger state changes and further actions. The existing `handle_launch()` already decides what action to return; it can check for pre-app sources and conditionally return the new action variant. No structural changes to the TEA loop needed. `UpdateResult` already supports one action + one message, which is sufficient.

Alternative considered: Gating inside `spawn_session()` (Option A from research). Rejected because it would block the async task and prevent UI updates during the readiness wait. The TEA message-based approach allows progress feedback to the UI.

### Decision 3: Timeout Behavior

**Chosen: Proceed with Flutter launch on timeout, with a warning log.**

When a readiness check times out:
1. Log a prominent warning: "Server 'my-server' readiness check timed out after 30s. Proceeding with Flutter launch."
2. The custom source process **continues running** (it may become ready later)
3. The Flutter app launches normally

Rationale: Blocking indefinitely is a worse UX than a potentially flaky first connection. Most Flutter apps handle backend unavailability gracefully (retry logic, error screens). The custom source's logs are still captured regardless of readiness status. Users can increase the timeout if needed.

### Decision 4: Hot Restart Behavior

**Chosen: Pre-app sources are NOT restarted on hot restart.**

Pre-app sources are long-running companion processes (servers, databases). They should persist across Flutter hot restarts. The existing `maybe_start_native_log_capture()` guard at `handler/session.rs:312` already prevents re-spawning when `custom_source_handles` is non-empty. Pre-app sources will be stored in the same `custom_source_handles` vec, so the guard automatically applies.

### Decision 5: `ready_check` Implies `start_before_app`

**Chosen: `ready_check` requires `start_before_app = true`. Specifying `ready_check` without `start_before_app = true` is a validation error.**

Rationale: A readiness check only makes sense for pre-app sources — there's nothing to gate for post-app sources. Making this explicit prevents user confusion and keeps the mental model clear.

`start_before_app = true` WITHOUT `ready_check` is valid — it means "start this process before the Flutter app, don't wait for it." Useful for fire-and-forget background processes.

### Decision 6: HTTP Check Implementation

**Chosen: Minimal HTTP/1.1 over `tokio::net::TcpStream` — no new dependency.**

For health check endpoints (almost always localhost HTTP, not HTTPS), a raw TCP connection with a minimal HTTP/1.1 GET request is sufficient:
1. `TcpStream::connect(parsed_url.host:port)`
2. Write `GET /path HTTP/1.1\r\nHost: host\r\nConnection: close\r\n\r\n`
3. Read response line, check for `HTTP/1.x 2xx`

Rationale: Avoids adding `reqwest` (~40 crates in dependency tree) for a single use case. Health check endpoints are almost universally HTTP (not HTTPS) on localhost. If HTTPS is needed, users can use the `tcp` check type instead (verifies the port is open).

### Decision 7: Stdout Ready Pattern and Log Capture Coexistence

**Chosen: Integrate stdout pattern matching into the existing `run_custom_capture()` loop, with a `oneshot::Sender<()>` for signaling.**

The stdout stream is already consumed by `run_custom_capture()` for log parsing. Rather than splitting the stream, we add an optional `ready_pattern: Option<Regex>` and `ready_tx: Option<oneshot::Sender<()>>` to the capture configuration. Each line is checked against the pattern before normal log processing. When matched, `ready_tx.send(())` fires and the pattern field is cleared (no further matching needed).

Rationale: Single consumer of stdout avoids stream-splitting complexity. The capture loop already processes every line, so the pattern check is ~zero cost. The readiness signal is separate from the log event channel, allowing the pre-app action to `select!` on readiness OR timeout.

### Decision 8: Command Check Type for Protocol-Agnostic Readiness

**Chosen: Run an arbitrary command in a loop, success = exit code 0.**

The `command` check type executes a user-specified command+args repeatedly until it exits with code 0, or the timeout is reached. This is the most flexible readiness strategy — it delegates protocol knowledge to external tools that already exist:

- **gRPC**: `grpcurl -plaintext localhost:50051 grpc.health.v1.Health/Check`
- **PostgreSQL**: `pg_isready -h localhost -p 5432`
- **Redis**: `redis-cli ping`
- **Custom**: any script or binary that returns 0 on success

Implementation: `tokio::process::Command::new(cmd).args(args).status().await` in a loop. No stdout/stderr capture needed — only the exit code matters. Uses the same `interval_ms` and `timeout_s` fields as HTTP/TCP checks.

Rationale: Adding native support for every protocol (gRPC/HTTP2, database wire protocols, etc.) would balloon complexity. The `command` type lets users leverage existing CLI tools with zero protocol-specific code in fdemon. It also serves as an escape hatch for any readiness scenario the built-in types can't handle.

### Decision 9: Pre-App Source Phase UI

**Chosen: Show a loading phase with progress messages in the session's log buffer.**

When pre-app sources are being started:
- Session phase shows "Starting services..." (or similar) instead of "Launching..."
- Progress messages appear as `LogSource::Daemon` entries: "Starting server 'my-server'...", "Waiting for server readiness (http://localhost:8080/health)...", "Server 'my-server' ready (took 3.2s)"
- The log view is live during the wait, so users see the custom source's stdout in real time (the capture loop is already running)

---

## Development Phases

### Phase 1: Pre-App Custom Sources (Single Phase)

**Goal**: Allow custom sources to start before the Flutter app with configurable readiness checks, gating the Flutter launch until dependencies are healthy.

#### Steps

1. **Add `ReadyCheck` Config Type**
   - Add `ReadyCheck` serde enum to `crates/fdemon-app/src/config/types.rs` (alongside `CustomSourceConfig`)
   - Variants: `Http { url, interval_ms, timeout_s }`, `Tcp { host, port, interval_ms, timeout_s }`, `Command { command, args, interval_ms, timeout_s }`, `Stdout { pattern, timeout_s }`, `Delay { seconds }`
   - Default `interval_ms`: 500, default `timeout_s`: 30, default `seconds`: 5
   - Validation: HTTP url must parse, TCP port must be valid, stdout pattern must be valid regex, command must be non-empty

2. **Extend `CustomSourceConfig`**
   - Add `start_before_app: bool` (default: false) to app-layer `CustomSourceConfig`
   - Add `ready_check: Option<ReadyCheck>` to app-layer `CustomSourceConfig`
   - Add validation: `ready_check.is_some()` requires `start_before_app == true`
   - Add `has_pre_app_sources()` helper on `NativeLogsSettings` to check if any source has `start_before_app = true`

3. **Add Message Variants**
   - `Message::PreAppSourcesReady { session_id, device, config }` — all pre-app sources are ready or timed out
   - `Message::PreAppSourcesTimedOut { session_id, source_name, device, config }` — a specific source timed out (informational, logged as warning)
   - `Message::PreAppSourceProgress { session_id, message }` — progress update for UI feedback

4. **Add `UpdateAction::SpawnPreAppSources`**
   - New variant in `UpdateAction` enum carrying: `session_id`, `device`, `config`, `pre_app_sources: Vec<CustomSourceConfig>`, `settings: NativeLogsSettings`, `project_path: PathBuf`
   - Dispatch in `actions/mod.rs` → `native_logs::spawn_pre_app_sources()`

5. **Modify Launch Flow**
   - In `handle_launch()` (`handler/new_session/launch_context.rs`): after creating `SessionHandle`, check `settings.native_logs.has_pre_app_sources()`
   - If pre-app sources exist: return `UpdateAction::SpawnPreAppSources { ... }` instead of `UpdateAction::SpawnSession`
   - If no pre-app sources: return `UpdateAction::SpawnSession` as before (zero behavioral change)
   - Handle `Message::PreAppSourcesReady` → return `UpdateAction::SpawnSession { ... }`
   - Handle `Message::PreAppSourceProgress` → add info log entry to session

6. **Implement Ready Check Execution**
   - Create `crates/fdemon-app/src/actions/ready_check.rs`
   - `async fn run_ready_check(check: &ReadyCheck, ready_rx: Option<oneshot::Receiver<()>>) -> Result<Duration>`
   - HTTP: loop { `TcpStream::connect` → write GET → read status line → break on 2xx; sleep interval }
   - TCP: loop { `TcpStream::connect` → break on Ok; sleep interval }
   - Command: loop { `tokio::process::Command::new(cmd).args(args).status().await` → break on exit code 0; sleep interval }
   - Stdout: `ready_rx.await` with timeout (pattern matching happens in capture loop)
   - Delay: `tokio::time::sleep(duration)`
   - All wrapped in `tokio::time::timeout(timeout_s)`
   - Returns elapsed duration on success, error on timeout

7. **Implement `spawn_pre_app_sources()` Action**
   - In `crates/fdemon-app/src/actions/native_logs.rs`:
   - Filter `custom_sources` for `start_before_app == true`
   - For each: spawn `CustomLogCapture` (reuse existing `create_custom_log_capture`)
   - Start log forwarding tasks immediately (user sees stdout in real time during wait)
   - Send `CustomSourceStarted` messages to register handles
   - Collect readiness futures (from `ready_check` config)
   - `tokio::select!` on all readiness futures joining — all must complete or individually time out
   - Send progress messages during wait
   - On all ready (or timed out): send `Message::PreAppSourcesReady`
   - Individual timeouts log warnings via `PreAppSourceProgress` but do NOT block other sources

8. **Extend Daemon-Layer Custom Capture for Stdout Readiness**
   - In `crates/fdemon-daemon/src/native_logs/custom.rs`:
   - Add `ready_pattern: Option<String>` and `ready_tx: Option<oneshot::Sender<()>>` to `CustomSourceConfig` (daemon layer)
   - In `run_custom_capture()`: if `ready_pattern` is set, compile regex on entry, check each stdout line against it, fire `ready_tx.send(())` on first match
   - After match, clear the pattern (stop checking subsequent lines)
   - If the process exits before the pattern matches, drop `ready_tx` (receiver gets `RecvError`, treated as failure)

9. **Prevent Double-Spawning of Pre-App Sources**
   - Pre-app sources are stored in `custom_source_handles` (same vec as regular custom sources)
   - The existing guard at `handler/session.rs:312` (`!handle.custom_source_handles.is_empty()`) already prevents re-spawning on hot restart
   - In `spawn_native_log_capture()` (called on `AppStarted`): skip custom sources that have `start_before_app = true` — they're already running
   - Add `start_before_app` field to `CustomSourceHandle` for identification

10. **Configuration & Documentation**
    - Update `docs/CONFIGURATION.md` with `start_before_app` and `ready_check` reference
    - Update `docs/ARCHITECTURE.md` with pre-app source flow diagram
    - Add example config to `example/` projects

**Milestone**: Users can configure backend servers and companion processes as custom sources that start before the Flutter app, with automatic health checking. The Flutter launch is gated until dependencies are ready (or time out). Existing configurations without `start_before_app` are completely unaffected.

---

### Phase 2: Shared Custom Sources (`shared = true`)

**Goal**: Allow custom sources to be shared across sessions — spawned once, logs broadcast to all active sessions, shut down only on app quit. Prevents port conflicts and redundant processes when multiple Flutter sessions connect to the same backend.

#### Background

Phase 1 custom sources are per-session: each new Flutter session spawns its own copy of every configured custom source. This is correct for device-specific sources (logcat per device) but wrong for shared backend servers that multiple Flutter instances connect to. Launching 3 sessions results in 3 Python backend servers all fighting for port 8085.

The fix introduces `shared = true` on `CustomSourceConfig`. Shared sources are spawned once at the project level (on first session that needs them) and their logs are broadcast to all active sessions. They are shut down only on fdemon quit, not when individual sessions close.

#### Design Decisions

**Decision 1: Config Field Name — `shared`**

```toml
[[native_logs.custom_sources]]
name = "backend"
command = "python3"
args = ["server/server.py"]
start_before_app = true
shared = true    # ← only one instance, logs visible in all sessions
ready_check = { type = "http", url = "http://127.0.0.1:8085/health" }
```

`shared` is concise, intuitive, and mirrors Docker Compose's mental model. Alternatives considered: `single_instance` (verbose), `global` (implies system-wide scope), `per_project` (accurate but unusual). `shared = true` reads naturally: "this source is shared across sessions."

Default: `false` (backwards-compatible). `shared = true` is valid with or without `start_before_app`.

**Decision 2: Shared Source Handle Storage — `AppState` Level**

Shared source handles live on `AppState` in a new `shared_source_handles: Vec<SharedSourceHandle>` field, NOT on any `SessionHandle`. This follows the existing pattern of `device_cache`, `dap_status`, and other cross-session state that lives at the `AppState` level.

`SharedSourceHandle` is structurally identical to `CustomSourceHandle` but stored globally. It carries: `name`, `shutdown_tx`, `task_handle`, `start_before_app`.

**Decision 3: Log Routing — Broadcast to All Active Sessions**

Shared sources send logs via a new `Message::SharedSourceLog { event }` variant (no `session_id`). The TEA handler broadcasts the event to all active sessions in `session_manager`, applying per-session tag filtering. This avoids binding to a specific session at spawn time and ensures new sessions immediately see shared source logs.

Alternative considered: Send `NativeLog` with a sentinel session ID. Rejected because it leaks a "fake session" concept into the handler and breaks the existing `session_manager.get_mut(session_id)` lookup.

**Decision 4: Spawn Timing — On First Session That Needs Them**

Shared sources with `start_before_app = true` are spawned on the first session's launch (before the Flutter process). Subsequent sessions skip spawning (sources already running) and skip the ready check wait (already healthy).

Shared sources with `start_before_app = false` are spawned on the first `AppStarted` event, same as per-session post-app sources.

**Decision 5: Shutdown — Only on fdemon Quit**

Shared sources are NOT shut down when individual sessions close. `SessionHandle::shutdown_native_logs()` only affects per-session sources. Shared sources are shut down in `Engine::shutdown()` alongside the existing session cleanup.

**Decision 6: Pre-App Gating for Subsequent Sessions**

When a second session launches with `start_before_app` shared sources already running:
- The launch flow checks `state.shared_source_handles` — if the shared source is already tracked and its `shutdown_tx` is not closed, skip spawning
- Skip the ready check entirely — the source is already healthy
- Emit `PreAppSourcesReady` immediately for the shared sources (only wait for any non-shared pre-app sources)

#### Steps

1. **Add `shared` Field to `CustomSourceConfig`**
   - Add `shared: bool` (default: false) to `CustomSourceConfig` in `config/types.rs`
   - Add `has_shared_sources()` helper on `NativeLogsSettings`
   - Validation: `shared = true` with `ready_check` requires `start_before_app = true` (same existing rule)
   - Update `docs/CONFIGURATION.md`

2. **Add `SharedSourceHandle` and `AppState` Storage**
   - Define `SharedSourceHandle` in `session/handle.rs` (or a new `shared_sources.rs` module)
   - Add `shared_source_handles: Vec<SharedSourceHandle>` to `AppState`
   - Add `shutdown_shared_sources()` method for cleanup

3. **Add `SharedSourceLog` Message Variant**
   - New `Message::SharedSourceLog { event: NativeLogEvent }` variant
   - New `Message::SharedSourceStarted { name, shutdown_tx, task_handle, start_before_app }` variant
   - New `Message::SharedSourceStopped { name }` variant

4. **Modify `spawn_pre_app_sources` to Handle Shared Sources**
   - Accept shared source state (names already running) as parameter
   - Skip shared sources that are already tracked in `state.shared_source_handles`
   - For new shared sources: spawn normally but send `SharedSourceStarted` instead of `CustomSourceStarted`
   - Shared source forwarding tasks send `SharedSourceLog` instead of `NativeLog`

5. **Modify `spawn_custom_sources` to Handle Shared Sources**
   - Skip `shared = true` sources that are already in `state.shared_source_handles`
   - For new shared post-app sources: same pattern as step 4

6. **Handle `SharedSourceLog` in TEA Handler**
   - Broadcast to all active sessions: iterate `session_manager.iter_mut()`, apply per-session tag filter, queue log on each
   - Observe tag on each session's `native_tag_state`

7. **Handle `SharedSourceStarted`/`SharedSourceStopped` in TEA Handler**
   - Push/remove from `state.shared_source_handles`
   - On `SharedSourceStopped`: log a warning to all sessions

8. **Modify Pre-App Gating for Shared Sources**
   - In `handle_launch()` and `AutoLaunchResult`: check `state.shared_source_handles` for already-running shared pre-app sources
   - Only wait for readiness of non-running sources
   - Pass running shared source names to `spawn_pre_app_sources` to skip

9. **Modify `shutdown_native_logs()` to Skip Shared Sources**
   - `SessionHandle::shutdown_native_logs()` must not kill shared sources
   - Since shared sources live on `AppState` (not `SessionHandle`), this is automatic — no change needed to per-session shutdown

10. **Add Shared Source Cleanup to `Engine::shutdown()`**
    - In `Engine::shutdown()`: iterate `state.shared_source_handles`, send shutdown signal, abort tasks
    - This runs alongside the existing per-session `shutdown_native_logs()` calls

11. **Testing**
    - Test: shared source spawned once across two sessions
    - Test: shared source logs broadcast to all active sessions
    - Test: shared source survives individual session close
    - Test: shared source cleaned up on engine shutdown
    - Test: second session skips ready check for already-running shared source
    - Test: non-shared sources still per-session (regression)

12. **Documentation**
    - Update `docs/CONFIGURATION.md` with `shared` field reference and examples
    - Update `docs/ARCHITECTURE.md` with shared source data flow

**Milestone**: Users can configure shared backend servers that spawn once and serve all Flutter sessions. No more port conflicts or redundant processes when running multi-device sessions.

---

## Edge Cases & Risks

### Ready Check Never Succeeds
- **Risk:** The backend server fails to start or the health endpoint never returns 2xx, causing the timeout to be reached every time
- **Mitigation:** Timeout defaults to 30s, after which Flutter launches anyway with a warning. The custom source's stderr/stdout is visible in the log view, helping the user diagnose the issue. Users can adjust `timeout_s` in config.

### Command Check Tool Not Installed
- **Risk:** The `command` ready check references a tool not on PATH (e.g., `grpcurl`, `pg_isready`)
- **Mitigation:** The command check loop handles spawn failures the same as non-zero exits — it logs a debug-level message and retries until timeout. On timeout, the warning message includes the command name to help the user diagnose. Validation does NOT check tool availability at config parse time (the tool may be in a non-standard location or installed later).

### Custom Source Process Exits Before Ready
- **Risk:** The command fails immediately (e.g., compile error in `cargo run -p server`), and the readiness check polls forever until timeout
- **Mitigation:** For `stdout` checks: dropping `ready_tx` signals failure immediately. For `http`/`tcp` checks: the check loop naturally fails on each poll and eventually times out. Additionally, the `CustomSourceStopped` message will fire, and we can detect this in the pre-app action to short-circuit the readiness wait.

### Multiple Pre-App Sources with Different Readiness Times
- **Risk:** One source is ready in 2s, another takes 25s. Flutter launch is gated on the slowest.
- **Mitigation:** This is by design — the Flutter app shouldn't launch until ALL dependencies are ready. Progress messages show which source is still pending. Each source has its own independent timeout.

### Hot Restart with Pre-App Sources Running
- **Risk:** `AppStarted` fires again on hot restart, potentially triggering duplicate custom source spawns
- **Mitigation:** The existing `!handle.custom_source_handles.is_empty()` guard prevents this. Pre-app sources are tracked in `custom_source_handles` and persist across hot restarts. The `spawn_native_log_capture()` function (called on `AppStarted`) will also skip `start_before_app = true` sources.

### Port Conflicts
- **Risk:** The custom source process (e.g., server) binds to a port that's already in use from a previous fdemon session
- **Mitigation:** This is the user's responsibility (same as running the server manually). The process's stderr will show the bind error, visible in fdemon's log view. fdemon cleans up custom sources on session close, so stale processes shouldn't persist.

### Session Close During Readiness Wait
- **Risk:** User quits fdemon or closes the session while pre-app sources are still waiting for readiness
- **Mitigation:** The `spawn_pre_app_sources()` task must respect session shutdown. Use `tokio::select!` with a shutdown signal alongside the readiness check. On shutdown, abort readiness checks and clean up spawned processes.

### `start_before_app` Source with `auto_start = false`
- **Risk:** If `auto_start` is false, the user goes through the device selector dialog. Pre-app sources should still start before the Flutter app in this case.
- **Mitigation:** Pre-app source logic is triggered from `handle_launch()`, which is the final step of both auto-launch and manual device selection. Works correctly in both paths.

### Concurrent Session Launches
- **Risk:** User launches two sessions — each has pre-app sources. If they share the same backend (e.g., same port), only one should start it.
- **Mitigation:** Out of scope for Phase 1. Custom sources are per-session. Phase 2 adds `shared = true` to handle this case properly.

### Shared Source Crash (Phase 2)
- **Risk:** A shared source crashes while multiple sessions are active. All sessions lose their shared backend.
- **Mitigation:** `SharedSourceStopped` logs a warning to all sessions. The user can see the source's stderr to diagnose. Auto-restart is deferred to a future enhancement.

### Race: Two Sessions Launch Simultaneously with Shared Pre-App Sources (Phase 2)
- **Risk:** Two `SpawnPreAppSources` actions fire in rapid succession; both try to spawn the same shared source.
- **Mitigation:** The TEA loop is single-threaded — messages are processed sequentially. The first `SharedSourceStarted` registers the handle on `AppState` before the second `spawn_pre_app_sources` runs. The second launch sees the source already tracked and skips spawning.

### Shared Source With No Active Sessions (Phase 2)
- **Risk:** All sessions are closed but fdemon is still running. Shared source keeps running with no session to display its logs.
- **Mitigation:** This is by design — the shared source persists until fdemon quits. Logs from the source are silently dropped (no active sessions in `session_manager.iter_mut()`). When a new session starts, it immediately begins receiving logs again.

---

## Configuration Additions

```toml
# .fdemon/config.toml

[native_logs]
enabled = true

# Backend REST API — starts before Flutter app, waits for health check
[[native_logs.custom_sources]]
name = "server"
command = "cargo"
args = ["run", "-p", "server"]
format = "raw"
working_dir = "/path/to/project"
start_before_app = true
ready_check = { type = "http", url = "http://localhost:8080/health", interval_ms = 500, timeout_s = 30 }

# gRPC server — uses grpcurl command to check standard health protocol
[[native_logs.custom_sources]]
name = "grpc-server"
command = "cargo"
args = ["run", "-p", "server"]
format = "raw"
working_dir = "/path/to/project"
start_before_app = true
ready_check = { type = "command", command = "grpcurl", args = ["-plaintext", "localhost:50051", "grpc.health.v1.Health/Check"], timeout_s = 60 }

# Node.js API — TCP port check (no health endpoint)
[[native_logs.custom_sources]]
name = "api"
command = "npm"
args = ["run", "dev"]
format = "json"
working_dir = "/path/to/node-project"
start_before_app = true
ready_check = { type = "tcp", host = "localhost", port = 3000 }

# Database — uses pg_isready to check PostgreSQL readiness
[[native_logs.custom_sources]]
name = "db"
command = "docker"
args = ["compose", "up", "postgres"]
format = "raw"
start_before_app = true
ready_check = { type = "command", command = "pg_isready", args = ["-h", "localhost", "-p", "5432"], interval_ms = 1000, timeout_s = 30 }

# Process that prints "ready" to stdout
[[native_logs.custom_sources]]
name = "worker"
command = "python"
args = ["worker.py"]
format = "raw"
start_before_app = true
ready_check = { type = "stdout", pattern = "Worker ready|Listening on" }

# Fire-and-forget pre-app process (no readiness check)
[[native_logs.custom_sources]]
name = "cache-warmer"
command = "bash"
args = ["warm-cache.sh"]
format = "raw"
start_before_app = true

# Simple delay-based readiness
[[native_logs.custom_sources]]
name = "slow-service"
command = "java"
args = ["-jar", "service.jar"]
format = "raw"
start_before_app = true
ready_check = { type = "delay", seconds = 5 }

# Regular custom source (unchanged behavior — starts after AppStarted)
[[native_logs.custom_sources]]
name = "log-watcher"
command = "tail"
args = ["-f", "/tmp/app.log"]
format = "raw"
```

### Ready Check Reference

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `type` | string | (required) | One of: `http`, `tcp`, `command`, `stdout`, `delay` |
| `url` | string | — | HTTP: full URL to GET (e.g., `http://localhost:8080/health`) |
| `host` | string | — | TCP: hostname to connect to |
| `port` | integer | — | TCP: port number to connect to |
| `command` | string | — | Command: executable to run (e.g., `grpcurl`, `pg_isready`) |
| `args` | string[] | `[]` | Command: arguments to pass to the executable |
| `pattern` | string | — | Stdout: regex pattern to match against stdout lines |
| `seconds` | integer | — | Delay: seconds to wait |
| `interval_ms` | integer | 500 | HTTP/TCP/Command: milliseconds between poll attempts |
| `timeout_s` | integer | 30 | HTTP/TCP/Command/Stdout: seconds before giving up and proceeding |

---

## Success Criteria

### Phase 1 Complete When:
- [ ] `start_before_app = true` causes custom source to spawn before Flutter app launch
- [ ] `ready_check` with `type = "http"` polls a URL and gates Flutter launch on 2xx response
- [ ] `ready_check` with `type = "tcp"` polls a host:port and gates Flutter launch on successful connection
- [ ] `ready_check` with `type = "command"` runs an external command and gates Flutter launch on exit code 0
- [ ] `ready_check` with `type = "stdout"` watches stdout for regex match and gates Flutter launch
- [ ] `ready_check` with `type = "delay"` waits a fixed duration before Flutter launch
- [ ] Timeout causes Flutter launch to proceed with a warning (not block indefinitely)
- [ ] Pre-app source stdout is visible in the log view during the readiness wait
- [ ] Progress messages show which sources are pending and readiness timing
- [ ] Hot restart does NOT re-spawn pre-app sources
- [ ] Session close during readiness wait cleans up properly (no orphaned processes)
- [ ] Configurations without `start_before_app` are completely unaffected (zero behavioral change)
- [ ] `ready_check` without `start_before_app = true` is a validation error
- [ ] Pre-app sources appear in tag filter UI (`T` key overlay)
- [ ] All new code has unit tests
- [ ] No regressions in existing custom source or native log pipeline
- [ ] `cargo fmt && cargo check && cargo test && cargo clippy -- -D warnings` passes
- [ ] `docs/CONFIGURATION.md` updated with `start_before_app` and `ready_check` reference
- [ ] `docs/ARCHITECTURE.md` updated with pre-app source flow

### Phase 2 Complete When:
- [ ] `shared = true` causes custom source to spawn once across all sessions
- [ ] Shared source logs are broadcast to all active sessions
- [ ] Shared source survives individual session close (only shut down on fdemon quit)
- [ ] Second session launch skips ready check for already-running shared sources
- [ ] Non-shared sources remain per-session (no regression)
- [ ] `Engine::shutdown()` cleans up shared sources
- [ ] `docs/CONFIGURATION.md` updated with `shared` field reference
- [ ] All new code has unit tests
- [ ] `cargo fmt && cargo check && cargo test && cargo clippy -- -D warnings` passes

---

## Future Enhancements
- **HTTPS health checks**: Add TLS support for health check endpoints (via `rustls` or `native-tls`). Low priority since localhost checks are almost always HTTP.
- **Restart-on-crash for pre-app sources**: Auto-restart a pre-app source if it exits unexpectedly, with backoff. Currently, the process stays dead and the user sees a warning.
- **Readiness check composition**: AND/OR logic for multiple readiness conditions (e.g., "HTTP health check AND stdout pattern"). Currently each source has at most one check.
- **Pre-app source dependency ordering**: Source A must be ready before source B starts. Currently all pre-app sources start simultaneously.
