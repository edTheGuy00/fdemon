# Code Review: Profile Mode Lag Fix (Issue #25) — Phase 3

**Date:** 2026-03-19
**Branch:** `fix/profile-mode-lag-25`
**Base:** `0f203b5` (Phase 2 complete + Phase 3 plan)
**Change Type:** Bug Fix — Panel-Aware Monitoring Lifecycle
**Files Changed:** 12 source files, ~2400 lines added
**Commits:** 4 (af6cadc, c108fd0, fb9e2f0, 4d0b728)

## Verdict: APPROVED WITH CONCERNS

The core bug (excessive VM Service polling while viewing logs) is fully addressed. Performance monitoring is lazy-started on first DevTools entry, paused on exit. Network monitoring pauses when leaving the Network tab. Two functional gaps exist around session switching and first-entry-with-Network-default — both are UX-only (stale data, not crashes or regressions) and suitable for a follow-up task.

### Agent Verdicts

| Agent | Verdict | Critical | Major | Minor | Nitpick |
|-------|---------|----------|-------|-------|---------|
| Bug Fix Reviewer | APPROVED | 0 | 0 | 2 | 3 |
| Architecture Enforcer | PASS | 0 | 0 | 2 | 1 |
| Code Quality Inspector | APPROVED | 0 | 0 | 5 | 3 |
| Logic & Reasoning Checker | CONCERNS | 0 | 0 | 2 | 3 |
| Security Reviewer | PASS | 0 | 0 | 3 (pre-existing) | 1 |

---

## Summary

Phase 3 eliminates all VM Service polling RPCs when the user is not viewing DevTools, achieving the target of zero background overhead while viewing logs:

1. **`perf_pause_tx` channel** — Gates both `memory_tick` and `alloc_tick` arms; paused outside DevTools
2. **`network_pause_tx` channel** — Gates `poll_tick` arm; paused outside Network tab
3. **Lazy-start** — Performance monitoring deferred from `VmServiceConnected` to first DevTools entry
4. **Immediate fetch on unpause** — `changed()` arms fire one RPC on resume so panels show fresh data
5. **Session switch coverage** — `maybe_start_monitoring_for_selected_session` starts perf monitoring for new sessions while in DevTools

Combined with Phase 2 (interval scaling, alloc gating), the full fix reduces profile-mode polling from ~8 RPCs/sec to zero when viewing logs, and ~1.5 RPCs/sec when in DevTools.

---

## Consolidated Findings

### WARNING-1: Session switch does not unpause existing paused tasks

**Files:** `crates/fdemon-app/src/handler/session_lifecycle.rs:131-163`
**Source:** bug_fix_reviewer, logic_reasoning_checker

`maybe_start_monitoring_for_selected_session` only starts monitoring when `perf_shutdown_tx.is_none()`. If switching back to a session that already has a running (but paused) perf task, the function returns without unpausing. The user sees stale performance data until they exit and re-enter DevTools. Same gap exists for network monitoring when the active panel is Network.

**Impact:** UX-only — stale data on session switch, no crashes or data loss.
**Recommendation:** After the `!needs_start` early return, send `false` on `perf_pause_tx` (and `network_pause_tx` if `active_panel == Network`) to unpause the existing task. File as follow-up task.

---

### WARNING-2: First DevTools entry with Network default panel doesn't start network monitoring

**Files:** `crates/fdemon-app/src/handler/devtools/mod.rs:147-169`
**Source:** logic_reasoning_checker

When `handle_enter_devtools_mode` takes the `needs_perf_start` path, it returns `StartPerformanceMonitoring` as the action but does NOT return `StartNetworkMonitoring`. The network unpause signal sent at line 199 targets a `None` sender (task never started), so it's a no-op. The user sees an empty Network panel until they switch away and back.

**Impact:** UX-only — empty Network panel on first DevTools entry with Network as default.
**Recommendation:** Emit a follow-up `Message::SwitchDevToolsPanel(Network)` after the lazy-start action completes, so `handle_switch_panel`'s Network arm fires `StartNetworkMonitoring`. File as follow-up task.

---

### MINOR-1: `VmServiceReconnected` does not clear `network_pause_tx`

**Files:** `crates/fdemon-app/src/handler/update.rs:1462-1468`
**Source:** code_quality_inspector, logic_reasoning_checker

`VmServiceConnected` (line 1348) and `VmServiceDisconnected` (line 1587) both clear all three pause senders. `VmServiceReconnected` clears `alloc_pause_tx` and `perf_pause_tx` but not `network_pause_tx`. The stale sender points to a dead receiver — functionally harmless but inconsistent with the cleanup invariant.

**Recommendation:** Add `handle.network_pause_tx = None;` after line 1467.

---

### MINOR-2: `handle_close_current_session` doesn't clear pause senders

**Files:** `crates/fdemon-app/src/handler/session_lifecycle.rs:184-218`
**Source:** code_quality_inspector

The function's comment says it mirrors `VmServiceDisconnected`, but it doesn't clear `perf_pause_tx`, `alloc_pause_tx`, or `network_pause_tx`. The session is removed immediately after so the handles are dropped — no actual resource leak, but pattern inconsistency.

**Recommendation:** Add three `None` assignments before session removal for consistency.

---

### MINOR-3: Duplicated memory fetch logic (~25 lines)

**Files:** `crates/fdemon-app/src/actions/performance.rs:328-384`
**Source:** code_quality_inspector, bug_fix_reviewer

The `perf_pause_rx.changed()` arm duplicates the `memory_tick` arm's `getMemoryUsage` + `VmServiceMemorySnapshot` + `VmServiceMemorySample` sequence. At 651 lines, `performance.rs` exceeds the 500-line file guideline.

**Recommendation:** Extract a `fetch_and_send_memory_snapshot()` async helper.

---

### MINOR-4: `handle_enter_devtools_mode` is 111 lines

**Files:** `crates/fdemon-app/src/handler/devtools/mod.rs:113-223`
**Source:** code_quality_inspector

Exceeds the 50-line function guideline, driven by repeated `session_manager.selected()` calls for three separate pause channels.

**Recommendation:** Extract pause-signal dispatch into a helper.

---

### MINOR-5: TEA exception not documented at watch channel send call sites

**Files:** `crates/fdemon-app/src/handler/devtools/mod.rs` (multiple sites)
**Source:** architecture_enforcer

`tx.send()` calls inside `update()` delegate handlers are side effects in a nominally pure function. The architecture docs acknowledge the pattern at the module level, but individual call sites lack the exception annotation.

**Recommendation:** Add `// TEA exception: watch channel send — see docs/ARCHITECTURE.md §Panel State Model` at each call site.

---

### MINOR-6: Module `//!` header not updated for pause channel architecture

**Files:** `crates/fdemon-app/src/handler/devtools/mod.rs:1-10`
**Source:** code_quality_inspector

The module header doesn't mention the pause channel pattern or lazy-start behavior.

**Recommendation:** Update the module header to describe monitoring lifecycle ownership.

---

### NITPICK-1: `unwrap()` without justification comments

**Files:** `handler/devtools/mod.rs:136`, `handler/session_lifecycle.rs:146`
**Source:** code_quality_inspector

Both `selected_id().unwrap()` calls are safe (guarded by prior `selected()` check) but lack the justification comment required by CODE_STANDARDS.md.

### NITPICK-2: Inconsistent mutex error handling idioms

**Files:** `actions/performance.rs:422` vs `actions/network.rs:309-311`
**Source:** code_quality_inspector, architecture_enforcer, security_reviewer

`performance.rs` uses `if let Ok(mut slot)` while `network.rs` uses `.lock().map()`. Both silently discard poison errors. Pick one style and add `tracing::warn!` on failure. (Carried forward from Phase 2 review MINOR-5.)

### NITPICK-3: Tautological disconnect tests

**Source:** bug_fix_reviewer (from validation phase)

`test_perf_pause_cleared_on_disconnect` and `test_network_pause_cleared_on_disconnect` manually set fields to `None` and assert `None` — they don't exercise the actual `update()` handler path.

---

## Pre-Existing Findings (Not Introduced by Phase 3)

The security reviewer flagged 3 medium and 1 low finding in code touched by this diff but not introduced by it:
- **Browser config unsanitized** (`actions/network.rs:417`) — `settings.devtools.browser` passed directly to `Command::new`
- **Unbounded filter input buffer** (`handler/devtools/network.rs:270-274`) — no length cap
- **Poisoned mutex silent discard** (`actions/network.rs:309`, `actions/performance.rs:422`) — carried from Phase 2
- **Raw VM error string stored without normalization** (`handler/devtools/network.rs:69-78`)

These are tracked separately from Phase 3 findings.

---

## Documentation Freshness

| Document | Update Needed? | Reason |
|----------|---------------|--------|
| `docs/ARCHITECTURE.md` | Done (Task 04) | Updated SessionHandle diagram, Panel State Model, VM Service Data Flow |
| `docs/DEVELOPMENT.md` | No | No new build steps or dependencies |
| `docs/CODE_STANDARDS.md` | No | No new patterns established |
| `docs/REVIEW_FOCUS.md` | Recommended | Watch channel sends in `update()` should be documented as approved TEA exception (carried from Phase 2 MINOR-8) |

---

## Regression Risk

**Risk Level:** Low

- Initial pause values (`true` for perf/alloc, `false` for network) ensure zero RPCs fire before explicit unpause
- Watch channel coalescing handles rapid panel switching safely
- Frame timing (push events from VM extension stream) is completely unaffected
- The `VmServicePerformanceMonitoringStarted` handler's post-store unpause eliminates the lazy-start timing race
- Debug mode behavior is unchanged (same polling lifecycle, now panel-gated)
- All existing tests pass; 25+ new tests cover pause/resume/lazy-start paths

---

## Test Coverage

| Task | New Tests | Coverage |
|------|-----------|----------|
| 01-pause-perf-when-not-devtools | 5 | Enter/exit DevTools, disconnect, panel switch |
| 02-pause-network-on-tab-switch | 8 | Panel switch, DevTools entry/exit, disconnect, Network default |
| 03-lazy-start-monitoring | 13 | Lazy start, VM reconnect, session switch, pause adjustment |
| 04-update-docs | 0 | Documentation only |
| **Total** | **26** | |

**Missing coverage:** Session switch back to a paused running task (WARNING-1); first DevTools entry with Network default (WARNING-2).

---

## Files Modified

| File | Tasks | Lines Changed |
|------|-------|---------------|
| `crates/fdemon-app/src/actions/performance.rs` | 01, 03 | +502 |
| `crates/fdemon-app/src/actions/network.rs` | 02 | +198 |
| `crates/fdemon-app/src/handler/devtools/mod.rs` | 01, 02, 03 | +709 |
| `crates/fdemon-app/src/handler/devtools/network.rs` | 02 | +19 |
| `crates/fdemon-app/src/handler/mod.rs` | 03 | +10 |
| `crates/fdemon-app/src/handler/session_lifecycle.rs` | 03 | +49 |
| `crates/fdemon-app/src/handler/tests.rs` | 01, 02, 03 | +658 |
| `crates/fdemon-app/src/handler/update.rs` | 01, 02, 03 | +123 |
| `crates/fdemon-app/src/message.rs` | 01, 02 | +29 |
| `crates/fdemon-app/src/process.rs` | 03 | +6 |
| `crates/fdemon-app/src/session/handle.rs` | 01, 02 | +48 |
| `docs/ARCHITECTURE.md` | 04 | +9 |

---

## Sign-off

- **Reviewed by:** 5 specialized agents (bug_fix, architecture, code_quality, logic, security)
- **Overall Verdict:** APPROVED WITH CONCERNS
- **Blocking Issues:** 0
- **Warnings:** 2 (session switch unpause gap, Network default panel gap)
- **Action Required:** 2 warnings recommended as follow-up task; 6 minor items for cleanup
