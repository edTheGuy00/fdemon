# Action Items: Phase 1 — VM Service Client Foundation

**Review Date:** 2026-02-17
**Verdict:** NEEDS WORK
**Blocking Issues:** 2 critical + 4 high

## Critical Issues (Must Fix)

### 1. Signal VM shutdown before session removal
- **Source:** Logic Reasoning Checker
- **File:** `crates/fdemon-app/src/handler/session_lifecycle.rs` (handle_close_current_session)
- **Problem:** `remove_session()` drops `vm_shutdown_tx` without sending `true`, causing a busy-spin loop in `forward_vm_events` where `changed()` returns immediately but `borrow()` is `false`
- **Required Action:** Extract and signal `vm_shutdown_tx` before calling `remove_session`, matching the pattern in `handle_session_exited` and `handle_session_message_state`(AppStop)
- **Acceptance:** Test that closing a session with an active VM connection exits the forwarding task cleanly (no CPU spike)

### 2. Add `LogSource::VmService` to `LogSourceFilter::matches`
- **Source:** Logic Reasoning Checker
- **File:** `crates/fdemon-core/src/types.rs` (LogSourceFilter::matches)
- **Problem:** VM Service logs are invisible under any source filter except "All", defeating the feature's purpose
- **Required Action:** Include `LogSource::VmService` in the `Flutter` filter variant (since VM errors are Flutter errors), or add a dedicated filter option
- **Acceptance:** VM Service Flutter errors are visible when the "Flutter" source filter is active

## High-Severity Issues (Should Fix)

### 3. Replace `blocking_read()` with safe alternative
- **Source:** Architecture, Quality, Risks (all 3 agents)
- **File:** `crates/fdemon-daemon/src/vm_service/client.rs:210,226`
- **Problem:** `blocking_read()` on `tokio::sync::RwLock` can panic if write lock is held
- **Suggested Action:** Replace `tokio::sync::RwLock` with `std::sync::RwLock` for `ConnectionState` (lock never held across `.await`) or use `AtomicU8`

### 4. Add `Error::VmService` to `is_recoverable()`
- **Source:** Quality, Logic (2 agents)
- **File:** `crates/fdemon-core/src/error.rs`
- **Problem:** `is_recoverable()` returns `false` for VM Service errors, which are clearly recoverable
- **Suggested Action:** Add `Error::VmService(_)` to the `is_recoverable()` match arm; add test assertion

### 5. Guard against double VM Service connection
- **Source:** Logic Reasoning Checker
- **File:** `crates/fdemon-app/src/handler/session.rs:189` (maybe_connect_vm_service)
- **Problem:** No check for existing connection; duplicate `app.debugPort` spawns second connection
- **Suggested Action:** Add `!handle.session.vm_connected && handle.vm_shutdown_tx.is_none()` guard

### 6. Re-subscribe to streams after reconnection
- **Source:** Logic Reasoning Checker
- **File:** `crates/fdemon-daemon/src/vm_service/client.rs:386-401`
- **Problem:** After WebSocket reconnect, Extension/Logging stream subscriptions are lost; events stop silently
- **Suggested Action:** Re-subscribe in `run_client_task` after successful reconnection, or send a `VmServiceReconnected` message

## Major Issues (Should Fix)

### 7. Replace `Arc::try_unwrap` with direct Arc storage
- **Source:** Architecture, Logic, Risks (3 agents)
- **File:** `crates/fdemon-app/src/handler/update.rs:1131`
- **Problem:** Fragile assumption that Arc has exactly one reference; breaks if Message is cloned
- **Suggested Action:** Store `Arc<watch::Sender<bool>>` directly in `SessionHandle::vm_shutdown_tx` and call `send(true)` through the Arc

### 8. Track VM Service task JoinHandle
- **Source:** Risks & Tradeoffs Analyzer
- **File:** `crates/fdemon-app/src/actions.rs:458`
- **Problem:** Untracked `tokio::spawn` — panics swallowed, shutdown can't guarantee task termination
- **Suggested Action:** Add `vm_task_handle: Option<JoinHandle<()>>` to `SessionHandle`

### 9. Split `client.rs` (929 lines > 500-line limit)
- **Source:** Code Quality Inspector
- **File:** `crates/fdemon-daemon/src/vm_service/client.rs`
- **Suggested Action:** Extract background task logic into `client/task.rs` submodule

### 10. Wire up `cleanup_stale()` for request tracker
- **Source:** Risks & Tradeoffs Analyzer
- **File:** `crates/fdemon-daemon/src/vm_service/protocol.rs:306`
- **Suggested Action:** Add periodic timer in `run_io_loop()` (e.g., every 30s)

## Minor Issues (Consider Fixing)

### 11. Remove unnecessary `.clone()` on `response.id` in `handle_ws_text`
- `client.rs:516` — borrow restructure avoids allocation on hot path

### 12. Replace `println!` with `tracing::debug!` in doc example
- `vm_service/mod.rs:27` — violates stdout ownership rule

### 13. Remove `native-tls` feature from `tokio-tungstenite`
- `Cargo.toml:42` — VM Service is always localhost, TLS unnecessary; reduces binary size and build deps

### 14. Add `const DEDUP_SCAN_DEPTH: usize = 10`
- `update.rs:1256` — eliminate magic number

### 15. Add connection timeout to `spawn_vm_service_connection`
- `actions.rs:463` — wrap `VmServiceClient::connect` in `tokio::time::timeout(10s)`

### 16. Remove or use `discover_main_isolate()` result
- `actions.rs:462` — currently a wasted RPC; either store result or defer to Phase 2

### 17. Verify MSRV for `is_none_or` (requires Rust 1.82+)
- `protocol.rs:210` — project MSRV is 1.70+; replace with `.map_or(true, Value::is_null)` if needed

### 18. Rename `subscribe_phase1_streams` to `subscribe_flutter_streams`
- Decouple public API from planning terminology

## Re-review Checklist

After addressing issues, the following must pass:
- [ ] All critical issues (1-2) resolved
- [ ] All high-severity issues (3-6) resolved or justified
- [ ] `cargo fmt --all` passes
- [ ] `cargo check --workspace` passes
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace -- -D warnings` passes
- [ ] Session close with active VM connection does not leak tasks
- [ ] VM Service logs visible under Flutter source filter
- [ ] No `blocking_read()` on tokio RwLock
- [ ] `Error::VmService` is recoverable
