# Task: Config Priority & Display

**Objective**: Create a unified configuration loading system that combines launch.toml and launch.json configs with proper priority ordering (launch.toml first).

**Depends on**: None

## Scope

- `src/config/priority.rs` — **NEW** Config priority loading
- `src/config/mod.rs` — Re-export new types

## Details

### New Types

```rust
// src/config/priority.rs

/// Wrapper for config with source tracking
#[derive(Debug, Clone)]
pub struct SourcedConfig {
    pub config: LaunchConfig,
    pub source: ConfigSource,
    pub display_name: String, // For UI display with source indicator
}

impl SourcedConfig {
    /// Create from ResolvedLaunchConfig
    pub fn from_resolved(resolved: ResolvedLaunchConfig) -> Self {
        let source_suffix = match resolved.source {
            ConfigSource::FDemon => "",
            ConfigSource::VSCode => " (VSCode)",
            _ => "",
        };
        Self {
            display_name: format!("{}{}", resolved.config.name, source_suffix),
            config: resolved.config,
            source: resolved.source,
        }
    }
}

/// Result of loading all configs
#[derive(Debug, Default)]
pub struct LoadedConfigs {
    /// All configs in priority order (launch.toml first, then launch.json)
    pub configs: Vec<SourcedConfig>,
    /// Index where launch.json configs start (for divider placement)
    pub vscode_start_index: Option<usize>,
    /// Whether any configs were loaded
    pub is_empty: bool,
}
```

### Functions to Implement

```rust
/// Load all launch configurations from both sources
///
/// Priority order:
/// 1. .fdemon/launch.toml configs
/// 2. .vscode/launch.json configs
///
/// Returns combined list with source tracking and divider index.
pub fn load_all_configs(project_path: &Path) -> LoadedConfigs {
    let fdemon_configs = load_launch_configs(project_path);
    let vscode_configs = load_vscode_configs(project_path);

    let mut configs: Vec<SourcedConfig> = Vec::new();

    // Add launch.toml configs first
    for resolved in fdemon_configs {
        configs.push(SourcedConfig::from_resolved(resolved));
    }

    // Track where VSCode configs start
    let vscode_start_index = if !vscode_configs.is_empty() {
        Some(configs.len())
    } else {
        None
    };

    // Add launch.json configs
    for resolved in vscode_configs {
        configs.push(SourcedConfig::from_resolved(resolved));
    }

    LoadedConfigs {
        is_empty: configs.is_empty(),
        configs,
        vscode_start_index,
    }
}

/// Find config by name (searches both sources)
pub fn find_config<'a>(
    configs: &'a LoadedConfigs,
    name: &str,
) -> Option<&'a SourcedConfig> {
    let name_lower = name.to_lowercase();
    configs.configs.iter().find(|c|
        c.config.name.to_lowercase() == name_lower
    )
}

/// Get first auto-start config (respects priority)
pub fn get_first_auto_start(configs: &LoadedConfigs) -> Option<&SourcedConfig> {
    configs.configs.iter().find(|c| c.config.auto_start)
}

/// Get first config overall (for fallback)
pub fn get_first_config(configs: &LoadedConfigs) -> Option<&SourcedConfig> {
    configs.configs.first()
}
```

### Module Registration

Update `src/config/mod.rs`:

```rust
mod priority;

pub use priority::{
    load_all_configs, find_config, get_first_auto_start, get_first_config,
    LoadedConfigs, SourcedConfig,
};
```

## Acceptance Criteria

1. `load_all_configs()` returns launch.toml configs before launch.json
2. `vscode_start_index` correctly indicates divider position
3. `find_config()` searches both sources
4. `get_first_auto_start()` respects priority (launch.toml first)
5. Empty state handled gracefully (both files missing)

## Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_load_all_configs_empty() {
        let temp = tempdir().unwrap();
        let loaded = load_all_configs(temp.path());

        assert!(loaded.is_empty);
        assert!(loaded.configs.is_empty());
        assert!(loaded.vscode_start_index.is_none());
    }

    #[test]
    fn test_load_all_configs_priority() {
        let temp = tempdir().unwrap();

        // Create launch.toml
        let fdemon_dir = temp.path().join(".fdemon");
        std::fs::create_dir_all(&fdemon_dir).unwrap();
        std::fs::write(fdemon_dir.join("launch.toml"), r#"
[[configurations]]
name = "Debug"
device = "auto"
"#).unwrap();

        // Create launch.json
        let vscode_dir = temp.path().join(".vscode");
        std::fs::create_dir_all(&vscode_dir).unwrap();
        std::fs::write(vscode_dir.join("launch.json"), r#"{
            "configurations": [
                {"name": "Flutter App", "type": "dart", "request": "launch"}
            ]
        }"#).unwrap();

        let loaded = load_all_configs(temp.path());

        assert_eq!(loaded.configs.len(), 2);
        assert_eq!(loaded.configs[0].config.name, "Debug");
        assert_eq!(loaded.configs[0].source, ConfigSource::FDemon);
        assert_eq!(loaded.configs[1].config.name, "Flutter App");
        assert_eq!(loaded.configs[1].source, ConfigSource::VSCode);
        assert_eq!(loaded.vscode_start_index, Some(1));
    }

    #[test]
    fn test_load_all_configs_only_vscode() {
        let temp = tempdir().unwrap();

        // Only create launch.json
        let vscode_dir = temp.path().join(".vscode");
        std::fs::create_dir_all(&vscode_dir).unwrap();
        std::fs::write(vscode_dir.join("launch.json"), r#"{
            "configurations": [
                {"name": "Flutter", "type": "dart", "request": "launch"}
            ]
        }"#).unwrap();

        let loaded = load_all_configs(temp.path());

        assert_eq!(loaded.configs.len(), 1);
        assert_eq!(loaded.vscode_start_index, Some(0));
    }

    #[test]
    fn test_find_config_case_insensitive() {
        let temp = tempdir().unwrap();
        let fdemon_dir = temp.path().join(".fdemon");
        std::fs::create_dir_all(&fdemon_dir).unwrap();
        std::fs::write(fdemon_dir.join("launch.toml"), r#"
[[configurations]]
name = "Development"
device = "auto"
"#).unwrap();

        let loaded = load_all_configs(temp.path());

        assert!(find_config(&loaded, "development").is_some());
        assert!(find_config(&loaded, "DEVELOPMENT").is_some());
        assert!(find_config(&loaded, "Development").is_some());
    }

    #[test]
    fn test_get_first_auto_start_priority() {
        let temp = tempdir().unwrap();

        // launch.toml without auto_start
        let fdemon_dir = temp.path().join(".fdemon");
        std::fs::create_dir_all(&fdemon_dir).unwrap();
        std::fs::write(fdemon_dir.join("launch.toml"), r#"
[[configurations]]
name = "Debug"
device = "auto"
auto_start = false
"#).unwrap();

        // launch.json with auto_start (simulated - VSCode doesn't have this)
        // In reality, only launch.toml supports auto_start

        let loaded = load_all_configs(temp.path());
        assert!(get_first_auto_start(&loaded).is_none());
    }

    #[test]
    fn test_sourced_config_display_name() {
        let resolved = ResolvedLaunchConfig {
            config: LaunchConfig {
                name: "Debug".to_string(),
                ..Default::default()
            },
            source: ConfigSource::VSCode,
        };

        let sourced = SourcedConfig::from_resolved(resolved);
        assert_eq!(sourced.display_name, "Debug (VSCode)");
    }
}
```

## Notes

- `ConfigSource` enum already exists in `src/config/types.rs`
- `ResolvedLaunchConfig` already wraps `LaunchConfig` with source
- This task adds the priority-aware loading layer on top

---

## Completion Summary

**Status:** Not Started

**Files Modified:**
- (none yet)

**Implementation Details:**
(to be filled after implementation)

**Testing Performed:**
- `cargo fmt` - Pending
- `cargo check` - Pending
- `cargo clippy -- -D warnings` - Pending
- `cargo test priority` - Pending
