# Code Review: Session Resilience Phase 2 — Emit VmServiceReconnecting Message

**Review Date:** 2026-02-25
**Branch:** `feat/session-resilience`
**Verdict:** ⚠️ APPROVED WITH CONCERNS

---

## Change Summary

Phase 2 wires the VM Service reconnection lifecycle from the daemon layer through to the existing TUI indicators. Four tasks were completed:

| Task | Description | Files |
|------|-------------|-------|
| 04 | Add `VmClientEvent` enum, update channel types | `protocol.rs`, `client.rs`, `mod.rs` |
| 05 | Emit lifecycle events from reconnection loop | `client.rs` |
| 06 | Update `forward_vm_events` to handle `VmClientEvent` | `actions.rs` |
| 07 | Add handler-level unit tests | `handler/tests.rs` |

**Stats:** 5 source files modified, +294 / -25 lines, 4 new tests

---

## Reviewer Verdicts

| Reviewer | Verdict | Critical | Major | Minor |
|----------|---------|----------|-------|-------|
| Architecture Enforcer | ⚠️ Warnings | 0 | 0 | 2 |
| Code Quality Inspector | ⚠️ Concerns | 0 | 1 | 4 |
| Logic & Reasoning Checker | ⚠️ Concerns | 0 | 1 | 2 |
| Risks & Tradeoffs Analyzer | ⚠️ Concerns | 0 | 2 | 3 |

---

## Issues Found

### Major Issues (Should Fix)

#### 1. Lifecycle events silently dropped without logging
**Source:** Code Quality, Logic
**File:** `crates/fdemon-daemon/src/vm_service/client.rs:612,620,647`

The three `let _ = event_tx.try_send(VmClientEvent::...)` calls discard errors silently, while the existing stream event handler at line 815 logs a `warn!` on send failure. This is an observability regression. `PermanentlyDisconnected` in particular is a terminal event — if dropped, the UI stays stuck on "Reconnecting" until the channel closes.

**Fix:** Add `warn!` logging on failure, matching the stream event precedent:
```rust
if let Err(e) = event_tx.try_send(VmClientEvent::PermanentlyDisconnected) {
    warn!("VM Service: failed to deliver PermanentlyDisconnected event: {}", e);
}
```

#### 2. Reconnection via `VmServiceConnected` wipes performance state
**Source:** Risks & Tradeoffs, Logic
**File:** `crates/fdemon-app/src/handler/update.rs:1189-1198` (pre-existing handler)

When `VmClientEvent::Reconnected` maps to `Message::VmServiceConnected`, the handler resets `PerformanceState` entirely — clearing memory history, allocation profiles, and frame timings accumulated before the brief disconnect. It also adds a "VM Service connected" log that doesn't distinguish reconnection from initial connection.

**Recommendation:** Track as a follow-up task. Consider either:
- A `Message::VmServiceReconnected` variant that preserves accumulated telemetry
- Or at minimum, a distinct log message for reconnection

#### 3. Old performance polling task not cleaned up on reconnection
**Source:** Logic, Risks & Tradeoffs
**File:** `crates/fdemon-app/src/handler/update.rs:1247` (pre-existing handler)

The `VmServiceConnected` handler spawns `StartPerformanceMonitoring` without aborting the previous polling task. During reconnection, the old task may still be running (its `VmRequestHandle` shares the same command channel). This could briefly produce duplicate `VmServiceMemorySample` messages.

**Recommendation:** Track as a follow-up. Add `perf_task_handle.take().abort()` and `perf_shutdown_tx` cleanup in the `VmServiceConnected` handler before spawning a new monitoring task.

### Minor Issues (Should Fix)

#### 4. Stale doc example in `mod.rs`
**Source:** Architecture Enforcer, Code Quality
**File:** `crates/fdemon-daemon/src/vm_service/mod.rs:38-39`

The quick-start example shows `event.params.stream_id` on a `VmClientEvent`, which no longer compiles. Should destructure `VmClientEvent::StreamEvent(e)` first.

**Fix:** Update the doc example to match on `VmClientEvent` variants.

#### 5. No test for `PermanentlyDisconnected` propagation
**Source:** Architecture Enforcer, Code Quality, Risks & Tradeoffs
**File:** `crates/fdemon-app/src/handler/tests.rs`

The four new tests cover `Reconnecting` and the reconnection cycle well, but there is no test for the full failure path: `Reconnecting` → `VmServiceDisconnected` (triggered by `PermanentlyDisconnected` breaking the forwarding loop).

**Fix:** Add `test_vm_service_disconnected_after_reconnecting_clears_status`.

#### 6. Inconsistent `connection_status` guarding across VM lifecycle handlers
**Source:** Risks & Tradeoffs
**File:** `crates/fdemon-app/src/handler/update.rs:1203,1280` (pre-existing)

`handle_vm_service_reconnecting` correctly guards on `active_id == Some(session_id)`, but `VmServiceConnected` and `VmServiceDisconnected` handlers do NOT. In multi-session scenarios, a background session's `VmServiceConnected` could incorrectly flip the global status.

**Recommendation:** Track as a follow-up — apply the same active-session guard to the `connection_status` update in both handlers.

### Notes (No Action Required)

- `resubscribe_streams` leaking tracker slots on serialization/send failure is **pre-existing** — not introduced by this PR
- `Ordering::SeqCst` for the monotonic request ID counter is **pre-existing**
- `VmConnectionStatus::Connected` as the default is **pre-existing** design
- The single-slash comment claim (`/ Action:` instead of `// Action:`) was **verified as a false positive** — all comments are correctly formed

---

## Architecture Compliance

| Check | Status |
|-------|--------|
| Layer boundaries (core → daemon → app → tui) | PASS |
| TEA pattern: update() is pure | PASS |
| TEA pattern: side effects via UpdateAction | PASS |
| TEA pattern: events routed through Message enum | PASS |
| Module responsibility boundaries | PASS |
| Re-exports appropriate for public API | PASS |
| No upstream imports in daemon layer | PASS |

---

## Quality Gate

| Check | Status |
|-------|--------|
| `cargo fmt --all --check` | PASS |
| `cargo check --workspace` | PASS |
| `cargo test --workspace` | PASS (2,529 passed, 0 failed) |
| `cargo clippy --workspace -- -D warnings` | PASS |

---

## Strengths

- **Clean enum design.** `VmClientEvent` wrapping `VmServiceEvent` is the right pattern — preserves the `Deserialize` contract on `VmServiceEvent` while adding typed lifecycle semantics
- **Correct emission ordering.** `Reconnecting` before backoff sleep (immediate feedback), `Reconnected` after state transition, `PermanentlyDisconnected` only at exhaustion — all verified
- **Exhaustive pattern matching.** `forward_vm_events` handles all 4 variants plus `None`; no events silently dropped at the match level
- **Good test coverage.** Four tests covering active session, inactive session, full cycle, and progressive attempts — directly testing the most important invariants
- **Leverages existing wiring.** The `Message::VmServiceReconnecting` variant, handler, state type, and all four TUI panels were already built; this phase only needed to emit and forward events

---

## Recommendation

Approve and merge after addressing:
1. **Must fix:** Add `warn!` logging to lifecycle event `try_send` failures (Issue #1)
2. **Must fix:** Update stale doc example in `mod.rs` (Issue #4)
3. **Should fix:** Add `PermanentlyDisconnected` propagation test (Issue #5)
4. **Track:** Issues #2, #3, #6 as follow-up tasks (pre-existing handler design, not blocking this PR)
