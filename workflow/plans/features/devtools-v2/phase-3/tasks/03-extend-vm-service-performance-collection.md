## Task: Extend VM Service Performance Data Collection

**Objective**: Extend the daemon's VM service layer to collect richer memory data (RSS, native/raster breakdown) and detect shader compilation in frame events. This provides the raw data pipeline that feeds the new memory time-series chart and frame bar chart markers.

**Depends on**: Task 01 (extend-core-performance-types)

### Scope

- `crates/fdemon-daemon/src/vm_service/performance.rs`: Extend memory parsing, add `MemorySample` construction
- `crates/fdemon-daemon/src/vm_service/timeline.rs`: Add shader compilation detection, extend frame parsing
- `crates/fdemon-daemon/src/vm_service/client.rs`: May need to expose RSS from `getIsolate` response

### Details

#### Extend memory data collection

The current `get_memory_usage()` calls `getMemoryUsage(isolateId)` and returns `MemoryUsage { heap_usage, heap_capacity, external_usage }`. We need a richer function that constructs a `MemorySample`.

Add a new function in `performance.rs`:

```rust
/// Collect a rich memory sample by combining data from multiple VM service calls.
///
/// - `getMemoryUsage(isolateId)` → heap_usage, heap_capacity, external_usage
/// - `getIsolate(isolateId)` → RSS from isolate data (if available)
///
/// Fields that cannot be determined are set to 0.
pub async fn get_memory_sample(
    handle: &VmRequestHandle,
    isolate_id: &str,
) -> Option<MemorySample> {
    // 1. Get basic memory usage (existing call)
    let memory = get_memory_usage(handle, isolate_id).await?;

    // 2. Attempt to get RSS from isolate info
    let rss = get_isolate_rss(handle, isolate_id).await.unwrap_or(0);

    Some(MemorySample {
        dart_heap: memory.heap_usage,
        dart_native: memory.external_usage,
        raster_cache: 0, // Not available from standard VM service calls
        allocated: memory.heap_capacity,
        rss,
        timestamp: memory.timestamp,
    })
}
```

#### Add RSS extraction from `getIsolate`

The `getIsolate` response may contain memory-related fields. Add a helper:

```rust
/// Extract RSS from the isolate info response.
///
/// The Dart VM's `getIsolate` response includes a `_heaps` field with
/// `new` and `old` space details. RSS is approximated from the combined
/// capacity of both spaces plus external usage.
///
/// Returns `None` if the data is not available in the response.
async fn get_isolate_rss(
    handle: &VmRequestHandle,
    isolate_id: &str,
) -> Option<u64> {
    let response = handle.call_method("getIsolate", Some(&serde_json::json!({
        "isolateId": isolate_id,
    }))).await.ok()?;

    // Try to extract RSS from _heaps data
    let result = response.get("result")?;

    // The _heaps field contains new/old space details with capacity
    if let Some(heaps) = result.get("_heaps") {
        let new_cap = heaps.get("new")
            .and_then(|n| n.get("capacity"))
            .and_then(|c| c.as_u64())
            .unwrap_or(0);
        let old_cap = heaps.get("old")
            .and_then(|o| o.get("capacity"))
            .and_then(|c| c.as_u64())
            .unwrap_or(0);
        let external = heaps.get("external")
            .and_then(|e| e.as_u64())
            .unwrap_or(0);
        return Some(new_cap + old_cap + external);
    }

    None
}
```

**Note**: The exact JSON shape depends on the Dart VM version. This should be defensive with `.get()` chains and return `None`/`0` on any unexpected shape.

#### Shader compilation detection in frame events

In `timeline.rs`, update `parse_frame_timing` to detect shader compilation. The detection heuristic:

1. Check if the `Flutter.Frame` event data includes a `shaderCompilation` field (some Flutter versions expose this)
2. Fallback heuristic: if the raster time is significantly longer than the build time AND this is one of the first ~10 frames, it's likely shader compilation

```rust
pub fn parse_frame_timing(event: &StreamEvent) -> Option<FrameTiming> {
    // ... existing parsing ...
    let number = parse_str_u64(data.get("number")?)?;
    let elapsed = parse_str_u64(data.get("elapsed")?)?;
    let build = parse_str_u64(data.get("build")?)?;
    let raster = parse_str_u64(data.get("raster")?)?;

    // NEW: detect shader compilation
    let shader_compilation = data.get("shaderCompilation")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    Some(FrameTiming {
        number,
        build_micros: build,
        raster_micros: raster,
        elapsed_micros: elapsed,
        timestamp: chrono::Local::now(),
        phases: None,  // Phase breakdown not available from Flutter.Frame events
        shader_compilation,
    })
}
```

#### Export new function

Ensure `get_memory_sample` is exported from `vm_service/mod.rs`:

```rust
pub use performance::get_memory_sample;
```

### Acceptance Criteria

1. `get_memory_sample()` function exists and returns `MemorySample` by combining `getMemoryUsage` + `getIsolate` data
2. `get_memory_sample()` returns `None` if the base memory usage call fails
3. RSS defaults to 0 when `getIsolate` doesn't provide heap data
4. `raster_cache` is set to 0 (not currently extractable from standard VM service)
5. `parse_frame_timing()` populates `shader_compilation` from event data when available
6. `parse_frame_timing()` defaults `shader_compilation` to `false` when not in event data
7. `parse_frame_timing()` sets `phases: None` (phase breakdown requires timeline events, deferred)
8. All existing `fdemon-daemon` tests pass
9. `cargo check -p fdemon-daemon` passes

### Testing

Add tests in `performance.rs`:

```rust
#[test]
fn test_parse_memory_usage_still_works() {
    // Existing test — verify no regression
}

#[test]
fn test_get_isolate_rss_parses_heaps() {
    let response = serde_json::json!({
        "result": {
            "_heaps": {
                "new": { "capacity": 1_000_000, "used": 500_000 },
                "old": { "capacity": 10_000_000, "used": 8_000_000 },
                "external": 2_000_000
            }
        }
    });
    // Test the parsing logic (may need to extract into a pure function for testability)
}

#[test]
fn test_get_isolate_rss_missing_heaps_returns_none() {
    let response = serde_json::json!({ "result": {} });
    // Should return None
}
```

Add tests in `timeline.rs`:

```rust
#[test]
fn test_parse_frame_timing_with_shader_compilation() {
    let event = make_extension_event("Flutter.Frame", serde_json::json!({
        "number": "42",
        "elapsed": "20000",
        "build": "5000",
        "raster": "15000",
        "shaderCompilation": true,
    }));
    let timing = parse_frame_timing(&event).unwrap();
    assert!(timing.shader_compilation);
    assert!(timing.phases.is_none());
}

#[test]
fn test_parse_frame_timing_without_shader_field_defaults_false() {
    let event = make_extension_event("Flutter.Frame", serde_json::json!({
        "number": "1",
        "elapsed": "10000",
        "build": "5000",
        "raster": "5000",
    }));
    let timing = parse_frame_timing(&event).unwrap();
    assert!(!timing.shader_compilation);
}

#[test]
fn test_parse_frame_timing_new_fields_populated() {
    let event = make_extension_event("Flutter.Frame", serde_json::json!({
        "number": "1",
        "elapsed": "10000",
        "build": "5000",
        "raster": "5000",
    }));
    let timing = parse_frame_timing(&event).unwrap();
    assert_eq!(timing.phases, None);
    assert!(!timing.shader_compilation);
}
```

### Notes

- **RSS is best-effort**: The `_heaps` field in `getIsolate` responses is a private Dart VM API (prefixed with `_`). It may not be available in all Dart VM versions. The code must be defensive and default to 0.
- **Raster cache size**: Currently not extractable from standard VM service APIs. Set to 0. Future enhancement could use `ext.flutter.rasterCache` if exposed.
- **Phase breakdown deferred**: `Flutter.Frame` events only provide `build` and `raster` totals. The detailed build/layout/paint phase breakdown would require subscribing to the `Timeline` stream and parsing `TimelineEvents`, which is significantly more complex. `phases` remains `None` for now. The frame bar chart renders using `build_micros` (UI) and `raster_micros` (Raster) from `FrameTiming`.
- **`get_memory_sample` is async**: It makes two VM service calls sequentially (`getMemoryUsage` then `getIsolate`). The second call is best-effort and won't block if it fails.
- **No changes to existing polling**: The existing `spawn_performance_polling` in `actions.rs` still calls `get_memory_usage()`. Task 08 wires the new `get_memory_sample()` into the polling loop.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/vm_service/performance.rs` | Added `get_memory_sample()` async public function, private `get_isolate_rss()` async helper, test-only `parse_isolate_rss()` pure function for unit testing, updated imports to include `MemorySample`, added 4 new tests |
| `crates/fdemon-daemon/src/vm_service/timeline.rs` | Updated `parse_frame_timing()` to read `shaderCompilation` from `extensionData`, added `make_extension_event()` test helper, added 4 new tests |
| `crates/fdemon-daemon/src/vm_service/mod.rs` | Exported `get_memory_sample` from the `performance` re-export |

### Notable Decisions/Tradeoffs

1. **`parse_isolate_rss` test helper pattern**: `get_isolate_rss` is an `async` function that requires a real `VmRequestHandle`. To make the parsing logic independently testable without network calls, a `#[cfg(test)]` function `parse_isolate_rss` extracts the pure JSON parsing logic. This pattern mirrors how `parse_memory_usage` is a sync testable counterpart to `get_memory_usage`.

2. **Direct `result` access for `getIsolate`**: The `VmRequestHandle::request()` method returns the `result` field of the JSON-RPC response directly (not the full envelope). So `get_isolate_rss` accesses `result.get("_heaps")` at the top level of the returned value, matching the actual Dart VM protocol shape.

3. **`shader_compilation` reads from `extensionData`**: Flutter's `Flutter.Frame` events nest timing data under `extensionData`. The `shaderCompilation` field is also expected there (not at the top-level `data`), which is consistent with the event structure.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check -p fdemon-daemon` - Passed
- `cargo test -p fdemon-daemon` - Passed (357 tests; 3 ignored integration tests)
- `cargo clippy -p fdemon-daemon -- -D warnings` - Passed

### Risks/Limitations

1. **`_heaps` is a private Dart VM API**: The `_heaps` field in `getIsolate` responses is prefixed with `_` indicating it is a private/internal API. It may not be present in all Dart VM versions or Flutter targets. The code is fully defensive — `get_isolate_rss` returns `None` on any missing field, and `get_memory_sample` defaults `rss` to 0 via `unwrap_or(0)`.

2. **`raster_cache` remains 0**: As noted in the task, the raster cache size is not extractable from standard VM service APIs. Set to 0 until `ext.flutter.rasterCache` is exposed by Flutter.
