## Task: Deduplicate `getMemoryUsage` in Performance Polling

**Objective**: Eliminate the redundant `getMemoryUsage` RPC call that fires on every memory tick, reducing VM Service pressure from 3 RPCs/tick to 2 RPCs/tick.

**Depends on**: None

**Estimated Time**: 1-1.5 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/vm_service/performance.rs`: Add `get_memory_sample_from_usage()` that accepts a pre-fetched `MemoryUsage` instead of re-fetching it
- `crates/fdemon-app/src/actions/performance.rs`: Restructure the memory arm to call `get_memory_usage` once and pass the result to the new function

**Files Read (Dependencies):**
- `crates/fdemon-core/src/performance.rs`: `MemoryUsage`, `MemorySample` type definitions
- `crates/fdemon-daemon/src/vm_service/mod.rs`: Re-exports to update

### Details

#### The Problem

On every `memory_tick` in the performance polling loop (`actions/performance.rs:120-180`), two `getMemoryUsage` RPCs fire:

1. **Line 134**: Explicit call to `get_memory_usage(&handle, &isolate_id)` — result sent as `VmServiceMemorySnapshot`
2. **Line 165**: Call to `get_memory_sample(&handle, &isolate_id)` — which internally calls `get_memory_usage` again at `daemon/vm_service/performance.rs:88`, then also calls `getIsolate` for RSS

The `MemoryUsage` result from call 1 is discarded before call 2 re-fetches the same data.

#### The Fix

**Step 1: Add `get_memory_sample_from_usage()` to daemon layer**

In `crates/fdemon-daemon/src/vm_service/performance.rs`, add a new public function that takes a pre-fetched `MemoryUsage` and only fetches the RSS portion:

```rust
/// Build a MemorySample from an already-fetched MemoryUsage.
/// Only issues one RPC (getIsolate for RSS), not two.
pub async fn get_memory_sample_from_usage(
    handle: &VmRequestHandle,
    isolate_id: &str,
    usage: &MemoryUsage,
) -> Option<MemorySample> {
    let rss = get_isolate_rss(handle, isolate_id).await.unwrap_or(0);
    Some(MemorySample {
        dart_heap: usage.heap_usage,
        dart_native: usage.external_usage,
        raster_cache: 0, // future: ext.flutter.rasterCache
        allocated: usage.heap_capacity,
        rss,
        timestamp: usage.timestamp,
    })
}
```

The existing `get_memory_sample` function should remain for backwards compatibility (it's used by its public API).

**Step 2: Restructure the memory arm in the polling loop**

In `crates/fdemon-app/src/actions/performance.rs`, refactor the memory tick arm (~lines 120-180) to:

1. Call `get_memory_usage` once
2. Send `VmServiceMemorySnapshot` from the result
3. Pass the same `MemoryUsage` to `get_memory_sample_from_usage` for `VmServiceMemorySample`

```rust
// Memory tick arm (pseudocode)
_ = memory_tick.tick() => {
    let isolate_id = match handle.main_isolate_id().await { ... };

    // Single RPC: getMemoryUsage
    match get_memory_usage(&handle, &isolate_id).await {
        Ok(usage) => {
            // Send snapshot from the same result
            if msg_tx.send(Message::VmServiceMemorySnapshot { session_id, memory: usage.clone() }).await.is_err() {
                break;
            }
            // Build sample from the already-fetched usage (only fetches RSS)
            if let Some(sample) = get_memory_sample_from_usage(&handle, &isolate_id, &usage).await {
                if msg_tx.send(Message::VmServiceMemorySample { session_id, sample }).await.is_err() {
                    break;
                }
            }
        }
        Err(e) => {
            debug!("Failed to get memory usage: {e}");
            continue;
        }
    }
}
```

**Step 3: Re-export the new function**

Update `crates/fdemon-daemon/src/vm_service/mod.rs` to re-export `get_memory_sample_from_usage` alongside `get_memory_sample`.

#### RPC Reduction

| Before | After |
|--------|-------|
| `getMemoryUsage` x2 + `getIsolate` x1 = 3 RPCs/tick | `getMemoryUsage` x1 + `getIsolate` x1 = 2 RPCs/tick |

At 500ms intervals: 6 RPCs/sec → 4 RPCs/sec (just from memory arm).

### Acceptance Criteria

1. `get_memory_usage` is called exactly once per memory tick (not twice)
2. `VmServiceMemorySnapshot` and `VmServiceMemorySample` messages are both still sent on each tick
3. The `MemoryUsage` data in the snapshot matches the data used to build the sample (same fetch)
4. `get_memory_sample_from_usage` has unit tests verifying correct field mapping
5. Existing `get_memory_sample` function is preserved (not deleted) for API stability
6. All existing tests pass: `cargo test -p fdemon-daemon` and `cargo test -p fdemon-app`

### Testing

**Unit test for the new function:**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_sample_from_usage_maps_fields_correctly() {
        // Verify that MemoryUsage fields map to the correct MemorySample fields
        // dart_heap ← heap_usage
        // dart_native ← external_usage
        // allocated ← heap_capacity
        // timestamp ← usage.timestamp
        // raster_cache ← 0 (hardcoded)
    }
}
```

**Integration-level verification:**
- Run `cargo test -p fdemon-daemon` — all 527 existing tests pass
- Run `cargo test -p fdemon-app` — all 1,511 existing tests pass
- `cargo clippy --workspace -- -D warnings` — no new warnings

### Notes

- `MemoryUsage.heap_usage` maps to `MemorySample.dart_heap`, `.external_usage` to `.dart_native`, `.heap_capacity` to `.allocated`. This mapping already exists in `get_memory_sample` at `daemon/vm_service/performance.rs:93-98` — the new function replicates it.
- `raster_cache` is hardcoded to 0 in the existing `get_memory_sample` (line 98) — keep this behavior.
- `get_isolate_rss` is a private helper in `daemon/vm_service/performance.rs` (lines 113-140). It calls `getIsolate` which is less expensive than `getMemoryUsage` (no heap walk), so keeping this call is fine.
- The existing `get_memory_sample` function at `daemon/vm_service/performance.rs:86-103` should NOT be removed — it's part of the public API surface re-exported from `mod.rs:106-109`. The new function is an optimization path for callers who already have a `MemoryUsage`.

---

## Completion Summary

**Status:** Not Started
