# Phase 3: Performance & Memory Monitoring — Code Review

**Date:** 2026-02-19
**Branch:** `feat/devtools`
**Base Commit:** `68207c3` (chore: devtool phase 3 docs)
**Reviewers:** architecture_enforcer, code_quality_inspector, logic_reasoning_checker, risks_tradeoffs_analyzer

---

## Verdict: NEEDS WORK

Multiple agents raised significant concerns. One blocking issue (resource leak on session close) and several major code quality issues must be addressed before merge.

---

## Change Summary

**17 files changed**, 1,645 insertions(+), 89 deletions(-)

### New Files (3)
| File | Lines | Purpose |
|------|-------|---------|
| `crates/fdemon-core/src/performance.rs` | 386 | Domain types: MemoryUsage, GcEvent, AllocationProfile, ClassHeapStats, FrameTiming, PerformanceStats, RingBuffer\<T\> |
| `crates/fdemon-daemon/src/vm_service/performance.rs` | 330 | getMemoryUsage, getAllocationProfile RPCs, GC event parsing |
| `crates/fdemon-daemon/src/vm_service/timeline.rs` | 293 | Flutter.Frame event parsing, frame timing extraction, enable_frame_tracking |

### Modified Files (14)
| File | Delta | Purpose |
|------|-------|---------|
| `crates/fdemon-app/src/session.rs` | +316 | PerformanceState struct, stats computation, FPS calculation |
| `crates/fdemon-app/src/handler/tests.rs` | +451 | 9+ new perf-related handler tests |
| `crates/fdemon-app/src/actions.rs` | +170 | spawn_performance_polling, GC/frame forwarding, enable_frame_tracking |
| `crates/fdemon-app/src/handler/update.rs` | +86 | Handlers for 5 new Message variants |
| `crates/fdemon-app/src/message.rs` | +58 | 5 new Message variants for perf data |
| `crates/fdemon-app/src/process.rs` | +57 | Two-phase handle hydration for StartPerformanceMonitoring |
| `crates/fdemon-daemon/src/vm_service/client.rs` | +307/-89 | VmRequestHandle extraction, delegation pattern |
| `crates/fdemon-daemon/src/vm_service/mod.rs` | +14 | Module exports for performance + timeline |
| `crates/fdemon-core/src/lib.rs` | +5 | performance module export |
| `crates/fdemon-app/src/handler/mod.rs` | +19 | StartPerformanceMonitoring action variant |

---

## Agent Reports

### 1. Architecture Enforcer — WARNING

**Layer boundaries:** Respected. `fdemon-core` types flow up through `fdemon-daemon` parsing into `fdemon-app` TEA state. No downward dependencies introduced.

**VmRequestHandle pattern:** Well-designed. Clone + Send + Sync, delegates to internal channel. Fits the existing architecture cleanly.

**Two-phase handle hydration:** Correct approach for keeping `handler::update()` pure. `process.rs` hydrates from `AppState` before dispatch.

**Issues found:**
1. **Submodule path access** — `actions.rs` imports `fdemon_daemon::vm_service::timeline::enable_frame_tracking` directly instead of using the re-exported path from `vm_service` module root. Violates the project's convention of importing from the crate's public API surface.
2. **JoinHandle discarded** — `spawn_performance_polling()` returns `JoinHandle<()>` but it's dropped in `handle_action`. Should be tracked in `session_tasks` for clean shutdown and error propagation.
3. **session.rs at 2,731 lines** — 5.4x the 500-line limit from CODE_STANDARDS.md. Contains 5 distinct types (LogBatcher, LogBlockState, CollapseState, PerformanceState, Session, SessionHandle) that should be extracted into a `session/` module directory.

### 2. Code Quality Inspector — NEEDS WORK

**Issues found:**
1. **Unused parameter** — `compute_stats(&self, _memory: &RingBuffer<MemoryUsage>)` has `_memory` parameter on a public method. Either use it or remove it.
2. **`frames.iter().count()` instead of `frames.len()`** — O(n) iteration where O(1) length check is available. `RingBuffer` wraps `VecDeque` which has `len()`.
3. **`calculate_fps` misnomer** — Returns frame count within window, not frames-per-second rate. Should either compute actual FPS or rename to `count_frames_in_window`.
4. **JoinHandle discarded** — Same as architecture finding. Fire-and-forget task without error tracking.
5. **Dead branch** — In `compute_stats`, the `frame_times.is_empty()` check after `frames.iter().count() > 0` is unreachable since frame_times is populated from the same iterator.
6. **`total_frames` misleading** — Field name suggests lifetime count but actually holds ring buffer's current length (bounded by capacity).
7. **Broken doc comment** — `/ Returns` instead of `/// Returns` on one function (missing leading slash).

### 3. Logic & Reasoning Checker — CONCERNS

**Verified correct:**
- Percentile calculation (ceiling-based index) is correct
- Handle hydration safety (None -> Some pattern)
- RingBuffer wrap-around behavior
- GC event parsing null safety

**Issues found:**
1. **Isolate ID cache not invalidated on hot restart** — `VmRequestHandle` caches `main_isolate_id` via `OnceLock`. A hot restart creates a new isolate with a different ID, but the cache retains the stale value. RPCs using the cached ID would target a dead isolate.
2. **FPS calculation clock domain** — `calculate_fps` compares `FrameTiming.start_time` (VM microsecond timestamps) against `Instant::now()` minus a Duration. These are different clock domains. Currently works because only recent frames are in the buffer, but the comparison is technically incorrect.
3. **`total_frames` semantics** — Same as code quality finding. The field's name doesn't match its behavior.

### 4. Risks & Tradeoffs Analyzer — CONCERNS (1 blocking)

**BLOCKING:**
1. **`perf_shutdown_tx` not signaled on session close** — In `handler/session_lifecycle.rs`, when a session is removed, `perf_shutdown_tx` is not signaled. The polling task continues running after the session is destroyed, sending messages to a dropped channel. This is a resource leak that scales with session creation/destruction.

**High Risk:**
2. **GC ring buffer drowning** — Dart VM emits frequent `Scavenge` GC events (young generation). At high allocation rates, the 100-slot GC ring buffer may fill entirely with minor GCs, pushing out more informative `MarkSweep` (major GC) events. Consider filtering or separating by GC type.

**Medium Risk:**
3. **Frame timing backpressure** — No throttling on frame timing message volume. If the Flutter app runs at high frame rates with complex frames, the message channel could see sustained high throughput.
4. **session.rs tech debt** — At 5.4x the line limit, further Phase 4 additions will compound the maintenance burden.

---

## Test Results

| Crate | Tests | Status |
|-------|-------|--------|
| fdemon-core | 253 | PASS |
| fdemon-daemon | 147 | PASS |
| fdemon-app | 792 | PASS |
| fdemon-tui | 427 | PASS |
| **Total** | **1,619** | **PASS** |

E2E test failures (25) are pre-existing settings page timeout issues unrelated to Phase 3.

---

## What Went Well

- **Clean layer separation** — New performance types in fdemon-core, parsing in fdemon-daemon, TEA integration in fdemon-app. No boundary violations.
- **VmRequestHandle design** — Elegant delegation pattern that shares the connection without exposing internals. Clone + Send + Sync enables ergonomic concurrent use.
- **Two-phase hydration** — Keeps `handler::update()` pure while enabling async side effects. Good TEA discipline.
- **Comprehensive test coverage** — 451 lines of new handler tests, mock JSON responses for all RPC parsers, ring buffer edge cases covered.
- **Existing tests unbroken** — All 1,619 unit tests pass. No regressions.
- **Flutter.Frame approach** — Parsing Extension stream events rather than raw Timeline chrome trace is the correct and documented approach.

---

## Overall Assessment

Phase 3 successfully implements the data pipeline for performance monitoring. The architecture is sound and the test coverage is strong. However, one blocking resource leak (shutdown signaling) and several code quality issues prevent immediate approval. The session.rs size is a growing concern that should be addressed alongside the bug fixes.
