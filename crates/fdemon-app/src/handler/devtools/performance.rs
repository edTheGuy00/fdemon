//! Performance panel handlers.
//!
//! Handles frame selection, allocation profile updates, and rich memory samples
//! for the Performance panel's bar chart and time-series views, plus the
//! Phase 2 keyboard interactivity handlers (section focus, scroll, page, jump,
//! alloc row selection).

use crate::handler::UpdateResult;
use crate::session::performance::PerfSection;
use crate::session::AllocationSortColumn;
use crate::session::SessionId;
use crate::state::AppState;
use fdemon_core::performance::{AllocationProfile, MemorySample};

// ── Phase 2 scroll helpers ────────────────────────────────────────────────────

/// Fallback page size when the render-hint visible dimension is 0 (not yet rendered).
const DEFAULT_PERF_PAGE_SIZE: usize = 10;

/// Scroll direction used by [`handle_perf_scroll`] and [`handle_perf_page`].
pub(crate) enum ScrollDir {
    Up,
    Down,
}

/// Clamp a chart scroll offset.
///
/// `buffer_len` is the number of items in the chart's data buffer.
/// `visible_width` is the number of items visible at once (render hint; 0 = use 1).
/// `current` is the current scroll offset (0 = live edge, higher = more scrolled back).
/// `delta` is the signed change (+1 scrolls back, -1 scrolls toward live edge).
///
/// Returns the new offset clamped to `[0, buffer_len.saturating_sub(visible_width.max(1))]`.
fn clamp_chart_scroll(
    buffer_len: usize,
    visible_width: usize,
    current: usize,
    delta: i64,
) -> usize {
    let max_back = buffer_len.saturating_sub(visible_width.max(1));
    let new = current as i64 + delta;
    new.clamp(0, max_back as i64) as usize
}

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
    }
    UpdateResult::none()
}

/// Handle rich memory sample received from the VM service.
///
/// Pushes the sample into `PerformanceState::memory_samples` for the session
/// identified by `session_id`. No-op if the session does not exist.
pub(crate) fn handle_memory_sample_received(
    state: &mut AppState,
    session_id: SessionId,
    sample: MemorySample,
) -> UpdateResult {
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        handle.session.performance.memory_samples.push(sample);
    }
    UpdateResult::none()
}

/// Handle allocation profile snapshot received from the VM service.
///
/// Replaces `PerformanceState::allocation_profile` with the new snapshot for
/// the session identified by `session_id`. Only the most recent profile is
/// retained in state. No-op if the session does not exist.
pub(crate) fn handle_allocation_profile_received(
    state: &mut AppState,
    session_id: SessionId,
    profile: AllocationProfile,
) -> UpdateResult {
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        tracing::debug!(
            "Allocation profile received for session {}: {} classes",
            session_id,
            profile.members.len(),
        );
        handle.session.performance.allocation_profile = Some(profile);
    }
    UpdateResult::none()
}

/// Toggle the allocation table sort between [`AllocationSortColumn::BySize`]
/// and [`AllocationSortColumn::ByInstances`].
///
/// No-op when no session is selected.
pub(crate) fn handle_toggle_allocation_sort(state: &mut AppState) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        handle.session.performance.allocation_sort =
            match handle.session.performance.allocation_sort {
                AllocationSortColumn::BySize => AllocationSortColumn::ByInstances,
                AllocationSortColumn::ByInstances => AllocationSortColumn::BySize,
            };
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
/// - `FrameChart` — adjusts `frame_chart_scroll_offset`, clamped to the frame history.
/// - `MemoryChart` — adjusts `memory_chart_scroll_offset`, clamped to the memory samples.
/// - `MemoryList` — moves `alloc_table_selected_row`; nudges `alloc_table_scroll_offset`
///   to keep the selection visible (using the `alloc_table_visible_height` render hint).
///
/// No-op when no session is selected.
pub(crate) fn handle_perf_scroll(state: &mut AppState, direction: ScrollDir) -> UpdateResult {
    let Some(handle) = state.session_manager.selected_mut() else {
        return UpdateResult::none();
    };
    let perf = &mut handle.session.performance;

    match perf.focused_section {
        PerfSection::FrameChart => {
            let buf_len = perf.frame_history.len();
            let visible = perf.frame_chart_visible_width.get();
            // Up = scroll back (higher offset), Down = scroll toward live edge (lower offset).
            let delta: i64 = match direction {
                ScrollDir::Up => 1,
                ScrollDir::Down => -1,
            };
            perf.frame_chart_scroll_offset =
                clamp_chart_scroll(buf_len, visible, perf.frame_chart_scroll_offset, delta);
        }
        PerfSection::MemoryChart => {
            let buf_len = perf.memory_samples.len();
            let visible = perf.memory_chart_visible_width.get();
            let delta: i64 = match direction {
                ScrollDir::Up => 1,
                ScrollDir::Down => -1,
            };
            perf.memory_chart_scroll_offset =
                clamp_chart_scroll(buf_len, visible, perf.memory_chart_scroll_offset, delta);
        }
        PerfSection::MemoryList => {
            scroll_alloc_table(perf, direction, 1);
        }
    }

    UpdateResult::none()
}

/// Scroll the focused Performance panel section by one page in `direction`.
///
/// Page size is taken from the appropriate render hint (`frame_chart_visible_width`,
/// `memory_chart_visible_width`, or `alloc_table_visible_height`); falls back to
/// [`DEFAULT_PERF_PAGE_SIZE`] when the hint is 0 (not yet rendered).
///
/// No-op when no session is selected.
pub(crate) fn handle_perf_page(state: &mut AppState, direction: ScrollDir) -> UpdateResult {
    let Some(handle) = state.session_manager.selected_mut() else {
        return UpdateResult::none();
    };
    let perf = &mut handle.session.performance;

    match perf.focused_section {
        PerfSection::FrameChart => {
            let visible = perf.frame_chart_visible_width.get();
            let page = if visible == 0 {
                DEFAULT_PERF_PAGE_SIZE
            } else {
                visible
            } as i64;
            let buf_len = perf.frame_history.len();
            let delta: i64 = match direction {
                ScrollDir::Up => page,
                ScrollDir::Down => -page,
            };
            perf.frame_chart_scroll_offset =
                clamp_chart_scroll(buf_len, visible, perf.frame_chart_scroll_offset, delta);
        }
        PerfSection::MemoryChart => {
            let visible = perf.memory_chart_visible_width.get();
            let page = if visible == 0 {
                DEFAULT_PERF_PAGE_SIZE
            } else {
                visible
            } as i64;
            let buf_len = perf.memory_samples.len();
            let delta: i64 = match direction {
                ScrollDir::Up => page,
                ScrollDir::Down => -page,
            };
            perf.memory_chart_scroll_offset =
                clamp_chart_scroll(buf_len, visible, perf.memory_chart_scroll_offset, delta);
        }
        PerfSection::MemoryList => {
            let page = {
                let h = perf.alloc_table_visible_height.get();
                if h == 0 {
                    DEFAULT_PERF_PAGE_SIZE
                } else {
                    h
                }
            };
            scroll_alloc_table(perf, direction, page);
        }
    }

    UpdateResult::none()
}

/// Jump to the furthest-back position in the focused section (oldest data / first row).
///
/// - `FrameChart` / `MemoryChart`: set scroll offset to `max_back` (oldest data visible).
/// - `MemoryList`: set selection to row 0, scroll offset to 0.
///
/// No-op when no session is selected.
pub(crate) fn handle_perf_jump_to_start(state: &mut AppState) -> UpdateResult {
    let Some(handle) = state.session_manager.selected_mut() else {
        return UpdateResult::none();
    };
    let perf = &mut handle.session.performance;

    match perf.focused_section {
        PerfSection::FrameChart => {
            let buf_len = perf.frame_history.len();
            let visible = perf.frame_chart_visible_width.get().max(1);
            perf.frame_chart_scroll_offset = buf_len.saturating_sub(visible);
        }
        PerfSection::MemoryChart => {
            let buf_len = perf.memory_samples.len();
            let visible = perf.memory_chart_visible_width.get().max(1);
            perf.memory_chart_scroll_offset = buf_len.saturating_sub(visible);
        }
        PerfSection::MemoryList => {
            // "Start" for a list means the first row (index 0).
            let row_count = alloc_row_count(perf);
            if row_count > 0 {
                perf.alloc_table_selected_row = Some(0);
            } else {
                perf.alloc_table_selected_row = None;
            }
            perf.alloc_table_scroll_offset = 0;
        }
    }

    UpdateResult::none()
}

/// Jump to the live edge in the focused section (newest data / last row).
///
/// - `FrameChart` / `MemoryChart`: set scroll offset to 0 (live edge).
/// - `MemoryList`: set selection to the last row.
///
/// No-op when no session is selected.
pub(crate) fn handle_perf_jump_to_end(state: &mut AppState) -> UpdateResult {
    let Some(handle) = state.session_manager.selected_mut() else {
        return UpdateResult::none();
    };
    let perf = &mut handle.session.performance;

    match perf.focused_section {
        PerfSection::FrameChart => {
            perf.frame_chart_scroll_offset = 0;
        }
        PerfSection::MemoryChart => {
            perf.memory_chart_scroll_offset = 0;
        }
        PerfSection::MemoryList => {
            let row_count = alloc_row_count(perf);
            if row_count > 0 {
                perf.alloc_table_selected_row = Some(row_count - 1);
                // Scroll to show the last row.
                let visible = {
                    let h = perf.alloc_table_visible_height.get();
                    if h == 0 {
                        DEFAULT_PERF_PAGE_SIZE
                    } else {
                        h
                    }
                };
                perf.alloc_table_scroll_offset = (row_count).saturating_sub(visible);
            } else {
                perf.alloc_table_selected_row = None;
            }
        }
    }

    UpdateResult::none()
}

/// Select a row in the allocation table by index, or clear the selection when `index` is `None`.
///
/// When `index` is `Some(_)`, also sets `focused_section = MemoryList` so the panel focus
/// follows the selection (used for both keyboard Enter and mouse click on a row).
///
/// When `index` is `None` (click outside a row, or explicit clear), only clears the
/// selection; focus is intentionally left unchanged so the user's current section
/// focus is not disturbed.
///
/// No-op when no session is selected.
pub(crate) fn handle_perf_select_alloc_row(
    state: &mut AppState,
    index: Option<usize>,
) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        handle.session.performance.alloc_table_selected_row = index;
        // Only pull focus to MemoryList when a concrete row is selected.
        // Clearing the selection (index = None) must not disturb the current
        // focused_section — a mouse click outside any row should not jump focus.
        if index.is_some() {
            handle.session.performance.focused_section = PerfSection::MemoryList;
        }
    }
    UpdateResult::none()
}

// ── Private helpers ───────────────────────────────────────────────────────────

/// Return the number of rows in the allocation table for the given `PerformanceState`.
///
/// Returns `profile.members.len()` when an allocation profile is available,
/// otherwise 0.
fn alloc_row_count(perf: &crate::session::performance::PerformanceState) -> usize {
    perf.allocation_profile
        .as_ref()
        .map(|p| p.members.len())
        .unwrap_or(0)
}

/// Scroll the allocation table selection by `steps` rows in `direction`,
/// adjusting the scroll offset to keep the selection visible.
///
/// Used by both `handle_perf_scroll` (steps = 1) and `handle_perf_page` (steps = page_size).
fn scroll_alloc_table(
    perf: &mut crate::session::performance::PerformanceState,
    direction: ScrollDir,
    steps: usize,
) {
    let row_count = alloc_row_count(perf);
    if row_count == 0 {
        return;
    }

    let current_row = perf.alloc_table_selected_row.unwrap_or(0);

    let new_row = match direction {
        ScrollDir::Up => current_row.saturating_sub(steps),
        ScrollDir::Down => (current_row + steps).min(row_count.saturating_sub(1)),
    };

    perf.alloc_table_selected_row = Some(new_row);

    // Nudge scroll offset to keep new_row within the visible window.
    let visible_height = {
        let h = perf.alloc_table_visible_height.get();
        if h == 0 {
            DEFAULT_PERF_PAGE_SIZE
        } else {
            h
        }
    };

    // Scroll forward if selection moved past the bottom of the visible window.
    if new_row >= perf.alloc_table_scroll_offset + visible_height {
        perf.alloc_table_scroll_offset = new_row.saturating_sub(visible_height - 1);
    }
    // Scroll back if selection moved above the top of the visible window.
    if new_row < perf.alloc_table_scroll_offset {
        perf.alloc_table_scroll_offset = new_row;
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::handle_toggle_allocation_sort;
    use crate::handler::update::update;
    use crate::message::Message;
    use crate::session::AllocationSortColumn;
    use crate::session::SessionId;
    use crate::state::{AppState, DevToolsPanel, UiMode};
    use fdemon_core::performance::{AllocationProfile, FrameTiming, MemorySample};

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

    fn make_memory_sample() -> MemorySample {
        MemorySample {
            dart_heap: 10_000_000,
            dart_native: 2_000_000,
            raster_cache: 1_000_000,
            allocated: 20_000_000,
            rss: 50_000_000,
            timestamp: chrono::Local::now(),
        }
    }

    fn make_allocation_profile() -> AllocationProfile {
        AllocationProfile {
            members: vec![],
            timestamp: chrono::Local::now(),
        }
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

    // ── VmServiceMemorySample message ─────────────────────────────────────────

    #[test]
    fn test_memory_sample_received_pushes_to_buffer() {
        let (mut state, session_id) = make_state_in_performance_panel();
        let sample = make_memory_sample();

        update(
            &mut state,
            Message::VmServiceMemorySample { session_id, sample },
        );

        let count = state
            .session_manager
            .selected()
            .unwrap()
            .session
            .performance
            .memory_samples
            .len();
        assert_eq!(count, 1, "One sample should be in the ring buffer");
    }

    #[test]
    fn test_memory_sample_received_multiple_samples_accumulate() {
        let (mut state, session_id) = make_state_in_performance_panel();

        for _ in 0..3 {
            update(
                &mut state,
                Message::VmServiceMemorySample {
                    session_id,
                    sample: make_memory_sample(),
                },
            );
        }

        let count = state
            .session_manager
            .selected()
            .unwrap()
            .session
            .performance
            .memory_samples
            .len();
        assert_eq!(
            count, 3,
            "Three samples should accumulate in the ring buffer"
        );
    }

    #[test]
    fn test_memory_sample_unknown_session_is_noop() {
        let (mut state, _) = make_state_in_performance_panel();
        let unknown_session_id: SessionId = 999_999;
        let sample = make_memory_sample();

        // Should not panic or change any state.
        update(
            &mut state,
            Message::VmServiceMemorySample {
                session_id: unknown_session_id,
                sample,
            },
        );
        // No assertions needed beyond "did not panic".
    }

    // ── VmServiceAllocationProfileReceived message ────────────────────────────

    #[test]
    fn test_allocation_profile_received_stores_profile() {
        let (mut state, session_id) = make_state_in_performance_panel();
        let profile = make_allocation_profile();

        update(
            &mut state,
            Message::VmServiceAllocationProfileReceived {
                session_id,
                profile,
            },
        );

        assert!(
            state
                .session_manager
                .selected()
                .unwrap()
                .session
                .performance
                .allocation_profile
                .is_some(),
            "allocation_profile should be set after receiving profile"
        );
    }

    #[test]
    fn test_allocation_profile_replaces_previous() {
        use fdemon_core::performance::ClassHeapStats;

        let (mut state, session_id) = make_state_in_performance_panel();

        // Store first profile.
        let profile1 = AllocationProfile {
            members: vec![ClassHeapStats {
                class_name: "String".to_string(),
                library_uri: None,
                new_space_instances: 10,
                new_space_size: 100,
                old_space_instances: 5,
                old_space_size: 50,
            }],
            timestamp: chrono::Local::now(),
        };
        update(
            &mut state,
            Message::VmServiceAllocationProfileReceived {
                session_id,
                profile: profile1,
            },
        );

        // Store second profile (empty members).
        let profile2 = AllocationProfile {
            members: vec![],
            timestamp: chrono::Local::now(),
        };
        update(
            &mut state,
            Message::VmServiceAllocationProfileReceived {
                session_id,
                profile: profile2,
            },
        );

        let stored = state
            .session_manager
            .selected()
            .unwrap()
            .session
            .performance
            .allocation_profile
            .as_ref()
            .unwrap();
        assert!(
            stored.members.is_empty(),
            "Second profile should replace the first; members should be empty"
        );
    }

    #[test]
    fn test_allocation_profile_unknown_session_is_noop() {
        let (mut state, _) = make_state_in_performance_panel();
        let unknown_session_id: SessionId = 999_999;
        let profile = make_allocation_profile();

        // Should not panic or change any state.
        update(
            &mut state,
            Message::VmServiceAllocationProfileReceived {
                session_id: unknown_session_id,
                profile,
            },
        );
    }

    // ── ToggleAllocationSort handler ──────────────────────────────────────────

    #[test]
    fn test_toggle_allocation_sort_size_to_instances() {
        let (mut state, _) = make_state_in_performance_panel();
        // Default is BySize.
        assert_eq!(
            state
                .session_manager
                .selected()
                .unwrap()
                .session
                .performance
                .allocation_sort,
            AllocationSortColumn::BySize
        );

        handle_toggle_allocation_sort(&mut state);

        assert_eq!(
            state
                .session_manager
                .selected()
                .unwrap()
                .session
                .performance
                .allocation_sort,
            AllocationSortColumn::ByInstances,
            "Toggle from BySize should produce ByInstances"
        );
    }

    #[test]
    fn test_toggle_allocation_sort_instances_to_size() {
        let (mut state, _) = make_state_in_performance_panel();
        // Set to ByInstances first.
        state
            .session_manager
            .selected_mut()
            .unwrap()
            .session
            .performance
            .allocation_sort = AllocationSortColumn::ByInstances;

        handle_toggle_allocation_sort(&mut state);

        assert_eq!(
            state
                .session_manager
                .selected()
                .unwrap()
                .session
                .performance
                .allocation_sort,
            AllocationSortColumn::BySize,
            "Toggle from ByInstances should produce BySize"
        );
    }

    #[test]
    fn test_toggle_allocation_sort_no_session_is_noop() {
        // State with no sessions: toggle should not panic.
        let mut state = AppState::new();
        // Should not panic.
        handle_toggle_allocation_sort(&mut state);
    }

    #[test]
    fn test_toggle_allocation_sort_via_message() {
        let (mut state, _) = make_state_in_performance_panel();

        update(&mut state, Message::ToggleAllocationSort);

        assert_eq!(
            state
                .session_manager
                .selected()
                .unwrap()
                .session
                .performance
                .allocation_sort,
            AllocationSortColumn::ByInstances,
            "ToggleAllocationSort message should toggle from BySize to ByInstances"
        );
    }

    // ── Phase 2 keyboard interactivity tests ─────────────────────────────────

    use super::{
        handle_perf_focus_section, handle_perf_jump_to_end, handle_perf_jump_to_start,
        handle_perf_page, handle_perf_scroll, handle_perf_select_alloc_row, ScrollDir,
    };
    use crate::session::performance::PerfSection;
    use fdemon_core::performance::ClassHeapStats;

    /// Push `count` synthetic memory samples into the current session.
    fn push_memory_samples(state: &mut AppState, count: usize) {
        if let Some(handle) = state.session_manager.selected_mut() {
            for _ in 0..count {
                handle
                    .session
                    .performance
                    .memory_samples
                    .push(MemorySample {
                        dart_heap: 1_000_000,
                        dart_native: 100_000,
                        raster_cache: 50_000,
                        allocated: 2_000_000,
                        rss: 5_000_000,
                        timestamp: chrono::Local::now(),
                    });
            }
        }
    }

    /// Install an allocation profile with `count` dummy class rows.
    fn push_alloc_rows(state: &mut AppState, count: usize) {
        if let Some(handle) = state.session_manager.selected_mut() {
            let members = (0..count)
                .map(|i| ClassHeapStats {
                    class_name: format!("Class{}", i),
                    library_uri: None,
                    new_space_instances: i as u64,
                    new_space_size: i as u64 * 100,
                    old_space_instances: 0,
                    old_space_size: 0,
                })
                .collect();
            handle.session.performance.allocation_profile = Some(AllocationProfile {
                members,
                timestamp: chrono::Local::now(),
            });
        }
    }

    fn perf_frame_scroll(state: &AppState) -> usize {
        state
            .session_manager
            .selected()
            .unwrap()
            .session
            .performance
            .frame_chart_scroll_offset
    }

    fn perf_memory_scroll(state: &AppState) -> usize {
        state
            .session_manager
            .selected()
            .unwrap()
            .session
            .performance
            .memory_chart_scroll_offset
    }

    fn perf_alloc_row(state: &AppState) -> Option<usize> {
        state
            .session_manager
            .selected()
            .unwrap()
            .session
            .performance
            .alloc_table_selected_row
    }

    fn perf_alloc_scroll(state: &AppState) -> usize {
        state
            .session_manager
            .selected()
            .unwrap()
            .session
            .performance
            .alloc_table_scroll_offset
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

        handle_perf_focus_section(&mut state, PerfSection::MemoryList);

        assert_eq!(perf_focused_section(&state), PerfSection::MemoryList);
    }

    #[test]
    fn perf_focus_section_via_tab_key() {
        let (mut state, _) = make_state_in_performance_panel();
        // Tab = PerfFocusSection(focused_section.next())
        // Default section FrameChart.next() == MemoryChart
        dispatch(&mut state, Message::Key(crate::input_key::InputKey::Tab));
        // The keys.rs Tab handler now routes to PerfFocusSection in the
        // performance guard; assert the section changed.
        assert_eq!(perf_focused_section(&state), PerfSection::MemoryChart);
    }

    #[test]
    fn perf_focus_section_via_shift_tab_key() {
        let (mut state, _) = make_state_in_performance_panel();
        // Shift+Tab = PerfFocusSection(focused_section.prev())
        // Default section FrameChart.prev() == MemoryList
        dispatch(
            &mut state,
            Message::Key(crate::input_key::InputKey::BackTab),
        );
        assert_eq!(perf_focused_section(&state), PerfSection::MemoryList);
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

    // ── handle_perf_scroll — MemoryChart ─────────────────────────────────────

    #[test]
    fn perf_scroll_up_in_memory_chart_increments_offset() {
        let (mut state, _) = make_state_in_performance_panel();
        if let Some(handle) = state.session_manager.selected_mut() {
            handle
                .session
                .performance
                .memory_chart_visible_width
                .set(10);
            handle.session.performance.focused_section = PerfSection::MemoryChart;
        }
        push_memory_samples(&mut state, 100);

        handle_perf_scroll(&mut state, ScrollDir::Up);

        assert_eq!(perf_memory_scroll(&state), 1);
    }

    #[test]
    fn perf_scroll_memory_chart_clamps_at_zero() {
        let (mut state, _) = make_state_in_performance_panel();
        if let Some(handle) = state.session_manager.selected_mut() {
            handle
                .session
                .performance
                .memory_chart_visible_width
                .set(10);
            handle.session.performance.focused_section = PerfSection::MemoryChart;
        }
        push_memory_samples(&mut state, 100);

        handle_perf_scroll(&mut state, ScrollDir::Down);

        assert_eq!(perf_memory_scroll(&state), 0, "Down at 0 must clamp");
    }

    // ── handle_perf_scroll — MemoryList ──────────────────────────────────────

    #[test]
    fn perf_scroll_down_in_alloc_list_moves_selection() {
        let (mut state, _) = make_state_in_performance_panel();
        if let Some(handle) = state.session_manager.selected_mut() {
            handle.session.performance.focused_section = PerfSection::MemoryList;
        }
        push_alloc_rows(&mut state, 20);

        handle_perf_scroll(&mut state, ScrollDir::Down);

        assert_eq!(
            perf_alloc_row(&state),
            Some(1),
            "Scrolling Down from row 0 should select row 1"
        );
    }

    #[test]
    fn perf_scroll_up_in_alloc_list_clamps_at_zero() {
        let (mut state, _) = make_state_in_performance_panel();
        if let Some(handle) = state.session_manager.selected_mut() {
            handle.session.performance.focused_section = PerfSection::MemoryList;
        }
        push_alloc_rows(&mut state, 20);

        handle_perf_scroll(&mut state, ScrollDir::Up);

        assert_eq!(
            perf_alloc_row(&state),
            Some(0),
            "Scrolling Up from row 0 should stay at 0"
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

    #[test]
    fn perf_page_down_moves_alloc_list_by_visible_height() {
        let (mut state, _) = make_state_in_performance_panel();
        if let Some(handle) = state.session_manager.selected_mut() {
            handle.session.performance.alloc_table_visible_height.set(5);
            handle.session.performance.focused_section = PerfSection::MemoryList;
        }
        push_alloc_rows(&mut state, 50);
        // Set initial selection to row 10.
        if let Some(handle) = state.session_manager.selected_mut() {
            handle.session.performance.alloc_table_selected_row = Some(10);
        }

        handle_perf_page(&mut state, ScrollDir::Down);

        // Down in a list moves toward higher indices: 10 + 5 = 15.
        assert_eq!(
            perf_alloc_row(&state),
            Some(15),
            "Page Down on MemoryList from row 10 with page=5 should land on row 15"
        );
    }

    #[test]
    fn perf_page_up_moves_alloc_list_back_by_visible_height() {
        let (mut state, _) = make_state_in_performance_panel();
        if let Some(handle) = state.session_manager.selected_mut() {
            handle.session.performance.alloc_table_visible_height.set(5);
            handle.session.performance.focused_section = PerfSection::MemoryList;
        }
        push_alloc_rows(&mut state, 50);
        // Set initial selection to row 10.
        if let Some(handle) = state.session_manager.selected_mut() {
            handle.session.performance.alloc_table_selected_row = Some(10);
        }

        handle_perf_page(&mut state, ScrollDir::Up);

        // Up in a list moves toward lower indices: 10 - 5 = 5.
        assert_eq!(
            perf_alloc_row(&state),
            Some(5),
            "Page Up on MemoryList from row 10 with page=5 should land on row 5"
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

    #[test]
    fn perf_jump_to_start_in_alloc_list_selects_row_zero() {
        let (mut state, _) = make_state_in_performance_panel();
        if let Some(handle) = state.session_manager.selected_mut() {
            handle.session.performance.focused_section = PerfSection::MemoryList;
            handle.session.performance.alloc_table_selected_row = Some(19);
            handle.session.performance.alloc_table_scroll_offset = 10;
        }
        push_alloc_rows(&mut state, 20);

        handle_perf_jump_to_start(&mut state);

        assert_eq!(
            perf_alloc_row(&state),
            Some(0),
            "Jump to start in MemoryList should select row 0"
        );
        assert_eq!(
            perf_alloc_scroll(&state),
            0,
            "Jump to start in MemoryList should reset scroll offset to 0"
        );
    }

    #[test]
    fn perf_jump_to_end_in_alloc_list_selects_last_row() {
        let (mut state, _) = make_state_in_performance_panel();
        if let Some(handle) = state.session_manager.selected_mut() {
            handle.session.performance.alloc_table_visible_height.set(5);
            handle.session.performance.focused_section = PerfSection::MemoryList;
        }
        push_alloc_rows(&mut state, 20);

        handle_perf_jump_to_end(&mut state);

        assert_eq!(
            perf_alloc_row(&state),
            Some(19),
            "Jump to end in MemoryList should select last row (index 19)"
        );
    }

    // ── handle_perf_select_alloc_row ─────────────────────────────────────────

    #[test]
    fn perf_select_alloc_row_focuses_memory_list() {
        let (mut state, _) = make_state_in_performance_panel();
        // Start with a different focused section.
        if let Some(handle) = state.session_manager.selected_mut() {
            handle.session.performance.focused_section = PerfSection::FrameChart;
        }

        handle_perf_select_alloc_row(&mut state, Some(3));

        assert_eq!(perf_alloc_row(&state), Some(3));
        assert_eq!(
            perf_focused_section(&state),
            PerfSection::MemoryList,
            "Selecting an alloc row should move focus to MemoryList"
        );
    }

    #[test]
    fn perf_select_alloc_row_none_clears_selection_without_changing_focus() {
        let (mut state, _) = make_state_in_performance_panel();
        // Start with focus on FrameChart (not MemoryList).
        if let Some(handle) = state.session_manager.selected_mut() {
            handle.session.performance.alloc_table_selected_row = Some(5);
            handle.session.performance.focused_section = PerfSection::FrameChart;
        }

        handle_perf_select_alloc_row(&mut state, None);

        assert_eq!(
            perf_alloc_row(&state),
            None,
            "index=None should clear the alloc row selection"
        );
        // Focus must NOT have jumped to MemoryList — clearing a selection from
        // outside a row (mouse click on empty space) should leave focus alone.
        assert_eq!(
            perf_focused_section(&state),
            PerfSection::FrameChart,
            "index=None must not change focused_section"
        );
    }

    // ── Via Message variants (integration) ───────────────────────────────────

    #[test]
    fn perf_focus_section_message_routes_correctly() {
        let (mut state, _) = make_state_in_performance_panel();

        update(
            &mut state,
            Message::PerfFocusSection(PerfSection::MemoryChart),
        );

        assert_eq!(perf_focused_section(&state), PerfSection::MemoryChart);
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

    #[test]
    fn perf_select_alloc_row_message_routes_correctly() {
        let (mut state, _) = make_state_in_performance_panel();

        update(&mut state, Message::PerfSelectAllocRow { index: Some(7) });

        assert_eq!(perf_alloc_row(&state), Some(7));
        assert_eq!(perf_focused_section(&state), PerfSection::MemoryList);
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

        // Keyboard-style dispatch (Tab from FrameChart → MemoryChart, then direct message).
        update(
            &mut state_keyboard,
            Message::PerfFocusSection(PerfSection::MemoryChart),
        );
        // Mouse-style dispatch (same message — mouse region emits it directly).
        update(
            &mut state_mouse,
            Message::PerfFocusSection(PerfSection::MemoryChart),
        );

        assert_eq!(
            perf_focused_section(&state_keyboard),
            perf_focused_section(&state_mouse),
            "keyboard and mouse PerfFocusSection dispatch must yield identical focused_section"
        );
        assert_eq!(
            perf_focused_section(&state_keyboard),
            PerfSection::MemoryChart,
        );
    }

    /// `PerfSelectAllocRow { index: Some(_) }` sets both the selected row AND
    /// moves focus to MemoryList — whether triggered by keyboard or mouse.
    #[test]
    fn perf_select_alloc_row_with_some_focuses_memory_list() {
        let (mut state, _) = make_state_in_performance_panel();
        // Start in a different section so the focus change is detectable.
        if let Some(handle) = state.session_manager.selected_mut() {
            handle.session.performance.focused_section = PerfSection::FrameChart;
        }

        update(&mut state, Message::PerfSelectAllocRow { index: Some(2) });

        let perf = state.session_manager.selected().unwrap();
        assert_eq!(
            perf.session.performance.alloc_table_selected_row,
            Some(2),
            "PerfSelectAllocRow {{ index: Some(2) }} must set alloc_table_selected_row = Some(2)"
        );
        assert_eq!(
            perf.session.performance.focused_section,
            PerfSection::MemoryList,
            "PerfSelectAllocRow {{ index: Some(_) }} must focus MemoryList"
        );
    }

    /// `PerfSelectAllocRow { index: None }` clears the row selection but must
    /// not change `focused_section`. A mouse click outside any row should not
    /// hijack the user's current section focus.
    #[test]
    fn perf_select_alloc_row_with_none_does_not_change_focus() {
        let (mut state, _) = make_state_in_performance_panel();
        // Park focus on MemoryChart; this is the focus that must survive.
        if let Some(handle) = state.session_manager.selected_mut() {
            handle.session.performance.alloc_table_selected_row = Some(3);
            handle.session.performance.focused_section = PerfSection::MemoryChart;
        }

        update(&mut state, Message::PerfSelectAllocRow { index: None });

        let perf = state.session_manager.selected().unwrap();
        assert_eq!(
            perf.session.performance.alloc_table_selected_row, None,
            "PerfSelectAllocRow {{ index: None }} must clear alloc_table_selected_row"
        );
        assert_eq!(
            perf.session.performance.focused_section,
            PerfSection::MemoryChart,
            "PerfSelectAllocRow {{ index: None }} must NOT change focused_section"
        );
    }
}
