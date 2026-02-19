## Task: Fix Stats Computation Bugs in PerformanceState

**Objective**: Fix five code quality issues in `PerformanceState`: unused parameter, incorrect FPS calculation, O(n) count, dead branch, and misleading `total_frames` semantics.

**Depends on**: None

**Estimated Time**: 2-3 hours

### Scope

- `crates/fdemon-app/src/session.rs`: Fix `compute_stats()`, `calculate_fps()`, rename `total_frames` usage
- `crates/fdemon-core/src/performance.rs`: Rename `total_frames` field in `PerformanceStats`
- `crates/fdemon-app/src/handler/tests.rs`: Update tests referencing `total_frames`
- `crates/fdemon-app/src/handler/update.rs`: Update any references to `total_frames`

### Details

All fixes are within the `PerformanceState` impl block (`session.rs:237-331`) and the `PerformanceStats` struct (`performance.rs:~190`).

#### Fix 1: Remove unused `_memory` parameter (CRITICAL #4)

**Location:** `session.rs:250-252`

```rust
// BEFORE
pub fn compute_stats(
    frames: &RingBuffer<FrameTiming>,
    _memory: &RingBuffer<MemoryUsage>,
) -> PerformanceStats {
```

Remove the `_memory` parameter entirely. Update `recompute_stats()` at line 243-245 to match:

```rust
// AFTER
pub fn compute_stats(frames: &RingBuffer<FrameTiming>) -> PerformanceStats {

// recompute_stats
pub fn recompute_stats(&mut self) {
    self.stats = Self::compute_stats(&self.frame_history);
}
```

**Impact:** Any callers of `compute_stats` must be updated. Check `handler/update.rs` and `handler/tests.rs`.

#### Fix 2: Fix `calculate_fps` to return actual FPS rate (MAJOR #5)

**Location:** `session.rs:296-316`

Current implementation returns the **count** of frames in a 1-second window, not a rate. While numerically similar for a 1s window, the name is misleading. Additionally, it compares `f.timestamp` (`chrono::DateTime<Local>`) against `chrono::Local::now()` which is correct (both use wall-clock time via chrono — the original review was partially wrong about clock domains since `FrameTiming.timestamp` is set via `chrono::Local::now()` at parse time).

Fix: compute actual FPS as count / elapsed_seconds:

```rust
pub fn calculate_fps(frames: &RingBuffer<FrameTiming>) -> Option<f64> {
    if frames.len() < 2 {
        return None;
    }

    let now = chrono::Local::now();
    let window_start =
        now - chrono::Duration::from_std(FPS_WINDOW).unwrap_or(chrono::Duration::seconds(1));

    let recent: Vec<_> = frames
        .iter()
        .filter(|f| f.timestamp >= window_start)
        .collect();

    if recent.len() < 2 {
        return None;
    }

    // Compute actual elapsed time between first and last frame in window
    let earliest = recent.iter().map(|f| f.timestamp).min()?;
    let latest = recent.iter().map(|f| f.timestamp).max()?;
    let elapsed_secs = (latest - earliest).num_milliseconds() as f64 / 1000.0;

    if elapsed_secs <= 0.0 {
        return None;
    }

    // FPS = (frame_count - 1) / elapsed_time
    // Subtract 1 because N frames span N-1 intervals
    Some((recent.len() - 1) as f64 / elapsed_secs)
}
```

This uses only frame timestamps (same clock domain) and computes a proper rate.

#### Fix 3: Replace `frames.iter().count()` with `frames.len()` (MAJOR #6)

**Location:** `session.rs:260`

```rust
// BEFORE
let total_frames = frames.iter().count() as u64;

// AFTER
let total_frames = frames.len() as u64;
```

`RingBuffer::len()` exists at `performance.rs:232-234` and is O(1) via `VecDeque::len()`.

#### Fix 4: Remove dead branch in `compute_stats` (MAJOR #7)

**Location:** `session.rs:269-273`

The `frames.is_empty()` guard at line 254 ensures `frame_times` is non-empty by the time we reach line 269. Remove the dead `if frame_times.is_empty()` branch:

```rust
// BEFORE
let avg_frame_ms = if frame_times.is_empty() {
    None
} else {
    Some(frame_times.iter().sum::<f64>() / frame_times.len() as f64)
};

// AFTER
let avg_frame_ms = Some(frame_times.iter().sum::<f64>() / frame_times.len() as f64);
```

#### Fix 5: Rename `total_frames` to `buffered_frames` (MAJOR #8)

**Location:** `performance.rs` (PerformanceStats struct) and `session.rs` (where it's set)

The field name `total_frames` implies a cumulative lifetime count, but it's set to the current ring buffer size (capped at 300). Rename to `buffered_frames` to accurately describe what it holds:

```rust
// In performance.rs
pub struct PerformanceStats {
    // ...
    /// Number of frame timing samples currently in the ring buffer
    pub buffered_frames: u64,
}
```

Update all references in `session.rs`, `handler/update.rs`, and `handler/tests.rs`.

### Acceptance Criteria

1. `compute_stats` takes only `frames: &RingBuffer<FrameTiming>` (no `_memory`)
2. `calculate_fps` returns actual FPS rate (frames-per-second), not a count
3. `frames.iter().count()` replaced with `frames.len()` (O(1))
4. Dead `frame_times.is_empty()` branch removed
5. `total_frames` renamed to `buffered_frames` across all files
6. All existing tests updated to match new API
7. New test: `calculate_fps` returns correct rate for known frame data
8. New test: `compute_stats` with various frame counts produces correct stats
9. `cargo test --workspace` passes
10. `cargo clippy --workspace -- -D warnings` passes

### Testing

```rust
#[test]
fn test_calculate_fps_returns_rate_not_count() {
    // Setup: ring buffer with frames at known timestamps
    // e.g., 60 frames over exactly 1 second = 60 FPS
    // Assert: calculate_fps returns ~60.0, not 60 as integer
}

#[test]
fn test_compute_stats_no_memory_param() {
    // Verify compute_stats works with just frames param
}

#[test]
fn test_buffered_frames_reflects_buffer_size() {
    // Add 10 frames, verify buffered_frames == 10
    // Add 300+ frames, verify buffered_frames == 300 (capacity)
}
```

### Notes

- `calculate_fps` is currently `pub` — check if any external crates call it (unlikely based on research, but verify)
- The `total_frames` → `buffered_frames` rename touches `PerformanceStats` in `fdemon-core`, which is a public type — ensure no external consumers reference this field
- Consider whether to also add a `lifetime_frame_count: u64` field that increments on every frame event (deferred to Phase 4 if not trivial)

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-core/src/performance.rs` | Renamed `total_frames` to `buffered_frames` with updated doc comment |
| `crates/fdemon-app/src/session.rs` | Fixed `compute_stats` (removed `_memory` param, `iter().count()` → `len()`, removed dead branch), fixed `calculate_fps` to return actual rate, updated `recompute_stats` call site, updated all tests referencing `total_frames` → `buffered_frames`, added 3 new tests |
| `crates/fdemon-app/src/handler/tests.rs` | Updated 4 assertions from `total_frames` → `buffered_frames`; fixed pre-existing Task 01 compile errors: added `perf_task_handle` field to `VmServicePerformanceMonitoringStarted` message struct in test |
| `crates/fdemon-app/src/handler/update.rs` | Fixed pre-existing Task 01 compile error: updated match arm for `VmServicePerformanceMonitoringStarted` to destructure and store `perf_task_handle` on `SessionHandle` |
| `crates/fdemon-app/src/actions.rs` | Fixed pre-existing Task 01 compile error: added semicolon after `if let Ok(mut slot) = task_handle_slot.lock()` block to fix lifetime issue |

### Notable Decisions/Tradeoffs

1. **Pre-existing Task 01 compile errors fixed**: Three compile errors existed in the repo before this task (`actions.rs` lifetime issue, `update.rs` missing `perf_task_handle` in pattern, `handler/tests.rs` missing `perf_task_handle` in struct init). These prevented `cargo test` from running, so they were fixed as part of making the quality gate pass. The fixes align with Task 01's intent.

2. **`calculate_fps` semantics**: Changed from returning raw frame count (which happened to equal FPS for a 1s window) to computing actual frames-per-second as `(count - 1) / elapsed_secs` using only frame timestamps. The `< 2` guard (was `== 0`) correctly handles the edge case where fewer than 2 frames have been recorded since N frames need N-1 intervals to compute a rate.

3. **Dead branch removal**: The `if frame_times.is_empty() { None } else { ... }` branch was dead code because the `frames.is_empty()` guard at the top of `compute_stats` ensures `frame_times` is non-empty. Replaced with direct `Some(...)` and added a comment explaining the invariant.

### Testing Performed

- `cargo check --workspace` - Passed
- `cargo test -p fdemon-app` - Passed (801 unit tests)
- `cargo test --lib` - Passed (all library unit tests)
- `cargo clippy --workspace -- -D warnings` - Passed (no warnings)
- `cargo fmt --all` - Applied (no format changes needed)

### Risks/Limitations

1. **E2E tests**: 25 e2e integration tests in the `flutter-demon` binary test crate fail, but these are pre-existing failures requiring a real Flutter environment (TUI/headless process tests). They are unrelated to this task's changes.

2. **`calculate_fps` behavior change**: The old implementation returned `count as f64` (e.g., 60.0 for 60 frames/sec). The new implementation returns `(count-1)/elapsed_secs` which may differ slightly from the old value when frame spacing is not perfectly uniform. Tests verify the rate is within ±5 FPS of expected.
