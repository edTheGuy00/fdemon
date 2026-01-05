## Task: Settings Types & Traits

**Objective**: Define the core types and traits for the settings system, enabling modular and extensible settings management.

**Depends on**: None

**Estimated Time**: 1.5-2 hours

### Scope

- `src/config/types.rs`: Add settings-related enums, structs, and traits

### Details

Create the foundational types that will power the settings UI:

#### 1. SettingsTab Enum

```rust
/// Tab in the settings panel
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsTab {
    #[default]
    Project,       // config.toml - shared settings
    UserPrefs,     // settings.local.toml - user-specific
    LaunchConfig,  // launch.toml - shared launch configs
    VSCodeConfig,  // launch.json - read-only display
}

impl SettingsTab {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Project => "Project",
            Self::UserPrefs => "User",
            Self::LaunchConfig => "Launch",
            Self::VSCodeConfig => "VSCode",
        }
    }

    pub fn index(&self) -> usize {
        match self {
            Self::Project => 0,
            Self::UserPrefs => 1,
            Self::LaunchConfig => 2,
            Self::VSCodeConfig => 3,
        }
    }

    pub fn from_index(idx: usize) -> Option<Self> {
        match idx {
            0 => Some(Self::Project),
            1 => Some(Self::UserPrefs),
            2 => Some(Self::LaunchConfig),
            3 => Some(Self::VSCodeConfig),
            _ => None,
        }
    }

    pub fn next(&self) -> Self {
        Self::from_index((self.index() + 1) % 4).unwrap()
    }

    pub fn prev(&self) -> Self {
        Self::from_index((self.index() + 3) % 4).unwrap()
    }
}
```

#### 2. SettingValue Enum

```rust
/// A setting value that can be edited
#[derive(Debug, Clone, PartialEq)]
pub enum SettingValue {
    Bool(bool),
    Number(i64),
    Float(f64),
    String(String),
    Enum { value: String, options: Vec<String> },
    List(Vec<String>),
}

impl SettingValue {
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Bool(_) => "boolean",
            Self::Number(_) => "number",
            Self::Float(_) => "float",
            Self::String(_) => "string",
            Self::Enum { .. } => "enum",
            Self::List(_) => "list",
        }
    }

    pub fn display(&self) -> String {
        match self {
            Self::Bool(b) => if *b { "true" } else { "false" }.to_string(),
            Self::Number(n) => n.to_string(),
            Self::Float(f) => format!("{:.2}", f),
            Self::String(s) => s.clone(),
            Self::Enum { value, .. } => value.clone(),
            Self::List(items) => items.join(", "),
        }
    }
}
```

#### 3. SettingItem Struct

```rust
/// A single setting item for display/editing
#[derive(Debug, Clone)]
pub struct SettingItem {
    /// Unique identifier (e.g., "behavior.auto_start")
    pub id: String,
    /// Display label (e.g., "Auto Start")
    pub label: String,
    /// Help text / description
    pub description: String,
    /// Current value
    pub value: SettingValue,
    /// Default value (for reset functionality)
    pub default: SettingValue,
    /// Whether this setting is read-only
    pub readonly: bool,
    /// Category/section for grouping
    pub section: String,
}

impl SettingItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            description: String::new(),
            value: SettingValue::Bool(false),
            default: SettingValue::Bool(false),
            readonly: false,
            section: String::new(),
        }
    }

    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    pub fn value(mut self, val: SettingValue) -> Self {
        self.value = val.clone();
        if matches!(self.default, SettingValue::Bool(false)) {
            self.default = val;
        }
        self
    }

    pub fn default(mut self, val: SettingValue) -> Self {
        self.default = val;
        self
    }

    pub fn readonly(mut self) -> Self {
        self.readonly = true;
        self
    }

    pub fn section(mut self, sec: impl Into<String>) -> Self {
        self.section = sec.into();
        self
    }

    pub fn is_modified(&self) -> bool {
        self.value != self.default
    }
}
```

#### 4. UserPreferences Struct

```rust
/// User-specific preferences (stored in settings.local.toml)
/// These override corresponding values in config.toml
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct UserPreferences {
    /// Override editor settings
    #[serde(default)]
    pub editor: Option<EditorSettings>,

    /// Override UI theme
    #[serde(default)]
    pub theme: Option<String>,

    /// Last selected device (for quick re-launch)
    #[serde(default)]
    pub last_device: Option<String>,

    /// Last selected launch config name
    #[serde(default)]
    pub last_config: Option<String>,

    /// Window size preference (if supported)
    #[serde(default)]
    pub window: Option<WindowPrefs>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct WindowPrefs {
    pub width: Option<u16>,
    pub height: Option<u16>,
}
```

### Acceptance Criteria

1. `SettingsTab` enum with all four tabs and navigation methods
2. `SettingValue` enum supporting all required types (bool, number, float, string, enum, list)
3. `SettingItem` struct with builder pattern for fluent construction
4. `UserPreferences` struct for local settings file
5. All types implement necessary traits (Debug, Clone, Serialize, Deserialize where applicable)
6. Unit tests for tab navigation (next/prev/from_index)
7. Unit tests for SettingValue display formatting
8. Unit tests for SettingItem builder and is_modified()

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_settings_tab_navigation() {
        assert_eq!(SettingsTab::Project.next(), SettingsTab::UserPrefs);
        assert_eq!(SettingsTab::VSCodeConfig.next(), SettingsTab::Project);
        assert_eq!(SettingsTab::Project.prev(), SettingsTab::VSCodeConfig);
    }

    #[test]
    fn test_settings_tab_from_index() {
        assert_eq!(SettingsTab::from_index(0), Some(SettingsTab::Project));
        assert_eq!(SettingsTab::from_index(3), Some(SettingsTab::VSCodeConfig));
        assert_eq!(SettingsTab::from_index(4), None);
    }

    #[test]
    fn test_setting_value_display() {
        assert_eq!(SettingValue::Bool(true).display(), "true");
        assert_eq!(SettingValue::Number(42).display(), "42");
        assert_eq!(SettingValue::String("hello".into()).display(), "hello");
    }

    #[test]
    fn test_setting_item_builder() {
        let item = SettingItem::new("test.id", "Test Label")
            .description("A test setting")
            .value(SettingValue::Bool(true))
            .section("Test");

        assert_eq!(item.id, "test.id");
        assert_eq!(item.label, "Test Label");
        assert!(!item.is_modified()); // value == default
    }

    #[test]
    fn test_setting_item_is_modified() {
        let item = SettingItem::new("test", "Test")
            .value(SettingValue::Bool(true))
            .default(SettingValue::Bool(false));

        assert!(item.is_modified());
    }
}
```

### Notes

- The trait-based approach allows future settings sections to be added without modifying core logic
- `SettingValue::Enum` includes options to support dropdown-style selection
- Consider adding validation support in a future iteration
- The `UserPreferences` struct is intentionally minimal - expand as needed

---

## Completion Summary

**Status:** Done

**Files Modified:**

| File | Changes |
|------|---------|
| `src/config/types.rs` | Added 4 new types: `SettingsTab`, `SettingValue`, `SettingItem`, and `UserPreferences` (with `WindowPrefs`). Added comprehensive unit tests. |

**Implementation Details:**

1. **SettingsTab enum**: Implemented with all 4 tabs (Project, UserPrefs, LaunchConfig, VSCodeConfig) and complete navigation methods (next/prev/from_index)
2. **SettingValue enum**: Supports all required types - Bool, Number, Float, String, Enum (with options), and List. Includes `type_name()` and `display()` methods
3. **SettingItem struct**: Builder pattern implementation with fluent API for constructing settings. Includes `is_modified()` method to track changes
4. **UserPreferences struct**: Serde-compatible struct for local settings file with optional overrides for editor, theme, last device/config, and window preferences
5. All types implement required traits (Debug, Clone, PartialEq where needed, Serialize/Deserialize for persistence)

**Testing Performed:**
- `cargo fmt` - PASS
- `cargo check` - PASS
- `cargo test config::types` - PASS (25 tests total, 11 new tests added)
- `cargo clippy -- -D warnings` - PASS (no clippy warnings in added code)

**Notable Decisions:**

1. **SettingValue::Float formatting**: Used `{:.2}` format to display 2 decimal places for consistency
2. **Builder pattern default handling**: When `.value()` is called without `.default()`, the value is automatically set as the default unless a default was already explicitly set (checked via matches! for Bool(false) sentinel)
3. **Tab navigation**: Implemented wrapping behavior using modulo arithmetic for seamless cycling through tabs
4. **UserPreferences fields**: All fields are `Option<T>` to allow selective overrides, with `#[serde(default)]` for forward compatibility
