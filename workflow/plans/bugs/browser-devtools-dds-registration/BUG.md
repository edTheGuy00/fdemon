# Bugfix Plan: Browser DevTools Returns "No DevTools Instance Registered with DDS"

## TL;DR

Pressing `B` in DevTools mode opens `http://<DDS-host>/devtools/?uri=<ws_uri>`, which works on pre-Flutter-3.16 DDS that bundled the DevTools web app, but fails on newer Flutter where DevTools must be **separately registered with DDS via the Flutter daemon's `daemon.devtools.serve` JSON-RPC method**. fdemon never calls that method and never parses the `daemon.devtools` event that the daemon emits in response. The fix: add the daemon command + event parsing, eagerly serve DevTools when a VM Service comes up, and open the served URL instead of the self-constructed one. Verify the exact daemon RPC contract via external research before implementing.

## Bug Reports

### Bug 1: GitHub issue #42 — `B` opens DDS endpoint that has no DevTools registered

**Symptom:** User presses `B` while in DevTools mode. Browser opens to a page showing the literal text `"No DevTools instance is registered with the Dart Development Service (DDS)."` instead of the Flutter DevTools web app.

**Expected:** Browser opens to the Flutter DevTools web app, connected to the running session's VM Service URI.

**Root Cause Analysis:**

1. **`build_local_devtools_url`** at `crates/fdemon-app/src/handler/devtools/mod.rs:443-452` constructs `http://<host>:<port>/<auth-token>=/devtools/?uri=<encoded ws_uri>`. This URL targets the **DDS HTTP server's `/devtools/` path**, which on pre-Flutter-3.16 was auto-served by DDS but on newer Flutter is empty unless the flutter tool registers a DevTools instance with DDS via `DevToolsService.registerDevToolsServer`.
2. fdemon never calls **`daemon.devtools.serve`** — the JSON-RPC method that triggers the flutter tool to start a DevTools server and register it with DDS. `DaemonCommand` enum at `crates/fdemon-daemon/src/commands.rs:180-201` has no such variant.
3. fdemon never parses the **`daemon.devtools`** event — the daemon-emitted event carrying `host` + `port` of the served DevTools instance. Protocol parser `crates/fdemon-daemon/src/protocol.rs:115-151` routes it to the `_ => unknown_event(...)` catch-all.

**Affected Files (research evidence):**

- `crates/fdemon-app/src/handler/keys.rs:471` — `b` key dispatches `Message::OpenBrowserDevTools`.
- `crates/fdemon-app/src/handler/devtools/mod.rs:385-401` — `handle_open_browser_devtools` reads `session.ws_uri` and calls `build_local_devtools_url`.
- `crates/fdemon-app/src/handler/devtools/mod.rs:443-452` — URL construction.
- `crates/fdemon-app/src/handler/devtools/mod.rs:720-744` — existing tests assert the broken URL shape.
- `crates/fdemon-daemon/src/commands.rs:180-201` — `DaemonCommand` enum (no `ServeDevTools` variant).
- `crates/fdemon-core/src/events.rs` — `DaemonMessage` enum (no `DevToolsServed` variant).
- `crates/fdemon-daemon/src/protocol.rs:115-151` — event router (no `daemon.devtools` branch).
- `crates/fdemon-app/src/session/session.rs` — `Session` struct (no place to store served DevTools URL).
- `crates/fdemon-app/src/handler/session.rs` — `app.debugPort` / VM Service ready handlers (would trigger eager `ServeDevTools`).

---

## Affected Modules

- `crates/fdemon-daemon/src/commands.rs` — Add `DaemonCommand::ServeDevTools` variant.
- `crates/fdemon-core/src/events.rs` — Add `DaemonMessage::DevToolsServed { host, port }` variant.
- `crates/fdemon-daemon/src/protocol.rs` — Parse `daemon.devtools` event.
- `crates/fdemon-app/src/session/session.rs` — Add `devtools_url: Option<String>` (or `(host, port)`) to `Session`.
- `crates/fdemon-app/src/handler/session.rs` — On VM Service ready, fire `ServeDevTools`.
- `crates/fdemon-app/src/handler/devtools/mod.rs` — On `OpenBrowserDevTools`, prefer `session.devtools_url` over the self-constructed URL; fall back gracefully.
- `crates/fdemon-app/src/handler/update.rs` — Route the new `DevToolsServed` message into the session.

---

## Phases

### Phase 0: External Research (prerequisite) - Critical

Verify the exact Flutter daemon RPC contract before implementing.

**Steps:**
1. Use the `external_researcher` agent to look up the Flutter daemon JSON-RPC method for DevTools serving across SDK versions (Flutter 3.13, 3.16, 3.19, 3.22+).
2. Confirm:
   - The method name (`daemon.devtools.serve` vs `daemon.serveDevTools` vs `daemon.devtools.show`).
   - The request `params` shape (likely empty, but verify).
   - The response result shape (`{ host: String, port: u16, pid?: u32 }`).
   - The `daemon.devtools` event params shape (likely matches the response).
   - The minimum Flutter SDK version that supports this method.
   - The behavior on older SDKs (`-32601 Method not found` — confirm).
3. Document findings in `workflow/plans/bugs/browser-devtools-dds-registration/RESEARCH.md`.

**Measurable Outcomes:**
- Implementor has a verified RPC contract before touching code.
- Fallback strategy is documented for SDKs that don't support the method.

### Phase 1: Daemon Command + Event Plumbing - Critical

Wire `devtools.serve` request and `daemon.devtools` event end-to-end through the daemon-side stack.

**Steps:**
1. Add `DaemonCommand::ServeDevTools { request_id: Option<String> }` to `commands.rs:180-201`. Use `RequestTracker` to correlate the request/response if needed.
2. Add serialization in `DaemonCommand::serialize` (or equivalent) emitting `{"method":"daemon.devtools.serve","params":{}}` to the daemon stdin.
3. Add `DaemonMessage::DevToolsServed { host: String, port: u16 }` to `crates/fdemon-core/src/events.rs`.
4. In `crates/fdemon-daemon/src/protocol.rs:115-151`, add an arm for `event == "daemon.devtools"` that parses `host` (string) + `port` (u16) into `DevToolsServed`.
5. Also handle the **response** to the `daemon.devtools.serve` request — likely a JSON-RPC result with the same `{host, port}` shape. Whichever arrives first (event or response) populates the session; if both arrive, deduplicate.

**Measurable Outcomes:**
- New unit tests in `commands.rs` confirm serialization shape.
- New unit tests in `protocol.rs` parse a sample `daemon.devtools` event from the Flutter daemon (use fixtures from the research phase).
- A response handler maps `-32601 Method not found` to a `DaemonMessage::DevToolsServeFailed { reason }` variant for fallback handling.

### Phase 2: Session State + Eager Serve - Critical

Store the served DevTools URL on `Session` and fire the serve command eagerly when a session is ready.

**Steps:**
1. Add `devtools_url: Option<String>` to `Session` (or `devtools_endpoint: Option<(String, u16)>`).
2. In `handler/session.rs`, when the session reaches `VmServiceReady` (or `AppStarted`, whichever fires first with a valid `ws_uri`), emit `UpdateAction::SendDaemonCommand(DaemonCommand::ServeDevTools { .. })`.
3. Handle `DaemonMessage::DevToolsServed` (and the response variant) → populate `session.devtools_url` via a new `Message::DevToolsServed { session_id, url }`.
4. Handle `DaemonMessage::DevToolsServeFailed` → log `warn!` and leave `devtools_url` as `None`.

**Measurable Outcomes:**
- New unit tests on `handler/session.rs` verify the eager `ServeDevTools` dispatch.
- New unit tests on the `DevToolsServed` handler populate `session.devtools_url`.

### Phase 3: Browser-Open Logic + Fallback - Critical

Use the served URL when available; fall back to the legacy URL with a clear toast when not.

**Steps:**
1. In `handle_open_browser_devtools` (`handler/devtools/mod.rs:385-401`):
   - If `session.devtools_url.is_some()`, build `<served_url>/?uri=<encoded ws_uri>` (or append `/inspector` if we want to deep-link to the active panel — verify research output).
   - If `None`, fall back to today's `build_local_devtools_url` but show a toast: "DevTools server not registered with DDS — using legacy URL (may fail on newer Flutter)."
2. If `devtools.serve` previously failed, the toast should say "Open `dart devtools` manually and paste this VM Service URI: `<ws_uri>`" as the recovery path.
3. Update existing tests at `mod.rs:720-744` to cover both URL shapes.

**Measurable Outcomes:**
- On modern Flutter, `B` opens the served DevTools URL and the app loads successfully.
- On older Flutter or when `devtools.serve` returns `-32601`, `B` opens the legacy URL **and** shows a recovery toast.

### Phase 4: Documentation + Verification - Minor

**Steps:**
1. Update `docs/ARCHITECTURE.md` "DevTools Subsystem" with the served-URL flow.
2. Update `docs/KEYBINDINGS.md` if behavior of `B` changes materially (it shouldn't — same key, better URL).
3. Add an entry to a "Known DevTools Quirks" subsection covering older Flutter behavior.
4. Manual verification: confirmed against a fresh `flutter create` on the user's target SDK version.

**Measurable Outcomes:**
- Docs accurately describe the new flow.
- Manual verification recorded in the task completion summary.

---

## Edge Cases & Risks

### Older Flutter SDK
- **Risk:** `daemon.devtools.serve` doesn't exist; daemon returns `-32601`.
- **Mitigation:** Phase 0 research must confirm minimum SDK; Phase 3 fallback handles missing method gracefully with a recovery toast.

### Serve request races browser open
- **Risk:** User presses `B` before the eager `ServeDevTools` response has arrived.
- **Mitigation:** If `session.devtools_url` is still pending and a request is in-flight (track via `pending_devtools_serve: bool` on session), defer opening for ≤ 2 s waiting for the response; if still pending, fall back to legacy URL with a toast.

### Multiple sessions sharing one DevTools server
- **Risk:** Each Flutter session may share a single served DevTools instance; we'd be serving once per session unnecessarily.
- **Mitigation:** Acceptable for now — DevTools server reuse is the flutter tool's responsibility. Just ensure we don't crash on duplicate event delivery.

### Web Flutter apps
- **Risk:** Web Flutter emits `app.webLaunchUrl` instead of the standard DevTools flow.
- **Mitigation:** Out of scope for this bug; document as a known limitation if research reveals it.

### DDS authentication tokens
- **Risk:** The DevTools server may need to be passed the DDS auth token in the URL.
- **Mitigation:** Verify in Phase 0 research; if so, ensure we forward the token portion of the ws_uri correctly.

---

## Task Dependency Graph

```
00-research-daemon-devtools-rpc          [Phase 0]
       │
       ▼
01-daemon-command-serve-devtools         [Phase 1]
02-daemon-message-devtools-served        [Phase 1, depends on 01 for shared types]
03-protocol-parse-daemon-devtools-event  [Phase 1, depends on 02]
       │
       ▼
04-session-stores-devtools-url           [Phase 2, depends on 03]
05-eager-serve-on-vmservice-ready        [Phase 2, depends on 04]
       │
       ▼
06-open-browser-uses-served-url          [Phase 3, depends on 05]
07-fallback-and-recovery-toast           [Phase 3, depends on 06]
       │
       ▼
08-update-keybindings-doc                [Phase 4]
09-update-architecture-doc               [Phase 4, doc_maintainer]
```

---

## Success Criteria

### Phase 0 Complete When:
- [ ] `RESEARCH.md` documents the exact method name, request/response/event shapes, and minimum SDK version.

### Phase 1 Complete When:
- [ ] `DaemonCommand::ServeDevTools` exists and serializes correctly.
- [ ] `DaemonMessage::DevToolsServed { host, port }` exists.
- [ ] `protocol.rs` parses `daemon.devtools` events correctly (unit-tested with fixtures).

### Phase 2 Complete When:
- [ ] `Session` carries `devtools_url`.
- [ ] On VM Service ready, fdemon eagerly fires `ServeDevTools`.
- [ ] On response, `session.devtools_url` is populated.

### Phase 3 Complete When:
- [ ] On modern Flutter, `B` opens the served URL and DevTools loads.
- [ ] On older Flutter (or method-not-found), `B` falls back with a clear recovery toast.

### Phase 4 Complete When:
- [ ] `docs/ARCHITECTURE.md` updated.
- [ ] Manual verification across two Flutter SDK versions recorded.

---

## Milestone Deliverable

Pressing `B` in DevTools mode reliably opens the Flutter DevTools web app, connected to the active session, on modern Flutter SDKs; older SDKs degrade gracefully with a clear, actionable error toast instead of a silent "registration" failure.
