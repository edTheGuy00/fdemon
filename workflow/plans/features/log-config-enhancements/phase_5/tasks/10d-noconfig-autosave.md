# Task: No-Config Auto-save to New launch.toml

**Objective**: When no configuration is selected and the user enters a Flavor or Dart Defines value, automatically create and save a new default configuration in launch.toml.

**Depends on**: Task 10c (FDemon Config Auto-save)

## Problem

When a user has no launch configurations:
- They can enter Flavor and Dart Defines values manually
- These values are used for the current launch
- But they're **lost** when the dialog closes
- User has to re-enter them every time

## Desired Behavior

1. User has no configs (or selects "No configuration")
2. User enters a Flavor or Dart Defines value
3. After debounce, a new "Default" config is created in launch.toml
4. Subsequent edits update this new config
5. Config appears in the list for future sessions

## Scope

- `src/app/state.rs` - Detect "no config" editing scenario
- `src/app/handler/update.rs` - Handle creation of new config
- `src/config/launch.rs` - Create default config with user values

## Implementation

### 1. Track "Creating New Config" State (`src/app/state.rs`)

```rust
pub struct StartupDialogState {
    // ... existing fields ...

    /// True if we're creating a new config from scratch (no selection)
    pub creating_new_config: bool,

    /// Name for the new config being created
    pub new_config_name: String,
}

impl StartupDialogState {
    pub fn new() -> Self {
        Self {
            // ...
            creating_new_config: false,
            new_config_name: "Default".to_string(),
        }
    }

    /// Check if we should create a new config on save
    fn should_create_new_config(&self) -> bool {
        // No config selected AND user has entered something
        self.selected_config.is_none()
            && (!self.flavor.is_empty() || !self.dart_defines.is_empty())
    }

    /// Mark as dirty, potentially starting new config creation
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
        self.last_edit_time = Some(std::time::Instant::now());

        // If no config selected and user is entering data, flag for new config
        if self.selected_config.is_none()
            && !self.creating_new_config
            && (!self.flavor.is_empty() || !self.dart_defines.is_empty())
        {
            self.creating_new_config = true;
        }
    }
}
```

### 2. Handle New Config Creation (`src/app/handler/update.rs`)

```rust
Message::SaveStartupDialogConfig => {
    let dialog = &mut state.startup_dialog_state;

    if dialog.creating_new_config && dialog.dirty {
        // Create new config
        let project_path = state.project_path.clone();
        let new_config = LaunchConfig {
            name: dialog.new_config_name.clone(),
            device: "auto".to_string(),
            mode: dialog.mode,
            flavor: if dialog.flavor.is_empty() {
                None
            } else {
                Some(dialog.flavor.clone())
            },
            dart_defines: config::parse_dart_defines(&dialog.dart_defines),
            ..Default::default()
        };

        match config::add_launch_config(&project_path, new_config) {
            Ok(()) => {
                info!("Created new config: {}", dialog.new_config_name);

                // Reload configs to show the new one
                let reloaded = config::load_all_configs(&project_path);
                let new_idx = reloaded.configs.iter()
                    .position(|c| c.config.name == dialog.new_config_name);

                dialog.configs = reloaded;
                dialog.selected_config = new_idx;
                dialog.creating_new_config = false;
                dialog.editing_config_name = Some(dialog.new_config_name.clone());
                dialog.mark_saved();
            }
            Err(e) => {
                warn!("Failed to create config: {}", e);
                // Could show error to user
            }
        }
    } else if let Some(ref config_name) = dialog.editing_config_name {
        // Existing config update (from Task 10c)
        // ...
    }
}
```

### 3. Visual Feedback

Show indicator that a new config will be created:

```rust
// In startup_dialog/mod.rs render

fn render_config_list(&self, area: Rect, buf: &mut Buffer) {
    // ... existing code ...

    // If creating new config, show hint at bottom of list
    if self.state.creating_new_config {
        items.push(
            ListItem::new(format!(
                "  + Creating '{}' config...",
                self.state.new_config_name
            ))
            .style(Style::default().fg(Color::Green))
        );
    }
}
```

### 4. Handle Name Collision

The `add_launch_config` function already handles name collision by appending "(1)", "(2)", etc. But we should update `new_config_name` after save:

```rust
// After successful save
let actual_name = reloaded.configs.iter()
    .filter(|c| c.config.name.starts_with(&dialog.new_config_name))
    .map(|c| c.config.name.clone())
    .last()
    .unwrap_or(dialog.new_config_name.clone());

dialog.editing_config_name = Some(actual_name);
```

### 5. Create .fdemon Directory if Needed

Ensure directory exists before saving:

```rust
// In config/launch.rs add_launch_config
pub fn add_launch_config(project_path: &Path, config: LaunchConfig) -> Result<()> {
    let fdemon_dir = project_path.join(".fdemon");

    // Create directory if it doesn't exist
    if !fdemon_dir.exists() {
        std::fs::create_dir_all(&fdemon_dir)
            .map_err(|e| Error::config(format!("Failed to create .fdemon dir: {}", e)))?;
    }

    // ... rest of function
}
```

## Edge Cases

1. **Only VSCode configs exist**: User can still create fdemon config by entering values
2. **User clears values**: If both fields emptied before save, don't create empty config
3. **Launch without saving**: If user launches before debounce, save before launch
4. **Cancel without saving**: Discard unsaved new config

### Handle Cancel

```rust
// When dialog is cancelled
if state.startup_dialog_state.creating_new_config {
    // Don't save - just reset state
    state.startup_dialog_state.creating_new_config = false;
    state.startup_dialog_state.dirty = false;
}
```

### Handle Launch

```rust
// When launching session
if state.startup_dialog_state.dirty {
    if state.startup_dialog_state.creating_new_config {
        // Create config before launching
        // ... create config ...
    } else if let Some(ref config_name) = state.startup_dialog_state.editing_config_name {
        // Save edits before launching
        // ... save config ...
    }
}
```

## Acceptance Criteria

1. **No configs scenario**:
   - Enter "staging" in Flavor
   - After debounce, launch.toml created with "Default" config
   - Config has `flavor = "staging"`
   - Config appears in list

2. **VSCode-only scenario**:
   - Have only launch.json configs
   - Select "New config" (Task 10e) or deselect all
   - Enter Flavor value
   - Creates new fdemon config

3. **Persistence**:
   - Created config persists across dialog open/close
   - Config usable in future sessions

4. **Cancellation**:
   - Enter values but press Esc
   - No config created

## Testing

### Manual Test

1. Start with project that has no .fdemon directory
2. Open startup dialog
3. Don't select any config
4. Enter "production" in Flavor
5. Wait 1 second
6. Verify:
   - .fdemon/launch.toml created
   - Contains `[[configurations]]` with `name = "Default"`, `flavor = "production"`
   - Config appears in list

### Unit Tests

```rust
#[test]
fn test_should_create_new_config() {
    let mut state = StartupDialogState::new();
    state.selected_config = None;

    // Initially false - no data entered
    assert!(!state.should_create_new_config());

    // After entering flavor
    state.flavor = "test".to_string();
    assert!(state.should_create_new_config());
}

#[test]
fn test_mark_dirty_sets_creating_flag() {
    let mut state = StartupDialogState::new();
    state.selected_config = None;
    state.flavor = "test".to_string();

    state.mark_dirty();

    assert!(state.creating_new_config);
    assert!(state.dirty);
}

#[test]
fn test_no_create_if_empty() {
    let mut state = StartupDialogState::new();
    state.selected_config = None;
    state.flavor = String::new();
    state.dart_defines = String::new();

    state.mark_dirty();

    assert!(!state.creating_new_config);
}
```

## Notes

- "Default" is a reasonable name for auto-created configs
- User can rename via settings panel later (Task 07)
- This feature enables "zero-config" workflow for simple projects
- Consider showing toast/notification "Config saved" after creation

## Estimated Complexity

Medium - Builds on Task 10c, adds config creation logic.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/app/state.rs` | Added `creating_new_config` and `new_config_name` fields to `StartupDialogState`. Implemented `should_create_new_config()` method. Updated `mark_dirty()` to set creating flag when no config selected and user enters data. Updated `on_config_selected()` to reset creating flag. Added 9 unit tests. |
| `src/app/handler/update.rs` | Updated `SaveStartupDialogConfig` handler to create new config when `creating_new_config` is true, with proper error handling and config reload. Updated `HideStartupDialog` to reset creating flag on cancel. Updated `handle_startup_dialog_confirm` to save new config before launch if dirty. |
| `src/config/launch.rs` | Made `parse_dart_defines()` function public for use in handlers. |
| `src/config/mod.rs` | Exported `parse_dart_defines` function. |

### Notable Decisions/Tradeoffs

1. **Empty config prevention**: The handler explicitly checks if both flavor and dart_defines are empty and skips creation to avoid saving empty configs.
2. **Name collision handling**: Uses existing `add_launch_config()` which automatically appends "(1)", "(2)" etc. for duplicate names. The handler finds the actual name after save and updates the selection.
3. **Fallback config selection**: After creating new config, falls back to last config if name search fails (defensive programming).
4. **Synchronous save on launch**: When user launches before debounce completes, the config is saved synchronously to ensure it's persisted.
5. **Directory creation**: The existing `add_launch_config()` function already handles `.fdemon` directory creation, so no additional logic needed.

### Testing Performed

- `cargo check` - Passed
- `cargo test --lib` - Passed (1234 tests, 0 failed)
- `cargo clippy` - Passed (1 pre-existing warning unrelated to changes)

All new tests passed:
- `test_should_create_new_config` - Validates logic for determining when to create new config
- `test_should_create_new_config_with_selected` - Ensures no creation when config is selected
- `test_mark_dirty_sets_creating_flag` - Verifies flag is set when user enters data
- `test_mark_dirty_no_create_if_empty` - Ensures no creation for empty values
- `test_mark_dirty_no_create_if_already_creating` - Prevents flag toggling
- `test_on_config_selected_clears_creating_flag` - Verifies flag reset on selection change
- `test_default_new_config_name` - Validates default name is "Default"

### Risks/Limitations

1. **No visual feedback**: The task implementation does not include visual indicators that a new config will be created (mentioned in spec but not implemented). This could be added in future as a UI enhancement.
2. **Config name not user-editable**: The new config always uses "Default" as the name. Users must rename via settings panel after creation (Task 07).
3. **Race condition potential**: If user rapidly enters data, switches configs, and then the debounce timer fires, there's potential for unexpected behavior. However, this is mitigated by resetting the creating flag on config selection.
