## Task: Per-Tag Configuration

**Objective**: Add per-tag priority thresholds to `NativeLogsSettings` so users can configure minimum log levels for individual native tags in `.fdemon/config.toml`.

**Depends on**: None

### Scope

- `crates/fdemon-app/src/config/types.rs`: Add `tag_overrides` field to `NativeLogsSettings`
- `crates/fdemon-app/src/config/settings.rs`: Parse `[native_logs.tags.<name>]` sections from config

### Details

#### 1. Config file format

The user-facing configuration in `.fdemon/config.toml`:

```toml
[native_logs]
enabled = true
exclude_tags = ["flutter"]
min_level = "info"               # Global minimum level

# Per-tag level overrides
[native_logs.tags.GoLog]
min_level = "debug"              # Show GoLog entries at debug+

[native_logs.tags.OkHttp]
min_level = "warning"            # Only show OkHttp warnings and above

[native_logs.tags."com.example.myplugin"]
min_level = "info"               # Subsystem-style tags work too
```

#### 2. Add `TagConfig` type

```rust
/// Per-tag configuration override.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TagConfig {
    /// Minimum log level for this tag (overrides the global `min_level`).
    /// Options: "verbose", "debug", "info", "warning", "error"
    pub min_level: Option<String>,
}
```

#### 3. Extend `NativeLogsSettings`

```rust
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeLogsSettings {
    /// Master toggle for native log capture (default: true).
    pub enabled: bool,
    /// Tags to exclude from capture (default: ["flutter"]).
    pub exclude_tags: Vec<String>,
    /// If non-empty, only capture these tags (overrides exclude_tags).
    pub include_tags: Vec<String>,
    /// Global minimum log level (default: "debug").
    pub min_level: String,
    /// Per-tag configuration overrides.
    /// Key: tag name (e.g., "GoLog", "OkHttp", "com.example.myplugin").
    #[serde(default)]
    pub tags: HashMap<String, TagConfig>,
}

impl Default for NativeLogsSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            exclude_tags: vec!["flutter".to_string()],
            include_tags: vec![],
            min_level: "debug".to_string(),
            tags: HashMap::new(),
        }
    }
}
```

#### 4. Add `effective_min_level()` helper

A helper that resolves the effective minimum level for a given tag, considering per-tag overrides:

```rust
impl NativeLogsSettings {
    /// Get the effective minimum log level for a specific tag.
    ///
    /// Returns the per-tag override if configured, otherwise the global `min_level`.
    pub fn effective_min_level(&self, tag: &str) -> &str {
        self.tags
            .get(tag)
            .and_then(|tc| tc.min_level.as_deref())
            .unwrap_or(&self.min_level)
    }
}
```

#### 5. TOML serialization verification

The `[native_logs.tags.GoLog]` section in TOML maps to `HashMap<String, TagConfig>` via serde. Verify that:
- Tags with dots in the name (e.g., `"com.example.myplugin"`) require quoting in TOML: `[native_logs.tags."com.example.myplugin"]`
- Empty `tags` section (no overrides) serializes as an empty map (no TOML output)
- The `#[serde(default)]` attribute ensures missing `tags` section in config.toml results in an empty HashMap

#### 6. Wire into capture backends

The per-tag level override should be applied in the native log capture backends (android.rs, macos.rs, ios.rs). The `NativeLogsSettings` is already passed to the capture via `UpdateAction::StartNativeLogCapture { settings }`.

The capture backends currently use `parse_min_priority(&config.min_level)` for global level filtering. Update to use `settings.effective_min_level(tag)` per event:

**Note**: This wiring is a code change in the daemon-layer backends, but since the `NativeLogsSettings` is passed through as config fields (`min_level`, `exclude_tags`, etc.), the per-tag level checking should happen at the **app layer** (in the `NativeLog` message handler in `update.rs`) rather than in the daemon layer. This keeps the daemon layer simple (it just captures and forwards) and the app layer handles filtering logic.

```rust
// In handler/update.rs, NativeLog handler:
Message::NativeLog { session_id, event } => {
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        // Check per-tag minimum level
        let effective_min = state.settings.native_logs.effective_min_level(&event.tag);
        let min_level = parse_log_level(effective_min);
        if let Some(min) = min_level {
            if event.level.severity() < min.severity() {
                // Below minimum level for this tag — skip
                return UpdateResult::none();
            }
        }

        handle.native_tag_state.observe_tag(&event.tag);

        if !handle.native_tag_state.is_tag_visible(&event.tag) {
            return UpdateResult::none();
        }

        let entry = LogEntry::new(
            event.level,
            LogSource::Native { tag: event.tag },
            event.message,
        );
        handle.session.queue_log(entry);
    }
    UpdateResult::none()
}
```

### Acceptance Criteria

1. `NativeLogsSettings` has a `tags: HashMap<String, TagConfig>` field
2. `TagConfig` has `min_level: Option<String>` field
3. `#[serde(default)]` ensures missing `tags` section in config.toml results in empty HashMap
4. `effective_min_level("GoLog")` returns the per-tag override when configured
5. `effective_min_level("UnknownTag")` falls back to the global `min_level`
6. TOML deserialization of `[native_logs.tags.GoLog]` sections works correctly
7. Tags with dots in the name (quoted in TOML) deserialize correctly
8. Per-tag level filtering is applied in the `NativeLog` message handler
9. Default `NativeLogsSettings` has empty `tags` HashMap
10. `cargo check --workspace` compiles
11. `cargo test -p fdemon-app` passes
12. Existing `NativeLogsSettings` tests still pass

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_settings_empty_tags() {
        let settings = NativeLogsSettings::default();
        assert!(settings.tags.is_empty());
    }

    #[test]
    fn test_effective_min_level_global_fallback() {
        let settings = NativeLogsSettings {
            min_level: "info".to_string(),
            tags: HashMap::new(),
            ..Default::default()
        };
        assert_eq!(settings.effective_min_level("GoLog"), "info");
    }

    #[test]
    fn test_effective_min_level_per_tag_override() {
        let mut settings = NativeLogsSettings::default();
        settings.tags.insert(
            "GoLog".to_string(),
            TagConfig { min_level: Some("debug".to_string()) },
        );
        assert_eq!(settings.effective_min_level("GoLog"), "debug");
        assert_eq!(settings.effective_min_level("OkHttp"), "debug"); // fallback to global
    }

    #[test]
    fn test_effective_min_level_per_tag_none_uses_global() {
        let mut settings = NativeLogsSettings::default();
        settings.tags.insert(
            "GoLog".to_string(),
            TagConfig { min_level: None },
        );
        assert_eq!(settings.effective_min_level("GoLog"), "debug"); // global default
    }

    #[test]
    fn test_toml_deserialization() {
        let toml_str = r#"
enabled = true
exclude_tags = ["flutter"]
min_level = "info"

[tags.GoLog]
min_level = "debug"

[tags.OkHttp]
min_level = "warning"
"#;
        let settings: NativeLogsSettings = toml::from_str(toml_str).unwrap();
        assert_eq!(settings.effective_min_level("GoLog"), "debug");
        assert_eq!(settings.effective_min_level("OkHttp"), "warning");
        assert_eq!(settings.effective_min_level("Unknown"), "info");
    }

    #[test]
    fn test_toml_deserialization_no_tags_section() {
        let toml_str = r#"
enabled = true
exclude_tags = ["flutter"]
min_level = "info"
"#;
        let settings: NativeLogsSettings = toml::from_str(toml_str).unwrap();
        assert!(settings.tags.is_empty());
    }

    #[test]
    fn test_toml_deserialization_dotted_tag_name() {
        let toml_str = r#"
enabled = true
min_level = "info"

[tags."com.example.myplugin"]
min_level = "debug"
"#;
        let settings: NativeLogsSettings = toml::from_str(toml_str).unwrap();
        assert_eq!(settings.effective_min_level("com.example.myplugin"), "debug");
    }
}
```

### Notes

- **Per-tag overrides happen at the app layer, not daemon layer**: The daemon capture backends use the global `min_level` for coarse filtering (to reduce channel traffic). The app layer's `NativeLog` handler applies per-tag overrides for fine-grained control. This is a two-level filtering approach: daemon does coarse, app does fine.
- **HashMap vs BTreeMap for tags config**: `HashMap` is used for config since the ordering doesn't matter for config lookup. The `NativeTagState` UI display uses `BTreeMap` (task 07) for alphabetical ordering.
- **TOML `[native_logs.tags.GoLog]` syntax**: TOML nested table syntax. Tag names with dots must be quoted: `[native_logs.tags."com.example.plugin"]`.
- **`TagConfig` is deliberately minimal**: Only `min_level` for now. Future phases could add per-tag `enabled`, `color`, or `alias` fields.
