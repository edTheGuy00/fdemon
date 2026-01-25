## Task: Add render_entry_point_field function

**Objective**: Create the `render_entry_point_field()` function to render the Entry Point field in the Launch Context widget.

**Depends on**: Task 03

### Scope

- `src/tui/widgets/new_session_dialog/launch_context.rs`: Add `render_entry_point_field()` function

### Details

Create a render function for the Entry Point field following the same pattern as `render_flavor_field()`. The field uses `DropdownField` widget to show the current value with an indicator that it can be opened.

#### Implementation

Add this function alongside `render_flavor_field()` and `render_dart_defines_field()`:

```rust
/// Render the entry point dropdown field
fn render_entry_point_field(
    area: Rect,
    buf: &mut Buffer,
    state: &LaunchContextState,
    is_focused: bool,
) {
    let entry_focused =
        is_focused && state.focused_field == super::state::LaunchContextField::EntryPoint;
    let entry_disabled = !state.is_entry_point_editable();

    let display = state.entry_point_display();

    let suffix = if should_show_disabled_suffix(state, super::state::LaunchContextField::EntryPoint)
    {
        Some("(from config)")
    } else {
        None
    };

    let mut field = DropdownField::new("Entry Point", display)
        .focused(entry_focused)
        .disabled(entry_disabled);

    if let Some(s) = suffix {
        field = field.suffix(s);
    }

    field.render(area, buf);
}
```

#### Update `should_show_disabled_suffix()` if needed

Check that the existing `should_show_disabled_suffix()` function handles the new `EntryPoint` field correctly. It should show "(from config)" suffix when:

1. A VSCode config is selected, AND
2. The config has a non-default value for the field

The function likely uses a pattern match or generic logic. Ensure `EntryPoint` is handled:

```rust
fn should_show_disabled_suffix(state: &LaunchContextState, field: LaunchContextField) -> bool {
    // Only show suffix if VSCode config is selected
    if state.selected_config_source() != Some(ConfigSource::VSCode) {
        return false;
    }

    // Check if the field has a value from config
    match field {
        LaunchContextField::Flavor => state.flavor.is_some(),
        LaunchContextField::EntryPoint => state.entry_point.is_some(),  // ADD THIS
        LaunchContextField::DartDefines => !state.dart_defines.is_empty(),
        _ => false,
    }
}
```

### Acceptance Criteria

1. `render_entry_point_field()` function exists
2. Field renders with "Entry Point" label
3. Field shows entry point path or "(default)"
4. Field highlights when focused
5. Field grays out when disabled (VSCode config)
6. "(from config)" suffix shows for VSCode configs with entry_point
7. Uses `DropdownField` widget (same as Flavor field)
8. Code compiles without errors

### Testing

Add snapshot or rendering tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    #[test]
    fn test_render_entry_point_field_default() {
        let state = LaunchContextState::new(LoadedConfigs::default());
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 1));

        render_entry_point_field(Rect::new(0, 0, 40, 1), &mut buf, &state, true);

        let content = buffer_to_string(&buf);
        assert!(content.contains("Entry Point"));
        assert!(content.contains("(default)"));
    }

    #[test]
    fn test_render_entry_point_field_with_value() {
        let mut state = LaunchContextState::new(LoadedConfigs::default());
        state.set_entry_point(Some(PathBuf::from("lib/main_dev.dart")));

        let mut buf = Buffer::empty(Rect::new(0, 0, 50, 1));
        render_entry_point_field(Rect::new(0, 0, 50, 1), &mut buf, &state, true);

        let content = buffer_to_string(&buf);
        assert!(content.contains("Entry Point"));
        assert!(content.contains("lib/main_dev.dart"));
    }

    #[test]
    fn test_render_entry_point_field_vscode_config_shows_suffix() {
        let mut configs = LoadedConfigs::default();
        configs.configs.push(SourcedConfig {
            config: LaunchConfig {
                entry_point: Some(PathBuf::from("lib/main_vscode.dart")),
                ..Default::default()
            },
            source: ConfigSource::VSCode,
            display_name: "VSCode".to_string(),
        });

        let mut state = LaunchContextState::new(configs);
        state.selected_config_index = Some(0);
        state.set_entry_point(Some(PathBuf::from("lib/main_vscode.dart")));

        let mut buf = Buffer::empty(Rect::new(0, 0, 60, 1));
        render_entry_point_field(Rect::new(0, 0, 60, 1), &mut buf, &state, false);

        let content = buffer_to_string(&buf);
        assert!(content.contains("(from config)"));
    }

    fn buffer_to_string(buf: &Buffer) -> String {
        buf.content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>()
    }
}
```

### Notes

- Follows exact same pattern as `render_flavor_field()`
- Uses `DropdownField` widget which shows a dropdown indicator (▼ or similar)
- Field is disabled (not editable) when VSCode config is selected
- "(from config)" suffix indicates value comes from read-only config
- The `entry_point_display()` method from Task 03 provides the display string

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/tui/widgets/new_session_dialog/launch_context.rs` | Added `render_entry_point_field()` function, updated layout to include entry point field (13 chunks instead of 11), updated `render_common_fields()` to render entry point field, updated both full and compact layout modes, added 7 comprehensive tests, updated `min_height()` from 12 to 14 |

### Notable Decisions/Tradeoffs

1. **No changes needed to `should_show_disabled_suffix()`**: The existing implementation already uses the generic `is_field_editable()` method which automatically handles the `EntryPoint` field. This is better than the task specification's pattern-match approach as it's more maintainable.

2. **Layout updates**: Updated the field layout from 11 to 13 chunks to accommodate the entry point field and its spacer. The field is positioned between Flavor and Dart Defines fields as specified in the task hierarchy.

3. **Compact mode support**: Updated the compact layout to include the entry point field, maintaining consistency with the full layout mode.

4. **Test helper function**: Added `buffer_to_string()` helper function in the test module to convert buffers to strings for content verification, following the pattern from existing tests.

### Testing Performed

- `cargo check --lib` - Passed
- `cargo test --lib launch_context` - Passed (51 tests including 7 new tests)
- `cargo clippy --lib -- -D warnings` - Passed (no warnings)
- `cargo fmt -- --check` - Passed

#### New Tests Added:
1. `test_render_entry_point_field_default` - Verifies default "(default)" display
2. `test_render_entry_point_field_with_value` - Verifies custom entry point path display
3. `test_render_entry_point_field_vscode_config_shows_suffix` - Verifies "(from config)" suffix for VSCode configs
4. `test_render_entry_point_field_focused` - Verifies focused state rendering
5. `test_render_entry_point_field_disabled` - Verifies disabled state rendering for VSCode configs
6. `test_min_height_updated_for_entry_point` - Verifies min_height accounts for entry point field
7. `test_launch_context_includes_entry_point` - Verifies entry point field appears in full widget

### Risks/Limitations

1. **Handler compilation errors**: As noted in the task, there may be compilation errors in handler files (tasks 06-07). These do not affect the TUI rendering layer implemented in this task.
