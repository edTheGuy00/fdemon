//! Timeline Events tab handlers — Phase 4 + Phase 5.
//!
//! Phase 4 pipeline:
//! - [`handle_batch`] — build per-thread event trees from the incoming batch,
//!   merge into existing tracks, update thread-name map, enforce buffer cap,
//!   and update the persistent `frame_anchor_map`.
//! - [`handle_cycle_filter`] — cycle the `TimelineFilter` and reset scroll.
//!
//! Phase 5 pan/zoom:
//! - [`handle_zoom_in`] / [`handle_zoom_out`] — halve/double the viewport width.
//! - [`handle_pan_left`] / [`handle_pan_right`] — pan by 10% of viewport width.
//! - [`handle_follow_latest`] — reset to live-edge/frame-anchored follow mode.

use std::collections::btree_map::Entry;
use std::collections::BTreeMap;

use crate::handler::UpdateResult;
use crate::session::performance::FRAME_ANCHOR_MAP_CAP;
use crate::session::SessionId;
use crate::state::AppState;
use fdemon_core::timeline::{ThreadMetadata, TimelineEvent, TimelineNode, TimelineTrack};

// ── Phase 5: Viewport constants (mirrors TUI crate's viewport.rs) ─────────────
//
// These are defined separately from the TUI crate to respect layer boundaries
// (fdemon-app must not depend on fdemon-tui). The values must remain in sync.

/// Default timeline viewport width (5 s) — same as `TIMELINE_VIEWPORT_MICROS`
/// in the TUI crate.
const DEFAULT_VIEWPORT_MICROS: u64 = 5_000_000;

/// Minimum viewport width (100 ms) — prevents over-zoom.
const TIMELINE_VIEWPORT_MIN_MICROS: u64 = 100_000;

/// Maximum viewport width (60 s) — prevents over-zoom out.
const TIMELINE_VIEWPORT_MAX_MICROS: u64 = 60_000_000;

/// Zoom factor per `+`/`-` keypress (2× = 4 keypresses span 100 ms → 60 s).
const TIMELINE_ZOOM_FACTOR: f64 = 2.0;

/// Pan fraction per `←`/`→` keypress (10% of viewport width per keypress).
const TIMELINE_PAN_FRACTION: f64 = 0.10;

// ── handle_batch ──────────────────────────────────────────────────────────────

/// Handle a batch of timeline events from the 1-Hz poll.
///
/// 1. Inserts thread-name metadata into `timeline_thread_name_map`.
/// 2. Builds per-thread event trees from the batch via `build_tracks`.
/// 3. Merges new tracks into the existing `timeline_tracks` map.
/// 4. Enforces the buffer cap by dropping oldest root events globally.
pub(crate) fn handle_batch(
    state: &mut AppState,
    session_id: SessionId,
    events: Vec<TimelineEvent>,
    metadata: Vec<ThreadMetadata>,
) -> UpdateResult {
    let buffer_cap = state.settings.devtools.timeline_event_buffer_size;

    let Some(handle) = state.session_manager.get_mut(session_id) else {
        return UpdateResult::none();
    };

    // 1. Update thread name map from metadata.
    for ThreadMetadata { tid, name } in &metadata {
        handle
            .session
            .performance
            .timeline_thread_name_map
            .insert(*tid, name.clone());
    }

    if events.is_empty() {
        return UpdateResult::none();
    }

    // 2. Build incremental tracks from this batch.
    let new_tracks = fdemon_core::timeline::build_tracks(&events);

    // 2b. Scan new_tracks for root events with frame_number and update the
    //     persistent frame_anchor_map before merging (avoids re-scanning the
    //     entire accumulated buffer).
    let anchor_map = &mut handle.session.performance.frame_anchor_map;
    for new_track in new_tracks.values() {
        for node in &new_track.root_events {
            if let Some(n) = node.frame_number {
                let ts = node.ts as u64;
                let end = (node.ts + node.dur.unwrap_or(0)) as u64;
                match anchor_map.entry(n) {
                    Entry::Occupied(mut e) => {
                        let (s, ee) = e.get_mut();
                        *s = (*s).min(ts);
                        *ee = (*ee).max(end);
                    }
                    Entry::Vacant(e) => {
                        e.insert((ts, end));
                    }
                }
            }
        }
    }
    // Cap the anchor map: evict oldest frame numbers (smallest keys) first.
    while anchor_map.len() > FRAME_ANCHOR_MAP_CAP {
        anchor_map.pop_first();
    }

    // 3. Merge into existing tracks (append root_events, update thread names).
    let tracks = &mut handle.session.performance.timeline_tracks;
    let names = &handle.session.performance.timeline_thread_name_map;
    for (tid, new_track) in new_tracks {
        let entry = tracks.entry(tid).or_insert_with(|| TimelineTrack {
            tid,
            name: names.get(&tid).cloned(),
            thread: new_track.thread,
            root_events: Vec::new(),
        });
        // Refresh thread name if metadata arrived later (e.g. first batch has
        // no metadata but a subsequent one does).
        if entry.name.is_none() {
            entry.name = names.get(&tid).cloned();
        }
        entry.root_events.extend(new_track.root_events);
    }

    // 4. Enforce buffer cap.
    enforce_track_buffer_cap(tracks, buffer_cap);

    UpdateResult::none()
}

// ── enforce_track_buffer_cap ──────────────────────────────────────────────────

/// Drops the oldest root events globally (across all tracks) until total node
/// count (including children) is at most `cap`.
///
/// Eviction strategy: find the track whose first root event has the smallest
/// `ts` (oldest globally) and pop it. Repeat until under cap. Preserves
/// children of all surviving root events — we never trim mid-subtree.
///
/// This matches the task specification: "drop the oldest events globally by
/// `ts`; trim each track's `root_events` from the front while preserving
/// children inside surviving roots."
fn enforce_track_buffer_cap(tracks: &mut BTreeMap<i64, TimelineTrack>, cap: usize) {
    fn count_nodes(node: &TimelineNode) -> usize {
        1 + node.children.iter().map(count_nodes).sum::<usize>()
    }

    fn total(tracks: &BTreeMap<i64, TimelineTrack>) -> usize {
        tracks
            .values()
            .flat_map(|t| t.root_events.iter())
            .map(count_nodes)
            .sum()
    }

    while total(tracks) > cap {
        // Find the track with the oldest first root event.
        let oldest_tid = tracks
            .iter()
            .filter(|(_, t)| !t.root_events.is_empty())
            .min_by_key(|(_, t)| t.root_events[0].ts)
            .map(|(tid, _)| *tid);
        match oldest_tid {
            Some(tid) => {
                tracks.get_mut(&tid).unwrap().root_events.remove(0);
            }
            None => break,
        }
    }
}

// ── handle_cycle_filter ───────────────────────────────────────────────────────

/// Handle a `TimelineEventsCycleFilter` message.
///
/// Cycles the filter: `All → Ui → Raster → All`, then resets the thread-row
/// scroll offset to the top so the user sees the most relevant threads first.
pub(crate) fn handle_cycle_filter(
    state: &mut AppState,
    session_id: crate::session::SessionId,
) -> UpdateResult {
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        let current = handle.session.performance.timeline_events_filter;
        handle.session.performance.timeline_events_filter = current.next();
        handle.session.performance.timeline_thread_scroll_offset = 0;
    }
    UpdateResult::none()
}

// ── Phase 5: Pan/zoom viewport handlers ──────────────────────────────────────

/// Zoom in: halve the viewport width, centered on the current midpoint.
///
/// Sets `timeline_follow_latest = false` (manual-viewport mode).
/// Width is clamped at [`TIMELINE_VIEWPORT_MIN_MICROS`].
pub(crate) fn handle_zoom_in(state: &mut AppState, session_id: SessionId) -> UpdateResult {
    let Some(handle) = state.session_manager.get_mut(session_id) else {
        return UpdateResult::none();
    };
    let perf = &mut handle.session.performance;

    // Materialize the current viewport before mutating.
    let (cur_start, cur_end) = materialize_viewport(perf);
    let cur_width = cur_end.saturating_sub(cur_start);
    let anchor = (cur_start + cur_end) / 2;

    let (new_start, _new_end) =
        zoom_viewport(cur_start, cur_width, 1.0 / TIMELINE_ZOOM_FACTOR, anchor);
    let new_width =
        (cur_width / 2).clamp(TIMELINE_VIEWPORT_MIN_MICROS, TIMELINE_VIEWPORT_MAX_MICROS);

    perf.timeline_viewport_start_micros = new_start;
    perf.timeline_viewport_width_micros = new_width;
    perf.timeline_follow_latest = false;
    UpdateResult::none()
}

/// Zoom out: double the viewport width, centered on the current midpoint.
///
/// Sets `timeline_follow_latest = false` (manual-viewport mode).
/// Width is clamped at [`TIMELINE_VIEWPORT_MAX_MICROS`].
pub(crate) fn handle_zoom_out(state: &mut AppState, session_id: SessionId) -> UpdateResult {
    let Some(handle) = state.session_manager.get_mut(session_id) else {
        return UpdateResult::none();
    };
    let perf = &mut handle.session.performance;

    let (cur_start, cur_end) = materialize_viewport(perf);
    let cur_width = cur_end.saturating_sub(cur_start);
    let anchor = (cur_start + cur_end) / 2;

    let (new_start, _new_end) = zoom_viewport(cur_start, cur_width, TIMELINE_ZOOM_FACTOR, anchor);
    let new_width = cur_width
        .saturating_mul(2)
        .clamp(TIMELINE_VIEWPORT_MIN_MICROS, TIMELINE_VIEWPORT_MAX_MICROS);

    perf.timeline_viewport_start_micros = new_start;
    perf.timeline_viewport_width_micros = new_width;
    perf.timeline_follow_latest = false;
    UpdateResult::none()
}

/// Pan left: decrease `viewport_start_micros` by 10% of current width.
///
/// Sets `timeline_follow_latest = false`. Start saturates at 0.
pub(crate) fn handle_pan_left(state: &mut AppState, session_id: SessionId) -> UpdateResult {
    let Some(handle) = state.session_manager.get_mut(session_id) else {
        return UpdateResult::none();
    };
    let perf = &mut handle.session.performance;

    let (cur_start, cur_end) = materialize_viewport(perf);
    let cur_width = cur_end.saturating_sub(cur_start);
    let delta = (cur_width as f64 * TIMELINE_PAN_FRACTION).round() as u64;

    perf.timeline_viewport_start_micros = cur_start.saturating_sub(delta);
    perf.timeline_viewport_width_micros = cur_width;
    perf.timeline_follow_latest = false;
    UpdateResult::none()
}

/// Pan right: increase `viewport_start_micros` by 10% of current width.
///
/// Sets `timeline_follow_latest = false`.
pub(crate) fn handle_pan_right(state: &mut AppState, session_id: SessionId) -> UpdateResult {
    let Some(handle) = state.session_manager.get_mut(session_id) else {
        return UpdateResult::none();
    };
    let perf = &mut handle.session.performance;

    let (cur_start, cur_end) = materialize_viewport(perf);
    let cur_width = cur_end.saturating_sub(cur_start);
    let delta = (cur_width as f64 * TIMELINE_PAN_FRACTION).round() as u64;

    perf.timeline_viewport_start_micros = cur_start.saturating_add(delta);
    perf.timeline_viewport_width_micros = cur_width;
    perf.timeline_follow_latest = false;
    UpdateResult::none()
}

/// Resume follow-latest mode.
///
/// Sets `timeline_follow_latest = true` and resets the viewport width to the
/// default 5 s. The `committed_frame_anchor` is preserved so the next render
/// returns to the frame-anchored viewport (PLAN D2 mode 2) if one was set.
pub(crate) fn handle_follow_latest(state: &mut AppState, session_id: SessionId) -> UpdateResult {
    let Some(handle) = state.session_manager.get_mut(session_id) else {
        return UpdateResult::none();
    };
    let perf = &mut handle.session.performance;
    perf.timeline_follow_latest = true;
    perf.timeline_viewport_width_micros = DEFAULT_VIEWPORT_MICROS;
    // timeline_viewport_start_micros becomes irrelevant in follow_latest mode;
    // reset it to 0 for cleanliness.
    perf.timeline_viewport_start_micros = 0;
    UpdateResult::none()
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Materialize the current effective viewport `(start, end)` from state.
///
/// This is a simplified version of `compute_active_viewport` from the TUI crate,
/// inlined here to avoid cross-crate dependency. It resolves the same 3-mode
/// priority (PLAN D2) but without the frame-anchor rendering math (which lives
/// in the TUI crate).
///
/// Mode 1: manual (`!follow_latest`) → `(start, start + width)`.
/// Mode 2/3: follow-latest → use stored start/width as best approximation
///           (the TUI render will recompute from frame anchor / live edge).
fn materialize_viewport(perf: &crate::session::performance::PerformanceState) -> (u64, u64) {
    let start = perf.timeline_viewport_start_micros;
    let width = perf
        .timeline_viewport_width_micros
        .clamp(TIMELINE_VIEWPORT_MIN_MICROS, TIMELINE_VIEWPORT_MAX_MICROS);
    (start, start.saturating_add(width))
}

/// Pure zoom computation (mirrors viewport.rs `zoom_viewport`).
fn zoom_viewport(start: u64, width: u64, factor: f64, anchor_micros: u64) -> (u64, u64) {
    let new_width_f = width as f64 * factor;
    let new_width = (new_width_f.round() as u64).max(1);
    let anchor_fraction = if width == 0 {
        0.5
    } else {
        let offset = anchor_micros.saturating_sub(start);
        (offset as f64 / width as f64).clamp(0.0, 1.0)
    };
    let anchor_new_offset = (anchor_fraction * new_width as f64).round() as u64;
    let new_start = anchor_micros.saturating_sub(anchor_new_offset);
    let new_end = new_start.saturating_add(new_width);
    (new_start, new_end)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use crate::handler::devtools::handle_switch_panel;
    use crate::handler::update::update;
    use crate::message::Message;
    use crate::session::performance::{TimelineFilter, FRAME_ANCHOR_MAP_CAP};
    use crate::state::{AppState, DevToolsPanel};
    use fdemon_core::timeline::{ThreadMetadata, TimelineEvent, TimelinePhase, TimelineThread};

    fn test_device() -> fdemon_daemon::Device {
        fdemon_daemon::Device {
            id: "test-device".to_string(),
            name: "Test Device".to_string(),
            platform: "android".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        }
    }

    fn make_state_with_session() -> (AppState, crate::session::SessionId) {
        let mut state = AppState::new();
        let id = state
            .session_manager
            .create_session(&test_device())
            .unwrap();
        (state, id)
    }

    fn make_complete_event(name: &str, tid: i64, ts: u64, thread: TimelineThread) -> TimelineEvent {
        TimelineEvent {
            name: name.to_string(),
            category: "Embedder".to_string(),
            thread,
            tid,
            phase: TimelinePhase::Complete,
            ts,
            dur: Some(100),
            frame_number: None,
        }
    }

    fn make_frame_event(
        name: &str,
        tid: i64,
        ts: u64,
        dur: u64,
        frame_number: u64,
        thread: TimelineThread,
    ) -> TimelineEvent {
        TimelineEvent {
            name: name.to_string(),
            category: "Embedder".to_string(),
            thread,
            tid,
            phase: TimelinePhase::Complete,
            ts,
            dur: Some(dur),
            frame_number: Some(frame_number),
        }
    }

    // ── handle_batch: basic append ────────────────────────────────────────────

    #[test]
    fn handle_batch_builds_tracks_from_events() {
        let (mut state, session_id) = make_state_with_session();

        let events = vec![
            make_complete_event("Frame", 1, 1000, TimelineThread::Ui),
            make_complete_event("Raster", 2, 2000, TimelineThread::Raster),
        ];
        update(
            &mut state,
            Message::TimelineEventsBatchReceived {
                session_id,
                events,
                metadata: vec![],
            },
        );

        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        assert_eq!(
            perf.timeline_tracks.len(),
            2,
            "two tids should produce two tracks"
        );
        assert_eq!(
            perf.timeline_tracks.get(&1).unwrap().root_events.len(),
            1,
            "tid=1 should have one root event"
        );
        assert_eq!(
            perf.timeline_tracks.get(&1).unwrap().root_events[0].name,
            "Frame"
        );
        assert_eq!(
            perf.timeline_tracks.get(&2).unwrap().root_events.len(),
            1,
            "tid=2 should have one root event"
        );
        assert_eq!(
            perf.timeline_tracks.get(&2).unwrap().root_events[0].name,
            "Raster"
        );
    }

    #[test]
    fn handle_batch_empty_events_is_noop() {
        let (mut state, session_id) = make_state_with_session();

        update(
            &mut state,
            Message::TimelineEventsBatchReceived {
                session_id,
                events: vec![],
                metadata: vec![],
            },
        );

        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        assert!(perf.timeline_tracks.is_empty());
    }

    // ── handle_batch: merging across batches ──────────────────────────────────

    #[test]
    fn handle_batch_merges_across_batches() {
        let (mut state, session_id) = make_state_with_session();

        // First batch: 1 event on tid=1.
        update(
            &mut state,
            Message::TimelineEventsBatchReceived {
                session_id,
                events: vec![make_complete_event("A", 1, 100, TimelineThread::Ui)],
                metadata: vec![],
            },
        );
        // Second batch: another event on tid=1.
        update(
            &mut state,
            Message::TimelineEventsBatchReceived {
                session_id,
                events: vec![make_complete_event("B", 1, 200, TimelineThread::Ui)],
                metadata: vec![],
            },
        );

        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        let track = perf.timeline_tracks.get(&1).unwrap();
        assert_eq!(
            track.root_events.len(),
            2,
            "two events across two batches on tid=1"
        );
        assert_eq!(track.root_events[0].name, "A");
        assert_eq!(track.root_events[1].name, "B");
    }

    // ── enforce_track_buffer_cap_drops_oldest (AC4) ───────────────────────────

    #[test]
    fn enforce_track_buffer_cap_drops_oldest() {
        let (mut state, session_id) = make_state_with_session();
        // Set buffer cap to 5.
        state.settings.devtools.timeline_event_buffer_size = 5;

        // Send 10 events on tid=1 with timestamps 1..=10.
        let events: Vec<TimelineEvent> = (1u64..=10)
            .map(|ts| make_complete_event("E", 1, ts, TimelineThread::Ui))
            .collect();
        update(
            &mut state,
            Message::TimelineEventsBatchReceived {
                session_id,
                events,
                metadata: vec![],
            },
        );

        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        let track = perf.timeline_tracks.get(&1).unwrap();
        assert_eq!(
            track.root_events.len(),
            5,
            "buffer cap 5 should retain only 5 root events on tid=1"
        );
        // The 5 most recent (ts=6..=10) should survive.
        assert_eq!(
            track.root_events[0].ts, 6,
            "oldest surviving event should have ts=6"
        );
        assert_eq!(
            track.root_events[4].ts, 10,
            "most recent event should have ts=10"
        );
    }

    // ── metadata_populates_thread_name_map (AC5) ──────────────────────────────

    #[test]
    fn metadata_populates_thread_name_map() {
        let (mut state, session_id) = make_state_with_session();

        update(
            &mut state,
            Message::TimelineEventsBatchReceived {
                session_id,
                events: vec![],
                metadata: vec![ThreadMetadata {
                    tid: 45067,
                    name: "io.flutter.raster".to_string(),
                }],
            },
        );

        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        assert_eq!(
            perf.timeline_thread_name_map
                .get(&45067)
                .map(|s| s.as_str()),
            Some("io.flutter.raster"),
            "metadata should populate timeline_thread_name_map"
        );
    }

    // ── handle_cycle_filter ───────────────────────────────────────────────────

    #[test]
    fn handle_cycle_filter_cycles_all_ui_raster_all() {
        let (mut state, session_id) = make_state_with_session();

        // Default is All.
        assert_eq!(
            state
                .session_manager
                .get(session_id)
                .unwrap()
                .session
                .performance
                .timeline_events_filter,
            TimelineFilter::All
        );

        update(
            &mut state,
            Message::TimelineEventsCycleFilter { session_id },
        );
        assert_eq!(
            state
                .session_manager
                .get(session_id)
                .unwrap()
                .session
                .performance
                .timeline_events_filter,
            TimelineFilter::Ui
        );

        update(
            &mut state,
            Message::TimelineEventsCycleFilter { session_id },
        );
        assert_eq!(
            state
                .session_manager
                .get(session_id)
                .unwrap()
                .session
                .performance
                .timeline_events_filter,
            TimelineFilter::Raster
        );

        update(
            &mut state,
            Message::TimelineEventsCycleFilter { session_id },
        );
        assert_eq!(
            state
                .session_manager
                .get(session_id)
                .unwrap()
                .session
                .performance
                .timeline_events_filter,
            TimelineFilter::All
        );
    }

    #[test]
    fn handle_cycle_filter_resets_thread_scroll_offset() {
        let (mut state, session_id) = make_state_with_session();

        // Set a non-zero thread scroll offset.
        if let Some(h) = state.session_manager.get_mut(session_id) {
            h.session.performance.timeline_thread_scroll_offset = 10;
        }

        update(
            &mut state,
            Message::TimelineEventsCycleFilter { session_id },
        );

        let offset = state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance
            .timeline_thread_scroll_offset;
        assert_eq!(offset, 0);
    }

    // ── frame_anchor_map: population ─────────────────────────────────────────

    /// Task AC: A batch with a Complete event carrying frame_number must populate
    /// `frame_anchor_map` with the correct `(ts, ts+dur)` range.
    #[test]
    fn handle_batch_populates_frame_anchor_map_for_events_with_frame_number() {
        let (mut state, session_id) = make_state_with_session();

        // Frame event: frame_number=7, ts=1_000_000, dur=16_000
        let events = vec![make_frame_event(
            "Frame",
            1,
            1_000_000,
            16_000,
            7,
            TimelineThread::Ui,
        )];
        update(
            &mut state,
            Message::TimelineEventsBatchReceived {
                session_id,
                events,
                metadata: vec![],
            },
        );

        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        assert!(
            perf.frame_anchor_map.contains_key(&7),
            "frame_anchor_map should have an entry for frame 7"
        );
        let &(ts_start, ts_end) = perf.frame_anchor_map.get(&7).unwrap();
        assert_eq!(ts_start, 1_000_000, "ts_start should equal event ts");
        assert_eq!(ts_end, 1_016_000, "ts_end should equal event ts + dur");
    }

    /// Task AC: Two batches for the same frame_number with different ranges must
    /// produce a map entry whose range is the union (min ts_start, max ts_end).
    #[test]
    fn handle_batch_extends_existing_frame_anchor_range() {
        let (mut state, session_id) = make_state_with_session();

        // First batch: ts=1_000_000, dur=8_000 → range [1_000_000, 1_008_000]
        update(
            &mut state,
            Message::TimelineEventsBatchReceived {
                session_id,
                events: vec![make_frame_event(
                    "Ui",
                    1,
                    1_000_000,
                    8_000,
                    42,
                    TimelineThread::Ui,
                )],
                metadata: vec![],
            },
        );
        // Second batch: ts=999_000, dur=20_000 → range [999_000, 1_019_000]
        // After union: [min(1_000_000, 999_000), max(1_008_000, 1_019_000)] = [999_000, 1_019_000]
        update(
            &mut state,
            Message::TimelineEventsBatchReceived {
                session_id,
                events: vec![make_frame_event(
                    "Raster",
                    2,
                    999_000,
                    20_000,
                    42,
                    TimelineThread::Raster,
                )],
                metadata: vec![],
            },
        );

        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        let &(ts_start, ts_end) = perf
            .frame_anchor_map
            .get(&42)
            .expect("frame 42 must exist in map");
        assert_eq!(ts_start, 999_000, "ts_start should be the minimum seen");
        assert_eq!(ts_end, 1_019_000, "ts_end should be the maximum seen");
    }

    /// Task AC: After inserting FRAME_ANCHOR_MAP_CAP + 5 distinct frames, the map
    /// must remain at most CAP entries and the oldest (smallest) frame numbers must
    /// have been evicted.
    #[test]
    fn frame_anchor_map_is_capped_at_max() {
        let (mut state, session_id) = make_state_with_session();

        // Send CAP + 5 distinct frame numbers in a single large batch.
        let total = FRAME_ANCHOR_MAP_CAP + 5;
        let events: Vec<TimelineEvent> = (0u64..total as u64)
            .map(|i| make_frame_event("Frame", 1, i * 1_000, 500, i, TimelineThread::Ui))
            .collect();
        update(
            &mut state,
            Message::TimelineEventsBatchReceived {
                session_id,
                events,
                metadata: vec![],
            },
        );

        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        assert!(
            perf.frame_anchor_map.len() <= FRAME_ANCHOR_MAP_CAP,
            "frame_anchor_map must not exceed FRAME_ANCHOR_MAP_CAP={FRAME_ANCHOR_MAP_CAP}, \
             got {}",
            perf.frame_anchor_map.len()
        );
        // Oldest frames (0..5) should have been evicted; newest (5..total) survive.
        for i in 0..5u64 {
            assert!(
                !perf.frame_anchor_map.contains_key(&i),
                "oldest frame {i} should have been evicted from the map"
            );
        }
        assert!(
            perf.frame_anchor_map.contains_key(&(total as u64 - 1)),
            "most recent frame should still be in the map"
        );
    }

    /// Task AC: Leaving the Performance panel (via handle_switch_panel) must clear
    /// `frame_anchor_map`.
    #[test]
    fn frame_anchor_map_resets_on_performance_leave() {
        let (mut state, session_id) = make_state_with_session();

        // Populate the map with a frame event.
        update(
            &mut state,
            Message::TimelineEventsBatchReceived {
                session_id,
                events: vec![make_frame_event(
                    "Frame",
                    1,
                    1_000_000,
                    16_000,
                    5,
                    TimelineThread::Ui,
                )],
                metadata: vec![],
            },
        );

        // Switch to Performance to simulate being on that panel.
        // (We need to set the active_panel to Performance first so the leave-logic fires.)
        state.devtools_view_state.active_panel = DevToolsPanel::Performance;
        assert!(
            !state
                .session_manager
                .get(session_id)
                .unwrap()
                .session
                .performance
                .frame_anchor_map
                .is_empty(),
            "frame_anchor_map should be non-empty before leaving"
        );

        // Leave Performance — switch to Inspector triggers the clear.
        handle_switch_panel(&mut state, DevToolsPanel::Inspector);

        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        assert!(
            perf.frame_anchor_map.is_empty(),
            "frame_anchor_map must be cleared when leaving the Performance panel"
        );
    }

    // ── Phase 5: handle_zoom_in ───────────────────────────────────────────────

    /// AC3: TimelineZoomIn halves the viewport width and sets follow_latest=false.
    #[test]
    fn test_zoom_in_halves_viewport() {
        let (mut state, session_id) = make_state_with_session();
        // Set up: width=2_000_000 (2s), start=0, follow_latest=true
        if let Some(h) = state.session_manager.get_mut(session_id) {
            h.session.performance.timeline_viewport_width_micros = 2_000_000;
            h.session.performance.timeline_viewport_start_micros = 0;
            h.session.performance.timeline_follow_latest = true;
        }
        update(&mut state, Message::TimelineZoomIn { session_id });

        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        assert!(
            !perf.timeline_follow_latest,
            "zoom-in should set follow_latest=false"
        );
        assert_eq!(
            perf.timeline_viewport_width_micros, 1_000_000,
            "zoom-in should halve the 2s viewport to 1s"
        );
    }

    /// AC3: Zooming in when already at MIN does not go below MIN.
    #[test]
    fn test_zoom_in_clamps_at_min() {
        let (mut state, session_id) = make_state_with_session();
        if let Some(h) = state.session_manager.get_mut(session_id) {
            h.session.performance.timeline_viewport_width_micros = 100_000; // at MIN
            h.session.performance.timeline_follow_latest = true;
        }
        update(&mut state, Message::TimelineZoomIn { session_id });

        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        assert_eq!(
            perf.timeline_viewport_width_micros, 100_000,
            "zooming in at MIN should stay at MIN"
        );
    }

    // ── Phase 5: handle_zoom_out ──────────────────────────────────────────────

    /// AC4: TimelineZoomOut doubles the viewport width and sets follow_latest=false.
    #[test]
    fn test_zoom_out_doubles_viewport() {
        let (mut state, session_id) = make_state_with_session();
        if let Some(h) = state.session_manager.get_mut(session_id) {
            h.session.performance.timeline_viewport_width_micros = 2_000_000;
            h.session.performance.timeline_viewport_start_micros = 0;
            h.session.performance.timeline_follow_latest = true;
        }
        update(&mut state, Message::TimelineZoomOut { session_id });

        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        assert!(
            !perf.timeline_follow_latest,
            "zoom-out should set follow_latest=false"
        );
        assert_eq!(
            perf.timeline_viewport_width_micros, 4_000_000,
            "zoom-out should double the 2s viewport to 4s"
        );
    }

    /// AC4: Zooming out when already at MAX does not exceed MAX.
    #[test]
    fn test_zoom_out_doubles_viewport_to_max() {
        let (mut state, session_id) = make_state_with_session();
        if let Some(h) = state.session_manager.get_mut(session_id) {
            h.session.performance.timeline_viewport_width_micros = 60_000_000; // at MAX
            h.session.performance.timeline_follow_latest = true;
        }
        update(&mut state, Message::TimelineZoomOut { session_id });

        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        assert_eq!(
            perf.timeline_viewport_width_micros, 60_000_000,
            "zooming out at MAX should stay at MAX"
        );
    }

    // ── Phase 5: handle_pan_left / handle_pan_right ───────────────────────────

    /// AC5: TimelinePanLeft decreases start by 10% of width.
    #[test]
    fn test_pan_left_decreases_start() {
        let (mut state, session_id) = make_state_with_session();
        if let Some(h) = state.session_manager.get_mut(session_id) {
            h.session.performance.timeline_viewport_start_micros = 5_000_000;
            h.session.performance.timeline_viewport_width_micros = 5_000_000;
            h.session.performance.timeline_follow_latest = true;
        }
        update(&mut state, Message::TimelinePanLeft { session_id });

        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        assert!(
            !perf.timeline_follow_latest,
            "pan should set follow_latest=false"
        );
        // delta = 5_000_000 * 0.10 = 500_000
        assert_eq!(
            perf.timeline_viewport_start_micros, 4_500_000,
            "pan-left should decrease start by 10% of width"
        );
    }

    /// AC5: TimelinePanRight increases start by 10% of width.
    #[test]
    fn test_pan_right_increases_start() {
        let (mut state, session_id) = make_state_with_session();
        if let Some(h) = state.session_manager.get_mut(session_id) {
            h.session.performance.timeline_viewport_start_micros = 5_000_000;
            h.session.performance.timeline_viewport_width_micros = 5_000_000;
            h.session.performance.timeline_follow_latest = true;
        }
        update(&mut state, Message::TimelinePanRight { session_id });

        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        assert!(
            !perf.timeline_follow_latest,
            "pan should set follow_latest=false"
        );
        assert_eq!(
            perf.timeline_viewport_start_micros, 5_500_000,
            "pan-right should increase start by 10% of width"
        );
    }

    /// AC5: TimelinePanLeft saturates at 0.
    #[test]
    fn test_pan_left_saturates_at_zero() {
        let (mut state, session_id) = make_state_with_session();
        if let Some(h) = state.session_manager.get_mut(session_id) {
            h.session.performance.timeline_viewport_start_micros = 100; // less than delta
            h.session.performance.timeline_viewport_width_micros = 5_000_000;
            h.session.performance.timeline_follow_latest = true;
        }
        update(&mut state, Message::TimelinePanLeft { session_id });

        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        assert_eq!(
            perf.timeline_viewport_start_micros, 0,
            "pan-left should saturate at 0"
        );
    }

    // ── Phase 5: handle_follow_latest ────────────────────────────────────────

    /// AC6: TimelineFollowLatest sets follow_latest=true and resets width to default.
    #[test]
    fn test_follow_latest_resets_to_live_edge() {
        let (mut state, session_id) = make_state_with_session();
        if let Some(h) = state.session_manager.get_mut(session_id) {
            h.session.performance.timeline_follow_latest = false;
            h.session.performance.timeline_viewport_width_micros = 1_000_000;
            h.session.performance.timeline_viewport_start_micros = 9_000_000;
        }
        update(&mut state, Message::TimelineFollowLatest { session_id });

        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        assert!(
            perf.timeline_follow_latest,
            "follow-latest should set follow_latest=true"
        );
        assert_eq!(
            perf.timeline_viewport_width_micros, 5_000_000,
            "follow-latest should reset width to default 5s"
        );
    }

    /// AC6: TimelineFollowLatest preserves committed_frame_anchor.
    #[test]
    fn test_follow_latest_preserves_frame_anchor() {
        let (mut state, session_id) = make_state_with_session();
        if let Some(h) = state.session_manager.get_mut(session_id) {
            h.session.performance.committed_frame_anchor = Some(42);
            h.session.performance.timeline_follow_latest = false;
        }
        update(&mut state, Message::TimelineFollowLatest { session_id });

        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        assert_eq!(
            perf.committed_frame_anchor,
            Some(42),
            "follow-latest should preserve committed_frame_anchor"
        );
    }
}
