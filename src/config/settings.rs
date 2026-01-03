//! Settings parser for .fdemon/config.toml

use super::types::Settings;
use crate::common::prelude::*;
use std::path::Path;

const CONFIG_FILENAME: &str = "config.toml";
const FDEMON_DIR: &str = ".fdemon";

/// Load settings from .fdemon/config.toml
///
/// Returns default settings if file doesn't exist or can't be parsed.
pub fn load_settings(project_path: &Path) -> Settings {
    let config_path = project_path.join(FDEMON_DIR).join(CONFIG_FILENAME);

    if !config_path.exists() {
        debug!("No config file at {:?}, using defaults", config_path);
        return Settings::default();
    }

    match std::fs::read_to_string(&config_path) {
        Ok(content) => match toml::from_str(&content) {
            Ok(settings) => {
                debug!("Loaded settings from {:?}", config_path);
                settings
            }
            Err(e) => {
                warn!("Failed to parse {:?}: {}", config_path, e);
                Settings::default()
            }
        },
        Err(e) => {
            warn!("Failed to read {:?}: {}", config_path, e);
            Settings::default()
        }
    }
}

/// Create default config files in .fdemon/ directory
pub fn init_config_dir(project_path: &Path) -> Result<()> {
    let fdemon_dir = project_path.join(FDEMON_DIR);

    if !fdemon_dir.exists() {
        std::fs::create_dir_all(&fdemon_dir)
            .map_err(|e| Error::config(format!("Failed to create .fdemon dir: {}", e)))?;
    }

    let config_path = fdemon_dir.join(CONFIG_FILENAME);
    if !config_path.exists() {
        let default_content = r#"# Flutter Demon Configuration
# See: https://github.com/example/flutter-demon#configuration

[behavior]
auto_start = false      # Set to true to skip device selection
confirm_quit = true     # Ask before quitting with running apps

[watcher]
paths = ["lib"]
debounce_ms = 500
auto_reload = true
extensions = ["dart"]

[ui]
log_buffer_size = 10000
show_timestamps = true
compact_logs = false
theme = "default"

[devtools]
auto_open = false
browser = ""            # Empty = system default
"#;
        std::fs::write(&config_path, default_content)
            .map_err(|e| Error::config(format!("Failed to write config.toml: {}", e)))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_load_settings_defaults() {
        let temp = tempdir().unwrap();
        let settings = load_settings(temp.path());

        assert!(!settings.behavior.auto_start);
        assert!(settings.behavior.confirm_quit);
        assert_eq!(settings.watcher.debounce_ms, 500);
        assert!(settings.watcher.auto_reload);
    }

    #[test]
    fn test_load_settings_custom() {
        let temp = tempdir().unwrap();
        let fdemon_dir = temp.path().join(".fdemon");
        std::fs::create_dir_all(&fdemon_dir).unwrap();

        let config = r#"
[behavior]
auto_start = true

[watcher]
debounce_ms = 1000
auto_reload = false
"#;
        std::fs::write(fdemon_dir.join("config.toml"), config).unwrap();

        let settings = load_settings(temp.path());

        assert!(settings.behavior.auto_start);
        assert_eq!(settings.watcher.debounce_ms, 1000);
        assert!(!settings.watcher.auto_reload);
    }

    #[test]
    fn test_load_settings_invalid_toml() {
        let temp = tempdir().unwrap();
        let fdemon_dir = temp.path().join(".fdemon");
        std::fs::create_dir_all(&fdemon_dir).unwrap();

        // Invalid TOML
        std::fs::write(fdemon_dir.join("config.toml"), "not valid toml {{{{").unwrap();

        // Should return defaults
        let settings = load_settings(temp.path());
        assert!(!settings.behavior.auto_start);
    }

    #[test]
    fn test_init_config_dir() {
        let temp = tempdir().unwrap();

        init_config_dir(temp.path()).unwrap();

        assert!(temp.path().join(".fdemon").exists());
        assert!(temp.path().join(".fdemon/config.toml").exists());

        // Content should be valid TOML
        let content = std::fs::read_to_string(temp.path().join(".fdemon/config.toml")).unwrap();
        let _: Settings = toml::from_str(&content).expect("Default config should be valid TOML");
    }

    #[test]
    fn test_init_config_dir_idempotent() {
        let temp = tempdir().unwrap();

        // First init
        init_config_dir(temp.path()).unwrap();

        // Modify the file
        let config_path = temp.path().join(".fdemon/config.toml");
        std::fs::write(&config_path, "[behavior]\nauto_start = true\n").unwrap();

        // Second init should not overwrite
        init_config_dir(temp.path()).unwrap();

        let content = std::fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("auto_start = true"));
    }
}
