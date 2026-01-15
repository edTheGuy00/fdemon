# Task: Consolidate Widget Rendering Code

## Summary

Extract shared rendering logic from `LaunchContext` and `LaunchContextWithDevice` widgets to reduce the 110+ duplicated lines.

## Files

| File | Action |
|------|--------|
| `src/tui/widgets/new_session_dialog/launch_context.rs` | Modify (extract shared logic) |

## Background

The code review identified that `LaunchContext` and `LaunchContextWithDevice` widgets share 110+ duplicated lines of rendering code. This duplication increases maintenance burden.

**Duplicated sections (around lines 583-777):**
- Field rendering (config, mode, flavor, dart defines, launch button)
- Styling logic (focused, disabled states)
- Layout calculations

## Implementation

### 1. Identify shared rendering logic

Both widgets render:
- Configuration dropdown field
- Mode radio buttons
- Flavor dropdown field
- Dart defines field
- Launch button

The difference is that `LaunchContextWithDevice` shows the selected device in the launch button.

### 2. Extract shared field renderers

```rust
// Private helper functions for field rendering

fn render_config_field(
    area: Rect,
    buf: &mut Buffer,
    config_name: &str,
    is_focused: bool,
    is_disabled: bool,
) {
    let style = field_style(is_focused, is_disabled);
    // ... rendering logic
}

fn render_mode_field(
    area: Rect,
    buf: &mut Buffer,
    mode: FlutterMode,
    is_focused: bool,
    is_disabled: bool,
) {
    let style = field_style(is_focused, is_disabled);
    // ... rendering logic
}

fn render_flavor_field(
    area: Rect,
    buf: &mut Buffer,
    flavor: Option<&str>,
    is_focused: bool,
    is_disabled: bool,
) {
    let style = field_style(is_focused, is_disabled);
    // ... rendering logic
}

fn render_dart_defines_field(
    area: Rect,
    buf: &mut Buffer,
    count: usize,
    is_focused: bool,
    is_disabled: bool,
) {
    let style = field_style(is_focused, is_disabled);
    // ... rendering logic
}

fn render_launch_button(
    area: Rect,
    buf: &mut Buffer,
    device_name: Option<&str>,  // None for LaunchContext, Some for LaunchContextWithDevice
    is_focused: bool,
) {
    let label = match device_name {
        Some(name) => format!("🚀 Launch on {} (Enter)", name),
        None => "🚀 LAUNCH (Enter)".to_string(),
    };
    // ... rendering logic
}

fn field_style(is_focused: bool, is_disabled: bool) -> Style {
    match (is_focused, is_disabled) {
        (true, false) => Style::default().fg(Color::Yellow),
        (true, true) => Style::default().fg(Color::DarkGray),
        (false, false) => Style::default().fg(Color::White),
        (false, true) => Style::default().fg(Color::DarkGray),
    }
}
```

### 3. Refactor LaunchContext widget

```rust
impl Widget for LaunchContext<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let layout = self.calculate_layout(area);

        render_config_field(
            layout.config,
            buf,
            &self.state.selected_config_name(),
            self.state.focused_field == LaunchContextField::Config,
            false,
        );

        render_mode_field(
            layout.mode,
            buf,
            self.state.mode,
            self.state.focused_field == LaunchContextField::Mode,
            !self.state.is_mode_editable(),
        );

        // ... other fields using shared helpers

        render_launch_button(
            layout.launch,
            buf,
            None,  // No device for basic LaunchContext
            self.state.focused_field == LaunchContextField::Launch,
        );
    }
}
```

### 4. Refactor LaunchContextWithDevice widget

```rust
impl Widget for LaunchContextWithDevice<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let layout = self.calculate_layout(area);

        // Same field rendering as LaunchContext
        render_config_field(/* ... */);
        render_mode_field(/* ... */);
        render_flavor_field(/* ... */);
        render_dart_defines_field(/* ... */);

        // Different launch button with device name
        render_launch_button(
            layout.launch,
            buf,
            Some(&self.device_name),  // Show device name
            self.state.focused_field == LaunchContextField::Launch,
        );
    }
}
```

### 5. Consider consolidating into single widget

If the only difference is the launch button text, consider:

```rust
pub struct LaunchContext<'a> {
    state: &'a LaunchContextState,
    device_name: Option<&'a str>,  // Optional device name
}

impl<'a> LaunchContext<'a> {
    pub fn new(state: &'a LaunchContextState) -> Self {
        Self { state, device_name: None }
    }

    pub fn with_device(mut self, name: &'a str) -> Self {
        self.device_name = Some(name);
        self
    }
}
```

## Acceptance Criteria

1. Shared rendering logic extracted to helper functions
2. No duplicated rendering code between widgets
3. Visual appearance unchanged (verify with manual testing)
4. All widget tests pass
5. `cargo clippy` has no warnings about code duplication

## Verification

```bash
cargo fmt && cargo check && cargo test launch_context && cargo clippy -- -D warnings
```

## Manual Testing

1. Open NewSessionDialog
2. Verify all fields render correctly
3. Test focused/disabled states visually
4. Compare before/after screenshots if possible

## Notes

- This is a refactoring task - no behavior change expected
- Prioritize readability over minimal line count
- Consider if a single parameterized widget is cleaner than two separate widgets

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/tui/widgets/new_session_dialog/launch_context.rs` | Extracted shared rendering logic into 7 private helper functions, reducing duplication from 195 lines to 70 lines |

### Notable Decisions/Tradeoffs

1. **Helper Function Approach**: Instead of consolidating into a single parameterized widget, I extracted shared logic into private helper functions. This approach maintains the existing public API while eliminating duplication.

2. **Function Granularity**: Created separate helper functions for:
   - `should_show_disabled_suffix()` - Suffix logic
   - `render_config_field()` - Config dropdown rendering
   - `render_mode_field()` - Mode selector rendering
   - `render_flavor_field()` - Flavor dropdown rendering
   - `render_dart_defines_field()` - Dart defines field rendering
   - `calculate_fields_layout()` - Layout calculation
   - `render_border()` - Border block rendering
   - `render_common_fields()` - Orchestrates all common field rendering

3. **Two Widgets Retained**: Kept both `LaunchContext` and `LaunchContextWithDevice` as separate widgets since they serve distinct use cases (basic launch vs device-aware launch). The only difference in their `render()` implementation is the launch button configuration.

### Testing Performed

- `cargo fmt` - Passed
- `cargo check` - Passed
- `cargo test launch_context` - Passed (41 tests)
- `cargo clippy -- -D warnings` - Passed (no warnings)

### Risks/Limitations

1. **No Visual Changes**: This is a pure refactoring with no visual or behavioral changes. All existing tests pass without modification.
2. **Maintained API**: Public API unchanged - `LaunchContext::new()` and `LaunchContextWithDevice::new()` signatures remain the same.
3. **Code Reduction**: Eliminated approximately 125 lines of duplicated code while improving maintainability.
