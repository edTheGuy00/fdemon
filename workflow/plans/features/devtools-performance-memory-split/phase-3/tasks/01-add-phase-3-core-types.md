## Task: Add Phase 3 Core Types (Rebuild Stats + Timeline)

**Objective**: Introduce `fdemon-core` domain types and parsers for the two Phase 3 data flows: widget rebuild stats (event payload + LocationMap + per-frame snapshot) and VM timeline events (Chrome-trace shape + thread classification). Pure data types and parsers, no app integration.

**Depends on**: None (Wave 1)

**Agent:** implementor

**Estimated Time**: 3–4 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-core/src/rebuild_stats.rs` (NEW): types + parsers + module docstring
- `crates/fdemon-core/src/timeline.rs` (NEW): types + parsers + module docstring
- `crates/fdemon-core/src/lib.rs`: add `pub mod rebuild_stats;` and `pub mod timeline;` to the module declarations (insert alphabetically next to existing `pub mod performance;`)

**Files Read (Dependencies):**
- `crates/fdemon-core/src/performance.rs`: confirm `FrameTiming.number` field type so `RebuildStatsSnapshot.frame_number` matches it.
- `crates/fdemon-core/src/lib.rs`: confirm `pub mod` ordering convention.
- `crates/fdemon-core/src/error.rs`: use the project `Error` enum for parse failures (no `anyhow`).

### Details

#### rebuild_stats.rs — Type definitions

The DevTools `Flutter.RebuiltWidgets` Extension event payload (verbatim from `tmp/devtools/.../rebuild_stats_model.dart` example comment):

```json
{
  "startTime": 2352949,
  "frameNumber": 57,
  "events": [1, 1, 2, 1, 5, 3],
  "locations": {
    "package:foo/main.dart": {
      "ids": [1, 2, ...],
      "lines": [23, 32, ...],
      "columns": [10, 12, ...],
      "names": ["PlanetsApp", "MaterialApp", ...]
    }
  }
}
```

`events` is a flat `[id, count, id, count, …]` pair list. `locations` is OPTIONAL — only present when new IDs are introduced; subsequent events for known IDs omit it.

```rust
//! # Rebuild Stats
//!
//! Domain types for the Flutter widget-rebuild profiler. Wraps the
//! `Flutter.RebuiltWidgets` Extension stream and the
//! `ext.flutter.inspector.widgetLocationIdMap` RPC.

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A single widget location entry (file:line:column + class name).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Location {
    pub file_uri: String,
    pub line: u32,
    pub column: u32,
    pub name: String,
}

/// Map from numeric location ID (assigned by the engine) to a `Location`.
/// Built incrementally from the `locations` sub-object that ships inside
/// `Flutter.RebuiltWidgets` events and from one-shot
/// `ext.flutter.inspector.widgetLocationIdMap` responses.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocationMap {
    pub by_id: HashMap<u32, Location>,
}

impl LocationMap {
    /// Merge a parallel-arrays location block (the shape used by both the
    /// event payload and the `widgetLocationIdMap` RPC) into this map.
    pub fn merge_parallel_arrays(&mut self, file_uri: &str, value: &serde_json::Value) -> Result<()> {
        // Read `ids`, `lines`, `columns`, `names` arrays. All must be equal
        // length. Insert one Location per index. Existing IDs are overwritten
        // (locations are immutable per the engine — only seen IDs ever change).
        // Return Error::protocol(...) on shape mismatch.
        // ...
        Ok(())
    }
}

/// One row of a per-frame rebuild snapshot: a location + the number of times
/// it rebuilt in that frame.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RebuildLocation {
    pub location: Location,
    pub build_count: u32,
}

/// All widget rebuilds observed during a single frame.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RebuildStatsSnapshot {
    pub frame_number: u64,
    pub start_time_micros: u64,
    pub rebuilds: Vec<RebuildLocation>,
}

/// Parsed `Flutter.RebuiltWidgets` event payload — pure transport shape.
/// Aggregation lives in the app layer (`fdemon-app`).
#[derive(Debug, Clone)]
pub struct RebuildEventPayload {
    pub frame_number: u64,
    pub start_time_micros: u64,
    /// Flat `[id, count, id, count, ...]` pairs.
    pub events: Vec<(u32, u32)>,
    /// Optional new locations introduced in this event.
    /// Caller (`fdemon-app`) merges into its persistent `LocationMap`.
    pub new_locations: Option<HashMap<String, serde_json::Value>>,
}

/// Parse a `Flutter.RebuiltWidgets` event's `extensionData` JSON object into
/// a structured payload. Returns `Error::protocol(...)` on malformed input.
pub fn parse_rebuilt_widgets_event(extension_data: &serde_json::Value) -> Result<RebuildEventPayload> {
    // Read frameNumber (i64 → u64), startTime (i64 → u64), events (array of i64, must be even-length).
    // Pair into (u32, u32). locations is optional — pass through as-is for the caller to merge.
    // ...
}
```

> **Decision: keep `RebuildEventPayload::new_locations` as raw JSON** to avoid making `fdemon-core` carry merge state. The app layer holds the persistent `LocationMap` and calls `merge_parallel_arrays` per file URI.

#### timeline.rs — Type definitions

The Chrome-trace JSON event shape (from `getVMTimeline` response — VM Service protocol, `Timeline` object's `traceEvents`):

```json
{
  "name": "Frame",
  "cat": "Embedder",
  "tid": 12345,
  "pid": 1,
  "ph": "X",
  "ts": 1234567,
  "dur": 8765,
  "args": { "frame_number": "42", ... }
}
```

`ph` is one of `B` (begin), `E` (end), `X` (complete with dur), `i` (instant), `M` (metadata), etc.

```rust
//! # Timeline Events
//!
//! Domain types for the Dart VM timeline. Wraps the `getVMTimeline` RPC
//! response (Chrome-trace JSON shape, not Perfetto protobuf).

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TimelineThread {
    /// Flutter UI thread (Dart isolate executing build/layout/paint).
    Ui,
    /// Flutter Raster thread (formerly GPU thread — submits to graphics).
    Raster,
    /// Anything else (Embedder, GC, Compiler, IO, etc.).
    Other,
}

/// A single Chrome-trace timeline event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimelineEvent {
    pub name: String,
    pub category: String,
    pub thread: TimelineThread,
    /// Raw `tid` from the event — needed for thread-name lookup.
    pub tid: i64,
    pub phase: TimelinePhase,
    /// Timestamp in microseconds (monotonic VM clock).
    pub ts: u64,
    /// Duration in microseconds. Only set for `ph == X` complete events.
    pub dur: Option<u64>,
    /// Frame number if the event's args contain `frame_number` or
    /// `flutterFrameNumber`. Used for per-frame correlation.
    pub frame_number: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimelinePhase {
    /// Begin (`ph: "B"`) — duration unknown until matching End event.
    Begin,
    /// End (`ph: "E"`).
    End,
    /// Complete (`ph: "X"`) — duration is set on the event itself.
    Complete,
    /// Instant (`ph: "i"`).
    Instant,
    /// Other (metadata, async, etc.) — kept but ignored by the filter.
    Other,
}

/// Parse the JSON-encoded VM Service `Timeline` object (response of
/// `getVMTimeline`) into a `Vec<TimelineEvent>`.
///
/// `thread_name_map` maps `tid` → human thread name (extracted separately
/// from the metadata events — `ph: "M", name: "thread_name"`). Pass an
/// empty map on the first call; subsequent calls accumulate.
pub fn parse_vm_timeline(
    response: &serde_json::Value,
    thread_name_map: &mut HashMap<i64, String>,
) -> Result<Vec<TimelineEvent>> {
    // 1. Walk response["traceEvents"] as array.
    // 2. For each event:
    //    a. If ph == "M" and name == "thread_name", record args["name"] → thread_name_map[tid].
    //    b. Otherwise build a TimelineEvent. Classify thread via classify_thread(name).
    //    c. Extract frame_number from args["frame_number"] OR args["flutterFrameNumber"]
    //       (handle both string and integer JSON forms).
    // 3. Discard metadata events from the returned vec.
    // ...
}

fn classify_thread(thread_name: &str) -> TimelineThread {
    // Upstream DevTools rules (from timeline_event_processor.dart):
    //   * Name contains ".ui" and not ".flutter.test..ui"  → Ui
    //   * Name contains ".raster"                          → Raster
    //   * Name contains ".platform" (fallback, macOS)      → Raster
    //   * Otherwise                                        → Other
    // The single-track Flutter tester case ("io.flutter.test..ui") maps to
    // Ui here — Raster events on tester run on the same thread and are
    // best-effort filtered by event name in T04.
    // ...
}
```

> **Decision: classify by thread NAME via the metadata events**, not by raw `tid`. The metadata-event pattern (`ph: "M", name: "thread_name", args: { name: "1.ui (12345)" }`) is stable across Dart VM versions; raw tid mapping varies per OS.

### Acceptance Criteria

1. `crates/fdemon-core/src/rebuild_stats.rs` exists with: `Location`, `LocationMap` (with `merge_parallel_arrays`), `RebuildLocation`, `RebuildStatsSnapshot`, `RebuildEventPayload`, `parse_rebuilt_widgets_event` — all `pub`.
2. `crates/fdemon-core/src/timeline.rs` exists with: `TimelineThread`, `TimelinePhase`, `TimelineEvent`, `parse_vm_timeline`, internal `classify_thread` — `TimelineThread`/`TimelinePhase`/`TimelineEvent`/`parse_vm_timeline` are `pub`.
3. `crates/fdemon-core/src/lib.rs` declares both modules via `pub mod rebuild_stats;` and `pub mod timeline;`.
4. `cargo check -p fdemon-core` passes.
5. `cargo test -p fdemon-core` includes the new unit tests below — all green.
6. `cargo clippy -p fdemon-core --all-targets -- -D warnings` is clean.
7. All `pub` items have `///` doc comments.

### Testing

`crates/fdemon-core/src/rebuild_stats.rs` — `#[cfg(test)] mod tests`:

- `parse_rebuilt_widgets_event_minimal` — payload with `frameNumber`, `startTime`, `events: []`, no `locations`. Verify returns empty events, `new_locations.is_none()`.
- `parse_rebuilt_widgets_event_with_pairs` — `events: [1, 1, 2, 5]` → returns `vec![(1, 1), (2, 5)]`.
- `parse_rebuilt_widgets_event_odd_events_array_errors` — `events: [1, 1, 2]` returns `Error::protocol`.
- `parse_rebuilt_widgets_event_missing_frame_number_errors`.
- `parse_rebuilt_widgets_event_with_locations` — verify `new_locations` is `Some(HashMap{ file_uri → JSON })`.
- `merge_parallel_arrays_basic` — given `{ ids: [1,2], lines: [10,20], columns: [3,4], names: ["A","B"] }`, verify both IDs end up in the map with correct fields and `file_uri` preserved.
- `merge_parallel_arrays_length_mismatch_errors` — `ids: [1,2]`, `lines: [10]` returns `Error::protocol`.
- `merge_parallel_arrays_overwrites_existing` — same ID merged twice keeps the later value (engine never re-uses IDs but defensive merge is safer).

`crates/fdemon-core/src/timeline.rs` — `#[cfg(test)] mod tests`:

- `parse_vm_timeline_empty` — `{ "traceEvents": [] }` → empty vec.
- `parse_vm_timeline_classifies_ui_thread` — metadata `thread_name` event with `1.ui (12345)`, then a `Frame` event with `tid: 12345, ph: "X", dur: 1000`. Verify result: 1 event, `thread == Ui`, `dur == Some(1000)`.
- `parse_vm_timeline_classifies_raster_thread` — same but `io.flutter.1.raster`.
- `parse_vm_timeline_classifies_other_thread` — `io.io` → `Other`.
- `parse_vm_timeline_extracts_frame_number_from_args_string` — `args: { "frame_number": "42" }`.
- `parse_vm_timeline_extracts_frame_number_from_args_int` — `args: { "frame_number": 42 }`.
- `parse_vm_timeline_handles_flutter_frame_number_alias`.
- `parse_vm_timeline_skips_metadata_events_from_output`.
- `parse_vm_timeline_phase_mapping` — table-driven: `B → Begin`, `E → End`, `X → Complete`, `i → Instant`, `n → Other`.
- `parse_vm_timeline_malformed_event_returns_error` — missing `name` field.
- `classify_thread_macos_platform_fallback` — `io.flutter.1.platform` → `Raster`.
- `classify_thread_tester_special_case` — `io.flutter.test..ui` → `Ui` (per comment in module docstring).

### Notes

- **No `prost` / protobuf dependency** — `getVMTimeline` returns Chrome-trace JSON, not Perfetto binary. We deliberately avoid the upstream DevTools Perfetto migration to keep the daemon dep tree small.
- **`LocationMap.merge_parallel_arrays` lives in `fdemon-core`** rather than the daemon because the same shape is used by both the event payload AND the `widgetLocationIdMap` RPC response. Centralizing the merge avoids duplication in T02.
- **Frame-number extraction in `parse_vm_timeline`** must handle BOTH string and integer JSON forms — DevTools test fixtures and real VM output disagree on this. Test both.
- **`RebuildEventPayload::new_locations` is raw JSON** rather than a `Vec<(String, ParallelLocationBlock)>` — defers shape decisions to the app layer and keeps the parser minimal.
- **No aggregation logic in core** — `RebuildStatsSnapshot` is a passive data shape. The accumulator (lifetime totals + per-frame snapshots, rolling window) lives in `fdemon-app/handler/devtools/performance/rebuild_stats.rs` (T04).
- **No `is_janky` / display-rate logic in `TimelineEvent`** — frame-budget reasoning stays in `FramePhases` / `frame_hints.rs`. Timeline events are presentational only.
