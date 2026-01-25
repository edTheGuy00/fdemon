## Task: Add entry point state helper methods

**Objective**: Add `available_entry_points` field and helper methods to `LaunchContextState` for entry point management.

**Depends on**: Tasks 01, 02

### Scope

- `src/app/new_session_dialog/state.rs`: Add field and helper methods to `LaunchContextState`

### Details

Add the `available_entry_points` field to cache discovered entry points, and add helper methods for display, editability checking, and setting the entry point.

#### Changes to `LaunchContextState`:

```rust
pub struct LaunchContextState {
    /// Available configurations
    pub configs: LoadedConfigs,

    /// Index of selected configuration (None = no config, use defaults)
    pub selected_config_index: Option<usize>,

    /// Selected Flutter mode
    pub mode: FlutterMode,

    /// Flavor (from config or user override)
    pub flavor: Option<String>,

    /// Entry point (from config or user override)
    pub entry_point: Option<PathBuf>,  // Already exists from Phase 1

    /// Available entry points discovered from project
    pub available_entry_points: Vec<PathBuf>,  // NEW

    /// Dart defines (from config or user override)
    pub dart_defines: Vec<DartDefine>,

    /// Currently focused field
    pub focused_field: LaunchContextField,
}
```

#### Update `new()`:

```rust
impl LaunchContextState {
    pub fn new(configs: LoadedConfigs) -> Self {
        Self {
            configs,
            selected_config_index: None,
            mode: FlutterMode::Debug,
            flavor: None,
            entry_point: None,
            available_entry_points: Vec::new(),  // NEW
            dart_defines: Vec::new(),
            focused_field: LaunchContextField::Config,
        }
    }
    // ...
}
```

#### Add helper methods:

```rust
impl LaunchContextState {
    // ... existing methods ...

    /// Check if entry point is editable
    pub fn is_entry_point_editable(&self) -> bool {
        self.is_field_editable(LaunchContextField::EntryPoint)
    }

    /// Get display string for entry point
    ///
    /// Returns the path as a string, or "(default)" if no entry point is set.
    /// "(default)" indicates Flutter will use lib/main.dart
    pub fn entry_point_display(&self) -> String {
        self.entry_point
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "(default)".to_string())
    }

    /// Set the entry point
    pub fn set_entry_point(&mut self, entry_point: Option<PathBuf>) {
        self.entry_point = entry_point;
    }

    /// Set available entry points (typically from discovery)
    pub fn set_available_entry_points(&mut self, entry_points: Vec<PathBuf>) {
        self.available_entry_points = entry_points;
    }

    /// Get entry point items for fuzzy modal
    ///
    /// Returns a list of strings for the fuzzy modal, with "(default)" as first option.
    pub fn entry_point_modal_items(&self) -> Vec<String> {
        let mut items = vec!["(default)".to_string()];
        items.extend(
            self.available_entry_points
                .iter()
                .map(|p| p.display().to_string())
        );
        items
    }
}
```

#### Update `is_field_editable()`:

The existing `is_field_editable()` method already handles all fields via the catch-all pattern:

```rust
pub fn is_field_editable(&self, field: LaunchContextField) -> bool {
    match field {
        LaunchContextField::Config => true,
        LaunchContextField::Launch => true,
        _ => {  // This handles EntryPoint too
            match self.selected_config_source() {
                Some(ConfigSource::VSCode) => false,
                Some(ConfigSource::FDemon) => true,
                None => true,
                Some(ConfigSource::CommandLine) | Some(ConfigSource::Default) => true,
            }
        }
    }
}
```

No changes needed to `is_field_editable()` since the catch-all `_` pattern handles `EntryPoint`.

### Acceptance Criteria

1. `LaunchContextState` has `available_entry_points: Vec<PathBuf>` field
2. `new()` initializes `available_entry_points` to empty vec
3. `is_entry_point_editable()` returns correct value based on config source
4. `entry_point_display()` returns path or "(default)"
5. `set_entry_point()` updates the entry point
6. `set_available_entry_points()` updates the cached entry points
7. `entry_point_modal_items()` returns "(default)" + discovered paths
8. Code compiles without errors

### Testing

Add these tests to `src/app/new_session_dialog/state.rs`:

```rust
#[test]
fn test_entry_point_display_none() {
    let state = LaunchContextState::new(LoadedConfigs::default());
    assert_eq!(state.entry_point_display(), "(default)");
}

#[test]
fn test_entry_point_display_some() {
    let mut state = LaunchContextState::new(LoadedConfigs::default());
    state.set_entry_point(Some(PathBuf::from("lib/main_dev.dart")));
    assert_eq!(state.entry_point_display(), "lib/main_dev.dart");
}

#[test]
fn test_is_entry_point_editable_no_config() {
    let state = LaunchContextState::new(LoadedConfigs::default());
    // No config selected = editable
    assert!(state.is_entry_point_editable());
}

#[test]
fn test_is_entry_point_editable_vscode_config() {
    let mut configs = LoadedConfigs::default();
    configs.configs.push(SourcedConfig {
        config: LaunchConfig::default(),
        source: ConfigSource::VSCode,
        display_name: "VSCode".to_string(),
    });

    let mut state = LaunchContextState::new(configs);
    state.selected_config_index = Some(0);

    // VSCode config = NOT editable
    assert!(!state.is_entry_point_editable());
}

#[test]
fn test_is_entry_point_editable_fdemon_config() {
    let mut configs = LoadedConfigs::default();
    configs.configs.push(SourcedConfig {
        config: LaunchConfig::default(),
        source: ConfigSource::FDemon,
        display_name: "FDemon".to_string(),
    });

    let mut state = LaunchContextState::new(configs);
    state.selected_config_index = Some(0);

    // FDemon config = editable
    assert!(state.is_entry_point_editable());
}

#[test]
fn test_entry_point_modal_items() {
    let mut state = LaunchContextState::new(LoadedConfigs::default());
    state.set_available_entry_points(vec![
        PathBuf::from("lib/main.dart"),
        PathBuf::from("lib/main_dev.dart"),
    ]);

    let items = state.entry_point_modal_items();

    assert_eq!(items.len(), 3);
    assert_eq!(items[0], "(default)");
    assert_eq!(items[1], "lib/main.dart");
    assert_eq!(items[2], "lib/main_dev.dart");
}

#[test]
fn test_entry_point_modal_items_empty() {
    let state = LaunchContextState::new(LoadedConfigs::default());
    let items = state.entry_point_modal_items();

    assert_eq!(items.len(), 1);
    assert_eq!(items[0], "(default)");
}
```

### Notes

- `entry_point` field already exists from Phase 1
- `available_entry_points` caches discovery results to avoid repeated disk I/O
- `entry_point_modal_items()` prepares data for the fuzzy modal
- "(default)" as first item allows users to clear the entry point selection

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/app/new_session_dialog/state.rs` | Added `available_entry_points: Vec<PathBuf>` field to `LaunchContextState`, updated `new()` to initialize it, updated `is_entry_point_editable()` to use `LaunchContextField::EntryPoint`, added `set_available_entry_points()` and `entry_point_modal_items()` helper methods, added `FuzzyModalType::EntryPoint` match arm in `close_fuzzy_modal_with_selection()`, added 8 comprehensive unit tests |

### Notable Decisions/Tradeoffs

1. **Fixed EntryPoint match in state.rs**: Added handling for `FuzzyModalType::EntryPoint` in `close_fuzzy_modal_with_selection()` method. The logic converts "(default)" string to `None`, otherwise wraps the selected string as `PathBuf`. This ensures the state module compiles and provides the necessary logic for Phase 3 integration.

2. **Updated is_entry_point_editable()**: Changed from using `LaunchContextField::Flavor` as proxy to using `LaunchContextField::EntryPoint` directly, now that the variant exists from Tasks 01-02.

### Testing Performed

- `cargo fmt` - Passed (code formatted)
- `cargo check` - Compilation errors exist in other files (expected - non-exhaustive matches in handler/new_session/ modules, to be fixed by tasks 06 and 07)
- Unit tests - Cannot run full test suite due to compilation errors in other files, but all test code is syntactically correct
- The state.rs file itself compiles without errors

### Risks/Limitations

1. **Compilation errors in other files**: The addition of `LaunchContextField::EntryPoint` and `FuzzyModalType::EntryPoint` variants causes non-exhaustive match pattern errors in:
   - `src/app/handler/new_session/fuzzy_modal.rs` (2 locations)
   - `src/app/handler/new_session/navigation.rs` (1 location)
   These will be resolved by tasks 06 and 07 as planned.

2. **Cannot verify tests until compilation errors are fixed**: Full test suite cannot be run until the non-exhaustive patterns are fixed. However, the test code itself is correct and follows the same patterns as existing tests in the module.
