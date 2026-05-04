//! Log view operation handlers
//!
//! Handles link highlighting and editor navigation.

use crate::editor::{open_in_editor, sanitize_path};
use crate::message::Message;
use crate::state::{AppState, LogClickStamp, UiMode};

use super::UpdateResult;

/// Window within which two consecutive clicks on the same row count as a
/// double click. 400 ms matches GNOME / KDE / macOS default double-click
/// thresholds and is short enough that an accidental re-click doesn't
/// trigger an unwanted stack-trace toggle.
const DOUBLE_CLICK_WINDOW: std::time::Duration = std::time::Duration::from_millis(400);

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

/// Handle a click on a single log-view row.
///
/// Tracks consecutive clicks in `state.last_log_click`. When the same
/// `entry_id` is clicked twice within [`DOUBLE_CLICK_WINDOW`], emits a
/// follow-up [`Message::ToggleStackTraceForEntry`] and clears the stamp so
/// a *third* click within the window does not chain another toggle.
///
/// `frame_index` is currently informational — Phase 4 v1 does not act on
/// stack-frame double-click (the natural action would be "open the link"
/// but that overlaps with the existing `LinkHighlight` mode). The field is
/// included in the message so future work can act on it without another
/// `Message` variant.
pub fn handle_click_log_row(
    state: &mut AppState,
    entry_id: u64,
    _frame_index: Option<usize>,
) -> UpdateResult {
    let now = std::time::Instant::now();

    let is_double = state.last_log_click.is_some_and(|prev| {
        prev.entry_id == entry_id && now.saturating_duration_since(prev.at) <= DOUBLE_CLICK_WINDOW
    });

    if is_double {
        // Consume the stamp so a third click within the window doesn't chain.
        state.last_log_click = None;
        return UpdateResult::message(Message::ToggleStackTraceForEntry { entry_id });
    }

    state.last_log_click = Some(LogClickStamp { entry_id, at: now });
    UpdateResult::none()
}

/// Toggle stack trace expansion for the explicit `entry_id`.
///
/// Distinct from [`Message::ToggleStackTrace`], which targets the
/// scroll-focused entry — that handler already exists at
/// `handler/update.rs:682` and stays unchanged. This sibling handler is
/// emitted only by [`handle_click_log_row`] on double-click.
pub fn handle_toggle_stack_trace_for_entry(state: &mut AppState, entry_id: u64) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        let default_collapsed = state.settings.ui.stack_trace_collapsed;
        handle
            .session
            .toggle_stack_trace(entry_id, default_collapsed);
    }
    UpdateResult::none()
}

#[cfg(test)]
mod click_handler_tests {
    use super::*;
    use crate::message::Message;
    use crate::state::AppState;

    fn fresh_state() -> AppState {
        AppState::new()
    }

    #[test]
    fn single_click_records_stamp_and_emits_no_followup() {
        let mut state = fresh_state();
        let result = handle_click_log_row(&mut state, /*entry_id=*/ 42, None);
        assert!(
            result.message.is_none(),
            "single click does not emit follow-up"
        );
        assert!(state.last_log_click.is_some(), "stamp recorded");
        assert_eq!(state.last_log_click.unwrap().entry_id, 42);
    }

    #[test]
    fn second_click_same_entry_within_window_emits_toggle() {
        let mut state = fresh_state();
        let _ = handle_click_log_row(&mut state, 42, None);
        let result = handle_click_log_row(&mut state, 42, None);
        assert!(matches!(
            result.message,
            Some(Message::ToggleStackTraceForEntry { entry_id: 42 })
        ));
        assert!(
            state.last_log_click.is_none(),
            "stamp consumed by double-click"
        );
    }

    #[test]
    fn second_click_different_entry_is_treated_as_fresh_single() {
        let mut state = fresh_state();
        let _ = handle_click_log_row(&mut state, 42, None);
        let result = handle_click_log_row(&mut state, 43, None);
        assert!(result.message.is_none());
        assert_eq!(state.last_log_click.unwrap().entry_id, 43);
    }

    #[test]
    fn third_click_within_window_does_not_chain_double() {
        // A → B → A pattern: third click on A should NOT immediately re-toggle,
        // because the stamp was cleared by the A → A double-click consumption.
        let mut state = fresh_state();
        let _ = handle_click_log_row(&mut state, 42, None);
        let _ = handle_click_log_row(&mut state, 42, None); // double-click → clears stamp
        let result = handle_click_log_row(&mut state, 42, None);
        assert!(
            result.message.is_none(),
            "third click is a fresh single click"
        );
    }

    #[test]
    fn second_click_after_window_is_treated_as_fresh_single() {
        let mut state = fresh_state();
        // Manually plant a stamp older than the window.
        state.last_log_click = Some(LogClickStamp {
            entry_id: 42,
            at: std::time::Instant::now() - std::time::Duration::from_millis(500),
        });
        let result = handle_click_log_row(&mut state, 42, None);
        assert!(result.message.is_none(), "outside window → no double-click");
    }

    #[test]
    fn toggle_stack_trace_for_entry_no_op_without_session() {
        let mut state = AppState::new();
        let result = handle_toggle_stack_trace_for_entry(&mut state, 42);
        assert!(result.message.is_none());
    }
}
