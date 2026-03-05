# Action Items: DAP Server Phase 3

**Review Date:** 2026-03-05
**Verdict:** NEEDS WORK
**Blocking Issues:** 3

---

## Critical Issues (Must Fix)

### 1. Wire real DebugBackend into TCP accept loop
- **Source:** All 4 agents (unanimous)
- **File:** `crates/fdemon-dap/src/server/mod.rs` (accept_loop, line 297)
- **Problem:** `DapClientSession::run()` hardcodes `NoopBackend`. No mechanism exists to pass a backend factory to the accept loop. All IDE attach requests fail.
- **Required Action:**
  1. Add a backend factory parameter to `accept_loop` (e.g., `Arc<dyn Fn() -> Option<VmServiceBackend> + Send + Sync>` or a shared `Arc<Mutex<Option<VmRequestHandle>>>`)
  2. Update `start()` and `DapService::start_tcp()` signatures to accept the factory
  3. In the accept loop, when a connection arrives, try to obtain a backend; if available, call `run_on_with_backend`, otherwise fall back to `run_on` with `NoopBackend`
  4. Update Engine's `SpawnDapServer` action to pass the factory/handle
- **Acceptance:** IDE connects via TCP → `attach` succeeds against running Flutter session → `threads` returns isolates

### 2. Fix stdio mode: either wire Engine or update docs
- **Source:** Logic Checker, Risks Analyzer
- **File:** `crates/fdemon-dap/src/transport/stdio.rs:96`, `src/dap_stdio/runner.rs`
- **Problem:** Stdio mode uses `NoopBackend` and does not start an Engine. IDE_SETUP.md presents it as functional.
- **Required Action (pick one):**
  - **Option A:** Wire stdio runner to accept a `VmRequestHandle` (or URI) from IDE launch args and construct a `VmServiceBackend`
  - **Option B:** Update `docs/IDE_SETUP.md` with a prominent disclaimer that stdio mode is transport-only, not yet connected to real debugging
- **Acceptance:** Either stdio debugging works end-to-end, or docs honestly reflect limitations

### 3. Fix busy-poll in stdio broadcast receiver
- **Source:** Logic Checker
- **File:** `crates/fdemon-dap/src/transport/stdio.rs:95`, `crates/fdemon-dap/src/server/session.rs:425-446`
- **Problem:** Dummy broadcast sender is immediately dropped. `recv()` returns `Closed` on every poll, causing hot spin in the select loop.
- **Required Action:** Either:
  - Make `log_event_rx` an `Option` in `run_on` and skip polling when `None`
  - Add a dedicated `run_on` variant for stdio without the broadcast parameter
  - Use a flag to stop polling after first `Closed`
- **Acceptance:** No CPU spin in stdio sessions; verified with a tracing assertion that "broadcast channel closed" is logged at most once

---

## Major Issues (Should Fix)

### 4. Consolidate `run_on` and `run_on_with_backend`
- **Source:** Code Quality, Risks Analyzer
- **File:** `crates/fdemon-dap/src/server/session.rs`
- **Problem:** ~80 lines of duplicated select loop logic
- **Suggested Action:** Extract shared logic into a common `run_inner` or use a generic event source trait/enum
- **Acceptance:** Single implementation of the DAP session event loop

### 5. Emit `terminated` event on client-initiated disconnect
- **Source:** Logic Checker, Risks Analyzer
- **File:** `crates/fdemon-dap/src/server/session.rs:622-626`
- **Problem:** Session's `handle_disconnect` transitions state but does not send `terminated` event. Adapter's `handle_disconnect` does but is unreachable (dead code).
- **Suggested Action:** Add `terminated` event emission in session's `handle_disconnect`, remove dead `disconnect` arm from adapter
- **Acceptance:** DAP client receives `terminated` event before disconnect response

### 6. Replace `eprintln!` with `tracing::info!`
- **Source:** Architecture Enforcer, Code Quality, Risks Analyzer
- **File:** `crates/fdemon-app/src/actions/mod.rs:451-458`
- **Suggested Action:** Use `tracing::info!` with a stderr subscriber for headless/TCP modes
- **Acceptance:** No `eprintln!` calls in library code

### 7. Fix `dart_uri_to_path` for Windows
- **Source:** Code Quality
- **File:** `crates/fdemon-dap/src/adapter/stack.rs:324`
- **Suggested Action:** Use `url::Url::parse().to_file_path()` or document Unix-only assumption with tests
- **Acceptance:** Function handles `file:///C:/path` correctly or has explicit platform guard

---

## Minor Issues (Consider Fixing)

### 8. Change `pub mod dap_backend` to `pub(crate) mod dap_backend`
- **File:** `crates/fdemon-app/src/handler/mod.rs:21`

### 9. Remove stale `#[allow(dead_code)]` from adapter `backend` field
- **File:** `crates/fdemon-dap/src/adapter/mod.rs:295-296`

### 10. Log serialization errors instead of `unwrap_or_default()`
- **Files:** `session.rs:587`, `evaluate.rs:132`

### 11. Use typed errors in `DebugBackend` trait instead of `String`
- **File:** `crates/fdemon-dap/src/adapter/mod.rs:56-125`

### 12. Use typed enum for `set_exception_pause_mode` parameter
- **File:** `crates/fdemon-dap/src/adapter/mod.rs`, `crates/fdemon-app/src/handler/dap_backend.rs`

### 13. Extract magic numbers into named constants
- **Files:** `server/mod.rs:331` (backoff), `actions/mod.rs` (channel capacity)

### 14. Remove empty `transport/tcp.rs` re-export module
- **File:** `crates/fdemon-dap/src/transport/tcp.rs`

### 15. Add security warning when binding to non-loopback
- **File:** `crates/fdemon-dap/src/server/mod.rs`

---

## Re-review Checklist

After addressing issues, the following must pass:
- [ ] All 3 critical issues resolved
- [ ] All major issues resolved or justified
- [ ] `cargo build --workspace` succeeds
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace` passes
- [ ] `cargo fmt --all` produces no changes
- [ ] IDE can connect via TCP and complete `attach` against a running Flutter session
- [ ] Stdio mode either works end-to-end or docs are updated
- [ ] No `eprintln!` in library code
