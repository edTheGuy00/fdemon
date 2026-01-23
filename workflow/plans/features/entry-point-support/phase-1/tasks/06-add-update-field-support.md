## Task: Add entry_point case to update_launch_config_field()

**Objective**: Enable auto-save of `entry_point` changes to `.fdemon/launch.toml` by adding support in `update_launch_config_field()`.

**Depends on**: None (independent of other tasks)

### Scope

- `src/config/launch.rs`: Add `entry_point` case to `update_launch_config_field()` function

### Details

The `update_launch_config_field()` function allows updating individual fields of a launch config and saving to disk. Currently it handles: `name`, `device`, `mode`, `flavor`, `auto_start`. Add `entry_point`.

#### Current implementation (around line 208-249):

```rust
pub fn update_launch_config_field(
    project_path: &Path,
    config_name: &str,
    field: &str,
    value: &str,
) -> Result<()> {
    // ... load configs ...

    match field {
        "name" => config.name = value.to_string(),
        "device" => config.device = value.to_string(),
        "mode" => {
            config.mode = match value.to_lowercase().as_str() {
                "debug" => FlutterMode::Debug,
                "profile" => FlutterMode::Profile,
                "release" => FlutterMode::Release,
                _ => return Err(Error::config(format!("Invalid mode: {}", value))),
            };
        }
        "flavor" => {
            config.flavor = if value.is_empty() {
                None
            } else {
                Some(value.to_string())
            };
        }
        "auto_start" => {
            config.auto_start = value.to_lowercase() == "true";
        }
        _ => return Err(Error::config(format!("Unknown field: {}", field))),
    }

    save_launch_configs(project_path, &configs)
}
```

#### Updated implementation:

```rust
pub fn update_launch_config_field(
    project_path: &Path,
    config_name: &str,
    field: &str,
    value: &str,
) -> Result<()> {
    // ... load configs ...

    match field {
        "name" => config.name = value.to_string(),
        "device" => config.device = value.to_string(),
        "mode" => {
            config.mode = match value.to_lowercase().as_str() {
                "debug" => FlutterMode::Debug,
                "profile" => FlutterMode::Profile,
                "release" => FlutterMode::Release,
                _ => return Err(Error::config(format!("Invalid mode: {}", value))),
            };
        }
        "flavor" => {
            config.flavor = if value.is_empty() {
                None
            } else {
                Some(value.to_string())
            };
        }
        "entry_point" => {  // ADD THIS CASE
            config.entry_point = if value.is_empty() {
                None
            } else {
                Some(std::path::PathBuf::from(value))
            };
        }
        "auto_start" => {
            config.auto_start = value.to_lowercase() == "true";
        }
        _ => return Err(Error::config(format!("Unknown field: {}", field))),
    }

    save_launch_configs(project_path, &configs)
}
```

### Acceptance Criteria

1. `update_launch_config_field()` accepts `"entry_point"` as a valid field
2. Empty value sets `entry_point` to `None`
3. Non-empty value sets `entry_point` to `Some(PathBuf::from(value))`
4. Changes are persisted to `.fdemon/launch.toml`
5. Existing tests continue to pass

### Testing

Add these tests to `src/config/launch.rs` in the `mod tests` block:

```rust
#[test]
fn test_update_launch_config_field_entry_point_set() {
    let temp = tempdir().unwrap();

    save_launch_configs(
        temp.path(),
        &[LaunchConfig {
            name: "Dev".to_string(),
            ..Default::default()
        }],
    )
    .unwrap();

    // Set entry_point
    update_launch_config_field(temp.path(), "Dev", "entry_point", "lib/main_dev.dart").unwrap();

    let loaded = load_launch_configs(temp.path());
    assert_eq!(
        loaded[0].config.entry_point,
        Some(std::path::PathBuf::from("lib/main_dev.dart"))
    );
}

#[test]
fn test_update_launch_config_field_entry_point_clear() {
    let temp = tempdir().unwrap();

    save_launch_configs(
        temp.path(),
        &[LaunchConfig {
            name: "Dev".to_string(),
            entry_point: Some("lib/main_dev.dart".into()),
            ..Default::default()
        }],
    )
    .unwrap();

    // Clear entry_point with empty string
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
        flavor: Some("development".to_string()),
        ..Default::default()
    }];

    save_launch_configs(temp.path(), &configs).unwrap();

    // Verify file content
    let content = std::fs::read_to_string(temp.path().join(".fdemon/launch.toml")).unwrap();
    assert!(content.contains("entry_point"));
    assert!(content.contains("lib/main_dev.dart"));

    // Verify roundtrip
    let loaded = load_launch_configs(temp.path());
    assert_eq!(
        loaded[0].config.entry_point,
        Some(std::path::PathBuf::from("lib/main_dev.dart"))
    );
}
```

### Notes

- This task is independent and can be done in parallel with tasks 1-5
- Follows the exact same pattern as `flavor` handling
- Required for Phase 3 auto-save functionality
- The TOML serialization already works (serde attributes are correct)

---

## Completion Summary

**Status:** Not Started
