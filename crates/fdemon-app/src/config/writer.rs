//! Configuration writer for .fdemon/launch.toml
//!
//! Provides functionality to write updated launch configurations back to disk,
//! including auto-save support with debouncing.

use super::priority::{LoadedConfigs, SourcedConfig};
use super::types::{ConfigSource, LaunchConfig};
use fdemon_core::prelude::*;
use fs2::FileExt;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Write updated launch configs back to .fdemon/launch.toml
///
/// Only saves configs with source == ConfigSource::FDemon.
/// VSCode configs are read-only and ignored.
pub fn save_fdemon_configs(project_path: &Path, configs: &LoadedConfigs) -> Result<()> {
    let config_path = project_path.join(".fdemon").join("launch.toml");

    // Filter to only FDemon configs
    let fdemon_configs: Vec<&SourcedConfig> = configs
        .configs
        .iter()
        .filter(|c| c.source == ConfigSource::FDemon)
        .collect();

    if fdemon_configs.is_empty() {
        // Nothing to save
        return Ok(());
    }

    // Build TOML content
    let content = build_launch_toml(&fdemon_configs)?;

    // Ensure directory exists
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| Error::config(format!("Failed to create .fdemon directory: {}", e)))?;
    }

    // Open file with exclusive lock for concurrent write protection
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&config_path)
        .map_err(|e| Error::config(format!("Failed to open launch.toml: {}", e)))?;

    // Acquire exclusive lock (blocks if another process has lock)
    file.lock_exclusive()
        .map_err(|e| Error::config(format!("Failed to lock launch.toml: {}", e)))?;

    // Write content
    use std::io::Write;
    let mut file = file;
    file.write_all(content.as_bytes())
        .map_err(|e| Error::config(format!("Failed to write launch.toml: {}", e)))?;
    file.flush()
        .map_err(|e| Error::config(format!("Failed to flush launch.toml: {}", e)))?;

    // Lock is automatically released when file is dropped
    info!("Saved launch config to {:?}", config_path);
    Ok(())
}

/// Build TOML content from configs
fn build_launch_toml(configs: &[&SourcedConfig]) -> Result<String> {
    use std::fmt::Write;

    let mut content = String::new();
    writeln!(content, "# Flutter Demon Launch Configurations")
        .map_err(|e| Error::config(format!("Failed to write header: {}", e)))?;
    writeln!(content, "# Auto-generated - manual edits will be preserved")
        .map_err(|e| Error::config(format!("Failed to write header: {}", e)))?;
    writeln!(content).map_err(|e| Error::config(format!("Failed to write header: {}", e)))?;

    for (i, config) in configs.iter().enumerate() {
        if i > 0 {
            writeln!(content)
                .map_err(|e| Error::config(format!("Failed to write separator: {}", e)))?;
        }

        write_config_section(&mut content, &config.config, &config.display_name)?;
    }

    Ok(content)
}

fn write_config_section(content: &mut String, config: &LaunchConfig, name: &str) -> Result<()> {
    use std::fmt::Write;

    writeln!(content, "[[configurations]]")
        .map_err(|e| Error::config(format!("Failed to write config section: {}", e)))?;
    writeln!(content, "name = \"{}\"", escape_toml_string(name))
        .map_err(|e| Error::config(format!("Failed to write config name: {}", e)))?;

    // Device (always write, even if default)
    writeln!(
        content,
        "device = \"{}\"",
        escape_toml_string(&config.device)
    )
    .map_err(|e| Error::config(format!("Failed to write device: {}", e)))?;

    // Mode (only if not default)
    if config.mode != crate::config::FlutterMode::Debug {
        writeln!(content, "mode = \"{}\"", config.mode)
            .map_err(|e| Error::config(format!("Failed to write mode: {}", e)))?;
    }

    // Flavor
    if let Some(ref flavor) = config.flavor {
        writeln!(content, "flavor = \"{}\"", escape_toml_string(flavor))
            .map_err(|e| Error::config(format!("Failed to write flavor: {}", e)))?;
    }

    // Entry point (program)
    if let Some(ref entry_point) = config.entry_point {
        writeln!(
            content,
            "entry_point = \"{}\"",
            escape_toml_string(&entry_point.to_string_lossy())
        )
        .map_err(|e| Error::config(format!("Failed to write entry_point: {}", e)))?;
    }

    // Dart defines (as HashMap, written as TOML table)
    if !config.dart_defines.is_empty() {
        writeln!(content)
            .map_err(|e| Error::config(format!("Failed to write dart_defines: {}", e)))?;
        writeln!(content, "[configurations.dart_defines]")
            .map_err(|e| Error::config(format!("Failed to write dart_defines: {}", e)))?;
        for (key, value) in &config.dart_defines {
            writeln!(content, "{} = \"{}\"", key, escape_toml_string(value))
                .map_err(|e| Error::config(format!("Failed to write dart_define: {}", e)))?;
        }
    }

    // Additional args (extra_args in the codebase)
    if !config.extra_args.is_empty() {
        writeln!(content, "extra_args = [")
            .map_err(|e| Error::config(format!("Failed to write extra_args: {}", e)))?;
        for arg in &config.extra_args {
            writeln!(content, "    \"{}\",", escape_toml_string(arg))
                .map_err(|e| Error::config(format!("Failed to write extra_arg: {}", e)))?;
        }
        writeln!(content, "]")
            .map_err(|e| Error::config(format!("Failed to write extra_args: {}", e)))?;
    }

    // Auto-start
    if config.auto_start {
        writeln!(content, "auto_start = true")
            .map_err(|e| Error::config(format!("Failed to write auto_start: {}", e)))?;
    }

    Ok(())
}

/// Escape a string for TOML
fn escape_toml_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

/// Update a specific config in the LoadedConfigs
pub fn update_fdemon_config(
    configs: &mut LoadedConfigs,
    config_index: usize,
    updater: impl FnOnce(&mut LaunchConfig),
) -> Result<()> {
    let config = configs
        .configs
        .get_mut(config_index)
        .ok_or_else(|| Error::config("Config not found"))?;

    // Only allow updating FDemon configs
    if config.source != ConfigSource::FDemon {
        return Err(Error::config("Cannot update non-FDemon config"));
    }

    updater(&mut config.config);
    Ok(())
}

/// Update flavor in a config
pub fn update_config_flavor(
    configs: &mut LoadedConfigs,
    config_index: usize,
    flavor: Option<String>,
) -> Result<()> {
    update_fdemon_config(configs, config_index, |config| {
        config.flavor = flavor;
    })
}

/// Update dart defines in a config
pub fn update_config_dart_defines(
    configs: &mut LoadedConfigs,
    config_index: usize,
    dart_defines: std::collections::HashMap<String, String>,
) -> Result<()> {
    update_fdemon_config(configs, config_index, |config| {
        config.dart_defines = dart_defines;
    })
}

/// Update mode in a config
pub fn update_config_mode(
    configs: &mut LoadedConfigs,
    config_index: usize,
    mode: crate::config::FlutterMode,
) -> Result<()> {
    update_fdemon_config(configs, config_index, |config| {
        config.mode = mode;
    })
}

/// Auto-saver that debounces writes to disk
///
/// Uses AtomicBool to skip overlapping saves and prevent race conditions.
/// If a save is already in progress when `schedule_save` is called, the new
/// request is skipped to avoid losing intermediate state.
pub struct ConfigAutoSaver {
    project_path: std::path::PathBuf,
    saving: Arc<AtomicBool>,
    debounce_ms: u64,
}

impl ConfigAutoSaver {
    pub fn new(project_path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            project_path: project_path.into(),
            saving: Arc::new(AtomicBool::new(false)),
            debounce_ms: 500,
        }
    }

    /// Schedule a save (debounced)
    ///
    /// Skips the save if another save is already in progress to prevent
    /// race conditions and ensure the latest state is preserved.
    pub fn schedule_save(&self, configs: LoadedConfigs) {
        let saving = self.saving.clone();
        let project_path = self.project_path.clone();
        let debounce_ms = self.debounce_ms;

        // Skip if already saving
        if saving
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            debug!("Skipping config save - already in progress");
            return;
        }

        // Spawn debounced save task
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(debounce_ms)).await;

            if let Err(e) = save_fdemon_configs(&project_path, &configs) {
                error!("Failed to auto-save config: {}", e);
            }

            // Mark save as complete
            saving.store(false, Ordering::SeqCst);
        });
    }

    /// Immediately save (bypass debounce)
    pub fn save_now(&self, configs: &LoadedConfigs) -> Result<()> {
        save_fdemon_configs(&self.project_path, configs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::types::FlutterMode;
    use tempfile::TempDir;

    #[test]
    fn test_escape_toml_string() {
        assert_eq!(escape_toml_string("hello"), "hello");
        assert_eq!(escape_toml_string("hello\"world"), "hello\\\"world");
        assert_eq!(escape_toml_string("line1\nline2"), "line1\\nline2");
        assert_eq!(escape_toml_string("tab\there"), "tab\\there");
        assert_eq!(escape_toml_string("back\\slash"), "back\\\\slash");
        assert_eq!(escape_toml_string("return\rhere"), "return\\rhere");
    }

    #[test]
    fn test_build_launch_toml_single_config() {
        let config = SourcedConfig {
            config: LaunchConfig {
                name: "Development".to_string(),
                device: "auto".to_string(),
                mode: FlutterMode::Debug,
                flavor: Some("dev".to_string()),
                entry_point: None,
                dart_defines: std::collections::HashMap::new(),
                extra_args: Vec::new(),
                auto_start: false,
            },
            source: ConfigSource::FDemon,
            display_name: "Development".to_string(),
        };

        let content = build_launch_toml(&[&config]).unwrap();

        assert!(content.contains("[[configurations]]"));
        assert!(content.contains("name = \"Development\""));
        assert!(content.contains("flavor = \"dev\""));
        assert!(content.contains("device = \"auto\""));
    }

    #[test]
    fn test_build_launch_toml_with_dart_defines() {
        let mut dart_defines = std::collections::HashMap::new();
        dart_defines.insert("API_URL".to_string(), "https://test.com".to_string());
        dart_defines.insert("DEBUG".to_string(), "true".to_string());

        let config = SourcedConfig {
            config: LaunchConfig {
                name: "Dev".to_string(),
                device: "auto".to_string(),
                mode: FlutterMode::Debug,
                flavor: None,
                entry_point: None,
                dart_defines,
                extra_args: Vec::new(),
                auto_start: false,
            },
            source: ConfigSource::FDemon,
            display_name: "Dev".to_string(),
        };

        let content = build_launch_toml(&[&config]).unwrap();

        assert!(content.contains("[configurations.dart_defines]"));
        assert!(content.contains("API_URL = \"https://test.com\""));
        assert!(content.contains("DEBUG = \"true\""));
    }

    #[test]
    fn test_build_launch_toml_with_extra_args() {
        let config = SourcedConfig {
            config: LaunchConfig {
                name: "Test".to_string(),
                device: "auto".to_string(),
                mode: FlutterMode::Debug,
                flavor: None,
                entry_point: None,
                dart_defines: std::collections::HashMap::new(),
                extra_args: vec!["--verbose".to_string(), "--trace-startup".to_string()],
                auto_start: false,
            },
            source: ConfigSource::FDemon,
            display_name: "Test".to_string(),
        };

        let content = build_launch_toml(&[&config]).unwrap();

        assert!(content.contains("extra_args = ["));
        assert!(content.contains("\"--verbose\""));
        assert!(content.contains("\"--trace-startup\""));
    }

    #[test]
    fn test_save_fdemon_configs() {
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path();

        let mut configs = LoadedConfigs::default();
        configs.configs.push(SourcedConfig {
            config: LaunchConfig {
                name: "Test".to_string(),
                device: "auto".to_string(),
                mode: FlutterMode::Debug,
                flavor: Some("test".to_string()),
                entry_point: None,
                dart_defines: std::collections::HashMap::new(),
                extra_args: Vec::new(),
                auto_start: false,
            },
            source: ConfigSource::FDemon,
            display_name: "Test".to_string(),
        });

        save_fdemon_configs(project_path, &configs).unwrap();

        let config_path = project_path.join(".fdemon").join("launch.toml");
        assert!(config_path.exists());

        let content = std::fs::read_to_string(config_path).unwrap();
        assert!(content.contains("flavor = \"test\""));
        assert!(content.contains("name = \"Test\""));
    }

    #[test]
    fn test_save_fdemon_configs_ignores_vscode() {
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path();

        let mut configs = LoadedConfigs::default();
        configs.configs.push(SourcedConfig {
            config: LaunchConfig {
                name: "VSCode Config".to_string(),
                device: "auto".to_string(),
                mode: FlutterMode::Debug,
                flavor: None,
                entry_point: None,
                dart_defines: std::collections::HashMap::new(),
                extra_args: Vec::new(),
                auto_start: false,
            },
            source: ConfigSource::VSCode,
            display_name: "VSCode Config".to_string(),
        });

        save_fdemon_configs(project_path, &configs).unwrap();

        // Should not create file since no FDemon configs
        let config_path = project_path.join(".fdemon").join("launch.toml");
        assert!(!config_path.exists());
    }

    #[test]
    fn test_save_fdemon_configs_mixed_sources() {
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path();

        let mut configs = LoadedConfigs::default();
        configs.configs.push(SourcedConfig {
            config: LaunchConfig {
                name: "FDemon Config".to_string(),
                device: "auto".to_string(),
                mode: FlutterMode::Debug,
                flavor: Some("dev".to_string()),
                entry_point: None,
                dart_defines: std::collections::HashMap::new(),
                extra_args: Vec::new(),
                auto_start: false,
            },
            source: ConfigSource::FDemon,
            display_name: "FDemon Config".to_string(),
        });
        configs.configs.push(SourcedConfig {
            config: LaunchConfig {
                name: "VSCode Config".to_string(),
                device: "auto".to_string(),
                mode: FlutterMode::Debug,
                flavor: None,
                entry_point: None,
                dart_defines: std::collections::HashMap::new(),
                extra_args: Vec::new(),
                auto_start: false,
            },
            source: ConfigSource::VSCode,
            display_name: "VSCode Config".to_string(),
        });

        save_fdemon_configs(project_path, &configs).unwrap();

        let config_path = project_path.join(".fdemon").join("launch.toml");
        let content = std::fs::read_to_string(config_path).unwrap();

        // Should only contain FDemon config
        assert!(content.contains("FDemon Config"));
        assert!(!content.contains("VSCode Config"));
    }

    #[test]
    fn test_update_config_flavor() {
        let mut configs = LoadedConfigs::default();
        configs.configs.push(SourcedConfig {
            config: LaunchConfig::default(),
            source: ConfigSource::FDemon,
            display_name: "Test".to_string(),
        });

        update_config_flavor(&mut configs, 0, Some("production".to_string())).unwrap();

        assert_eq!(
            configs.configs[0].config.flavor,
            Some("production".to_string())
        );
    }

    #[test]
    fn test_update_config_flavor_clear() {
        let mut configs = LoadedConfigs::default();
        configs.configs.push(SourcedConfig {
            config: LaunchConfig {
                flavor: Some("dev".to_string()),
                ..Default::default()
            },
            source: ConfigSource::FDemon,
            display_name: "Test".to_string(),
        });

        update_config_flavor(&mut configs, 0, None).unwrap();

        assert_eq!(configs.configs[0].config.flavor, None);
    }

    #[test]
    fn test_update_config_dart_defines() {
        let mut configs = LoadedConfigs::default();
        configs.configs.push(SourcedConfig {
            config: LaunchConfig::default(),
            source: ConfigSource::FDemon,
            display_name: "Test".to_string(),
        });

        let mut dart_defines = std::collections::HashMap::new();
        dart_defines.insert("KEY".to_string(), "value".to_string());

        update_config_dart_defines(&mut configs, 0, dart_defines).unwrap();

        assert_eq!(configs.configs[0].config.dart_defines.len(), 1);
        assert_eq!(
            configs.configs[0].config.dart_defines.get("KEY"),
            Some(&"value".to_string())
        );
    }

    #[test]
    fn test_update_config_mode() {
        let mut configs = LoadedConfigs::default();
        configs.configs.push(SourcedConfig {
            config: LaunchConfig::default(),
            source: ConfigSource::FDemon,
            display_name: "Test".to_string(),
        });

        update_config_mode(&mut configs, 0, FlutterMode::Release).unwrap();

        assert_eq!(configs.configs[0].config.mode, FlutterMode::Release);
    }

    #[test]
    fn test_cannot_update_vscode_config() {
        let mut configs = LoadedConfigs::default();
        configs.configs.push(SourcedConfig {
            config: LaunchConfig::default(),
            source: ConfigSource::VSCode,
            display_name: "Test".to_string(),
        });

        let result = update_config_flavor(&mut configs, 0, Some("test".to_string()));
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Cannot update non-FDemon config"));
    }

    #[test]
    fn test_update_fdemon_config_not_found() {
        let mut configs = LoadedConfigs::default();

        let result = update_config_flavor(&mut configs, 0, Some("test".to_string()));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Config not found"));
    }

    #[test]
    fn test_build_launch_toml_multiple_configs() {
        let config1 = SourcedConfig {
            config: LaunchConfig {
                name: "Dev".to_string(),
                device: "auto".to_string(),
                mode: FlutterMode::Debug,
                flavor: Some("dev".to_string()),
                entry_point: None,
                dart_defines: std::collections::HashMap::new(),
                extra_args: Vec::new(),
                auto_start: true,
            },
            source: ConfigSource::FDemon,
            display_name: "Dev".to_string(),
        };

        let config2 = SourcedConfig {
            config: LaunchConfig {
                name: "Prod".to_string(),
                device: "ios".to_string(),
                mode: FlutterMode::Release,
                flavor: Some("prod".to_string()),
                entry_point: None,
                dart_defines: std::collections::HashMap::new(),
                extra_args: Vec::new(),
                auto_start: false,
            },
            source: ConfigSource::FDemon,
            display_name: "Prod".to_string(),
        };

        let content = build_launch_toml(&[&config1, &config2]).unwrap();

        // Check both configs are present
        assert!(content.contains("name = \"Dev\""));
        assert!(content.contains("name = \"Prod\""));
        assert!(content.contains("flavor = \"dev\""));
        assert!(content.contains("flavor = \"prod\""));
        assert!(content.contains("mode = \"release\""));
        assert!(content.contains("auto_start = true"));

        // Check they're separated
        let config_count = content.matches("[[configurations]]").count();
        assert_eq!(config_count, 2);
    }

    #[test]
    fn test_write_config_section_escapes_special_chars() {
        let config = LaunchConfig {
            name: "Test \"quoted\"".to_string(),
            device: "auto".to_string(),
            mode: FlutterMode::Debug,
            flavor: Some("dev\nwith\nnewlines".to_string()),
            entry_point: None,
            dart_defines: std::collections::HashMap::new(),
            extra_args: Vec::new(),
            auto_start: false,
        };

        let mut content = String::new();
        write_config_section(&mut content, &config, "Test \"quoted\"").unwrap();

        assert!(content.contains("Test \\\"quoted\\\""));
        assert!(content.contains("dev\\nwith\\nnewlines"));
    }

    #[test]
    fn test_save_fdemon_configs_roundtrip() {
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path();

        let mut dart_defines = std::collections::HashMap::new();
        dart_defines.insert("API_URL".to_string(), "https://test.com".to_string());

        let mut configs = LoadedConfigs::default();
        configs.configs.push(SourcedConfig {
            config: LaunchConfig {
                name: "Dev".to_string(),
                device: "iphone".to_string(),
                mode: FlutterMode::Profile,
                flavor: Some("development".to_string()),
                entry_point: Some("lib/main_dev.dart".into()),
                dart_defines,
                extra_args: vec!["--verbose".to_string()],
                auto_start: true,
            },
            source: ConfigSource::FDemon,
            display_name: "Dev".to_string(),
        });

        save_fdemon_configs(project_path, &configs).unwrap();

        // Read back and verify
        let content = std::fs::read_to_string(project_path.join(".fdemon/launch.toml")).unwrap();
        assert!(content.contains("name = \"Dev\""));
        assert!(content.contains("device = \"iphone\""));
        assert!(content.contains("mode = \"profile\""));
        assert!(content.contains("flavor = \"development\""));
        assert!(content.contains("entry_point = \"lib/main_dev.dart\""));
        assert!(content.contains("[configurations.dart_defines]"));
        assert!(content.contains("API_URL = \"https://test.com\""));
        assert!(content.contains("extra_args = ["));
        assert!(content.contains("\"--verbose\""));
        assert!(content.contains("auto_start = true"));
    }
}
