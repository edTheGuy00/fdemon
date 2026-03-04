# Phase 2 DAP Server Implementation — Consolidated Review

**Date:** 2026-03-04
**Branch:** `feat/dap-server`
**Scope:** 7 tasks, ~1,340 new lines across `fdemon-dap` crate + integration in `fdemon-app`, `fdemon-tui`, headless runner, binary crate
**Tests:** 2,955 passing (workspace), clippy clean

---

## Overall Verdict: APPROVED WITH CONCERNS

All four reviewer agents returned **APPROVED WITH CONCERNS**. No critical blocking issues were found. The implementation is architecturally sound, follows TEA patterns correctly, maintains clean layer boundaries, and has thorough test coverage. Several medium-severity issues should be tracked for resolution before or shortly after merge.

---

## Agent Verdicts

| Agent | Verdict | Critical | Major/Warning | Minor/Suggestion |
|-------|---------|----------|---------------|------------------|
| Architecture Enforcer | APPROVED WITH CONCERNS | 0 | 2 | 2 |
| Code Quality Inspector | APPROVED WITH CONCERNS | 0 | 2 | 4 |
| Logic Reasoning Checker | APPROVED WITH CONCERNS | 0 | 3 | 1 |
| Risks & Tradeoffs Analyzer | CONCERNS (non-blocking) | 0 | 1 HIGH, 5 MEDIUM | 2 LOW |

---

## Strengths

- **Clean architecture**: `fdemon-dap` correctly sits below `fdemon-app` with no circular dependencies. The TUI crate has zero direct `fdemon-dap` imports — accesses DAP state only through `AppState::dap_status`.
- **TEA compliance**: Side effects flow through `UpdateAction` (`SpawnDapServer`/`StopDapServer`). State transitions happen in `handler/dap.rs`. The view layer is pure.
- **Cross-crate decoupling**: `DapServerEvent -> Message` bridge via mpsc channels avoids circular dependency between `fdemon-dap` and `fdemon-app`.
- **Thorough test coverage**: 8 server tests, 31 session tests, 17 codec tests, 4 service tests, 23 handler tests, 17 settings tests — all passing.
- **Correct primitives**: `watch::channel` for shutdown propagation, `Arc<Mutex<Option<DapServerHandle>>>` for handle sharing (mirrors existing `SessionTaskMap` pattern).
- **Defensive coding**: `saturating_sub` for client count, `MAX_MESSAGE_SIZE` guard (10 MB), graceful handling of unknown commands.

---

## Issues Found

### HIGH Priority

#### 1. No Header Line Length Limit in Content-Length Codec
- **Source:** Risks & Tradeoffs Analyzer
- **File:** `crates/fdemon-dap/src/protocol/codec.rs:60`
- **Problem:** `reader.read_line()` has no limit on individual header line length. A malicious client can send an unbounded header line to exhaust memory before `MAX_MESSAGE_SIZE` check.
- **Recommendation:** Add a maximum header line length (e.g., 4 KB) using `take()` on the reader or manual bounded reading.

### MEDIUM Priority

#### 2. Double SpawnDapServer Race Condition
- **Source:** Logic Reasoning Checker, Risks & Tradeoffs Analyzer
- **File:** `crates/fdemon-app/src/handler/dap.rs:28-35`
- **Problem:** `handle_start` only guards against `is_running()`, which returns `false` for `Starting`. Two rapid `StartDapServer` messages can spawn two servers; the first handle gets overwritten, leaving an orphaned server.
- **Recommendation:** Change guard from `if state.dap_status.is_running()` to `if state.dap_status.is_running() || state.dap_status == DapStatus::Starting`.

#### 3. Capabilities Advertise Unimplemented Features
- **Source:** Risks & Tradeoffs Analyzer
- **File:** `crates/fdemon-dap/src/protocol/types.rs:316-327`
- **Problem:** `Capabilities::fdemon_defaults()` advertises `supports_evaluate_for_hovers`, `supports_exception_info_request`, `supports_loaded_sources_request`, etc. as `true`, but none are implemented. DAP spec: advertising a capability means the adapter MUST handle the corresponding request.
- **Recommendation:** Remove all unimplemented capabilities. Only advertise `supports_configuration_done_request: true`. Re-add as Phase 3 implements handlers.

#### 4. Direct AppState Mutation Bypasses TEA Cycle
- **Source:** Architecture Enforcer, Code Quality Inspector
- **Files:** `crates/fdemon-tui/src/runner.rs:80-84`, `src/headless/runner.rs:39-43`
- **Problem:** Both runners apply `--dap-port` by directly mutating `engine.settings.dap` AND `engine.state.settings.dap` — two separate structs that can drift out of sync.
- **Recommendation:** Add `Engine::apply_cli_dap_override(port: u16)` that atomically updates both structs.

#### 5. `handle_toggle` Inconsistent Behavior During Transitional States
- **Source:** Logic Reasoning Checker
- **File:** `crates/fdemon-app/src/handler/dap.rs:46-52`
- **Problem:** `handle_toggle` during `Starting` calls `handle_start` (re-spawning), during `Stopping` also calls `handle_start`. Neither is intuitive for a toggle operation.
- **Recommendation:** Return `UpdateResult::none()` for transitional states (`Starting`/`Stopping`), or document the intended behavior.

#### 6. Shutdown Timeout Silently Abandons Server Task
- **Source:** Risks & Tradeoffs Analyzer
- **File:** `crates/fdemon-dap/src/service.rs:94`
- **Problem:** If the 5-second timeout fires, the accept loop task is abandoned (not aborted), leaving it running. No diagnostic is logged.
- **Recommendation:** Log a warning on timeout and call `handle.task.abort()`.

#### 7. No Maximum Client Connection Limit
- **Source:** Risks & Tradeoffs Analyzer
- **File:** `crates/fdemon-dap/src/server/mod.rs:161`
- **Problem:** Accept loop spawns a new task per connection with no cap.
- **Recommendation:** Add `tokio::sync::Semaphore` with a default of 8-16 concurrent clients.

#### 8. Public Fields on DapServerHandle
- **Source:** Architecture Enforcer
- **File:** `crates/fdemon-dap/src/server/mod.rs:64-79`
- **Problem:** `shutdown_tx` and `task` are `pub`, allowing callers to bypass `DapService::stop`. Nothing enforces the shutdown protocol.
- **Recommendation:** Make fields `pub(crate)` and expose only `port()` as an accessor.

### LOW Priority

#### 9. `support_terminate_debuggee` Typo (Missing 's')
- **Source:** Code Quality Inspector
- **File:** `crates/fdemon-dap/src/protocol/types.rs`
- **Problem:** DAP spec field is `supportTerminateDebuggee` (no 's'). If this field name is used in serde rename, it should match spec exactly. Verify against the DAP JSON schema.

#### 10. Unused `fdemon-daemon` Dependency
- **Source:** Architecture Enforcer
- **File:** `crates/fdemon-dap/Cargo.toml:10`
- **Problem:** `fdemon-daemon.workspace = true` is declared but unused in Phase 2. Increases compile time unnecessarily.
- **Recommendation:** Remove and add back when Phase 3 VM Service bridge is implemented.

#### 11. Headless Output Bypasses HeadlessEvent Pattern
- **Source:** Architecture Enforcer
- **File:** `src/headless/runner.rs:148-158`
- **Problem:** `emit_dap_port_json` writes directly to stdout instead of using `HeadlessEvent::emit()`.
- **Recommendation:** Add `HeadlessEvent::dap_server_started(port)` variant.

#### 12. `client_count` Integer Without Client Registry
- **Source:** Risks & Tradeoffs Analyzer
- **File:** `crates/fdemon-app/src/handler/dap.rs:79-93`
- **Problem:** Simple counter can drift if connect/disconnect events are lost.
- **Recommendation:** Track client IDs in a `HashSet<String>` for self-correcting count (Phase 3).

#### 13. No Rate Limiting on Accept Loop Error Path
- **Source:** Risks & Tradeoffs Analyzer
- **File:** `crates/fdemon-dap/src/server/mod.rs:220-228`
- **Problem:** Persistent accept failures create tight error loop.
- **Recommendation:** Add backoff delay after accept failure.

---

## Action Items Summary

| Priority | Count | Recommendation |
|----------|-------|---------------|
| HIGH | 1 | Fix before merge (header line length limit) |
| MEDIUM | 7 | Fix before merge or track for immediate follow-up |
| LOW | 5 | Track for Phase 3 |

### Suggested Pre-Merge Fixes
1. Add header line length limit in codec (#1)
2. Guard `handle_start` against `DapStatus::Starting` (#2)
3. Remove unimplemented capabilities from `fdemon_defaults()` (#3)
4. Log warning + abort on shutdown timeout (#6)

### Suggested Post-Merge / Phase 3 Tracking
5. Consolidate CLI settings override into Engine method (#4)
6. Handle `toggle` during transitional states (#5)
7. Add connection limit (#7)
8. Restrict DapServerHandle field visibility (#8)
9-13. Low priority items

---

## Reviewed By

| Agent | Files Analyzed | Duration |
|-------|---------------|----------|
| Architecture Enforcer | 28 | ~4 min |
| Code Quality Inspector | 25+ | ~3 min |
| Logic Reasoning Checker | 20+ | ~3 min |
| Risks & Tradeoffs Analyzer | 30+ | ~3 min |
