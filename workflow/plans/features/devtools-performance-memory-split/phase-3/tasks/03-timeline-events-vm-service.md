## Task: Timeline Events VM Service RPCs

**Objective**: Add the raw VM-level `getVMTimeline` and `getVMTimelineMicros` RPCs (NOT Flutter extensions — they live on the VM Service directly) plus a `fetch_timeline_chunk` convenience wrapper that the app layer's 1-Hz polling loop can call. Returns parsed `Vec<TimelineEvent>` using T01's parser.

**Depends on**: T01 (`fdemon-core::timeline::{TimelineEvent, parse_vm_timeline}`)

**Agent:** implementor

**Estimated Time**: 3–4 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/vm_service/timeline.rs`: extend the existing module with:
  - `pub async fn get_vm_timeline_micros(handle: &VmRequestHandle) -> Result<u64>` — wraps `getVMTimelineMicros`.
  - `pub async fn fetch_timeline_chunk(handle: &VmRequestHandle, since_micros: u64, extent_micros: u64, thread_name_map: &mut HashMap<i64, String>) -> Result<Vec<TimelineEvent>>` — wraps `getVMTimeline` with `timeOriginMicros` + `timeExtentMicros` params, calls `parse_vm_timeline`.
  - Unit tests against canned JSON fixtures.
  - No changes to existing `parse_frame_timing` / `enable_frame_tracking` / `flutter_extension_kind` / `is_frame_event` / `parse_str_u64` (Phase 2 surface preserved).

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/vm_service/client.rs:329–390`: `VmRequestHandle::request` signature (the underlying RPC mechanism — `getVMTimeline` is NOT an extension call).
- `crates/fdemon-daemon/src/vm_service/timeline.rs`: existing module structure + `Result` type alias.
- T01 outputs (`TimelineEvent`, `TimelinePhase`, `TimelineThread`, `parse_vm_timeline`).

### Details

#### `getVMTimeline` and `getVMTimelineMicros` — VM Service RPCs

These are **raw VM Service methods**, not Flutter extensions. The protocol shape:

```jsonc
// Request
{
  "method": "getVMTimeline",
  "params": {
    "timeOriginMicros": 12345000,
    "timeExtentMicros":    50000
  }
}

// Response (subset)
{
  "type": "Timeline",
  "traceEvents": [
    { "name": "thread_name", "ph": "M", "tid": 1, "args": { "name": "1.ui (1234)" } },
    { "name": "Frame", "cat": "Embedder", "tid": 1, "ph": "X", "ts": 12350000, "dur": 8000, "args": { "frame_number": "42" } }
  ],
  "timeOriginMicros": 12345000,
  "timeExtentMicros":    50000
}
```

`getVMTimelineMicros` returns:

```json
{ "type": "Timestamp", "timestamp": 12395000 }
```

#### `timeline.rs` additions

Append below the existing `parse_str_u64` (line 119) and `enable_frame_tracking` (line 141):

```rust
use fdemon_core::timeline::{TimelineEvent, parse_vm_timeline};
use serde_json::json;
use std::collections::HashMap;

/// Wrap the VM Service `getVMTimelineMicros` RPC. Returns the current VM
/// timeline clock value in microseconds.
pub async fn get_vm_timeline_micros(handle: &VmRequestHandle) -> Result<u64> {
    let response = handle.request("getVMTimelineMicros", None).await?;
    let ts = response
        .get("timestamp")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| Error::protocol("getVMTimelineMicros response missing timestamp"))?;
    Ok(ts.max(0) as u64)
}

/// Fetch a slice of the VM timeline (`getVMTimeline`) covering the window
/// `[since_micros, since_micros + extent_micros)` and return it as a vector
/// of parsed `TimelineEvent`s with thread classification applied.
///
/// `thread_name_map` is the caller's persistent `tid → thread name` cache —
/// it is updated in place as metadata events arrive. Pass a fresh
/// `HashMap::new()` on the very first call; reuse for subsequent calls.
///
/// Returns an empty vec if the VM had no events in the window.
pub async fn fetch_timeline_chunk(
    handle: &VmRequestHandle,
    since_micros: u64,
    extent_micros: u64,
    thread_name_map: &mut HashMap<i64, String>,
) -> Result<Vec<TimelineEvent>> {
    let params = json!({
        "timeOriginMicros": since_micros as i64,
        "timeExtentMicros": extent_micros as i64,
    });
    let response = handle.request("getVMTimeline", Some(params)).await?;
    parse_vm_timeline(&response, thread_name_map)
}
```

> **Decision: pass `&mut HashMap<i64, String>` instead of caching on the daemon.** Thread-name lookups are stateful across calls because metadata events arrive once at process start and never again. The caller (T04's spawn loop) owns the cache and threads it through each `fetch_timeline_chunk` call. This keeps the daemon function pure and avoids hidden global state.

> **Decision: `i64` cast safety.** `timeOriginMicros` and `timeExtentMicros` use `i64` per VM Service protocol. Casting `u64 → i64` is safe for values < `i64::MAX` (~292 years of micros). We document the assumption inline (see Notes).

#### Why `request` and not `call_extension`?

`getVMTimeline` is a top-level VM Service method, not an extension. It does NOT carry an `isolateId` argument and is NOT prefixed with `ext.`. Use `VmRequestHandle::request(method, params)` directly. Reference: existing call sites in `client.rs` for `getVM`, `getIsolate`, `streamListen`.

### Acceptance Criteria

1. `crates/fdemon-daemon/src/vm_service/timeline.rs` exports `pub async fn get_vm_timeline_micros` and `pub async fn fetch_timeline_chunk` with the signatures above.
2. Existing `parse_frame_timing`, `enable_frame_tracking`, `flutter_extension_kind`, `is_frame_event`, `parse_str_u64` are unchanged (verify byte-equal except for trailing additions).
3. `fetch_timeline_chunk` issues a single `request("getVMTimeline", ...)` with both `timeOriginMicros` and `timeExtentMicros` set; the request goes through `VmRequestHandle`, not through `call_extension`.
4. `parse_vm_timeline` is called from T01 — `fetch_timeline_chunk` is purely a transport wrapper.
5. `cargo check -p fdemon-daemon` passes.
6. `cargo test -p fdemon-daemon` includes the new unit tests below — all green.
7. `cargo clippy -p fdemon-daemon --all-targets -- -D warnings` is clean.

### Testing

`crates/fdemon-daemon/src/vm_service/timeline.rs` — extend `#[cfg(test)] mod tests`:

- `get_vm_timeline_micros_parses_timestamp` — mock response `{ "type": "Timestamp", "timestamp": 12345 }` → `Ok(12345)`.
- `get_vm_timeline_micros_missing_timestamp_errors`.
- `get_vm_timeline_micros_negative_clamped_to_zero` — `{ "timestamp": -1 }` → `Ok(0)`.
- `fetch_timeline_chunk_sends_correct_method` — capture mock request, assert `method == "getVMTimeline"`.
- `fetch_timeline_chunk_sends_origin_and_extent` — capture params, assert both keys present with the passed values cast to `i64`.
- `fetch_timeline_chunk_parses_empty_response` — `{ "type": "Timeline", "traceEvents": [] }` → empty vec.
- `fetch_timeline_chunk_parses_metadata_and_ui_event_fixture` — full fixture with metadata `thread_name` events for UI + Raster threads, plus 2 `Frame` and 1 `GC` event. Verify: 3 events returned (metadata excluded), thread classifications correct, `thread_name_map` populated.
- `fetch_timeline_chunk_accumulates_thread_name_map_across_calls` — call twice with disjoint metadata; second call's events benefit from first call's metadata.
- `fetch_timeline_chunk_propagates_request_errors` — mock request returns `Err`, asserts error is propagated.

### Notes

- **`u64 → i64` cast for `timeOriginMicros`/`timeExtentMicros`** — VM Service uses `i64` per protocol. Real timeline values stay well under `i64::MAX` (months of micros, not centuries). The cast is documented in an inline comment; no `saturating_cast` because `as i64` overflow would still produce a defined value and the VM clamps internally.
- **`thread_name_map` ownership lives in the caller** — T04 stores it on a `SessionHandle` extension struct (or alongside `timeline_events` ring buffer). This task does not own the cache; it just mutates the passed reference.
- **Metadata events are silently dropped from the returned vec** — they have no presentation value; their only role is populating `thread_name_map`.
- **No retry / debounce logic in this layer** — T04's spawn loop handles polling cadence, pause/resume, and shutdown signals. This task is a pure transport wrapper.
- **Use `json!` macro for params construction** rather than `HashMap<String, String>`. `getVMTimeline` accepts integer params, unlike Flutter extensions which require all-string args. The `VmRequestHandle::request` signature accepts `Option<serde_json::Value>` — pass the `json!({...})` value directly.
- **No `pub mod` declarations needed** — the additions go into the existing `timeline.rs` module which is already declared in `vm_service/mod.rs`.
- **`parse_vm_timeline` signature** (from T01): `(response: &serde_json::Value, thread_name_map: &mut HashMap<i64, String>) -> Result<Vec<TimelineEvent>>`. Confirm this matches; if T01 ends up with a different signature, this task adapts the call but keeps `fetch_timeline_chunk`'s public signature unchanged.
