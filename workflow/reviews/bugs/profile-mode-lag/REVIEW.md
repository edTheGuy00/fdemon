# Code Review: Profile Mode Lag Fix (Issue #25) — Phase 2

**Date:** 2026-03-19
**Branch:** `fix/profile-mode-lag-25`
**Base:** `main` (`7d5f648`)
**Change Type:** Bug Fix
**Files Changed:** 12 source files, ~1142 lines added

## Verdict: APPROVED WITH CONCERNS

All 5 reviewer agents approved the implementation. No critical or major issues found. Several minor concerns are noted below for tracking. The fix correctly addresses the root cause and all acceptance criteria are met.

### Agent Verdicts

| Agent | Verdict | Critical | Major | Minor | Nitpick |
|-------|---------|----------|-------|-------|---------|
| Bug Fix Reviewer | APPROVED | 0 | 0 | 1 | 2 |
| Architecture Enforcer | PASS | 0 | 0 | 2 | 1 |
| Code Quality Inspector | APPROVED | 0 | 0 | 3 | 5 |
| Logic & Reasoning Checker | PASS | 0 | 0 | 2 | 2 |
| Security Reviewer | PASS | 0 | 0 | 0 | 4 |

---

## Summary

This fix reduces VM Service RPC pressure in profile/release mode from ~8 RPCs/sec to ~1.5 RPCs/sec through five layered optimizations:

1. **Dedup `getMemoryUsage`** — eliminates redundant RPC call (3 RPCs/tick -> 2)
2. **`MissedTickBehavior::Skip`** — prevents burst recovery after slow RPCs
3. **Thread `FlutterMode`** — plumbs build mode from config through the monitoring chain
4. **Scale intervals by mode** — 3x multiplier with profile-mode minimums (mem: 2000ms, alloc: 5000ms, net: 3000ms)
5. **Gate alloc on panel** — `getAllocationProfile` only fires when Performance panel is visible

Debug mode behavior is provably unchanged. All 1824 tests pass.

---

## Consolidated Findings

### MINOR-1: Use `saturating_mul` for interval multiplication
**Files:** `actions/performance.rs:92`, `actions/network.rs:75`
**Source:** security_reviewer, logic_reasoning_checker

`clamped * PROFILE_MODE_MULTIPLIER` uses plain `u64` multiplication. With the hardcoded multiplier of 3, overflow is practically impossible (~195 million years). However, code comments note the multiplier may become user-configurable. Using `saturating_mul` costs nothing and is consistent with `saturating_sub` usage elsewhere in the codebase.

**Recommendation:** Replace with `clamped.saturating_mul(PROFILE_MODE_MULTIPLIER)`.

---

### MINOR-2: Duplicated `PROFILE_MODE_MULTIPLIER` constant
**Files:** `actions/performance.rs:55`, `actions/network.rs:49`
**Source:** architecture_enforcer, code_quality_inspector, logic_reasoning_checker, bug_fix_reviewer

Both modules independently define `const PROFILE_MODE_MULTIPLIER: u64 = 3`. If one is changed during a future tuning pass, the other silently diverges. The code comments acknowledge this will become configurable.

**Recommendation:** Extract to a shared constant in `actions/mod.rs` and import from both submodules.

---

### MINOR-3: Duplicated `getAllocationProfile` dispatch block (~50 lines)
**Files:** `actions/performance.rs:295-345` (alloc_tick arm), `actions/performance.rs:347-399` (alloc_pause_rx.changed arm)
**Source:** code_quality_inspector

Both arms contain the same fetch-parse-send-break logic. Any future change to allocation fetching must be applied in both places.

**Recommendation:** Extract a local async helper to eliminate the duplication.

---

### MINOR-4: Silent discard of `alloc_pause_tx` send errors
**Files:** `handler/devtools/mod.rs:118,152,184,217`
**Source:** security_reviewer, architecture_enforcer

`let _ = tx.send(...)` discards the `Result`. If the receiver is dropped (polling task exited unexpectedly), the pause/unpause signal is lost. The stale value could leave allocation polling in an unintended state.

**Recommendation:** Add a `tracing::debug!` log when `send` returns `Err`.

---

### MINOR-5: Poisoned mutex on `task_handle_slot` silently discarded
**Files:** `actions/performance.rs:417`, `actions/network.rs:257`
**Source:** code_quality_inspector, security_reviewer

Both `task_handle_slot.lock()` calls silently ignore `PoisonError`, leaving the `JoinHandle` undelivered. The polling task becomes unabortable (though shutdown channel still works). Additionally, the two files use inconsistent idioms (`if let Ok` vs `.lock().map()`).

**Recommendation:** Add `tracing::warn!` on poison, consistent with `actions/mod.rs` patterns.

---

### MINOR-6: Dead `else` branch on `get_memory_sample_from_usage`
**Files:** `actions/performance.rs:287-292`
**Source:** bug_fix_reviewer

`get_memory_sample_from_usage` always returns `Some(...)`, but the caller handles `None` with a debug log that will never fire. This could mislead future debugging.

**Recommendation:** Add a comment noting the branch is unreachable, or change the return type to `MemorySample`.

---

### MINOR-7: Duplicated `parse_isolate_rss` body
**Files:** `daemon/vm_service/performance.rs:153-180` (get_isolate_rss), `daemon/vm_service/performance.rs:187-203` (parse_isolate_rss)
**Source:** code_quality_inspector

The `#[cfg(test)]` helper replicates the JSON traversal logic verbatim.

**Recommendation:** Extract a shared `fn extract_isolate_rss(value: &serde_json::Value) -> Option<u64>`.

---

### MINOR-8: TEA exception for watch channel sends should be documented
**Files:** `handler/devtools/mod.rs`, `docs/REVIEW_FOCUS.md`
**Source:** architecture_enforcer

`alloc_pause_tx.send()` calls inside `update()` are side effects in a pure function. The same pattern exists for `perf_shutdown_tx` already, but neither is documented as an approved TEA exception in `REVIEW_FOCUS.md`.

**Recommendation:** Add an "Approved TEA Exception: Watch Channel Signals" subsection to `docs/REVIEW_FOCUS.md`.

---

### NITPICK-1: `alloc_pause_rx.changed()` sender drop comment slightly misleading
**Files:** `handler/update.rs:1543-1544`
**Source:** logic_reasoning_checker

Comment says "the shutdown arm handles the clean exit" when the alloc_pause arm's `changed()` returns `Err` — this is correct but could be clearer that the `Ok(()) = ...` pattern simply stops matching.

### NITPICK-2: Consider `SessionHandle::shutdown_performance_monitoring()` method
**Source:** architecture_enforcer

Pre-existing gap: cleanup of `perf_shutdown_tx`, `perf_task_handle`, and now `alloc_pause_tx` is inline in `update.rs` rather than in a method on `SessionHandle` (unlike `shutdown_native_logs()`).

### NITPICK-3: Triple `.unwrap()` chains in tests
**Files:** `handler/devtools/mod.rs:695-700`
**Source:** code_quality_inspector

Could use `.expect("...")` for clearer failure messages.

---

## Documentation Freshness

| Document | Update Needed? | Reason |
|----------|---------------|--------|
| `docs/ARCHITECTURE.md` | No | No new modules, crates, or structural changes |
| `docs/DEVELOPMENT.md` | No | No new build steps or dependencies |
| `docs/CODE_STANDARDS.md` | No | No new patterns established |
| `docs/REVIEW_FOCUS.md` | Yes (MINOR-8) | Watch channel sends in `update()` should be documented as approved TEA exception |

---

## Regression Risk

**Risk Level:** Low

- Debug mode behavior is provably unchanged (`FlutterMode::Debug` arm returns base-clamped interval)
- Allocation polling starts paused — strictly better than unconditional polling
- `MissedTickBehavior::Skip` is strictly better than `Burst` for polling loops
- `alloc_pause_tx` lifecycle mirrors established `perf_shutdown_tx` pattern
- All 1824 tests pass; 27 new tests cover the new functionality

---

## Test Coverage

| Task | New Tests | Coverage |
|------|-----------|----------|
| 01-dedup-memory-rpc | 3 | Field mapping verification |
| 02-missed-tick-skip | 0 | Verified by inspection (tokio internals) |
| 03-thread-flutter-mode | 3 | Mode extraction and threading |
| 04-scale-intervals-by-mode | 16 | All mode/interval combinations |
| 05-gate-alloc-on-panel | 8 | All pause/unpause transitions |
| **Total** | **30** | |

---

## Files Modified

| File | Tasks | Lines Changed |
|------|-------|---------------|
| `crates/fdemon-daemon/src/vm_service/performance.rs` | 01 | +155 |
| `crates/fdemon-daemon/src/vm_service/mod.rs` | 01 | +4/-1 |
| `crates/fdemon-app/src/actions/performance.rs` | 01,02,03,04,05 | +395/-18 |
| `crates/fdemon-app/src/actions/network.rs` | 02,03,04 | +146/-5 |
| `crates/fdemon-app/src/actions/mod.rs` | 03 | +11/-3 |
| `crates/fdemon-app/src/handler/mod.rs` | 03 | +10/-4 |
| `crates/fdemon-app/src/handler/update.rs` | 03,05 | +31 |
| `crates/fdemon-app/src/handler/devtools/mod.rs` | 03,05 | +233/-3 |
| `crates/fdemon-app/src/handler/tests.rs` | 03,05 | +128/-4 |
| `crates/fdemon-app/src/process.rs` | 03 | +6 |
| `crates/fdemon-app/src/session/handle.rs` | 05 | +15 |
| `crates/fdemon-app/src/message.rs` | 05 | +10 |

---

## Sign-off

- **Reviewed by:** 5 specialized agents (bug_fix, architecture, code_quality, logic, security)
- **Overall Verdict:** APPROVED WITH CONCERNS
- **Blocking Issues:** 0
- **Action Required:** 8 minor items recommended for follow-up (non-blocking)
