## Task: Config Module - TOML Parsing and Types

**Objective**: Create a configuration module with typed structs for launch configurations and settings, implementing TOML parsing for the `.fdemon/` directory structure.

**Depends on**: None (foundational task for Phase 3)

---

### Scope

- `src/config/mod.rs`: **NEW** - Module declaration and re-exports
- `src/config/types.rs`: **NEW** - Core configuration types (LaunchConfig, Settings, etc.)
- `src/config/settings.rs`: **NEW** - Parse `.fdemon/config.toml`
- `src/config/launch.rs`: **NEW** - Parse `.fdemon/launch.toml`
- `src/lib.rs`: Add `pub mod config;`
- `Cargo.toml`: Add `toml = "0.8"` dependency

---

### Implementation Details

#### Core Types (`src/config/types.rs`)

```rust
use std::collections::HashMap;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};

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
#[derive(Debug, Clone, Deserialize, Serialize)]
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

impl Default for Settings {
    fn default() -> Self {
        Self {
            behavior: BehaviorSettings::default(),
            watcher: WatcherSettings::default(),
            ui: UiSettings::default(),
            devtools: DevToolsSettings::default(),
        }
    }
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
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            log_buffer_size: default_log_buffer_size(),
            show_timestamps: true,
            compact_logs: false,
            theme: default_theme(),
        }
    }
}

fn default_log_buffer_size() -> usize {
    10_000
}

fn default_theme() -> String {
    "default".to_string()
}

/// DevTools settings
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DevToolsSettings {
    /// Auto-open DevTools when app starts
    #[serde(default)]
    pub auto_open: bool,
    
    /// Browser to use (empty = system default)
    #[serde(default)]
    pub browser: String,
}

impl Default for DevToolsSettings {
    fn default() -> Self {
        Self {
            auto_open: false,
            browser: String::new(),
        }
    }
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
```

#### Settings Parser (`src/config/settings.rs`)

```rust
use std::path::Path;
use tracing::{debug, warn};
use crate::common::prelude::*;
use super::types::Settings;

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
    fn test_init_config_dir() {
        let temp = tempdir().unwrap();
        
        init_config_dir(temp.path()).unwrap();
        
        assert!(temp.path().join(".fdemon").exists());
        assert!(temp.path().join(".fdemon/config.toml").exists());
    }
}
```

#### Launch Config Parser (`src/config/launch.rs`)

```rust
use std::path::Path;
use tracing::{debug, warn};
use crate::common::prelude::*;
use super::types::{LaunchConfig, LaunchFile, ResolvedLaunchConfig, ConfigSource};

const LAUNCH_FILENAME: &str = "launch.toml";
const FDEMON_DIR: &str = ".fdemon";

/// Load launch configurations from .fdemon/launch.toml
pub fn load_launch_configs(project_path: &Path) -> Vec<ResolvedLaunchConfig> {
    let launch_path = project_path.join(FDEMON_DIR).join(LAUNCH_FILENAME);
    
    if !launch_path.exists() {
        debug!("No launch file at {:?}", launch_path);
        return Vec::new();
    }
    
    match std::fs::read_to_string(&launch_path) {
        Ok(content) => match toml::from_str::<LaunchFile>(&content) {
            Ok(launch_file) => {
                debug!("Loaded {} configurations from {:?}", 
                    launch_file.configurations.len(), launch_path);
                launch_file.configurations
                    .into_iter()
                    .map(|config| ResolvedLaunchConfig {
                        config,
                        source: ConfigSource::FDemon,
                    })
                    .collect()
            }
            Err(e) => {
                warn!("Failed to parse {:?}: {}", launch_path, e);
                Vec::new()
            }
        },
        Err(e) => {
            warn!("Failed to read {:?}: {}", launch_path, e);
            Vec::new()
        }
    }
}

/// Get all auto-start configurations
pub fn get_auto_start_configs(configs: &[ResolvedLaunchConfig]) -> Vec<&ResolvedLaunchConfig> {
    configs.iter().filter(|c| c.config.auto_start).collect()
}

/// Find a configuration by name (case-insensitive)
pub fn find_config_by_name<'a>(
    configs: &'a [ResolvedLaunchConfig], 
    name: &str
) -> Option<&'a ResolvedLaunchConfig> {
    let name_lower = name.to_lowercase();
    configs.iter()
        .find(|c| c.config.name.to_lowercase() == name_lower)
}

/// Create default launch.toml file
pub fn init_launch_file(project_path: &Path) -> Result<()> {
    let fdemon_dir = project_path.join(FDEMON_DIR);
    
    if !fdemon_dir.exists() {
        std::fs::create_dir_all(&fdemon_dir)
            .map_err(|e| Error::config(format!("Failed to create .fdemon dir: {}", e)))?;
    }
    
    let launch_path = fdemon_dir.join(LAUNCH_FILENAME);
    if !launch_path.exists() {
        let default_content = r#"# Flutter Demon Launch Configurations
# See: https://github.com/example/flutter-demon#launch-configurations

[[configurations]]
name = "Debug"
device = "auto"         # "auto", device ID, or platform (e.g., "ios", "android")
mode = "debug"          # debug | profile | release
# flavor = "development"
# entry_point = "lib/main.dart"
# auto_start = false

# [configurations.dart_defines]
# API_URL = "https://dev.example.com"
# DEBUG_MODE = "true"

# [[configurations]]
# name = "Release iOS"
# device = "ios"
# mode = "release"
# flavor = "production"
# extra_args = ["--obfuscate", "--split-debug-info=build/symbols"]
"#;
        std::fs::write(&launch_path, default_content)
            .map_err(|e| Error::config(format!("Failed to write launch.toml: {}", e)))?;
    }
    
    Ok(())
}

impl LaunchConfig {
    /// Build flutter run arguments from this configuration
    pub fn build_flutter_args(&self, device_id: &str) -> Vec<String> {
        let mut args = vec![
            "run".to_string(),
            "--machine".to_string(),
            "-d".to_string(),
            device_id.to_string(),
            self.mode.as_arg().to_string(),
        ];
        
        // Add entry point if specified
        if let Some(ref entry) = self.entry_point {
            args.push("-t".to_string());
            args.push(entry.to_string_lossy().to_string());
        }
        
        // Add flavor if specified
        if let Some(ref flavor) = self.flavor {
            args.push("--flavor".to_string());
            args.push(flavor.clone());
        }
        
        // Add dart defines
        for (key, value) in &self.dart_defines {
            args.push("--dart-define".to_string());
            args.push(format!("{}={}", key, value));
        }
        
        // Add extra args
        args.extend(self.extra_args.clone());
        
        args
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use super::super::types::FlutterMode;
    
    #[test]
    fn test_load_launch_configs_empty() {
        let temp = tempdir().unwrap();
        let configs = load_launch_configs(temp.path());
        assert!(configs.is_empty());
    }
    
    #[test]
    fn test_load_launch_configs() {
        let temp = tempdir().unwrap();
        let fdemon_dir = temp.path().join(".fdemon");
        std::fs::create_dir_all(&fdemon_dir).unwrap();
        
        let content = r#"
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
        std::fs::write(fdemon_dir.join("launch.toml"), content).unwrap();
        
        let configs = load_launch_configs(temp.path());
        
        assert_eq!(configs.len(), 2);
        assert_eq!(configs[0].config.name, "Development");
        assert_eq!(configs[0].config.device, "iphone");
        assert!(configs[0].config.auto_start);
        assert_eq!(configs[0].source, ConfigSource::FDemon);
        
        assert_eq!(configs[1].config.name, "Production");
        assert_eq!(configs[1].config.mode, FlutterMode::Release);
        assert_eq!(configs[1].config.flavor, Some("production".to_string()));
    }
    
    #[test]
    fn test_get_auto_start_configs() {
        let configs = vec![
            ResolvedLaunchConfig {
                config: LaunchConfig {
                    name: "A".to_string(),
                    auto_start: true,
                    ..Default::default()
                },
                source: ConfigSource::FDemon,
            },
            ResolvedLaunchConfig {
                config: LaunchConfig {
                    name: "B".to_string(),
                    auto_start: false,
                    ..Default::default()
                },
                source: ConfigSource::FDemon,
            },
        ];
        
        let auto = get_auto_start_configs(&configs);
        assert_eq!(auto.len(), 1);
        assert_eq!(auto[0].config.name, "A");
    }
    
    #[test]
    fn test_build_flutter_args() {
        let config = LaunchConfig {
            name: "Test".to_string(),
            device: "iphone".to_string(),
            mode: FlutterMode::Debug,
            flavor: Some("dev".to_string()),
            dart_defines: [("API".to_string(), "test.com".to_string())]
                .into_iter().collect(),
            extra_args: vec!["--verbose".to_string()],
            ..Default::default()
        };
        
        let args = config.build_flutter_args("iphone-123");
        
        assert!(args.contains(&"run".to_string()));
        assert!(args.contains(&"--machine".to_string()));
        assert!(args.contains(&"-d".to_string()));
        assert!(args.contains(&"iphone-123".to_string()));
        assert!(args.contains(&"--debug".to_string()));
        assert!(args.contains(&"--flavor".to_string()));
        assert!(args.contains(&"dev".to_string()));
        assert!(args.contains(&"--dart-define".to_string()));
        assert!(args.contains(&"API=test.com".to_string()));
        assert!(args.contains(&"--verbose".to_string()));
    }
}
```

#### Module Re-exports (`src/config/mod.rs`)

```rust
//! Configuration file parsing for Flutter Demon
//! 
//! Supports:
//! - `.fdemon/config.toml` - Global settings
//! - `.fdemon/launch.toml` - Launch configurations
//! - `.vscode/launch.json` - VSCode compatibility (separate task)

pub mod types;
pub mod settings;
pub mod launch;

pub use types::*;
pub use settings::load_settings;
pub use launch::{load_launch_configs, get_auto_start_configs, find_config_by_name};
```

---

### Acceptance Criteria

1. [ ] `toml = "0.8"` added to Cargo.toml dependencies
2. [ ] `src/config/mod.rs` created with module structure
3. [ ] `LaunchConfig` struct deserializes from TOML correctly
4. [ ] `Settings` struct deserializes with all defaults working
5. [ ] `FlutterMode` enum serializes/deserializes as lowercase strings
6. [ ] `load_settings()` returns defaults when file doesn't exist
7. [ ] `load_settings()` logs warning on parse error and returns defaults
8. [ ] `load_launch_configs()` returns empty vec when file doesn't exist
9. [ ] `LaunchConfig::build_flutter_args()` produces correct CLI arguments
10. [ ] `init_config_dir()` creates `.fdemon/` with template files
11. [ ] All new code has unit tests
12. [ ] `cargo test` passes
13. [ ] `cargo clippy` has no warnings

---

### Testing

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    use tempfile::tempdir;
    
    #[test]
    fn test_full_config_workflow() {
        let temp = tempdir().unwrap();
        
        // Initialize creates directory and files
        init_config_dir(temp.path()).unwrap();
        init_launch_file(temp.path()).unwrap();
        
        // Can load the created files
        let settings = load_settings(temp.path());
        assert!(!settings.behavior.auto_start);
        
        let configs = load_launch_configs(temp.path());
        assert_eq!(configs.len(), 1); // Default "Debug" config
    }
    
    #[test]
    fn test_dart_defines_parsing() {
        let toml = r#"
[[configurations]]
name = "Test"
device = "auto"

[configurations.dart_defines]
API_URL = "https://example.com"
DEBUG = "true"
EMPTY = ""
"#;
        let launch_file: LaunchFile = toml::from_str(toml).unwrap();
        let config = &launch_file.configurations[0];
        
        assert_eq!(config.dart_defines.len(), 3);
        assert_eq!(config.dart_defines.get("API_URL"), Some(&"https://example.com".to_string()));
    }
}
```

---

### Notes

- Use `#[serde(default)]` extensively for forward compatibility
- All default functions should be standalone for serde
- Keep `Settings` separate from `LaunchConfig` for clear separation
- The `ConfigSource` enum enables tracking where configs came from
- `build_flutter_args()` is a convenience method for process spawning

---

### Files to Create/Modify

| File | Action |
|------|--------|
| `Cargo.toml` | Add `toml = "0.8"` to dependencies |
| `src/lib.rs` | Add `pub mod config;` |
| `src/config/mod.rs` | Create with re-exports |
| `src/config/types.rs` | Create with all type definitions |
| `src/config/settings.rs` | Create with settings parser |
| `src/config/launch.rs` | Create with launch config parser |

---

## Completion Summary

**Status**: ✅ Done

**Date Completed**: 2026-01-03

### Files Modified

| File | Action |
|------|--------|
| `Cargo.toml` | Added `toml = "0.8"` dependency |
| `src/lib.rs` | Added `pub mod config;` declaration |
| `src/config/mod.rs` | Created with re-exports |
| `src/config/types.rs` | Created with all type definitions |
| `src/config/settings.rs` | Created with settings parser |
| `src/config/launch.rs` | Created with launch config parser |

### Notable Decisions/Tradeoffs

1. **Derivable Default impls**: Used `#[derive(Default)]` where possible (e.g., `Settings`, `DevToolsSettings`) instead of manual implementations to satisfy clippy's `derivable_impls` lint.

2. **TOML deserialization test**: Fixed test `test_flutter_mode_deserialize` to use a wrapper struct since TOML requires key-value pairs rather than bare values.

3. **Comprehensive test coverage**: Added 26 new unit tests covering:
   - Type serialization/deserialization
   - Settings loading (defaults, custom, invalid TOML)
   - Launch config loading and building flutter args
   - Config initialization and idempotency

### Testing Performed

```bash
cargo check    # ✅ Passed
cargo test     # ✅ 250 tests passed (26 new config tests)
cargo clippy   # ✅ No warnings
cargo fmt      # ✅ Applied
```

### Acceptance Criteria Status

1. [x] `toml = "0.8"` added to Cargo.toml dependencies
2. [x] `src/config/mod.rs` created with module structure
3. [x] `LaunchConfig` struct deserializes from TOML correctly
4. [x] `Settings` struct deserializes with all defaults working
5. [x] `FlutterMode` enum serializes/deserializes as lowercase strings
6. [x] `load_settings()` returns defaults when file doesn't exist
7. [x] `load_settings()` logs warning on parse error and returns defaults
8. [x] `load_launch_configs()` returns empty vec when file doesn't exist
9. [x] `LaunchConfig::build_flutter_args()` produces correct CLI arguments
10. [x] `init_config_dir()` creates `.fdemon/` with template files
11. [x] All new code has unit tests
12. [x] `cargo test` passes
13. [x] `cargo clippy` has no warnings

### Risks/Limitations

- None identified. The implementation follows the plan exactly and all tests pass.