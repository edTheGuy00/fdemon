//! Scroll message handlers
//!
//! Handles vertical and horizontal scrolling in the log view.

use crate::state::{AppState, UiMode};

use super::UpdateResult;

/// Handle scroll up message
pub fn handle_scroll_up(state: &mut AppState) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        handle.session.log_view_state.scroll_up(1);
    }
    rescan_links_if_active(state);
    UpdateResult::none()
}

/// Handle scroll down message
pub fn handle_scroll_down(state: &mut AppState) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        let was_following = handle.session.log_view_state.auto_scroll;
        handle.session.log_view_state.scroll_down(1);
        // If scroll_down naturally re-engaged auto-scroll (false → true),
        // also clear the unseen counter so the jump-to-latest pill disappears.
        if !was_following && handle.session.log_view_state.auto_scroll {
            handle.session.mark_tail_followed();
        }
    }
    rescan_links_if_active(state);
    UpdateResult::none()
}

/// Handle scroll to top message
pub fn handle_scroll_to_top(state: &mut AppState) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        handle.session.log_view_state.scroll_to_top();
    }
    rescan_links_if_active(state);
    UpdateResult::none()
}

/// Handle scroll to bottom message
pub fn handle_scroll_to_bottom(state: &mut AppState) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        handle.session.log_view_state.scroll_to_bottom();
        handle.session.mark_tail_followed();
    }
    rescan_links_if_active(state);
    UpdateResult::none()
}

/// Handle page up message
pub fn handle_page_up(state: &mut AppState) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        handle.session.log_view_state.page_up();
    }
    rescan_links_if_active(state);
    UpdateResult::none()
}

/// Handle page down message
pub fn handle_page_down(state: &mut AppState) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        let was_following = handle.session.log_view_state.auto_scroll;
        handle.session.log_view_state.page_down();
        // If page_down naturally re-engaged auto-scroll (false → true),
        // also clear the unseen counter so the jump-to-latest pill disappears.
        if !was_following && handle.session.log_view_state.auto_scroll {
            handle.session.mark_tail_followed();
        }
    }
    rescan_links_if_active(state);
    UpdateResult::none()
}

/// Handle horizontal scroll left message.
///
/// No-op when wrap mode is enabled — horizontal scrolling is meaningless when
/// lines are wrapped to fit the visible width.
pub fn handle_scroll_left(state: &mut AppState, n: usize) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        if !handle.session.log_view_state.wrap_mode {
            handle.session.log_view_state.scroll_left(n);
        }
    }
    UpdateResult::none()
}

/// Handle horizontal scroll right message.
///
/// No-op when wrap mode is enabled — horizontal scrolling is meaningless when
/// lines are wrapped to fit the visible width.
pub fn handle_scroll_right(state: &mut AppState, n: usize) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        if !handle.session.log_view_state.wrap_mode {
            handle.session.log_view_state.scroll_right(n);
        }
    }
    UpdateResult::none()
}

/// Handle scroll to line start message.
///
/// No-op when wrap mode is enabled — horizontal scrolling is meaningless when
/// lines are wrapped to fit the visible width.
pub fn handle_scroll_to_line_start(state: &mut AppState) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        if !handle.session.log_view_state.wrap_mode {
            handle.session.log_view_state.scroll_to_line_start();
        }
    }
    UpdateResult::none()
}

/// Handle scroll to line end message.
///
/// No-op when wrap mode is enabled — horizontal scrolling is meaningless when
/// lines are wrapped to fit the visible width.
pub fn handle_scroll_to_line_end(state: &mut AppState) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        if !handle.session.log_view_state.wrap_mode {
            handle.session.log_view_state.scroll_to_line_end();
        }
    }
    UpdateResult::none()
}

/// Re-scan links if in link highlight mode (called after scroll operations).
///
/// When the user scrolls while in link mode, the viewport changes and we need
/// to re-scan for file references to update the shortcut assignments.
fn rescan_links_if_active(state: &mut AppState) {
    if state.ui_mode != UiMode::LinkHighlight {
        return;
    }

    if let Some(handle) = state.session_manager.selected_mut() {
        let (visible_start, visible_end) = handle.session.log_view_state.visible_range();

        handle.session.link_highlight_state.scan_viewport(
            &handle.session.logs,
            visible_start,
            visible_end,
            Some(&handle.session.filter_state),
            &handle.session.collapse_state,
            state.settings.ui.stack_trace_collapsed,
            state.settings.ui.stack_trace_max_frames,
        );

        tracing::debug!(
            "Re-scanned links after scroll: {} links found",
            handle.session.link_highlight_state.link_count()
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to create a test Device
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

    /// Helper to create an AppState with one session selected
    fn create_test_state_with_session() -> AppState {
        let mut state = AppState::new();
        let device = test_device();
        let session_id = state.session_manager.create_session(&device).unwrap();
        state.session_manager.select_by_id(session_id);
        state
    }

    // --- Horizontal scroll guard tests ---

    #[test]
    fn test_scroll_left_noop_when_wrap_enabled() {
        let mut state = create_test_state_with_session();
        let handle = state.session_manager.selected_mut().unwrap();
        handle.session.log_view_state.h_offset = 10;
        handle.session.log_view_state.wrap_mode = true;

        handle_scroll_left(&mut state, 5);

        let handle = state.session_manager.selected().unwrap();
        assert_eq!(
            handle.session.log_view_state.h_offset, 10,
            "scroll_left should be no-op in wrap mode"
        );
    }

    #[test]
    fn test_scroll_right_noop_when_wrap_enabled() {
        let mut state = create_test_state_with_session();
        let handle = state.session_manager.selected_mut().unwrap();
        handle.session.log_view_state.h_offset = 0;
        handle.session.log_view_state.max_line_width = 200;
        handle.session.log_view_state.visible_width = 80;
        handle.session.log_view_state.wrap_mode = true;

        handle_scroll_right(&mut state, 10);

        let handle = state.session_manager.selected().unwrap();
        assert_eq!(
            handle.session.log_view_state.h_offset, 0,
            "scroll_right should be no-op in wrap mode"
        );
    }

    #[test]
    fn test_scroll_to_line_start_noop_when_wrap_enabled() {
        let mut state = create_test_state_with_session();
        let handle = state.session_manager.selected_mut().unwrap();
        handle.session.log_view_state.h_offset = 15;
        handle.session.log_view_state.wrap_mode = true;

        handle_scroll_to_line_start(&mut state);

        let handle = state.session_manager.selected().unwrap();
        assert_eq!(
            handle.session.log_view_state.h_offset, 15,
            "scroll_to_line_start should be no-op in wrap mode"
        );
    }

    #[test]
    fn test_scroll_to_line_end_noop_when_wrap_enabled() {
        let mut state = create_test_state_with_session();
        let handle = state.session_manager.selected_mut().unwrap();
        handle.session.log_view_state.h_offset = 0;
        handle.session.log_view_state.max_line_width = 200;
        handle.session.log_view_state.visible_width = 80;
        handle.session.log_view_state.wrap_mode = true;

        handle_scroll_to_line_end(&mut state);

        let handle = state.session_manager.selected().unwrap();
        assert_eq!(
            handle.session.log_view_state.h_offset, 0,
            "scroll_to_line_end should be no-op in wrap mode"
        );
    }

    #[test]
    fn test_scroll_left_works_when_wrap_disabled() {
        let mut state = create_test_state_with_session();
        let handle = state.session_manager.selected_mut().unwrap();
        handle.session.log_view_state.wrap_mode = false;
        handle.session.log_view_state.h_offset = 10;

        handle_scroll_left(&mut state, 5);

        let handle = state.session_manager.selected().unwrap();
        assert_eq!(
            handle.session.log_view_state.h_offset, 5,
            "scroll_left should reduce h_offset when wrap is disabled"
        );
    }

    #[test]
    fn test_scroll_right_works_when_wrap_disabled() {
        let mut state = create_test_state_with_session();
        let handle = state.session_manager.selected_mut().unwrap();
        handle.session.log_view_state.wrap_mode = false;
        handle.session.log_view_state.h_offset = 0;
        handle.session.log_view_state.max_line_width = 200;
        handle.session.log_view_state.visible_width = 80;

        handle_scroll_right(&mut state, 10);

        let handle = state.session_manager.selected().unwrap();
        assert_eq!(
            handle.session.log_view_state.h_offset, 10,
            "scroll_right should increase h_offset when wrap is disabled"
        );
    }

    #[test]
    fn test_scroll_to_line_start_works_when_wrap_disabled() {
        let mut state = create_test_state_with_session();
        let handle = state.session_manager.selected_mut().unwrap();
        handle.session.log_view_state.wrap_mode = false;
        handle.session.log_view_state.h_offset = 30;

        handle_scroll_to_line_start(&mut state);

        let handle = state.session_manager.selected().unwrap();
        assert_eq!(
            handle.session.log_view_state.h_offset, 0,
            "scroll_to_line_start should reset h_offset when wrap is disabled"
        );
    }

    #[test]
    fn test_scroll_to_line_end_works_when_wrap_disabled() {
        let mut state = create_test_state_with_session();
        let handle = state.session_manager.selected_mut().unwrap();
        handle.session.log_view_state.wrap_mode = false;
        handle.session.log_view_state.h_offset = 0;
        handle.session.log_view_state.max_line_width = 200;
        handle.session.log_view_state.visible_width = 80;

        handle_scroll_to_line_end(&mut state);

        let handle = state.session_manager.selected().unwrap();
        assert_eq!(
            handle.session.log_view_state.h_offset,
            120, // 200 - 80
            "scroll_to_line_end should jump to max h_offset when wrap is disabled"
        );
    }

    // ─────────────────────────────────────────────────────────
    // Tests — unseen_log_count wiring (Phase 4, Task 01)
    // ─────────────────────────────────────────────────────────

    #[test]
    fn handle_scroll_to_bottom_clears_unseen_count() {
        let mut state = create_test_state_with_session();
        let handle = state.session_manager.selected_mut().unwrap();
        handle.session.log_view_state.auto_scroll = false;
        handle.session.unseen_log_count = 7;

        let _ = handle_scroll_to_bottom(&mut state);

        let handle = state.session_manager.selected().unwrap();
        assert!(handle.session.log_view_state.auto_scroll);
        assert_eq!(handle.session.unseen_log_count, 0);
    }

    #[test]
    fn handle_scroll_down_clears_unseen_count_on_natural_follow() {
        let mut state = create_test_state_with_session();
        let handle = state.session_manager.selected_mut().unwrap();
        // Position one line above the bottom with auto_scroll off.
        // max_offset = total_lines - visible_lines = 10 - 5 = 5
        // offset = 4, so one scroll_down(1) hits 5 == max_offset → auto_scroll = true
        handle.session.log_view_state.total_lines = 10;
        handle.session.log_view_state.visible_lines = 5;
        handle.session.log_view_state.offset = 4;
        handle.session.log_view_state.auto_scroll = false;
        handle.session.unseen_log_count = 3;

        let _ = handle_scroll_down(&mut state);

        let handle = state.session_manager.selected().unwrap();
        assert!(handle.session.log_view_state.auto_scroll);
        assert_eq!(handle.session.unseen_log_count, 0);
    }

    #[test]
    fn handle_scroll_down_preserves_unseen_count_when_not_yet_at_bottom() {
        let mut state = create_test_state_with_session();
        let handle = state.session_manager.selected_mut().unwrap();
        handle.session.log_view_state.total_lines = 100;
        handle.session.log_view_state.visible_lines = 5;
        handle.session.log_view_state.offset = 10; // far from bottom (max_offset = 95)
        handle.session.log_view_state.auto_scroll = false;
        handle.session.unseen_log_count = 3;

        let _ = handle_scroll_down(&mut state);

        let handle = state.session_manager.selected().unwrap();
        assert!(!handle.session.log_view_state.auto_scroll);
        assert_eq!(handle.session.unseen_log_count, 3); // unchanged
    }

    #[test]
    fn handle_scroll_down_no_op_when_already_following() {
        let mut state = create_test_state_with_session();
        let handle = state.session_manager.selected_mut().unwrap();
        handle.session.log_view_state.total_lines = 10;
        handle.session.log_view_state.visible_lines = 5;
        handle.session.log_view_state.offset = 5;
        handle.session.log_view_state.auto_scroll = true; // already following
        handle.session.unseen_log_count = 0;

        let _ = handle_scroll_down(&mut state);

        let handle = state.session_manager.selected().unwrap();
        assert!(handle.session.log_view_state.auto_scroll);
        assert_eq!(handle.session.unseen_log_count, 0);
    }

    // ─────────────────────────────────────────────────────────
    // Tests — M3: handle_page_down transition guard
    // ─────────────────────────────────────────────────────────

    #[test]
    fn handle_page_down_clears_unseen_count_on_natural_follow() {
        // Geometry: total_lines=10, visible_lines=5 → max_offset=5, page=3
        // Starting at offset=2: scroll_down(3) → offset=min(5,5)=5 → auto_scroll=true
        let mut state = create_test_state_with_session();
        let handle = state.session_manager.selected_mut().unwrap();
        handle.session.log_view_state.total_lines = 10;
        handle.session.log_view_state.visible_lines = 5;
        handle.session.log_view_state.offset = 2;
        handle.session.log_view_state.auto_scroll = false;
        handle.session.unseen_log_count = 4;

        let _ = handle_page_down(&mut state);

        let handle = state.session_manager.selected().unwrap();
        assert!(handle.session.log_view_state.auto_scroll);
        assert_eq!(handle.session.unseen_log_count, 0);
    }

    #[test]
    fn handle_page_down_preserves_unseen_count_when_not_yet_at_bottom() {
        // Geometry: total_lines=1000, visible_lines=5 → max_offset=995, page=3
        // Starting at offset=0: scroll_down(3) → offset=3, not at bottom
        let mut state = create_test_state_with_session();
        let handle = state.session_manager.selected_mut().unwrap();
        handle.session.log_view_state.total_lines = 1000;
        handle.session.log_view_state.visible_lines = 5;
        handle.session.log_view_state.offset = 0;
        handle.session.log_view_state.auto_scroll = false;
        handle.session.unseen_log_count = 4;

        let _ = handle_page_down(&mut state);

        let handle = state.session_manager.selected().unwrap();
        assert!(!handle.session.log_view_state.auto_scroll);
        assert_eq!(handle.session.unseen_log_count, 4);
    }

    #[test]
    fn handle_page_down_no_op_when_already_following() {
        // pre-true → true: no mark_tail_followed called (was_following = true)
        let mut state = create_test_state_with_session();
        let handle = state.session_manager.selected_mut().unwrap();
        handle.session.log_view_state.total_lines = 10;
        handle.session.log_view_state.visible_lines = 5;
        handle.session.log_view_state.offset = 5;
        handle.session.log_view_state.auto_scroll = true;
        handle.session.unseen_log_count = 0;

        let _ = handle_page_down(&mut state);

        let handle = state.session_manager.selected().unwrap();
        assert!(handle.session.log_view_state.auto_scroll);
        assert_eq!(handle.session.unseen_log_count, 0);
    }
}
