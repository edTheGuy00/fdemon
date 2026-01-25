## Task: Reorder editability check in entry point handler

**Objective**: Move `is_entry_point_editable()` check before parsing selection to maintain consistency with `handle_flavor_selected()` pattern.

**Depends on**: None (can be done independently)

### Scope

- `src/app/handler/new_session/launch_context.rs`: Reorder checks in `handle_entry_point_selected()`

### Details

The current implementation parses the selection before checking if the field is editable:

```rust
// CURRENT:
pub fn handle_entry_point_selected(
    state: &mut AppState,
    selected: Option<String>,
) -> UpdateResult {
    // Parse selection into Option<PathBuf>
    let entry_point = match selected {     // Parse first
        None => None,
        Some(s) if s == "(default)" => None,
        Some(s) => Some(PathBuf::from(s)),
    };

    // Check if field is editable
    if !state.new_session_dialog_state.launch_context
        .is_entry_point_editable()          // Check second
    {
        state.new_session_dialog_state.close_modal();
        return UpdateResult::none();
    }
    // ...
}
```

This should be reordered to check editability first, matching `handle_flavor_selected()`:

```rust
// IMPROVED:
pub fn handle_entry_point_selected(
    state: &mut AppState,
    selected: Option<String>,
) -> UpdateResult {
    use crate::config::ConfigSource;

    // Check if field is editable FIRST
    if !state.new_session_dialog_state.launch_context
        .is_entry_point_editable()
    {
        state.new_session_dialog_state.close_modal();
        return UpdateResult::none();
    }

    // Parse selection into Option<PathBuf>
    let entry_point = selected
        .filter(|s| s != "(default)")
        .map(PathBuf::from);

    // ... rest of handler ...
}
```

Also use the more idiomatic functional style for parsing:

```rust
// Before:
let entry_point = match selected {
    None => None,
    Some(s) if s == "(default)" => None,
    Some(s) => Some(PathBuf::from(s)),
};

// After:
let entry_point = selected
    .filter(|s| s != "(default)")
    .map(PathBuf::from);
```

### Acceptance Criteria

1. Editability check happens before parsing selection
2. Functional style used for parsing (`filter` + `map`)
3. All existing tests pass
4. Behavior unchanged (this is just a refactor)
5. Code compiles without warnings

### Testing

Existing tests should pass unchanged. Optionally add test verifying early return:

```rust
#[test]
fn test_entry_point_selected_checks_editable_first() {
    let mut state = AppState::default();
    state.ui_mode = UiMode::NewSessionDialog;

    // Add VSCode config (read-only)
    state.new_session_dialog_state.launch_context.configs.configs.push(SourcedConfig {
        config: LaunchConfig::default(),
        source: ConfigSource::VSCode,
        display_name: "VSCode".to_string(),
    });
    state.new_session_dialog_state.launch_context.selected_config_index = Some(0);

    // Simulate modal open
    state.new_session_dialog_state.fuzzy_modal =
        Some(FuzzyModalState::new(FuzzyModalType::EntryPoint, vec![]));

    // Try to select entry point
    let result = handle_entry_point_selected(
        &mut state,
        Some("lib/main_dev.dart".to_string()),
    );

    // Should return early without processing
    assert!(result.action.is_none());

    // Modal should be closed
    assert!(state.new_session_dialog_state.fuzzy_modal.is_none());

    // Entry point should NOT be set (handler returned early)
    assert!(state.new_session_dialog_state.launch_context.entry_point.is_none());
}
```

### Notes

- This is a code quality improvement, not a bug fix
- The current code works correctly; this is about pattern consistency
- Checking editability first avoids unnecessary parsing work
- The functional style (`filter` + `map`) is more idiomatic Rust
- Low priority since behavior is unchanged
