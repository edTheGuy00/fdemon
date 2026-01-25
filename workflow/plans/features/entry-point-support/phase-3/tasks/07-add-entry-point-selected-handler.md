## Task: Add handle_entry_point_selected handler

**Objective**: Handle entry point selection from the fuzzy modal, including auto-save for FDemon configurations.

**Depends on**: Task 06

### Scope

- `src/app/handler/new_session/launch_context.rs`: Add `handle_entry_point_selected()` function
- `src/app/handler/update.rs`: Wire up the handler to modal confirmation

### Details

When the user selects an entry point from the fuzzy modal:
1. Parse the selection (handle "(default)" vs actual path)
2. Auto-create FDemon config if none selected and entry point is set
3. Update `state.launch_context.entry_point`
4. Close the modal
5. Trigger auto-save for FDemon configs

This follows the same pattern as `handle_flavor_selected()`.

#### Add `handle_entry_point_selected()` function

```rust
use std::path::PathBuf;

/// Handles entry point selection from the fuzzy modal.
///
/// - "(default)" selection clears the entry point (Flutter uses lib/main.dart)
/// - Path selection sets the entry point
/// - Auto-creates FDemon config if none selected and setting a value
/// - Triggers auto-save for FDemon configurations
pub fn handle_entry_point_selected(
    state: &mut AppState,
    selected: Option<String>,
) -> UpdateResult {
    use crate::config::ConfigSource;

    // Parse selection into Option<PathBuf>
    let entry_point = match selected {
        None => None,
        Some(s) if s == "(default)" => None,
        Some(s) => Some(PathBuf::from(s)),
    };

    // Check if field is editable
    if !state
        .new_session_dialog_state
        .launch_context
        .is_entry_point_editable()
    {
        state.new_session_dialog_state.close_modal();
        return UpdateResult::none();
    }

    // Determine if we need to auto-create a config
    // Only create if setting an entry point (Some), not when clearing (None)
    let needs_auto_create = state
        .new_session_dialog_state
        .launch_context
        .selected_config_index
        .is_none()
        && entry_point.is_some();

    // Auto-create config if needed
    if needs_auto_create {
        state
            .new_session_dialog_state
            .launch_context
            .create_and_select_default_config();
        if let Some(config) = state
            .new_session_dialog_state
            .launch_context
            .selected_config()
        {
            tracing::info!(
                "Auto-created config '{}' for entry point selection",
                config.config.name
            );
        }
    }

    // Apply the entry point to state
    state
        .new_session_dialog_state
        .launch_context
        .set_entry_point(entry_point);
    state.new_session_dialog_state.close_modal();

    // Determine if we should auto-save
    let should_auto_save = if let Some(config_idx) = state
        .new_session_dialog_state
        .launch_context
        .selected_config_index
    {
        if let Some(config) = state
            .new_session_dialog_state
            .launch_context
            .configs
            .configs
            .get(config_idx)
        {
            config.source == ConfigSource::FDemon
        } else {
            false
        }
    } else {
        false
    };

    if should_auto_save {
        return UpdateResult::action(UpdateAction::AutoSaveConfig {
            configs: state
                .new_session_dialog_state
                .launch_context
                .configs
                .clone(),
        });
    }

    UpdateResult::none()
}
```

#### Wire up in modal confirmation handler

In the fuzzy modal confirmation handler (when user presses Enter in modal):

```rust
// In modal Enter key handler:
if let Some(modal) = &state.new_session_dialog_state.fuzzy_modal {
    let selected = modal.selected_value();
    match modal.modal_type {
        FuzzyModalType::Config => handle_config_selected(state, selected),
        FuzzyModalType::Flavor => handle_flavor_selected(state, selected),
        FuzzyModalType::EntryPoint => handle_entry_point_selected(state, selected),  // NEW
    }
}
```

### Acceptance Criteria

1. `handle_entry_point_selected()` function exists
2. "(default)" selection clears entry point to `None`
3. Path selection sets entry point to `Some(PathBuf)`
4. Auto-creates FDemon config when no config and setting entry point
5. Does NOT auto-create config when clearing entry point
6. Updates `state.launch_context.entry_point`
7. Closes the fuzzy modal
8. Triggers `AutoSaveConfig` action for FDemon configs
9. Does NOT trigger auto-save for VSCode configs
10. Handler wired up to modal confirmation
11. Code compiles without errors

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::state::UiMode;
    use crate::config::{ConfigSource, LaunchConfig, SourcedConfig};

    #[test]
    fn test_entry_point_selected_sets_path() {
        let mut state = AppState::default();
        state.ui_mode = UiMode::NewSessionDialog;
        state.new_session_dialog_state.focused_pane = DialogPane::LaunchContext;

        // Add FDemon config so auto-save can trigger
        state
            .new_session_dialog_state
            .launch_context
            .configs
            .configs
            .push(SourcedConfig {
                config: LaunchConfig::default(),
                source: ConfigSource::FDemon,
                display_name: "Default".to_string(),
            });
        state
            .new_session_dialog_state
            .launch_context
            .selected_config_index = Some(0);

        let result = handle_entry_point_selected(
            &mut state,
            Some("lib/main_dev.dart".to_string()),
        );

        // Entry point should be set
        assert_eq!(
            state.new_session_dialog_state.launch_context.entry_point,
            Some(PathBuf::from("lib/main_dev.dart"))
        );

        // Should trigger auto-save
        assert!(matches!(
            result.action,
            Some(UpdateAction::AutoSaveConfig { .. })
        ));
    }

    #[test]
    fn test_entry_point_selected_default_clears() {
        let mut state = AppState::default();
        state.ui_mode = UiMode::NewSessionDialog;
        state.new_session_dialog_state.launch_context.entry_point =
            Some(PathBuf::from("lib/old.dart"));

        let _result = handle_entry_point_selected(&mut state, Some("(default)".to_string()));

        // Entry point should be cleared
        assert_eq!(
            state.new_session_dialog_state.launch_context.entry_point,
            None
        );
    }

    #[test]
    fn test_entry_point_selected_none_clears() {
        let mut state = AppState::default();
        state.ui_mode = UiMode::NewSessionDialog;
        state.new_session_dialog_state.launch_context.entry_point =
            Some(PathBuf::from("lib/old.dart"));

        let _result = handle_entry_point_selected(&mut state, None);

        // Entry point should be cleared
        assert_eq!(
            state.new_session_dialog_state.launch_context.entry_point,
            None
        );
    }

    #[test]
    fn test_entry_point_selected_auto_creates_config() {
        let mut state = AppState::default();
        state.ui_mode = UiMode::NewSessionDialog;
        // No config selected
        assert!(state
            .new_session_dialog_state
            .launch_context
            .selected_config_index
            .is_none());

        let result = handle_entry_point_selected(
            &mut state,
            Some("lib/main_dev.dart".to_string()),
        );

        // Config should be created and selected
        assert!(state
            .new_session_dialog_state
            .launch_context
            .selected_config_index
            .is_some());
        let idx = state
            .new_session_dialog_state
            .launch_context
            .selected_config_index
            .unwrap();
        let config = &state
            .new_session_dialog_state
            .launch_context
            .configs
            .configs[idx];

        assert_eq!(config.config.name, "Default");
        assert_eq!(config.source, ConfigSource::FDemon);

        // Entry point should be set
        assert_eq!(
            state.new_session_dialog_state.launch_context.entry_point,
            Some(PathBuf::from("lib/main_dev.dart"))
        );

        // Should trigger auto-save
        assert!(matches!(
            result.action,
            Some(UpdateAction::AutoSaveConfig { .. })
        ));
    }

    #[test]
    fn test_entry_point_cleared_no_config_no_create() {
        let mut state = AppState::default();
        state.ui_mode = UiMode::NewSessionDialog;

        // Clear entry point (set to default) - should NOT create config
        let result = handle_entry_point_selected(&mut state, Some("(default)".to_string()));

        assert!(state
            .new_session_dialog_state
            .launch_context
            .selected_config_index
            .is_none());
        assert!(state
            .new_session_dialog_state
            .launch_context
            .configs
            .configs
            .is_empty());
        assert!(result.action.is_none());
    }

    #[test]
    fn test_entry_point_selected_vscode_config_no_save() {
        let mut state = AppState::default();
        state.ui_mode = UiMode::NewSessionDialog;

        // Add VSCode config (read-only)
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

        let result = handle_entry_point_selected(
            &mut state,
            Some("lib/main_dev.dart".to_string()),
        );

        // Should NOT trigger auto-save for VSCode config
        // Note: The handler checks is_entry_point_editable() and returns early
        // Entry point should NOT be set because field is not editable
        assert!(result.action.is_none());
    }

    #[test]
    fn test_entry_point_selected_closes_modal() {
        let mut state = AppState::default();
        state.ui_mode = UiMode::NewSessionDialog;

        // Simulate modal being open
        state.new_session_dialog_state.fuzzy_modal =
            Some(FuzzyModalState::new(FuzzyModalType::EntryPoint, vec![]));

        handle_entry_point_selected(&mut state, Some("lib/main.dart".to_string()));

        // Modal should be closed
        assert!(state.new_session_dialog_state.fuzzy_modal.is_none());
    }
}
```

### Notes

- Follows exact same pattern as `handle_flavor_selected()`
- "(default)" is the special value that clears the entry point
- Auto-create only happens when SETTING a value, not when clearing
- VSCode configs are read-only, so selection is blocked
- `AutoSaveConfig` action is handled elsewhere to persist changes

### Auto-Save Flow

When `AutoSaveConfig` action is returned:
1. The action runner calls `update_launch_config_field()` (from Phase 1)
2. This persists the entry_point to `.fdemon/launch.toml`
3. The file is saved with the updated configuration

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/app/handler/new_session/launch_context.rs` | Added `handle_entry_point_selected()` function with auto-save logic and comprehensive tests (7 tests) |
| `src/app/handler/new_session/fuzzy_modal.rs` | Updated `handle_fuzzy_confirm()` to dispatch EntryPoint selection to handler via Message; updated 2 tests to use real update function |
| `src/app/message.rs` | Added `NewSessionDialogEntryPointSelected` message variant |
| `src/app/handler/update.rs` | Wired up `NewSessionDialogEntryPointSelected` message to handler |

### Notable Decisions/Tradeoffs

1. **Message-based dispatch**: Following the pattern established by `handle_flavor_selected()`, the handler is invoked via the Message enum rather than being called directly from the fuzzy modal. This maintains consistency with TEA architecture and allows the handler to return UpdateActions for auto-save.
2. **"(default)" special value**: Following the flavor pattern, the string "(default)" is treated as a request to clear the entry point (set to None), while any other string becomes a PathBuf.
3. **Auto-create behavior**: Config is only auto-created when SETTING a value (Some), not when clearing to default (None). This prevents unnecessary config creation when the user just wants to use the default entry point.
4. **VSCode config protection**: The handler checks `is_entry_point_editable()` first and returns early if the field is read-only (VSCode config), preventing any state changes.

### Testing Performed

- `cargo check` - Passed
- `cargo test --lib handler::new_session::launch_context::tests::test_entry_point` - Passed (7 tests)
- `cargo test --lib handler::new_session::fuzzy_modal::tests::test_entry_point` - Passed (5 tests)
- `cargo test --lib` - Passed (1557 tests)
- `cargo clippy -- -D warnings` - Passed (0 warnings)
- `cargo fmt` - Applied formatting

### Test Coverage

**New tests in launch_context.rs:**
- `test_entry_point_selected_sets_path` - Verifies entry point is set and auto-save triggers
- `test_entry_point_selected_default_clears` - Verifies "(default)" clears entry point
- `test_entry_point_selected_none_clears` - Verifies None selection clears entry point
- `test_entry_point_selected_auto_creates_config` - Verifies config creation when no config exists
- `test_entry_point_cleared_no_config_no_create` - Verifies no config created when clearing
- `test_entry_point_selected_vscode_config_no_save` - Verifies VSCode configs are read-only
- `test_entry_point_selected_closes_modal` - Verifies modal closes after selection

**Updated tests in fuzzy_modal.rs:**
- `test_entry_point_confirm_with_default` - Now uses real update function
- `test_entry_point_confirm_with_file` - Now uses real update function

### Risks/Limitations

None identified. The implementation follows established patterns from flavor and dart-defines handlers, ensuring consistency and maintainability.
