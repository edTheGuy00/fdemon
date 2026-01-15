//! Log view operation handlers
//!
//! Handles log filtering, search, stack trace toggling, and link highlighting.

use crate::app::state::{AppState, UiMode};
use crate::tui::editor::{open_in_editor, sanitize_path};

use super::UpdateResult;

/// Handle clear logs message
pub fn handle_clear_logs(state: &mut AppState) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        handle.session.clear_logs();
    }
    // No fallback needed - only clear logs if a session is selected
    UpdateResult::none()
}

/// Handle cycle level filter message
pub fn handle_cycle_level_filter(state: &mut AppState) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        handle.session.cycle_level_filter();
    }
    UpdateResult::none()
}

/// Handle cycle source filter message
pub fn handle_cycle_source_filter(state: &mut AppState) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        handle.session.cycle_source_filter();
    }
    UpdateResult::none()
}

/// Handle reset filters message
pub fn handle_reset_filters(state: &mut AppState) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        handle.session.reset_filters();
    }
    UpdateResult::none()
}

/// Handle start search message
pub fn handle_start_search(state: &mut AppState) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        handle.session.start_search();
    }
    state.ui_mode = UiMode::SearchInput;
    UpdateResult::none()
}

/// Handle cancel search message
pub fn handle_cancel_search(state: &mut AppState) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        handle.session.cancel_search();
    }
    state.ui_mode = UiMode::Normal;
    UpdateResult::none()
}

/// Handle clear search message
pub fn handle_clear_search(state: &mut AppState) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        handle.session.clear_search();
    }
    state.ui_mode = UiMode::Normal;
    UpdateResult::none()
}

/// Handle search input message
pub fn handle_search_input(state: &mut AppState, text: String) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        handle.session.set_search_query(&text);

        // Execute search immediately
        handle
            .session
            .search_state
            .execute_search(&handle.session.logs);

        // Scroll to first match if found
        if let Some(entry_index) = handle.session.search_state.current_match_entry_index() {
            scroll_to_log_entry(&mut handle.session, entry_index);
        }
    }
    UpdateResult::none()
}

/// Handle next search match message
pub fn handle_next_search_match(state: &mut AppState) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        handle.session.search_state.next_match();

        // Scroll to new current match
        if let Some(entry_index) = handle.session.search_state.current_match_entry_index() {
            scroll_to_log_entry(&mut handle.session, entry_index);
        }
    }
    UpdateResult::none()
}

/// Handle previous search match message
pub fn handle_prev_search_match(state: &mut AppState) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        handle.session.search_state.prev_match();

        // Scroll to new current match
        if let Some(entry_index) = handle.session.search_state.current_match_entry_index() {
            scroll_to_log_entry(&mut handle.session, entry_index);
        }
    }
    UpdateResult::none()
}

/// Handle search completed message
pub fn handle_search_completed(
    state: &mut AppState,
    matches: Vec<crate::core::SearchMatch>,
) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        handle.session.search_state.update_matches(matches);
    }
    UpdateResult::none()
}

/// Handle next error message
pub fn handle_next_error(state: &mut AppState) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        if let Some(error_idx) = handle.session.find_next_error() {
            scroll_to_log_entry(&mut handle.session, error_idx);
        }
    }
    UpdateResult::none()
}

/// Handle previous error message
pub fn handle_prev_error(state: &mut AppState) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        if let Some(error_idx) = handle.session.find_prev_error() {
            scroll_to_log_entry(&mut handle.session, error_idx);
        }
    }
    UpdateResult::none()
}

/// Handle toggle stack trace message
pub fn handle_toggle_stack_trace(state: &mut AppState) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        if let Some(entry_id) = handle.session.focused_entry_id() {
            let default_collapsed = state.settings.ui.stack_trace_collapsed;
            handle
                .session
                .toggle_stack_trace(entry_id, default_collapsed);
        }
    }
    UpdateResult::none()
}

/// Handle enter link mode message
pub fn handle_enter_link_mode(state: &mut AppState) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        // Get visible range from log view state
        let (visible_start, visible_end) = handle.session.log_view_state.visible_range();

        // Scan viewport for links
        handle.session.link_highlight_state.scan_viewport(
            &handle.session.logs,
            visible_start,
            visible_end,
            Some(&handle.session.filter_state),
            &handle.session.collapse_state,
            state.settings.ui.stack_trace_collapsed,
            state.settings.ui.stack_trace_max_frames,
        );

        // Only enter link mode if there are links to show
        if handle.session.link_highlight_state.has_links() {
            handle.session.link_highlight_state.activate();
            state.ui_mode = UiMode::LinkHighlight;
            tracing::debug!(
                "Entered link mode with {} links",
                handle.session.link_highlight_state.link_count()
            );
        } else {
            tracing::debug!("No links found in viewport");
        }
    }
    UpdateResult::none()
}

/// Handle exit link mode message
pub fn handle_exit_link_mode(state: &mut AppState) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        handle.session.link_highlight_state.deactivate();
    }
    state.ui_mode = UiMode::Normal;
    tracing::debug!("Exited link mode");
    UpdateResult::none()
}

/// Handle select link message
pub fn handle_select_link(state: &mut AppState, shortcut: char) -> UpdateResult {
    // Find the link by shortcut before exiting link mode
    let file_ref = if let Some(handle) = state.session_manager.selected_mut() {
        handle
            .session
            .link_highlight_state
            .link_by_shortcut(shortcut)
            .map(|link| link.file_ref.clone())
    } else {
        None
    };

    // Exit link mode
    if let Some(handle) = state.session_manager.selected_mut() {
        handle.session.link_highlight_state.deactivate();
    }
    state.ui_mode = UiMode::Normal;

    // Open the file if we found a matching link
    if let Some(file_ref) = file_ref {
        // Sanitize path
        if sanitize_path(&file_ref.path).is_none() {
            tracing::warn!("Rejected suspicious file path: {}", file_ref.path);
            return UpdateResult::none();
        }

        // Open in editor
        match open_in_editor(&file_ref, &state.settings.editor, &state.project_path) {
            Ok(result) => {
                if result.used_parent_ide {
                    tracing::info!(
                        "Opened {}:{} in {} (parent IDE)",
                        result.file,
                        result.line,
                        result.editor_display_name
                    );
                } else {
                    tracing::info!(
                        "Opened {}:{} in {}",
                        result.file,
                        result.line,
                        result.editor_display_name
                    );
                }
            }
            Err(e) => {
                tracing::warn!("Failed to open file: {}", e);
            }
        }
    } else {
        tracing::debug!("No link found for shortcut '{}'", shortcut);
    }

    UpdateResult::none()
}

/// Scroll the log view to show a specific log entry
fn scroll_to_log_entry(session: &mut crate::app::session::Session, entry_index: usize) {
    // Account for filtering if active
    let visible_index = if session.filter_state.is_active() {
        // Find the position in filtered list
        session
            .logs
            .iter()
            .enumerate()
            .filter(|(_, e)| session.filter_state.matches(e))
            .position(|(i, _)| i == entry_index)
    } else {
        Some(entry_index)
    };

    if let Some(idx) = visible_index {
        // Center the match in the view if possible
        let visible_lines = session.log_view_state.visible_lines;
        let center_offset = visible_lines / 2;
        session.log_view_state.offset = idx.saturating_sub(center_offset);
        session.log_view_state.auto_scroll = false;
    }
}
