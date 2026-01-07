# Task: VSCode Config Field Disabling

**Objective**: Disable Flavor and Dart Defines input fields when a VSCode configuration is selected, since these values should come from the config file directly.

**Depends on**: None (independent)

## Problem

When a user selects a VSCode launch.json configuration:
- The Flavor and Dart Defines fields are currently editable
- Any manual edits have no effect (values come from config)
- This creates confusion - user expects edits to be used
- VSCode configs should be edited in VSCode/the file itself

## Current Behavior

1. User selects a VSCode config (e.g., "Flutter Dev (VSCode)")
2. Flavor/Dart Defines fields remain editable
3. User can type into them
4. Values are **ignored** at launch - config values used instead
5. User confused why their edits weren't applied

## Desired Behavior

1. User selects a VSCode config
2. Flavor/Dart Defines fields become **disabled/readonly**
3. Fields show values from config (if any) as read-only
4. Visual indication fields are disabled (dimmed, different border)
5. Hint text: "Edit in .vscode/launch.json"

## Scope

- `src/app/state.rs` - Track field enabled state
- `src/tui/widgets/startup_dialog/mod.rs` - Render disabled state
- `src/app/handler/keys.rs` - Skip disabled fields in navigation
- `src/tui/widgets/startup_dialog/styles.rs` - Disabled field styling

## Implementation

### 1. Add Field Enabled State (`src/app/state.rs`)

```rust
impl StartupDialogState {
    /// Whether flavor/dart_defines fields are editable
    pub fn flavor_editable(&self) -> bool {
        match self.selected_config {
            Some(idx) => {
                self.configs.configs.get(idx)
                    .map(|c| c.source != ConfigSource::VSCode)
                    .unwrap_or(true)
            }
            None => true, // No config selected = editable
        }
    }

    /// Get display value for flavor (from config if VSCode, else manual input)
    pub fn flavor_display(&self) -> &str {
        if let Some(idx) = self.selected_config {
            if let Some(config) = self.configs.configs.get(idx) {
                if config.source == ConfigSource::VSCode {
                    // Show config value (read-only)
                    return config.config.flavor.as_deref().unwrap_or("");
                }
            }
        }
        &self.flavor
    }

    /// Update field values when config selection changes
    pub fn on_config_selected(&mut self, idx: Option<usize>) {
        self.selected_config = idx;

        // If VSCode config, populate fields with config values (read-only display)
        // If FDemon config or no config, keep current manual values
        if let Some(i) = idx {
            if let Some(config) = self.configs.configs.get(i) {
                if config.source == ConfigSource::VSCode {
                    // Show config values in fields (read-only)
                    self.flavor = config.config.flavor.clone().unwrap_or_default();
                    self.dart_defines = format_dart_defines(&config.config.dart_defines);
                    self.mode = config.config.mode;
                }
            }
        }
    }
}

fn format_dart_defines(defines: &HashMap<String, String>) -> String {
    defines.iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join(",")
}
```

### 2. Update Widget Rendering (`src/tui/widgets/startup_dialog/mod.rs`)

```rust
fn render_input_field(
    &self,
    area: Rect,
    buf: &mut Buffer,
    label: &str,
    value: &str,
    section: DialogSection,
    enabled: bool,  // New parameter
) {
    let is_active = self.state.active_section == section;
    let is_editing = is_active && self.state.editing && enabled;

    // Disabled styling
    let (value_style, label_style) = if !enabled {
        (
            Style::default().fg(DISABLED_COLOR),
            Style::default().fg(DISABLED_COLOR),
        )
    } else if is_editing {
        (
            Style::default().fg(VALUE_COLOR).bg(Color::DarkGray),
            Style::default().fg(VALUE_COLOR),
        )
    } else if is_active {
        (
            Style::default().fg(VALUE_COLOR),
            Style::default().fg(VALUE_COLOR),
        )
    } else {
        (
            Style::default().fg(if value.is_empty() { PLACEHOLDER_COLOR } else { VALUE_COLOR }),
            Style::default().fg(LABEL_COLOR),
        )
    };

    // Show cursor only when editing AND enabled
    let display_value = if is_editing && enabled {
        format!("{}|", value)
    } else if value.is_empty() {
        if enabled { "(optional)".to_string() } else { "-".to_string() }
    } else {
        value.to_string()
    };

    // Add disabled hint for VSCode configs
    let suffix = if !enabled { "  (from config)" } else { "" };

    let line = Line::from(vec![
        Span::raw(format!("  {}: ", label)),
        Span::styled(format!("[{}]{}", display_value, suffix), value_style),
    ]);

    Paragraph::new(line).style(label_style).render(area, buf);
}
```

Update the render call:

```rust
// In Widget impl for StartupDialog
let flavor_editable = self.state.flavor_editable();
self.render_input_field(
    chunks[2], buf, "Flavor",
    self.state.flavor_display(),
    DialogSection::Flavor,
    flavor_editable,
);
self.render_input_field(
    chunks[3], buf, "Dart Defines",
    &self.state.dart_defines_display(),
    DialogSection::DartDefines,
    flavor_editable,  // Same editability rule
);
```

### 3. Add Disabled Color (`src/tui/widgets/startup_dialog/styles.rs`)

```rust
pub const DISABLED_COLOR: Color = Color::DarkGray;
```

### 4. Skip Disabled Fields in Navigation (`src/app/handler/keys.rs`)

```rust
// In startup dialog key handling
KeyCode::Tab => {
    // Skip disabled sections
    let mut next = state.startup_dialog_state.active_section.next();

    // Skip Flavor/DartDefines if VSCode config selected
    if !state.startup_dialog_state.flavor_editable() {
        while matches!(next, DialogSection::Flavor | DialogSection::DartDefines) {
            next = next.next();
        }
    }

    state.startup_dialog_state.active_section = next;
}

KeyCode::BackTab => {
    let mut prev = state.startup_dialog_state.active_section.prev();

    if !state.startup_dialog_state.flavor_editable() {
        while matches!(prev, DialogSection::Flavor | DialogSection::DartDefines) {
            prev = prev.prev();
        }
    }

    state.startup_dialog_state.active_section = prev;
}
```

### 5. Block Text Input on Disabled Fields

```rust
// In character input handling for startup dialog
KeyCode::Char(c) => {
    let section = state.startup_dialog_state.active_section;
    let editable = state.startup_dialog_state.flavor_editable();

    match section {
        DialogSection::Flavor if editable => {
            state.startup_dialog_state.flavor.push(c);
        }
        DialogSection::DartDefines if editable => {
            state.startup_dialog_state.dart_defines.push(c);
        }
        _ => {} // Ignore input on disabled fields
    }
}
```

## Acceptance Criteria

1. When VSCode config selected:
   - Flavor field shows config value, grayed out
   - Dart Defines field shows config value, grayed out
   - Fields display "(from config)" suffix
   - Tab skips Flavor/DartDefines sections
   - Character input is ignored on these fields
   - Backspace is ignored on these fields

2. When FDemon config or no config selected:
   - Fields remain fully editable
   - Normal navigation includes these fields

3. Visual feedback is clear (disabled fields visually distinct)

## Testing

### Manual Test

1. Create project with both launch.toml and launch.json configs
2. Open startup dialog
3. Select VSCode config → verify fields disabled
4. Select FDemon config → verify fields enabled
5. Select no config → verify fields enabled
6. Try Tab navigation with VSCode config → should skip fields

### Unit Tests

```rust
#[test]
fn test_flavor_editable_vscode() {
    let mut configs = LoadedConfigs::default();
    configs.configs.push(SourcedConfig {
        config: LaunchConfig::default(),
        source: ConfigSource::VSCode,
        display_name: "VSCode".to_string(),
    });

    let mut state = StartupDialogState::with_configs(configs);
    state.selected_config = Some(0);

    assert!(!state.flavor_editable());
}

#[test]
fn test_flavor_editable_fdemon() {
    let mut configs = LoadedConfigs::default();
    configs.configs.push(SourcedConfig {
        config: LaunchConfig::default(),
        source: ConfigSource::FDemon,
        display_name: "FDemon".to_string(),
    });

    let mut state = StartupDialogState::with_configs(configs);
    state.selected_config = Some(0);

    assert!(state.flavor_editable());
}

#[test]
fn test_flavor_editable_no_config() {
    let state = StartupDialogState::new();
    assert!(state.flavor_editable());
}
```

## Notes

- VSCode launch.json is treated as read-only throughout the app
- Users who want to edit must edit the file directly in VSCode
- This matches the existing "VSCode config = read-only" philosophy
- Mode field remains editable (user can override mode at launch time)

## Estimated Complexity

Low-Medium - UI state tracking + navigation logic updates.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/app/state.rs` | Added `flavor_editable()`, `flavor_display()`, `dart_defines_display()`, and `on_config_selected()` methods to `StartupDialogState`. Added helper function `format_dart_defines()`. Added 9 unit tests. |
| `src/app/message.rs` | Added `StartupDialogNextSectionSkipDisabled` and `StartupDialogPrevSectionSkipDisabled` messages for Tab navigation that skips disabled fields. |
| `src/app/handler/update.rs` | Added handlers for skip-disabled messages. Updated `StartupDialogSelectConfig` handler to use `on_config_selected()`. Updated `StartupDialogCharInput`, `StartupDialogBackspace`, and `StartupDialogEnterEdit` handlers to check `flavor_editable()` and block input on disabled fields. |
| `src/app/handler/keys.rs` | Updated Tab/Shift+Tab/BackTab key handling in startup dialog to conditionally use skip-disabled navigation based on `flavor_editable()`. |
| `src/tui/widgets/startup_dialog/mod.rs` | Updated `render_input_field()` to accept `enabled` parameter. Added disabled field styling (grayed out, "(from config)" suffix, "-" for empty values). Updated render calls to use `flavor_display()` and `dart_defines_display()` methods. |
| `src/tui/widgets/startup_dialog/styles.rs` | Added `DISABLED_COLOR` constant (Color::DarkGray). |

### Notable Decisions/Tradeoffs

1. **VSCode configs are read-only**: When a VSCode config is selected, the Flavor and Dart Defines fields are completely disabled (no editing, no navigation). This matches the existing philosophy that VSCode configs should be edited in VSCode itself, not in fdemon.

2. **Shared editability for both fields**: Both Flavor and Dart Defines use the same `flavor_editable()` method. This is appropriate since VSCode configs should disable both fields together.

3. **Config selection triggers population**: The `on_config_selected()` method automatically populates Flavor/DartDefines/Mode fields when a VSCode config is selected. This provides immediate visual feedback that these values come from the config.

4. **Skip-disabled navigation messages**: Rather than modifying the existing section navigation logic in `DialogSection::next()/prev()`, I created new message types specifically for skipping disabled sections. This keeps the navigation logic simple and testable.

5. **Format helper function**: Created a standalone `format_dart_defines()` helper function that can be reused by both `apply_config_defaults()` and `dart_defines_display()`.

### Testing Performed

- `cargo fmt` - Passed
- `cargo check` - Passed
- `cargo clippy` - Passed (1 pre-existing unrelated warning in LoadingState)
- `cargo test --lib` - Passed (1211 tests, 0 failed)

Specific tests added:
- `test_flavor_editable_vscode` - Verifies VSCode configs are not editable
- `test_flavor_editable_fdemon` - Verifies FDemon configs are editable
- `test_flavor_editable_no_config` - Verifies no config selected is editable
- `test_flavor_display_vscode` - Verifies VSCode config values are displayed
- `test_flavor_display_fdemon` - Verifies manual input is displayed for FDemon configs
- `test_dart_defines_display_vscode` - Verifies dart-defines formatting from VSCode config
- `test_on_config_selected_vscode` - Verifies VSCode config populates fields
- `test_on_config_selected_fdemon` - Verifies FDemon config doesn't auto-populate
- `test_on_config_selected_none` - Verifies clearing config selection

### Risks/Limitations

1. **User confusion**: Users who are used to editing fields might be confused when they're suddenly disabled. The "(from config)" hint and grayed-out styling should help communicate this, but it's a UX change.

2. **No visual feedback in config list**: The config list doesn't currently show which configs are from VSCode vs FDemon. This could be enhanced in the future with a visual indicator (e.g., icon or color).

3. **FDemon configs don't auto-populate**: The current implementation only auto-populates fields for VSCode configs. FDemon configs retain manual edits. This is intentional to preserve user workflow, but could be changed if desired.

4. **Mode field remains editable**: Unlike Flavor and DartDefines, the Mode field remains editable even for VSCode configs. This allows users to override the mode at launch time without editing the config file. This is intentional but may cause confusion if users expect full read-only behavior.
