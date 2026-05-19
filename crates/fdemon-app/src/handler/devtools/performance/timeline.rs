//! Timeline Events tab handlers — Phase 3.
//!
//! Handles the 1-Hz timeline polling event pipeline:
//! - [`handle_batch`] — append new events to the ring buffer and truncate.
//! - [`handle_cycle_filter`] — cycle the `TimelineFilter` and reset scroll.

use crate::handler::UpdateResult;
use crate::session::SessionId;
use crate::state::AppState;
use fdemon_core::timeline::TimelineEvent;

// ── handle_batch ──────────────────────────────────────────────────────────────

/// Handle a batch of timeline events from the 1-Hz poll.
///
/// Appends all events to `PerformanceState::timeline_events` and truncates
/// from the front to stay within `settings.devtools.timeline_event_buffer_size`.
pub(crate) fn handle_batch(
    state: &mut AppState,
    session_id: SessionId,
    events: Vec<TimelineEvent>,
) -> UpdateResult {
    let buffer_cap = state.settings.devtools.timeline_event_buffer_size;

    let Some(handle) = state.session_manager.get_mut(session_id) else {
        return UpdateResult::none();
    };

    let timeline = &mut handle.session.performance.timeline_events;
    for event in events {
        timeline.push_back(event);
    }

    // Truncate from the front to keep within the buffer cap.
    let cap = buffer_cap.max(1); // never allow 0-size buffer
    while timeline.len() > cap {
        timeline.pop_front();
    }

    UpdateResult::none()
}

// ── handle_cycle_filter ───────────────────────────────────────────────────────

/// Handle a `TimelineEventsCycleFilter` message.
///
/// Cycles the filter: `All → Ui → Raster → All`, then resets the scroll
/// offset to the top so the user sees the most relevant events immediately.
pub(crate) fn handle_cycle_filter(state: &mut AppState, session_id: SessionId) -> UpdateResult {
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        let current = handle.session.performance.timeline_events_filter;
        handle.session.performance.timeline_events_filter = current.next();
        handle.session.performance.timeline_events_scroll_offset = 0;
    }
    UpdateResult::none()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::update::update;
    use crate::message::Message;
    use crate::session::performance::TimelineFilter;
    use crate::state::AppState;
    use fdemon_core::timeline::{TimelineEvent, TimelinePhase, TimelineThread};

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

    fn make_state_with_session() -> (AppState, SessionId) {
        let mut state = AppState::new();
        let id = state
            .session_manager
            .create_session(&test_device())
            .unwrap();
        (state, id)
    }

    fn make_event(name: &str, ts: u64, thread: TimelineThread) -> TimelineEvent {
        TimelineEvent {
            name: name.to_string(),
            category: "Embedder".to_string(),
            thread,
            tid: 1,
            phase: TimelinePhase::Complete,
            ts,
            dur: Some(100),
            frame_number: None,
        }
    }

    // ── handle_batch ─────────────────────────────────────────────────────────

    #[test]
    fn handle_batch_appends_events() {
        let (mut state, session_id) = make_state_with_session();

        let events = vec![
            make_event("Frame", 1000, TimelineThread::Ui),
            make_event("Raster", 2000, TimelineThread::Raster),
        ];
        update(
            &mut state,
            Message::TimelineEventsBatchReceived { session_id, events },
        );

        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        assert_eq!(perf.timeline_events.len(), 2);
        assert_eq!(perf.timeline_events[0].name, "Frame");
        assert_eq!(perf.timeline_events[1].name, "Raster");
    }

    #[test]
    fn handle_batch_truncates_at_buffer_cap() {
        let (mut state, session_id) = make_state_with_session();
        state.settings.devtools.timeline_event_buffer_size = 3;

        // Send 5 events across two batches.
        let batch1 = vec![
            make_event("A", 1, TimelineThread::Ui),
            make_event("B", 2, TimelineThread::Ui),
            make_event("C", 3, TimelineThread::Ui),
        ];
        let batch2 = vec![
            make_event("D", 4, TimelineThread::Ui),
            make_event("E", 5, TimelineThread::Ui),
        ];

        update(
            &mut state,
            Message::TimelineEventsBatchReceived {
                session_id,
                events: batch1,
            },
        );
        update(
            &mut state,
            Message::TimelineEventsBatchReceived {
                session_id,
                events: batch2,
            },
        );

        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        // Buffer capped at 3 — should retain the most recent 3 events.
        assert_eq!(perf.timeline_events.len(), 3);
        assert_eq!(perf.timeline_events[0].name, "C");
        assert_eq!(perf.timeline_events[1].name, "D");
        assert_eq!(perf.timeline_events[2].name, "E");
    }

    #[test]
    fn handle_batch_empty_vec_is_noop() {
        let (mut state, session_id) = make_state_with_session();

        update(
            &mut state,
            Message::TimelineEventsBatchReceived {
                session_id,
                events: vec![],
            },
        );

        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        assert!(perf.timeline_events.is_empty());
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
    fn handle_cycle_filter_resets_scroll_offset() {
        let (mut state, session_id) = make_state_with_session();

        // Set a non-zero scroll offset.
        if let Some(h) = state.session_manager.get_mut(session_id) {
            h.session.performance.timeline_events_scroll_offset = 10;
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
            .timeline_events_scroll_offset;
        assert_eq!(offset, 0);
    }
}
