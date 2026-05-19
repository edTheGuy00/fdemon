//! Performance panel — frame selection and chart scroll/page/jump handlers.
//!
//! Contains all frame-chart interactivity: frame selection by index, section
//! focus cycling, bar-chart scroll, page navigation, and jump-to-start/end.
//!
//! Phase 2 details-pane tab cycling lives in [`super::details`].
//! Memory and allocation profile handlers live in [`super::super::memory`].

use super::super::scroll_helpers::{clamp_chart_scroll, ScrollDir};
use crate::handler::{UpdateAction, UpdateResult};
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
///
/// On each call the handler also increments `frame_anchor_generation` and
/// returns a `DebounceFrameAnchor` action so the Timeline Events Gantt will
/// update its anchored viewport 200 ms after the user stops navigating.
pub(crate) fn handle_select_performance_frame(
    state: &mut AppState,
    index: Option<usize>,
) -> UpdateResult {
    let Some(handle) = state.session_manager.selected_mut() else {
        return UpdateResult::none();
    };

    let session_id = handle.session.id;

    handle.session.performance.selected_frame = index;
    // Viewport-aware scroll: only adjust scroll_offset when the newly-selected
    // frame falls outside the current viewport. Deselect (None) leaves the
    // offset unchanged — the user may have scrolled deliberately and pressed
    // Esc only to drop the selection highlight.
    if let Some(sel_idx) = index {
        let total = handle.session.performance.frame_history.len();
        // EXCEPTION: TEA render-hint write-back via Cell — see docs/REVIEW_FOCUS.md
        let visible_width = handle.session.performance.frame_chart_visible_width.get();
        let scroll = &mut handle.session.performance.frame_chart_scroll_offset;
        // Visible window: [visible_start, visible_end)
        // Model A: end = total - scroll_offset, start = end - visible_width
        let visible_end = total.saturating_sub(*scroll);
        let visible_start = visible_end.saturating_sub(visible_width);
        if sel_idx < visible_start {
            // Selection moved off the left edge — scroll left to keep it visible.
            *scroll = total.saturating_sub(sel_idx + visible_width);
        } else if sel_idx >= visible_end {
            // Selection moved off the right edge — scroll right to keep it visible.
            *scroll = total.saturating_sub(sel_idx + 1);
        }
        // Otherwise the selection is within the viewport — leave scroll_offset alone.
    }

    // Increment the monotonic generation counter so that stale ApplyFrameAnchor
    // messages from previous debounce timers are silently dropped.
    handle.session.performance.frame_anchor_generation = handle
        .session
        .performance
        .frame_anchor_generation
        .wrapping_add(1);
    let generation = handle.session.performance.frame_anchor_generation;

    // Resolve the frame number for the selected index (None when deselecting).
    let frame_number = index.and_then(|sel_idx| {
        handle
            .session
            .performance
            .frame_history
            .iter()
            .nth(sel_idx)
            .map(|f| f.number)
    });

    UpdateResult::action(UpdateAction::DebounceFrameAnchor {
        session_id,
        generation,
        frame_number,
        delay_ms: FRAME_ANCHOR_DEBOUNCE_MS,
    })
}

/// Handler invoked when the debounced `ApplyFrameAnchor` message fires.
///
/// Sets `committed_frame_anchor` only when `generation` matches the current
/// `frame_anchor_generation`; stale firings are silently dropped.
pub(crate) fn handle_apply_frame_anchor(
    state: &mut AppState,
    session_id: crate::session::SessionId,
    generation: u64,
    frame_number: Option<u64>,
) -> UpdateResult {
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        if handle.session.performance.frame_anchor_generation == generation {
            handle.session.performance.committed_frame_anchor = frame_number;
        }
        // Else: stale generation — drop silently.
    }
    UpdateResult::none()
}

/// Debounce delay for the frame-anchor commit (milliseconds).
///
/// 200 ms is long enough that rapid Left/Right navigation doesn't trigger
/// layout fetches for every intermediate frame, but short enough that it
/// feels responsive after a deliberate selection.
const FRAME_ANCHOR_DEBOUNCE_MS: u64 = 200;

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
            let visible_width = handle.session.performance.frame_chart_visible_width.get();
            let visible = if visible_width == 0 {
                DEFAULT_PERF_PAGE_SIZE
            } else {
                visible_width
            };
            handle.session.performance.frame_chart_scroll_offset = buf_len.saturating_sub(visible);
        }
        PerfSection::Details => {
            // No-op for scroll/jump. Details pane content (Phase 3 Rebuild Stats /
            // Timeline Events) will own its own scroll handlers.
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
            // No-op for scroll/jump. Details pane content (Phase 3 Rebuild Stats /
            // Timeline Events) will own its own scroll handlers.
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

    // ── Phase 5: frame anchor pipeline tests ────────────────────────────────

    #[test]
    fn frame_anchor_generation_increments_on_select() {
        let (mut state, _) = make_state_in_performance_panel();
        push_frames(&mut state, 5);

        let gen_before = state
            .session_manager
            .selected()
            .unwrap()
            .session
            .performance
            .frame_anchor_generation;

        update(
            &mut state,
            Message::SelectPerformanceFrame { index: Some(2) },
        );

        let gen_after = state
            .session_manager
            .selected()
            .unwrap()
            .session
            .performance
            .frame_anchor_generation;

        assert_eq!(
            gen_after,
            gen_before + 1,
            "frame_anchor_generation must increment by 1 on each selection"
        );
    }

    #[test]
    fn apply_frame_anchor_with_stale_generation_is_dropped() {
        let (mut state, session_id) = make_state_in_performance_panel();
        push_frames(&mut state, 5);

        // Advance the generation to 2 by selecting twice.
        update(
            &mut state,
            Message::SelectPerformanceFrame { index: Some(1) },
        );
        update(
            &mut state,
            Message::SelectPerformanceFrame { index: Some(2) },
        );

        let current_gen = state
            .session_manager
            .selected()
            .unwrap()
            .session
            .performance
            .frame_anchor_generation;
        assert_eq!(current_gen, 2);

        // Apply with stale generation 1 — must be ignored.
        update(
            &mut state,
            Message::ApplyFrameAnchor {
                session_id,
                generation: 1,
                frame_number: Some(99),
            },
        );

        let anchor = state
            .session_manager
            .selected()
            .unwrap()
            .session
            .performance
            .committed_frame_anchor;
        assert_eq!(
            anchor, None,
            "stale ApplyFrameAnchor must not change committed_frame_anchor"
        );
    }

    #[test]
    fn apply_frame_anchor_with_current_generation_commits() {
        let (mut state, session_id) = make_state_in_performance_panel();
        push_frames(&mut state, 5);

        update(
            &mut state,
            Message::SelectPerformanceFrame { index: Some(3) },
        );

        let current_gen = state
            .session_manager
            .selected()
            .unwrap()
            .session
            .performance
            .frame_anchor_generation;

        // Frame at index 3 has number = 4 (1-based in push_test_frames).
        update(
            &mut state,
            Message::ApplyFrameAnchor {
                session_id,
                generation: current_gen,
                frame_number: Some(4),
            },
        );

        let anchor = state
            .session_manager
            .selected()
            .unwrap()
            .session
            .performance
            .committed_frame_anchor;
        assert_eq!(
            anchor,
            Some(4),
            "current-generation ApplyFrameAnchor must commit frame_number"
        );
    }

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

    /// Regression: `handle_perf_page` and `handle_perf_jump_to_start` must produce
    /// the same scroll offset for a pre-first-render keypress (visible_width == 0).
    ///
    /// Before m13 was fixed, `handle_perf_jump_to_start` used `.max(1)` while
    /// `handle_perf_page` used `DEFAULT_PERF_PAGE_SIZE`. This caused inconsistent
    /// behaviour at startup before the first render sets the visible-width hint.
    #[test]
    fn perf_jump_to_start_and_page_use_same_fallback_when_hint_is_zero() {
        let (mut state_page, _) = make_state_in_performance_panel();
        let (mut state_jump, _) = make_state_in_performance_panel();

        // Push the same frames into both states. Leave visible_width at 0 (not yet rendered).
        push_frames(&mut state_page, 1000);
        push_frames(&mut state_jump, 1000);

        // Page Up with hint=0 uses DEFAULT_PERF_PAGE_SIZE = 10.
        handle_perf_page(&mut state_page, ScrollDir::Up);
        let page_offset = perf_frame_scroll(&state_page);

        // Jump-to-start with hint=0 should also use DEFAULT_PERF_PAGE_SIZE.
        handle_perf_jump_to_start(&mut state_jump);
        let jump_offset = perf_frame_scroll(&state_jump);

        // Page Up from 0 moves 10 back; jump-to-start with visible=10 sets
        // buf_len - visible = 1000 - 10 = 990.
        // Both results should use DEFAULT_PERF_PAGE_SIZE (10) as their visible size.
        assert_eq!(
            page_offset,
            super::DEFAULT_PERF_PAGE_SIZE,
            "page_up with hint=0 should offset by DEFAULT_PERF_PAGE_SIZE"
        );
        assert_eq!(
            jump_offset,
            1000 - super::DEFAULT_PERF_PAGE_SIZE,
            "jump_to_start with hint=0 should use DEFAULT_PERF_PAGE_SIZE as visible size"
        );
    }

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
    /// selects the most-recent frame (index 199). The new selection is outside
    /// the current viewport ([100, 150)), so the viewport-aware handler adjusts
    /// `frame_chart_scroll_offset` to bring index 199 into view (offset → 0,
    /// i.e. the live edge).
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

        // Index 199 is right of the old viewport [100, 150) so the handler
        // viewport-scrolls right: offset = 200 - (199 + 1) = 0 (live edge).
        let offset = perf_frame_scroll(&state);
        assert_eq!(
            offset, 0,
            "selecting index 199 from offset=50 should viewport-scroll to live edge (offset=0)"
        );
    }

    // ── Task 01, Fix 3: viewport-aware selection scrolling ───────────────────
    //
    // Setup: frames=200, visible_width=30, scroll_offset=70
    //   visible_end   = 200 - 70 = 130
    //   visible_start = 130 - 30 = 100
    // Visible range: indices [100, 130).

    /// Selecting a frame that is within the current viewport must leave
    /// `frame_chart_scroll_offset` unchanged.
    ///
    /// Start: offset=70, visible=[100,130). Select index 130 (task says
    /// selected_frame=130 with this setup is at the right edge, within view).
    /// Wait — task AC says "selected_frame = 130" is visible_end, i.e. the
    /// frame at the boundary. Pressing Left → 129 → inside [100, 130) → no scroll.
    #[test]
    fn test_select_within_viewport_does_not_scroll() {
        let (mut state, _) = make_state_in_performance_panel();
        if let Some(handle) = state.session_manager.selected_mut() {
            // visible_width=30, scroll_offset=70 → visible=[100,130)
            handle.session.performance.frame_chart_visible_width.set(30);
            handle.session.performance.frame_chart_scroll_offset = 70;
            // Pre-select frame 130 (right edge of viewport, but visible_end = 130,
            // so frame 130 is actually OUTSIDE [100,130). Use 129 instead —
            // pressing Left from 130 → 129 which is inside the viewport.
            handle.session.performance.selected_frame = Some(130);
        }
        push_frames(&mut state, 200);

        // Set the selection directly to 129 (within [100, 130)) via the message.
        use crate::handler::update::update;
        update(
            &mut state,
            Message::SelectPerformanceFrame { index: Some(129) },
        );

        assert_eq!(
            perf_frame_scroll(&state),
            70,
            "selecting frame 129 (within viewport [100,130)) must leave scroll_offset at 70"
        );
    }

    /// Selecting a frame just off the left edge of the viewport must scroll
    /// the viewport left by one so the frame becomes the leftmost visible item.
    ///
    /// Start: offset=70, visible=[100,130). Selecting index 99 (one below
    /// visible_start=100) should set offset = 200 - (99 + 30) = 71.
    #[test]
    fn test_select_at_left_edge_scrolls_viewport_left() {
        let (mut state, _) = make_state_in_performance_panel();
        if let Some(handle) = state.session_manager.selected_mut() {
            handle.session.performance.frame_chart_visible_width.set(30);
            handle.session.performance.frame_chart_scroll_offset = 70;
            handle.session.performance.selected_frame = Some(100); // leftmost visible
        }
        push_frames(&mut state, 200);

        use crate::handler::update::update;
        // Select index 99 — one past the left edge of [100, 130)
        update(
            &mut state,
            Message::SelectPerformanceFrame { index: Some(99) },
        );

        // offset = total - (sel_idx + visible_width) = 200 - (99 + 30) = 71
        assert_eq!(
            perf_frame_scroll(&state),
            71,
            "selecting frame 99 (left of viewport [100,130)) must scroll left: offset = 71"
        );
    }

    /// Selecting a frame just off the right edge of the viewport must scroll
    /// the viewport right by one so the frame becomes the rightmost visible item.
    ///
    /// Start: offset=70, visible=[100,130). Selecting index 130 (equal to
    /// visible_end=130, i.e. just past the right edge) should set
    /// offset = 200 - (130 + 1) = 69.
    #[test]
    fn test_select_at_right_edge_scrolls_viewport_right() {
        let (mut state, _) = make_state_in_performance_panel();
        if let Some(handle) = state.session_manager.selected_mut() {
            handle.session.performance.frame_chart_visible_width.set(30);
            handle.session.performance.frame_chart_scroll_offset = 70;
            handle.session.performance.selected_frame = Some(129); // rightmost visible
        }
        push_frames(&mut state, 200);

        use crate::handler::update::update;
        // Select index 130 — one past the right edge (visible_end = 130)
        update(
            &mut state,
            Message::SelectPerformanceFrame { index: Some(130) },
        );

        // offset = total - (sel_idx + 1) = 200 - 131 = 69
        assert_eq!(
            perf_frame_scroll(&state),
            69,
            "selecting frame 130 (right of viewport [100,130)) must scroll right: offset = 69"
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
