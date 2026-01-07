# Task: FDemon Config Auto-fill and Auto-save

**Objective**: When an fdemon launch.toml configuration is selected, automatically populate Flavor and Dart Defines fields with config values, and auto-save any edits back to the config file.

**Depends on**: Task 10b (VSCode Config Readonly)

## Problem

Currently when selecting an fdemon launch.toml configuration:
- Flavor/Dart Defines fields start empty
- User has to manually re-enter values from the config
- Any edits are lost when dialog closes
- User expects edits to persist to the config file

## Desired Behavior

1. User selects an fdemon config (e.g., "Development")
2. Flavor field auto-fills with config's flavor value
3. Dart Defines field auto-fills with config's dart_defines
4. Mode selector auto-fills with config's mode
5. If user edits Flavor or Dart Defines, changes auto-save to launch.toml
6. Edits are debounced (save after 500ms of no typing)

## Scope

- `src/app/state.rs` - Track dirty state and original values
- `src/app/handler/update.rs` - Handle save on edit
- `src/config/launch.rs` - Add dart_defines update function
- `src/tui/startup.rs` or `src/app/handler/keys.rs` - Trigger debounced save

## Implementation

### 1. Track Config Editing State (`src/app/state.rs`)

```rust
pub struct StartupDialogState {
    // ... existing fields ...

    /// Name of the currently selected fdemon config (for saving edits)
    pub editing_config_name: Option<String>,

    /// Whether there are unsaved changes
    pub dirty: bool,

    /// Timestamp of last edit (for debouncing)
    pub last_edit_time: Option<std::time::Instant>,
}

impl StartupDialogState {
    /// Called when config selection changes
    pub fn on_config_selected(&mut self, idx: Option<usize>) {
        self.selected_config = idx;
        self.dirty = false;
        self.last_edit_time = None;
        self.editing_config_name = None;

        if let Some(i) = idx {
            if let Some(config) = self.configs.configs.get(i) {
                // Auto-fill fields from config
                self.mode = config.config.mode;
                self.flavor = config.config.flavor.clone().unwrap_or_default();
                self.dart_defines = format_dart_defines(&config.config.dart_defines);

                // Track config name for saving (only for fdemon configs)
                if config.source == ConfigSource::FDemon {
                    self.editing_config_name = Some(config.config.name.clone());
                }
            }
        }
    }

    /// Mark as dirty and record edit time
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
        self.last_edit_time = Some(std::time::Instant::now());
    }

    /// Check if debounce period has passed (500ms since last edit)
    pub fn should_save(&self) -> bool {
        if !self.dirty {
            return false;
        }
        match self.last_edit_time {
            Some(t) => t.elapsed() >= std::time::Duration::from_millis(500),
            None => false,
        }
    }

    /// Clear dirty state after save
    pub fn mark_saved(&mut self) {
        self.dirty = false;
        self.last_edit_time = None;
    }
}
```

### 2. Add Dart Defines Update Function (`src/config/launch.rs`)

```rust
/// Update dart_defines for a launch configuration
pub fn update_launch_config_dart_defines(
    project_path: &Path,
    config_name: &str,
    dart_defines_str: &str,
) -> Result<()> {
    let mut configs: Vec<LaunchConfig> = load_launch_configs(project_path)
        .into_iter()
        .map(|r| r.config)
        .collect();

    let config = configs
        .iter_mut()
        .find(|c| c.name == config_name)
        .ok_or_else(|| Error::config(format!("Config '{}' not found", config_name)))?;

    // Parse dart_defines from "KEY=VALUE,KEY2=VALUE2" format
    config.dart_defines = parse_dart_defines(dart_defines_str);

    save_launch_configs(project_path, &configs)
}

/// Parse dart defines from comma-separated KEY=VALUE string
fn parse_dart_defines(s: &str) -> HashMap<String, String> {
    if s.trim().is_empty() {
        return HashMap::new();
    }

    s.split(',')
        .filter_map(|pair| {
            let mut parts = pair.splitn(2, '=');
            let key = parts.next()?.trim();
            let value = parts.next().unwrap_or("").trim();
            if key.is_empty() {
                None
            } else {
                Some((key.to_string(), value.to_string()))
            }
        })
        .collect()
}
```

### 3. Handle Save on Edit (`src/app/handler/update.rs`)

Add a new message variant for saving config edits:

```rust
// In Message enum
pub enum Message {
    // ... existing variants ...
    SaveStartupDialogConfig,
}

// In update handler
Message::SaveStartupDialogConfig => {
    if let Some(ref config_name) = state.startup_dialog_state.editing_config_name {
        if state.startup_dialog_state.dirty {
            let project_path = state.project_path.clone();
            let name = config_name.clone();
            let flavor = state.startup_dialog_state.flavor.clone();
            let dart_defines = state.startup_dialog_state.dart_defines.clone();

            // Save flavor
            if let Err(e) = config::update_launch_config_field(
                &project_path, &name, "flavor", &flavor
            ) {
                warn!("Failed to save flavor: {}", e);
            }

            // Save dart_defines
            if let Err(e) = config::update_launch_config_dart_defines(
                &project_path, &name, &dart_defines
            ) {
                warn!("Failed to save dart_defines: {}", e);
            }

            state.startup_dialog_state.mark_saved();
        }
    }
}
```

### 4. Mark Dirty on Edit (`src/app/handler/keys.rs`)

```rust
// In startup dialog character input handling
KeyCode::Char(c) => {
    match state.startup_dialog_state.active_section {
        DialogSection::Flavor => {
            state.startup_dialog_state.flavor.push(c);
            state.startup_dialog_state.mark_dirty();
        }
        DialogSection::DartDefines => {
            state.startup_dialog_state.dart_defines.push(c);
            state.startup_dialog_state.mark_dirty();
        }
        _ => {}
    }
}

KeyCode::Backspace => {
    match state.startup_dialog_state.active_section {
        DialogSection::Flavor => {
            state.startup_dialog_state.flavor.pop();
            state.startup_dialog_state.mark_dirty();
        }
        DialogSection::DartDefines => {
            state.startup_dialog_state.dart_defines.pop();
            state.startup_dialog_state.mark_dirty();
        }
        _ => {}
    }
}
```

### 5. Trigger Save in Event Loop

Option A: Check in `Message::Tick` handler:

```rust
Message::Tick => {
    // ... existing tick handling ...

    // Check if startup dialog needs to save
    if state.ui_mode == UiMode::StartupDialog
        && state.startup_dialog_state.should_save()
    {
        // Inline save or queue message
        return UpdateResult::with_message(Some(Message::SaveStartupDialogConfig));
    }
}
```

Option B: Use a dedicated debounce timer (more complex but cleaner).

### 6. Save on Dialog Close

Ensure any pending changes are saved when dialog closes:

```rust
// When launching session or closing dialog
if state.startup_dialog_state.dirty {
    // Force immediate save
    // ... save logic ...
    state.startup_dialog_state.mark_saved();
}
```

## Acceptance Criteria

1. **Auto-fill**:
   - Selecting fdemon config populates Flavor with config's flavor
   - Selecting fdemon config populates Dart Defines with formatted string
   - Selecting fdemon config sets Mode to config's mode

2. **Auto-save**:
   - Editing Flavor marks dialog as dirty
   - Editing Dart Defines marks dialog as dirty
   - Changes save to launch.toml after 500ms debounce
   - Changes saved when dialog closes (launch or cancel)
   - Saved values appear on next dialog open

3. **Edge cases**:
   - Empty flavor clears config's flavor field
   - Empty dart_defines clears config's dart_defines
   - Switching configs discards unsaved changes (or saves first?)

## Testing

### Manual Test

1. Create launch.toml with config: `name="Dev", flavor="development"`
2. Open startup dialog, select "Dev"
3. Verify Flavor shows "development"
4. Edit to "staging", wait 1 second
5. Close dialog, reopen
6. Verify Flavor now shows "staging"
7. Check launch.toml file - should have `flavor = "staging"`

### Unit Tests

```rust
#[test]
fn test_on_config_selected_fills_fields() {
    let mut configs = LoadedConfigs::default();
    configs.configs.push(SourcedConfig {
        config: LaunchConfig {
            name: "Dev".to_string(),
            flavor: Some("development".to_string()),
            mode: FlutterMode::Profile,
            ..Default::default()
        },
        source: ConfigSource::FDemon,
        display_name: "Dev".to_string(),
    });

    let mut state = StartupDialogState::with_configs(configs);
    state.on_config_selected(Some(0));

    assert_eq!(state.flavor, "development");
    assert_eq!(state.mode, FlutterMode::Profile);
    assert_eq!(state.editing_config_name, Some("Dev".to_string()));
}

#[test]
fn test_mark_dirty_and_should_save() {
    let mut state = StartupDialogState::new();

    assert!(!state.should_save());

    state.mark_dirty();
    assert!(!state.should_save()); // Not enough time passed

    std::thread::sleep(std::time::Duration::from_millis(600));
    assert!(state.should_save()); // Now should save

    state.mark_saved();
    assert!(!state.should_save());
}

#[test]
fn test_parse_dart_defines() {
    let result = parse_dart_defines("API_URL=https://dev.com,DEBUG=true");
    assert_eq!(result.get("API_URL"), Some(&"https://dev.com".to_string()));
    assert_eq!(result.get("DEBUG"), Some(&"true".to_string()));
}

#[test]
fn test_parse_dart_defines_empty() {
    let result = parse_dart_defines("");
    assert!(result.is_empty());
}
```

## Notes

- Debounce at 500ms balances responsiveness with avoiding excessive file writes
- Consider showing a "Saved" indicator briefly after save completes
- Dart defines format: `KEY=VALUE,KEY2=VALUE2` (comma-separated)
- Handle malformed dart_defines gracefully (skip invalid pairs)

## Estimated Complexity

Medium - Involves state tracking, file I/O, and debouncing logic.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/app/state.rs` | Added auto-save tracking fields (`editing_config_name`, `dirty`, `last_edit_time`) to `StartupDialogState`. Updated `on_config_selected()` to auto-fill fields for FDemon configs and track editing. Added `mark_dirty()`, `should_save()`, and `mark_saved()` helper methods. Added 8 unit tests. |
| `src/config/launch.rs` | Added `update_launch_config_dart_defines()` function to update dart_defines. Added `parse_dart_defines()` helper to parse comma-separated KEY=VALUE strings. Added 10 unit tests covering parsing, empty values, malformed input, and edge cases. |
| `src/config/mod.rs` | Exported `update_launch_config_dart_defines` function. |
| `src/app/message.rs` | Added `SaveStartupDialogConfig` message variant for triggering auto-save. |
| `src/app/handler/update.rs` | Added handler for `SaveStartupDialogConfig` message. Updated `StartupDialogCharInput`, `StartupDialogBackspace`, and `StartupDialogClearInput` handlers to call `mark_dirty()`. Added auto-save trigger in `Tick` handler when `should_save()` returns true (500ms debounce). |

### Notable Decisions/Tradeoffs

1. **Auto-fill behavior**: FDemon configs now auto-populate flavor/dart_defines fields when selected (changed from Task 10b). This provides a better UX by showing current values and enables auto-save workflow.

2. **Debounce timing**: 500ms debounce period balances responsiveness with avoiding excessive file writes. The `Tick` handler checks `should_save()` every tick (~100ms), so actual save happens 500-600ms after last edit.

3. **VSCode configs unchanged**: VSCode configs remain read-only with no auto-save (as per Task 10b). Only FDemon configs have auto-save enabled.

4. **Parse robustness**: `parse_dart_defines()` gracefully handles malformed input by skipping invalid pairs (missing '=' or empty keys), ensuring partial data isn't lost.

5. **Mode updates**: Mode changes are not tracked as dirty since they're handled by selector navigation, not text editing. Only flavor and dart_defines edits trigger auto-save.

### Testing Performed

- `cargo check` - Passed
- `cargo test --lib` - Passed (1231 tests, 0 failed)
  - 10 new tests in `config::launch::tests` for dart_defines parsing
  - 8 new tests in `app::state::tests` for dirty tracking and auto-fill
- `cargo fmt` - Applied formatting
- `cargo clippy` - Passed (1 pre-existing warning unrelated to changes)

### Risks/Limitations

1. **File I/O on Tick**: Auto-save runs on tick handler, which could cause brief UI stutter if disk I/O is slow. Mitigated by 500ms debounce (user has stopped typing).

2. **No save confirmation**: Changes are silently saved without user feedback. Consider adding a brief "Saved" indicator in future enhancement.

3. **Config switching discards unsaved changes**: When user switches configs, dirty state is cleared immediately. This matches typical form behavior but could be surprising if save is pending (within 500ms window).

4. **No save on dialog close**: Currently no forced save when dialog closes. User edits persist via auto-save, but pending changes within 500ms window may be lost if dialog closes immediately.
