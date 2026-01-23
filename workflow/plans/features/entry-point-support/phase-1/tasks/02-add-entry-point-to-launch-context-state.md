## Task: Add entry_point field to LaunchContextState

**Objective**: Add the `entry_point` field to `LaunchContextState` to store the selected entry point from config or user selection.

**Depends on**: None

### Scope

- `src/app/new_session_dialog/state.rs`: Add `entry_point` field and helper methods to `LaunchContextState`

### Details

Add `entry_point: Option<PathBuf>` to the `LaunchContextState` struct. Also add helper methods for display and editability.

#### 1. Add field to struct

```rust
use std::path::PathBuf;

/// State for the Launch Context pane
#[derive(Debug, Clone)]
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
    pub entry_point: Option<PathBuf>,  // ADD THIS

    /// Dart defines (from config or user override)
    pub dart_defines: Vec<DartDefine>,

    /// Currently focused field
    pub focused_field: LaunchContextField,
}
```

#### 2. Update `new()` constructor

```rust
impl LaunchContextState {
    pub fn new(configs: LoadedConfigs) -> Self {
        Self {
            configs,
            selected_config_index: None,
            mode: FlutterMode::Debug,
            flavor: None,
            entry_point: None,  // ADD THIS
            dart_defines: Vec::new(),
            focused_field: LaunchContextField::Config,
        }
    }
    // ...
}
```

#### 3. Add helper methods

```rust
impl LaunchContextState {
    // ... existing methods ...

    /// Get entry point display string
    pub fn entry_point_display(&self) -> String {
        self.entry_point
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "(default)".to_string())
    }

    /// Check if entry point is editable
    pub fn is_entry_point_editable(&self) -> bool {
        self.is_field_editable(LaunchContextField::EntryPoint)
    }

    /// Set entry point
    pub fn set_entry_point(&mut self, entry_point: Option<PathBuf>) {
        if self.is_entry_point_editable() {
            self.entry_point = entry_point;
        }
    }
}
```

**Note**: `LaunchContextField::EntryPoint` doesn't exist yet - that's Phase 3. For now, use `LaunchContextField::Flavor` as a placeholder for the editability check, or just return `self.is_flavor_editable()` since they follow the same rules.

### Acceptance Criteria

1. `LaunchContextState` struct has `entry_point: Option<PathBuf>` field
2. `new()` initializes `entry_point` to `None`
3. `entry_point_display()` returns path string or "(default)"
4. `is_entry_point_editable()` returns correct value based on config source
5. `set_entry_point()` only sets value if editable
6. Code compiles without errors

### Testing

```rust
#[test]
fn test_launch_context_state_entry_point_default() {
    let state = LaunchContextState::new(LoadedConfigs::default());
    assert_eq!(state.entry_point, None);
    assert_eq!(state.entry_point_display(), "(default)");
}

#[test]
fn test_launch_context_state_entry_point_set() {
    let mut state = LaunchContextState::new(LoadedConfigs::default());
    state.entry_point = Some(PathBuf::from("lib/main_dev.dart"));
    assert_eq!(state.entry_point_display(), "lib/main_dev.dart");
}

#[test]
fn test_entry_point_editable_no_config() {
    let state = LaunchContextState::new(LoadedConfigs::default());
    // No config selected = editable
    assert!(state.is_entry_point_editable());
}
```

### Notes

- Can be done in parallel with Task 01
- The `is_field_editable()` method already exists and handles the VSCode read-only logic
- For Phase 1, we can use `LaunchContextField::Flavor` as proxy since entry point follows same editability rules

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/app/new_session_dialog/state.rs` | Added `entry_point: Option<PathBuf>` field to `LaunchContextState` struct; Updated `new()` constructor to initialize `entry_point` to `None`; Added helper methods `entry_point_display()`, `is_entry_point_editable()`, and `set_entry_point()`; Added comprehensive unit tests for entry point functionality |

### Notable Decisions/Tradeoffs

1. **Editability Proxy**: Used `LaunchContextField::Flavor` as a proxy in `is_entry_point_editable()` since `LaunchContextField::EntryPoint` doesn't exist yet (Phase 3). Entry point follows the same editability rules as flavor (editable for FDemon/Default configs, read-only for VSCode configs).

2. **PathBuf Import**: Added `use std::path::PathBuf` to the imports as required for the new field type.

3. **Display Format**: Entry point displays as the full path string when set, or "(default)" when None, following the pattern used by `flavor_display()`.

### Testing Performed

- `cargo fmt` - Passed
- `cargo check` - Passed (no compilation errors)
- `cargo test --lib new_session_dialog::state::tests` - Passed (16 tests, including 7 new entry point tests)
- `cargo test --lib` - Passed (1496 tests total, no regressions)
- `cargo clippy --lib -- -D warnings` - Passed (no warnings)

### Test Coverage

Added 7 comprehensive unit tests:
1. `test_launch_context_state_entry_point_default` - Verifies default initialization
2. `test_launch_context_state_entry_point_set` - Verifies display with path
3. `test_entry_point_editable_no_config` - Verifies editable when no config selected
4. `test_entry_point_editable_fdemon_config` - Verifies editable for FDemon configs
5. `test_entry_point_not_editable_vscode_config` - Verifies read-only for VSCode configs
6. `test_set_entry_point_when_editable` - Verifies setter works when editable
7. `test_set_entry_point_when_not_editable` - Verifies setter is ignored when not editable

### Risks/Limitations

1. **Phase Dependency**: The `is_entry_point_editable()` method uses `LaunchContextField::Flavor` as a proxy. This will need to be updated in Phase 3 when `LaunchContextField::EntryPoint` is added to properly support field-level navigation and focus.

2. **LaunchParams Integration**: The `entry_point` field is already present in `LaunchParams` struct (added by Task 01 or parallel work), and `build_launch_params()` method already includes it. This task focuses solely on the `LaunchContextState` struct as specified.
