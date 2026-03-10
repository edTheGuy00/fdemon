## Task: Native Log Configuration Settings

**Objective**: Add `[native_logs]` configuration section to `Settings` with master toggle, tag exclusion, and minimum priority level. This enables users to control native log capture behavior via `.fdemon/config.toml`.

**Depends on**: None

### Scope

- `crates/fdemon-app/src/config/types.rs`: Add `NativeLogsSettings` struct
- `crates/fdemon-app/src/config/mod.rs`: Re-export new type
- `crates/fdemon-app/src/config/settings.rs`: Add to default config TOML template

### Details

#### 1. Define `NativeLogsSettings` struct

Add to `crates/fdemon-app/src/config/types.rs`, following the existing pattern (e.g., `BehaviorSettings` at line 132, `DapSettings` at line 475):

```rust
/// Configuration for native platform log capture.
///
/// Controls whether fdemon runs parallel log capture processes (e.g., `adb logcat`,
/// `log stream`) to surface native plugin logs alongside Flutter logs.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NativeLogsSettings {
    /// Master toggle for native log capture. When false, no native log processes are spawned.
    #[serde(default = "default_native_logs_enabled")]
    pub enabled: bool,

    /// Tags to exclude from native log output. Default: `["flutter"]` to avoid
    /// duplicating Flutter's own log output which is already captured via `--machine`.
    #[serde(default = "default_native_logs_exclude_tags")]
    pub exclude_tags: Vec<String>,

    /// If set, ONLY show logs from these tags (overrides `exclude_tags`).
    /// Empty means "show all tags (minus exclude_tags)".
    #[serde(default)]
    pub include_tags: Vec<String>,

    /// Minimum native log priority level. Logs below this level are discarded.
    /// Options: "verbose", "debug", "info", "warning", "error"
    #[serde(default = "default_native_logs_min_level")]
    pub min_level: String,
}

fn default_native_logs_enabled() -> bool {
    true
}

fn default_native_logs_exclude_tags() -> Vec<String> {
    vec!["flutter".to_string()]
}

fn default_native_logs_min_level() -> String {
    "info".to_string()
}

impl Default for NativeLogsSettings {
    fn default() -> Self {
        Self {
            enabled: default_native_logs_enabled(),
            exclude_tags: default_native_logs_exclude_tags(),
            include_tags: Vec::new(),
            min_level: default_native_logs_min_level(),
        }
    }
}
```

Add a helper method to parse `min_level` into `NativeLogPriority` (from `fdemon-core`). This needs `fdemon-core` to have `NativeLogPriority` (task 01), but the settings struct itself can exist before that — the method can parse to a string and let the caller map. Alternatively, keep this as a simple string field and let the consumer (task 07) parse it:

```rust
impl NativeLogsSettings {
    /// Check if a given tag should be included in native log output.
    pub fn should_include_tag(&self, tag: &str) -> bool {
        if !self.include_tags.is_empty() {
            // Whitelist mode: only show tags in include_tags
            return self.include_tags.iter().any(|t| t.eq_ignore_ascii_case(tag));
        }
        // Blacklist mode: show all tags except those in exclude_tags
        !self.exclude_tags.iter().any(|t| t.eq_ignore_ascii_case(tag))
    }
}
```

#### 2. Add to `Settings` struct

In `crates/fdemon-app/src/config/types.rs`, add to the `Settings` struct (line 110–129):

```rust
pub struct Settings {
    // ... existing fields ...
    #[serde(default)]
    pub native_logs: NativeLogsSettings,
}
```

#### 3. Re-export from config module

In `crates/fdemon-app/src/config/mod.rs` (line 32–37), add `NativeLogsSettings` to the `pub use types::{...}` block.

#### 4. Add to default config TOML template

In `crates/fdemon-app/src/config/settings.rs`, find the `init_config_dir()` function (line 417–458) where the default TOML is written. Add:

```toml
# Native platform log capture (Android logcat, macOS log stream)
[native_logs]
# enabled = true                    # Master toggle (default: true)
# exclude_tags = ["flutter"]        # Tags to exclude (default: ["flutter"])
# include_tags = []                 # If set, ONLY show these tags (overrides exclude)
# min_level = "info"                # Minimum priority: "verbose", "debug", "info", "warning", "error"
```

### Acceptance Criteria

1. `NativeLogsSettings::default()` returns `enabled: true`, `exclude_tags: ["flutter"]`, `include_tags: []`, `min_level: "info"`
2. `Settings::default()` includes `native_logs: NativeLogsSettings::default()`
3. TOML with `[native_logs]\nenabled = false` deserializes correctly
4. TOML with no `[native_logs]` section deserializes to defaults (via `#[serde(default)]`)
5. TOML with `include_tags = ["GoLog", "MyPlugin"]` deserializes correctly
6. `should_include_tag("flutter")` returns `false` with default settings
7. `should_include_tag("GoLog")` returns `true` with default settings
8. `should_include_tag("GoLog")` returns `false` when `include_tags = ["OtherTag"]`
9. Tag matching is case-insensitive (`"Flutter"` matches `"flutter"` in exclude list)
10. Workspace compiles and all existing tests pass

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_native_logs_settings_default() {
        let settings = NativeLogsSettings::default();
        assert!(settings.enabled);
        assert_eq!(settings.exclude_tags, vec!["flutter".to_string()]);
        assert!(settings.include_tags.is_empty());
        assert_eq!(settings.min_level, "info");
    }

    #[test]
    fn test_should_include_tag_default_excludes_flutter() {
        let settings = NativeLogsSettings::default();
        assert!(!settings.should_include_tag("flutter"));
        assert!(!settings.should_include_tag("Flutter")); // case-insensitive
        assert!(settings.should_include_tag("GoLog"));
        assert!(settings.should_include_tag("OkHttp"));
    }

    #[test]
    fn test_should_include_tag_whitelist_mode() {
        let settings = NativeLogsSettings {
            include_tags: vec!["GoLog".to_string(), "MyPlugin".to_string()],
            ..Default::default()
        };
        assert!(settings.should_include_tag("GoLog"));
        assert!(settings.should_include_tag("golog")); // case-insensitive
        assert!(settings.should_include_tag("MyPlugin"));
        assert!(!settings.should_include_tag("OkHttp"));
        // include_tags overrides exclude_tags
        assert!(!settings.should_include_tag("flutter"));
    }

    #[test]
    fn test_native_logs_settings_toml_deserialization() {
        let toml_str = r#"
            [native_logs]
            enabled = false
            exclude_tags = ["flutter", "art"]
            min_level = "debug"
        "#;
        // Wrap in a Settings-like struct for testing
        #[derive(Deserialize)]
        struct TestConfig {
            native_logs: NativeLogsSettings,
        }
        let config: TestConfig = toml::from_str(toml_str).unwrap();
        assert!(!config.native_logs.enabled);
        assert_eq!(config.native_logs.exclude_tags, vec!["flutter", "art"]);
        assert_eq!(config.native_logs.min_level, "debug");
    }

    #[test]
    fn test_native_logs_settings_missing_section_uses_defaults() {
        let toml_str = "";
        let settings: Settings = toml::from_str(toml_str).unwrap();
        assert!(settings.native_logs.enabled);
        assert_eq!(settings.native_logs.exclude_tags, vec!["flutter".to_string()]);
    }
}
```

### Notes

- The `min_level` field is stored as a `String` rather than `NativeLogPriority` to avoid a dependency from `fdemon-app/config` on the core `NativeLogPriority` type at deserialization time. The consumer (app layer integration, task 07) parses this string into `NativeLogPriority` at runtime.
- Case-insensitive tag matching is important because Android logcat tags are case-sensitive but users shouldn't need to remember exact casing in config.
- The commented-out defaults in the TOML template follow the existing convention where all settings are commented out to show available options without overriding defaults.

---

## Completion Summary

**Status:** Not Started
