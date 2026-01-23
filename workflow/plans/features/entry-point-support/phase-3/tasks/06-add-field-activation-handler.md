## Task: Add Entry Point field activation handler

**Objective**: Handle Enter key press on Entry Point field to open fuzzy modal with discovered entry points.

**Depends on**: Task 03, Phase 2 (discover_entry_points)

### Scope

- `src/app/handler/new_session/launch_context.rs`: Add handler for Entry Point field activation
- `src/app/handler/update.rs`: Wire up the handler to message routing

### Details

When the user presses Enter on the Entry Point field, discover available entry points and open a fuzzy modal. The modal should include "(default)" as the first option to allow clearing the selection.

#### Add `handle_entry_point_activate()` function

```rust
use crate::core::discovery::discover_entry_points;

/// Opens the entry point selection modal when Enter is pressed on the EntryPoint field.
///
/// Discovers entry points from the project's lib/ directory and populates the fuzzy modal.
/// Includes "(default)" as first option to clear the entry point selection.
pub fn handle_entry_point_activate(state: &mut AppState) -> UpdateResult {
    use crate::app::new_session_dialog::{DialogPane, FuzzyModalType, LaunchContextField};

    // Only activate if Entry Point field is focused
    if state.new_session_dialog_state.focused_pane != DialogPane::LaunchContext
        || state.new_session_dialog_state.launch_context.focused_field
            != LaunchContextField::EntryPoint
    {
        return UpdateResult::none();
    }

    // Check if field is editable
    if !state
        .new_session_dialog_state
        .launch_context
        .is_entry_point_editable()
    {
        return UpdateResult::none();
    }

    // Discover entry points from project
    let entry_points = if let Some(project_path) = &state.project_path {
        discover_entry_points(project_path)
    } else {
        Vec::new()
    };

    // Cache discovered entry points in state
    state
        .new_session_dialog_state
        .launch_context
        .set_available_entry_points(entry_points);

    // Build modal items: "(default)" + discovered paths
    let items = state
        .new_session_dialog_state
        .launch_context
        .entry_point_modal_items();

    // Open fuzzy modal
    state
        .new_session_dialog_state
        .open_fuzzy_modal(FuzzyModalType::EntryPoint, items);

    UpdateResult::none()
}
```

#### Wire up in message handler

In `src/app/handler/update.rs` or the appropriate message handling location, add a case for Entry Point field activation:

```rust
// In the Enter key handler for NewSessionDialog:
Message::KeyPress(KeyEvent { code: KeyCode::Enter, .. }) => {
    match state.new_session_dialog_state.launch_context.focused_field {
        LaunchContextField::Config => handle_config_activate(state),
        LaunchContextField::Flavor => handle_flavor_activate(state),
        LaunchContextField::EntryPoint => handle_entry_point_activate(state),  // NEW
        LaunchContextField::DartDefines => handle_dart_defines_activate(state),
        LaunchContextField::Launch => handle_launch(state),
        _ => UpdateResult::none(),
    }
}
```

#### Update `open_fuzzy_modal()` if needed

Ensure the method in `NewSessionDialogState` can handle the new `FuzzyModalType::EntryPoint`:

```rust
impl NewSessionDialogState {
    pub fn open_fuzzy_modal(&mut self, modal_type: FuzzyModalType, items: Vec<String>) {
        self.fuzzy_modal = Some(FuzzyModalState::new(modal_type, items));
    }
}
```

This should already work generically, but verify it handles the new type.

### Acceptance Criteria

1. `handle_entry_point_activate()` function exists
2. Only activates when EntryPoint field is focused
3. Only activates when field is editable (not VSCode config)
4. Discovers entry points using `discover_entry_points()`
5. Caches discovered entry points in `state.launch_context.available_entry_points`
6. Opens fuzzy modal with type `FuzzyModalType::EntryPoint`
7. Modal items include "(default)" as first option
8. Modal items include all discovered entry points
9. Handler wired up to Enter key in message routing
10. Code compiles without errors

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::state::UiMode;
    use tempfile::TempDir;
    use std::fs;

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
    fn test_entry_point_activate_opens_modal() {
        let temp = create_test_project();
        let mut state = AppState::default();
        state.ui_mode = UiMode::NewSessionDialog;
        state.project_path = Some(temp.path().to_path_buf());
        state.new_session_dialog_state.focused_pane = DialogPane::LaunchContext;
        state.new_session_dialog_state.launch_context.focused_field =
            LaunchContextField::EntryPoint;

        handle_entry_point_activate(&mut state);

        // Modal should be open
        assert!(state.new_session_dialog_state.fuzzy_modal.is_some());

        let modal = state.new_session_dialog_state.fuzzy_modal.as_ref().unwrap();
        assert_eq!(modal.modal_type, FuzzyModalType::EntryPoint);

        // Should have "(default)" + discovered entry points
        assert!(modal.items.len() >= 2); // At least (default) + main.dart
        assert_eq!(modal.items[0], "(default)");
    }

    #[test]
    fn test_entry_point_activate_not_focused_does_nothing() {
        let mut state = AppState::default();
        state.ui_mode = UiMode::NewSessionDialog;
        state.new_session_dialog_state.focused_pane = DialogPane::LaunchContext;
        state.new_session_dialog_state.launch_context.focused_field =
            LaunchContextField::Flavor; // Not EntryPoint

        handle_entry_point_activate(&mut state);

        // Modal should NOT be open
        assert!(state.new_session_dialog_state.fuzzy_modal.is_none());
    }

    #[test]
    fn test_entry_point_activate_vscode_config_does_nothing() {
        let mut state = AppState::default();
        state.ui_mode = UiMode::NewSessionDialog;
        state.new_session_dialog_state.focused_pane = DialogPane::LaunchContext;
        state.new_session_dialog_state.launch_context.focused_field =
            LaunchContextField::EntryPoint;

        // Select VSCode config (read-only)
        state
            .new_session_dialog_state
            .launch_context
            .configs
            .configs
            .push(SourcedConfig {
                config: LaunchConfig::default(),
                source: ConfigSource::VSCode,
                display_name: "VSCode".to_string(),
            });
        state
            .new_session_dialog_state
            .launch_context
            .selected_config_index = Some(0);

        handle_entry_point_activate(&mut state);

        // Modal should NOT be open (VSCode config is read-only)
        assert!(state.new_session_dialog_state.fuzzy_modal.is_none());
    }

    #[test]
    fn test_entry_point_activate_caches_discovery() {
        let temp = create_test_project();
        let mut state = AppState::default();
        state.ui_mode = UiMode::NewSessionDialog;
        state.project_path = Some(temp.path().to_path_buf());
        state.new_session_dialog_state.focused_pane = DialogPane::LaunchContext;
        state.new_session_dialog_state.launch_context.focused_field =
            LaunchContextField::EntryPoint;

        // Initially empty
        assert!(state
            .new_session_dialog_state
            .launch_context
            .available_entry_points
            .is_empty());

        handle_entry_point_activate(&mut state);

        // Should have cached discovered entry points
        assert!(!state
            .new_session_dialog_state
            .launch_context
            .available_entry_points
            .is_empty());
    }
}
```

### Notes

- Requires Phase 2's `discover_entry_points()` function
- Discovery is performed on each activation (could be optimized with caching later)
- "(default)" is always first, allowing users to clear the entry point
- VSCode configs block activation since they're read-only
- `state.project_path` must be set for discovery to work
