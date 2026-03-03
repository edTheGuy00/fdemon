# Review: Session Resilience Phase 3 — Process Health Monitoring

**Date:** 2026-02-26
**Branch:** feat/session-resilience
**Verdict:** :warning: **NEEDS WORK**
**Blocking Issues:** 2

---

## Summary

Phase 3 implements two watchdog mechanisms (process-level and VM-level) plus proper exit code capture. The core design is architecturally sound: the `FlutterProcess` refactor moving `Child` into a dedicated `wait_for_exit` task with `AtomicBool` + `Notify` + `oneshot` is the correct Rust idiom. The `VersionInfo` heartbeat probe is minimal and well-placed. However, two concrete issues must be addressed before merge: (1) the heartbeat failure counter is not reset on reconnection events, risking premature disconnect, and (2) the watchdog can produce duplicate `Exited` events that overwrite the real exit code.

## Files Changed

| File | Lines | Changes |
|------|-------|---------|
| `crates/fdemon-app/src/actions.rs` | +126 | Process watchdog arm, VM heartbeat arm, 4 constants, 2 constant tests |
| `crates/fdemon-app/src/handler/tests.rs` | +139 | Exit code tests (code 0, None, nonzero), disconnect cleanup test |
| `crates/fdemon-daemon/src/process.rs` | +421/-57 | Major refactor: Child moved to wait task, AtomicBool/Notify/oneshot model, 5 new tests |
| `crates/fdemon-daemon/src/vm_service/protocol.rs` | +46 | `VersionInfo` struct + 3 deserialization tests |
| `crates/fdemon-daemon/src/vm_service/client.rs` | +19 | `get_version()` method |
| `crates/fdemon-daemon/src/vm_service/mod.rs` | +4 | `VersionInfo` re-export |

## Quality Gate

| Check | Result |
|-------|--------|
| `cargo fmt --all` | PASS |
| `cargo check --workspace` | PASS |
| `cargo test --workspace` | PASS (2,783 passed, 0 failed) |
| `cargo clippy --workspace -- -D warnings` | PASS |

---

## Agent Verdicts

| Agent | Verdict | Critical | Major | Minor | Nitpick |
|-------|---------|----------|-------|-------|---------|
| Architecture Enforcer | PASS | 0 | 0 | 2 warnings | 1 |
| Code Quality Inspector | NEEDS WORK | 0 | 2 | 5 | 2 |
| Logic & Reasoning Checker | CONCERNS | 1 | 2 | 0 | 5 |
| Risks & Tradeoffs Analyzer | CONCERNS | 0 | 2 | 1 | 0 |

---

## Blocking Issues

### 1. Heartbeat failure counter not reset on reconnection events

**Source:** Code Quality, Logic, Risks (all three agents flagged this independently)
**File:** `crates/fdemon-app/src/actions.rs` (forward_vm_events, ~line 1060)
**Severity:** HIGH

When `VmClientEvent::Reconnected` arrives, the code forwards the message to the TEA handler but does NOT reset `consecutive_failures` to 0. During a reconnection backoff (up to 127s), heartbeat probes fail with `ChannelClosed`, incrementing the counter. If 2 failures accumulate during reconnection and the first post-reconnect heartbeat encounters any transient issue, `consecutive_failures` hits 3 and the connection is terminated — even though it just reconnected successfully.

**Fix:** Add `consecutive_failures = 0;` inside the `VmClientEvent::Reconnected` arm. Also consider resetting on `VmClientEvent::Reconnecting` to prevent accumulation during the reconnection window.

### 2. Duplicate Exited events: watchdog vs wait_for_exit race

**Source:** Architecture, Logic, Risks (all three agents flagged this)
**File:** `crates/fdemon-app/src/actions.rs` (spawn_session, ~lines 479-493)
**Severity:** HIGH

The `wait_for_exit` task sets `exited = true` (AtomicBool) BEFORE sending `DaemonEvent::Exited { code: Some(N) }` to the daemon channel. There is a window where:
1. `wait_for_exit` sets atomic to `true` and queues the real `Exited` event in `daemon_rx`
2. `daemon_rx.recv()` arm picks up the real `Exited`, sets `process_exited = true`, forwards it — but does NOT break the loop
3. Next iteration: `watchdog.tick()` fires, sees `has_exited() == true`, synthesizes a SECOND `Exited { code: None }` and breaks

The TEA handler receives two exit messages. The handler is idempotent (uses `.take()`), so no crash — but duplicate log entries appear and the watchdog's `code: None` may overwrite the real exit code.

**Fix (recommended):** In the watchdog arm, check the local `process_exited` flag before synthesizing an event:
```rust
_ = watchdog.tick() => {
    if !process_exited && process.has_exited() {
        // ... synthesize event
    }
}
```

**Alternative fix:** In the `daemon_rx.recv()` arm, `break` the loop when an `Exited` event is received (after forwarding it), preventing the watchdog from ever firing again.

---

## Major Issues (Should Fix)

### 3. Heartbeat uses raw `request("getVersion", None)` instead of typed `get_version()`

**Source:** Architecture Enforcer
**File:** `crates/fdemon-app/src/actions.rs:1083`

The heartbeat calls `heartbeat_handle.request("getVersion", None)` directly because `get_version()` only exists on `VmServiceClient`, not `VmRequestHandle`. This means `VmServiceClient::get_version()` has zero callers — it's dead code. Either add `get_version()` to `VmRequestHandle` (making the heartbeat self-documenting) or remove it from `VmServiceClient`.

### 4. Missing `#[serde(rename_all = "camelCase")]` on `VersionInfo`

**Source:** Code Quality Inspector
**File:** `crates/fdemon-daemon/src/vm_service/protocol.rs`

Every other VM Service response type in this file uses `#[serde(rename_all = "camelCase")]`. `VersionInfo` works because `major`/`minor` are already lowercase, but the missing attribute creates an inconsistency that could break silently if fields are added later.

### 5. Magic number `9999` as JSON-RPC request ID in shutdown

**Source:** Code Quality Inspector
**File:** `crates/fdemon-daemon/src/process.rs:333`

The hardcoded `"id":9999` in the shutdown command violates the project's named-constants standard. Should be extracted to a constant. (Note: this is pre-existing code, not introduced by this phase — but it was touched in the refactor.)

---

## Minor Issues

### 6. `consecutive_failures` not reset on `Reconnecting` events

**Source:** Risks Analyzer
**File:** `crates/fdemon-app/src/actions.rs` (~line 1051)

Even better than just resetting on `Reconnected`, skip heartbeat probes entirely during reconnection state to avoid wasting command channel capacity.

### 7. No test for double-exit handling at TEA layer

**Source:** Risks Analyzer
**File:** `crates/fdemon-app/src/handler/tests.rs`

There's no test verifying `handle_session_exited` is idempotent when called twice. The duplicate race makes this a real code path. Add a test that sends two `Exited` events and verifies no panic, only one log entry.

### 8. Test naming convention violated

**Source:** Code Quality Inspector
**File:** `crates/fdemon-app/src/handler/tests.rs`

New tests don't follow the documented `test_<function>_<scenario>_<expected_result>` pattern from CODE_STANDARDS.md. E.g., `test_session_exited_with_code_zero` should indicate the expected outcome.

### 9. Platform-dependent tests missing `#[cfg(unix)]` guard

**Source:** Architecture Enforcer
**File:** `crates/fdemon-daemon/src/process.rs` (spawn_test_process helper)

Tests use `sh -c "exit N"` which won't work on Windows. Add `#[cfg(unix)]` to the helper and tests that depend on it.

### 10. Duplicate test: `test_session_exited_updates_session_phase`

**Source:** Code Quality Inspector
**File:** `crates/fdemon-app/src/handler/tests.rs:655`

This test is a strict subset of the new `test_session_exited_with_code_zero` — it sends `code: Some(0)` and checks the phase. The new test checks the same thing plus the log message. Remove the duplicate.

---

## Strengths

- **FlutterProcess refactor is excellent.** The oneshot + AtomicBool + Notify design cleanly separates Child ownership from the process API surface. `has_exited()` changing from `&mut self` to `&self` is a meaningful ergonomic gain.
- **AtomicBool ordering is correct.** `Release` store / `Acquire` load forms a proper publication pattern.
- **shutdown() race-free pattern is correct.** The `notified()` future is created before the `has_exited()` check, eliminating the TOCTOU gap.
- **Layer boundaries fully respected.** No architecture violations. All changes are in the correct crates.
- **Test coverage is solid.** 13+ new tests covering deserialization, exit codes, single-event guarantees, and disconnect cleanup.
- **Constants are well-named and documented.** No magic numbers in the new code (the `9999` is pre-existing).

---

## Verdict Rationale

Two blocking issues prevent approval:
1. The heartbeat-reconnection interaction is a functional correctness bug that will cause premature VM Service disconnects in production under reconnection scenarios.
2. The duplicate exit event race violates the explicit success criteria ("No duplicate Exited events when watchdog and wait task race") and can lose the real exit code.

Both fixes are small (a few lines each) and well-understood. After addressing the 2 blocking issues, this phase should be ready to merge.
