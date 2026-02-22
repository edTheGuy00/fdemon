//! Performance panel handlers.
//!
//! Handles frame selection, allocation profile updates, and rich memory samples
//! for the Performance panel's bar chart and time-series views.

use crate::handler::UpdateResult;
use crate::session::AllocationSortColumn;
use crate::session::SessionId;
use crate::state::AppState;
use fdemon_core::performance::{AllocationProfile, MemorySample};

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
}
