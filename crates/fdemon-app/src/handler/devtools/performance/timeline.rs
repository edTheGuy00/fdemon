//! Timeline Events tab handlers — Phase 4.
//!
//! Handles the 1-Hz timeline polling event pipeline:
//! - [`handle_batch`] — build per-thread event trees from the incoming batch,
//!   merge into existing tracks, update thread-name map, enforce buffer cap,
//!   and update the persistent `frame_anchor_map`.
//! - [`handle_cycle_filter`] — cycle the `TimelineFilter` and reset scroll.

use std::collections::btree_map::Entry;
use std::collections::BTreeMap;

use crate::handler::UpdateResult;
use crate::session::performance::FRAME_ANCHOR_MAP_CAP;
use crate::session::SessionId;
use crate::state::AppState;
use fdemon_core::timeline::{ThreadMetadata, TimelineEvent, TimelineNode, TimelineTrack};

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
}
