## Task: Memory Usage & GC RPCs

**Objective**: Implement VM Service RPC methods for fetching memory usage and allocation profiles, plus parsing logic for GC stream events. These are the raw data-fetching primitives that Task 05 (Memory Monitoring Integration) will orchestrate.

**Depends on**: 01-performance-data-models (for `MemoryUsage`, `GcEvent`, `AllocationProfile`, `ClassHeapStats` types)

**Estimated Time**: 3-4 hours

### Scope

- `crates/fdemon-daemon/src/vm_service/performance.rs`: **NEW** — Memory/GC RPC wrappers and parsers
- `crates/fdemon-daemon/src/vm_service/client.rs`: Add `RESUBSCRIBE_STREAMS` entry for `"GC"`
- `crates/fdemon-daemon/src/vm_service/mod.rs`: Add `pub mod performance` and re-exports

### Details

#### 1. getMemoryUsage RPC

The `getMemoryUsage` method returns current heap statistics for an isolate.

**Request:**
```json
{
    "jsonrpc": "2.0",
    "id": "42",
    "method": "getMemoryUsage",
    "params": { "isolateId": "isolates/1234" }
}
```

**Response:**
```json
{
    "jsonrpc": "2.0",
    "id": "42",
    "result": {
        "type": "MemoryUsage",
        "heapUsage": 52428800,
        "heapCapacity": 104857600,
        "externalUsage": 10485760
    }
}
```

Implement as a standalone function that takes a `VmRequestHandle`:

```rust
use fdemon_core::performance::MemoryUsage;

/// Fetch current memory usage for an isolate.
///
/// Calls the `getMemoryUsage` VM Service RPC and parses the response
/// into a `MemoryUsage` struct.
pub async fn get_memory_usage(
    handle: &VmRequestHandle,
    isolate_id: &str,
) -> Result<MemoryUsage> {
    let params = serde_json::json!({ "isolateId": isolate_id });
    let result = handle.request("getMemoryUsage", Some(params)).await?;
    parse_memory_usage(&result)
}

/// Parse a `getMemoryUsage` response into `MemoryUsage`.
pub fn parse_memory_usage(result: &serde_json::Value) -> Result<MemoryUsage> {
    let heap_usage = result.get("heapUsage")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| Error::protocol("missing heapUsage in getMemoryUsage response"))?;
    let heap_capacity = result.get("heapCapacity")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| Error::protocol("missing heapCapacity in getMemoryUsage response"))?;
    let external_usage = result.get("externalUsage")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| Error::protocol("missing externalUsage in getMemoryUsage response"))?;
    Ok(MemoryUsage {
        heap_usage,
        heap_capacity,
        external_usage,
        timestamp: chrono::Local::now(),
    })
}
```

#### 2. getAllocationProfile RPC

The `getAllocationProfile` method returns per-class heap allocation statistics.

**Request:**
```json
{
    "jsonrpc": "2.0",
    "id": "43",
    "method": "getAllocationProfile",
    "params": {
        "isolateId": "isolates/1234",
        "gc": true
    }
}
```

The `gc: true` parameter forces a GC before collecting the profile (more accurate but slower). Task 05 will decide when to use it.

**Response (simplified):**
```json
{
    "jsonrpc": "2.0",
    "id": "43",
    "result": {
        "type": "AllocationProfile",
        "members": [
            {
                "classRef": {
                    "type": "@Class",
                    "id": "classes/42",
                    "name": "String",
                    "library": { "type": "@Library", "name": "", "uri": "dart:core" }
                },
                "bytesCurrent": 10240,
                "instancesCurrent": 128,
                "accumulatedSize": 51200,
                "instancesAccumulated": 640
            }
        ],
        "memoryUsage": {
            "type": "MemoryUsage",
            "heapUsage": 52428800,
            "heapCapacity": 104857600,
            "externalUsage": 10485760
        }
    }
}
```

```rust
use fdemon_core::performance::{AllocationProfile, ClassHeapStats};

/// Fetch the allocation profile for an isolate.
///
/// When `gc` is true, forces a garbage collection before collecting the profile.
/// This produces more accurate numbers but is slower.
pub async fn get_allocation_profile(
    handle: &VmRequestHandle,
    isolate_id: &str,
    gc: bool,
) -> Result<AllocationProfile> {
    let mut params = serde_json::json!({ "isolateId": isolate_id });
    if gc {
        params["gc"] = serde_json::json!(true);
    }
    let result = handle.request("getAllocationProfile", Some(params)).await?;
    parse_allocation_profile(&result)
}

/// Parse a `getAllocationProfile` response.
pub fn parse_allocation_profile(result: &serde_json::Value) -> Result<AllocationProfile> {
    let members = result.get("members")
        .and_then(|v| v.as_array())
        .ok_or_else(|| Error::protocol("missing members in getAllocationProfile response"))?;

    let stats: Vec<ClassHeapStats> = members.iter()
        .filter_map(parse_class_heap_stats)
        .collect();

    Ok(AllocationProfile {
        members: stats,
        timestamp: chrono::Local::now(),
    })
}

fn parse_class_heap_stats(member: &serde_json::Value) -> Option<ClassHeapStats> {
    let class_ref = member.get("classRef")?;
    let class_name = class_ref.get("name")?.as_str()?.to_string();
    let library_uri = class_ref.get("library")
        .and_then(|lib| lib.get("uri"))
        .and_then(|v| v.as_str())
        .map(String::from);

    // The VM Service response uses `bytesCurrent` and `instancesCurrent`
    // for live objects, and `accumulatedSize`/`instancesAccumulated` for totals.
    // We map these to new/old space approximations:
    // - "current" values represent live (old space retained) objects
    // - The difference (accumulated - current) approximates new-space churn
    let bytes_current = member.get("bytesCurrent").and_then(|v| v.as_u64()).unwrap_or(0);
    let instances_current = member.get("instancesCurrent").and_then(|v| v.as_u64()).unwrap_or(0);
    let accumulated_size = member.get("accumulatedSize").and_then(|v| v.as_u64()).unwrap_or(0);
    let instances_accumulated = member.get("instancesAccumulated").and_then(|v| v.as_u64()).unwrap_or(0);

    Some(ClassHeapStats {
        class_name,
        library_uri,
        // Map current stats to old space (retained)
        old_space_instances: instances_current,
        old_space_size: bytes_current,
        // Map delta (accumulated - current) to new space (churn)
        new_space_instances: instances_accumulated.saturating_sub(instances_current),
        new_space_size: accumulated_size.saturating_sub(bytes_current),
    })
}
```

#### 3. GC Stream Subscription

Add `"GC"` to the resubscription list in `client.rs`:

```rust
// Before:
const RESUBSCRIBE_STREAMS: &[&str] = &["Extension", "Logging"];

// After:
const RESUBSCRIBE_STREAMS: &[&str] = &["Extension", "Logging", "GC"];
```

Also update `subscribe_flutter_streams()` to subscribe to the GC stream:

```rust
pub async fn subscribe_flutter_streams(&self) -> Vec<String> {
    let mut errors = Vec::new();

    if let Err(e) = self.stream_listen("Extension").await {
        errors.push(format!("Extension stream: {e}"));
    }
    if let Err(e) = self.stream_listen("Logging").await {
        errors.push(format!("Logging stream: {e}"));
    }
    // NEW: GC events for memory monitoring
    if let Err(e) = self.stream_listen("GC").await {
        errors.push(format!("GC stream: {e}"));
    }

    errors
}
```

#### 4. GC Event Parsing

Parse GC stream events in `performance.rs`:

```rust
use fdemon_core::performance::GcEvent;

/// Parse a GC stream event from the VM Service.
///
/// GC events have `kind: "GC"` in the stream event. The `gcType` field
/// contains the type of GC (e.g., "Scavenge", "MarkSweep").
pub fn parse_gc_event(event: &StreamEvent) -> Option<GcEvent> {
    if event.kind != "GC" {
        return None;
    }

    let gc_type = event.data.get("gcType")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown")
        .to_string();

    let reason = event.data.get("reason")
        .and_then(|v| v.as_str())
        .map(String::from);

    let isolate_id = event.isolate.as_ref().map(|iso| iso.id.clone());

    Some(GcEvent {
        gc_type,
        reason,
        isolate_id,
        timestamp: chrono::Local::now(),
    })
}
```

### Acceptance Criteria

1. `get_memory_usage()` sends correct `getMemoryUsage` JSON-RPC and parses `heapUsage`, `heapCapacity`, `externalUsage`
2. `parse_memory_usage()` handles all three fields correctly
3. `parse_memory_usage()` returns `Error::Protocol` on missing fields
4. `get_allocation_profile(gc: false)` omits the `gc` parameter
5. `get_allocation_profile(gc: true)` includes `"gc": true` in the request
6. `parse_allocation_profile()` extracts class name, library URI, and byte/instance counts
7. `parse_class_heap_stats()` handles missing optional fields gracefully
8. `"GC"` added to `RESUBSCRIBE_STREAMS` and `subscribe_flutter_streams()`
9. `parse_gc_event()` parses `gcType`, `reason`, and `isolate` from GC stream events
10. `parse_gc_event()` returns `None` for non-GC events

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_memory_usage() {
        let result = json!({
            "type": "MemoryUsage",
            "heapUsage": 52428800,
            "heapCapacity": 104857600,
            "externalUsage": 10485760
        });
        let mem = parse_memory_usage(&result).unwrap();
        assert_eq!(mem.heap_usage, 52428800);
        assert_eq!(mem.heap_capacity, 104857600);
        assert_eq!(mem.external_usage, 10485760);
    }

    #[test]
    fn test_parse_memory_usage_missing_field() {
        let result = json!({ "heapUsage": 100, "heapCapacity": 200 });
        assert!(parse_memory_usage(&result).is_err());
    }

    #[test]
    fn test_parse_allocation_profile() {
        let result = json!({
            "type": "AllocationProfile",
            "members": [
                {
                    "classRef": {
                        "type": "@Class",
                        "id": "classes/42",
                        "name": "String",
                        "library": { "type": "@Library", "name": "", "uri": "dart:core" }
                    },
                    "bytesCurrent": 10240,
                    "instancesCurrent": 128,
                    "accumulatedSize": 20480,
                    "instancesAccumulated": 256
                }
            ]
        });
        let profile = parse_allocation_profile(&result).unwrap();
        assert_eq!(profile.members.len(), 1);
        assert_eq!(profile.members[0].class_name, "String");
        assert_eq!(profile.members[0].library_uri.as_deref(), Some("dart:core"));
        assert_eq!(profile.members[0].old_space_size, 10240);
        assert_eq!(profile.members[0].old_space_instances, 128);
        assert_eq!(profile.members[0].new_space_size, 10240); // 20480 - 10240
        assert_eq!(profile.members[0].new_space_instances, 128); // 256 - 128
    }

    #[test]
    fn test_parse_allocation_profile_empty() {
        let result = json!({ "type": "AllocationProfile", "members": [] });
        let profile = parse_allocation_profile(&result).unwrap();
        assert!(profile.members.is_empty());
    }

    #[test]
    fn test_parse_gc_event() {
        let event = StreamEvent {
            kind: "GC".to_string(),
            isolate: Some(IsolateRef {
                id: "isolates/1234".to_string(),
                name: "main".to_string(),
                number: None,
                is_system_isolate: Some(false),
            }),
            timestamp: Some(1704067200000),
            data: json!({
                "gcType": "Scavenge",
                "reason": "allocation"
            }),
        };
        let gc = parse_gc_event(&event).unwrap();
        assert_eq!(gc.gc_type, "Scavenge");
        assert_eq!(gc.reason.as_deref(), Some("allocation"));
        assert_eq!(gc.isolate_id.as_deref(), Some("isolates/1234"));
    }

    #[test]
    fn test_parse_gc_event_not_gc() {
        let event = StreamEvent {
            kind: "Extension".to_string(),
            isolate: None,
            timestamp: None,
            data: json!({}),
        };
        assert!(parse_gc_event(&event).is_none());
    }

    #[test]
    fn test_parse_gc_event_minimal() {
        let event = StreamEvent {
            kind: "GC".to_string(),
            isolate: None,
            timestamp: None,
            data: json!({}),
        };
        let gc = parse_gc_event(&event).unwrap();
        assert_eq!(gc.gc_type, "Unknown");
        assert!(gc.reason.is_none());
        assert!(gc.isolate_id.is_none());
    }
}
```

### Notes

- **Function signatures take `&VmRequestHandle`** rather than `&VmServiceClient`. This is intentional — callers will use the handle from Task 02, not the full client.
- **`getAllocationProfile` is expensive** — it forces the VM to walk the heap. Task 05 will call it infrequently (e.g., on user request or every 30s), not every polling cycle.
- **GC stream events are high-frequency** — the Dart VM scavenges new-space very frequently (multiple times per second). Task 05's handler should batch or throttle if needed.
- **The response format for `getAllocationProfile`** uses `bytesCurrent`/`instancesCurrent` for retained objects and `accumulatedSize`/`instancesAccumulated` for lifetime totals. We map these to old-space (retained) and new-space (churn) respectively. This is an approximation — the actual new/old space split isn't directly exposed.
- **`parse_memory_usage` uses `u64`** rather than `i64` because the VM Service protocol specifies these as non-negative integers. The `as_u64()` serde method handles this.
