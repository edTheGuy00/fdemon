//! NewSessionDialog fuzzy modal handlers
//!
//! Handles fuzzy search modal for config and flavor selection.

use crate::app::handler::UpdateResult;
use crate::app::message::Message;
use crate::app::new_session_dialog::FuzzyModalType;
use crate::app::state::AppState;
use tracing::warn;

/// Handle opening fuzzy modal
pub fn handle_open_fuzzy_modal(state: &mut AppState, modal_type: FuzzyModalType) -> UpdateResult {
    // Prevent opening a modal when another is already open
    if state.new_session_dialog_state.has_modal_open() {
        warn!("Cannot open fuzzy modal while another modal is open");
        return UpdateResult::none();
    }

    match modal_type {
        FuzzyModalType::Config => {
            state.new_session_dialog_state.open_config_modal();
            // Initial filter with empty query (show all)
            apply_fuzzy_filter(state);
        }
        FuzzyModalType::Flavor => {
            // TODO: Get flavors from project analysis
            // For now, use any existing flavor as suggestion
            let mut flavors = Vec::new();
            if let Some(ref flavor) = state.new_session_dialog_state.launch_context.flavor {
                if !flavor.is_empty() {
                    flavors.push(flavor.clone());
                }
            }
            state.new_session_dialog_state.open_flavor_modal(flavors);
            // Initial filter with empty query (show all)
            apply_fuzzy_filter(state);
        }
        FuzzyModalType::EntryPoint => {
            // Set loading state
            state
                .new_session_dialog_state
                .launch_context
                .entry_points_loading = true;

            // Open modal with placeholder (will be populated when discovery completes)
            use crate::app::new_session_dialog::FuzzyModalState;
            state.new_session_dialog_state.fuzzy_modal = Some(FuzzyModalState::new(
                FuzzyModalType::EntryPoint,
                vec!["(discovering...)".to_string()],
            ));

            // Return action to spawn async discovery
            use crate::app::handler::UpdateAction;
            return UpdateResult::action(UpdateAction::DiscoverEntryPoints {
                project_path: state.project_path.clone(),
            });
        }
    };
    UpdateResult::none()
}

/// Handle closing fuzzy modal
pub fn handle_close_fuzzy_modal(state: &mut AppState) -> UpdateResult {
    state.new_session_dialog_state.close_modal();
    UpdateResult::none()
}

/// Handle fuzzy modal navigation up
pub fn handle_fuzzy_up(state: &mut AppState) -> UpdateResult {
    if let Some(ref mut modal) = state.new_session_dialog_state.fuzzy_modal {
        modal.navigate_up();
    }
    UpdateResult::none()
}

/// Handle fuzzy modal navigation down
pub fn handle_fuzzy_down(state: &mut AppState) -> UpdateResult {
    if let Some(ref mut modal) = state.new_session_dialog_state.fuzzy_modal {
        modal.navigate_down();
    }
    UpdateResult::none()
}

/// Handle fuzzy modal confirm selection
pub fn handle_fuzzy_confirm(
    state: &mut AppState,
    update_fn: fn(&mut AppState, Message) -> UpdateResult,
) -> UpdateResult {
    if let Some(ref modal) = state.new_session_dialog_state.fuzzy_modal {
        if let Some(value) = modal.selected_value() {
            match modal.modal_type {
                FuzzyModalType::Config => {
                    // Use the new config selected message
                    return update_fn(
                        state,
                        Message::NewSessionDialogConfigSelected { config_name: value },
                    );
                }
                FuzzyModalType::Flavor => {
                    // Use the new flavor selected message which handles auto-save
                    return update_fn(
                        state,
                        Message::NewSessionDialogFlavorSelected {
                            flavor: if value.is_empty() { None } else { Some(value) },
                        },
                    );
                }
                FuzzyModalType::EntryPoint => {
                    // Use the new entry point selected message which handles auto-save
                    return update_fn(
                        state,
                        Message::NewSessionDialogEntryPointSelected {
                            entry_point: if value.is_empty() { None } else { Some(value) },
                        },
                    );
                }
            }
        }
    }
    state.new_session_dialog_state.close_modal();
    UpdateResult::none()
}

/// Handle fuzzy modal character input
pub fn handle_fuzzy_input(state: &mut AppState, c: char) -> UpdateResult {
    if let Some(ref mut modal) = state.new_session_dialog_state.fuzzy_modal {
        modal.input_char(c);
        apply_fuzzy_filter(state);
    }
    UpdateResult::none()
}

/// Handle fuzzy modal backspace
pub fn handle_fuzzy_backspace(state: &mut AppState) -> UpdateResult {
    if let Some(ref mut modal) = state.new_session_dialog_state.fuzzy_modal {
        modal.backspace();
        apply_fuzzy_filter(state);
    }
    UpdateResult::none()
}

/// Handle fuzzy modal clear query
pub fn handle_fuzzy_clear(state: &mut AppState) -> UpdateResult {
    if let Some(ref mut modal) = state.new_session_dialog_state.fuzzy_modal {
        modal.clear_query();
        apply_fuzzy_filter(state);
    }
    UpdateResult::none()
}

/// Apply fuzzy filter to current modal state
fn apply_fuzzy_filter(state: &mut AppState) {
    if let Some(ref mut modal) = state.new_session_dialog_state.fuzzy_modal {
        // Import the filter function from app layer
        use crate::app::new_session_dialog::fuzzy::fuzzy_filter;

        let query = &modal.query;
        let items = &modal.items;
        let filtered = fuzzy_filter(query, items);
        modal.update_filter(filtered);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::new_session_dialog::{DialogPane, LaunchContextField};
    use crate::app::state::UiMode;
    use std::fs;
    use tempfile::TempDir;

    /// Helper to create a test project with Dart files
    fn create_test_project() -> TempDir {
        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join("lib")).unwrap();
        fs::write(
            temp.path().join("lib/main.dart"),
            "void main() { runApp(MyApp()); }",
        )
        .unwrap();
        fs::write(
            temp.path().join("lib/main_dev.dart"),
            "void main() { runApp(DevApp()); }",
        )
        .unwrap();
        fs::write(
            temp.path().join("pubspec.yaml"),
            "name: test_app\ndependencies:\n  flutter:\n    sdk: flutter\n",
        )
        .unwrap();
        temp
    }

    #[test]
    fn test_entry_point_modal_returns_action() {
        let temp = create_test_project();
        let mut state = AppState::with_settings(
            temp.path().to_path_buf(),
            crate::config::Settings::default(),
        );
        state.ui_mode = UiMode::NewSessionDialog;
        state.new_session_dialog_state.focused_pane = DialogPane::LaunchContext;
        state.new_session_dialog_state.launch_context.focused_field =
            LaunchContextField::EntryPoint;

        let result = handle_open_fuzzy_modal(&mut state, FuzzyModalType::EntryPoint);

        // Should return action to spawn discovery
        assert!(matches!(
            result.action,
            Some(crate::app::handler::UpdateAction::DiscoverEntryPoints { .. })
        ));

        // Loading flag should be set
        assert!(
            state
                .new_session_dialog_state
                .launch_context
                .entry_points_loading
        );

        // Modal should be open with placeholder
        assert!(state.new_session_dialog_state.fuzzy_modal.is_some());
        let modal = state.new_session_dialog_state.fuzzy_modal.as_ref().unwrap();
        assert_eq!(modal.modal_type, FuzzyModalType::EntryPoint);
        assert_eq!(modal.items[0], "(discovering...)");
    }

    #[test]
    fn test_entry_points_discovered_updates_modal() {
        use crate::app::handler::update;
        use crate::app::message::Message;
        use std::path::PathBuf;

        let temp = create_test_project();
        let mut state = AppState::with_settings(
            temp.path().to_path_buf(),
            crate::config::Settings::default(),
        );
        state.ui_mode = UiMode::NewSessionDialog;

        // Simulate modal open with placeholder
        use crate::app::new_session_dialog::FuzzyModalState;
        state.new_session_dialog_state.fuzzy_modal = Some(FuzzyModalState::new(
            FuzzyModalType::EntryPoint,
            vec!["(discovering...)".to_string()],
        ));
        state
            .new_session_dialog_state
            .launch_context
            .entry_points_loading = true;

        // Simulate discovery completion
        let entry_points = vec![
            PathBuf::from("lib/main.dart"),
            PathBuf::from("lib/main_dev.dart"),
        ];
        let result = update(&mut state, Message::EntryPointsDiscovered { entry_points });

        // Loading flag should be cleared
        assert!(
            !state
                .new_session_dialog_state
                .launch_context
                .entry_points_loading
        );

        // Modal should have items (including "(default)")
        let modal = state.new_session_dialog_state.fuzzy_modal.as_ref().unwrap();
        assert!(modal.items.contains(&"(default)".to_string()));
        assert!(modal.items.iter().any(|i| i.contains("main.dart")));

        // UpdateResult should be none (no further action)
        assert!(result.action.is_none());
        assert!(result.message.is_none());
    }

    #[test]
    fn test_entry_point_confirm_with_default() {
        use crate::app::handler::update;
        use crate::app::new_session_dialog::FuzzyModalState;
        use std::path::PathBuf;

        let temp = create_test_project();
        let mut state = AppState::with_settings(
            temp.path().to_path_buf(),
            crate::config::Settings::default(),
        );
        state.ui_mode = UiMode::NewSessionDialog;
        state.new_session_dialog_state.launch_context.entry_point =
            Some(PathBuf::from("lib/main_dev.dart"));

        // Simulate modal already populated (after discovery)
        state.new_session_dialog_state.fuzzy_modal = Some(FuzzyModalState::new(
            FuzzyModalType::EntryPoint,
            vec![
                "(default)".to_string(),
                "lib/main.dart".to_string(),
                "lib/main_dev.dart".to_string(),
            ],
        ));

        // Default is already selected (index 0)
        handle_fuzzy_confirm(&mut state, update);

        // Entry point should be cleared
        assert_eq!(
            state.new_session_dialog_state.launch_context.entry_point,
            None
        );
        assert!(!state.new_session_dialog_state.is_fuzzy_modal_open());
    }

    #[test]
    fn test_entry_point_confirm_with_file() {
        use crate::app::handler::update;
        use crate::app::new_session_dialog::FuzzyModalState;

        let temp = create_test_project();
        let mut state = AppState::with_settings(
            temp.path().to_path_buf(),
            crate::config::Settings::default(),
        );
        state.ui_mode = UiMode::NewSessionDialog;

        // Simulate modal already populated (after discovery)
        state.new_session_dialog_state.fuzzy_modal = Some(FuzzyModalState::new(
            FuzzyModalType::EntryPoint,
            vec![
                "(default)".to_string(),
                "lib/main.dart".to_string(),
                "lib/main_dev.dart".to_string(),
            ],
        ));

        // Navigate to select main_dev.dart (index 2, after default and main.dart)
        if let Some(ref mut modal) = state.new_session_dialog_state.fuzzy_modal {
            modal.navigate_down(); // Move to main.dart
            modal.navigate_down(); // Move to main_dev.dart
        }

        handle_fuzzy_confirm(&mut state, update);

        // Entry point should be set
        assert!(state
            .new_session_dialog_state
            .launch_context
            .entry_point
            .is_some());
        let entry_point = state
            .new_session_dialog_state
            .launch_context
            .entry_point
            .as_ref()
            .unwrap();
        assert!(entry_point.to_string_lossy().contains("main_dev.dart"));
        assert!(!state.new_session_dialog_state.is_fuzzy_modal_open());
    }

    #[test]
    fn test_entry_point_cached_after_discovery() {
        use crate::app::handler::update;
        use crate::app::message::Message;
        use std::path::PathBuf;

        let temp = create_test_project();
        let mut state = AppState::with_settings(
            temp.path().to_path_buf(),
            crate::config::Settings::default(),
        );
        state.ui_mode = UiMode::NewSessionDialog;

        // Initially empty
        assert!(state
            .new_session_dialog_state
            .launch_context
            .available_entry_points
            .is_empty());

        // Simulate discovery completion
        let entry_points = vec![
            PathBuf::from("lib/main.dart"),
            PathBuf::from("lib/main_dev.dart"),
        ];
        update(&mut state, Message::EntryPointsDiscovered { entry_points });

        // Should have cached discovered entry points
        assert!(!state
            .new_session_dialog_state
            .launch_context
            .available_entry_points
            .is_empty());
        assert_eq!(
            state
                .new_session_dialog_state
                .launch_context
                .available_entry_points
                .len(),
            2
        );
    }
}
