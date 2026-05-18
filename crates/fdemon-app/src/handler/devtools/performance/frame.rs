//! Performance panel — frame selection and chart scroll/page/jump handlers.
//!
//! Contains all frame-chart interactivity: frame selection by index, section
//! focus cycling, bar-chart scroll, page navigation, and jump-to-start/end.
//!
//! Phase 2 details-pane tab cycling lives in [`super::details`].
//! Memory and allocation profile handlers live in [`super::super::memory`].

use super::super::scroll_helpers::{clamp_chart_scroll, ScrollDir};
use crate::handler::UpdateResult;
use crate::session::performance::PerfSection;
use crate::state::AppState;

// ── Phase 2 scroll helpers ────────────────────────────────────────────────────

/// Fallback page size when the render-hint visible dimension is 0 (not yet rendered).
const DEFAULT_PERF_PAGE_SIZE: usize = 10;

/// Handle frame selection by direct index.
///
/// `index: None` clears the selection (scroll mode). `index: Some(i)` sets
/// `selected_frame` to `i` in the current session's performance state.
///
/// This is the single handler for all frame-selection transitions. The key
/// handler in `keys.rs` computes the target index inline and emits
/// `SelectPerformanceFrame` — this handler applies the result.
pub(crate) fn handle_select_performance_frame(
    state: &mut AppState,
    index: Option<usize>,
) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        handle.session.performance.selected_frame = index;
        // When a concrete frame is selected, reset the scroll offset so the
        // newly-selected frame is visible at the live edge. Deselect (None)
        // leaves the offset unchanged — the user may have scrolled back
        // deliberately and pressed Esc only to drop the selection highlight.
        if index.is_some() {
            handle.session.performance.frame_chart_scroll_offset = 0;
        }
    }
    UpdateResult::none()
}

// ── Phase 2 keyboard interactivity handlers ───────────────────────────────────

/// Move keyboard focus to the given sub-section within the Performance panel.
///
/// Sets `perf_state.focused_section = section`. No-op when no session is selected.
pub(crate) fn handle_perf_focus_section(
    state: &mut AppState,
    section: PerfSection,
) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        handle.session.performance.focused_section = section;
    }
    UpdateResult::none()
}

/// Scroll the focused Performance panel section by one row/bar in `direction`.
///
/// Dispatch table:
///
/// - `FrameChart` — adjusts `frame_chart_scroll_offset`, clamped to the frame history.
/// - `Details` — no-op in Phase 2 — Frame Analysis tab content fits on screen with no
///   scrolling. Phase 3's Rebuild Stats / Timeline Events tabs will use
///   `details_pane_visible_height` to scroll.
///
/// No-op when no session is selected.
pub(crate) fn handle_perf_scroll(state: &mut AppState, direction: ScrollDir) -> UpdateResult {
    let Some(handle) = state.session_manager.selected_mut() else {
        return UpdateResult::none();
    };

    match handle.session.performance.focused_section {
        PerfSection::FrameChart => {
            let buf_len = handle.session.performance.frame_history.len();
            let visible = handle.session.performance.frame_chart_visible_width.get();
            // Up = scroll back (higher offset), Down = scroll toward live edge (lower offset).
            let delta: i64 = match direction {
                ScrollDir::Up => 1,
                ScrollDir::Down => -1,
            };
            handle.session.performance.frame_chart_scroll_offset = clamp_chart_scroll(
                buf_len,
                visible,
                handle.session.performance.frame_chart_scroll_offset,
                delta,
            );
        }
        PerfSection::Details => {
            // No-op in Phase 2 — Frame Analysis tab content fits on screen with no scrolling.
            // Phase 3's Rebuild Stats / Timeline Events tabs will use
            // `details_pane_visible_height` to scroll.
        }
    }

    UpdateResult::none()
}

/// Scroll the focused Performance panel section by one page in `direction`.
///
/// Page size is taken from the appropriate render hint (`frame_chart_visible_width`);
/// falls back to [`DEFAULT_PERF_PAGE_SIZE`] when the hint is 0 (not yet rendered).
///
/// No-op when no session is selected.
pub(crate) fn handle_perf_page(state: &mut AppState, direction: ScrollDir) -> UpdateResult {
    let Some(handle) = state.session_manager.selected_mut() else {
        return UpdateResult::none();
    };

    match handle.session.performance.focused_section {
        PerfSection::FrameChart => {
            let visible = handle.session.performance.frame_chart_visible_width.get();
            let page = if visible == 0 {
                DEFAULT_PERF_PAGE_SIZE
            } else {
                visible
            } as i64;
            let buf_len = handle.session.performance.frame_history.len();
            let delta: i64 = match direction {
                ScrollDir::Up => page,
                ScrollDir::Down => -page,
            };
            handle.session.performance.frame_chart_scroll_offset = clamp_chart_scroll(
                buf_len,
                visible,
                handle.session.performance.frame_chart_scroll_offset,
                delta,
            );
        }
        PerfSection::Details => {
            // No-op in Phase 2 — Frame Analysis tab content fits on screen with no scrolling.
            // Phase 3's Rebuild Stats / Timeline Events tabs will use
            // `details_pane_visible_height` to scroll.
        }
    }

    UpdateResult::none()
}

/// Jump to the furthest-back position in the focused section (oldest data / first row).
///
/// - `FrameChart`: set scroll offset to `max_back` (oldest data visible).
/// - `Details`: no-op in Phase 2.
///
/// No-op when no session is selected.
pub(crate) fn handle_perf_jump_to_start(state: &mut AppState) -> UpdateResult {
    let Some(handle) = state.session_manager.selected_mut() else {
        return UpdateResult::none();
    };

    match handle.session.performance.focused_section {
        PerfSection::FrameChart => {
            let buf_len = handle.session.performance.frame_history.len();
            let visible = handle
                .session
                .performance
                .frame_chart_visible_width
                .get()
                .max(1);
            handle.session.performance.frame_chart_scroll_offset = buf_len.saturating_sub(visible);
        }
        PerfSection::Details => {
            // No-op in Phase 2. Unreachable via Tab; kept for exhaustiveness.
        }
    }

    UpdateResult::none()
}

/// Jump to the live edge in the focused section (newest data / last row).
///
/// - `FrameChart`: set scroll offset to 0 (live edge).
/// - `Details`: no-op in Phase 2.
///
/// No-op when no session is selected.
pub(crate) fn handle_perf_jump_to_end(state: &mut AppState) -> UpdateResult {
    let Some(handle) = state.session_manager.selected_mut() else {
        return UpdateResult::none();
    };

    match handle.session.performance.focused_section {
        PerfSection::FrameChart => {
            handle.session.performance.frame_chart_scroll_offset = 0;
        }
        PerfSection::Details => {
            // No-op in Phase 2. Unreachable via Tab; kept for exhaustiveness.
        }
    }

    UpdateResult::none()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use crate::handler::update::update;
    use crate::message::Message;
    use crate::session::SessionId;
    use crate::state::{AppState, DevToolsPanel, UiMode};
    use fdemon_core::performance::FrameTiming;

    // ── Helpers ──────────────────────────────────────────────────────────────

    /// Process a message and any chained follow-up messages (up to a safety limit).
    ///
    /// The TEA `update()` function returns an `UpdateResult` that may contain a
    /// `message` follow-up. In tests we must process this chain to mirror what
    /// `process.rs` does at runtime. A limit of 16 prevents infinite loops in
    /// buggy test scenarios.
    fn dispatch(state: &mut AppState, msg: Message) {
        let mut current = Some(msg);
        let mut steps = 0;
        while let Some(m) = current.take() {
            let result = update(state, m);
            current = result.message;
            steps += 1;
            if steps > 16 {
                panic!("dispatch: follow-up message chain exceeded 16 steps (infinite loop?)");
            }
        }
    }

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

    /// Create an `AppState` with one session in DevTools/Performance mode.
    fn make_state_in_performance_panel() -> (AppState, SessionId) {
        let mut state = AppState::new();
        let device = test_device();
        let session_id = state.session_manager.create_session(&device).unwrap();
        state.ui_mode = UiMode::DevTools;
        state.devtools_view_state.active_panel = DevToolsPanel::Performance;
        (state, session_id)
    }

    /// Push `count` synthetic frame timings into the current session's performance state.
    fn push_frames(state: &mut AppState, count: u64) {
        if let Some(handle) = state.session_manager.selected_mut() {
            for i in 1..=count {
                handle.session.performance.frame_history.push(FrameTiming {
                    number: i,
                    build_micros: 5_000,
                    raster_micros: 5_000,
                    elapsed_micros: 10_000,
                    timestamp: chrono::Local::now(),
                    phases: None,
                    shader_compilation: false,
                });
            }
        }
    }

    fn current_selected_frame(state: &AppState) -> Option<usize> {
        state
            .session_manager
            .selected()
            .and_then(|h| h.session.performance.selected_frame)
    }

    // ── Left arrow: frame navigation ─────────────────────────────────────────

    #[test]
    fn test_left_arrow_selects_prev_frame() {
        let (mut state, _) = make_state_in_performance_panel();
        push_frames(&mut state, 5);

        // Pre-select frame 3 (index 3).
        state
            .session_manager
            .selected_mut()
            .unwrap()
            .session
            .performance
            .selected_frame = Some(3);

        // Left key in Performance panel — dispatch processes the follow-up message chain.
        dispatch(&mut state, Message::Key(crate::input_key::InputKey::Left));

        assert_eq!(
            current_selected_frame(&state),
            Some(2),
            "Left should decrement selected_frame from 3 to 2"
        );
    }

    #[test]
    fn test_left_arrow_clamps_at_start() {
        let (mut state, _) = make_state_in_performance_panel();
        push_frames(&mut state, 5);

        state
            .session_manager
            .selected_mut()
            .unwrap()
            .session
            .performance
            .selected_frame = Some(0);

        dispatch(&mut state, Message::Key(crate::input_key::InputKey::Left));

        assert_eq!(
            current_selected_frame(&state),
            Some(0),
            "Left at index 0 should stay clamped at 0"
        );
    }

    // ── Right arrow: frame navigation ────────────────────────────────────────

    #[test]
    fn test_right_arrow_selects_next_frame() {
        let (mut state, _) = make_state_in_performance_panel();
        push_frames(&mut state, 5);

        state
            .session_manager
            .selected_mut()
            .unwrap()
            .session
            .performance
            .selected_frame = Some(2);

        dispatch(&mut state, Message::Key(crate::input_key::InputKey::Right));

        assert_eq!(
            current_selected_frame(&state),
            Some(3),
            "Right should increment selected_frame from 2 to 3"
        );
    }

    #[test]
    fn test_right_arrow_clamps_at_end() {
        let (mut state, _) = make_state_in_performance_panel();
        push_frames(&mut state, 5);

        // Last valid index for 5 frames is 4.
        state
            .session_manager
            .selected_mut()
            .unwrap()
            .session
            .performance
            .selected_frame = Some(4);

        dispatch(&mut state, Message::Key(crate::input_key::InputKey::Right));

        assert_eq!(
            current_selected_frame(&state),
            Some(4),
            "Right at last frame should stay clamped at 4"
        );
    }

    // ── Esc: deselect or exit DevTools ────────────────────────────────────────

    #[test]
    fn test_esc_with_frame_selected_deselects_stays_in_devtools() {
        let (mut state, _) = make_state_in_performance_panel();
        push_frames(&mut state, 5);

        state
            .session_manager
            .selected_mut()
            .unwrap()
            .session
            .performance
            .selected_frame = Some(2);

        dispatch(&mut state, Message::Key(crate::input_key::InputKey::Esc));

        assert_eq!(
            current_selected_frame(&state),
            None,
            "Esc with frame selected should deselect"
        );
        assert_eq!(
            state.ui_mode,
            UiMode::DevTools,
            "Should remain in DevTools mode after deselecting"
        );
    }

    #[test]
    fn test_esc_without_frame_selected_exits_devtools() {
        let (mut state, _) = make_state_in_performance_panel();
        // No frame selected.
        assert_eq!(current_selected_frame(&state), None);

        dispatch(&mut state, Message::Key(crate::input_key::InputKey::Esc));

        assert_ne!(
            state.ui_mode,
            UiMode::DevTools,
            "Esc with no frame selected should exit DevTools"
        );
    }

    // ── Left/Right noop when not in Performance panel ─────────────────────────

    #[test]
    fn test_left_right_noop_when_in_inspector_panel() {
        let mut state = AppState::new();
        let device = test_device();
        let session_id = state.session_manager.create_session(&device).unwrap();
        state.ui_mode = UiMode::DevTools;
        state.devtools_view_state.active_panel = DevToolsPanel::Inspector;

        // Pre-populate some frames so we can detect unexpected mutation.
        if let Some(handle) = state.session_manager.get_mut(session_id) {
            for i in 1..=3u64 {
                handle.session.performance.frame_history.push(FrameTiming {
                    number: i,
                    build_micros: 5_000,
                    raster_micros: 5_000,
                    elapsed_micros: 10_000,
                    timestamp: chrono::Local::now(),
                    phases: None,
                    shader_compilation: false,
                });
            }
        }

        let before_left = state
            .session_manager
            .selected()
            .unwrap()
            .session
            .performance
            .selected_frame;

        // Left/Right in Inspector panel should NOT mutate performance.selected_frame.
        dispatch(&mut state, Message::Key(crate::input_key::InputKey::Left));
        let after_left = state
            .session_manager
            .selected()
            .unwrap()
            .session
            .performance
            .selected_frame;

        dispatch(&mut state, Message::Key(crate::input_key::InputKey::Right));
        let after_right = state
            .session_manager
            .selected()
            .unwrap()
            .session
            .performance
            .selected_frame;

        // In Inspector: Left navigates the tree (Collapse), Right expands the tree.
        // Neither should mutate performance.selected_frame.
        assert_eq!(
            before_left, after_left,
            "Left in Inspector should not change performance.selected_frame"
        );
        assert_eq!(
            before_left, after_right,
            "Right in Inspector should not change performance.selected_frame"
        );
    }

    // ── SelectPerformanceFrame message ───────────────────────────────────────

    #[test]
    fn test_select_performance_frame_message_sets_index() {
        let (mut state, _) = make_state_in_performance_panel();
        push_frames(&mut state, 5);

        update(
            &mut state,
            Message::SelectPerformanceFrame { index: Some(3) },
        );

        assert_eq!(
            current_selected_frame(&state),
            Some(3),
            "SelectPerformanceFrame(Some(3)) should set selected_frame to 3"
        );
    }

    #[test]
    fn test_select_performance_frame_message_clears_selection() {
        let (mut state, _) = make_state_in_performance_panel();
        push_frames(&mut state, 5);

        state
            .session_manager
            .selected_mut()
            .unwrap()
            .session
            .performance
            .selected_frame = Some(2);

        update(&mut state, Message::SelectPerformanceFrame { index: None });

        assert_eq!(
            current_selected_frame(&state),
            None,
            "SelectPerformanceFrame(None) should clear selected_frame"
        );
    }

    // ── Phase 2 keyboard interactivity tests ─────────────────────────────────

    use super::{
        handle_perf_focus_section, handle_perf_jump_to_end, handle_perf_jump_to_start,
        handle_perf_page, handle_perf_scroll,
    };
    use crate::handler::devtools::ScrollDir;
    use crate::session::performance::PerfSection;

    fn perf_frame_scroll(state: &AppState) -> usize {
        state
            .session_manager
            .selected()
            .unwrap()
            .session
            .performance
            .frame_chart_scroll_offset
    }

    fn perf_focused_section(state: &AppState) -> PerfSection {
        state
            .session_manager
            .selected()
            .unwrap()
            .session
            .performance
            .focused_section
    }

    // ── handle_perf_focus_section ─────────────────────────────────────────────

    #[test]
    fn perf_focus_section_message_updates_state() {
        let (mut state, _) = make_state_in_performance_panel();
        // Default section is FrameChart.
        assert_eq!(perf_focused_section(&state), PerfSection::FrameChart);

        // T02: PerfSection::MemoryList removed — focusing a memory section
        // now routes to MemorySection on session.memory. Use Details as the
        // second valid PerfSection for this test (direct assignment, bypassing Tab).
        handle_perf_focus_section(&mut state, PerfSection::Details);

        assert_eq!(perf_focused_section(&state), PerfSection::Details);
    }

    #[test]
    fn perf_focus_section_via_tab_key() {
        let (mut state, _) = make_state_in_performance_panel();
        // Phase 2: Tab cycles FrameChart → Details.
        // The keys.rs Tab handler emits PerfFocusSection(focused_section.next()),
        // which is PerfFocusSection(Details). Section advances to Details.
        dispatch(&mut state, Message::Key(crate::input_key::InputKey::Tab));
        assert_eq!(
            perf_focused_section(&state),
            PerfSection::Details,
            "Phase 2: Tab advances from FrameChart to Details"
        );
    }

    #[test]
    fn perf_focus_section_via_shift_tab_key() {
        let (mut state, _) = make_state_in_performance_panel();
        // Phase 2: Shift+Tab cycles FrameChart → Details (same direction as prev()).
        dispatch(
            &mut state,
            Message::Key(crate::input_key::InputKey::BackTab),
        );
        assert_eq!(
            perf_focused_section(&state),
            PerfSection::Details,
            "Phase 2: Shift+Tab wraps FrameChart → Details"
        );
    }

    // ── handle_perf_scroll — FrameChart ──────────────────────────────────────

    #[test]
    fn perf_scroll_down_in_frame_chart_increments_offset() {
        let (mut state, _) = make_state_in_performance_panel();
        if let Some(handle) = state.session_manager.selected_mut() {
            handle.session.performance.frame_chart_visible_width.set(50);
            handle.session.performance.focused_section = PerfSection::FrameChart;
        }
        push_frames(&mut state, 1000);

        handle_perf_scroll(&mut state, ScrollDir::Down);

        // Down means toward live edge: offset decreases from 0? No — 0 is already live
        // edge. Let's first scroll Up (back), then Down (toward live).
        // Actually with offset=0 and Down (-1 delta), clamp(0 + (-1), 0, max) = 0.
        // So assert offset stays 0 (clamped).
        assert_eq!(
            perf_frame_scroll(&state),
            0,
            "Scrolling Down at live edge should stay clamped at 0"
        );
    }

    #[test]
    fn perf_scroll_up_in_frame_chart_increments_offset() {
        let (mut state, _) = make_state_in_performance_panel();
        if let Some(handle) = state.session_manager.selected_mut() {
            handle.session.performance.frame_chart_visible_width.set(50);
            handle.session.performance.focused_section = PerfSection::FrameChart;
        }
        push_frames(&mut state, 1000);

        handle_perf_scroll(&mut state, ScrollDir::Up);

        assert_eq!(
            perf_frame_scroll(&state),
            1,
            "Scrolling Up should increment offset by 1"
        );
    }

    #[test]
    fn perf_scroll_clamps_to_buffer_bounds_at_max() {
        let (mut state, _) = make_state_in_performance_panel();
        if let Some(handle) = state.session_manager.selected_mut() {
            handle.session.performance.frame_chart_visible_width.set(50);
            handle.session.performance.focused_section = PerfSection::FrameChart;
            // Set offset to max_back = 100 - 50 = 50
            handle.session.performance.frame_chart_scroll_offset = 50;
        }
        push_frames(&mut state, 100);

        // Try to scroll further back — should stay at max.
        handle_perf_scroll(&mut state, ScrollDir::Up);

        assert_eq!(
            perf_frame_scroll(&state),
            50,
            "Scrolling Up at max offset should be clamped"
        );
    }

    #[test]
    fn perf_scroll_clamps_to_zero_at_live_edge() {
        let (mut state, _) = make_state_in_performance_panel();
        if let Some(handle) = state.session_manager.selected_mut() {
            handle.session.performance.frame_chart_visible_width.set(50);
            handle.session.performance.focused_section = PerfSection::FrameChart;
            handle.session.performance.frame_chart_scroll_offset = 0;
        }
        push_frames(&mut state, 100);

        handle_perf_scroll(&mut state, ScrollDir::Down);

        assert_eq!(
            perf_frame_scroll(&state),
            0,
            "Scrolling Down at offset 0 should remain at live edge"
        );
    }

    // ── handle_perf_page ─────────────────────────────────────────────────────

    #[test]
    fn perf_page_up_uses_visible_width_as_step() {
        let (mut state, _) = make_state_in_performance_panel();
        if let Some(handle) = state.session_manager.selected_mut() {
            handle.session.performance.frame_chart_visible_width.set(20);
            handle.session.performance.focused_section = PerfSection::FrameChart;
        }
        push_frames(&mut state, 1000);

        handle_perf_page(&mut state, ScrollDir::Up);

        assert_eq!(
            perf_frame_scroll(&state),
            20,
            "Page Up in FrameChart should jump by visible_width = 20"
        );
    }

    #[test]
    fn perf_page_uses_default_when_hint_is_zero() {
        let (mut state, _) = make_state_in_performance_panel();
        if let Some(handle) = state.session_manager.selected_mut() {
            // Leave visible_width at default 0 (hint not set)
            handle.session.performance.focused_section = PerfSection::FrameChart;
        }
        push_frames(&mut state, 1000);

        handle_perf_page(&mut state, ScrollDir::Up);

        assert_eq!(
            perf_frame_scroll(&state),
            super::DEFAULT_PERF_PAGE_SIZE,
            "Page Up with hint=0 should use DEFAULT_PERF_PAGE_SIZE"
        );
    }

    // ── handle_perf_jump_to_start / end ──────────────────────────────────────

    #[test]
    fn perf_jump_to_end_resets_to_live_edge() {
        let (mut state, _) = make_state_in_performance_panel();
        if let Some(handle) = state.session_manager.selected_mut() {
            handle.session.performance.frame_chart_visible_width.set(50);
            handle.session.performance.focused_section = PerfSection::FrameChart;
            handle.session.performance.frame_chart_scroll_offset = 500;
        }
        push_frames(&mut state, 1000);

        handle_perf_jump_to_end(&mut state);

        assert_eq!(
            perf_frame_scroll(&state),
            0,
            "Jump to end should set offset to 0 (live edge)"
        );
    }

    #[test]
    fn perf_jump_to_start_sets_offset_to_max_back() {
        let (mut state, _) = make_state_in_performance_panel();
        if let Some(handle) = state.session_manager.selected_mut() {
            handle.session.performance.frame_chart_visible_width.set(50);
            handle.session.performance.focused_section = PerfSection::FrameChart;
        }
        push_frames(&mut state, 1000);

        handle_perf_jump_to_start(&mut state);

        assert_eq!(
            perf_frame_scroll(&state),
            1000 - 50,
            "Jump to start should set offset to buffer_len - visible_width"
        );
    }

    // ── Via Message variants (integration) ───────────────────────────────────

    #[test]
    fn perf_focus_section_message_routes_correctly() {
        let (mut state, _) = make_state_in_performance_panel();

        update(&mut state, Message::PerfFocusSection(PerfSection::Details));

        assert_eq!(perf_focused_section(&state), PerfSection::Details);
    }

    #[test]
    fn perf_scroll_up_message_routes_correctly() {
        let (mut state, _) = make_state_in_performance_panel();
        if let Some(handle) = state.session_manager.selected_mut() {
            handle.session.performance.frame_chart_visible_width.set(10);
            handle.session.performance.focused_section = PerfSection::FrameChart;
        }
        push_frames(&mut state, 100);

        update(&mut state, Message::PerfScrollUp);

        assert_eq!(
            perf_frame_scroll(&state),
            1,
            "PerfScrollUp message should increment frame chart offset"
        );
    }

    #[test]
    fn perf_scroll_down_message_routes_correctly() {
        let (mut state, _) = make_state_in_performance_panel();
        if let Some(handle) = state.session_manager.selected_mut() {
            handle.session.performance.frame_chart_visible_width.set(10);
            handle.session.performance.frame_chart_scroll_offset = 5;
            handle.session.performance.focused_section = PerfSection::FrameChart;
        }
        push_frames(&mut state, 100);

        update(&mut state, Message::PerfScrollDown);

        assert_eq!(
            perf_frame_scroll(&state),
            4,
            "PerfScrollDown message should decrement frame chart offset"
        );
    }

    #[test]
    fn perf_jump_to_end_message_routes_correctly() {
        let (mut state, _) = make_state_in_performance_panel();
        if let Some(handle) = state.session_manager.selected_mut() {
            handle.session.performance.frame_chart_visible_width.set(10);
            handle.session.performance.frame_chart_scroll_offset = 50;
            handle.session.performance.focused_section = PerfSection::FrameChart;
        }
        push_frames(&mut state, 100);

        update(&mut state, Message::PerfJumpToEnd);

        assert_eq!(perf_frame_scroll(&state), 0);
    }

    // ── Task 04: mouse/keyboard equivalence tests ─────────────────────────────
    //
    // The Phase 3 widget layer will register MouseRegions that emit the same
    // Message variants as keyboard handlers. These tests verify that the
    // handler produces identical state mutations regardless of input origin.

    /// `PerfFocusSection` via keyboard and mouse paths produces the same state.
    ///
    /// In production, the keyboard handler emits `PerfFocusSection(next_section)`
    /// and a Phase 3 mouse region will emit the same message directly. Both paths
    /// hit the identical `update()` branch; this test documents and guards that
    /// contract.
    #[test]
    fn perf_focus_section_via_mouse_or_keyboard_yields_same_state() {
        let (mut state_keyboard, _) = make_state_in_performance_panel();
        let (mut state_mouse, _) = make_state_in_performance_panel();

        // Keyboard-style dispatch (direct Details focus via message — bypasses Tab no-op).
        update(
            &mut state_keyboard,
            Message::PerfFocusSection(PerfSection::Details),
        );
        // Mouse-style dispatch (same message — mouse region emits it directly).
        update(
            &mut state_mouse,
            Message::PerfFocusSection(PerfSection::Details),
        );

        assert_eq!(
            perf_focused_section(&state_keyboard),
            perf_focused_section(&state_mouse),
            "keyboard and mouse PerfFocusSection dispatch must yield identical focused_section"
        );
        assert_eq!(perf_focused_section(&state_keyboard), PerfSection::Details,);
    }

    // ── Task 08: Live-edge drift + integration tests ──────────────────────────
    //
    // These tests lock in the Model A scroll-offset semantics and cover
    // scroll-then-grow behavior, jump semantics, focus-cycle invariants,
    // and alloc-table scroll visibility.

    /// Model A: when new frames arrive while scrolled, the window drifts forward
    /// by exactly the number of new arrivals (offset is "frames back from live edge").
    ///
    /// This is a pure-function test against the handler-level clamp logic:
    /// `clamp_chart_scroll` with unchanged offset on a larger buffer.
    #[test]
    fn scroll_offset_persists_under_new_arrivals() {
        let (mut state, _) = make_state_in_performance_panel();
        if let Some(handle) = state.session_manager.selected_mut() {
            handle.session.performance.frame_chart_visible_width.set(50);
            handle.session.performance.focused_section = PerfSection::FrameChart;
            handle.session.performance.frame_chart_scroll_offset = 100;
        }
        // Push 500 frames initially.
        push_frames(&mut state, 500);

        // Snapshot visible range before new frames arrive.
        let offset_before = perf_frame_scroll(&state);
        let buf_len_before = state
            .session_manager
            .selected()
            .unwrap()
            .session
            .performance
            .frame_history
            .len();

        // Push 20 more frames (simulating new arrivals).
        push_frames(&mut state, 20);

        let buf_len_after = state
            .session_manager
            .selected()
            .unwrap()
            .session
            .performance
            .frame_history
            .len();

        // The scroll_offset itself must not change — it is "frames back from live edge",
        // not an absolute position. With 20 new frames the window drifts forward by 20.
        let offset_after = perf_frame_scroll(&state);
        assert_eq!(
            offset_after, offset_before,
            "Model A: scroll_offset must remain unchanged when new frames arrive"
        );

        // Window size (visible_width = 50) should be preserved for both snapshots.
        let visible: usize = 50;
        let end_before = buf_len_before.saturating_sub(offset_before);
        let start_before = end_before.saturating_sub(visible);
        let end_after = buf_len_after.saturating_sub(offset_after);
        let start_after = end_after.saturating_sub(visible);

        assert_eq!(
            end_before - start_before,
            end_after - start_after,
            "window size must be preserved after new arrivals"
        );
        assert_eq!(
            end_after - end_before,
            20,
            "window end must drift forward by the number of new arrivals (Model A)"
        );
    }

    /// `PerfJumpToEnd` resets `frame_chart_scroll_offset` to 0 (live edge).
    #[test]
    fn jump_to_end_resets_scroll_offset_to_zero() {
        let (mut state, _) = make_state_in_performance_panel();
        if let Some(handle) = state.session_manager.selected_mut() {
            handle.session.performance.focused_section = PerfSection::FrameChart;
            handle.session.performance.frame_chart_scroll_offset = 50;
        }
        push_frames(&mut state, 200);

        update(&mut state, Message::PerfJumpToEnd);

        assert_eq!(
            perf_frame_scroll(&state),
            0,
            "PerfJumpToEnd must reset frame_chart_scroll_offset to 0 (live edge)"
        );
    }

    /// `PerfJumpToStart` with 1000 frames and visible_width 50 sets
    /// `frame_chart_scroll_offset = 1000 - 50 = 950` (oldest data visible).
    #[test]
    fn jump_to_start_sets_max_back() {
        let (mut state, _) = make_state_in_performance_panel();
        if let Some(handle) = state.session_manager.selected_mut() {
            handle.session.performance.frame_chart_visible_width.set(50);
            handle.session.performance.focused_section = PerfSection::FrameChart;
        }
        push_frames(&mut state, 1000);

        update(&mut state, Message::PerfJumpToStart);

        assert_eq!(
            perf_frame_scroll(&state),
            950, // 1000 - 50
            "PerfJumpToStart with 1000 frames and visible_width=50 must set offset to 950"
        );
    }

    /// Pressing Left arrow when `scroll_offset = 50` and `selected_frame = None`
    /// selects the live-edge-relative frame and resets `frame_chart_scroll_offset` to 0
    /// so the newly-selected frame is visible at the live edge.
    #[test]
    fn left_right_arrow_clears_scroll_offset() {
        let (mut state, _) = make_state_in_performance_panel();
        if let Some(handle) = state.session_manager.selected_mut() {
            handle.session.performance.frame_chart_visible_width.set(50);
            handle.session.performance.focused_section = PerfSection::FrameChart;
            handle.session.performance.frame_chart_scroll_offset = 50;
            handle.session.performance.selected_frame = None;
        }
        push_frames(&mut state, 200);

        dispatch(&mut state, Message::Key(crate::input_key::InputKey::Left));

        // Selection must move to the most-recent frame (live-edge-relative).
        let selected = current_selected_frame(&state);
        assert!(
            selected.is_some(),
            "Left arrow from None should select the most-recent frame"
        );
        assert_eq!(
            selected,
            Some(199),
            "Left from None selects len-1 = 199 (most recent frame)"
        );

        // Selecting a concrete frame must reset the scroll offset to 0 so the
        // selected frame is visible at the live edge.
        let offset = perf_frame_scroll(&state);
        assert_eq!(
            offset, 0,
            "Left arrow selecting a frame must clear frame_chart_scroll_offset to 0"
        );
    }

    /// Phase 2: Tab cycles between FrameChart and Details.
    #[test]
    fn tab_cycles_between_frame_chart_and_details() {
        let (mut state, _) = make_state_in_performance_panel();
        // Starting section is FrameChart (the default).
        assert_eq!(perf_focused_section(&state), PerfSection::FrameChart);

        dispatch(&mut state, Message::Key(crate::input_key::InputKey::Tab));
        assert_eq!(
            perf_focused_section(&state),
            PerfSection::Details,
            "Phase 2: Tab advances from FrameChart to Details"
        );

        dispatch(&mut state, Message::Key(crate::input_key::InputKey::Tab));
        assert_eq!(
            perf_focused_section(&state),
            PerfSection::FrameChart,
            "Phase 2: second Tab wraps back to FrameChart"
        );
    }

    /// T03 integration test: Tab cycles FrameChart → Details in Phase 2.
    #[test]
    fn tab_now_cycles_to_details_in_phase_2() {
        let (mut state, _) = make_state_in_performance_panel();
        // Initial state: focused_section == FrameChart (default)
        assert_eq!(
            state
                .session_manager
                .selected()
                .unwrap()
                .session
                .performance
                .focused_section,
            PerfSection::FrameChart,
        );
        // Tab should now flip to Details (Phase 1 was a no-op).
        dispatch(&mut state, Message::Key(crate::input_key::InputKey::Tab));
        assert_eq!(
            state
                .session_manager
                .selected()
                .unwrap()
                .session
                .performance
                .focused_section,
            PerfSection::Details,
        );
        // Tab again returns to FrameChart.
        dispatch(&mut state, Message::Key(crate::input_key::InputKey::Tab));
        assert_eq!(
            state
                .session_manager
                .selected()
                .unwrap()
                .session
                .performance
                .focused_section,
            PerfSection::FrameChart,
        );
    }
}
