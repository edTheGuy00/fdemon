//! # Timeline Events
//!
//! Domain types for the Dart VM timeline. Wraps the `getVMTimeline` RPC
//! response (Chrome-trace JSON shape, not Perfetto protobuf).
//!
//! ## Thread classification
//!
//! Thread identity is determined from metadata events (`ph: "M"`,
//! `name: "thread_name"`) using the thread-name string, **not** the raw `tid`.
//! This is stable across Dart VM versions.
//!
//! Classification rules (applied in order, simple substring containment):
//! - Name contains `.ui` → [`TimelineThread::Ui`]
//! - Name contains `.raster` or `.platform` → [`TimelineThread::Raster`]
//! - Otherwise → [`TimelineThread::Other`]
//!
//! Note: the Flutter test-runner UI thread (`io.flutter.test..ui`) is
//! intentionally classified as [`TimelineThread::Ui`] because its name contains
//! `.ui`. This is by design — fdemon treats the single-track tester as a UI
//! thread, which is the correct behavior for displaying tester frame timings.
//! (Upstream DevTools uses an exclusion guard for this case; fdemon does not
//! need one because the containment check already produces the correct result.)

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::debug;

// ── TimelineThread ────────────────────────────────────────────────────────────

/// Flutter VM thread classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TimelineThread {
    /// Flutter UI thread (Dart isolate executing build/layout/paint).
    Ui,
    /// Flutter Raster thread (formerly GPU thread — submits to graphics
    /// backend). Also covers the macOS `.platform` thread fallback.
    Raster,
    /// Anything else (Embedder, GC, Compiler, IO, etc.).
    Other,
}

// ── TimelinePhase ─────────────────────────────────────────────────────────────

/// Chrome-trace event phase (`ph` field).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimelinePhase {
    /// Begin (`ph: "B"`) — duration unknown until a matching End event.
    Begin,
    /// End (`ph: "E"`).
    End,
    /// Complete (`ph: "X"`) — duration is embedded in the event itself.
    Complete,
    /// Instant (`ph: "i"`).
    Instant,
    /// Any other phase (metadata, async, flow, counter, …). These events are
    /// parsed but excluded from [`parse_vm_timeline`]'s output.
    Other,
}

// ── TimelineEvent ─────────────────────────────────────────────────────────────

/// A single Chrome-trace timeline event from the Dart VM.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimelineEvent {
    /// Human-readable event name (e.g. `"Frame"`, `"Layout"`, `"Paint"`).
    pub name: String,
    /// Event category (`cat` field, e.g. `"Embedder"`, `"Dart"`).
    pub category: String,
    /// Classified thread based on thread-name metadata.
    pub thread: TimelineThread,
    /// Raw `tid` from the event — needed for thread-name lookup.
    pub tid: i64,
    /// Chrome-trace phase.
    pub phase: TimelinePhase,
    /// Timestamp in microseconds (monotonic VM clock).
    pub ts: u64,
    /// Duration in microseconds. Only set for [`TimelinePhase::Complete`]
    /// events (`ph: "X"`).
    pub dur: Option<u64>,
    /// Frame number if the event's `args` contain `frame_number` or
    /// `flutterFrameNumber`. Used for per-frame correlation with
    /// [`RebuildStatsSnapshot`](crate::rebuild_stats::RebuildStatsSnapshot).
    pub frame_number: Option<u64>,
}

// ── parse_vm_timeline ─────────────────────────────────────────────────────────

/// Parse the JSON-encoded VM Service `Timeline` object (response of
/// `getVMTimeline`) into a [`Vec<TimelineEvent>`].
///
/// `thread_name_map` maps `tid` → human thread name. It is populated on the
/// fly from metadata events (`ph: "M"`, `name: "thread_name"`) within the
/// same response. Pass a shared mutable map across successive calls so that
/// thread-name associations from earlier responses are available for later ones.
///
/// Metadata events (`ph: "M"`) are NOT included in the returned vec.
///
/// ## Field tolerance
///
/// - `name` and `ts` are **required** — their absence causes an [`Error::protocol`]
///   return so the caller can surface a parse failure.
/// - `ph` and `tid` are **tolerated as absent** (defensive — the Chrome-trace
///   spec technically allows unknown/missing fields). A missing `ph` defaults to
///   `"?"` (parsed as [`TimelinePhase::Other`]); a missing `tid` defaults to `0`
///   (the event is classified as [`TimelineThread::Other`] unless a thread-name
///   mapping exists for tid 0). Both cases emit a `tracing::debug!` log so
///   callers can diagnose unexpected data without treating it as a hard error.
///
/// Returns [`Error::protocol`] if the top-level `traceEvents` field is missing
/// or if any non-metadata event is missing required fields (`name`, `ts`).
pub fn parse_vm_timeline(
    response: &serde_json::Value,
    thread_name_map: &mut HashMap<i64, String>,
) -> Result<Vec<TimelineEvent>> {
    let trace_events = response
        .get("traceEvents")
        .and_then(|v| v.as_array())
        .ok_or_else(|| Error::protocol("getVMTimeline response missing 'traceEvents' array"))?;

    let mut events = Vec::new();

    for raw in trace_events {
        let ph_opt = raw.get("ph").and_then(|v| v.as_str());
        let ph = ph_opt.unwrap_or("?");

        let tid_opt = raw.get("tid").and_then(|v| v.as_i64());
        let tid = tid_opt.unwrap_or(0);

        // Step 1: collect metadata events for thread-name resolution.
        if ph == "M" {
            if raw.get("name").and_then(|v| v.as_str()) == Some("thread_name") {
                if let Some(thread_name) = raw
                    .get("args")
                    .and_then(|a| a.get("name"))
                    .and_then(|v| v.as_str())
                {
                    thread_name_map.insert(tid, thread_name.to_owned());
                }
            }
            // Metadata events are never added to the output vec.
            continue;
        }

        // Step 2: parse non-metadata events.
        let name = raw
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::protocol("timeline event missing 'name' field"))?
            .to_owned();

        // Log when optional fields were absent and their defaults were used.
        if ph_opt.is_none() {
            debug!(
                event_name = %name,
                "timeline event missing 'ph' field; defaulting to '?' (TimelinePhase::Other)"
            );
        }
        if tid_opt.is_none() {
            debug!(
                event_name = %name,
                "timeline event missing 'tid' field; defaulting to 0 (thread classification uses tid 0 lookup)"
            );
        }

        let ts = raw.get("ts").and_then(|v| v.as_u64()).ok_or_else(|| {
            Error::protocol(format!(
                "timeline event '{name}' missing or invalid 'ts' field"
            ))
        })?;

        let category = raw
            .get("cat")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_owned();

        let phase = parse_phase(ph);

        let dur = if phase == TimelinePhase::Complete {
            raw.get("dur").and_then(|v| v.as_u64())
        } else {
            None
        };

        // Classify thread via name lookup (accumulated from metadata events).
        let thread_name = thread_name_map.get(&tid).map(|s| s.as_str()).unwrap_or("");
        let thread = classify_thread(thread_name);

        // Frame-number extraction: try both key names, handle string and integer forms.
        let frame_number = extract_frame_number(raw);

        events.push(TimelineEvent {
            name,
            category,
            thread,
            tid,
            phase,
            ts,
            dur,
            frame_number,
        });
    }

    Ok(events)
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Classify a thread name string into a [`TimelineThread`] variant.
///
/// Uses simple substring containment — no exclusion guards. Rules applied in
/// order:
/// 1. Contains `.ui` → [`TimelineThread::Ui`].
///    This includes `io.flutter.test..ui` (the Flutter test-runner UI thread),
///    which is intentionally classified as `Ui`. fdemon does not replicate the
///    upstream DevTools exclusion guard because the containment check already
///    produces the correct result for our use case.
/// 2. Contains `.raster` or `.platform` → [`TimelineThread::Raster`].
///    `.platform` is the macOS fallback name for the raster thread.
/// 3. Otherwise → [`TimelineThread::Other`].
fn classify_thread(thread_name: &str) -> TimelineThread {
    if thread_name.contains(".ui") {
        TimelineThread::Ui
    } else if thread_name.contains(".raster") || thread_name.contains(".platform") {
        // `.raster` is the standard Flutter raster thread;
        // `.platform` is the macOS fallback (same role).
        TimelineThread::Raster
    } else {
        TimelineThread::Other
    }
}

/// Parse a Chrome-trace `ph` string into a [`TimelinePhase`].
fn parse_phase(ph: &str) -> TimelinePhase {
    match ph {
        "B" => TimelinePhase::Begin,
        "E" => TimelinePhase::End,
        "X" => TimelinePhase::Complete,
        "i" | "I" => TimelinePhase::Instant,
        _ => TimelinePhase::Other,
    }
}

/// Extract `frame_number` from a timeline event's `args` sub-object.
///
/// Handles:
/// - `args.frame_number` as a string or integer.
/// - `args.flutterFrameNumber` as a string or integer (alias used in some
///   DevTools test fixtures).
fn extract_frame_number(raw: &serde_json::Value) -> Option<u64> {
    let args = raw.get("args")?;

    for key in ["frame_number", "flutterFrameNumber"] {
        if let Some(val) = args.get(key) {
            // Try integer form first, then string form.
            if let Some(n) = val.as_u64() {
                return Some(n);
            }
            if let Some(s) = val.as_str() {
                if let Ok(n) = s.parse::<u64>() {
                    return Some(n);
                }
            }
        }
    }

    None
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_thread_metadata(tid: i64, thread_name: &str) -> serde_json::Value {
        json!({
            "ph": "M",
            "name": "thread_name",
            "pid": 1,
            "tid": tid,
            "args": { "name": thread_name }
        })
    }

    fn make_complete_event(
        name: &str,
        tid: i64,
        ts: u64,
        dur: u64,
        args: Option<serde_json::Value>,
    ) -> serde_json::Value {
        let mut v = json!({
            "ph": "X",
            "name": name,
            "cat": "Embedder",
            "pid": 1,
            "tid": tid,
            "ts": ts,
            "dur": dur
        });
        if let Some(a) = args {
            v["args"] = a;
        }
        v
    }

    // ── parse_vm_timeline ─────────────────────────────────────────────────────

    #[test]
    fn parse_vm_timeline_empty() {
        let mut map = HashMap::new();
        let response = json!({ "traceEvents": [] });
        let events = parse_vm_timeline(&response, &mut map).expect("should parse");
        assert!(events.is_empty());
    }

    #[test]
    fn parse_vm_timeline_classifies_ui_thread() {
        let mut map = HashMap::new();
        let response = json!({
            "traceEvents": [
                make_thread_metadata(12345, "io.flutter.1.ui (12345)"),
                make_complete_event("Frame", 12345, 1000, 1000, None)
            ]
        });
        let events = parse_vm_timeline(&response, &mut map).expect("should parse");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].thread, TimelineThread::Ui);
        assert_eq!(events[0].dur, Some(1000));
    }

    #[test]
    fn parse_vm_timeline_classifies_raster_thread() {
        let mut map = HashMap::new();
        let response = json!({
            "traceEvents": [
                make_thread_metadata(99, "io.flutter.1.raster (99)"),
                make_complete_event("Raster", 99, 2000, 500, None)
            ]
        });
        let events = parse_vm_timeline(&response, &mut map).expect("should parse");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].thread, TimelineThread::Raster);
    }

    #[test]
    fn parse_vm_timeline_classifies_other_thread() {
        let mut map = HashMap::new();
        let response = json!({
            "traceEvents": [
                make_thread_metadata(77, "io.io"),
                make_complete_event("IO", 77, 3000, 100, None)
            ]
        });
        let events = parse_vm_timeline(&response, &mut map).expect("should parse");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].thread, TimelineThread::Other);
    }

    #[test]
    fn parse_vm_timeline_extracts_frame_number_from_args_string() {
        let mut map = HashMap::new();
        let response = json!({
            "traceEvents": [
                make_complete_event("Frame", 1, 1000, 100, Some(json!({ "frame_number": "42" })))
            ]
        });
        let events = parse_vm_timeline(&response, &mut map).expect("should parse");
        assert_eq!(events[0].frame_number, Some(42));
    }

    #[test]
    fn parse_vm_timeline_extracts_frame_number_from_args_int() {
        let mut map = HashMap::new();
        let response = json!({
            "traceEvents": [
                make_complete_event("Frame", 1, 1000, 100, Some(json!({ "frame_number": 42 })))
            ]
        });
        let events = parse_vm_timeline(&response, &mut map).expect("should parse");
        assert_eq!(events[0].frame_number, Some(42));
    }

    #[test]
    fn parse_vm_timeline_handles_flutter_frame_number_alias() {
        let mut map = HashMap::new();
        let response = json!({
            "traceEvents": [
                make_complete_event("Frame", 1, 1000, 100, Some(json!({ "flutterFrameNumber": "7" })))
            ]
        });
        let events = parse_vm_timeline(&response, &mut map).expect("should parse");
        assert_eq!(events[0].frame_number, Some(7));
    }

    #[test]
    fn parse_vm_timeline_skips_metadata_events_from_output() {
        let mut map = HashMap::new();
        let response = json!({
            "traceEvents": [
                make_thread_metadata(1, "io.flutter.1.ui (1)"),
                make_thread_metadata(2, "io.flutter.1.raster (2)"),
                make_complete_event("Frame", 1, 1000, 100, None)
            ]
        });
        let events = parse_vm_timeline(&response, &mut map).expect("should parse");
        // Only the non-metadata event should be in the output.
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].name, "Frame");
    }

    #[test]
    fn parse_vm_timeline_phase_mapping() {
        let mut map = HashMap::new();
        let phases: &[(&str, TimelinePhase)] = &[
            ("B", TimelinePhase::Begin),
            ("E", TimelinePhase::End),
            ("X", TimelinePhase::Complete),
            ("i", TimelinePhase::Instant),
            ("n", TimelinePhase::Other),
        ];

        for (ph_str, expected_phase) in phases {
            let event = json!({
                "ph": ph_str,
                "name": "TestEvent",
                "cat": "Test",
                "pid": 1,
                "tid": 1,
                "ts": 1000
            });
            let response = json!({ "traceEvents": [event] });
            let events = parse_vm_timeline(&response, &mut map).expect("should parse");
            assert_eq!(
                events[0].phase, *expected_phase,
                "ph='{}' should map to {:?}",
                ph_str, expected_phase
            );
        }
    }

    #[test]
    fn parse_vm_timeline_malformed_event_returns_error() {
        let mut map = HashMap::new();
        // Event is missing the 'name' field.
        let response = json!({
            "traceEvents": [
                {
                    "ph": "X",
                    "cat": "Embedder",
                    "pid": 1,
                    "tid": 1,
                    "ts": 1000,
                    "dur": 100
                }
            ]
        });
        let err = parse_vm_timeline(&response, &mut map).expect_err("should fail");
        assert!(
            matches!(err, crate::error::Error::Protocol { .. }),
            "expected Protocol error, got: {err:?}"
        );
    }

    // ── classify_thread ───────────────────────────────────────────────────────

    #[test]
    fn classify_thread_macos_platform_fallback() {
        assert_eq!(
            classify_thread("io.flutter.1.platform"),
            TimelineThread::Raster
        );
    }

    #[test]
    fn classify_thread_tester_special_case() {
        // The Flutter test runner's UI thread name contains ".ui" and should
        // classify as Ui, not be excluded.
        assert_eq!(classify_thread("io.flutter.test..ui"), TimelineThread::Ui);
    }
}
