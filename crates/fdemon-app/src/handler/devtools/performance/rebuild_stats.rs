//! Rebuild Stats tab handlers — Phase 3.
//!
//! Handles the `Flutter.RebuiltWidgets` event pipeline:
//! - [`handle_event`] — accumulate per-frame snapshots from incoming events.
//! - [`handle_toggle`] — toggle `ext.flutter.profileWidgetBuilds` via an async RPC.
//! - [`handle_extension_state_changed`] — update state when the toggle completes.
//! - [`handle_location_map_fetched`] — merge a one-shot location map fallback.
//! - [`handle_toggle_failed`] — surface RPC toggle failure to the session log buffer.

use crate::handler::{UpdateAction, UpdateResult};
use crate::session::SessionId;
use crate::state::{AppState, PerfDetailsTab};
use fdemon_core::rebuild_stats::{
    LocationMap, RebuildEventPayload, RebuildLocation, RebuildStatsSnapshot,
};

// ── handle_event ──────────────────────────────────────────────────────────────

/// Handle a parsed `Flutter.RebuiltWidgets` extension event.
///
/// Merges any new location data, builds a [`RebuildStatsSnapshot`], appends it
/// to the ring buffer (evicting the oldest snapshot when over the window), and
/// updates the lifetime totals.
pub(crate) fn handle_event(
    state: &mut AppState,
    session_id: SessionId,
    payload: RebuildEventPayload,
) -> UpdateResult {
    let frame_window = state.settings.devtools.rebuild_stats_frame_window as usize;

    let Some(handle) = state.session_manager.get_mut(session_id) else {
        return UpdateResult::none();
    };
    let perf = &mut handle.session.performance;

    // 1. Merge any new location data shipped inline with this event.
    if let Some(new_locations) = payload.new_locations {
        for (file_uri, block) in &new_locations {
            if let Err(e) = perf
                .rebuild_stats_location_map
                .merge_parallel_arrays(file_uri, block)
            {
                tracing::warn!("Failed to merge location block for '{}': {}", file_uri, e);
            }
        }
    }

    // 2. Resolve location IDs → RebuildLocation entries (best-effort, skip unknowns).
    let rebuilds: Vec<RebuildLocation> = payload
        .events
        .iter()
        .filter_map(|(id, count)| {
            let location = perf.rebuild_stats_location_map.by_id.get(id)?.clone();
            Some(RebuildLocation {
                location,
                build_count: *count,
            })
        })
        .collect();

    // 3. Build and append the per-frame snapshot.
    let snapshot = RebuildStatsSnapshot {
        frame_number: payload.frame_number,
        start_time_micros: payload.start_time_micros,
        rebuilds,
    };
    perf.rebuild_stats_frames.push_back(snapshot);

    // Evict oldest snapshot if over the window limit.
    let window = frame_window.max(1); // never allow 0-size window
    while perf.rebuild_stats_frames.len() > window {
        perf.rebuild_stats_frames.pop_front();
    }

    // 4. Update lifetime totals.
    for (id, count) in &payload.events {
        *perf.rebuild_stats_totals.entry(*id).or_insert(0) += count;
    }

    UpdateResult::none()
}

// ── handle_toggle ─────────────────────────────────────────────────────────────

/// Handle a `ToggleRebuildStats` request from the user (`R` key).
///
/// Spawns an async action that calls `ext.flutter.profileWidgetBuilds` with
/// the opposite of the current state, then emits
/// `Message::RebuildStatsExtensionStateChanged` on success.
pub(crate) fn handle_toggle(state: &mut AppState, session_id: SessionId) -> UpdateResult {
    let Some(handle) = state.session_manager.get(session_id) else {
        return UpdateResult::none();
    };

    let target = !handle.session.performance.rebuild_stats_enabled;

    UpdateResult::action(UpdateAction::ToggleProfileWidgetBuilds {
        session_id,
        enabled: target,
        vm_handle: None, // hydrated by process.rs
    })
}

// ── handle_extension_state_changed ───────────────────────────────────────────

/// Handle the async result of `ToggleRebuildStats`.
///
/// When `enabled` flips to `false`, clears the rebuild stats ring buffer and
/// totals, and snaps the active details tab away from `RebuildStats` if needed.
/// When `enabled` flips to `true`, triggers a one-shot `widgetLocationIdMap`
/// fetch to seed the location map.
pub(crate) fn handle_extension_state_changed(
    state: &mut AppState,
    session_id: SessionId,
    enabled: bool,
) -> UpdateResult {
    let Some(handle) = state.session_manager.get_mut(session_id) else {
        return UpdateResult::none();
    };

    handle.session.performance.rebuild_stats_enabled = enabled;

    if !enabled {
        // Clear accumulated rebuild state.
        handle.session.performance.rebuild_stats_totals.clear();
        handle.session.performance.rebuild_stats_frames.clear();
        handle.session.performance.rebuild_stats_scroll_offset = 0;
        handle.session.performance.rebuild_stats_selected_row = None;

        // If the user was looking at the Rebuild Stats tab, snap to Timeline.
        if handle.session.performance.details_tab == PerfDetailsTab::RebuildStats {
            handle.session.performance.details_tab = PerfDetailsTab::TimelineEvents;
        }
        return UpdateResult::none();
    }

    // enabled = true: trigger one-shot widgetLocationIdMap fetch.
    UpdateResult::action(UpdateAction::FetchWidgetLocationIdMap {
        session_id,
        vm_handle: None, // hydrated by process.rs
    })
}

// ── handle_location_map_fetched ───────────────────────────────────────────────

/// Merge a newly fetched `widgetLocationIdMap` into the persistent location map.
pub(crate) fn handle_location_map_fetched(
    state: &mut AppState,
    session_id: SessionId,
    map: LocationMap,
) -> UpdateResult {
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        // Merge by inserting all entries from the fetched map.
        for (id, location) in map.by_id {
            handle
                .session
                .performance
                .rebuild_stats_location_map
                .by_id
                .insert(id, location);
        }
    }
    UpdateResult::none()
}

// ── handle_toggle_failed ──────────────────────────────────────────────────────

/// Handle a failed `ToggleProfileWidgetBuilds` RPC or `widgetLocationIdMap` fetch.
///
/// Appends a `LogEntry` with [`fdemon_core::LogLevel::Warning`] to the session's
/// log buffer so the user knows the rebuild-tracking toggle (or location-map fetch)
/// did not succeed. Returns `UpdateResult::none()` — no further action required,
/// since the companion `RebuildStatsExtensionStateChanged` rollback message has
/// already been emitted by the action task.
pub(crate) fn handle_toggle_failed(
    state: &mut AppState,
    session_id: SessionId,
    reason: String,
) -> UpdateResult {
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        handle.session.add_log(fdemon_core::LogEntry::new(
            fdemon_core::LogLevel::Warning,
            fdemon_core::LogSource::App,
            format!("Rebuild tracking toggle failed: {reason}"),
        ));
    }
    UpdateResult::none()
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::update::update;
    use crate::message::Message;
    use crate::state::{AppState, DevToolsPanel, PerfDetailsTab, UiMode};
    use fdemon_core::rebuild_stats::{
        Location, LocationMap, RebuildEventPayload, RebuildStatsSnapshot,
    };
    use serde_json::json;

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
            is_supported: true,
            capabilities: None,
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

    fn make_payload(
        frame_number: u64,
        start_time: u64,
        events: Vec<(u32, u32)>,
        new_locations: Option<std::collections::HashMap<String, serde_json::Value>>,
    ) -> RebuildEventPayload {
        RebuildEventPayload {
            frame_number,
            start_time_micros: start_time,
            events,
            new_locations,
        }
    }

    // ── handle_event: basic accumulation ─────────────────────────────────────

    #[test]
    fn handle_event_appends_snapshot() {
        let (mut state, session_id) = make_state_with_session();
        // Seed the location map with id=1.
        if let Some(h) = state.session_manager.get_mut(session_id) {
            h.session
                .performance
                .rebuild_stats_location_map
                .by_id
                .insert(
                    1,
                    Location {
                        file_uri: "package:foo/main.dart".to_string(),
                        line: 10,
                        column: 3,
                        name: "MyWidget".to_string(),
                    },
                );
        }

        let payload = make_payload(1, 1000, vec![(1, 2)], None);
        update(
            &mut state,
            Message::RebuildStatsEventReceived {
                session_id,
                payload,
            },
        );

        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        assert_eq!(perf.rebuild_stats_frames.len(), 1);
        assert_eq!(perf.rebuild_stats_frames[0].frame_number, 1);
        assert_eq!(perf.rebuild_stats_frames[0].rebuilds.len(), 1);
        assert_eq!(perf.rebuild_stats_frames[0].rebuilds[0].build_count, 2);
        assert_eq!(*perf.rebuild_stats_totals.get(&1).unwrap(), 2);
    }

    #[test]
    fn handle_event_evicts_oldest_at_window_boundary() {
        let (mut state, session_id) = make_state_with_session();
        // Set a small window.
        state.settings.devtools.rebuild_stats_frame_window = 3;

        // Push 4 frames.
        for i in 1_u64..=4 {
            let payload = make_payload(i, i * 1000, vec![], None);
            update(
                &mut state,
                Message::RebuildStatsEventReceived {
                    session_id,
                    payload,
                },
            );
        }

        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        // Only the 3 most recent frames should remain.
        assert_eq!(perf.rebuild_stats_frames.len(), 3);
        assert_eq!(perf.rebuild_stats_frames[0].frame_number, 2); // oldest kept
        assert_eq!(perf.rebuild_stats_frames[2].frame_number, 4); // newest
    }

    #[test]
    fn handle_event_skips_unknown_location_ids() {
        let (mut state, session_id) = make_state_with_session();
        // No location map seeded — id=99 is unknown.
        let payload = make_payload(1, 0, vec![(99, 5)], None);
        update(
            &mut state,
            Message::RebuildStatsEventReceived {
                session_id,
                payload,
            },
        );

        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        assert_eq!(perf.rebuild_stats_frames.len(), 1);
        assert_eq!(perf.rebuild_stats_frames[0].rebuilds.len(), 0); // skipped
                                                                    // Totals still accumulate even for unknown IDs.
        assert_eq!(*perf.rebuild_stats_totals.get(&99).unwrap(), 5);
    }

    #[test]
    fn handle_event_merges_new_locations_inline() {
        let (mut state, session_id) = make_state_with_session();

        let mut new_locations = std::collections::HashMap::new();
        new_locations.insert(
            "package:foo/main.dart".to_string(),
            json!({
                "ids": [1],
                "lines": [23],
                "columns": [5],
                "names": ["TestWidget"]
            }),
        );
        let payload = make_payload(1, 0, vec![(1, 3)], Some(new_locations));
        update(
            &mut state,
            Message::RebuildStatsEventReceived {
                session_id,
                payload,
            },
        );

        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        assert!(perf.rebuild_stats_location_map.by_id.contains_key(&1));
        assert_eq!(perf.rebuild_stats_frames[0].rebuilds[0].build_count, 3);
    }

    // ── handle_extension_state_changed: disable ───────────────────────────────

    #[test]
    fn handle_extension_state_changed_false_clears_state() {
        let (mut state, session_id) = make_state_with_session();

        // Set up some existing state.
        if let Some(h) = state.session_manager.get_mut(session_id) {
            h.session.performance.rebuild_stats_enabled = true;
            h.session.performance.rebuild_stats_totals.insert(1, 5);
            h.session
                .performance
                .rebuild_stats_frames
                .push_back(RebuildStatsSnapshot {
                    frame_number: 1,
                    start_time_micros: 0,
                    rebuilds: vec![],
                });
        }

        update(
            &mut state,
            Message::RebuildStatsExtensionStateChanged {
                session_id,
                enabled: false,
            },
        );

        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        assert!(!perf.rebuild_stats_enabled);
        assert!(perf.rebuild_stats_totals.is_empty());
        assert!(perf.rebuild_stats_frames.is_empty());
    }

    #[test]
    fn handle_extension_state_changed_false_snaps_tab_from_rebuild_stats() {
        let (mut state, session_id) = make_state_with_session();
        state.ui_mode = UiMode::DevTools;
        state.devtools_view_state.active_panel = DevToolsPanel::Performance;

        if let Some(h) = state.session_manager.get_mut(session_id) {
            h.session.performance.rebuild_stats_enabled = true;
            h.session.performance.details_tab = PerfDetailsTab::RebuildStats;
        }

        update(
            &mut state,
            Message::RebuildStatsExtensionStateChanged {
                session_id,
                enabled: false,
            },
        );

        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        // Should have snapped to TimelineEvents.
        assert_eq!(perf.details_tab, PerfDetailsTab::TimelineEvents);
    }

    #[test]
    fn handle_extension_state_changed_false_does_not_snap_other_tabs() {
        let (mut state, session_id) = make_state_with_session();

        if let Some(h) = state.session_manager.get_mut(session_id) {
            h.session.performance.rebuild_stats_enabled = true;
            h.session.performance.details_tab = PerfDetailsTab::FrameAnalysis;
        }

        update(
            &mut state,
            Message::RebuildStatsExtensionStateChanged {
                session_id,
                enabled: false,
            },
        );

        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        // FrameAnalysis should stay.
        assert_eq!(perf.details_tab, PerfDetailsTab::FrameAnalysis);
    }

    // ── handle_location_map_fetched ───────────────────────────────────────────

    #[test]
    fn handle_location_map_fetched_merges_into_existing() {
        let (mut state, session_id) = make_state_with_session();

        // Pre-seed one entry.
        if let Some(h) = state.session_manager.get_mut(session_id) {
            h.session
                .performance
                .rebuild_stats_location_map
                .by_id
                .insert(
                    1,
                    Location {
                        file_uri: "package:foo/a.dart".to_string(),
                        line: 1,
                        column: 1,
                        name: "OldWidget".to_string(),
                    },
                );
        }

        let mut new_map = LocationMap::default();
        new_map.by_id.insert(
            2,
            Location {
                file_uri: "package:foo/b.dart".to_string(),
                line: 10,
                column: 5,
                name: "NewWidget".to_string(),
            },
        );

        update(
            &mut state,
            Message::RebuildStatsLocationMapFetched {
                session_id,
                map: new_map,
            },
        );

        let perf = &state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .performance;
        assert!(perf.rebuild_stats_location_map.by_id.contains_key(&1));
        assert!(perf.rebuild_stats_location_map.by_id.contains_key(&2));
    }

    // ── handle_toggle_failed ──────────────────────────────────────────────────

    /// Verify that `RebuildStatsToggleFailed` appends exactly one log entry to
    /// the session's log buffer with the expected message text.
    #[test]
    fn test_handle_toggle_failed_appends_log_entry() {
        let (mut state, session_id) = make_state_with_session();

        let initial_log_count = state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .logs
            .len();

        update(
            &mut state,
            Message::RebuildStatsToggleFailed {
                session_id,
                reason: "test error reason".to_string(),
            },
        );

        let session = state.session_manager.get(session_id).unwrap();
        assert_eq!(
            session.session.logs.len(),
            initial_log_count + 1,
            "expected one new log entry"
        );

        let last = session.session.logs.iter().last().unwrap();
        assert!(
            last.message.contains("test error reason"),
            "log message should contain the reason: {}",
            last.message
        );
        assert!(
            last.message.contains("Rebuild tracking toggle failed"),
            "log message should mention 'Rebuild tracking toggle failed': {}",
            last.message
        );
        assert_eq!(
            last.level,
            fdemon_core::LogLevel::Warning,
            "log entry should use Warning level"
        );
    }

    /// Verify that when both rollback (`RebuildStatsExtensionStateChanged`) and
    /// failure (`RebuildStatsToggleFailed`) messages arrive (as emitted by the
    /// `ToggleProfileWidgetBuilds` action on RPC failure), the state is consistent:
    /// - The optimistic state is rolled back.
    /// - The session log has a failure entry.
    #[test]
    fn test_toggle_failure_emits_rollback_and_log() {
        let (mut state, session_id) = make_state_with_session();

        // Simulate: user presses R, handle_toggle optimistically sets enabled = true.
        if let Some(h) = state.session_manager.get_mut(session_id) {
            h.session.performance.rebuild_stats_enabled = true;
        }

        let initial_log_count = state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .logs
            .len();

        // The action task emits:
        //   1. RebuildStatsExtensionStateChanged { enabled: false } — rollback
        //   2. RebuildStatsToggleFailed { reason: "..." }
        update(
            &mut state,
            Message::RebuildStatsExtensionStateChanged {
                session_id,
                enabled: false,
            },
        );
        update(
            &mut state,
            Message::RebuildStatsToggleFailed {
                session_id,
                reason: "isolate disconnected".to_string(),
            },
        );

        let session = state.session_manager.get(session_id).unwrap();

        // State should be rolled back to disabled.
        assert!(
            !session.session.performance.rebuild_stats_enabled,
            "rebuild_stats_enabled should be false after rollback"
        );

        // Log buffer should have grown by at least one entry (the failure notice).
        assert!(
            session.session.logs.len() > initial_log_count,
            "expected at least one new log entry from toggle failure"
        );

        let last = session.session.logs.iter().last().unwrap();
        assert!(
            last.message.contains("isolate disconnected"),
            "log should contain the failure reason: {}",
            last.message
        );
    }

    /// Verify that `RebuildStatsToggleFailed` is also the mechanism used when the
    /// location-map fetch fails (observable behavior: same handler path).
    ///
    /// This verifies the handler contract for AC #3 and AC #5 from the task spec.
    /// The action-layer wiring (T04) calls the same message variant whether the
    /// failure comes from `ToggleProfileWidgetBuilds` or `FetchWidgetLocationIdMap`.
    #[test]
    fn test_location_map_fetch_failure_emits_toggle_failed() {
        let (mut state, session_id) = make_state_with_session();

        let initial_log_count = state
            .session_manager
            .get(session_id)
            .unwrap()
            .session
            .logs
            .len();

        // A `FetchWidgetLocationIdMap` failure sends `RebuildStatsToggleFailed`
        // with a reason that includes "Failed to fetch widget location map".
        update(
            &mut state,
            Message::RebuildStatsToggleFailed {
                session_id,
                reason: "Failed to fetch widget location map: channel closed".to_string(),
            },
        );

        let session = state.session_manager.get(session_id).unwrap();
        assert_eq!(
            session.session.logs.len(),
            initial_log_count + 1,
            "expected one new log entry for location-map fetch failure"
        );

        let last = session.session.logs.iter().last().unwrap();
        assert!(
            last.message.contains("Failed to fetch widget location map"),
            "log should contain the location-map failure reason: {}",
            last.message
        );
    }
}
