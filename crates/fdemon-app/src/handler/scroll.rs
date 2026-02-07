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

/// Handle horizontal scroll left message
pub fn handle_scroll_left(state: &mut AppState, n: usize) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        handle.session.log_view_state.scroll_left(n);
    }
    UpdateResult::none()
}

/// Handle horizontal scroll right message
pub fn handle_scroll_right(state: &mut AppState, n: usize) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        handle.session.log_view_state.scroll_right(n);
    }
    UpdateResult::none()
}

/// Handle scroll to line start message
pub fn handle_scroll_to_line_start(state: &mut AppState) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        handle.session.log_view_state.scroll_to_line_start();
    }
    UpdateResult::none()
}

/// Handle scroll to line end message
pub fn handle_scroll_to_line_end(state: &mut AppState) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        handle.session.log_view_state.scroll_to_line_end();
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
