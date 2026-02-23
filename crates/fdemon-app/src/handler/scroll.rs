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
        handle.session.log_view_state.scroll_down(1);
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
        handle.session.log_view_state.page_down();
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
}
