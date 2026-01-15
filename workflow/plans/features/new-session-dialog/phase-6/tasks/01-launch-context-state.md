# Task: Launch Context State

## Summary

Create the state structure for the Launch Context pane, tracking selected configuration, mode, flavor, dart defines, and field focus.

## Files

| File | Action |
|------|--------|
| `src/tui/widgets/new_session_dialog/state.rs` | Modify (add LaunchContextState) |

## Implementation

### 1. Define field enum

```rust
// src/tui/widgets/new_session_dialog/state.rs

/// Fields in the Launch Context pane
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LaunchContextField {
    #[default]
    Config,
    Mode,
    Flavor,
    DartDefines,
    Launch,
}

impl LaunchContextField {
    /// Get next field (wrapping)
    pub fn next(self) -> Self {
        match self {
            Self::Config => Self::Mode,
            Self::Mode => Self::Flavor,
            Self::Flavor => Self::DartDefines,
            Self::DartDefines => Self::Launch,
            Self::Launch => Self::Config,
        }
    }

    /// Get previous field (wrapping)
    pub fn prev(self) -> Self {
        match self {
            Self::Config => Self::Launch,
            Self::Mode => Self::Config,
            Self::Flavor => Self::Mode,
            Self::DartDefines => Self::Flavor,
            Self::Launch => Self::DartDefines,
        }
    }

    /// Skip disabled fields when navigating
    pub fn next_enabled(self, is_disabled: impl Fn(Self) -> bool) -> Self {
        let mut next = self.next();
        // Avoid infinite loop if all fields disabled
        let start = next;
        while is_disabled(next) && next.next() != start {
            next = next.next();
        }
        next
    }

    pub fn prev_enabled(self, is_disabled: impl Fn(Self) -> bool) -> Self {
        let mut prev = self.prev();
        let start = prev;
        while is_disabled(prev) && prev.prev() != start {
            prev = prev.prev();
        }
        prev
    }
}
```

### 2. Launch context state

```rust
use crate::config::{ConfigSource, FlutterMode, LoadedConfigs, SourcedConfig};

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

    /// Dart defines (from config or user override)
    pub dart_defines: Vec<DartDefine>,

    /// Currently focused field
    pub focused_field: LaunchContextField,
}

/// A dart define key-value pair
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DartDefine {
    pub key: String,
    pub value: String,
}

impl DartDefine {
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
        }
    }

    /// Format as command line argument
    pub fn to_arg(&self) -> String {
        format!("{}={}", self.key, self.value)
    }
}
```

### 3. Editability methods

```rust
impl LaunchContextState {
    pub fn new(configs: LoadedConfigs) -> Self {
        Self {
            configs,
            selected_config_index: None,
            mode: FlutterMode::Debug,
            flavor: None,
            dart_defines: Vec::new(),
            focused_field: LaunchContextField::Config,
        }
    }

    /// Get the currently selected config
    pub fn selected_config(&self) -> Option<&SourcedConfig> {
        self.selected_config_index
            .and_then(|i| self.configs.configs.get(i))
    }

    /// Get the source of the selected config
    pub fn selected_config_source(&self) -> Option<ConfigSource> {
        self.selected_config().map(|c| c.source)
    }

    /// Check if a field is editable based on config source
    pub fn is_field_editable(&self, field: LaunchContextField) -> bool {
        match field {
            // Config is always selectable
            LaunchContextField::Config => true,
            // Launch button is always enabled
            LaunchContextField::Launch => true,
            // Other fields depend on config source
            _ => {
                match self.selected_config_source() {
                    // VSCode configs: all fields read-only
                    Some(ConfigSource::VSCode) => false,
                    // FDemon configs: all fields editable
                    Some(ConfigSource::FDemon) => true,
                    // No config: all fields editable (transient)
                    None => true,
                }
            }
        }
    }

    /// Check if mode is editable
    pub fn is_mode_editable(&self) -> bool {
        self.is_field_editable(LaunchContextField::Mode)
    }

    /// Check if flavor is editable
    pub fn is_flavor_editable(&self) -> bool {
        self.is_field_editable(LaunchContextField::Flavor)
    }

    /// Check if dart defines are editable
    pub fn are_dart_defines_editable(&self) -> bool {
        self.is_field_editable(LaunchContextField::DartDefines)
    }
}
```

### 4. Config selection

```rust
impl LaunchContextState {
    /// Select a configuration by index
    pub fn select_config(&mut self, index: Option<usize>) {
        self.selected_config_index = index;

        // Apply config values
        if let Some(config) = self.selected_config() {
            self.mode = config.config.mode;

            if let Some(ref flavor) = config.config.flavor {
                self.flavor = Some(flavor.clone());
            }

            if !config.config.dart_defines.is_empty() {
                self.dart_defines = config.config.dart_defines
                    .iter()
                    .filter_map(|s| {
                        let parts: Vec<&str> = s.splitn(2, '=').collect();
                        if parts.len() == 2 {
                            Some(DartDefine::new(parts[0], parts[1]))
                        } else {
                            None
                        }
                    })
                    .collect();
            }
        }
    }

    /// Select a configuration by name
    pub fn select_config_by_name(&mut self, name: &str) {
        let index = self.configs.configs
            .iter()
            .position(|c| c.display_name == name);
        self.select_config(index);
    }
}
```

### 5. Navigation

```rust
impl LaunchContextState {
    /// Move to next field (skip disabled)
    pub fn focus_next(&mut self) {
        self.focused_field = self.focused_field.next_enabled(|f| !self.is_field_editable(f));
    }

    /// Move to previous field (skip disabled)
    pub fn focus_prev(&mut self) {
        self.focused_field = self.focused_field.prev_enabled(|f| !self.is_field_editable(f));
    }

    /// Cycle mode selection (when mode field is focused)
    pub fn cycle_mode_next(&mut self) {
        if self.is_mode_editable() {
            self.mode = match self.mode {
                FlutterMode::Debug => FlutterMode::Profile,
                FlutterMode::Profile => FlutterMode::Release,
                FlutterMode::Release => FlutterMode::Debug,
            };
        }
    }

    pub fn cycle_mode_prev(&mut self) {
        if self.is_mode_editable() {
            self.mode = match self.mode {
                FlutterMode::Debug => FlutterMode::Release,
                FlutterMode::Profile => FlutterMode::Debug,
                FlutterMode::Release => FlutterMode::Profile,
            };
        }
    }

    /// Set flavor
    pub fn set_flavor(&mut self, flavor: Option<String>) {
        if self.is_flavor_editable() {
            self.flavor = flavor;
        }
    }

    /// Set dart defines
    pub fn set_dart_defines(&mut self, defines: Vec<DartDefine>) {
        if self.are_dart_defines_editable() {
            self.dart_defines = defines;
        }
    }
}
```

### 6. Display helpers

```rust
impl LaunchContextState {
    /// Get flavor display string
    pub fn flavor_display(&self) -> String {
        self.flavor.clone().unwrap_or_else(|| "(none)".to_string())
    }

    /// Get dart defines display string
    pub fn dart_defines_display(&self) -> String {
        let count = self.dart_defines.len();
        if count == 0 {
            "(none)".to_string()
        } else if count == 1 {
            "1 item".to_string()
        } else {
            format!("{} items", count)
        }
    }

    /// Get config display string
    pub fn config_display(&self) -> String {
        self.selected_config()
            .map(|c| c.display_name.clone())
            .unwrap_or_else(|| "(none)".to_string())
    }
}
```

## Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_navigation() {
        let field = LaunchContextField::Config;
        assert_eq!(field.next(), LaunchContextField::Mode);
        assert_eq!(field.prev(), LaunchContextField::Launch);
    }

    #[test]
    fn test_field_navigation_wraps() {
        let field = LaunchContextField::Launch;
        assert_eq!(field.next(), LaunchContextField::Config);
    }

    #[test]
    fn test_editability_no_config() {
        let state = LaunchContextState::new(LoadedConfigs::default());

        assert!(state.is_field_editable(LaunchContextField::Config));
        assert!(state.is_field_editable(LaunchContextField::Mode));
        assert!(state.is_field_editable(LaunchContextField::Flavor));
        assert!(state.is_field_editable(LaunchContextField::DartDefines));
    }

    #[test]
    fn test_editability_vscode_config() {
        let mut configs = LoadedConfigs::default();
        configs.configs.push(SourcedConfig {
            config: LaunchConfig::default(),
            source: ConfigSource::VSCode,
            display_name: "Test".to_string(),
        });

        let mut state = LaunchContextState::new(configs);
        state.select_config(Some(0));

        assert!(state.is_field_editable(LaunchContextField::Config)); // Always editable
        assert!(!state.is_field_editable(LaunchContextField::Mode));
        assert!(!state.is_field_editable(LaunchContextField::Flavor));
        assert!(!state.is_field_editable(LaunchContextField::DartDefines));
        assert!(state.is_field_editable(LaunchContextField::Launch)); // Always editable
    }

    #[test]
    fn test_dart_define() {
        let define = DartDefine::new("API_KEY", "secret123");
        assert_eq!(define.to_arg(), "API_KEY=secret123");
    }
}
```

## Verification

```bash
cargo fmt && cargo check && cargo test launch_context && cargo clippy -- -D warnings
```

## Notes

- Config selection applies config values to state
- Editability rules enforce VSCode config read-only behavior
- Navigation skips disabled fields
- Launch button is always enabled (can launch with any valid state)

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/tui/widgets/new_session_dialog/state.rs` | Added `next_enabled()` and `prev_enabled()` methods to `LaunchContextField`, added `to_arg()` method to `DartDefine`, implemented `LaunchContextState` struct with editability, navigation, config selection, and display methods, added 23 comprehensive unit tests |

### Notable Decisions/Tradeoffs

1. **Borrow Checker Fix**: Had to clone the selected config in `select_config()` to avoid borrow checker issues when applying config values to state fields. This is a small performance cost but necessary for safety.

2. **Extended Editability**: Added support for `ConfigSource::CommandLine` and `ConfigSource::Default` to treat them as editable (like FDemon configs). This wasn't in the original spec but makes logical sense.

3. **DartDefines Conversion**: The `dart_defines` field in `LaunchConfig` is a HashMap, so we convert it to Vec<DartDefine> when applying config values. This allows for ordered display in the UI.

### Testing Performed

- `cargo fmt` - Passed
- `cargo check` - Passed
- `cargo test launch_context` - Passed (23 tests)
- `cargo test --lib` - Passed (1574 tests total)
- `cargo clippy -- -D warnings` - Passed (no warnings)

### Test Coverage

The implementation includes comprehensive test coverage:
- Field navigation (next/prev with wrapping)
- Editability logic for different config sources (no config, VSCode, FDemon)
- Focus navigation that skips disabled fields
- Mode cycling (forward and backward)
- Mode cycling disabled when read-only
- Flavor/dart defines setters with editability checks
- Display helper methods
- Config selection by index and by name
- Config value application on selection

### Risks/Limitations

None identified. The implementation follows Rust best practices, has comprehensive test coverage, and aligns with the existing codebase patterns.
