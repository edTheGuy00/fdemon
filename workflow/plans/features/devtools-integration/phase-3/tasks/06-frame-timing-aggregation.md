## Task: Frame Timing Integration & Data Aggregation

**Objective**: Integrate frame timing events from the Extension stream into the TEA architecture, calculate FPS and jank metrics, and aggregate performance statistics across memory and frame data. This is the final task that completes the Phase 3 data pipeline — all metrics are flowing and aggregated, ready for Phase 4's TUI panels to visualize.

**Depends on**: 01-performance-data-models, 02-vm-request-handle, 04-frame-timing-rpcs, 05-memory-monitoring-integration

**Estimated Time**: 4-6 hours

### Scope

- `crates/fdemon-app/src/message.rs`: Add `VmServiceFrameTiming` message variant
- `crates/fdemon-app/src/handler/update.rs`: Handle frame timing messages, compute aggregated stats
- `crates/fdemon-app/src/actions.rs`: Parse `Flutter.Frame` events in forwarding loop
- `crates/fdemon-app/src/session.rs`: Add stats computation methods to `PerformanceState`

### Details

#### 1. New Message Variant

Add to `Message` enum:

```rust
/// Frame timing data received from Flutter.Frame Extension event.
VmServiceFrameTiming {
    session_id: SessionId,
    timing: fdemon_core::performance::FrameTiming,
},
```

#### 2. Forward Frame Events from Extension Stream

Extend `forward_vm_events` in `actions.rs` to parse `Flutter.Frame` events. These arrive on the Extension stream (already subscribed). Add after the existing `Flutter.Error` handling:

```rust
// In forward_vm_events, inside the event match, BEFORE the LogRecord check:

// Try parsing as a Flutter.Frame event (frame timing)
if let Some(timing) = fdemon_daemon::vm_service::timeline::parse_frame_timing(
    &event.params.event
) {
    let _ = msg_tx
        .send(Message::VmServiceFrameTiming {
            session_id,
            timing,
        })
        .await;
    continue;
}
```

The ordering in `forward_vm_events` should be:
1. `Flutter.Error` (crash logs) — most critical
2. `Flutter.Frame` (frame timing) — **NEW**
3. `GC` events — **from Task 05**
4. `LogRecord` (structured logs) — existing
5. Everything else — ignored

#### 3. Frame Timing Handler

```rust
Message::VmServiceFrameTiming { session_id, timing } => {
    if let Some(handle) = state.session_manager.get_mut(&session_id) {
        handle.session.performance.frame_history.push(timing);
        // Recompute stats periodically (every N frames to avoid per-frame overhead)
        if handle.session.performance.frame_history.len() % STATS_RECOMPUTE_INTERVAL == 0 {
            handle.session.performance.recompute_stats();
        }
    }
    UpdateResult::default()
}
```

#### 4. Stats Recomputation

Add methods to `PerformanceState` in `session.rs`:

```rust
/// How often to recompute aggregated stats (every N frames).
const STATS_RECOMPUTE_INTERVAL: usize = 10;

/// Time window for FPS calculation (1 second).
const FPS_WINDOW: std::time::Duration = std::time::Duration::from_secs(1);

impl PerformanceState {
    /// Recompute aggregated performance statistics from the ring buffers.
    pub fn recompute_stats(&mut self) {
        self.stats = Self::compute_stats(&self.frame_history, &self.memory_history);
    }

    /// Compute performance statistics from frame and memory history.
    fn compute_stats(
        frames: &RingBuffer<FrameTiming>,
        _memory: &RingBuffer<MemoryUsage>,
    ) -> PerformanceStats {
        if frames.is_empty() {
            return PerformanceStats::default();
        }

        let frame_times: Vec<f64> = frames.iter()
            .map(|f| f.elapsed_ms())
            .collect();

        let total_frames = frames.iter().count() as u64;

        // FPS: count frames in the last 1 second
        let fps = Self::calculate_fps(frames);

        // Jank count: frames exceeding 60fps budget
        let jank_count = frames.iter()
            .filter(|f| f.is_janky())
            .count() as u32;

        // Average frame time
        let avg_frame_ms = if frame_times.is_empty() {
            None
        } else {
            Some(frame_times.iter().sum::<f64>() / frame_times.len() as f64)
        };

        // P95 frame time
        let p95_frame_ms = Self::percentile(&frame_times, 95.0);

        // Max frame time
        let max_frame_ms = frame_times.iter().copied().reduce(f64::max);

        PerformanceStats {
            fps,
            jank_count,
            avg_frame_ms,
            p95_frame_ms,
            max_frame_ms,
            total_frames,
        }
    }

    /// Calculate FPS from recent frame timings.
    ///
    /// Counts the number of frames in the last 1 second using timestamps.
    fn calculate_fps(frames: &RingBuffer<FrameTiming>) -> Option<f64> {
        if frames.len() < 2 {
            return None;
        }

        let now = chrono::Local::now();
        let window_start = now - chrono::Duration::seconds(1);

        let recent_count = frames.iter()
            .filter(|f| f.timestamp >= window_start)
            .count();

        if recent_count == 0 {
            return None; // No frames in the last second (app idle or backgrounded)
        }

        Some(recent_count as f64)
    }

    /// Calculate the Nth percentile from a sorted slice of values.
    fn percentile(values: &[f64], pct: f64) -> Option<f64> {
        if values.is_empty() {
            return None;
        }
        let mut sorted = values.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let index = ((pct / 100.0) * (sorted.len() - 1) as f64).round() as usize;
        Some(sorted[index.min(sorted.len() - 1)])
    }
}
```

#### 5. Periodic Stats Refresh via Memory Polling

The memory polling task (from Task 05) already runs at 2s intervals. Piggyback stats recomputation on memory snapshot receipt:

```rust
// In the VmServiceMemorySnapshot handler (Task 05):
Message::VmServiceMemorySnapshot { session_id, memory } => {
    if let Some(handle) = state.session_manager.get_mut(&session_id) {
        handle.session.performance.memory_history.push(memory);
        handle.session.performance.monitoring_active = true;
        // Also recompute stats periodically with the polling cycle
        handle.session.performance.recompute_stats();
    }
    UpdateResult::default()
}
```

This ensures stats are recomputed at least every 2 seconds (memory poll interval) even if frame events are sparse.

#### 6. Enable Frame Tracking on Connection

In `spawn_vm_service_connection` (actions.rs), after stream subscription, optionally enable frame tracking:

```rust
// After subscribe_flutter_streams():
// Best-effort: enable frame event emission
if let Ok(isolate_id) = client.main_isolate_id().await {
    let _ = fdemon_daemon::vm_service::timeline::enable_frame_tracking(
        &client.request_handle(),
        &isolate_id,
    ).await;
}
```

#### 7. Handle Empty/Stale Frame Data

When the app is idle (no animation), `Flutter.Frame` events stop. The stats should reflect this:

```rust
impl PerformanceStats {
    /// Whether the FPS data is stale (no recent frames).
    pub fn is_stale(&self) -> bool {
        self.fps.is_none()
    }
}
```

Phase 4's TUI can show "idle" or "–" when FPS is `None`.

### Acceptance Criteria

1. `VmServiceFrameTiming` message correctly receives frame timing data
2. Frame timings are pushed into `frame_history` ring buffer
3. FPS is calculated from frames in the last 1-second window
4. Jank count tracks frames exceeding 16.667ms budget
5. Average, P95, and max frame times are computed correctly
6. `PerformanceStats` is recomputed every 10 frames and every memory poll cycle
7. `Flutter.Frame` events are parsed in `forward_vm_events` after `Flutter.Error`
8. Stats are `None` when no frame data is available (app idle)
9. Frame tracking is enabled best-effort on VM Service connection
10. Stats computation is efficient (no per-frame allocations beyond the ring buffer push)
11. All existing VM Service behavior unchanged

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn make_frame(number: u64, elapsed_micros: u64) -> FrameTiming {
        FrameTiming {
            number,
            build_micros: elapsed_micros / 2,
            raster_micros: elapsed_micros / 2,
            elapsed_micros,
            timestamp: chrono::Local::now(),
        }
    }

    #[test]
    fn test_frame_timing_handler() {
        let mut state = make_test_state_with_session();
        let session_id = active_session_id(&state);
        let timing = make_frame(1, 10000);

        let msg = Message::VmServiceFrameTiming { session_id, timing };
        update(&mut state, msg);

        let perf = &state.session_manager.get(&session_id).unwrap()
            .session.performance;
        assert_eq!(perf.frame_history.len(), 1);
    }

    #[test]
    fn test_stats_computation_empty() {
        let stats = PerformanceState::compute_stats(
            &RingBuffer::new(10),
            &RingBuffer::new(10),
        );
        assert!(stats.fps.is_none());
        assert!(stats.avg_frame_ms.is_none());
        assert_eq!(stats.jank_count, 0);
    }

    #[test]
    fn test_stats_computation_with_frames() {
        let mut frames = RingBuffer::new(100);
        for i in 0..60 {
            frames.push(make_frame(i, 10_000)); // 10ms = smooth
        }
        // Add 5 janky frames
        for i in 60..65 {
            frames.push(make_frame(i, 25_000)); // 25ms = janky
        }

        let stats = PerformanceState::compute_stats(&frames, &RingBuffer::new(10));
        assert_eq!(stats.jank_count, 5);
        assert_eq!(stats.total_frames, 65);
        // Average: (60*10 + 5*25) / 65 ≈ 11.15ms
        let avg = stats.avg_frame_ms.unwrap();
        assert!(avg > 11.0 && avg < 12.0);
    }

    #[test]
    fn test_percentile_calculation() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        let p95 = PerformanceState::percentile(&values, 95.0).unwrap();
        assert!((p95 - 10.0).abs() < f64::EPSILON); // 95th of 10 values rounds to index 9
    }

    #[test]
    fn test_percentile_empty() {
        assert!(PerformanceState::percentile(&[], 95.0).is_none());
    }

    #[test]
    fn test_percentile_single() {
        assert_eq!(PerformanceState::percentile(&[42.0], 95.0), Some(42.0));
    }

    #[test]
    fn test_jank_detection() {
        let smooth = make_frame(1, 10_000);
        let janky = make_frame(2, 20_000);
        assert!(!smooth.is_janky());
        assert!(janky.is_janky());
    }

    #[test]
    fn test_performance_state_reset_on_reconnect() {
        let mut perf = PerformanceState::default();
        perf.memory_history.push(MemoryUsage {
            heap_usage: 100, heap_capacity: 200, external_usage: 0,
            timestamp: chrono::Local::now(),
        });
        perf.monitoring_active = true;

        // Simulate reset
        perf = PerformanceState::default();
        assert!(perf.memory_history.is_empty());
        assert!(!perf.monitoring_active);
    }
}
```

### Notes

- **`STATS_RECOMPUTE_INTERVAL` of 10 frames** prevents per-frame stats computation. At 60 FPS, stats update ~6 times per second — fast enough for a TUI that renders at ~30 FPS. Additionally, the 2s memory poll recomputes stats as a backstop.
- **FPS calculation uses a 1-second sliding window** over frame timestamps. This is simple and effective. More sophisticated approaches (exponential moving average) can be added later if needed.
- **`percentile()` creates a sorted copy** which is fine for the ring buffer sizes we use (~300 frames). For larger datasets, consider a streaming percentile algorithm.
- **Frame events share the Extension stream** with `Flutter.Error` events. The parse order in `forward_vm_events` must check `Flutter.Error` first (crash logs are more critical than timing data).
- **This task depends on Task 05** because it builds on the `PerformanceState` and handlers established there. The frame timing data flows into the same `PerformanceState` and benefits from the same shutdown coordination.
- **Phase 4 (TUI)** will consume `PerformanceStats` and the ring buffers directly for rendering sparklines, gauges, and tables. This task ensures all the data is available and pre-aggregated.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-core/src/performance.rs` | Added `is_stale()` method to `PerformanceStats` impl block |
| `crates/fdemon-app/src/message.rs` | Added `VmServiceFrameTiming { session_id, timing }` variant |
| `crates/fdemon-app/src/session.rs` | Added `STATS_RECOMPUTE_INTERVAL`, `FPS_WINDOW` constants; added `recompute_stats()`, `compute_stats()`, `calculate_fps()`, `percentile()` methods to `PerformanceState`; added 9 unit tests |
| `crates/fdemon-app/src/handler/update.rs` | Added `VmServiceFrameTiming` handler with interval-based stats recompute; added `recompute_stats()` call to `VmServiceMemorySnapshot` handler |
| `crates/fdemon-app/src/actions.rs` | Added `parse_frame_timing` import; added `Flutter.Frame` parsing in `forward_vm_events` (after `Flutter.Error`, before GC); added `enable_frame_tracking` best-effort call in `spawn_vm_service_connection` |
| `crates/fdemon-app/src/handler/tests.rs` | Added 4 handler tests for frame timing: handler basic, interval recompute, unknown session, memory snapshot triggering recompute |

### Notable Decisions/Tradeoffs

1. **Event ordering in `forward_vm_events`**: `Flutter.Frame` is checked after `Flutter.Error` (crash logs are more critical) but before `GC` and `LogRecord`. The GC stream and Logging stream events are non-Extension events so they wouldn't match the `Flutter.Frame` extension check anyway — the ordering between GC and LogRecord was preserved from Task 05 to avoid any risk of regression.

2. **`STATS_RECOMPUTE_INTERVAL` is `pub(crate)`**: The handler in `update.rs` references `crate::session::STATS_RECOMPUTE_INTERVAL` to perform the modulo check, so it needs to be visible at crate scope. Making it `pub(crate)` is the minimal visibility needed.

3. **`FPS_WINDOW` uses `chrono::Duration::from_std`**: Converting `std::time::Duration` to `chrono::Duration` requires `from_std()`, which can fail if the duration exceeds `i64::MAX` nanoseconds. The fallback to `chrono::Duration::seconds(1)` handles this edge case gracefully.

4. **`enable_frame_tracking` placement**: Called after `subscribe_flutter_streams()` but before extracting the request handle for the forwarding loop. This is fine because the best-effort call completes (or silently fails) before the loop starts.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check --workspace` - Passed
- `cargo test --lib --workspace` - Passed (446 unit tests in fdemon-tui, 792 in fdemon-app, 314 in fdemon-core, all passing)
- `cargo clippy --workspace -- -D warnings` - Passed (no warnings)
- E2E tests have pre-existing failures unrelated to this task (settings/TUI interaction timeouts)

### Risks/Limitations

1. **FPS window relies on `chrono::Local::now()`**: In tests, all frames are created with `chrono::Local::now()` timestamps, meaning they all fall within the 1-second window. In production, idle/backgrounded apps will have no recent frames and correctly return `None` for FPS.

2. **`percentile()` allocates a sorted copy**: For ~300-item ring buffers this is negligible. Larger datasets would benefit from a streaming percentile algorithm, but this is called only every 10 frames or on memory poll, not per-frame.
