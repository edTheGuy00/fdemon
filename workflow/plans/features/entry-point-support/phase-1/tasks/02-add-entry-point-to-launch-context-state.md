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

**Status:** Not Started
