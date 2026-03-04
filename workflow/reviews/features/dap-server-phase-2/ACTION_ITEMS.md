# Action Items: DAP Server Phase 2

**Review Date:** 2026-03-04
**Verdict:** APPROVED WITH CONCERNS
**Blocking Issues:** 0 (but 4 recommended pre-merge fixes)

## Pre-Merge Fixes (Should Fix)

### 1. Add Header Line Length Limit in Codec
- **Source:** Risks & Tradeoffs Analyzer
- **File:** `crates/fdemon-dap/src/protocol/codec.rs:60`
- **Problem:** `reader.read_line()` has no limit on header line length. Unbounded allocation risk.
- **Required Action:** Add a max header line length constant (e.g., `MAX_HEADER_LINE: usize = 4096`) and use bounded reading. Either wrap reader with `take()` or read byte-by-byte with a cap.
- **Acceptance:** Unit test that sends a header line >4KB and gets an error (not OOM).

### 2. Guard handle_start Against Starting State
- **Source:** Logic Reasoning Checker, Risks & Tradeoffs Analyzer
- **File:** `crates/fdemon-app/src/handler/dap.rs:29`
- **Problem:** Double `StartDapServer` can orphan a server task.
- **Required Action:** Change line 29 from:
  ```rust
  if state.dap_status.is_running() {
  ```
  to:
  ```rust
  if state.dap_status.is_running() || state.dap_status == DapStatus::Starting {
  ```
- **Acceptance:** Existing tests still pass. Add test: `StartDapServer` when `Starting` returns `UpdateResult::none()`.

### 3. Remove Unimplemented Capabilities
- **Source:** Risks & Tradeoffs Analyzer
- **File:** `crates/fdemon-dap/src/protocol/types.rs:316-327`
- **Problem:** Advertising capabilities the adapter cannot handle violates DAP spec.
- **Required Action:** In `Capabilities::fdemon_defaults()`, keep only `supports_configuration_done_request: Some(true)`. Set all others to `None`. Re-add as Phase 3 implements handlers.
- **Acceptance:** `cargo test -p fdemon-dap` passes. Verify initialize response only contains `supportsConfigurationDoneRequest: true`.

### 4. Log Warning + Abort on Shutdown Timeout
- **Source:** Risks & Tradeoffs Analyzer
- **File:** `crates/fdemon-dap/src/service.rs:94`
- **Problem:** Timeout silently abandons the server task.
- **Required Action:** Replace `let _ = tokio::time::timeout(...)` with:
  ```rust
  match tokio::time::timeout(Duration::from_secs(5), handle.task).await {
      Ok(_) => {} // Task completed normally
      Err(_) => {
          tracing::warn!("DAP server task did not complete within 5s timeout");
      }
  }
  ```
- **Acceptance:** Compiles. Warning is visible in tracing output when timeout fires.

## Post-Merge Tracking (Phase 3)

### 5. Consolidate CLI Settings Override
- **File:** `crates/fdemon-tui/src/runner.rs:80-84`, `src/headless/runner.rs:39-43`
- **Problem:** Dual mutation of `engine.settings` and `engine.state.settings` can drift.
- **Suggested Action:** Add `Engine::apply_cli_dap_override(port: u16)` method.

### 6. Handle Toggle During Transitional States
- **File:** `crates/fdemon-app/src/handler/dap.rs:46-52`
- **Problem:** `ToggleDap` during `Starting`/`Stopping` has surprising behavior.
- **Suggested Action:** Return `UpdateResult::none()` for transitional states.

### 7. Add Connection Limit
- **File:** `crates/fdemon-dap/src/server/mod.rs:161`
- **Suggested Action:** Add `tokio::sync::Semaphore` with configurable max (default 8).

### 8. Restrict DapServerHandle Field Visibility
- **File:** `crates/fdemon-dap/src/server/mod.rs:64-79`
- **Suggested Action:** Make `shutdown_tx`/`task` `pub(crate)`, expose `port()` accessor.

### 9. Remove Unused fdemon-daemon Dependency
- **File:** `crates/fdemon-dap/Cargo.toml:10`
- **Suggested Action:** Remove until Phase 3 VM Service bridge needs it.

### 10. Add HeadlessEvent Variant for DAP Port
- **File:** `src/headless/runner.rs:148-158`
- **Suggested Action:** Replace `emit_dap_port_json` with `HeadlessEvent::dap_server_started(port).emit()`.

### 11. Client Registry Instead of Counter
- **File:** `crates/fdemon-app/src/handler/dap.rs:79-93`
- **Suggested Action:** Track client IDs in `HashSet<String>`.

### 12. Accept Loop Error Backoff
- **File:** `crates/fdemon-dap/src/server/mod.rs:220-228`
- **Suggested Action:** Add delay after accept failure.

### 13. Verify support_terminate_debuggee Field Name
- **File:** `crates/fdemon-dap/src/protocol/types.rs`
- **Suggested Action:** Verify serde rename matches DAP JSON schema exactly.

## Re-review Checklist

After addressing pre-merge fixes, verify:
- [ ] `cargo test -p fdemon-dap` passes
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace -- -D warnings` clean
- [ ] Initialize response contains only implemented capabilities
- [ ] Double `StartDapServer` is a no-op when already Starting
- [ ] Codec rejects oversized header lines
