## Task: Update widget layout for Entry Point field

**Objective**: Update `calculate_fields_layout()` and `render_common_fields()` to include the Entry Point field in the Launch Context widget.

**Depends on**: Task 04

### Scope

- `src/tui/widgets/new_session_dialog/launch_context.rs`:
  - Update `calculate_fields_layout()`
  - Update `render_common_fields()`
  - Update compact layout if applicable

### Details

Add a row for the Entry Point field in the layout calculation and call `render_entry_point_field()` in the render function.

#### Update `calculate_fields_layout()`

The current layout has 11 chunks. Add 2 more (spacer + field) for Entry Point:

```rust
/// Calculate the layout for all fields
fn calculate_fields_layout(inner: Rect) -> [Rect; 13] {  // Was 11, now 13
    let chunks = Layout::vertical([
        Constraint::Length(1), // Spacer
        Constraint::Length(1), // Config field
        Constraint::Length(1), // Spacer
        Constraint::Length(1), // Mode field
        Constraint::Length(1), // Spacer
        Constraint::Length(1), // Flavor field
        Constraint::Length(1), // Spacer           // NEW
        Constraint::Length(1), // Entry Point field // NEW
        Constraint::Length(1), // Spacer
        Constraint::Length(1), // Dart Defines field
        Constraint::Length(1), // Spacer
        Constraint::Length(1), // Launch button
        Constraint::Min(0),    // Remaining space
    ])
    .split(inner);

    // Convert to fixed-size array
    chunks.to_vec().try_into().expect("Layout has 13 elements")
}
```

**Note**: The exact indices need to be verified against the current implementation. The goal is to insert Entry Point between Flavor and Dart Defines.

#### Update `render_common_fields()`

Add the call to render the Entry Point field:

```rust
/// Render the common fields (Config, Mode, Flavor, EntryPoint, DartDefines)
fn render_common_fields(
    chunks: &[Rect; 13],  // Updated size
    buf: &mut Buffer,
    state: &LaunchContextState,
    is_focused: bool,
) {
    render_config_field(chunks[1], buf, state, is_focused);
    render_mode_field(chunks[3], buf, state, is_focused);
    render_flavor_field(chunks[5], buf, state, is_focused);
    render_entry_point_field(chunks[7], buf, state, is_focused);  // NEW
    render_dart_defines_field(chunks[9], buf, state, is_focused); // Index shifted
}
```

#### Update `LaunchContext` widget render

Update the main widget's render method to use the new layout:

```rust
impl Widget for LaunchContext<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let inner = render_border(area, buf, self.is_focused);
        let chunks = calculate_fields_layout(inner);

        render_common_fields(&chunks, buf, self.state, self.is_focused);

        // Render Launch button (index shifted)
        let launch_focused =
            self.is_focused && self.state.focused_field == LaunchContextField::Launch;
        let launch_button = LaunchButton::new().focused(launch_focused);
        launch_button.render(chunks[11], buf);  // Index updated
    }
}
```

#### Update compact layout (`LaunchContextCompact`)

If there's a compact layout variant, update it similarly:

```rust
impl Widget for LaunchContextCompact<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // ... border rendering ...

        let chunks = Layout::vertical([
            Constraint::Length(1), // Config
            Constraint::Length(1), // Mode
            Constraint::Length(1), // Flavor
            Constraint::Length(1), // Entry Point  // NEW
            Constraint::Length(1), // Dart Defines
            Constraint::Length(1), // Spacer
            Constraint::Length(1), // Launch button
        ])
        .split(inner);

        // Render fields...
        render_config_field(chunks[0], buf, self.state, self.is_focused);
        // ... mode inline render ...
        render_flavor_field(chunks[2], buf, self.state, self.is_focused);
        render_entry_point_field(chunks[3], buf, self.state, self.is_focused);  // NEW
        render_dart_defines_field(chunks[4], buf, self.state, self.is_focused); // Shifted

        // Launch button
        let launch_button = LaunchButton::new()
            .focused(launch_focused)
            .enabled(self.has_device_selected);
        launch_button.render(chunks[6], buf);  // Shifted
    }
}
```

#### Update minimum height

If `min_height()` is defined, increase it to account for the new field:

```rust
impl<'a> LaunchContext<'a> {
    /// Calculate minimum height needed
    pub fn min_height() -> u16 {
        14  // Was 12, now 14 (added 2 for spacer + field)
    }
}
```

### Acceptance Criteria

1. `calculate_fields_layout()` returns array with space for Entry Point
2. `render_common_fields()` calls `render_entry_point_field()`
3. Entry Point field appears between Flavor and Dart Defines
4. Launch button renders at correct position
5. Compact layout (if exists) includes Entry Point field
6. Minimum height updated if applicable
7. Code compiles without errors
8. UI renders correctly (manual verification)

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout_has_entry_point_row() {
        let area = Rect::new(0, 0, 60, 20);
        let chunks = calculate_fields_layout(area);

        // Verify we have enough chunks
        assert_eq!(chunks.len(), 13);

        // Verify Entry Point row has height 1
        assert_eq!(chunks[7].height, 1);
    }

    #[test]
    fn test_full_widget_renders_entry_point() {
        let state = LaunchContextState::new(LoadedConfigs::default());
        let widget = LaunchContext::new(&state, true);

        let mut buf = Buffer::empty(Rect::new(0, 0, 60, 20));
        widget.render(Rect::new(0, 0, 60, 20), &mut buf);

        let content = buffer_to_string(&buf);
        assert!(content.contains("Entry Point"));
    }
}
```

### Notes

- Exact chunk indices depend on current implementation - verify before implementing
- The compact layout may use different chunk organization
- Test rendering visually with `cargo run` after changes
- If layout becomes too tall, consider if compact mode needs adjustment
