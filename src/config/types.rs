//! Configuration types for Flutter Demon
//!
//! Defines:
//! - `LaunchConfig` - A single launch configuration
//! - `Settings` - Global application settings
//! - Related sub-types and enums

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// A single launch configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct LaunchConfig {
    /// Display name for this configuration
    pub name: String,

    /// Target device ID, platform prefix, or "auto" for first available
    #[serde(default = "default_device")]
    pub device: String,

    /// Flutter mode: debug, profile, or release
    #[serde(default = "default_mode")]
    pub mode: FlutterMode,

    /// Build flavor (e.g., "development", "production")
    #[serde(default)]
    pub flavor: Option<String>,

    /// Entry point (defaults to lib/main.dart)
    #[serde(default)]
    pub entry_point: Option<PathBuf>,

    /// Dart defines (--dart-define)
    #[serde(default)]
    pub dart_defines: HashMap<String, String>,

    /// Additional arguments to pass to flutter run
    #[serde(default)]
    pub extra_args: Vec<String>,

    /// Whether to start this config automatically
    #[serde(default)]
    pub auto_start: bool,
}

impl Default for LaunchConfig {
    fn default() -> Self {
        Self {
            name: "Default".to_string(),
            device: default_device(),
            mode: default_mode(),
            flavor: None,
            entry_point: None,
            dart_defines: HashMap::new(),
            extra_args: Vec::new(),
            auto_start: false,
        }
    }
}

fn default_device() -> String {
    "auto".to_string()
}

fn default_mode() -> FlutterMode {
    FlutterMode::Debug
}

/// Flutter build mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FlutterMode {
    #[default]
    Debug,
    Profile,
    Release,
}

impl FlutterMode {
    pub fn as_arg(&self) -> &'static str {
        match self {
            FlutterMode::Debug => "--debug",
            FlutterMode::Profile => "--profile",
            FlutterMode::Release => "--release",
        }
    }
}

impl std::fmt::Display for FlutterMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FlutterMode::Debug => write!(f, "debug"),
            FlutterMode::Profile => write!(f, "profile"),
            FlutterMode::Release => write!(f, "release"),
        }
    }
}

/// Launch configurations file (.fdemon/launch.toml)
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct LaunchFile {
    /// List of launch configurations
    #[serde(default)]
    pub configurations: Vec<LaunchConfig>,
}

/// Application settings (.fdemon/config.toml)
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Settings {
    #[serde(default)]
    pub behavior: BehaviorSettings,

    #[serde(default)]
    pub watcher: WatcherSettings,

    #[serde(default)]
    pub ui: UiSettings,

    #[serde(default)]
    pub devtools: DevToolsSettings,
}

/// Behavior settings
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BehaviorSettings {
    /// If false, show device selector on startup
    #[serde(default)]
    pub auto_start: bool,

    /// Ask before quitting with running apps
    #[serde(default = "default_true")]
    pub confirm_quit: bool,
}

impl Default for BehaviorSettings {
    fn default() -> Self {
        Self {
            auto_start: false,
            confirm_quit: true,
        }
    }
}

/// File watcher settings
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WatcherSettings {
    /// Paths to watch (relative to project root)
    #[serde(default = "default_watch_paths")]
    pub paths: Vec<String>,

    /// Debounce duration in milliseconds
    #[serde(default = "default_debounce_ms")]
    pub debounce_ms: u64,

    /// Auto-reload on file change
    #[serde(default = "default_true")]
    pub auto_reload: bool,

    /// File extensions to watch
    #[serde(default = "default_extensions")]
    pub extensions: Vec<String>,
}

impl Default for WatcherSettings {
    fn default() -> Self {
        Self {
            paths: default_watch_paths(),
            debounce_ms: default_debounce_ms(),
            auto_reload: true,
            extensions: default_extensions(),
        }
    }
}

fn default_watch_paths() -> Vec<String> {
    vec!["lib".to_string()]
}

fn default_debounce_ms() -> u64 {
    500
}

fn default_extensions() -> Vec<String> {
    vec!["dart".to_string()]
}

fn default_true() -> bool {
    true
}

/// UI settings
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UiSettings {
    /// Maximum number of log entries to keep
    #[serde(default = "default_log_buffer_size")]
    pub log_buffer_size: usize,

    /// Show timestamps in logs
    #[serde(default = "default_true")]
    pub show_timestamps: bool,

    /// Collapse similar consecutive logs
    #[serde(default)]
    pub compact_logs: bool,

    /// Theme name
    #[serde(default = "default_theme")]
    pub theme: String,

    /// Whether stack traces start collapsed (default: true)
    #[serde(default = "default_true")]
    pub stack_trace_collapsed: bool,

    /// Maximum frames to show when collapsed (default: 3)
    #[serde(default = "default_stack_trace_max_frames")]
    pub stack_trace_max_frames: usize,
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            log_buffer_size: default_log_buffer_size(),
            show_timestamps: true,
            compact_logs: false,
            theme: default_theme(),
            stack_trace_collapsed: true,
            stack_trace_max_frames: default_stack_trace_max_frames(),
        }
    }
}

fn default_stack_trace_max_frames() -> usize {
    3
}

fn default_log_buffer_size() -> usize {
    10_000
}

fn default_theme() -> String {
    "default".to_string()
}

/// DevTools settings
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct DevToolsSettings {
    /// Auto-open DevTools when app starts
    #[serde(default)]
    pub auto_open: bool,

    /// Browser to use (empty = system default)
    #[serde(default)]
    pub browser: String,
}

/// Source of a launch configuration (for tracking origin)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigSource {
    /// From .fdemon/launch.toml
    FDemon,
    /// From .vscode/launch.json
    VSCode,
    /// From command-line arguments
    CommandLine,
    /// Default/fallback
    Default,
}

impl std::fmt::Display for ConfigSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigSource::FDemon => write!(f, ".fdemon"),
            ConfigSource::VSCode => write!(f, ".vscode"),
            ConfigSource::CommandLine => write!(f, "CLI"),
            ConfigSource::Default => write!(f, "default"),
        }
    }
}

/// A launch configuration with metadata about its source
#[derive(Debug, Clone)]
pub struct ResolvedLaunchConfig {
    pub config: LaunchConfig,
    pub source: ConfigSource,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flutter_mode_as_arg() {
        assert_eq!(FlutterMode::Debug.as_arg(), "--debug");
        assert_eq!(FlutterMode::Profile.as_arg(), "--profile");
        assert_eq!(FlutterMode::Release.as_arg(), "--release");
    }

    #[test]
    fn test_flutter_mode_display() {
        assert_eq!(FlutterMode::Debug.to_string(), "debug");
        assert_eq!(FlutterMode::Profile.to_string(), "profile");
        assert_eq!(FlutterMode::Release.to_string(), "release");
    }

    #[test]
    fn test_flutter_mode_deserialize() {
        // TOML requires key = value, so we test via a wrapper struct
        #[derive(Debug, Deserialize)]
        struct ModeWrapper {
            mode: FlutterMode,
        }

        let wrapper: ModeWrapper = toml::from_str(r#"mode = "debug""#).unwrap();
        assert_eq!(wrapper.mode, FlutterMode::Debug);

        let wrapper: ModeWrapper = toml::from_str(r#"mode = "release""#).unwrap();
        assert_eq!(wrapper.mode, FlutterMode::Release);

        let wrapper: ModeWrapper = toml::from_str(r#"mode = "profile""#).unwrap();
        assert_eq!(wrapper.mode, FlutterMode::Profile);
    }

    #[test]
    fn test_launch_config_defaults() {
        let config = LaunchConfig::default();
        assert_eq!(config.device, "auto");
        assert_eq!(config.mode, FlutterMode::Debug);
        assert!(!config.auto_start);
    }

    #[test]
    fn test_settings_defaults() {
        let settings = Settings::default();
        assert!(!settings.behavior.auto_start);
        assert!(settings.behavior.confirm_quit);
        assert_eq!(settings.watcher.debounce_ms, 500);
        assert!(settings.watcher.auto_reload);
        assert_eq!(settings.ui.log_buffer_size, 10_000);
    }

    #[test]
    fn test_config_source_display() {
        assert_eq!(ConfigSource::FDemon.to_string(), ".fdemon");
        assert_eq!(ConfigSource::VSCode.to_string(), ".vscode");
        assert_eq!(ConfigSource::CommandLine.to_string(), "CLI");
        assert_eq!(ConfigSource::Default.to_string(), "default");
    }

    #[test]
    fn test_launch_file_deserialize() {
        let toml_content = r#"
[[configurations]]
name = "Development"
device = "iphone"
mode = "debug"
auto_start = true

[configurations.dart_defines]
API_URL = "https://dev.api.com"

[[configurations]]
name = "Production"
device = "ios"
mode = "release"
flavor = "production"
"#;

        let launch_file: LaunchFile = toml::from_str(toml_content).unwrap();
        assert_eq!(launch_file.configurations.len(), 2);

        let dev = &launch_file.configurations[0];
        assert_eq!(dev.name, "Development");
        assert_eq!(dev.device, "iphone");
        assert_eq!(dev.mode, FlutterMode::Debug);
        assert!(dev.auto_start);
        assert_eq!(
            dev.dart_defines.get("API_URL"),
            Some(&"https://dev.api.com".to_string())
        );

        let prod = &launch_file.configurations[1];
        assert_eq!(prod.name, "Production");
        assert_eq!(prod.mode, FlutterMode::Release);
        assert_eq!(prod.flavor, Some("production".to_string()));
    }

    #[test]
    fn test_settings_deserialize_partial() {
        let toml_content = r#"
[behavior]
auto_start = true

[watcher]
debounce_ms = 1000
"#;

        let settings: Settings = toml::from_str(toml_content).unwrap();
        assert!(settings.behavior.auto_start);
        assert!(settings.behavior.confirm_quit); // default
        assert_eq!(settings.watcher.debounce_ms, 1000);
        assert!(settings.watcher.auto_reload); // default
    }
}
