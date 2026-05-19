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
use std::collections::{BTreeMap, HashMap};
use tracing::debug;

// ── TimelineThread ────────────────────────────────────────────────────────────

/// Flutter VM thread classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum TimelineThread {
    /// Flutter UI thread (Dart isolate executing build/layout/paint).
    Ui,
    /// Flutter Raster thread (formerly GPU thread — submits to graphics
    /// backend). Also covers the macOS `.platform` thread fallback.
    Raster,
    /// Anything else (Embedder, GC, Compiler, IO, etc.).
    #[default]
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

// ── TimelineNode ──────────────────────────────────────────────────────────────

/// A single event node in a per-thread tree.
///
/// Begin/End pairs are reconciled into a single node with
/// `dur = Some(end_ts - start_ts)`. Complete (`X`) events become nodes
/// directly. Instant (`i`) events become zero-duration nodes
/// (`dur = None`). Children are nested by interval containment within the
/// same `tid`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimelineNode {
    /// Human-readable event name (e.g. `"Frame"`, `"Layout"`, `"Paint"`).
    pub name: String,
    /// Event category (`cat` field), if present.
    pub category: Option<String>,
    /// Timestamp in microseconds (monotonic VM clock). Stored as `i64` to
    /// allow signed arithmetic in duration and containment checks.
    pub ts: i64,
    /// Duration in microseconds. `None` for unmatched Begin events and
    /// Instant events.
    pub dur: Option<i64>,
    /// Chrome-trace phase of the originating event.
    pub phase: TimelinePhase,
    /// Classified thread.
    pub thread: TimelineThread,
    /// Child nodes whose intervals are fully contained within this node's
    /// interval, ordered by `(ts asc, dur desc)`.
    pub children: Vec<TimelineNode>,
}

// ── TimelineTrack ─────────────────────────────────────────────────────────────

/// A per-thread track containing the root-level events for one `tid`.
///
/// Events within the track are the result of B/E pairing and nesting via
/// [`pair_be_events`].
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct TimelineTrack {
    /// Raw thread ID.
    pub tid: i64,
    /// Human-readable thread name, if available from metadata events.
    pub name: Option<String>,
    /// Classified thread role.
    pub thread: TimelineThread,
    /// Root-level event nodes (events not contained within any other event on
    /// this thread). Ordered by `(ts asc, dur desc)`.
    pub root_events: Vec<TimelineNode>,
}

// ── ThreadMetadata ────────────────────────────────────────────────────────────

/// Thread-metadata extracted from `ph: "M"` events.
///
/// Used by the handler to populate
/// `PerformanceState::timeline_thread_name_map` with human-readable names
/// like `"io.flutter.raster"`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThreadMetadata {
    /// Raw thread ID.
    pub tid: i64,
    /// Human-readable thread name from `args.name`.
    pub name: String,
}

// ── Constants ─────────────────────────────────────────────────────────────────

/// Maximum stack depth for unmatched Begin events.
///
/// Prevents OOM on malformed streams. Events beyond this depth are emitted
/// with `dur = None` rather than pushed onto the stack.
const MAX_BE_STACK_DEPTH: usize = 256;

// ── pair_be_events ────────────────────────────────────────────────────────────

/// Reconstructs duration events from a sorted slice of [`TimelineEvent`]s
/// belonging to a single `tid`.
///
/// ## Algorithm
///
/// 1. Walk events in `ts` order.
/// 2. `Begin` → push onto stack with `start_ts`.
/// 3. `End` → pop the topmost stack entry (matching name preferred; defensive
///    pop if names mismatch). Emit a [`TimelineNode`] with
///    `dur = end_ts - start_ts`.
/// 4. `Complete` → emit directly with its embedded `dur`.
/// 5. `Instant` → emit as a leaf with `dur = None`.
/// 6. Unmatched begins (still on stack at end) are emitted with `dur = None`
///    and a debug log entry.
/// 7. After flattening, nest by interval containment: a parent contains a
///    child iff `parent.ts <= child.ts && parent.ts + dur >= child.ts + child_dur`.
///    Equal-`ts` ties resolve by larger `dur` becoming the parent.
///
/// The input slice must be sorted by `ts` ascending. Callers (e.g.,
/// [`build_tracks`]) are responsible for sorting before calling this function.
pub fn pair_be_events(events: &[TimelineEvent]) -> Vec<TimelineNode> {
    // Stack entries: (name, ts, category, phase, thread) for unmatched Begin events.
    struct StackEntry {
        name: String,
        category: String,
        ts: i64,
        thread: TimelineThread,
    }

    let mut stack: Vec<StackEntry> = Vec::new();
    let mut flat: Vec<TimelineNode> = Vec::new();

    for event in events {
        let ts = event.ts as i64;
        match event.phase {
            TimelinePhase::Begin => {
                if stack.len() < MAX_BE_STACK_DEPTH {
                    stack.push(StackEntry {
                        name: event.name.clone(),
                        category: event.category.clone(),
                        ts,
                        thread: event.thread,
                    });
                } else {
                    debug!(
                        event_name = %event.name,
                        stack_depth = stack.len(),
                        "pair_be_events: stack depth limit reached; emitting Begin with dur=None"
                    );
                    flat.push(TimelineNode {
                        name: event.name.clone(),
                        category: if event.category.is_empty() {
                            None
                        } else {
                            Some(event.category.clone())
                        },
                        ts,
                        dur: None,
                        phase: TimelinePhase::Begin,
                        thread: event.thread,
                        children: vec![],
                    });
                }
            }

            TimelinePhase::End => {
                let end_ts = ts;
                if let Some(top) = stack.pop() {
                    if top.name != event.name {
                        debug!(
                            end_name = %event.name,
                            stack_top = %top.name,
                            "pair_be_events: mismatched B/E names; popping defensively"
                        );
                    }
                    let dur = end_ts - top.ts;
                    flat.push(TimelineNode {
                        name: top.name,
                        category: if top.category.is_empty() {
                            None
                        } else {
                            Some(top.category)
                        },
                        ts: top.ts,
                        dur: Some(dur),
                        phase: TimelinePhase::Begin,
                        thread: top.thread,
                        children: vec![],
                    });
                } else {
                    // Unmatched End — emit as a zero-duration leaf.
                    debug!(
                        event_name = %event.name,
                        "pair_be_events: unmatched End event; emitting with dur=None"
                    );
                    flat.push(TimelineNode {
                        name: event.name.clone(),
                        category: if event.category.is_empty() {
                            None
                        } else {
                            Some(event.category.clone())
                        },
                        ts,
                        dur: None,
                        phase: TimelinePhase::End,
                        thread: event.thread,
                        children: vec![],
                    });
                }
            }

            TimelinePhase::Complete => {
                let dur = event.dur.map(|d| d as i64);
                flat.push(TimelineNode {
                    name: event.name.clone(),
                    category: if event.category.is_empty() {
                        None
                    } else {
                        Some(event.category.clone())
                    },
                    ts,
                    dur,
                    phase: TimelinePhase::Complete,
                    thread: event.thread,
                    children: vec![],
                });
            }

            TimelinePhase::Instant => {
                flat.push(TimelineNode {
                    name: event.name.clone(),
                    category: if event.category.is_empty() {
                        None
                    } else {
                        Some(event.category.clone())
                    },
                    ts,
                    dur: None,
                    phase: TimelinePhase::Instant,
                    thread: event.thread,
                    children: vec![],
                });
            }

            TimelinePhase::Other => {
                // Skip Other-phase events; they are not useful for tree building.
            }
        }
    }

    // Drain unmatched Begin entries from the stack (outermost first after reverse).
    while let Some(entry) = stack.pop() {
        debug!(
            event_name = %entry.name,
            "pair_be_events: unmatched Begin at end of batch; emitting with dur=None"
        );
        flat.push(TimelineNode {
            name: entry.name,
            category: if entry.category.is_empty() {
                None
            } else {
                Some(entry.category)
            },
            ts: entry.ts,
            dur: None,
            phase: TimelinePhase::Begin,
            thread: entry.thread,
            children: vec![],
        });
    }

    // Sort flattened list by (ts asc, dur desc) — larger-dur events become
    // parents when they share the same start time.
    flat.sort_by(|a, b| {
        a.ts.cmp(&b.ts).then_with(|| {
            // Larger dur first (desc) — None treated as 0.
            let da = a.dur.unwrap_or(0);
            let db = b.dur.unwrap_or(0);
            db.cmp(&da)
        })
    });

    nest_by_containment(flat)
}

/// Nest a sorted (ts asc, dur desc) flat list of nodes into a tree using
/// interval containment.
///
/// A node P is the parent of node C iff:
/// - `P.ts <= C.ts`
/// - `P.ts + P.dur >= C.ts + C.dur` (where missing dur is treated as 0)
///
/// The algorithm walks the list maintaining a stack of open intervals. When
/// a new node's start is within the topmost open interval, it becomes a child.
/// When it falls outside, the stack is unwound until a containing ancestor is
/// found (or the root level is reached).
fn nest_by_containment(nodes: Vec<TimelineNode>) -> Vec<TimelineNode> {
    // Each stack frame: (node, parent_end_ts)
    // We accumulate children on a parallel children-stack to avoid
    // partial moves.
    struct Frame {
        node: TimelineNode,
        end_ts: i64, // ts + dur (where dur=0 for None)
    }

    let mut stack: Vec<Frame> = Vec::new();
    let mut roots: Vec<TimelineNode> = Vec::new();

    for node in nodes {
        let node_end = node.ts + node.dur.unwrap_or(0);

        // Pop frames that don't contain this node.
        loop {
            match stack.last() {
                None => break,
                Some(top) => {
                    // Containment: top.ts <= node.ts (already guaranteed by sort)
                    // and top.end_ts >= node_end.
                    if top.end_ts >= node_end {
                        break; // This frame contains the node — keep it.
                    }
                    // Top frame doesn't contain this node — close it.
                    let finished = stack.pop().unwrap();
                    match stack.last_mut() {
                        Some(parent) => parent.node.children.push(finished.node),
                        None => roots.push(finished.node),
                    }
                }
            }
        }

        // Push the current node onto the stack as a potential parent.
        stack.push(Frame {
            end_ts: node_end,
            node,
        });
    }

    // Close remaining open frames.
    while let Some(finished) = stack.pop() {
        match stack.last_mut() {
            Some(parent) => parent.node.children.push(finished.node),
            None => roots.push(finished.node),
        }
    }

    // Roots were pushed in order (oldest first) — reverse so oldest is first.
    // Actually they were pushed in order as nodes were consumed left-to-right,
    // which means roots[0] is the oldest. No reversal needed.
    roots
}

// ── parse_vm_timeline_with_metadata ──────────────────────────────────────────

/// Like [`parse_vm_timeline`] but also returns metadata events as a
/// [`Vec<ThreadMetadata>`].
///
/// The existing `parse_vm_timeline` continues to filter `ph:"M"` from the
/// event stream (no breaking change for current consumers). This function
/// returns both the event stream and the metadata stream for consumers that
/// need human-readable thread names.
///
/// When an event has `ph == "M"` and `name == "thread_name"`, the `args.name`
/// value is extracted as the thread label and a [`ThreadMetadata`] record is
/// appended to the returned metadata vec.
///
/// `thread_name_map` is updated in-place exactly as in [`parse_vm_timeline`].
pub fn parse_vm_timeline_with_metadata(
    response: &serde_json::Value,
    thread_name_map: &mut HashMap<i64, String>,
) -> Result<(Vec<TimelineEvent>, Vec<ThreadMetadata>)> {
    let trace_events = response
        .get("traceEvents")
        .and_then(|v| v.as_array())
        .ok_or_else(|| Error::protocol("getVMTimeline response missing 'traceEvents' array"))?;

    let mut events = Vec::new();
    let mut metadata = Vec::new();

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
                    metadata.push(ThreadMetadata {
                        tid,
                        name: thread_name.to_owned(),
                    });
                }
            }
            // Metadata events are never added to the output events vec.
            continue;
        }

        // Step 2: parse non-metadata events (identical to parse_vm_timeline).
        let name = raw
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::protocol("timeline event missing 'name' field"))?
            .to_owned();

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

        let thread_name = thread_name_map.get(&tid).map(|s| s.as_str()).unwrap_or("");
        let thread = classify_thread(thread_name);

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

    Ok((events, metadata))
}

// ── build_tracks ──────────────────────────────────────────────────────────────

/// Convenience: build full per-thread tracks from a batch of events.
///
/// Groups events by `tid`, sorts each group by `ts` ascending, then calls
/// [`pair_be_events`] per group to produce the event trees.
///
/// The returned [`BTreeMap`] uses `tid` as the key so tracks are iterated in
/// ascending `tid` order — matching the DevTools convention.
///
/// Thread names are taken from the event's `thread` field (already classified
/// by the parser) but the human-readable `name` on each [`TimelineTrack`]
/// must be filled in by the caller from a `thread_name_map` if desired, since
/// the raw name string is not stored on [`TimelineEvent`].
pub fn build_tracks(events: &[TimelineEvent]) -> BTreeMap<i64, TimelineTrack> {
    // Group events by tid.
    let mut by_tid: BTreeMap<i64, Vec<&TimelineEvent>> = BTreeMap::new();
    for event in events {
        by_tid.entry(event.tid).or_default().push(event);
    }

    let mut tracks = BTreeMap::new();
    for (tid, mut group) in by_tid {
        // Sort by ts ascending for correct B/E pairing.
        group.sort_by_key(|e| e.ts);

        // Determine thread classification from the first event (all events in
        // a group share the same tid, so they share the same classification).
        let thread = group
            .first()
            .map(|e| e.thread)
            .unwrap_or(TimelineThread::Other);

        // Collect owned copies for pair_be_events.
        let owned: Vec<TimelineEvent> = group.into_iter().cloned().collect();
        let root_events = pair_be_events(&owned);

        tracks.insert(
            tid,
            TimelineTrack {
                tid,
                name: None, // caller fills in from thread_name_map
                thread,
                root_events,
            },
        );
    }

    tracks
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

    // ── pair_be_events ────────────────────────────────────────────────────────

    fn make_be_event(name: &str, phase: TimelinePhase, ts: u64, dur: Option<u64>) -> TimelineEvent {
        TimelineEvent {
            name: name.to_owned(),
            category: "Embedder".to_owned(),
            thread: TimelineThread::Ui,
            tid: 1,
            phase,
            ts,
            dur,
            frame_number: None,
        }
    }

    /// AC1: B/E pairing happy path — nested A(100..200) contains B(150..180).
    #[test]
    fn pair_be_events_happy_path_nested() {
        let events = vec![
            make_be_event("A", TimelinePhase::Begin, 100, None),
            make_be_event("B", TimelinePhase::Begin, 150, None),
            make_be_event("B", TimelinePhase::End, 180, None),
            make_be_event("A", TimelinePhase::End, 200, None),
        ];

        let roots = pair_be_events(&events);

        assert_eq!(roots.len(), 1, "should have one root node A");
        let a = &roots[0];
        assert_eq!(a.name, "A");
        assert_eq!(a.ts, 100);
        assert_eq!(a.dur, Some(100)); // 200 - 100

        assert_eq!(a.children.len(), 1, "A should have one child B");
        let b = &a.children[0];
        assert_eq!(b.name, "B");
        assert_eq!(b.ts, 150);
        assert_eq!(b.dur, Some(30)); // 180 - 150
        assert!(b.children.is_empty());
    }

    /// AC2: Complete events pass through with existing dur.
    #[test]
    fn pair_be_events_complete_passes_through() {
        let events = vec![make_be_event(
            "Frame",
            TimelinePhase::Complete,
            1000,
            Some(50),
        )];
        let roots = pair_be_events(&events);

        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].name, "Frame");
        assert_eq!(roots[0].dur, Some(50));
        assert_eq!(roots[0].phase, TimelinePhase::Complete);
    }

    /// AC3: Instant events become zero-dur leaves (dur = None).
    #[test]
    fn pair_be_events_instant_becomes_none_dur_leaf() {
        let events = vec![make_be_event("Instant", TimelinePhase::Instant, 500, None)];
        let roots = pair_be_events(&events);

        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].name, "Instant");
        assert_eq!(roots[0].dur, None, "Instant events should have dur=None");
        assert_eq!(roots[0].phase, TimelinePhase::Instant);
    }

    /// AC4: Unmatched Begin tolerance — emits with dur=None, doesn't crash.
    #[test]
    fn pair_be_events_unmatched_begin_emits_with_dur_none() {
        let events = vec![make_be_event("Orphan", TimelinePhase::Begin, 100, None)];
        let roots = pair_be_events(&events);

        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].name, "Orphan");
        assert_eq!(roots[0].dur, None, "unmatched Begin should have dur=None");
    }

    /// AC5: Mismatched B/E names — pops defensively, doesn't crash.
    #[test]
    fn pair_be_events_mismatched_names_pops_defensively() {
        let events = vec![
            make_be_event("A", TimelinePhase::Begin, 100, None),
            make_be_event("B", TimelinePhase::End, 200, None),
        ];
        // Should not panic; should produce one node from popping the stack.
        let roots = pair_be_events(&events);
        // The Begin "A" was popped when End "B" arrived. It gets a dur based on
        // the End timestamp.
        assert!(!roots.is_empty(), "should produce at least one node");
        let node = &roots[0];
        assert_eq!(node.name, "A", "popped entry should use Begin's name");
        assert_eq!(node.dur, Some(100), "dur = end_ts - begin_ts = 200 - 100");
    }

    /// AC6: Nesting — 3-level tree: outer[100,200], middle[120,180], inner[140,160].
    #[test]
    fn pair_be_events_three_level_nesting() {
        // Use Complete events so we have explicit durations.
        let events = vec![
            make_be_event("outer", TimelinePhase::Complete, 100, Some(100)),
            make_be_event("middle", TimelinePhase::Complete, 120, Some(60)),
            make_be_event("inner", TimelinePhase::Complete, 140, Some(20)),
        ];

        let roots = pair_be_events(&events);

        assert_eq!(roots.len(), 1, "should have one root: outer");
        let outer = &roots[0];
        assert_eq!(outer.name, "outer");
        assert_eq!(outer.dur, Some(100));

        assert_eq!(outer.children.len(), 1, "outer should contain middle");
        let middle = &outer.children[0];
        assert_eq!(middle.name, "middle");
        assert_eq!(middle.dur, Some(60));

        assert_eq!(middle.children.len(), 1, "middle should contain inner");
        let inner = &middle.children[0];
        assert_eq!(inner.name, "inner");
        assert_eq!(inner.dur, Some(20));
        assert!(inner.children.is_empty());
    }

    // ── build_tracks ──────────────────────────────────────────────────────────

    fn make_tid_event(name: &str, tid: i64, ts: u64, dur: Option<u64>) -> TimelineEvent {
        TimelineEvent {
            name: name.to_owned(),
            category: "Embedder".to_owned(),
            thread: if tid == 1 {
                TimelineThread::Ui
            } else {
                TimelineThread::Raster
            },
            tid,
            phase: if dur.is_some() {
                TimelinePhase::Complete
            } else {
                TimelinePhase::Instant
            },
            ts,
            dur,
            frame_number: None,
        }
    }

    /// AC7: Per-tid isolation — events on tid=1 don't nest under tid=2.
    #[test]
    fn build_tracks_per_tid_isolation() {
        let events = vec![
            // tid=1: outer[100,300]
            make_tid_event("outer", 1, 100, Some(200)),
            // tid=2: inner[110,290] — overlaps in time but different tid
            make_tid_event("inner", 2, 110, Some(180)),
        ];

        let tracks = build_tracks(&events);

        assert_eq!(tracks.len(), 2);

        let t1 = tracks.get(&1).expect("tid=1 track missing");
        assert_eq!(t1.root_events.len(), 1);
        assert!(
            t1.root_events[0].children.is_empty(),
            "tid=1's outer should have no children from tid=2"
        );

        let t2 = tracks.get(&2).expect("tid=2 track missing");
        assert_eq!(t2.root_events.len(), 1);
        assert!(
            t2.root_events[0].children.is_empty(),
            "tid=2's inner should not be nested under tid=1 events"
        );
    }

    /// build_tracks preserves tid and thread on the track.
    #[test]
    fn build_tracks_sets_tid_and_thread() {
        let events = vec![make_tid_event("Frame", 42, 1000, Some(100))];
        let tracks = build_tracks(&events);

        let track = tracks.get(&42).expect("tid=42 track missing");
        assert_eq!(track.tid, 42);
        assert_eq!(track.thread, TimelineThread::Raster); // tid != 1 → Raster in helper
    }

    /// build_tracks sorts by ts before pairing so out-of-order events work.
    #[test]
    fn build_tracks_sorts_events_by_ts() {
        // Deliver events in reverse order.
        let events = vec![
            make_be_event("B", TimelinePhase::End, 200, None),
            make_be_event("A", TimelinePhase::Begin, 100, None),
        ];

        let tracks = build_tracks(&events);
        let track = tracks.get(&1).expect("tid=1 track missing");

        // After sort+pair, A(100) should be matched with B(200) → dur=100.
        assert_eq!(track.root_events.len(), 1);
        assert_eq!(track.root_events[0].name, "A");
        assert_eq!(track.root_events[0].dur, Some(100));
    }

    // ── parse_vm_timeline_with_metadata ───────────────────────────────────────

    /// AC8: Metadata extraction returns ThreadMetadata for ph="M" thread_name events.
    #[test]
    fn parse_vm_timeline_with_metadata_extracts_thread_metadata() {
        let mut map = HashMap::new();
        let response = json!({
            "traceEvents": [
                {
                    "ph": "M",
                    "name": "thread_name",
                    "pid": 1,
                    "tid": 45067,
                    "args": { "name": "io.flutter.raster" }
                },
                make_complete_event("Raster", 45067, 1000, 500, None)
            ]
        });

        let (events, metadata) =
            parse_vm_timeline_with_metadata(&response, &mut map).expect("should parse");

        assert_eq!(metadata.len(), 1);
        assert_eq!(metadata[0].tid, 45067);
        assert_eq!(metadata[0].name, "io.flutter.raster");

        // Events should still be classified correctly.
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].thread, TimelineThread::Raster);
    }

    /// Metadata events are excluded from the events vec.
    #[test]
    fn parse_vm_timeline_with_metadata_excludes_m_events_from_event_vec() {
        let mut map = HashMap::new();
        let response = json!({
            "traceEvents": [
                {
                    "ph": "M",
                    "name": "thread_name",
                    "pid": 1,
                    "tid": 1,
                    "args": { "name": "io.flutter.ui" }
                }
            ]
        });

        let (events, metadata) =
            parse_vm_timeline_with_metadata(&response, &mut map).expect("should parse");
        assert!(
            events.is_empty(),
            "metadata events must not appear in event vec"
        );
        assert_eq!(metadata.len(), 1);
    }

    /// Non-thread_name metadata events are ignored.
    #[test]
    fn parse_vm_timeline_with_metadata_ignores_non_thread_name_metadata() {
        let mut map = HashMap::new();
        let response = json!({
            "traceEvents": [
                {
                    "ph": "M",
                    "name": "process_name",
                    "pid": 1,
                    "tid": 1,
                    "args": { "name": "flutter" }
                }
            ]
        });

        let (events, metadata) =
            parse_vm_timeline_with_metadata(&response, &mut map).expect("should parse");
        assert!(events.is_empty());
        assert!(
            metadata.is_empty(),
            "process_name metadata should not be extracted"
        );
    }

    // ── AC9: Backward compatibility — parse_vm_timeline unchanged ────────────

    /// AC9: parse_vm_timeline signature and M-filtering are unchanged.
    #[test]
    fn parse_vm_timeline_backward_compat_still_filters_metadata() {
        let mut map = HashMap::new();
        let response = json!({
            "traceEvents": [
                {
                    "ph": "M",
                    "name": "thread_name",
                    "pid": 1,
                    "tid": 1,
                    "args": { "name": "io.flutter.ui" }
                },
                make_complete_event("Frame", 1, 1000, 100, None)
            ]
        });

        let events = parse_vm_timeline(&response, &mut map).expect("should parse");
        assert_eq!(
            events.len(),
            1,
            "M events must still be filtered from output"
        );
        assert_eq!(events[0].name, "Frame");
    }

    // ── AC10: Serde round-trip ────────────────────────────────────────────────

    #[test]
    fn track_serde_round_trip() {
        let track = TimelineTrack {
            tid: 42,
            name: Some("io.flutter.ui".to_owned()),
            thread: TimelineThread::Ui,
            root_events: vec![TimelineNode {
                name: "Frame".to_owned(),
                category: Some("Embedder".to_owned()),
                ts: 1000,
                dur: Some(8000),
                phase: TimelinePhase::Complete,
                thread: TimelineThread::Ui,
                children: vec![TimelineNode {
                    name: "Layout".to_owned(),
                    category: None,
                    ts: 1200,
                    dur: Some(3000),
                    phase: TimelinePhase::Begin,
                    thread: TimelineThread::Ui,
                    children: vec![],
                }],
            }],
        };

        let json = serde_json::to_string(&track).expect("serialize failed");
        let restored: TimelineTrack = serde_json::from_str(&json).expect("deserialize failed");

        assert_eq!(restored, track);
    }

    #[test]
    fn node_serde_round_trip() {
        let node = TimelineNode {
            name: "Test".to_owned(),
            category: None,
            ts: 500,
            dur: None,
            phase: TimelinePhase::Instant,
            thread: TimelineThread::Other,
            children: vec![],
        };

        let json = serde_json::to_string(&node).expect("serialize failed");
        let restored: TimelineNode = serde_json::from_str(&json).expect("deserialize failed");
        assert_eq!(restored, node);
    }
}
