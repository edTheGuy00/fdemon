# Plan: Entry Point / Target File Support

## TL;DR

Flutter supports specifying a custom entry point via `--target lib/main_develop.dart` (mapped from VSCode's `program` field). While the config module already parses this, the NewSessionDialog launch flow does NOT pass `entry_point` to the spawned session. Additionally, we need UI support in the Launch Context pane to display and select entry points, with automatic discovery of Dart files containing `main()` functions.

---

## Background

In Flutter development, projects often have multiple entry points for different environments:

```
lib/
├── main.dart           # Production
├── main_develop.dart   # Development
├── main_staging.dart   # Staging
└── main_test.dart      # Testing
```

Users configure these in VSCode's `launch.json`:

```json
{
  "name": "My App (develop)",
  "type": "dart",
  "request": "launch",
  "program": "lib/main_develop.dart",
  "args": ["--flavor", "develop"]
}
```

### Current State

1. **Config Parsing (WORKS):** `config/vscode.rs` correctly parses the `program` field and maps it to `LaunchConfig.entry_point`
2. **Flutter Args Building (WORKS):** `LaunchConfig.build_flutter_args()` correctly adds `-t <entry_point>` to the command
3. **TOML Serialization (WORKS):** `LaunchConfig.entry_point` has proper serde attributes for `.fdemon/launch.toml`
4. **Launch Flow (BROKEN):** `handle_launch()` in `app/handler/new_session/launch_context.rs` builds a new `LaunchConfig` but does NOT copy the `entry_point` field from the selected config
5. **Field Update (BROKEN):** `update_launch_config_field()` in `config/launch.rs` does NOT handle `entry_point` field - returns "Unknown field" error
6. **UI (MISSING):** No field in Launch Context pane to display or edit entry point

### Impact

When a user selects a VSCode configuration with a custom `program` field:
- The entry point is parsed and stored correctly
- But when launching, the `entry_point` is lost because `handle_launch()` builds a fresh `LaunchConfig` without copying this field
- Flutter runs with `lib/main.dart` instead of the user's intended target file

---

## Affected Modules

### Phase 1 (Core Fix)
- `src/app/handler/new_session/launch_context.rs` - Pass `entry_point` when building `LaunchConfig`
- `src/app/new_session_dialog/types.rs` - Add `entry_point` to `LaunchParams`
- `src/app/new_session_dialog/state.rs` - Include `entry_point` in `build_launch_params()` and `LaunchContextState`
- `src/config/launch.rs` - Add `entry_point` case to `update_launch_config_field()` for auto-save support

### Phase 2 (Entry Point Discovery)
- `src/core/discovery.rs` - **ADD**: `discover_entry_points()` function to find Dart files with `main()`

### Phase 3 (UI Support)
- `src/app/new_session_dialog/types.rs` - Add `EntryPoint` to `LaunchContextField` enum, add `EntryPoint` to `FuzzyModalType`
- `src/app/new_session_dialog/state.rs` - Add `entry_point` field and discovery state to `LaunchContextState`
- `src/tui/widgets/new_session_dialog/launch_context.rs` - Add Entry Point field rendering
- `src/app/handler/new_session/launch_context.rs` - Add handler for entry point selection with auto-save

---

## Development Phases

### Phase 1: Core Fix (Critical Path)

**Goal**: Ensure `entry_point` flows from selected config through to Flutter process spawn

**Duration**: 1-2 hours

#### Steps

1. **Add `entry_point` to `LaunchParams`**
   - In `src/app/new_session_dialog/types.rs`:
     ```rust
     pub struct LaunchParams {
         pub device_id: String,
         pub mode: crate::config::FlutterMode,
         pub flavor: Option<String>,
         pub dart_defines: Vec<String>,
         pub config_name: Option<String>,
         pub entry_point: Option<std::path::PathBuf>,  // NEW
     }
     ```

2. **Add `entry_point` to `LaunchContextState`**
   - In `src/app/new_session_dialog/state.rs`:
     ```rust
     pub struct LaunchContextState {
         // ... existing fields ...
         /// Entry point (from config or user override)
         pub entry_point: Option<std::path::PathBuf>,
     }
     ```

3. **Update `select_config()` to apply entry_point**
   - When selecting a config, copy `entry_point` to state (like flavor and dart_defines)

4. **Update `build_launch_params()` to include `entry_point`**
   - Extract `entry_point` from `LaunchContextState`

5. **Update `handle_launch()` to use `entry_point` from params**
   - In `src/app/handler/new_session/launch_context.rs`, include `entry_point` in the `LaunchConfig`

6. **Add `entry_point` case to `update_launch_config_field()`**
   - In `src/config/launch.rs`, add handling for `entry_point` field:
     ```rust
     // In update_launch_config_field() match block:
     "entry_point" => {
         config.entry_point = if value.is_empty() {
             None
         } else {
             Some(PathBuf::from(value))
         };
     }
     ```
   - This enables auto-save when entry point is changed in the UI

**Milestone**: VSCode configs with `program` field result in correct `-t` argument to Flutter.

---

### Phase 2: Entry Point Discovery

**Goal**: Automatically find Dart files with `main()` function in `lib/` directory

**Duration**: 1-2 hours

#### Steps

1. **Create `discover_entry_points()` function**
   - Location: `src/core/discovery.rs`
   - Scan `lib/` directory recursively for `.dart` files
   - Parse each file to check for `void main(` or `main(` pattern
   - Return list of relative paths (e.g., `lib/main.dart`, `lib/main_dev.dart`)

   ```rust
   /// Discovers Dart files containing a main() function in the lib/ directory.
   ///
   /// Returns paths relative to project root, sorted alphabetically.
   /// Common entry points like main.dart appear first.
   pub fn discover_entry_points(project_path: &Path) -> Vec<PathBuf> {
       let lib_path = project_path.join("lib");
       if !lib_path.exists() {
           return Vec::new();
       }

       let mut entry_points = Vec::new();

       // Walk lib/ directory
       for entry in walkdir::WalkDir::new(&lib_path)
           .into_iter()
           .filter_map(|e| e.ok())
           .filter(|e| e.path().extension() == Some(std::ffi::OsStr::new("dart")))
       {
           if has_main_function(entry.path()) {
               if let Ok(rel_path) = entry.path().strip_prefix(project_path) {
                   entry_points.push(rel_path.to_path_buf());
               }
           }
       }

       // Sort with main.dart first, then alphabetically
       entry_points.sort_by(|a, b| {
           let a_is_main = a.file_name() == Some(std::ffi::OsStr::new("main.dart"));
           let b_is_main = b.file_name() == Some(std::ffi::OsStr::new("main.dart"));
           match (a_is_main, b_is_main) {
               (true, false) => std::cmp::Ordering::Less,
               (false, true) => std::cmp::Ordering::Greater,
               _ => a.cmp(b),
           }
       });

       entry_points
   }

   /// Check if a Dart file contains a main() function.
   fn has_main_function(path: &Path) -> bool {
       if let Ok(content) = std::fs::read_to_string(path) {
           // Look for main function declaration patterns:
           // - void main(
           // - main(
           // - Future<void> main(
           // Handle whitespace variations
           let patterns = [
               r"void\s+main\s*\(",
               r"Future<void>\s+main\s*\(",
               r"^\s*main\s*\(",  // main( at start of line
           ];

           for pattern in patterns {
               if regex::Regex::new(pattern).unwrap().is_match(&content) {
                   return true;
               }
           }
       }
       false
   }
   ```

2. **Add dependency if needed**
   - `walkdir` crate for recursive directory traversal (or use std::fs)

**Milestone**: Can programmatically discover all entry points in a Flutter project.

---

### Phase 3: UI Support in Launch Context

**Goal**: Display and allow editing of entry point in the NewSessionDialog UI

**Duration**: 3-4 hours

#### Steps

1. **Add `EntryPoint` to `LaunchContextField` enum**
   - In `src/app/new_session_dialog/types.rs`:
     ```rust
     pub enum LaunchContextField {
         Config,
         Mode,
         Flavor,
         EntryPoint,  // NEW - between Flavor and DartDefines
         DartDefines,
         Launch,
     }
     ```
   - Update `next()` and `prev()` methods

2. **Add entry point state to `LaunchContextState`**
   - In `src/app/new_session_dialog/state.rs`:
     ```rust
     pub struct LaunchContextState {
         // ... existing fields ...

         /// Entry point (from config or user override)
         pub entry_point: Option<PathBuf>,

         /// Available entry points discovered from project
         pub available_entry_points: Vec<PathBuf>,
     }
     ```
   - Add methods:
     - `entry_point_display()` - Returns display string
     - `set_entry_point(path: Option<PathBuf>)`
     - `is_entry_point_editable()` - Check if field can be edited

3. **Add Entry Point field to Launch Context widget**
   - In `src/tui/widgets/new_session_dialog/launch_context.rs`:
   - Add `render_entry_point_field()` function:
     ```rust
     fn render_entry_point_field(
         area: Rect,
         buf: &mut Buffer,
         state: &LaunchContextState,
         is_focused: bool,
     ) {
         let entry_focused = is_focused
             && state.focused_field == LaunchContextField::EntryPoint;
         let entry_disabled = !state.is_entry_point_editable();

         let display = state.entry_point
             .as_ref()
             .map(|p| p.display().to_string())
             .unwrap_or_else(|| "(default)".to_string());

         let suffix = if should_show_disabled_suffix(state, LaunchContextField::EntryPoint) {
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
   - Update `calculate_fields_layout()` to add row for Entry Point
   - Update `render_common_fields()` to include entry point field

4. **Add fuzzy modal for entry point selection**
   - Add `EntryPoint` variant to `FuzzyModalType` enum
   - In handler, when Entry Point field activated:
     - Discover entry points using `discover_entry_points()`
     - Open fuzzy modal with discovered files
     - Include "(default)" option to clear selection

5. **Add handler for entry point selection with auto-save**
   - In `src/app/handler/new_session/launch_context.rs`:
     ```rust
     pub fn handle_entry_point_selected(
         state: &mut AppState,
         entry_point: Option<PathBuf>,
     ) -> UpdateResult {
         // 1. If no config selected and entry_point is set, auto-create FDemon config
         // 2. Update state.new_session_dialog.launch_context.entry_point
         // 3. If selected config is FDemon source, trigger auto-save:
         //    - Call update_launch_config_field() with "entry_point" field
         //    - This persists the change to .fdemon/launch.toml
     }
     ```
   - Follow the same pattern as `handle_flavor_selected()` and `handle_dart_defines_saved()`

6. **Update `build_launch_params()` to include discovered entry points**
   - For populating the fuzzy modal

7. **Ensure auto-save for FDemon configs**
   - When entry point is changed in the UI:
     - If config source is `ConfigSource::FDemon`, call `update_launch_config_field()`
     - Uses the fix from Phase 1 Step 6
   - VSCode configs remain read-only (entry point displayed but not editable)

**Milestone**: Users can select entry point from fuzzy modal showing discovered files.

---

## Edge Cases & Risks

### Entry Point Validation
- **Risk:** User specifies non-existent file path
- **Mitigation:** Flutter itself will error with a clear message; discovery only shows existing files

### Path Normalization
- **Risk:** Relative vs absolute paths
- **Mitigation:** Always store and display as relative paths (e.g., `lib/main_dev.dart`)

### Config Merging
- **Risk:** User selects config with entry_point, then clears the config selection
- **Mitigation:** When no config is selected, entry_point is None (uses Flutter's default `lib/main.dart`)

### Performance
- **Risk:** Large projects with many Dart files slow down discovery
- **Mitigation:**
  - Only scan `lib/` directory (not test/, etc.)
  - Cache results per session
  - Use async discovery with loading indicator

### Main Function Detection
- **Risk:** False positives (main() in comments) or false negatives (unusual formatting)
- **Mitigation:**
  - Use regex that handles common patterns
  - Accept some edge cases - users can always type custom path

---

## Configuration Examples

### .vscode/launch.json

```json
{
  "version": "0.2.0",
  "configurations": [
    {
      "name": "My App (develop)",
      "type": "dart",
      "request": "launch",
      "program": "lib/main_develop.dart",
      "args": ["--flavor", "develop"]
    },
    {
      "name": "My App (staging)",
      "type": "dart",
      "request": "launch",
      "program": "lib/main_staging.dart",
      "args": ["--flavor", "staging"]
    }
  ]
}
```

### .fdemon/launch.toml

```toml
[[configurations]]
name = "Development"
device = "auto"
mode = "debug"
flavor = "develop"
entry_point = "lib/main_develop.dart"
```

---

## Keyboard Shortcuts Summary

| Key | Action |
|-----|--------|
| `Enter` | Open entry point selection modal (when focused) |
| `↑/↓` | Navigate entry point list in modal |
| `Esc` | Close modal / clear selection |
| Type | Filter entry points in fuzzy modal |

---

## Success Criteria

### Phase 1 Complete When:
- [ ] `LaunchParams` includes `entry_point` field
- [ ] `LaunchContextState` includes `entry_point` field
- [ ] `build_launch_params()` extracts `entry_point` from state
- [ ] `select_config()` applies `entry_point` from selected config
- [ ] `handle_launch()` passes `entry_point` to `LaunchConfig`
- [ ] `update_launch_config_field()` handles `entry_point` field for auto-save
- [ ] VSCode configs with `program` field result in correct `-t` argument to Flutter
- [ ] FDemon configs with `entry_point` field load and save correctly
- [ ] Unit tests verify entry_point flows through the launch chain

### Phase 2 Complete When:
- [ ] `discover_entry_points()` function implemented
- [ ] Correctly finds files with `void main(` patterns
- [ ] Handles nested directories in `lib/`
- [ ] Returns sorted list with `main.dart` first
- [ ] Unit tests cover various main() declaration styles

### Phase 3 Complete When:
- [ ] Entry Point field visible in Launch Context pane
- [ ] Field shows current value or "(default)"
- [ ] Fuzzy modal opens with discovered entry points
- [ ] `FuzzyModalType::EntryPoint` variant added and allows custom input
- [ ] Can select from list or type custom path
- [ ] Selection updates `LaunchContextState.entry_point`
- [ ] For FDemon configs: selection triggers auto-save to `.fdemon/launch.toml`
- [ ] VSCode configs show entry point as read-only with "(from config)" suffix
- [ ] Compact layout handles new field gracefully
- [ ] Navigation (↑/↓) works correctly with new field in the field order

---

## Testing

### Unit Tests

```rust
// Phase 1 Tests
#[test]
fn test_launch_params_includes_entry_point() {
    let mut state = NewSessionDialogState::new(configs);
    state.launch_context.entry_point = Some("lib/main_dev.dart".into());

    let params = state.build_launch_params().unwrap();
    assert_eq!(params.entry_point, Some("lib/main_dev.dart".into()));
}

#[test]
fn test_select_config_applies_entry_point() {
    let mut state = LaunchContextState::new(configs_with_entry_point);
    state.select_config(Some(0));

    assert_eq!(state.entry_point, Some("lib/main_dev.dart".into()));
}

#[test]
fn test_update_launch_config_field_entry_point() {
    let temp = tempdir().unwrap();
    save_launch_configs(temp.path(), &[LaunchConfig {
        name: "Dev".to_string(),
        ..Default::default()
    }]).unwrap();

    // Set entry_point
    update_launch_config_field(temp.path(), "Dev", "entry_point", "lib/main_dev.dart").unwrap();

    let loaded = load_launch_configs(temp.path());
    assert_eq!(loaded[0].config.entry_point, Some("lib/main_dev.dart".into()));

    // Clear entry_point
    update_launch_config_field(temp.path(), "Dev", "entry_point", "").unwrap();

    let loaded = load_launch_configs(temp.path());
    assert_eq!(loaded[0].config.entry_point, None);
}

#[test]
fn test_launch_toml_roundtrip_with_entry_point() {
    let temp = tempdir().unwrap();
    let configs = vec![LaunchConfig {
        name: "Dev".to_string(),
        entry_point: Some("lib/main_dev.dart".into()),
        ..Default::default()
    }];

    save_launch_configs(temp.path(), &configs).unwrap();
    let loaded = load_launch_configs(temp.path());

    assert_eq!(loaded[0].config.entry_point, Some("lib/main_dev.dart".into()));
}

// Phase 2 Tests
#[test]
fn test_discover_entry_points_finds_main() {
    let temp = create_test_project();
    write_dart_file(&temp, "lib/main.dart", "void main() {}");
    write_dart_file(&temp, "lib/main_dev.dart", "void main() {}");
    write_dart_file(&temp, "lib/utils.dart", "void helper() {}");

    let entry_points = discover_entry_points(temp.path());

    assert_eq!(entry_points.len(), 2);
    assert_eq!(entry_points[0], PathBuf::from("lib/main.dart"));
    assert_eq!(entry_points[1], PathBuf::from("lib/main_dev.dart"));
}

#[test]
fn test_has_main_function_patterns() {
    assert!(has_main_function_in_content("void main() {}"));
    assert!(has_main_function_in_content("void main(List<String> args) {}"));
    assert!(has_main_function_in_content("Future<void> main() async {}"));
    assert!(has_main_function_in_content("  main() {}"));
    assert!(!has_main_function_in_content("void notMain() {}"));
    assert!(!has_main_function_in_content("// void main() {}"));  // Comment
}

// Phase 3 Tests
#[test]
fn test_entry_point_field_renders() {
    let state = LaunchContextState::new(LoadedConfigs::default());
    // ... render test
    assert!(content.contains("Entry Point"));
}

#[test]
fn test_handle_entry_point_selected() {
    let mut state = AppState::default();
    let result = handle_entry_point_selected(
        &mut state,
        Some("lib/main_dev.dart".into()),
    );

    assert_eq!(
        state.new_session_dialog_state.launch_context.entry_point,
        Some("lib/main_dev.dart".into())
    );
}

#[test]
fn test_entry_point_auto_save_for_fdemon_config() {
    // Setup: Create FDemon config and select it
    let temp = tempdir().unwrap();
    let mut state = create_test_state_with_fdemon_config(temp.path());

    // Select entry point
    handle_entry_point_selected(&mut state, Some("lib/main_dev.dart".into()));

    // Verify auto-save was triggered
    let loaded = load_launch_configs(temp.path());
    assert_eq!(loaded[0].config.entry_point, Some("lib/main_dev.dart".into()));
}

#[test]
fn test_entry_point_readonly_for_vscode_config() {
    let mut configs = LoadedConfigs::default();
    configs.configs.push(SourcedConfig {
        config: LaunchConfig {
            entry_point: Some("lib/main_vscode.dart".into()),
            ..Default::default()
        },
        source: ConfigSource::VSCode,
        display_name: "VSCode Config".to_string(),
    });

    let mut state = LaunchContextState::new(configs);
    state.select_config(Some(0));

    // VSCode config entry_point should NOT be editable
    assert!(!state.is_entry_point_editable());
}

#[test]
fn test_fuzzy_modal_type_entry_point() {
    let modal_type = FuzzyModalType::EntryPoint;
    assert!(modal_type.allows_custom()); // Should allow typing custom paths
    assert_eq!(modal_type.title(), "Select Entry Point");
}
```

### Manual Testing

1. Create Flutter project with multiple entry points:
   ```
   lib/main.dart
   lib/main_develop.dart
   lib/main_staging.dart
   ```

2. Create `.vscode/launch.json` with custom `program`:
   ```json
   {
     "configurations": [{
       "name": "Dev",
       "type": "dart",
       "request": "launch",
       "program": "lib/main_develop.dart"
     }]
   }
   ```

3. Launch fdemon and open NewSessionDialog (`n`)
4. Select the VSCode "Dev" config
5. Verify Entry Point field shows `lib/main_develop.dart`
6. Launch and verify Flutter command includes `-t lib/main_develop.dart`
7. Test fuzzy modal by focusing Entry Point and pressing Enter
8. Verify all discovered entry points are listed

---

## Future Enhancements

1. **Entry Point Creation Wizard**: Quick-create new entry point from template
2. **Entry Point Validation**: Show warning if selected file doesn't exist
3. **Recent Entry Points**: Remember recently used entry points
4. **Entry Point Preview**: Show file contents in preview pane

---

## References

- Flutter CLI: `flutter run --help` (`-t, --target` option)
- VSCode Dart Extension: [Launch Configurations](https://dartcode.org/docs/launch-configuration/)
- Existing implementation: `src/config/vscode.rs:159` (program → entry_point mapping)
- Existing implementation: `src/config/launch.rs:309` (build_flutter_args with `-t`)
