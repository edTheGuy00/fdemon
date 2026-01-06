//! Configuration priority loading system
//!
//! Provides unified loading of launch configurations from multiple sources
//! with proper priority ordering (launch.toml first, then launch.json).

use super::launch::load_launch_configs;
use super::types::{ConfigSource, LaunchConfig, ResolvedLaunchConfig};
use super::vscode::load_vscode_configs;
use std::path::Path;

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
#[derive(Debug, Default, Clone)]
pub struct LoadedConfigs {
    /// All configs in priority order (launch.toml first, then launch.json)
    pub configs: Vec<SourcedConfig>,
    /// Index where launch.json configs start (for divider placement)
    pub vscode_start_index: Option<usize>,
    /// Whether any configs were loaded
    pub is_empty: bool,
}

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
pub fn find_config<'a>(configs: &'a LoadedConfigs, name: &str) -> Option<&'a SourcedConfig> {
    let name_lower = name.to_lowercase();
    configs
        .configs
        .iter()
        .find(|c| c.config.name.to_lowercase() == name_lower)
}

/// Get first auto-start config (respects priority)
pub fn get_first_auto_start(configs: &LoadedConfigs) -> Option<&SourcedConfig> {
    configs.configs.iter().find(|c| c.config.auto_start)
}

/// Get first config overall (for fallback)
pub fn get_first_config(configs: &LoadedConfigs) -> Option<&SourcedConfig> {
    configs.configs.first()
}

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
        std::fs::write(
            fdemon_dir.join("launch.toml"),
            r#"
[[configurations]]
name = "Debug"
device = "auto"
"#,
        )
        .unwrap();

        // Create launch.json
        let vscode_dir = temp.path().join(".vscode");
        std::fs::create_dir_all(&vscode_dir).unwrap();
        std::fs::write(
            vscode_dir.join("launch.json"),
            r#"{
            "configurations": [
                {"name": "Flutter App", "type": "dart", "request": "launch"}
            ]
        }"#,
        )
        .unwrap();

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
        std::fs::write(
            vscode_dir.join("launch.json"),
            r#"{
            "configurations": [
                {"name": "Flutter", "type": "dart", "request": "launch"}
            ]
        }"#,
        )
        .unwrap();

        let loaded = load_all_configs(temp.path());

        assert_eq!(loaded.configs.len(), 1);
        assert_eq!(loaded.vscode_start_index, Some(0));
    }

    #[test]
    fn test_find_config_case_insensitive() {
        let temp = tempdir().unwrap();
        let fdemon_dir = temp.path().join(".fdemon");
        std::fs::create_dir_all(&fdemon_dir).unwrap();
        std::fs::write(
            fdemon_dir.join("launch.toml"),
            r#"
[[configurations]]
name = "Development"
device = "auto"
"#,
        )
        .unwrap();

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
        std::fs::write(
            fdemon_dir.join("launch.toml"),
            r#"
[[configurations]]
name = "Debug"
device = "auto"
auto_start = false
"#,
        )
        .unwrap();

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

    #[test]
    fn test_sourced_config_display_name_fdemon() {
        let resolved = ResolvedLaunchConfig {
            config: LaunchConfig {
                name: "Debug".to_string(),
                ..Default::default()
            },
            source: ConfigSource::FDemon,
        };

        let sourced = SourcedConfig::from_resolved(resolved);
        assert_eq!(sourced.display_name, "Debug");
    }

    #[test]
    fn test_get_first_config() {
        let temp = tempdir().unwrap();
        let fdemon_dir = temp.path().join(".fdemon");
        std::fs::create_dir_all(&fdemon_dir).unwrap();
        std::fs::write(
            fdemon_dir.join("launch.toml"),
            r#"
[[configurations]]
name = "First"
device = "auto"

[[configurations]]
name = "Second"
device = "auto"
"#,
        )
        .unwrap();

        let loaded = load_all_configs(temp.path());

        let first = get_first_config(&loaded);
        assert!(first.is_some());
        assert_eq!(first.unwrap().config.name, "First");
    }

    #[test]
    fn test_get_first_config_empty() {
        let temp = tempdir().unwrap();
        let loaded = load_all_configs(temp.path());

        assert!(get_first_config(&loaded).is_none());
    }

    #[test]
    fn test_get_first_auto_start_finds_config() {
        let temp = tempdir().unwrap();
        let fdemon_dir = temp.path().join(".fdemon");
        std::fs::create_dir_all(&fdemon_dir).unwrap();
        std::fs::write(
            fdemon_dir.join("launch.toml"),
            r#"
[[configurations]]
name = "Manual"
device = "auto"
auto_start = false

[[configurations]]
name = "Auto"
device = "auto"
auto_start = true
"#,
        )
        .unwrap();

        let loaded = load_all_configs(temp.path());

        let auto_start = get_first_auto_start(&loaded);
        assert!(auto_start.is_some());
        assert_eq!(auto_start.unwrap().config.name, "Auto");
    }

    #[test]
    fn test_vscode_start_index_none_when_only_fdemon() {
        let temp = tempdir().unwrap();
        let fdemon_dir = temp.path().join(".fdemon");
        std::fs::create_dir_all(&fdemon_dir).unwrap();
        std::fs::write(
            fdemon_dir.join("launch.toml"),
            r#"
[[configurations]]
name = "Debug"
device = "auto"
"#,
        )
        .unwrap();

        let loaded = load_all_configs(temp.path());

        assert_eq!(loaded.configs.len(), 1);
        assert!(loaded.vscode_start_index.is_none());
    }

    #[test]
    fn test_multiple_vscode_configs() {
        let temp = tempdir().unwrap();
        let vscode_dir = temp.path().join(".vscode");
        std::fs::create_dir_all(&vscode_dir).unwrap();
        std::fs::write(
            vscode_dir.join("launch.json"),
            r#"{
            "configurations": [
                {"name": "Debug", "type": "dart", "request": "launch"},
                {"name": "Profile", "type": "dart", "request": "launch"},
                {"name": "Release", "type": "dart", "request": "launch"}
            ]
        }"#,
        )
        .unwrap();

        let loaded = load_all_configs(temp.path());

        assert_eq!(loaded.configs.len(), 3);
        assert_eq!(loaded.vscode_start_index, Some(0));
        assert!(loaded
            .configs
            .iter()
            .all(|c| c.source == ConfigSource::VSCode));
    }

    #[test]
    fn test_mixed_configs_priority_order() {
        let temp = tempdir().unwrap();

        // Create 2 fdemon configs
        let fdemon_dir = temp.path().join(".fdemon");
        std::fs::create_dir_all(&fdemon_dir).unwrap();
        std::fs::write(
            fdemon_dir.join("launch.toml"),
            r#"
[[configurations]]
name = "FDemon1"
device = "auto"

[[configurations]]
name = "FDemon2"
device = "auto"
"#,
        )
        .unwrap();

        // Create 2 vscode configs
        let vscode_dir = temp.path().join(".vscode");
        std::fs::create_dir_all(&vscode_dir).unwrap();
        std::fs::write(
            vscode_dir.join("launch.json"),
            r#"{
            "configurations": [
                {"name": "VSCode1", "type": "dart", "request": "launch"},
                {"name": "VSCode2", "type": "dart", "request": "launch"}
            ]
        }"#,
        )
        .unwrap();

        let loaded = load_all_configs(temp.path());

        assert_eq!(loaded.configs.len(), 4);
        assert_eq!(loaded.configs[0].config.name, "FDemon1");
        assert_eq!(loaded.configs[1].config.name, "FDemon2");
        assert_eq!(loaded.configs[2].config.name, "VSCode1");
        assert_eq!(loaded.configs[3].config.name, "VSCode2");
        assert_eq!(loaded.vscode_start_index, Some(2));
    }

    #[test]
    fn test_find_config_not_found() {
        let temp = tempdir().unwrap();
        let fdemon_dir = temp.path().join(".fdemon");
        std::fs::create_dir_all(&fdemon_dir).unwrap();
        std::fs::write(
            fdemon_dir.join("launch.toml"),
            r#"
[[configurations]]
name = "Debug"
device = "auto"
"#,
        )
        .unwrap();

        let loaded = load_all_configs(temp.path());

        assert!(find_config(&loaded, "NonExistent").is_none());
    }
}
