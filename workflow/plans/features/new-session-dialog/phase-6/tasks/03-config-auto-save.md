# Task: Config Auto-Save

## Summary

Implement automatic saving of configuration changes to `.fdemon/launch.toml` when the selected config is an FDemon config.

## Files

| File | Action |
|------|--------|
| `src/config/writer.rs` | Create |
| `src/config/mod.rs` | Modify (add export) |

## Implementation

### 1. TOML writer for launch configs

```rust
// src/config/writer.rs

use std::path::Path;
use crate::common::Error;
use crate::config::{LaunchConfig, LoadedConfigs, SourcedConfig, ConfigSource};

/// Write updated launch configs back to .fdemon/launch.toml
pub fn save_fdemon_configs(
    project_path: &Path,
    configs: &LoadedConfigs,
) -> Result<(), Error> {
    let config_path = project_path.join(".fdemon").join("launch.toml");

    // Filter to only FDemon configs
    let fdemon_configs: Vec<&SourcedConfig> = configs.configs
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
            .map_err(|e| Error::recoverable(format!("Failed to create .fdemon directory: {}", e)))?;
    }

    // Write file
    std::fs::write(&config_path, content)
        .map_err(|e| Error::recoverable(format!("Failed to write launch.toml: {}", e)))?;

    tracing::info!("Saved launch config to {:?}", config_path);
    Ok(())
}

/// Build TOML content from configs
fn build_launch_toml(configs: &[&SourcedConfig]) -> Result<String, Error> {
    use std::fmt::Write;

    let mut content = String::new();
    writeln!(content, "# Flutter Demon Launch Configurations")?;
    writeln!(content, "# Auto-generated - manual edits will be preserved")?;
    writeln!(content)?;

    for (i, config) in configs.iter().enumerate() {
        if i > 0 {
            writeln!(content)?;
        }

        write_config_section(&mut content, &config.config, &config.display_name)?;
    }

    Ok(content)
}

fn write_config_section(
    content: &mut String,
    config: &LaunchConfig,
    name: &str,
) -> Result<(), std::fmt::Error> {
    use std::fmt::Write;

    writeln!(content, "[[configurations]]")?;
    writeln!(content, "name = \"{}\"", escape_toml_string(name))?;

    // Mode (only if not default)
    if config.mode != crate::config::FlutterMode::Debug {
        writeln!(content, "mode = \"{}\"", config.mode)?;
    }

    // Flavor
    if let Some(ref flavor) = config.flavor {
        writeln!(content, "flavor = \"{}\"", escape_toml_string(flavor))?;
    }

    // Device ID
    if let Some(ref device) = config.device_id {
        writeln!(content, "device_id = \"{}\"", escape_toml_string(device))?;
    }

    // Dart defines
    if !config.dart_defines.is_empty() {
        writeln!(content, "dart_defines = [")?;
        for define in &config.dart_defines {
            writeln!(content, "    \"{}\",", escape_toml_string(define))?;
        }
        writeln!(content, "]")?;
    }

    // Program (entry point)
    if let Some(ref program) = config.program {
        writeln!(content, "program = \"{}\"", escape_toml_string(program))?;
    }

    // Additional args
    if !config.args.is_empty() {
        writeln!(content, "args = [")?;
        for arg in &config.args {
            writeln!(content, "    \"{}\",", escape_toml_string(arg))?;
        }
        writeln!(content, "]")?;
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
```

### 2. Update single config

```rust
/// Update a specific config in the LoadedConfigs
pub fn update_fdemon_config(
    configs: &mut LoadedConfigs,
    config_index: usize,
    updater: impl FnOnce(&mut LaunchConfig),
) -> Result<(), Error> {
    let config = configs.configs.get_mut(config_index)
        .ok_or_else(|| Error::recoverable("Config not found"))?;

    // Only allow updating FDemon configs
    if config.source != ConfigSource::FDemon {
        return Err(Error::recoverable("Cannot update non-FDemon config"));
    }

    updater(&mut config.config);
    Ok(())
}

/// Update flavor in a config
pub fn update_config_flavor(
    configs: &mut LoadedConfigs,
    config_index: usize,
    flavor: Option<String>,
) -> Result<(), Error> {
    update_fdemon_config(configs, config_index, |config| {
        config.flavor = flavor;
    })
}

/// Update dart defines in a config
pub fn update_config_dart_defines(
    configs: &mut LoadedConfigs,
    config_index: usize,
    dart_defines: Vec<String>,
) -> Result<(), Error> {
    update_fdemon_config(configs, config_index, |config| {
        config.dart_defines = dart_defines;
    })
}

/// Update mode in a config
pub fn update_config_mode(
    configs: &mut LoadedConfigs,
    config_index: usize,
    mode: crate::config::FlutterMode,
) -> Result<(), Error> {
    update_fdemon_config(configs, config_index, |config| {
        config.mode = mode;
    })
}
```

### 3. Auto-save integration

```rust
use std::sync::Arc;
use tokio::sync::Mutex;

/// Auto-saver that debounces writes to disk
pub struct ConfigAutoSaver {
    project_path: std::path::PathBuf,
    pending_save: Arc<Mutex<bool>>,
    debounce_ms: u64,
}

impl ConfigAutoSaver {
    pub fn new(project_path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            project_path: project_path.into(),
            pending_save: Arc::new(Mutex::new(false)),
            debounce_ms: 500,
        }
    }

    /// Schedule a save (debounced)
    pub async fn schedule_save(&self, configs: LoadedConfigs) {
        let pending = self.pending_save.clone();
        let project_path = self.project_path.clone();
        let debounce_ms = self.debounce_ms;

        // Mark as pending
        *pending.lock().await = true;

        // Spawn debounced save task
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(debounce_ms)).await;

            // Check if still pending (not cancelled by newer save)
            if *pending.lock().await {
                if let Err(e) = save_fdemon_configs(&project_path, &configs) {
                    tracing::error!("Failed to auto-save config: {}", e);
                }
                *pending.lock().await = false;
            }
        });
    }

    /// Immediately save (bypass debounce)
    pub fn save_now(&self, configs: &LoadedConfigs) -> Result<(), Error> {
        save_fdemon_configs(&self.project_path, configs)
    }
}
```

### 4. Export from config module

```rust
// src/config/mod.rs

mod writer;

pub use writer::{
    save_fdemon_configs,
    update_config_flavor,
    update_config_dart_defines,
    update_config_mode,
    ConfigAutoSaver,
};
```

## Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_escape_toml_string() {
        assert_eq!(escape_toml_string("hello"), "hello");
        assert_eq!(escape_toml_string("hello\"world"), "hello\\\"world");
        assert_eq!(escape_toml_string("line1\nline2"), "line1\\nline2");
    }

    #[test]
    fn test_build_launch_toml_single_config() {
        let config = SourcedConfig {
            config: LaunchConfig {
                name: "Development".to_string(),
                mode: FlutterMode::Debug,
                flavor: Some("dev".to_string()),
                ..Default::default()
            },
            source: ConfigSource::FDemon,
            display_name: "Development".to_string(),
        };

        let content = build_launch_toml(&[&config]).unwrap();

        assert!(content.contains("[[configurations]]"));
        assert!(content.contains("name = \"Development\""));
        assert!(content.contains("flavor = \"dev\""));
    }

    #[test]
    fn test_save_fdemon_configs() {
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path();

        let mut configs = LoadedConfigs::default();
        configs.configs.push(SourcedConfig {
            config: LaunchConfig {
                name: "Test".to_string(),
                flavor: Some("test".to_string()),
                ..Default::default()
            },
            source: ConfigSource::FDemon,
            display_name: "Test".to_string(),
        });

        save_fdemon_configs(project_path, &configs).unwrap();

        let config_path = project_path.join(".fdemon").join("launch.toml");
        assert!(config_path.exists());

        let content = std::fs::read_to_string(config_path).unwrap();
        assert!(content.contains("flavor = \"test\""));
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

        assert_eq!(configs.configs[0].config.flavor, Some("production".to_string()));
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
    }
}
```

## Verification

```bash
cargo fmt && cargo check && cargo test config_writer && cargo clippy -- -D warnings
```

## Notes

- Only FDemon configs are saved; VSCode configs are read-only
- Auto-saver uses debouncing to avoid excessive disk writes
- TOML strings are properly escaped
- Save preserves config order
- Comments are added for human readability
