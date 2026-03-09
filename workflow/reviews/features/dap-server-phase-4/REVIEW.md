# Code Review: DAP Server Phase 4 — Flutter Integration & Polish

**Date:** 2026-03-06
**Branch:** `feat/dap-server`
**Scope:** 12 tasks, 35 files changed, ~10,700 lines added
**Verdict:** ⚠️ **NEEDS WORK** (2 critical, 6 major, 8 minor issues)

---

## Review Summary

Phase 4 is a substantial and largely well-structured implementation that completes the DAP debugging experience with debug event routing, hot reload/restart, conditional breakpoints, logpoints, expression evaluation, source references, multi-session support, and production hardening. The architecture correctly maintains the critical `fdemon-dap` → `fdemon-app` boundary via trait objects and channels. Test coverage is comprehensive (~220 new tests, all passing).

However, **two critical issues** block merge: (1) `IsolateRunnable` events are incorrectly translated to `IsolateStart`, making breakpoint persistence after hot restart non-functional (Task 10 is dead code), and (2) `adapter/mod.rs` at 5,000+ lines violates the project's 500-line file size standard by 10x.

### Agent Verdicts

| Agent | Verdict | Key Finding |
|-------|---------|-------------|
| Architecture Enforcer | ⚠️ CONCERNS | TEA purity violation: blocking mutex lock + channel sends inside `update()` |
| Code Quality Inspector | ⚠️ NEEDS WORK | `adapter/mod.rs` at 5000+ lines; `expect()` in library code; swallowed errors |
| Logic & Reasoning Checker | ⚠️ CONCERNS | `IsolateRunnable` → `IsolateStart` mistranslation breaks breakpoint persistence |
| Risks & Tradeoffs Analyzer | ⚠️ CONCERNS | No backend call timeouts; `allThreadsStopped: true` incorrect for multi-isolate |

---

## Critical Issues (Must Fix)

### 1. `IsolateRunnable` event never forwarded — breakpoint persistence is dead code

- **Source:** Logic Checker, Risks Analyzer (both independently identified)
- **File:** `crates/fdemon-app/src/handler/devtools/debug.rs:263-266`
- **Problem:** `IsolateEvent::IsolateRunnable` is translated to `DapDebugEvent::IsolateStart` instead of `DapDebugEvent::IsolateRunnable`. The adapter's breakpoint re-application logic (`adapter/mod.rs:1127-1231`) matches on `DebugEvent::IsolateRunnable` which is never produced. After hot restart, breakpoints are marked unverified but never re-applied.
- **Impact:** Task 10 (breakpoint persistence across hot restart) is structurally complete but **non-functional**. Users will see grey breakpoints after every hot restart.
- **Required Action:** Change the translation to produce `DapDebugEvent::IsolateRunnable { isolate_id, name }` for `IsolateEvent::IsolateRunnable`.
- **Acceptance:** After hot restart, breakpoints automatically re-appear as verified in the IDE.

### 2. `adapter/mod.rs` at 5,000+ lines — 10x over file size limit

- **Source:** Code Quality Inspector
- **File:** `crates/fdemon-dap/src/adapter/mod.rs` (5,031 lines)
- **Problem:** `CODE_STANDARDS.md` mandates files > 500 lines be split into submodules. This file contains the `DebugBackend` trait definition, `DynDebugBackend` vtable, all `DapAdapter` method implementations, all supporting types, constants, and the full inline test suite.
- **Impact:** Severe maintainability risk. Future contributors must understand the entire 5,000-line file to modify any handler.
- **Required Action:** Split into submodules: `adapter/backend.rs` (trait + vtable), `adapter/types.rs` (supporting types + constants), `adapter/handlers.rs` (request dispatch), `adapter/events.rs` (debug event processing), `adapter/variables.rs` (scope/variable expansion). `adapter/mod.rs` becomes a thin re-export facade.
- **Acceptance:** No single file exceeds 800 lines. All existing tests pass.

---

## Major Issues (Should Fix)

### 3. Blocking mutex lock inside TEA `update()` function

- **Source:** Architecture Enforcer
- **File:** `crates/fdemon-app/src/handler/devtools/debug.rs:333-356`
- **Problem:** `forward_dap_event()` acquires a `std::sync::Mutex` lock and performs `try_send` channel operations inside the synchronous TEA update path. This violates the TEA principle that `update()` should be a pure state transformer with side effects routed via `UpdateAction`.
- **Suggested Action:** Return `UpdateAction::ForwardDapDebugEvents(Vec<DapDebugEvent>)` from handlers; perform channel sends in `handle_action()`.

### 4. Silent event loss on channel full (`TrySendError::Full`)

- **Source:** Architecture Enforcer, Risks Analyzer
- **File:** `crates/fdemon-app/src/handler/devtools/debug.rs:341-345`
- **Problem:** When the debug event channel is full, events are silently dropped with a `debug!` log. For `stopped`/`continued` events, dropping even one permanently desynchronizes the IDE's debugger state (play/pause button stuck).
- **Suggested Action:** Elevate log to `warn!` at minimum. Consider pruning the stalled sender (same as `Closed`) since a 64-item backlog means the session is broken.

### 5. `expect()` in library code — panic risk

- **Source:** Code Quality Inspector
- **File:** `crates/fdemon-dap/src/adapter/mod.rs:1702`
- **Problem:** `self.breakpoint_state.lookup_by_dap_id(dap_id).expect("entry was just inserted")` will panic if the invariant is ever violated by future refactoring.
- **Suggested Action:** Replace with graceful error handling: `let Some(entry) = ... else { return DapResponse::error(...); }`.

### 6. Three silent `resume()` failures in conditional breakpoint / logpoint paths

- **Source:** Code Quality Inspector, Logic Checker
- **File:** `crates/fdemon-dap/src/adapter/mod.rs:1026, 1047, 1094`
- **Problem:** `let _ = self.backend.resume(&isolate_id, None).await;` — if resume fails, the isolate stays paused forever from the VM's perspective, but the adapter has already returned without emitting a `stopped` event. The IDE is left in an inconsistent state.
- **Suggested Action:** Log errors at `warn!` level. Consider emitting a `stopped` event as fallback when resume fails.

### 7. `paused_isolates` not pruned on `IsolateExit`

- **Source:** Logic Checker
- **File:** `crates/fdemon-dap/src/adapter/mod.rs:928-967`
- **Problem:** When an isolate exits, it's removed from `thread_map` but NOT from `paused_isolates`. `most_recent_paused_isolate()` may return a dead isolate ID, causing evaluate requests to fail against a non-existent isolate.
- **Suggested Action:** Add `self.paused_isolates.retain(|id| id != &isolate_id)` in the `IsolateExit` handler.

### 8. Breakpoint condition updates ignored for same-line breakpoints

- **Source:** Logic Checker
- **File:** `crates/fdemon-dap/src/adapter/mod.rs:1652-1659`
- **Problem:** When `setBreakpoints` reuses an existing breakpoint at the same line, it skips updating conditions/hit_conditions/log_messages. Changing a breakpoint from unconditional to conditional at the same line has no effect.
- **Suggested Action:** Compare existing conditions against incoming request; update the entry if conditions differ.

---

## Minor Issues (Consider Fixing)

### 9. `#[allow(dead_code)]` on public API items

- **Files:** `dap_backend.rs:489,592` (`DapSessionMetadata::new`, `session_metadata_slot`); `adapter/mod.rs:697-718` (5 error code constants, `REQUEST_TIMEOUT`)
- **Problem:** 7 public items marked dead code indicate incomplete wiring. Either complete the integration or remove until needed.

### 10. `SourceReferenceStore::get_or_create` uses O(n) linear scan

- **File:** `crates/fdemon-dap/src/adapter/stack.rs:89-95`
- **Problem:** Linear scan for every stack frame. Add a reverse `HashMap<(String, String), i64>` index for O(1) lookup.

### 11. `AppState` holds infrastructure state (`Arc<Mutex<Vec<Sender>>>`)

- **File:** `crates/fdemon-app/src/state.rs:887`
- **Problem:** TEA model should contain pure domain state, not live channel infrastructure. Consider keeping `dap_debug_senders` exclusively on `Engine`.

### 12. `on_resume()` called eagerly before `backend.resume()` succeeds

- **File:** `crates/fdemon-dap/src/adapter/mod.rs:1888-1897`
- **Problem:** Variable/frame stores are cleared before the resume RPC is confirmed. If resume fails, stale reference errors occur even though the isolate is still paused.

### 13. `allThreadsStopped: true` always emitted

- **File:** `crates/fdemon-dap/src/adapter/mod.rs:1108`
- **Problem:** Incorrect for multi-isolate Flutter apps. Should be `false` or computed dynamically.

### 14. Five dead `UpdateAction` arms with misleading "Phase 2" comments

- **File:** `crates/fdemon-app/src/actions/mod.rs:357-416`
- **Problem:** `PauseIsolate`, `ResumeIsolate`, `AddBreakpoint`, `RemoveBreakpoint`, `SetIsolatePauseMode` are matched but log "not yet wired (Phase 2)". Phase 2 is complete; these are superseded by `DebugBackend` trait calls.

### 15. Globals scope returns empty list

- **File:** `crates/fdemon-dap/src/adapter/mod.rs:2360-2364`
- **Problem:** Globals scope is advertised in scopes response but always returns `Ok(Vec::new())`. Remove from scopes response until implemented.

### 16. Duplicate mock backend implementations in tests

- **File:** `crates/fdemon-dap/src/adapter/mod.rs`
- **Problem:** `MockBackend` and `MockBackendWithUri` share 14/17 method implementations. Consolidate into a single configurable mock.

---

## Quality Gate Results

| Check | Result |
|-------|--------|
| `cargo fmt --all` | ✅ Pass |
| `cargo check --workspace` | ✅ Pass |
| `cargo test --workspace` | ✅ Pass (3,634+ tests, 0 failures) |
| `cargo clippy --workspace -- -D warnings` | ✅ Pass |

---

## Metrics

| Metric | Score | Notes |
|--------|-------|-------|
| Architecture | 4/5 | Layer boundaries correctly maintained; TEA purity violation in event forwarding |
| Rust Idioms | 3/5 | Unnecessary clones, `expect()` in library code, swallowed errors |
| Error Handling | 3/5 | Silent resume failures, dead error code constants, stringly-typed `get_source` |
| Testing | 4/5 | ~220 new tests, good coverage; missing file-watcher gate tests |
| Documentation | 4/5 | Strong module docs; dead code items left documented but unused |
| Maintainability | 2/5 | `adapter/mod.rs` at 5,000+ lines is the dominant issue |
| **Overall** | **3/5** | Solid functionality, needs structural cleanup before merge |

---

## Verdict: ⚠️ NEEDS WORK

**Blocking issues:** 2 (IsolateRunnable mistranslation, file size violation)
**Major issues:** 6 (TEA purity, silent event loss, expect panic, silent resume, stale isolates, condition update)
**Minor issues:** 8

The implementation is functionally comprehensive and architecturally sound at the crate boundary level. However, the `IsolateRunnable` mistranslation renders Task 10 (breakpoint persistence) non-functional, and the 5,000+ line `adapter/mod.rs` is a maintenance liability that must be addressed before merge. The remaining major issues are correctness concerns that could cause debugging session failures in production.

**Recommended action:** Fix the 2 critical issues, address the 6 major issues, then re-review.
