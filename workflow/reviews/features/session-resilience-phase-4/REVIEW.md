# Review: Session Resilience Phase 4 — Stopped Session Device Reuse

**Date:** 2026-02-26
**Verdict:** APPROVED WITH CONCERNS
**Change Type:** Bug Fix
**Branch:** `feat/session-resilience`

---

## Summary

Phase 4 fixes a UX bug where stopped sessions block new session creation on the same device. The fix adds `Session::is_active()`, `SessionManager::find_active_by_device_id()`, and swaps the launch guard call site. The implementation is minimal, correct, and well-tested. No critical issues were found, but several concerns warrant attention — most notably that stopped session accumulation against the `MAX_SESSIONS` limit is now a reachable UX problem.

---

## Agent Verdicts

| Agent | Verdict | Critical | Major | Minor | Nitpick |
|-------|---------|----------|-------|-------|---------|
| Architecture Enforcer | PASS | 0 | 0 | 2 | 0 |
| Code Quality Inspector | PASS | 0 | 0 | 3 | 2 |
| Logic & Reasoning Checker | PASS | 0 | 0 | 2 | 0 |
| Risks & Tradeoffs Analyzer | CONCERNS | 0 | 1 | 2 | 2 |
| Bug Fix Reviewer | APPROVED | 0 | 0 | 2 | 1 |

---

## Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/session/session.rs` | Added `is_active()` method (lines 521-529) |
| `crates/fdemon-app/src/session/tests.rs` | Added 5 unit tests for all `AppPhase` variants |
| `crates/fdemon-app/src/session_manager.rs` | Added `find_active_by_device_id()` + 3 unit tests |
| `crates/fdemon-app/src/handler/new_session/launch_context.rs` | Swapped `find_by_device_id` → `find_active_by_device_id` + 3 integration tests |
| `crates/fdemon-app/src/handler/tests.rs` | Replaced dead test stub with comment pointing to new tests |

---

## Consolidated Findings

### Concerns (Should Address)

#### 1. Stopped sessions count toward MAX_SESSIONS limit
- **Source:** Risks & Tradeoffs Analyzer, Logic & Reasoning Checker
- **File:** `crates/fdemon-app/src/session_manager.rs` (lines 45, 79, 117, 152)
- **Problem:** `create_session*` methods check `self.sessions.len() >= MAX_SESSIONS` counting ALL sessions including stopped ones. Before this fix, the user was forced to close stopped tabs to reuse a device. Now stopped tabs accumulate naturally. After 9 accumulated stopped sessions, the user cannot create new sessions — receiving "Maximum of 9 concurrent sessions reached" even though 0 sessions are active.
- **Impact:** UX dead-end for users who start/stop sessions without manually closing tabs.
- **Recommendation:** File a follow-up task. Best option: auto-evict the oldest stopped session when `MAX_SESSIONS` is reached and a new active session is being created. This preserves the cap while preventing the UX blocker.

#### 2. `find_by_device_id` is now dead production code
- **Source:** Architecture Enforcer, Code Quality Inspector, Risks & Tradeoffs Analyzer
- **File:** `crates/fdemon-app/src/session_manager.rs:310`
- **Problem:** After the launch guard swap, `find_by_device_id` has zero production callers — only used in its own tests and as a cross-check in the `find_active_by_device_id` test. The method is kept "for backward compatibility" but nothing depends on it. Future developers calling it in a new launch guard would reintroduce the original bug.
- **Recommendation:** Add a doc comment warning: `/// Note: returns sessions in any phase including Stopped. For launch guards that should skip stopped sessions, use find_active_by_device_id() instead.`

#### 3. Missing `Quitting` and `Reloading` phase tests at SessionManager level
- **Source:** Architecture Enforcer, Code Quality Inspector, Logic & Reasoning Checker, Bug Fix Reviewer
- **File:** `crates/fdemon-app/src/session_manager.rs` (test block)
- **Problem:** `find_active_by_device_id` tests cover `Stopped`, `Running`, and unknown device — but not `Quitting` or `Reloading`. These are tested at the `Session::is_active()` level but not at the manager integration level.
- **Recommendation:** Add `test_find_active_by_device_id_skips_quitting_session` and optionally `test_find_active_by_device_id_finds_reloading_session`.

### Observations (Non-Blocking)

#### 4. `Quitting` phase is unreachable on individual sessions
- **Source:** Risks & Tradeoffs Analyzer, Logic & Reasoning Checker
- `AppPhase::Quitting` is only set on `AppState.phase` (global), never on `Session.phase`. Including it in `is_active()` is correct defensive coding, but the branch is currently unreachable in production.

#### 5. Negated match pattern is future-safe but warrants a note
- **Source:** Logic & Reasoning Checker
- `!matches!(self.phase, AppPhase::Stopped | AppPhase::Quitting)` means new `AppPhase` variants will default to "active" — the correct safe default (new states should block device reuse until explicitly excluded).

#### 6. Task reference comment in source code
- **Source:** Code Quality Inspector
- `// Phase 4 Task 04: Device Reuse Tests for handle_launch` is a workflow reference, not a code purpose description. Prefer: `// Device reuse guard tests — verify stopped sessions allow reuse, active sessions block`.

#### 7. `find_active_by_device_id` doc comment could state positive contract
- **Source:** Code Quality Inspector
- The doc says "skips sessions in Stopped or Quitting phases" (negative). Adding "Returns Some(id) for sessions in Initializing, Running, or Reloading phase" makes the contract self-documenting.

#### 8. Inaccurate claim in task-03 notes
- **Source:** Risks & Tradeoffs Analyzer
- Task-03 says "stopped sessions have app_id = None" — this is incorrect. `mark_stopped()` does not clear `app_id`. The conclusion (no impact on `find_by_app_id`) is still correct, but the reasoning is wrong.

---

## Quality Gate Results

| Check | Status |
|-------|--------|
| `cargo fmt --all` | PASS |
| `cargo check --workspace` | PASS |
| `cargo test --workspace` | PASS (2,794 tests, 0 failures) |
| `cargo clippy --workspace -- -D warnings` | PASS |

---

## Architectural Compliance

- All changes within `fdemon-app` crate — correct layer
- Imports only from `fdemon-core` (for `AppPhase`) — no cross-layer violations
- TEA pattern respected — `is_active()` and `find_active_by_device_id()` are pure queries, no side effects
- `handle_launch` remains pure — reads state, returns `UpdateAction::SpawnSession`

---

## Test Coverage Assessment

| Layer | Tests Added | Coverage |
|-------|-------------|----------|
| `Session::is_active()` | 5 tests (all 5 `AppPhase` variants) | Complete |
| `SessionManager::find_active_by_device_id()` | 3 tests (Stopped, Running, unknown) | Missing Quitting, Reloading |
| `handle_launch` integration | 3 tests (Stopped allows, Running blocks, Initializing blocks) | Missing Quitting, Reloading |
| Dead test cleanup | Comment redirect | Appropriate |

---

## Verdict Rationale

The fix is **correct, minimal, and well-tested** at the core. It properly addresses the root cause with a clean three-layer approach. No critical or major issues were found in the code itself. The `MAX_SESSIONS` interaction is the most significant concern but is a pre-existing design characteristic exacerbated (not introduced) by this change — it warrants a follow-up task, not a merge blocker. The dead `find_by_device_id` and missing test coverage for `Quitting`/`Reloading` phases are minor gaps that should be addressed but don't affect correctness.

**Approve and merge.** Address concerns 1-3 in follow-up work.
