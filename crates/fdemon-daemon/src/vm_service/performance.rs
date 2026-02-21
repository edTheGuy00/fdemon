//! VM Service RPC wrappers for memory usage and GC event parsing.
//!
//! This module provides functions for fetching heap memory statistics and
//! allocation profiles from the Dart VM Service, plus parsing logic for
//! GC stream events.
//!
//! ## Callers
//!
//! All public functions take a [`VmRequestHandle`] rather than a full
//! [`crate::vm_service::VmServiceClient`]. This allows callers to share the
//! handle across background tasks without holding a reference to the whole
//! client.
//!
//! ## Note on `getAllocationProfile`
//!
//! This RPC is expensive — it forces the VM to walk the entire heap. Callers
//! should invoke it infrequently (e.g., on user request or on a long timer),
//! not in tight polling loops.

use fdemon_core::performance::{
    AllocationProfile, ClassHeapStats, GcEvent, MemorySample, MemoryUsage,
};
use fdemon_core::prelude::*;

use super::client::VmRequestHandle;
use super::protocol::{IsolateRef, StreamEvent};

// ── getMemoryUsage ────────────────────────────────────────────────────────────

/// Fetch current memory usage for an isolate.
///
/// Calls the `getMemoryUsage` VM Service RPC and parses the response
/// into a [`MemoryUsage`] struct.
///
/// # Errors
///
/// Returns [`Error::ChannelClosed`] if the background task has exited,
/// [`Error::Protocol`] if required fields are missing from the response,
/// or a transport error if the request fails.
pub async fn get_memory_usage(handle: &VmRequestHandle, isolate_id: &str) -> Result<MemoryUsage> {
    let params = serde_json::json!({ "isolateId": isolate_id });
    let result = handle.request("getMemoryUsage", Some(params)).await?;
    parse_memory_usage(&result)
}

/// Parse a `getMemoryUsage` response into [`MemoryUsage`].
///
/// Expects a JSON object with `heapUsage`, `heapCapacity`, and
/// `externalUsage` fields (all unsigned integers in bytes).
///
/// # Errors
///
/// Returns [`Error::Protocol`] if any required field is missing or has
/// an unexpected type.
pub fn parse_memory_usage(result: &serde_json::Value) -> Result<MemoryUsage> {
    let heap_usage = result
        .get("heapUsage")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| Error::protocol("missing heapUsage in getMemoryUsage response"))?;
    let heap_capacity = result
        .get("heapCapacity")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| Error::protocol("missing heapCapacity in getMemoryUsage response"))?;
    let external_usage = result
        .get("externalUsage")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| Error::protocol("missing externalUsage in getMemoryUsage response"))?;

    Ok(MemoryUsage {
        heap_usage,
        heap_capacity,
        external_usage,
        timestamp: chrono::Local::now(),
    })
}

// ── get_memory_sample ─────────────────────────────────────────────────────────

/// Collect a rich memory sample by combining data from multiple VM service calls.
///
/// - `getMemoryUsage(isolateId)` → heap_usage, heap_capacity, external_usage
/// - `getIsolate(isolateId)` → RSS approximated from `_heaps` field (if available)
///
/// Returns `None` if the base `getMemoryUsage` call fails.
/// Fields that cannot be determined from the VM service are set to 0.
pub async fn get_memory_sample(handle: &VmRequestHandle, isolate_id: &str) -> Option<MemorySample> {
    // 1. Get basic memory usage (required — failure returns None).
    let memory = get_memory_usage(handle, isolate_id).await.ok()?;

    // 2. Attempt to get RSS from isolate info — best-effort, defaults to 0.
    let rss = get_isolate_rss(handle, isolate_id).await.unwrap_or(0);

    Some(MemorySample {
        dart_heap: memory.heap_usage,
        dart_native: memory.external_usage,
        // Raster cache is not available from standard VM service APIs.
        // Future enhancement: use ext.flutter.rasterCache if exposed.
        raster_cache: 0,
        allocated: memory.heap_capacity,
        rss,
        timestamp: memory.timestamp,
    })
}

/// Extract an RSS approximation from the `getIsolate` response.
///
/// The Dart VM's `getIsolate` response may include a `_heaps` field with
/// `new` and `old` space details. RSS is approximated from the combined
/// capacity of both spaces plus external usage.
///
/// This field is a private Dart VM API (prefixed with `_`) and may not be
/// present in all Dart VM versions. Returns `None` if the data is unavailable.
async fn get_isolate_rss(handle: &VmRequestHandle, isolate_id: &str) -> Option<u64> {
    let result = handle
        .request(
            "getIsolate",
            Some(serde_json::json!({ "isolateId": isolate_id })),
        )
        .await
        .ok()?;

    // The _heaps field contains new/old space details with capacity values.
    // `handle.request()` returns the `result` field of the JSON-RPC response
    // directly, so the isolate object is at the top level.
    let heaps = result.get("_heaps")?;

    let new_cap = heaps
        .get("new")
        .and_then(|n| n.get("capacity"))
        .and_then(|c| c.as_u64())
        .unwrap_or(0);
    let old_cap = heaps
        .get("old")
        .and_then(|o| o.get("capacity"))
        .and_then(|c| c.as_u64())
        .unwrap_or(0);
    let external = heaps.get("external").and_then(|e| e.as_u64()).unwrap_or(0);

    Some(new_cap + old_cap + external)
}

/// Parse RSS from a raw `getIsolate` JSON result for unit testing.
///
/// Extracts RSS from the `_heaps` field of the isolate result.
/// Returns `None` if `_heaps` is absent.
#[cfg(test)]
fn parse_isolate_rss(result: &serde_json::Value) -> Option<u64> {
    let heaps = result.get("_heaps")?;

    let new_cap = heaps
        .get("new")
        .and_then(|n| n.get("capacity"))
        .and_then(|c| c.as_u64())
        .unwrap_or(0);
    let old_cap = heaps
        .get("old")
        .and_then(|o| o.get("capacity"))
        .and_then(|c| c.as_u64())
        .unwrap_or(0);
    let external = heaps.get("external").and_then(|e| e.as_u64()).unwrap_or(0);

    Some(new_cap + old_cap + external)
}

// ── getAllocationProfile ──────────────────────────────────────────────────────

/// Fetch the allocation profile for an isolate.
///
/// When `gc` is `true`, forces a garbage collection before collecting the
/// profile. This produces more accurate numbers but is slower.
///
/// # Errors
///
/// Returns [`Error::ChannelClosed`] if the background task has exited,
/// [`Error::Protocol`] if the `members` field is missing, or a transport
/// error if the request fails.
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
///
/// Extracts per-class heap statistics from the `members` array. Entries that
/// cannot be parsed are silently skipped (graceful degradation).
///
/// # Errors
///
/// Returns [`Error::Protocol`] if the `members` field is absent or not an
/// array.
pub fn parse_allocation_profile(result: &serde_json::Value) -> Result<AllocationProfile> {
    let members = result
        .get("members")
        .and_then(|v| v.as_array())
        .ok_or_else(|| Error::protocol("missing members in getAllocationProfile response"))?;

    let stats: Vec<ClassHeapStats> = members.iter().filter_map(parse_class_heap_stats).collect();

    if stats.is_empty() && !members.is_empty() {
        // All members were filtered out — the response format may differ from
        // what we expect. Log the first member's keys for debugging.
        if let Some(first) = members.first() {
            tracing::warn!(
                "getAllocationProfile: {} raw members but 0 parsed. First member keys: {:?}",
                members.len(),
                first.as_object().map(|o| o.keys().collect::<Vec<_>>()),
            );
        }
    }

    Ok(AllocationProfile {
        members: stats,
        timestamp: chrono::Local::now(),
    })
}

/// Parse a single `member` entry from an `getAllocationProfile` response.
///
/// Returns `None` if the entry is missing required fields (e.g., `classRef`
/// or `name`), allowing the caller to skip malformed entries gracefully.
///
/// ## Field mapping
///
/// The VM Service protocol exposes:
/// - `bytesCurrent` / `instancesCurrent` — live (retained) objects
/// - `accumulatedSize` / `instancesAccumulated` — lifetime totals
///
/// We map these to old-space (retained) and new-space (churn) respectively.
/// The difference `accumulated - current` approximates new-space allocation
/// churn. This is an approximation — the real new/old split is not directly
/// exposed by the protocol.
fn parse_class_heap_stats(member: &serde_json::Value) -> Option<ClassHeapStats> {
    // The wire JSON uses "class" (the protocol field name), not "classRef"
    // (the Dart client renames it because `class` is a Dart reserved keyword).
    let class_ref = member.get("class").or_else(|| member.get("classRef"))?;
    let class_name = class_ref.get("name")?.as_str()?.to_string();
    let library_uri = class_ref
        .get("library")
        .and_then(|lib| lib.get("uri"))
        .and_then(|v| v.as_str())
        .map(String::from);

    let bytes_current = member
        .get("bytesCurrent")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let instances_current = member
        .get("instancesCurrent")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let accumulated_size = member
        .get("accumulatedSize")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let instances_accumulated = member
        .get("instancesAccumulated")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

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

// ── GC stream event parsing ───────────────────────────────────────────────────

/// Parse a GC stream event from the VM Service.
///
/// GC events have `kind: "GC"` in the stream event. The `gcType` field
/// contains the type of GC (e.g., `"Scavenge"`, `"MarkSweep"`).
///
/// Returns `None` for non-GC events (i.e., when `event.kind != "GC"`).
///
/// ## Note on frequency
///
/// GC stream events are high-frequency — the Dart VM scavenges new-space very
/// frequently (potentially multiple times per second). Consumers should batch
/// or throttle if they do any non-trivial work per event.
pub fn parse_gc_event(event: &StreamEvent) -> Option<GcEvent> {
    if event.kind != "GC" {
        return None;
    }

    let gc_type = event
        .data
        .get("gcType")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown")
        .to_string();

    let reason = event
        .data
        .get("reason")
        .and_then(|v| v.as_str())
        .map(String::from);

    let isolate_id = event
        .isolate
        .as_ref()
        .map(|iso: &IsolateRef| iso.id.clone());

    Some(GcEvent {
        gc_type,
        reason,
        isolate_id,
        timestamp: chrono::Local::now(),
    })
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_memory_usage_still_works() {
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
    fn test_get_isolate_rss_parses_heaps() {
        // Simulate a `getIsolate` result (the `result` field of the JSON-RPC
        // response) containing the private `_heaps` field.
        let result = json!({
            "_heaps": {
                "new": { "capacity": 1_000_000_u64, "used": 500_000_u64 },
                "old": { "capacity": 10_000_000_u64, "used": 8_000_000_u64 },
                "external": 2_000_000_u64
            }
        });
        let rss = parse_isolate_rss(&result);
        assert_eq!(rss, Some(13_000_000));
    }

    #[test]
    fn test_get_isolate_rss_missing_heaps_returns_none() {
        let result = json!({ "type": "Isolate", "id": "isolates/1" });
        let rss = parse_isolate_rss(&result);
        assert!(rss.is_none());
    }

    #[test]
    fn test_get_isolate_rss_partial_heaps() {
        // Only `new` space is present — `old` and `external` default to 0.
        let result = json!({
            "_heaps": {
                "new": { "capacity": 500_000_u64, "used": 200_000_u64 }
            }
        });
        let rss = parse_isolate_rss(&result);
        assert_eq!(rss, Some(500_000));
    }

    #[test]
    fn test_get_isolate_rss_empty_heaps_object() {
        // `_heaps` exists but all sub-fields are absent — each defaults to 0.
        let result = json!({ "_heaps": {} });
        let rss = parse_isolate_rss(&result);
        assert_eq!(rss, Some(0));
    }

    #[test]
    fn test_parse_memory_usage_missing_field() {
        let result = json!({ "heapUsage": 100, "heapCapacity": 200 });
        assert!(parse_memory_usage(&result).is_err());
    }

    #[test]
    fn test_parse_allocation_profile() {
        // Wire format uses "class" (not "classRef") — the Dart client renames
        // it because `class` is a reserved keyword in Dart.
        let result = json!({
            "type": "AllocationProfile",
            "members": [
                {
                    "class": {
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
