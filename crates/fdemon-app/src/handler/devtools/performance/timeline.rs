//! Timeline Events tab handlers — Phase 4.
//!
//! Handles the 1-Hz timeline polling event pipeline:
//! - [`handle_batch`] — build per-thread event trees from the incoming batch,
//!   merge into existing tracks, update thread-name map, enforce buffer cap.
//! - [`handle_cycle_filter`] — cycle the `TimelineFilter` and reset scroll.

use std::collections::BTreeMap;

use crate::handler::UpdateResult;
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
    use crate::handler::update::update;
    use crate::message::Message;
    use crate::session::performance::TimelineFilter;
    use crate::state::AppState;
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
}
