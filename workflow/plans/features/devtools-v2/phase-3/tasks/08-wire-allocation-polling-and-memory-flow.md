## Task: Wire Allocation Profile Polling and Memory Sample Flow

**Objective**: Connect the daemon's `get_memory_sample()` and `get_allocation_profile()` functions to the app layer's polling loop, producing `VmServiceMemorySample` and `VmServiceAllocationProfileReceived` messages that flow into `PerformanceState`.

**Depends on**: Task 03 (VM service extensions), Task 04 (performance handlers)

### Scope

- `crates/fdemon-app/src/handler/update.rs`: Update existing `VmServiceMemorySnapshot` handler to also produce `MemorySample`
- `crates/fdemon-app/src/actions.rs` (or equivalent engine/process layer): Modify `spawn_performance_polling` to call `get_memory_sample()` and `get_allocation_profile()`
- `crates/fdemon-app/src/handler/mod.rs`: Add `UpdateAction` variants if needed

### Details

#### Strategy: Extend existing polling loop

The current `spawn_performance_polling()` task (in `actions.rs`) runs on a timer and calls `get_memory_usage()`. Rather than creating a new polling task, extend this existing task to:

1. Call `get_memory_sample()` instead of (or in addition to) `get_memory_usage()`
2. Periodically call `get_allocation_profile()` at a lower frequency

```rust
async fn performance_polling_loop(
    handle: VmRequestHandle,
    isolate_id: String,
    msg_tx: mpsc::UnboundedSender<Message>,
    session_id: SessionId,
    memory_interval_ms: u64,
    allocation_interval_ms: u64,  // NEW — typically 5000ms
    mut shutdown_rx: watch::Receiver<bool>,
) {
    let memory_interval = Duration::from_millis(memory_interval_ms);
    let allocation_interval = Duration::from_millis(allocation_interval_ms);
    let mut memory_tick = tokio::time::interval(memory_interval);
    let mut allocation_tick = tokio::time::interval(allocation_interval);

    loop {
        tokio::select! {
            _ = shutdown_rx.changed() => break,

            _ = memory_tick.tick() => {
                // Existing: get basic memory usage
                if let Some(memory) = get_memory_usage(&handle, &isolate_id).await {
                    let _ = msg_tx.send(Message::VmServiceMemorySnapshot {
                        session_id,
                        memory: memory.clone(),
                    });

                    // NEW: also produce rich memory sample
                    if let Some(sample) = get_memory_sample(&handle, &isolate_id).await {
                        let _ = msg_tx.send(Message::VmServiceMemorySample {
                            session_id,
                            sample,
                        });
                    }
                }
            }

            _ = allocation_tick.tick() => {
                // NEW: periodic allocation profile fetch
                if let Some(profile) = get_allocation_profile(&handle, &isolate_id, false).await {
                    let _ = msg_tx.send(Message::VmServiceAllocationProfileReceived {
                        session_id,
                        profile,
                    });
                }
            }
        }
    }
}
```

#### Configuration

Add configuration for allocation profile polling interval. In `config/types.rs`:

```rust
pub struct DevToolsSettings {
    // ... existing fields ...
    pub performance_refresh_ms: u64,      // existing, default 2000
    pub memory_history_size: usize,       // existing, default 60
    pub allocation_profile_interval_ms: u64,  // NEW, default 5000
}
```

Default value: `5000` (every 5 seconds). This is intentionally lower frequency than memory polling because `getAllocationProfile` is more expensive (returns per-class data).

#### Update `StartPerformanceMonitoring` action

If the current `UpdateAction::StartPerformanceMonitoring` doesn't include the allocation interval, extend it:

```rust
UpdateAction::StartPerformanceMonitoring {
    session_id: SessionId,
    handle: Option<VmRequestHandle>,
    performance_refresh_ms: u64,
    allocation_profile_interval_ms: u64,  // NEW
}
```

Update the handler in `update.rs` that produces this action (on `VmServiceConnected`) to include the new interval from settings.

#### Backward compatibility

The existing `VmServiceMemorySnapshot` message and handler continue to work unchanged. The new `VmServiceMemorySample` message arrives alongside it. Both push to their respective ring buffers:
- `VmServiceMemorySnapshot` → `perf.memory_history.push(memory)` (existing)
- `VmServiceMemorySample` → `perf.memory_samples.push(sample)` (new, handled in Task 04's handler)

This means the existing gauge fallback (used when `memory_samples` is empty) still works during the transition period.

#### Memory sample polling frequency

The memory sample and basic memory usage share the same timer tick. This is intentional:
- `get_memory_sample()` calls `get_memory_usage()` internally, then adds RSS
- We send both messages from the same tick to keep the two ring buffers in sync
- If `get_memory_sample()` fails (e.g., `getIsolate` unavailable), the basic `VmServiceMemorySnapshot` still succeeds

### Acceptance Criteria

1. `spawn_performance_polling` (or equivalent) calls `get_memory_sample()` on each memory tick
2. `VmServiceMemorySample` messages are sent and arrive at the handler
3. `memory_samples` ring buffer is populated during active monitoring
4. `get_allocation_profile()` is called every `allocation_profile_interval_ms` (default 5s)
5. `VmServiceAllocationProfileReceived` messages are sent and arrive at the handler
6. `allocation_profile` is populated in `PerformanceState`
7. Allocation polling stops when session disconnects or Performance panel is not active
8. Existing `VmServiceMemorySnapshot` flow continues to work (no regression)
9. Configuration value `allocation_profile_interval_ms` respected
10. `cargo check -p fdemon-app` passes
11. `cargo test -p fdemon-app` passes

### Testing

Add tests for the polling setup and message flow:

```rust
#[test]
fn test_vm_connected_starts_perf_monitoring_with_allocation_interval() {
    // Verify UpdateAction::StartPerformanceMonitoring includes allocation_profile_interval_ms
}

#[test]
fn test_memory_sample_received_populates_buffer() {
    let mut state = make_state_with_session();
    let session_id = current_session_id(&state);
    let sample = MemorySample {
        dart_heap: 10_000_000,
        dart_native: 5_000_000,
        raster_cache: 0,
        allocated: 20_000_000,
        rss: 50_000_000,
        timestamp: chrono::Local::now(),
    };
    update(&mut state, Message::VmServiceMemorySample { session_id, sample });
    let perf = &current_session(&state).performance;
    assert_eq!(perf.memory_samples.len(), 1);
    assert_eq!(perf.memory_samples.latest().unwrap().dart_heap, 10_000_000);
}

#[test]
fn test_allocation_profile_received_stores_profile() {
    let mut state = make_state_with_session();
    let session_id = current_session_id(&state);
    let profile = AllocationProfile {
        members: vec![ClassHeapStats {
            class_name: "String".to_string(),
            library_uri: Some("dart:core".to_string()),
            new_space_instances: 100,
            new_space_size: 5_000,
            old_space_instances: 1_000,
            old_space_size: 50_000,
        }],
        timestamp: chrono::Local::now(),
    };
    update(&mut state, Message::VmServiceAllocationProfileReceived {
        session_id,
        profile,
    });
    let perf = &current_session(&state).performance;
    assert!(perf.allocation_profile.is_some());
    assert_eq!(perf.allocation_profile.as_ref().unwrap().members.len(), 1);
}

#[test]
fn test_memory_snapshot_still_works_alongside_sample() {
    // Verify VmServiceMemorySnapshot still populates memory_history
    let mut state = make_state_with_session();
    let session_id = current_session_id(&state);
    let memory = MemoryUsage {
        heap_usage: 10_000_000,
        heap_capacity: 20_000_000,
        external_usage: 5_000_000,
        timestamp: chrono::Local::now(),
    };
    update(&mut state, Message::VmServiceMemorySnapshot { session_id, memory });
    let perf = &current_session(&state).performance;
    assert_eq!(perf.memory_history.len(), 1);
}

#[test]
fn test_disconnect_clears_allocation_profile() {
    // After VmServiceDisconnected, allocation_profile should be None
    // (reset happens via PerformanceState reset on reconnect)
}
```

### Notes

- **Two ring buffers intentional**: `memory_history` (simple) and `memory_samples` (rich) coexist. The simple buffer is the fallback when the rich sample fails. Both are populated from the same tick, keeping them in sync.
- **Allocation profile is expensive**: `getAllocationProfile` iterates all classes in the Dart heap. 5-second interval is a reasonable default. Do NOT call it on every memory tick (2s) — that would double the VM service load.
- **Allocation polling when panel inactive**: Consider only polling allocation profiles when the Performance panel is the active DevTools panel. This saves VM service load when the user is on the Inspector or Network tabs. Implement via a check in the polling loop or by starting/stopping the allocation timer when switching panels.
- **`get_allocation_profile` already exists**: The function is implemented in `fdemon-daemon/src/vm_service/performance.rs` and exported. This task just calls it from the polling loop.
- **Error handling**: Both `get_memory_sample()` and `get_allocation_profile()` return `Option`. On `None`, silently skip the message send. No error messages shown to the user for individual poll failures — the data will refresh on the next tick.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/config/types.rs` | Added `allocation_profile_interval_ms: u64` field to `DevToolsSettings` with default 5000ms; added `default_allocation_profile_interval_ms()` fn |
| `crates/fdemon-app/src/handler/mod.rs` | Added `allocation_profile_interval_ms: u64` field to `UpdateAction::StartPerformanceMonitoring` variant |
| `crates/fdemon-app/src/handler/update.rs` | Updated `VmServiceConnected` handler to read and pass `allocation_profile_interval_ms` from settings into `StartPerformanceMonitoring` action |
| `crates/fdemon-app/src/process.rs` | Updated `hydrate_start_performance_monitoring()` to thread `allocation_profile_interval_ms` through the hydration pattern |
| `crates/fdemon-app/src/actions.rs` | Added `ALLOC_PROFILE_POLL_MIN_MS` constant (1000ms); updated `spawn_performance_polling` signature and implementation to use dual-timer loop with separate memory and allocation ticks; calls `get_memory_sample()` on memory tick and `get_allocation_profile()` on allocation tick; updated `handle_action` match arm to destructure new field |
| `crates/fdemon-app/src/handler/tests.rs` | Added 4 new tests: `test_vm_connected_starts_perf_monitoring_with_allocation_interval`, `test_vm_connected_uses_default_allocation_interval`, `test_memory_snapshot_still_works_alongside_sample`, `test_disconnect_clears_allocation_profile` |

### Notable Decisions/Tradeoffs

1. **Dual-timer loop**: The existing single-interval polling loop was replaced with `tokio::select!` across two `tokio::time::interval` timers — one for memory polling (`performance_refresh_ms`, min 500ms) and one for allocation profile polling (`allocation_profile_interval_ms`, min 1000ms). This cleanly separates the two frequencies without requiring a separate task.

2. **`continue` on `main_isolate_id` failure in allocation tick**: Each tick independently fetches the isolate ID. Failures (e.g., during hot restart when the isolate cache is stale) are logged at debug level and skipped — the next tick retries. This is consistent with the memory tick pattern.

3. **`get_memory_sample` called after `get_memory_usage` on same tick**: The memory sample internally calls `get_memory_usage` again. This means two `getMemoryUsage` RPC calls happen per memory tick (one explicit, one inside `get_memory_sample`). The redundancy is intentional per the task spec — it keeps both ring buffers in sync and lets the basic snapshot succeed even if `get_memory_sample` fails (e.g., `getIsolate` unavailable). The extra RPC is acceptable at 2s intervals.

4. **Minimum allocation interval (1000ms)**: A higher minimum than memory polling (500ms) was chosen because `getAllocationProfile` walks the entire Dart heap. This protects against aggressive configurations degrading VM performance.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check -p fdemon-app` - Passed
- `cargo test -p fdemon-app` - Passed (949 unit tests, 1 doc test; 4 new tests added)
- `cargo clippy -p fdemon-app -- -D warnings` - Passed

### Risks/Limitations

1. **Double `getMemoryUsage` per memory tick**: Calling `get_memory_sample` after `get_memory_usage` means the VM service receives two `getMemoryUsage` requests per tick. At the default 2s interval this is negligible, but could be optimised in the future by having `get_memory_sample` accept an already-fetched `MemoryUsage` to avoid the duplicate call.

2. **No panel-active guard**: The task notes mention "only polling allocation profiles when the Performance panel is active". This optimisation was not implemented in this task — the polling loop runs unconditionally once started. Implementing a panel-active check would require state inspection inside an async background task, which would need additional synchronisation (e.g., a watch channel). This is left as a future optimisation.

3. **Allocation tick fires immediately**: `tokio::time::interval` fires immediately on the first tick. The first allocation profile fetch will happen shortly after VM connection, not after the first 5-second interval. This is generally acceptable behaviour and matches how memory polling works.
