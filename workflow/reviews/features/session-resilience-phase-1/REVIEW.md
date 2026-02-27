# Review: Session Resilience Phase 1 — Network Polling Cleanup

**Date:** 2026-02-25
**Branch:** `feat/session-resilience`
**Verdict:** APPROVED WITH CONCERNS

---

## Summary

Phase 1 fixes zombie network polling tasks that persist after session termination. Two of five cleanup paths (`handle_session_exited` and `AppStop`) had performance cleanup but were missing the identical network cleanup pattern. The fix adds matching `network_task_handle.take().abort()` + `network_shutdown_tx.take().send(true)` blocks to both paths, with two new unit tests.

**Changes are minimal, correct, and follow established patterns.** All concerns are non-blocking.

---

## Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/session.rs` | +23 lines — network cleanup blocks in `handle_session_exited` and `AppStop` branch |
| `crates/fdemon-app/src/handler/tests.rs` | +74 lines — two new test functions |
| `workflow/plans/.../TASKS.md` | Status updates |
| `workflow/plans/.../tasks/*.md` | Completion summaries |

---

## Agent Verdicts

| Agent | Verdict | Critical | Major | Minor | Nitpick |
|-------|---------|----------|-------|-------|---------|
| Bug Fix Reviewer | APPROVED | 0 | 0 | 1 | 1 |
| Architecture Enforcer | APPROVED (warnings) | 0 | 0 | 2 | 0 |
| Logic & Reasoning | PASS | 0 | 0 | 2 | 0 |
| Code Quality Inspector | APPROVED (caveats) | 0 | 1* | 3 | 1 |

*\*Downgraded from MAJOR to MINOR after analysis — see Finding #1 below.*

---

## Findings

### Finding 1: `network.recording` Not Reset on Teardown (MINOR — Not a Bug)

**Raised by:** Architecture Enforcer, Code Quality Inspector
**Severity:** MINOR (originally flagged as MAJOR by Quality Inspector)

The performance cleanup resets `handle.session.performance.monitoring_active = false` at both sites. The new network cleanup does not reset `handle.session.network.recording`.

**Why this is intentional and correct:**
- The task spec explicitly addresses this: *"`NetworkState` has no `monitoring_active` flag. The `NetworkState::recording` field controls UI recording preference, not task lifecycle — leave it unchanged."*
- `performance.monitoring_active` is a task lifecycle flag (is the polling task running?). `network.recording` is a user preference toggle (should incoming entries be stored?). They serve different purposes.
- All other cleanup paths (`VmServiceDisconnected`, `CloseCurrentSession`) also do not reset `network.recording` — this is consistent across the entire codebase.
- Preserving the user's recording preference across session restarts is the correct UX.

**Recommendation:** Add a one-line comment in both cleanup blocks explaining the intentional omission, to prevent future reviewers from re-flagging:
```rust
// Note: network.recording is a UI preference (not a lifecycle flag like
// performance.monitoring_active), so it is intentionally preserved.
```

---

### Finding 2: `network_task_handle.is_none()` Assertion Is Trivially True in Tests (MINOR)

**Raised by:** All four agents
**Severity:** MINOR

Both new tests use `attach_network_shutdown` which only sets `network_shutdown_tx`, not `network_task_handle`. Since `SessionHandle::new()` initializes `network_task_handle` to `None`, the assertion `handle.network_task_handle.is_none()` passes without ever exercising the `.take().abort()` path.

**Mitigating factor:** The existing `test_close_session_cleans_up_network_monitoring` test already covers the real `JoinHandle` abort path with a live Tokio task. The abort logic is structurally identical across all cleanup sites.

**Recommendation:** Either add a comment above the assertion:
```rust
// network_task_handle is None by default (attach_network_shutdown only sets shutdown_tx).
// This confirms .take() on None is a safe no-op. The abort path is covered by
// test_close_session_cleans_up_network_monitoring.
```
Or upgrade the tests to create a real `JoinHandle` (matching the existing close-session test pattern).

---

### Finding 3: Inconsistent Shutdown Idiom in `update.rs` (MINOR — Pre-existing)

**Raised by:** All four agents
**Severity:** MINOR (pre-existing, not introduced by this change)

The `VmServiceDisconnected` handler in `update.rs` uses `if let Some(ref tx)` + separate `= None`, while all other sites (including these new blocks) use the more idiomatic `.take()` pattern.

**Recommendation:** Harmonize `update.rs` to use `.take()` in a follow-up commit. The new code is the correct model.

---

### Finding 4: `let _ = tx.send(true)` Lacks Justification Comment (NITPICK)

**Raised by:** Code Quality Inspector
**Severity:** NITPICK

`CODE_STANDARDS.md` lists `let _ = do_something()` as an anti-pattern. In this context, `watch::Sender::send` only returns `Err` when all receivers are dropped (task already exited), making the discard intentionally correct. However, all six `let _ = tx.send(true)` sites in the cleanup code lack comments explaining this.

**Recommendation:** Not blocking. Consider adding an inline comment in a future cleanup pass.

---

### Finding 5: TEA Purity — Side Effects in Handlers (OBSERVATION — Pre-existing)

**Raised by:** Architecture Enforcer
**Severity:** Observation only

The handlers execute `.abort()` and `.send()` as direct side effects rather than returning an `UpdateAction`. This is a pre-existing, codebase-wide compromise that applies equally to all cleanup sites. The new code correctly follows the established pattern. A future `UpdateAction::TeardownSession` variant would be cleaner but is out of scope.

---

## Quality Gate Results

| Check | Result |
|-------|--------|
| `cargo fmt --all --check` | PASS |
| `cargo clippy --workspace -- -D warnings` | PASS |
| `cargo test --workspace` | PASS (2,759 passed, 0 failed, 74 ignored) |

---

## Conclusion

The fix is correct, complete, minimal in scope, and consistent with established patterns. All five session termination paths now have network cleanup coverage. The three non-blocking concerns are documentation improvements (comments explaining design decisions) rather than code defects.

**Approved for merge.**
