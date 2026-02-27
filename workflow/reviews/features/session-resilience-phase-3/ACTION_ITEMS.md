# Action Items: Session Resilience Phase 3

**Review Date:** 2026-02-26
**Verdict:** :warning: NEEDS WORK
**Blocking Issues:** 2

## Critical Issues (Must Fix)

### 1. Reset heartbeat failure counter on reconnection events
- **Source:** Code Quality, Logic, Risks (all 3 agents)
- **File:** `crates/fdemon-app/src/actions.rs`
- **Line:** ~1060 (inside `forward_vm_events`, `VmClientEvent::Reconnected` arm)
- **Problem:** `consecutive_failures` is not reset when `Reconnected` arrives, so stale failure counts from the disconnection period persist and can cause premature disconnect after reconnection.
- **Required Action:** Add `consecutive_failures = 0;` inside the `VmClientEvent::Reconnected` arm. Also add it to the `VmClientEvent::Reconnecting` arm to prevent accumulation during reconnection.
- **Acceptance:** After fix, a reconnection event must reset the failure counter to 0. Verify manually or via test.

### 2. Fix duplicate Exited event race between watchdog and wait_for_exit
- **Source:** Architecture, Logic, Risks (all 3 agents)
- **File:** `crates/fdemon-app/src/actions.rs`
- **Line:** ~479-493 (inside `spawn_session`, watchdog arm)
- **Problem:** The watchdog checks `process.has_exited()` but not the local `process_exited` flag, creating a window where both the `daemon_rx.recv()` arm and the watchdog arm emit `Exited` events.
- **Required Action:** Guard the watchdog's Exited synthesis with the `process_exited` flag:
  ```rust
  _ = watchdog.tick() => {
      if !process_exited && process.has_exited() {
          // ... existing synthesis code
      }
  }
  ```
- **Acceptance:** No duplicate `Exited` messages reach the TEA handler when both paths race. The real exit code from `wait_for_exit` is preserved.

## Major Issues (Should Fix)

### 3. `get_version()` is dead code — move to VmRequestHandle or remove
- **Source:** Architecture Enforcer
- **File:** `crates/fdemon-daemon/src/vm_service/client.rs`
- **Problem:** `VmServiceClient::get_version()` has no callers — the heartbeat uses `heartbeat_handle.request("getVersion", None)` directly.
- **Suggested Action:** Either add `get_version()` to `VmRequestHandle` and use it in the heartbeat, or remove it from `VmServiceClient`.

### 4. Add `#[serde(rename_all = "camelCase")]` to `VersionInfo`
- **Source:** Code Quality Inspector
- **File:** `crates/fdemon-daemon/src/vm_service/protocol.rs`
- **Problem:** Inconsistent with every other VM Service response type in the same module.
- **Suggested Action:** Add the attribute for consistency.

### 5. Add idempotency guard to `handle_session_exited`
- **Source:** Risks Analyzer
- **File:** `crates/fdemon-app/src/handler/session.rs` (~line 96)
- **Problem:** No early return when session is already `Stopped`, allowing duplicate log entries.
- **Suggested Action:** Add `if handle.session.phase == AppPhase::Stopped { return; }` at the top.

## Minor Issues (Consider Fixing)

### 6. Add test for double-exit idempotency
- Send two `DaemonEvent::Exited` to the same session, verify no panic and only one log entry.

### 7. Remove duplicate test `test_session_exited_updates_session_phase`
- It's a strict subset of `test_session_exited_with_code_zero`.

### 8. Add `#[cfg(unix)]` to process.rs test helper
- `spawn_test_process` uses `sh -c` which is Unix-only.

### 9. Rename new tests to follow naming convention
- Should be `test_<function>_<scenario>_<expected_result>` per CODE_STANDARDS.md.

## Re-review Checklist

After addressing issues, the following must pass:
- [ ] Issue 1: `consecutive_failures = 0` added to Reconnected (and ideally Reconnecting) arm
- [ ] Issue 2: Watchdog checks `process_exited` flag before synthesizing Exited event
- [ ] `cargo fmt --all`
- [ ] `cargo check --workspace`
- [ ] `cargo test --workspace`
- [ ] `cargo clippy --workspace -- -D warnings`
