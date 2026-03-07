# Action Items: DAP Server Phase 4

**Review Date:** 2026-03-06
**Verdict:** ⚠️ NEEDS WORK
**Blocking Issues:** 2
**Major Issues:** 6
**Minor Issues:** 8

---

## Critical Issues (Must Fix)

### 1. Fix `IsolateRunnable` event forwarding

- **Source:** Logic Checker, Risks Analyzer
- **File:** `crates/fdemon-app/src/handler/devtools/debug.rs:263-266`
- **Problem:** `IsolateEvent::IsolateRunnable` is translated to `DapDebugEvent::IsolateStart` instead of `DapDebugEvent::IsolateRunnable`, making the adapter's breakpoint re-application logic unreachable after hot restart.
- **Required Action:** Change the match arm to produce `DapDebugEvent::IsolateRunnable { isolate_id, name }`. Verify the `DapDebugEvent::IsolateRunnable` variant exists in the enum (it does, at `adapter/mod.rs:583`).
- **Acceptance:** After hot restart with breakpoints set, breakpoints are automatically re-applied and verified. The adapter's `IsolateRunnable` handler at `adapter/mod.rs:1127` is exercised.

### 2. Split `adapter/mod.rs` into submodules

- **Source:** Code Quality Inspector
- **File:** `crates/fdemon-dap/src/adapter/mod.rs` (5,031 lines)
- **Problem:** 10x over the project's 500-line file size limit.
- **Required Action:** Split into:
  - `adapter/backend.rs` — `LocalDebugBackend` trait, `DebugBackend` trait, `DynDebugBackendInner`, `DynDebugBackend`, `BackendError`
  - `adapter/types.rs` — `StepMode`, `BreakpointResult`, `DapExceptionPauseMode`, `DebugEvent`, `PauseReason`, constants
  - `adapter/handlers.rs` — Request dispatch methods (`handle_attach`, `handle_set_breakpoints`, etc.)
  - `adapter/events.rs` — `handle_debug_event`, event processing
  - `adapter/variables.rs` — `get_scope_variables`, `expand_object`, `instance_ref_to_variable`
  - `adapter/mod.rs` — `DapAdapter` struct definition + re-exports (< 200 lines)
- **Acceptance:** No file exceeds 800 lines. All 561+ adapter tests pass. `cargo test -p fdemon-dap` green.

---

## Major Issues (Should Fix)

### 3. Move debug event forwarding out of TEA `update()`

- **Source:** Architecture Enforcer
- **File:** `crates/fdemon-app/src/handler/devtools/debug.rs:333-356`
- **Problem:** `forward_dap_event()` acquires a blocking mutex and sends on channels inside the synchronous TEA update path.
- **Suggested Action:** Add `UpdateAction::ForwardDapDebugEvents(Vec<DapDebugEvent>)`. Return it from `handle_debug_event`/`handle_isolate_event`. Perform sends in `handle_action()`.

### 4. Handle channel-full scenario for debug events

- **Source:** Architecture Enforcer, Risks Analyzer
- **File:** `crates/fdemon-app/src/handler/devtools/debug.rs:341-345`
- **Problem:** `TrySendError::Full` silently drops pause/resume events with a `debug!` log.
- **Suggested Action:** Elevate to `warn!("DAP debug event channel full — event dropped, IDE may desync")`. Consider pruning the sender.

### 5. Remove `expect()` in `handle_set_breakpoints`

- **Source:** Code Quality Inspector
- **File:** `crates/fdemon-dap/src/adapter/mod.rs:1702`
- **Problem:** `expect("entry was just inserted")` can panic in library code.
- **Suggested Action:** Replace with `let Some(entry) = ... else { tracing::error!("BUG: ..."); return DapResponse::error(...); };`.

### 6. Log resume failures in conditional breakpoint / logpoint paths

- **Source:** Code Quality Inspector, Logic Checker
- **Files:** `crates/fdemon-dap/src/adapter/mod.rs:1026, 1047, 1094`
- **Problem:** `let _ = self.backend.resume(...)` discards errors silently.
- **Suggested Action:** `if let Err(e) = self.backend.resume(...).await { tracing::warn!("Auto-resume failed: {}", e); }`.

### 7. Prune `paused_isolates` on `IsolateExit`

- **Source:** Logic Checker
- **File:** `crates/fdemon-dap/src/adapter/mod.rs:928-967`
- **Problem:** Dead isolate IDs persist in `paused_isolates`, causing stale evaluate context.
- **Suggested Action:** Add `self.paused_isolates.retain(|id| id != &isolate_id);` in the `IsolateExit` handler.

### 8. Update breakpoint conditions when changed at same line

- **Source:** Logic Checker
- **File:** `crates/fdemon-dap/src/adapter/mod.rs:1652-1659`
- **Problem:** Same-line breakpoint reuse skips condition updates.
- **Suggested Action:** Compare conditions; if different, update the entry's `condition`, `hit_condition`, and `log_message`.

---

## Minor Issues (Consider Fixing)

### 9. Remove or wire `#[allow(dead_code)]` items

- `dap_backend.rs:489,592` — `DapSessionMetadata::new`, `session_metadata_slot`
- `adapter/mod.rs:697-718` — `REQUEST_TIMEOUT`, `ERR_NOT_CONNECTED`, `ERR_NO_DEBUG_SESSION`, `ERR_THREAD_NOT_FOUND`, `ERR_EVAL_FAILED`

### 10. Add reverse index to `SourceReferenceStore`

- `stack.rs:89-95` — Add `HashMap<(String, String), i64>` for O(1) lookup

### 11. Move `dap_debug_senders` from `AppState` to `Engine`

- `state.rs:887` — Keep infrastructure out of the TEA model

### 12. Defer `on_resume()` until resume RPC succeeds

- `adapter/mod.rs:1888-1897` — Clear stores after confirmed resume

### 13. Fix `allThreadsStopped` for multi-isolate

- `adapter/mod.rs:1108` — Set to `false` or compute dynamically

### 14. Clean up dead `UpdateAction` arms

- `actions/mod.rs:357-416` — Remove or convert to `unreachable!()`

### 15. Remove Globals scope from scopes response

- `adapter/mod.rs:2360-2364` — Returns empty; confuses users

### 16. Consolidate duplicate mock backends in tests

- `adapter/mod.rs` — Merge `MockBackend` and `MockBackendWithUri`

---

## Re-review Checklist

After addressing issues, the following must pass:

- [ ] Critical issue #1 resolved: `IsolateRunnable` forwarded correctly
- [ ] Critical issue #2 resolved: `adapter/mod.rs` split into submodules (< 800 lines each)
- [ ] All major issues resolved or justified
- [ ] `cargo fmt --all` — Pass
- [ ] `cargo check --workspace` — Pass
- [ ] `cargo test --workspace` — Pass (all tests green)
- [ ] `cargo clippy --workspace -- -D warnings` — Pass
