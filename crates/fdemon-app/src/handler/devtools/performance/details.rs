//! Performance Details pane handlers — Phase 2 tab cycling.

use crate::handler::UpdateResult;
use crate::session::performance::PerfSection;
use crate::state::{AppState, PerfDetailsTab};

/// Cycle the active tab in the Performance Details pane.
///
/// Only mutates state when the user actually has the Details section focused;
/// otherwise the key emission is a no-op. (The keys.rs guard already enforces
/// this, but the handler is defensive — a future mouse-driven dispatch path
/// could land here without the keyboard guard.)
///
/// Forward cycling respects rebuild-stats visibility: when
/// `rebuild_stats_enabled == false`, the cycle skips `RebuildStats` and goes
/// directly `FrameAnalysis → TimelineEvents → FrameAnalysis`.
pub(crate) fn handle_perf_cycle_details_tab(state: &mut AppState, forward: bool) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        if handle.session.performance.focused_section != PerfSection::Details {
            return UpdateResult::none();
        }
        let rebuild_stats_enabled = handle.session.performance.rebuild_stats_enabled;
        let next = if forward {
            handle
                .session
                .performance
                .details_tab
                .next_visible(rebuild_stats_enabled)
        } else {
            handle.session.performance.details_tab.prev()
        };
        handle.session.performance.details_tab = next;
    }
    UpdateResult::none()
}

/// Set the active tab in the Performance Details pane directly.
///
/// Phase 2: emitted only by tests. Phase 3 wires up mouse-click regions on the
/// tab strip that emit this variant.
pub(crate) fn handle_perf_focus_details_tab(
    state: &mut AppState,
    tab: PerfDetailsTab,
) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        handle.session.performance.details_tab = tab;
    }
    UpdateResult::none()
}

#[cfg(test)]
mod tests {
    use crate::handler::update::update;
    use crate::message::Message;
    use crate::session::performance::PerfSection;
    use crate::state::{AppState, DevToolsPanel, PerfDetailsTab, UiMode};

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

    fn make_state_in_performance_details() -> AppState {
        let mut state = AppState::new();
        let _id = state
            .session_manager
            .create_session(&test_device())
            .unwrap();
        state.ui_mode = UiMode::DevTools;
        state.devtools_view_state.active_panel = DevToolsPanel::Performance;
        if let Some(h) = state.session_manager.selected_mut() {
            h.session.performance.focused_section = PerfSection::Details;
        }
        state
    }

    #[test]
    fn cycle_forward_advances_details_tab() {
        // rebuild_stats_enabled defaults to false, so FrameAnalysis → TimelineEvents
        // (RebuildStats is skipped via next_visible).
        let mut state = make_state_in_performance_details();
        update(&mut state, Message::PerfCycleDetailsTab { forward: true });
        assert_eq!(
            state
                .session_manager
                .selected()
                .unwrap()
                .session
                .performance
                .details_tab,
            PerfDetailsTab::TimelineEvents,
        );
    }

    #[test]
    fn cycle_forward_advances_details_tab_with_rebuild_enabled() {
        // With rebuild_stats_enabled = true, FrameAnalysis → RebuildStats (full cycle).
        let mut state = make_state_in_performance_details();
        if let Some(h) = state.session_manager.selected_mut() {
            h.session.performance.rebuild_stats_enabled = true;
        }
        update(&mut state, Message::PerfCycleDetailsTab { forward: true });
        assert_eq!(
            state
                .session_manager
                .selected()
                .unwrap()
                .session
                .performance
                .details_tab,
            PerfDetailsTab::RebuildStats,
        );
    }

    #[test]
    fn cycle_backward_wraps_to_timeline_events() {
        let mut state = make_state_in_performance_details();
        update(&mut state, Message::PerfCycleDetailsTab { forward: false });
        assert_eq!(
            state
                .session_manager
                .selected()
                .unwrap()
                .session
                .performance
                .details_tab,
            PerfDetailsTab::TimelineEvents,
        );
    }

    #[test]
    fn cycle_is_noop_when_frame_chart_focused() {
        let mut state = make_state_in_performance_details();
        if let Some(h) = state.session_manager.selected_mut() {
            h.session.performance.focused_section = PerfSection::FrameChart;
        }
        update(&mut state, Message::PerfCycleDetailsTab { forward: true });
        assert_eq!(
            state
                .session_manager
                .selected()
                .unwrap()
                .session
                .performance
                .details_tab,
            PerfDetailsTab::FrameAnalysis,
        );
    }

    #[test]
    fn focus_details_tab_sets_active_tab() {
        let mut state = make_state_in_performance_details();
        update(
            &mut state,
            Message::PerfFocusDetailsTab(PerfDetailsTab::TimelineEvents),
        );
        assert_eq!(
            state
                .session_manager
                .selected()
                .unwrap()
                .session
                .performance
                .details_tab,
            PerfDetailsTab::TimelineEvents,
        );
    }

    // ── Visibility-aware cycle tests (M1) ────────────────────────────────────

    #[test]
    fn test_cycle_skips_rebuild_stats_when_disabled() {
        // FrameAnalysis → TimelineEvents → FrameAnalysis when rebuild tracking is off.
        let mut state = make_state_in_performance_details();
        // Ensure rebuild_stats_enabled is false (default).
        if let Some(h) = state.session_manager.selected_mut() {
            h.session.performance.rebuild_stats_enabled = false;
            h.session.performance.details_tab = PerfDetailsTab::FrameAnalysis;
        }

        // First forward cycle: FrameAnalysis → TimelineEvents (skipping RebuildStats).
        update(&mut state, Message::PerfCycleDetailsTab { forward: true });
        assert_eq!(
            state
                .session_manager
                .selected()
                .unwrap()
                .session
                .performance
                .details_tab,
            PerfDetailsTab::TimelineEvents,
            "With rebuild disabled, FrameAnalysis should cycle directly to TimelineEvents"
        );

        // Second forward cycle: TimelineEvents → FrameAnalysis.
        update(&mut state, Message::PerfCycleDetailsTab { forward: true });
        assert_eq!(
            state
                .session_manager
                .selected()
                .unwrap()
                .session
                .performance
                .details_tab,
            PerfDetailsTab::FrameAnalysis,
            "With rebuild disabled, TimelineEvents should cycle back to FrameAnalysis"
        );
    }

    #[test]
    fn test_cycle_includes_rebuild_stats_when_enabled() {
        // FrameAnalysis → RebuildStats → TimelineEvents → FrameAnalysis when on.
        let mut state = make_state_in_performance_details();
        if let Some(h) = state.session_manager.selected_mut() {
            h.session.performance.rebuild_stats_enabled = true;
            h.session.performance.details_tab = PerfDetailsTab::FrameAnalysis;
        }

        // First: FrameAnalysis → RebuildStats.
        update(&mut state, Message::PerfCycleDetailsTab { forward: true });
        assert_eq!(
            state
                .session_manager
                .selected()
                .unwrap()
                .session
                .performance
                .details_tab,
            PerfDetailsTab::RebuildStats,
            "With rebuild enabled, FrameAnalysis should cycle to RebuildStats"
        );

        // Second: RebuildStats → TimelineEvents.
        update(&mut state, Message::PerfCycleDetailsTab { forward: true });
        assert_eq!(
            state
                .session_manager
                .selected()
                .unwrap()
                .session
                .performance
                .details_tab,
            PerfDetailsTab::TimelineEvents,
            "With rebuild enabled, RebuildStats should cycle to TimelineEvents"
        );

        // Third: TimelineEvents → FrameAnalysis.
        update(&mut state, Message::PerfCycleDetailsTab { forward: true });
        assert_eq!(
            state
                .session_manager
                .selected()
                .unwrap()
                .session
                .performance
                .details_tab,
            PerfDetailsTab::FrameAnalysis,
            "With rebuild enabled, TimelineEvents should cycle back to FrameAnalysis"
        );
    }
}
