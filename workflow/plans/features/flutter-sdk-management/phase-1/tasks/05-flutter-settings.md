## Task: Flutter Config Settings

**Objective**: Add a `[flutter]` section to `config.toml` with an optional `sdk_path` field, allowing users to explicitly override the SDK detection chain with a manually configured path.

**Depends on**: None (can run in parallel with task 01)

### Scope

- `crates/fdemon-app/src/config/types.rs`: Add `FlutterSettings` struct and field to `Settings`
- `crates/fdemon-app/src/config/settings.rs`: Update default config template

### Details

#### 1. Add `FlutterSettings` struct to `types.rs`

```rust
/// Settings for Flutter SDK configuration.
///
/// Corresponds to the `[flutter]` section in config.toml.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct FlutterSettings {
    /// Explicit SDK path override. When set, this takes highest priority
    /// in the detection chain, bypassing all version manager detection.
    ///
    /// Example: `/Users/me/flutter` or `C:\flutter`
    #[serde(default)]
    pub sdk_path: Option<PathBuf>,
}
```

#### 2. Add to `Settings` struct

Add the new field alongside the existing settings groups:

```rust
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Settings {
    #[serde(default)]  pub behavior:    BehaviorSettings,
    #[serde(default)]  pub watcher:     WatcherSettings,
    #[serde(default)]  pub ui:          UiSettings,
    #[serde(default)]  pub devtools:    DevToolsSettings,
    #[serde(default)]  pub editor:      EditorSettings,
    #[serde(default)]  pub dap:         DapSettings,
    #[serde(default)]  pub native_logs: NativeLogsSettings,
    #[serde(default)]  pub flutter:     FlutterSettings,  // NEW
}
```

The `#[serde(default)]` attribute ensures that existing `config.toml` files without a `[flutter]` section deserialize correctly — `FlutterSettings::default()` is used, which has `sdk_path: None`.

#### 3. Update default config template in `settings.rs`

In the `init_fdemon_directory()` or default config generation function, add the `[flutter]` section to the template:

```toml
# [flutter]
# Explicit Flutter SDK path override (highest priority in detection chain).
# If not set, fdemon auto-detects via version managers and system PATH.
# sdk_path = "/path/to/flutter"
```

The section is commented out by default — the auto-detection chain handles the common case.

### Acceptance Criteria

1. `FlutterSettings` struct compiles with `Deserialize`, `Serialize`, `Default`, `Debug`, `Clone`
2. `Settings` struct includes `pub flutter: FlutterSettings` with `#[serde(default)]`
3. Existing `config.toml` files without `[flutter]` section deserialize without errors
4. A `config.toml` with `[flutter]\nsdk_path = "/path/to/flutter"` deserializes correctly
5. `settings.flutter.sdk_path` is `None` by default
6. `settings.flutter.sdk_path` is `Some(PathBuf)` when set
7. Default config template includes commented-out `[flutter]` section
8. `cargo test -p fdemon-app` passes (no existing test regressions)

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flutter_settings_default() {
        let settings = FlutterSettings::default();
        assert!(settings.sdk_path.is_none());
    }

    #[test]
    fn test_settings_without_flutter_section() {
        let toml_str = r#"
[behavior]
auto_start = true

[ui]
show_timestamps = false
"#;
        let settings: Settings = toml::from_str(toml_str).unwrap();
        assert!(settings.flutter.sdk_path.is_none());
    }

    #[test]
    fn test_settings_with_flutter_sdk_path() {
        let toml_str = r#"
[flutter]
sdk_path = "/Users/me/flutter"
"#;
        let settings: Settings = toml::from_str(toml_str).unwrap();
        assert_eq!(
            settings.flutter.sdk_path,
            Some(PathBuf::from("/Users/me/flutter"))
        );
    }

    #[test]
    fn test_settings_with_empty_flutter_section() {
        let toml_str = r#"
[flutter]
"#;
        let settings: Settings = toml::from_str(toml_str).unwrap();
        assert!(settings.flutter.sdk_path.is_none());
    }

    #[test]
    fn test_settings_roundtrip_serialization() {
        let mut settings = Settings::default();
        settings.flutter.sdk_path = Some(PathBuf::from("/opt/flutter"));

        let serialized = toml::to_string(&settings).unwrap();
        let deserialized: Settings = toml::from_str(&serialized).unwrap();

        assert_eq!(deserialized.flutter.sdk_path, Some(PathBuf::from("/opt/flutter")));
    }
}
```

### Notes

- **Backward-compatible by design**: `#[serde(default)]` on every field means existing config files never break.
- **`PathBuf` serialization**: `serde` handles `PathBuf` serialization as a string by default, which is exactly what we want for TOML.
- **No validation in config layer**: The config layer just stores the path. Validation (checking if the path is a valid Flutter SDK) happens in the locator (task 04) when it's used as the highest-priority strategy.
- **This task can be implemented and merged independently** of the daemon-side work since it only touches `fdemon-app` config types.

---

## Completion Summary

**Status:** Not Started
