# Action Items: DevTools Integration Phase 3

**Review Date:** 2026-02-19
**Verdict:** NEEDS WORK
**Blocking Issues:** 1
**Critical Issues:** 3
**Major Issues:** 5
**Minor Issues:** 3

---

## Blocking Issues (Must Fix Before Merge)

### 1. `perf_shutdown_tx` not signaled on session close
- **Source:** risks_tradeoffs_analyzer
- **File:** `crates/fdemon-app/src/handler/session_lifecycle.rs` (session removal path)
- **Problem:** When a session is removed/closed, `perf_shutdown_tx` (the `watch::Sender<bool>` that stops the polling task) is never signaled. The background polling task continues running, sending `VmServiceMemorySnapshot` messages to a channel whose receiver has been dropped. This is a resource leak that accumulates with each session create/destroy cycle.
- **Required Action:** In the session close/remove handler, call `perf_shutdown_tx.send(true)` before dropping the `SessionHandle`. This mirrors the disconnect handler pattern already in `update.rs`.
- **Acceptance:** Create a test that verifies polling task terminates when session is removed. Verify no tokio task leak after creating and destroying 3+ sessions.

---

## Critical Issues (Must Fix)

### 2. JoinHandle from `spawn_performance_polling` discarded
- **Source:** architecture_enforcer, code_quality_inspector
- **File:** `crates/fdemon-app/src/actions.rs`
- **Problem:** `spawn_performance_polling()` returns `JoinHandle<()>` but the return value is dropped in `handle_action`. If the polling task panics, it silently disappears. The handle should be tracked for clean shutdown and error propagation.
- **Required Action:** Store the JoinHandle in `SessionHandle.session_tasks` (or equivalent), consistent with how other background tasks are managed. On shutdown, abort or await the handle.
- **Acceptance:** JoinHandle stored and cleaned up on session close.

### 3. Isolate ID cache not invalidated on hot restart
- **Source:** logic_reasoning_checker
- **File:** `crates/fdemon-daemon/src/vm_service/client.rs` (VmRequestHandle)
- **Problem:** `VmRequestHandle` caches `main_isolate_id` via `OnceLock`. A hot restart creates a new Dart isolate with a different ID, but the cache retains the stale value. Any RPC using the cached isolate ID would target a dead isolate, causing silent failures.
- **Required Action:** Either (a) replace `OnceLock` with `RwLock`/`Mutex` and add an `invalidate_isolate_cache()` method called on hot restart, or (b) re-fetch the isolate ID on each use (more reliable, slight overhead). Option (a) preferred for performance.
- **Acceptance:** After hot restart, `main_isolate_id()` returns the new isolate's ID.

### 4. Unused `_memory` parameter on public `compute_stats`
- **Source:** code_quality_inspector
- **File:** `crates/fdemon-app/src/session.rs` (~line 273)
- **Problem:** `compute_stats(&self, _memory: &RingBuffer<MemoryUsage>)` accepts a memory parameter but ignores it (prefixed with `_`). Public API with unused parameter is a code smell.
- **Required Action:** Either integrate memory stats into the computation (was this the intent?) or remove the parameter.
- **Acceptance:** No unused parameters on public methods. Clippy passes.

---

## Major Issues (Should Fix)

### 5. `calculate_fps` returns count, not rate
- **Source:** code_quality_inspector, logic_reasoning_checker
- **File:** `crates/fdemon-app/src/session.rs` (~line 308)
- **Problem:** `calculate_fps()` returns the count of frames within a 1-second window, not a true frames-per-second rate. While numerically similar for 1s windows, the function name is misleading and the implementation compares incompatible clock domains (VM timestamps vs `Instant::now()`).
- **Suggested Action:** Either rename to `count_recent_frames()` or compute actual FPS as `frame_count / actual_elapsed_seconds`. Fix the clock domain issue by using frame timestamps exclusively (e.g., latest frame time minus earliest frame time in window).

### 6. `frames.iter().count()` instead of `frames.len()`
- **Source:** code_quality_inspector
- **File:** `crates/fdemon-app/src/session.rs`
- **Problem:** O(n) iteration to count elements when O(1) `.len()` is available on `RingBuffer` (backed by `VecDeque`).
- **Suggested Action:** Replace with `frames.len()` (add `len()` method to `RingBuffer` if not present).

### 7. Dead branch in `compute_stats`
- **Source:** code_quality_inspector
- **File:** `crates/fdemon-app/src/session.rs`
- **Problem:** `frame_times.is_empty()` check after `frames.iter().count() > 0` is unreachable — if count > 0, frame_times will always have elements.
- **Suggested Action:** Remove the dead branch or restructure the logic to avoid the redundant check.

### 8. `total_frames` misleading semantics
- **Source:** code_quality_inspector, logic_reasoning_checker
- **File:** `crates/fdemon-core/src/performance.rs` (PerformanceStats) and `crates/fdemon-app/src/session.rs`
- **Problem:** `total_frames` is set to the ring buffer's current size, not a lifetime count of frames seen. The name suggests cumulative counting.
- **Suggested Action:** Either rename to `buffered_frames` / `frame_count` (to reflect buffer size) or add a separate counter that increments on each frame event and tracks lifetime total.

### 9. Submodule path access in actions.rs
- **Source:** architecture_enforcer
- **File:** `crates/fdemon-app/src/actions.rs`
- **Problem:** Imports `fdemon_daemon::vm_service::timeline::enable_frame_tracking` reaching into a submodule instead of using the re-exported path.
- **Suggested Action:** Use `fdemon_daemon::vm_service::enable_frame_tracking` (the re-export from `vm_service/mod.rs`).

---

## Minor Issues (Consider Fixing)

### 10. Broken doc comment
- **Source:** code_quality_inspector
- **File:** One function has `/ Returns` instead of `/// Returns` (missing leading slash).
- **Suggested Action:** Fix the triple-slash.

### 11. GC ring buffer drowning by Scavenge events
- **Source:** risks_tradeoffs_analyzer
- **Problem:** High-frequency minor GC (Scavenge) events could fill the 100-slot ring buffer, pushing out more informative MarkSweep events.
- **Suggested Action:** Consider filtering to only store major GC events, or separate ring buffers by GC type. Low priority for Phase 3 but worth addressing in Phase 4.

### 12. session.rs at 2,731 lines (5.4x limit)
- **Source:** architecture_enforcer, risks_tradeoffs_analyzer
- **Problem:** CODE_STANDARDS.md sets a 500-line soft limit. Phase 4 will add more to this file.
- **Suggested Action:** Split into `session/` module directory. See session.rs refactoring plan below.

---

## session.rs Refactoring Plan

The file contains 6 distinct logical units that should be extracted into a `session/` module directory:

### Proposed Structure

```
crates/fdemon-app/src/session/
├── mod.rs                # Re-exports, SessionId, next_session_id()
├── session.rs            # Session struct + impl (core state, log management)
├── session_handle.rs     # SessionHandle struct + impl (process ownership)
├── log_batcher.rs        # LogBatcher struct + impl + constants
├── log_block.rs          # LogBlockState struct + impl
├── collapse_state.rs     # CollapseState struct + impl
├── performance.rs        # PerformanceState struct + impl + constants
└── tests.rs              # All session-related tests
```

### Extraction Boundaries

| Unit | Current Lines | Description |
|------|--------------|-------------|
| LogBatcher | 37-108 (~72 lines) | Log batching with flush interval + max size |
| LogBlockState | 110-130 (~21 lines) | Stack trace block tracking |
| CollapseState | 132-189 (~58 lines) | Collapsible section state |
| PerformanceState | 191-332 (~142 lines) | Memory/GC/frame history, stats computation |
| Session | 334-1025 (~692 lines) | Core session state and log management |
| SessionHandle | 1027-1106 (~80 lines) | Process handle, command sender, request tracker |
| Tests | 1108-2731 (~1,624 lines) | All unit tests |

### Migration Steps

1. Create `crates/fdemon-app/src/session/` directory
2. Move `LogBatcher` + constants to `session/log_batcher.rs`
3. Move `LogBlockState` to `session/log_block.rs`
4. Move `CollapseState` to `session/collapse_state.rs`
5. Move `PerformanceState` + constants to `session/performance.rs`
6. Move `SessionHandle` to `session/session_handle.rs`
7. Move `Session` to `session/session.rs`
8. Move tests to `session/tests.rs`
9. Create `session/mod.rs` with re-exports (maintain backward compatibility)
10. Verify all imports across the workspace still resolve
11. Run `cargo test --workspace` + `cargo clippy --workspace`

**Risk:** Low. Pure structural refactor with re-exports preserving the public API.

---

## Re-review Checklist

After addressing issues, the following must pass:

- [ ] Blocking issue #1 resolved (shutdown signaling)
- [ ] Critical issues #2-#4 resolved
- [ ] Major issues #5-#9 resolved or justified
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace -- -D warnings` passes
- [ ] `cargo fmt --all -- --check` passes
- [ ] No new E2E test failures
