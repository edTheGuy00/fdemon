//! # Rebuild Stats
//!
//! Domain types for the Flutter widget-rebuild profiler. Wraps the
//! `Flutter.RebuiltWidgets` Extension stream and the
//! `ext.flutter.inspector.widgetLocationIdMap` RPC.
//!
//! ## Data flow
//!
//! 1. The VM Service emits `Flutter.RebuiltWidgets` extension events on the
//!    `Extension` stream. Each event's `extensionData` has the shape parsed
//!    by [`parse_rebuilt_widgets_event`].
//! 2. New location IDs are shipped inline (see `locations` block). Callers in
//!    `fdemon-app` hold a persistent [`LocationMap`] and call
//!    [`LocationMap::merge_parallel_arrays`] for each new file URI.
//! 3. After merging, the caller resolves the flat `events` pairs into a
//!    [`RebuildStatsSnapshot`] (one [`RebuildLocation`] per ID).

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── Location ──────────────────────────────────────────────────────────────────

/// A single widget location entry (file URI + line:column + class name).
///
/// Assigned by the Flutter engine and stable for the lifetime of the process.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Location {
    /// `package:` URI of the source file (e.g. `package:foo/main.dart`).
    pub file_uri: String,
    /// 1-based line number within `file_uri`.
    pub line: u32,
    /// 1-based column number within `file_uri`.
    pub column: u32,
    /// Widget class name at this location (e.g. `"MaterialApp"`).
    pub name: String,
}

// ── LocationMap ───────────────────────────────────────────────────────────────

/// Map from numeric location ID (assigned by the engine) to a [`Location`].
///
/// Built incrementally from the `locations` sub-object that ships inside
/// `Flutter.RebuiltWidgets` events and from one-shot
/// `ext.flutter.inspector.widgetLocationIdMap` responses.
///
/// IDs are stable per engine run. Merging an already-known ID overwrites it,
/// which is safe because the engine never reassigns IDs.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocationMap {
    /// The underlying id → location store.
    pub by_id: HashMap<u32, Location>,
}

impl LocationMap {
    /// Merge a parallel-arrays location block into this map.
    ///
    /// The `value` argument must be a JSON object with four equal-length arrays:
    /// `ids`, `lines`, `columns`, and `names`. This is the shape used by both the
    /// `Flutter.RebuiltWidgets` event payload and the `widgetLocationIdMap` RPC.
    ///
    /// `file_uri` is stored verbatim on each produced [`Location`].
    ///
    /// Returns [`Error::protocol`] on shape mismatch (wrong JSON shape or unequal
    /// array lengths).
    pub fn merge_parallel_arrays(
        &mut self,
        file_uri: &str,
        value: &serde_json::Value,
    ) -> Result<()> {
        let ids = value
            .get("ids")
            .and_then(|v| v.as_array())
            .ok_or_else(|| Error::protocol("location block missing 'ids' array"))?;
        let lines = value
            .get("lines")
            .and_then(|v| v.as_array())
            .ok_or_else(|| Error::protocol("location block missing 'lines' array"))?;
        let columns = value
            .get("columns")
            .and_then(|v| v.as_array())
            .ok_or_else(|| Error::protocol("location block missing 'columns' array"))?;
        let names = value
            .get("names")
            .and_then(|v| v.as_array())
            .ok_or_else(|| Error::protocol("location block missing 'names' array"))?;

        let len = ids.len();
        if lines.len() != len || columns.len() != len || names.len() != len {
            return Err(Error::protocol(format!(
                "location block array length mismatch: ids={}, lines={}, columns={}, names={}",
                len,
                lines.len(),
                columns.len(),
                names.len()
            )));
        }

        for i in 0..len {
            let id = ids[i]
                .as_u64()
                .ok_or_else(|| {
                    Error::protocol(format!("location id at index {i} is not a valid u64"))
                })
                .map(|v| v as u32)?;

            let line = lines[i]
                .as_u64()
                .ok_or_else(|| {
                    Error::protocol(format!("location line at index {i} is not a valid u64"))
                })
                .map(|v| v as u32)?;

            let column = columns[i]
                .as_u64()
                .ok_or_else(|| {
                    Error::protocol(format!("location column at index {i} is not a valid u64"))
                })
                .map(|v| v as u32)?;

            let name = names[i]
                .as_str()
                .ok_or_else(|| {
                    Error::protocol(format!("location name at index {i} is not a string"))
                })?
                .to_owned();

            self.by_id.insert(
                id,
                Location {
                    file_uri: file_uri.to_owned(),
                    line,
                    column,
                    name,
                },
            );
        }

        Ok(())
    }
}

// ── RebuildLocation ───────────────────────────────────────────────────────────

/// One row of a per-frame rebuild snapshot: a resolved location plus the
/// number of times it rebuilt during that frame.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RebuildLocation {
    /// Resolved widget location.
    pub location: Location,
    /// Number of times this widget rebuilt in the frame.
    pub build_count: u32,
}

// ── RebuildStatsSnapshot ──────────────────────────────────────────────────────

/// All widget rebuilds observed during a single frame.
///
/// Produced by the app layer after resolving the flat `events` pairs in a
/// [`RebuildEventPayload`] against a [`LocationMap`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RebuildStatsSnapshot {
    /// Monotonically increasing frame counter from the engine.
    /// Matches [`fdemon_core::performance::FrameTiming::number`] when correlated.
    pub frame_number: u64,
    /// Frame start time in microseconds (VM monotonic clock).
    pub start_time_micros: u64,
    /// Per-widget rebuild counts for this frame.
    pub rebuilds: Vec<RebuildLocation>,
}

// ── RebuildEventPayload ───────────────────────────────────────────────────────

/// Parsed `Flutter.RebuiltWidgets` event payload — pure transport shape.
///
/// Aggregation and location resolution live in the app layer (`fdemon-app`).
/// Callers should:
/// 1. Merge `new_locations` into their persistent [`LocationMap`] first.
/// 2. Resolve `events` pairs via the now-updated map.
/// 3. Produce a [`RebuildStatsSnapshot`].
#[derive(Debug, Clone)]
pub struct RebuildEventPayload {
    /// Monotonically increasing frame counter from the engine.
    pub frame_number: u64,
    /// Frame start time in microseconds (VM monotonic clock).
    pub start_time_micros: u64,
    /// Flat `[id, count]` pairs decoded from the event's `events` array.
    pub events: Vec<(u32, u32)>,
    /// Optional new locations introduced in this event.
    ///
    /// `None` when the event omits the `locations` key (all referenced IDs
    /// are already known). When `Some`, each entry maps a file URI to the
    /// raw JSON object containing `ids`, `lines`, `columns`, and `names`
    /// arrays. Callers should pass each entry to
    /// [`LocationMap::merge_parallel_arrays`].
    pub new_locations: Option<HashMap<String, serde_json::Value>>,
}

// ── parse_rebuilt_widgets_event ───────────────────────────────────────────────

/// Parse a `Flutter.RebuiltWidgets` event's `extensionData` JSON object into
/// a structured [`RebuildEventPayload`].
///
/// Returns [`Error::protocol`] on malformed input (missing required fields,
/// wrong types, or an odd-length `events` array).
pub fn parse_rebuilt_widgets_event(
    extension_data: &serde_json::Value,
) -> Result<RebuildEventPayload> {
    let frame_number = extension_data
        .get("frameNumber")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| {
            Error::protocol("Flutter.RebuiltWidgets event missing 'frameNumber' field")
        })?;

    let start_time_micros = extension_data
        .get("startTime")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| {
            Error::protocol("Flutter.RebuiltWidgets event missing 'startTime' field")
        })?;

    let raw_events = extension_data
        .get("events")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            Error::protocol("Flutter.RebuiltWidgets event missing 'events' array")
        })?;

    if raw_events.len() % 2 != 0 {
        return Err(Error::protocol(format!(
            "Flutter.RebuiltWidgets 'events' array has odd length ({}); expected [id, count, ...] pairs",
            raw_events.len()
        )));
    }

    let mut events = Vec::with_capacity(raw_events.len() / 2);
    let mut i = 0;
    while i < raw_events.len() {
        let id = raw_events[i]
            .as_u64()
            .ok_or_else(|| {
                Error::protocol(format!(
                    "Flutter.RebuiltWidgets events[{i}] is not a valid u64 (expected location id)"
                ))
            })
            .map(|v| v as u32)?;

        let count = raw_events[i + 1]
            .as_u64()
            .ok_or_else(|| {
                Error::protocol(format!(
                    "Flutter.RebuiltWidgets events[{}] is not a valid u64 (expected build count)",
                    i + 1
                ))
            })
            .map(|v| v as u32)?;

        events.push((id, count));
        i += 2;
    }

    // `locations` is optional — only present when new IDs are introduced.
    let new_locations = extension_data.get("locations").and_then(|v| v.as_object()).map(
        |obj| {
            obj.iter()
                .map(|(file_uri, block)| (file_uri.clone(), block.clone()))
                .collect::<HashMap<_, _>>()
        },
    );

    Ok(RebuildEventPayload {
        frame_number,
        start_time_micros,
        events,
        new_locations,
    })
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ── parse_rebuilt_widgets_event ───────────────────────────────────────────

    #[test]
    fn parse_rebuilt_widgets_event_minimal() {
        let payload = json!({
            "frameNumber": 1,
            "startTime": 12345,
            "events": []
        });
        let result = parse_rebuilt_widgets_event(&payload).expect("should parse");
        assert_eq!(result.frame_number, 1);
        assert_eq!(result.start_time_micros, 12345);
        assert!(result.events.is_empty());
        assert!(result.new_locations.is_none());
    }

    #[test]
    fn parse_rebuilt_widgets_event_with_pairs() {
        let payload = json!({
            "frameNumber": 10,
            "startTime": 0,
            "events": [1, 1, 2, 5]
        });
        let result = parse_rebuilt_widgets_event(&payload).expect("should parse");
        assert_eq!(result.events, vec![(1, 1), (2, 5)]);
    }

    #[test]
    fn parse_rebuilt_widgets_event_odd_events_array_errors() {
        let payload = json!({
            "frameNumber": 1,
            "startTime": 0,
            "events": [1, 1, 2]
        });
        let err = parse_rebuilt_widgets_event(&payload).expect_err("should fail");
        assert!(
            matches!(err, crate::error::Error::Protocol { .. }),
            "expected Protocol error, got: {err:?}"
        );
    }

    #[test]
    fn parse_rebuilt_widgets_event_missing_frame_number_errors() {
        let payload = json!({
            "startTime": 0,
            "events": []
        });
        let err = parse_rebuilt_widgets_event(&payload).expect_err("should fail");
        assert!(
            matches!(err, crate::error::Error::Protocol { .. }),
            "expected Protocol error, got: {err:?}"
        );
    }

    #[test]
    fn parse_rebuilt_widgets_event_with_locations() {
        let payload = json!({
            "frameNumber": 57,
            "startTime": 2352949,
            "events": [1, 1, 2, 1],
            "locations": {
                "package:foo/main.dart": {
                    "ids": [1, 2],
                    "lines": [23, 32],
                    "columns": [10, 12],
                    "names": ["PlanetsApp", "MaterialApp"]
                }
            }
        });
        let result = parse_rebuilt_widgets_event(&payload).expect("should parse");
        let locs = result.new_locations.expect("locations should be Some");
        assert!(locs.contains_key("package:foo/main.dart"));
    }

    // ── LocationMap::merge_parallel_arrays ────────────────────────────────────

    #[test]
    fn merge_parallel_arrays_basic() {
        let mut map = LocationMap::default();
        let block = json!({
            "ids": [1, 2],
            "lines": [10, 20],
            "columns": [3, 4],
            "names": ["A", "B"]
        });
        map.merge_parallel_arrays("package:foo/main.dart", &block)
            .expect("should merge");

        let loc1 = map.by_id.get(&1).expect("id 1 should be present");
        assert_eq!(loc1.file_uri, "package:foo/main.dart");
        assert_eq!(loc1.line, 10);
        assert_eq!(loc1.column, 3);
        assert_eq!(loc1.name, "A");

        let loc2 = map.by_id.get(&2).expect("id 2 should be present");
        assert_eq!(loc2.file_uri, "package:foo/main.dart");
        assert_eq!(loc2.line, 20);
        assert_eq!(loc2.column, 4);
        assert_eq!(loc2.name, "B");
    }

    #[test]
    fn merge_parallel_arrays_length_mismatch_errors() {
        let mut map = LocationMap::default();
        let block = json!({
            "ids": [1, 2],
            "lines": [10],
            "columns": [3, 4],
            "names": ["A", "B"]
        });
        let err = map
            .merge_parallel_arrays("package:foo/main.dart", &block)
            .expect_err("should fail on length mismatch");
        assert!(
            matches!(err, crate::error::Error::Protocol { .. }),
            "expected Protocol error, got: {err:?}"
        );
    }

    #[test]
    fn merge_parallel_arrays_overwrites_existing() {
        let mut map = LocationMap::default();
        let block_v1 = json!({
            "ids": [1],
            "lines": [10],
            "columns": [3],
            "names": ["OldName"]
        });
        map.merge_parallel_arrays("package:foo/main.dart", &block_v1)
            .expect("first merge should succeed");

        let block_v2 = json!({
            "ids": [1],
            "lines": [99],
            "columns": [7],
            "names": ["NewName"]
        });
        map.merge_parallel_arrays("package:foo/main.dart", &block_v2)
            .expect("second merge should succeed");

        let loc = map.by_id.get(&1).expect("id 1 should be present");
        assert_eq!(loc.name, "NewName");
        assert_eq!(loc.line, 99);
        assert_eq!(loc.column, 7);
    }
}
