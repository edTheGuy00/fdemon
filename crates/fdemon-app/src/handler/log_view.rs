//! Log view operation handlers
//!
//! Handles link highlighting and editor navigation.

use crate::editor::{open_in_editor, sanitize_path};
use crate::state::{AppState, UiMode};

use super::UpdateResult;

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
