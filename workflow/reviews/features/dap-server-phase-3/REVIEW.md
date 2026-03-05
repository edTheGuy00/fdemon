# Review: DAP Server Phase 3

**Date:** 2026-03-05
**Branch:** `feat/dap-server`
**Reviewers:** Architecture Enforcer, Logic & Reasoning Checker, Code Quality Inspector, Risks & Tradeoffs Analyzer
**Change Size:** +2,466 / -261 lines across 25 modified files + 13 new files

---

## Overall Verdict: NEEDS WORK

| Agent | Verdict |
|-------|---------|
| Architecture Enforcer | APPROVED WITH CONCERNS |
| Logic & Reasoning Checker | NEEDS WORK |
| Code Quality Inspector | NEEDS WORK |
| Risks & Tradeoffs Analyzer | NEEDS WORK |

**Consolidated:** 3 of 4 agents returned NEEDS WORK or equivalent. One critical blocking issue identified by all 4 agents independently.

---

## Executive Summary

Phase 3 delivers a well-structured DAP adapter layer: `DebugBackend` trait abstraction, `DapAdapter` with thread/breakpoint/stack/variable/evaluate management, stdio transport, expanded protocol types, and a session state machine generic over the backend. The architectural foundations are **sound** -- layer boundaries are correctly observed, `fdemon-dap` depends only on `fdemon-core`, and the VM Service bridge lives in `fdemon-app` where it belongs.

However, **the entire DAP feature is non-functional for real debugging**. The TCP accept loop and stdio transport both hardcode `NoopBackend`, so every `attach` request fails with "NoopBackend: no VM Service connected". The `VmServiceBackend` implementation exists but is never instantiated at runtime. The `run_on_with_backend` entry point exists but is never called from any production code path. IDE_SETUP.md presents the feature as working when it is not.

---

## Critical Issues (Must Fix)

### 1. TCP accept loop hardcodes NoopBackend -- all attach requests fail

- **Source:** All 4 agents (unanimous)
- **File:** `crates/fdemon-dap/src/server/mod.rs:297`
- **Problem:** The accept loop calls `DapClientSession::run(stream, session_shutdown, log_event_rx)` which resolves to `DapClientSession<NoopBackend>::run()`. There is no parameter in the `accept_loop` function signature to accept a backend factory. The `run_on_with_backend` method exists but is unreachable from any production code path. Every IDE connecting via TCP gets a session where `attach` fails with "NoopBackend: no VM Service connected".
- **Required Action:** Extend the `accept_loop` signature to accept a backend factory (e.g., `Arc<dyn Fn() -> impl DebugBackend>`) or a shared `VmRequestHandle` retrieval mechanism. Use `run_on_with_backend` instead of `run` for accepted connections when a backend is available.
- **Acceptance:** An IDE can connect via TCP, complete the `attach` handshake against a running Flutter session, and receive thread/breakpoint/stepping responses.

### 2. Stdio mode also uses NoopBackend -- equally non-functional

- **Source:** Logic Checker, Risks Analyzer
- **File:** `crates/fdemon-dap/src/transport/stdio.rs:96`
- **Problem:** `run_stdio_session` calls `DapClientSession::run_on(reader, writer, shutdown_rx, log_event_rx)` with `NoopBackend`. The stdio runner (`src/dap_stdio/runner.rs`) does not start an Engine or Flutter process. The module doc explicitly says "Does not route `attach` commands to the Dart VM Service." Yet IDE_SETUP.md presents stdio as the primary transport for Zed/Helix.
- **Required Action:** Either wire the stdio runner to launch an Engine and provide a real backend, or update IDE_SETUP.md to clearly state that stdio mode is transport-only and not connected to a real debug session.
- **Acceptance:** Users following IDE_SETUP.md docs can successfully debug, or the docs are honest about current limitations.

### 3. Stdio mode creates a dummy broadcast channel causing potential busy-poll

- **Source:** Logic Checker
- **File:** `crates/fdemon-dap/src/transport/stdio.rs:95`
- **Problem:** A broadcast channel `(_, log_event_rx) = tokio::sync::broadcast::channel(1)` is created with the sender immediately dropped. In the `run_on` select loop, `log_event_rx.recv()` returns `Err(RecvError::Closed)` immediately on every poll, creating a hot spin between the closed-channel branch and the read branch. The handler logs at debug level and continues.
- **Required Action:** Convert the broadcast receiver to `Option` and set to `None` on `Closed`, use `fuse()`, or provide a dedicated `run_on` variant for stdio that omits the broadcast receiver.
- **Acceptance:** No busy-polling in the stdio session select loop.

---

## Major Issues (Should Fix)

### 4. Code duplication: `run_on` vs `run_on_with_backend` (~80 lines identical)

- **Source:** Code Quality, Risks Analyzer
- **File:** `crates/fdemon-dap/src/server/session.rs:260-461`
- **Problem:** Both methods contain nearly identical `tokio::select!` loops. Only difference: the event source type (`mpsc::Receiver` vs `broadcast::Receiver`). Any bug fix to the event loop must be applied in two places.
- **Suggested Action:** Extract the common select loop into a single method with a generic event source abstraction. At minimum, add `// SYNC WITH run_on_with_backend` comments.

### 5. Adapter `handle_disconnect` is dead code

- **Source:** Logic Checker, Risks Analyzer
- **File:** `crates/fdemon-dap/src/adapter/mod.rs:406` and `crates/fdemon-dap/src/server/session.rs:518`
- **Problem:** The session's `handle_request` intercepts `"disconnect"` before the wildcard branch that delegates to the adapter. The adapter's `handle_disconnect` (which sends a `terminated` event) is never called. Consequence: no `terminated` event is sent on client-initiated disconnect (only on server shutdown).
- **Suggested Action:** Either emit the `terminated` event from the session's `handle_disconnect`, or let `disconnect` fall through to the adapter. Remove the dead `disconnect` arm from the adapter or mark it clearly.

### 6. `eprintln!` violates CODE_STANDARDS.md

- **Source:** Architecture Enforcer, Code Quality, Risks Analyzer
- **File:** `crates/fdemon-app/src/actions/mod.rs:451-458`
- **Problem:** Five `eprintln!` calls for DAP connection info. CODE_STANDARDS.md: "NEVER use `println!` or `eprintln!`". The inline comment argues stderr is safe, but this sets a precedent.
- **Suggested Action:** Replace with `tracing::info!`. Configure a stderr-targeted subscriber for headless/TCP modes if terminal visibility is needed.

### 7. `dart_uri_to_path` produces wrong paths on Windows

- **Source:** Code Quality
- **File:** `crates/fdemon-dap/src/adapter/stack.rs:324`
- **Problem:** Strips `"file://"` (two slashes) which works on Unix by accident (`file:///path` → `/path`) but on Windows `file:///C:/path` → `/C:/path` (invalid). Tests only cover Unix paths.
- **Suggested Action:** Use `url::Url::parse().to_file_path()`, or add an explicit comment about Unix-only assumption and a test for Windows-style paths.

---

## Minor Issues (Consider Fixing)

### 8. `pub mod dap_backend` should be `pub(crate)`
- **File:** `crates/fdemon-app/src/handler/mod.rs:21`
- **Problem:** Exposes `VmServiceBackend` as part of `fdemon-app`'s public API. All other handler submodules are `pub(crate)`.

### 9. `#[allow(dead_code)]` on `backend` field is stale
- **File:** `crates/fdemon-dap/src/adapter/mod.rs:295-296`
- **Problem:** The `backend` field IS used by handlers (handle_attach, handle_continue, etc.). The annotation appears to be left over from an earlier phase.

### 10. `unwrap_or_default()` silently swallows serialization errors
- **Files:** `session.rs:587`, `evaluate.rs:132`
- **Problem:** `serde_json::to_value(...).unwrap_or_default()` silently sends `{}` if serialization fails. Should log the error.

### 11. `DebugBackend` uses `Result<_, String>` instead of typed errors
- **File:** `crates/fdemon-dap/src/adapter/mod.rs:56-125`
- **Problem:** All trait methods return `Result<_, String>`. Project standards require typed errors via `fdemon-core::Error` enum.

### 12. `set_exception_pause_mode` is stringly-typed
- **File:** Trait definition in `adapter/mod.rs`, impl in `dap_backend.rs`
- **Problem:** Takes `mode: &str` then matches to enum internally. Should accept a typed enum.

### 13. Magic numbers without constants
- **Files:** `server/mod.rs:331` (100ms backoff), `actions/mod.rs` (channel capacity 32)
- **Problem:** Per CODE_STANDARDS.md, magic numbers should be named constants.

### 14. `transport/tcp.rs` is a one-line re-export
- **File:** `crates/fdemon-dap/src/transport/tcp.rs`
- **Problem:** Contains only `pub use crate::server::start as start_server;`. Adds no value.

### 15. No security warning when DAP binds to non-loopback
- **File:** `crates/fdemon-dap/src/server/mod.rs`
- **Problem:** The `evaluate` DAP command allows arbitrary code execution. Binding to `0.0.0.0` exposes this to the network with no warning.

---

## Strengths

- **Layer boundaries are correct.** `fdemon-dap` depends only on `fdemon-core`. No circular dependencies. Cargo enforces this at compile time.
- **DebugBackend trait design is sound.** Defined in `fdemon-dap`, implemented in `fdemon-app` -- the only correct place for the bridge.
- **TEA compliance is maintained.** DAP events flow through `Message` variants. State mutations go through `handler::update()`. Side effects via `UpdateAction`.
- **Session state machine is rigorous.** Out-of-order request validation, proper state transitions, error responses for invalid sequences.
- **Test coverage is excellent.** Unit tests cover the state machine, adapter handlers, protocol types, and transport. MockBackend enables thorough adapter testing.
- **Documentation quality is high.** Module headers, public API docs, decision rationale, and doc comments are thorough.
- **Variable/frame reference management is correct.** `on_resume()` properly invalidates per-stop state. Reference stores are reset on continue/step.

---

## Requirement Verification

| Phase 3 Acceptance Criteria | Status |
|----------------------------|--------|
| DAP client can connect via TCP or stdio | PARTIAL -- connects but cannot debug |
| Full initialization handshake (initialize → configurationDone → attach) | FAIL -- attach always fails with NoopBackend |
| Breakpoints can be set by file URI and line | FAIL |
| Exception breakpoints work | FAIL |
| Stepping (over/into/out) works | FAIL |
| Stack traces display correctly | FAIL |
| Variables can be inspected | FAIL |
| Expression evaluation works | FAIL |
| Log output in debug console | PARTIAL -- TCP broadcast works, stdio disconnected |
| Zed/Helix can debug via config | FAIL |
| All new code has unit tests | PASS |
| Existing tests pass | PASS |
| clippy passes | PASS |

---

## Action Required

See `ACTION_ITEMS.md` for the prioritized fix list.

**Blocking count:** 3 critical issues must be resolved before Phase 3 can be considered complete.
