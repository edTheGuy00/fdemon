//! Launch configuration parser for .fdemon/launch.toml

use super::types::{ConfigSource, FlutterMode, LaunchConfig, LaunchFile, ResolvedLaunchConfig};
use fdemon_core::prelude::*;
use std::path::Path;

const LAUNCH_FILENAME: &str = "launch.toml";
const FDEMON_DIR: &str = ".fdemon";

/// Maximum accepted byte length of a single extra CLI argument.
///
/// Chosen to be generous enough that realistic Flutter flags are never rejected
/// (e.g. a long `--split-debug-info=<path>` value with a deep build directory),
/// while still blocking accidental injection of large free-text blobs (e.g. a
/// file path accidentally placed in `extra_args` instead of `entry_point`).
/// 256 bytes covers any plausible flag or path component with significant margin.
const MAX_EXTRA_ARG_LEN: usize = 256;

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
                debug!(
                    "Loaded {} configurations from {:?}",
                    launch_file.configurations.len(),
                    launch_path
                );
                launch_file
                    .configurations
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
    name: &str,
) -> Option<&'a ResolvedLaunchConfig> {
    let name_lower = name.to_lowercase();
    configs
        .iter()
        .find(|c| c.config.name.to_lowercase() == name_lower)
}

/// Save launch configurations to .fdemon/launch.toml
///
/// Uses atomic write (temp file + rename) for safety.
pub fn save_launch_configs(project_path: &Path, configs: &[LaunchConfig]) -> Result<()> {
    let fdemon_dir = project_path.join(FDEMON_DIR);

    // Ensure directory exists
    if !fdemon_dir.exists() {
        std::fs::create_dir_all(&fdemon_dir)
            .map_err(|e| Error::config(format!("Failed to create .fdemon dir: {}", e)))?;
    }

    let launch_path = fdemon_dir.join(LAUNCH_FILENAME);
    let temp_path = fdemon_dir.join(".launch.toml.tmp");

    // Format-preserving write: each `[[configurations]]` block is matched by
    // `name` against the editor's load-time baseline, so editing one config's
    // fields rewrites only those fields and leaves every other block — and the
    // file's comments and ordering — intact. Baseline is `load_launch_configs`,
    // the same path the panel loaded from.
    let baseline_configs: Vec<LaunchConfig> = load_launch_configs(project_path)
        .into_iter()
        .map(|r| r.config)
        .collect();
    let baseline_val = toml::Value::try_from(LaunchFile {
        configurations: baseline_configs,
    })
    .map_err(|e| Error::config(format!("Failed to encode baseline launch configs: {}", e)))?;
    let current_val = toml::Value::try_from(LaunchFile {
        configurations: configs.to_vec(),
    })
    .map_err(|e| Error::config(format!("Failed to serialize launch configs: {}", e)))?;

    let existing = std::fs::read_to_string(&launch_path).unwrap_or_default();
    let is_new = existing.trim().is_empty();
    let merged = super::toml_merge::merge_array_of_tables_by_key(
        &existing,
        "configurations",
        "name",
        &baseline_val,
        &current_val,
    )
    .map_err(|e| {
        Error::config(format!(
            "Refusing to overwrite malformed {}: {}",
            launch_path.display(),
            e
        ))
    })?;
    let full_content = if is_new {
        format!("{}{}", generate_launch_header(), merged)
    } else {
        merged
    };

    // Atomic write: write to temp, then rename
    std::fs::write(&temp_path, &full_content)
        .map_err(|e| Error::config(format!("Failed to write temp file: {}", e)))?;

    std::fs::rename(&temp_path, &launch_path)
        .map_err(|e| Error::config(format!("Failed to rename temp file: {}", e)))?;

    info!("Saved launch configs to {:?}", launch_path);
    Ok(())
}

fn generate_launch_header() -> String {
    r#"# Flutter Demon Launch Configurations
# Define multiple launch configurations for different scenarios
# Generated by fdemon settings panel

"#
    .to_string()
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
# See: https://fdemon.dev/docs/configuration

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

/// Create a new launch configuration with sensible defaults
pub fn create_default_launch_config() -> LaunchConfig {
    LaunchConfig {
        name: "New Configuration".to_string(),
        device: "auto".to_string(),
        mode: FlutterMode::Debug,
        flavor: None,
        entry_point: None,
        dart_defines: std::collections::HashMap::new(),
        extra_args: Vec::new(),
        auto_start: false,
    }
}

/// Add a new launch configuration
///
/// Ensures unique name by appending counter if needed.
pub fn add_launch_config(project_path: &Path, config: LaunchConfig) -> Result<()> {
    let mut configs: Vec<LaunchConfig> = load_launch_configs(project_path)
        .into_iter()
        .map(|r| r.config)
        .collect();

    // Ensure unique name
    let base_name = config.name.clone();
    let mut new_config = config;
    let mut counter = 1;

    while configs.iter().any(|c| c.name == new_config.name) {
        new_config.name = format!("{} ({})", base_name, counter);
        counter += 1;
    }

    configs.push(new_config);
    save_launch_configs(project_path, &configs)
}

/// Delete a launch configuration by name
///
/// Reloads configs, removes matching one, saves back.
pub fn delete_launch_config(project_path: &Path, config_name: &str) -> Result<()> {
    let mut configs: Vec<LaunchConfig> = load_launch_configs(project_path)
        .into_iter()
        .map(|r| r.config)
        .collect();

    let initial_len = configs.len();
    configs.retain(|c| c.name != config_name);

    if configs.len() == initial_len {
        return Err(Error::config(format!("Config '{}' not found", config_name)));
    }

    save_launch_configs(project_path, &configs)
}

/// Update a specific field of a launch configuration
pub fn update_launch_config_field(
    project_path: &Path,
    config_name: &str,
    field: &str,
    value: &str,
) -> Result<()> {
    let mut configs: Vec<LaunchConfig> = load_launch_configs(project_path)
        .into_iter()
        .map(|r| r.config)
        .collect();

    let config = configs
        .iter_mut()
        .find(|c| c.name == config_name)
        .ok_or_else(|| Error::config(format!("Config '{}' not found", config_name)))?;

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
        "entry_point" => {
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

/// Update dart_defines for a launch configuration (Task 10c)
pub fn update_launch_config_dart_defines(
    project_path: &Path,
    config_name: &str,
    dart_defines_str: &str,
) -> Result<()> {
    let mut configs: Vec<LaunchConfig> = load_launch_configs(project_path)
        .into_iter()
        .map(|r| r.config)
        .collect();

    let config = configs
        .iter_mut()
        .find(|c| c.name == config_name)
        .ok_or_else(|| Error::config(format!("Config '{}' not found", config_name)))?;

    // Parse dart_defines from "KEY=VALUE,KEY2=VALUE2" format
    config.dart_defines = parse_dart_defines(dart_defines_str);

    save_launch_configs(project_path, &configs)
}

/// Parse dart defines from comma-separated KEY=VALUE string (Task 10c)
pub fn parse_dart_defines(s: &str) -> std::collections::HashMap<String, String> {
    if s.trim().is_empty() {
        return std::collections::HashMap::new();
    }

    s.split(',')
        .filter_map(|pair| {
            let trimmed = pair.trim();
            if !trimmed.contains('=') {
                return None; // Skip entries without '='
            }
            let mut parts = trimmed.splitn(2, '=');
            let key = parts.next()?.trim();
            let value = parts.next().unwrap_or("").trim();
            if key.is_empty() {
                None
            } else {
                Some((key.to_string(), value.to_string()))
            }
        })
        .collect()
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

        // Add extra args (validated positionally). Flags must start with '-', but
        // a flag that takes its value as the *next* list entry (split syntax, e.g.
        // VS Code `toolArgs` `["--web-port", "8080"]`) must keep that value even
        // though it does not start with '-'. So: a token starting with '-' is a
        // flag; the single token immediately following a flag (when the flag has
        // no inline `=value`) is accepted as that flag's value. A leading bare
        // token with no preceding flag is a stray positional and is dropped.
        //
        // Every retained token must contain no NUL bytes and stay within the
        // bounded length cap. Entries that fail are silently dropped with a
        // warning — we never reject the whole config for a single malformed arg,
        // and there is no shell-evaluation risk because args reach
        // `Command::args()` as separate, non-shell-evaluated elements.
        let mut expect_value = false;
        for arg in &self.extra_args {
            let is_flag = arg.starts_with('-');
            // Accept flags, and value tokens that follow a flag lacking inline `=`.
            let positionally_ok = is_flag || expect_value;
            let ok = positionally_ok && !arg.contains('\0') && arg.len() <= MAX_EXTRA_ARG_LEN;
            if ok {
                args.push(arg.clone());
                // The next token is this flag's value only when this is a flag
                // without an inline `=value`. A consumed value never expects a
                // following value itself.
                expect_value = is_flag && !arg.contains('=');
            } else {
                warn!("Ignoring malformed extra_arg: {:?}", arg);
                expect_value = false;
            }
        }

        args
    }
}

#[cfg(test)]
mod tests {
    use super::super::types::FlutterMode;
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_load_launch_configs_empty() {
        let temp = tempdir().unwrap();
        let configs = load_launch_configs(temp.path());
        assert!(configs.is_empty());
    }

    #[test]
    fn save_launch_configs_preserves_comments_and_other_blocks() {
        let temp = tempdir().unwrap();
        let fdemon_dir = temp.path().join(".fdemon");
        std::fs::create_dir_all(&fdemon_dir).unwrap();

        let original = "\
# my launch configs — keep these notes
[[configurations]]
name = \"Dev\"  # everyday driver
device = \"auto\"
mode = \"debug\"

[[configurations]]
name = \"Prod\"
device = \"pixel\"  # the test phone
mode = \"release\"
";
        std::fs::write(fdemon_dir.join("launch.toml"), original).unwrap();

        // Load (editor's starting point), change ONE field of ONE config, save.
        let mut configs: Vec<LaunchConfig> = load_launch_configs(temp.path())
            .into_iter()
            .map(|r| r.config)
            .collect();
        configs[0].device = "emulator-5554".to_string();
        save_launch_configs(temp.path(), &configs).unwrap();

        let after = std::fs::read_to_string(fdemon_dir.join("launch.toml")).unwrap();
        assert!(
            after.contains("device = \"emulator-5554\""),
            "edit applied: {after}"
        );
        assert!(after.contains("# my launch configs"), "header comment kept");
        assert!(
            after.contains("# everyday driver"),
            "edited block's comment kept"
        );
        assert!(
            after.contains("# the test phone"),
            "other block's comment kept"
        );
        assert!(after.contains("name = \"Prod\""), "other block preserved");
        // The Prod block was untouched, so its release mode must be unchanged.
        assert!(after.contains("mode = \"release\""));
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
    fn test_load_launch_configs_invalid_toml() {
        let temp = tempdir().unwrap();
        let fdemon_dir = temp.path().join(".fdemon");
        std::fs::create_dir_all(&fdemon_dir).unwrap();

        std::fs::write(fdemon_dir.join("launch.toml"), "not valid {{{{").unwrap();

        let configs = load_launch_configs(temp.path());
        assert!(configs.is_empty());
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
    fn test_find_config_by_name() {
        let configs = vec![
            ResolvedLaunchConfig {
                config: LaunchConfig {
                    name: "Development".to_string(),
                    ..Default::default()
                },
                source: ConfigSource::FDemon,
            },
            ResolvedLaunchConfig {
                config: LaunchConfig {
                    name: "Production".to_string(),
                    ..Default::default()
                },
                source: ConfigSource::FDemon,
            },
        ];

        // Exact match
        let found = find_config_by_name(&configs, "Development");
        assert!(found.is_some());
        assert_eq!(found.unwrap().config.name, "Development");

        // Case insensitive
        let found = find_config_by_name(&configs, "development");
        assert!(found.is_some());
        assert_eq!(found.unwrap().config.name, "Development");

        // Not found
        let found = find_config_by_name(&configs, "Staging");
        assert!(found.is_none());
    }

    #[test]
    fn test_build_flutter_args_basic() {
        let config = LaunchConfig {
            name: "Test".to_string(),
            device: "auto".to_string(),
            mode: FlutterMode::Debug,
            ..Default::default()
        };

        let args = config.build_flutter_args("iphone-123");

        assert_eq!(args[0], "run");
        assert_eq!(args[1], "--machine");
        assert_eq!(args[2], "-d");
        assert_eq!(args[3], "iphone-123");
        assert_eq!(args[4], "--debug");
    }

    #[test]
    fn test_build_flutter_args_with_flavor() {
        let config = LaunchConfig {
            name: "Test".to_string(),
            flavor: Some("development".to_string()),
            ..Default::default()
        };

        let args = config.build_flutter_args("device-id");

        assert!(args.contains(&"--flavor".to_string()));
        assert!(args.contains(&"development".to_string()));
    }

    #[test]
    fn test_build_flutter_args_with_entry_point() {
        let config = LaunchConfig {
            name: "Test".to_string(),
            entry_point: Some("lib/main_dev.dart".into()),
            ..Default::default()
        };

        let args = config.build_flutter_args("device-id");

        assert!(args.contains(&"-t".to_string()));
        assert!(args.contains(&"lib/main_dev.dart".to_string()));
    }

    #[test]
    fn test_build_flutter_args_with_dart_defines() {
        let config = LaunchConfig {
            name: "Test".to_string(),
            dart_defines: [("API_URL".to_string(), "https://test.com".to_string())]
                .into_iter()
                .collect(),
            ..Default::default()
        };

        let args = config.build_flutter_args("device-id");

        assert!(args.contains(&"--dart-define".to_string()));
        assert!(args.contains(&"API_URL=https://test.com".to_string()));
    }

    #[test]
    fn test_build_flutter_args_with_extra_args() {
        let config = LaunchConfig {
            name: "Test".to_string(),
            extra_args: vec![
                "--verbose".to_string(),
                "--no-sound-null-safety".to_string(),
            ],
            ..Default::default()
        };

        let args = config.build_flutter_args("device-id");

        assert!(args.contains(&"--verbose".to_string()));
        assert!(args.contains(&"--no-sound-null-safety".to_string()));
    }

    #[test]
    fn test_build_flutter_args_full() {
        let config = LaunchConfig {
            name: "Full Config".to_string(),
            device: "iphone".to_string(),
            mode: FlutterMode::Release,
            flavor: Some("production".to_string()),
            entry_point: Some("lib/main_prod.dart".into()),
            dart_defines: [
                ("API_URL".to_string(), "https://prod.com".to_string()),
                ("DEBUG".to_string(), "false".to_string()),
            ]
            .into_iter()
            .collect(),
            extra_args: vec!["--obfuscate".to_string()],
            auto_start: false,
        };

        let args = config.build_flutter_args("iphone-15");

        assert!(args.contains(&"run".to_string()));
        assert!(args.contains(&"--machine".to_string()));
        assert!(args.contains(&"-d".to_string()));
        assert!(args.contains(&"iphone-15".to_string()));
        assert!(args.contains(&"--release".to_string()));
        assert!(args.contains(&"-t".to_string()));
        assert!(args.contains(&"lib/main_prod.dart".to_string()));
        assert!(args.contains(&"--flavor".to_string()));
        assert!(args.contains(&"production".to_string()));
        assert!(args.contains(&"--dart-define".to_string()));
        assert!(args.contains(&"--obfuscate".to_string()));
    }

    #[test]
    fn test_init_launch_file() {
        let temp = tempdir().unwrap();

        init_launch_file(temp.path()).unwrap();

        assert!(temp.path().join(".fdemon").exists());
        assert!(temp.path().join(".fdemon/launch.toml").exists());

        // Content should be valid TOML
        let content = std::fs::read_to_string(temp.path().join(".fdemon/launch.toml")).unwrap();
        let launch_file: LaunchFile = toml::from_str(&content).expect("Default should be valid");
        assert_eq!(launch_file.configurations.len(), 1);
        assert_eq!(launch_file.configurations[0].name, "Debug");
    }

    #[test]
    fn test_init_launch_file_idempotent() {
        let temp = tempdir().unwrap();

        // First init
        init_launch_file(temp.path()).unwrap();

        // Modify the file
        let launch_path = temp.path().join(".fdemon/launch.toml");
        let custom_content = r#"
[[configurations]]
name = "Custom"
device = "android"
mode = "profile"
"#;
        std::fs::write(&launch_path, custom_content).unwrap();

        // Second init should not overwrite
        init_launch_file(temp.path()).unwrap();

        let content = std::fs::read_to_string(&launch_path).unwrap();
        assert!(content.contains("Custom"));
    }

    #[test]
    fn test_default_launch_references_fdemon_dev_docs() {
        let temp = tempdir().unwrap();
        init_launch_file(temp.path()).unwrap();
        let content = std::fs::read_to_string(temp.path().join(".fdemon/launch.toml")).unwrap();
        assert!(
            content.contains("https://fdemon.dev/docs/configuration"),
            "launch.toml header must point at fdemon.dev docs"
        );
        assert!(
            !content.contains("github.com/example"),
            "launch.toml must not carry the placeholder URL"
        );
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
        assert_eq!(
            config.dart_defines.get("API_URL"),
            Some(&"https://example.com".to_string())
        );
        assert_eq!(config.dart_defines.get("DEBUG"), Some(&"true".to_string()));
        assert_eq!(config.dart_defines.get("EMPTY"), Some(&"".to_string()));
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Launch Config Editing Tests (Task 07)
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_create_default_launch_config() {
        let config = create_default_launch_config();

        assert_eq!(config.name, "New Configuration");
        assert_eq!(config.device, "auto");
        assert_eq!(config.mode, FlutterMode::Debug);
        assert!(!config.auto_start);
        assert!(config.dart_defines.is_empty());
        assert!(config.extra_args.is_empty());
    }

    #[test]
    fn test_add_launch_config() {
        let temp = tempdir().unwrap();

        // Add first config
        let config1 = LaunchConfig {
            name: "Debug".to_string(),
            ..Default::default()
        };
        add_launch_config(temp.path(), config1).unwrap();

        // Verify
        let loaded = load_launch_configs(temp.path());
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].config.name, "Debug");
    }

    #[test]
    fn test_add_launch_config_unique_name() {
        let temp = tempdir().unwrap();

        // Add first config
        let config1 = LaunchConfig {
            name: "Debug".to_string(),
            ..Default::default()
        };
        add_launch_config(temp.path(), config1).unwrap();

        // Add second with same name
        let config2 = LaunchConfig {
            name: "Debug".to_string(),
            ..Default::default()
        };
        add_launch_config(temp.path(), config2).unwrap();

        // Should have unique names
        let loaded = load_launch_configs(temp.path());
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].config.name, "Debug");
        assert_eq!(loaded[1].config.name, "Debug (1)");
    }

    #[test]
    fn test_add_launch_config_unique_name_multiple() {
        let temp = tempdir().unwrap();

        // Add three configs with same base name
        for _ in 0..3 {
            let config = LaunchConfig {
                name: "Test".to_string(),
                ..Default::default()
            };
            add_launch_config(temp.path(), config).unwrap();
        }

        let loaded = load_launch_configs(temp.path());
        assert_eq!(loaded.len(), 3);
        assert_eq!(loaded[0].config.name, "Test");
        assert_eq!(loaded[1].config.name, "Test (1)");
        assert_eq!(loaded[2].config.name, "Test (2)");
    }

    #[test]
    fn test_delete_launch_config() {
        let temp = tempdir().unwrap();

        // Add two configs
        save_launch_configs(
            temp.path(),
            &[
                LaunchConfig {
                    name: "Debug".to_string(),
                    ..Default::default()
                },
                LaunchConfig {
                    name: "Release".to_string(),
                    ..Default::default()
                },
            ],
        )
        .unwrap();

        // Delete one
        delete_launch_config(temp.path(), "Debug").unwrap();

        // Verify
        let loaded = load_launch_configs(temp.path());
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].config.name, "Release");
    }

    #[test]
    fn test_delete_launch_config_not_found() {
        let temp = tempdir().unwrap();

        save_launch_configs(
            temp.path(),
            &[LaunchConfig {
                name: "Debug".to_string(),
                ..Default::default()
            }],
        )
        .unwrap();

        // Try to delete non-existent config
        let result = delete_launch_config(temp.path(), "NonExistent");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Config 'NonExistent' not found"));
    }

    #[test]
    fn test_update_launch_config_field_name() {
        let temp = tempdir().unwrap();

        save_launch_configs(
            temp.path(),
            &[LaunchConfig {
                name: "Debug".to_string(),
                ..Default::default()
            }],
        )
        .unwrap();

        // Update name
        update_launch_config_field(temp.path(), "Debug", "name", "Production").unwrap();

        // Verify
        let loaded = load_launch_configs(temp.path());
        assert_eq!(loaded[0].config.name, "Production");
    }

    #[test]
    fn test_update_launch_config_field_device() {
        let temp = tempdir().unwrap();

        save_launch_configs(
            temp.path(),
            &[LaunchConfig {
                name: "Debug".to_string(),
                ..Default::default()
            }],
        )
        .unwrap();

        // Update device
        update_launch_config_field(temp.path(), "Debug", "device", "iphone-15").unwrap();

        // Verify
        let loaded = load_launch_configs(temp.path());
        assert_eq!(loaded[0].config.device, "iphone-15");
    }

    #[test]
    fn test_update_launch_config_field_mode() {
        let temp = tempdir().unwrap();

        save_launch_configs(
            temp.path(),
            &[LaunchConfig {
                name: "Debug".to_string(),
                ..Default::default()
            }],
        )
        .unwrap();

        // Update mode
        update_launch_config_field(temp.path(), "Debug", "mode", "release").unwrap();

        // Verify
        let loaded = load_launch_configs(temp.path());
        assert_eq!(loaded[0].config.mode, FlutterMode::Release);
    }

    #[test]
    fn test_update_launch_config_field_mode_case_insensitive() {
        let temp = tempdir().unwrap();

        save_launch_configs(
            temp.path(),
            &[LaunchConfig {
                name: "Test".to_string(),
                ..Default::default()
            }],
        )
        .unwrap();

        // Update with different case
        update_launch_config_field(temp.path(), "Test", "mode", "PROFILE").unwrap();

        let loaded = load_launch_configs(temp.path());
        assert_eq!(loaded[0].config.mode, FlutterMode::Profile);
    }

    #[test]
    fn test_update_launch_config_field_flavor() {
        let temp = tempdir().unwrap();

        save_launch_configs(
            temp.path(),
            &[LaunchConfig {
                name: "Debug".to_string(),
                ..Default::default()
            }],
        )
        .unwrap();

        // Set flavor
        update_launch_config_field(temp.path(), "Debug", "flavor", "development").unwrap();

        let loaded = load_launch_configs(temp.path());
        assert_eq!(loaded[0].config.flavor, Some("development".to_string()));

        // Clear flavor (empty string)
        update_launch_config_field(temp.path(), "Debug", "flavor", "").unwrap();

        let loaded = load_launch_configs(temp.path());
        assert_eq!(loaded[0].config.flavor, None);
    }

    #[test]
    fn test_update_launch_config_field_auto_start() {
        let temp = tempdir().unwrap();

        save_launch_configs(
            temp.path(),
            &[LaunchConfig {
                name: "Debug".to_string(),
                auto_start: false,
                ..Default::default()
            }],
        )
        .unwrap();

        // Enable auto_start
        update_launch_config_field(temp.path(), "Debug", "auto_start", "true").unwrap();

        let loaded = load_launch_configs(temp.path());
        assert!(loaded[0].config.auto_start);

        // Disable auto_start
        update_launch_config_field(temp.path(), "Debug", "auto_start", "false").unwrap();

        let loaded = load_launch_configs(temp.path());
        assert!(!loaded[0].config.auto_start);
    }

    #[test]
    fn test_update_launch_config_field_invalid_mode() {
        let temp = tempdir().unwrap();

        save_launch_configs(
            temp.path(),
            &[LaunchConfig {
                name: "Debug".to_string(),
                ..Default::default()
            }],
        )
        .unwrap();

        // Try invalid mode
        let result = update_launch_config_field(temp.path(), "Debug", "mode", "invalid");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid mode"));
    }

    #[test]
    fn test_update_launch_config_field_unknown_field() {
        let temp = tempdir().unwrap();

        save_launch_configs(
            temp.path(),
            &[LaunchConfig {
                name: "Debug".to_string(),
                ..Default::default()
            }],
        )
        .unwrap();

        // Try unknown field
        let result = update_launch_config_field(temp.path(), "Debug", "unknown", "value");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unknown field"));
    }

    #[test]
    fn test_update_launch_config_field_not_found() {
        let temp = tempdir().unwrap();

        save_launch_configs(
            temp.path(),
            &[LaunchConfig {
                name: "Debug".to_string(),
                ..Default::default()
            }],
        )
        .unwrap();

        // Try to update non-existent config
        let result = update_launch_config_field(temp.path(), "NonExistent", "name", "value");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Config 'NonExistent' not found"));
    }

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

    // ─────────────────────────────────────────────────────────────────────────
    // Launch Config Save Tests (Task 11)
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_save_launch_configs_roundtrip() {
        let temp = tempdir().unwrap();

        let configs = vec![
            LaunchConfig {
                name: "Test".to_string(),
                device: "auto".to_string(),
                mode: FlutterMode::Debug,
                ..Default::default()
            },
            LaunchConfig {
                name: "Release".to_string(),
                device: "ios".to_string(),
                mode: FlutterMode::Release,
                flavor: Some("production".to_string()),
                ..Default::default()
            },
        ];

        save_launch_configs(temp.path(), &configs).unwrap();

        let loaded = load_launch_configs(temp.path());
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].config.name, "Test");
        assert_eq!(loaded[0].config.device, "auto");
        assert_eq!(loaded[0].config.mode, FlutterMode::Debug);
        assert_eq!(loaded[1].config.name, "Release");
        assert_eq!(loaded[1].config.mode, FlutterMode::Release);
        assert_eq!(loaded[1].config.flavor, Some("production".to_string()));
    }

    #[test]
    fn test_save_launch_configs_creates_directory() {
        let temp = tempdir().unwrap();
        // Don't create .fdemon directory

        let configs = vec![LaunchConfig::default()];
        save_launch_configs(temp.path(), &configs).unwrap();

        assert!(temp.path().join(".fdemon/launch.toml").exists());
    }

    #[test]
    fn test_save_launch_configs_atomic_write() {
        let temp = tempdir().unwrap();
        std::fs::create_dir_all(temp.path().join(".fdemon")).unwrap();

        let configs = vec![LaunchConfig::default()];
        save_launch_configs(temp.path(), &configs).unwrap();

        // Verify no temp file left behind
        assert!(!temp.path().join(".fdemon/.launch.toml.tmp").exists());
    }

    #[test]
    fn test_saved_launch_configs_file_has_header() {
        let temp = tempdir().unwrap();
        let configs = vec![LaunchConfig::default()];

        save_launch_configs(temp.path(), &configs).unwrap();

        let content = std::fs::read_to_string(temp.path().join(".fdemon/launch.toml")).unwrap();

        assert!(content.contains("Flutter Demon Launch Configurations"));
        assert!(content.contains("Generated by fdemon settings panel"));
        assert!(content.starts_with('#'));
    }

    #[test]
    fn test_save_launch_configs_empty_list() {
        let temp = tempdir().unwrap();

        // Save empty list
        save_launch_configs(temp.path(), &[]).unwrap();

        let loaded = load_launch_configs(temp.path());
        assert_eq!(loaded.len(), 0);
    }

    #[test]
    fn test_save_launch_configs_with_dart_defines() {
        let temp = tempdir().unwrap();

        let mut dart_defines = std::collections::HashMap::new();
        dart_defines.insert("API_URL".to_string(), "https://test.com".to_string());
        dart_defines.insert("DEBUG".to_string(), "true".to_string());

        let configs = vec![LaunchConfig {
            name: "Dev".to_string(),
            device: "iphone".to_string(),
            mode: FlutterMode::Debug,
            dart_defines,
            ..Default::default()
        }];

        save_launch_configs(temp.path(), &configs).unwrap();

        let loaded = load_launch_configs(temp.path());
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].config.dart_defines.len(), 2);
        assert_eq!(
            loaded[0].config.dart_defines.get("API_URL"),
            Some(&"https://test.com".to_string())
        );
        assert_eq!(
            loaded[0].config.dart_defines.get("DEBUG"),
            Some(&"true".to_string())
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Dart Defines Parsing Tests (Task 10c)
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_parse_dart_defines() {
        let result = parse_dart_defines("API_URL=https://dev.com,DEBUG=true");
        assert_eq!(result.len(), 2);
        assert_eq!(result.get("API_URL"), Some(&"https://dev.com".to_string()));
        assert_eq!(result.get("DEBUG"), Some(&"true".to_string()));
    }

    #[test]
    fn test_parse_dart_defines_empty() {
        let result = parse_dart_defines("");
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_dart_defines_whitespace() {
        let result = parse_dart_defines("  ");
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_dart_defines_single_pair() {
        let result = parse_dart_defines("KEY=value");
        assert_eq!(result.len(), 1);
        assert_eq!(result.get("KEY"), Some(&"value".to_string()));
    }

    #[test]
    fn test_parse_dart_defines_with_spaces() {
        let result = parse_dart_defines(" KEY1 = value1 , KEY2 = value2 ");
        assert_eq!(result.len(), 2);
        assert_eq!(result.get("KEY1"), Some(&"value1".to_string()));
        assert_eq!(result.get("KEY2"), Some(&"value2".to_string()));
    }

    #[test]
    fn test_parse_dart_defines_empty_value() {
        let result = parse_dart_defines("KEY=");
        assert_eq!(result.len(), 1);
        assert_eq!(result.get("KEY"), Some(&"".to_string()));
    }

    #[test]
    fn test_parse_dart_defines_no_equals() {
        let result = parse_dart_defines("INVALID");
        // Should skip invalid pairs
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_parse_dart_defines_mixed_valid_invalid() {
        let result = parse_dart_defines("KEY1=value1,INVALID,KEY2=value2");
        assert_eq!(result.len(), 2);
        assert_eq!(result.get("KEY1"), Some(&"value1".to_string()));
        assert_eq!(result.get("KEY2"), Some(&"value2".to_string()));
    }

    #[test]
    fn test_parse_dart_defines_with_equals_in_value() {
        let result = parse_dart_defines("URL=https://api.com?key=value");
        assert_eq!(result.len(), 1);
        assert_eq!(
            result.get("URL"),
            Some(&"https://api.com?key=value".to_string())
        );
    }

    #[test]
    fn test_update_launch_config_dart_defines() {
        let temp = tempdir().unwrap();

        save_launch_configs(
            temp.path(),
            &[LaunchConfig {
                name: "Dev".to_string(),
                ..Default::default()
            }],
        )
        .unwrap();

        // Update dart_defines
        update_launch_config_dart_defines(
            temp.path(),
            "Dev",
            "API_URL=https://test.com,DEBUG=true",
        )
        .unwrap();

        let loaded = load_launch_configs(temp.path());
        assert_eq!(loaded[0].config.dart_defines.len(), 2);
        assert_eq!(
            loaded[0].config.dart_defines.get("API_URL"),
            Some(&"https://test.com".to_string())
        );
        assert_eq!(
            loaded[0].config.dart_defines.get("DEBUG"),
            Some(&"true".to_string())
        );
    }

    #[test]
    fn test_update_launch_config_dart_defines_empty() {
        let temp = tempdir().unwrap();

        let mut dart_defines = std::collections::HashMap::new();
        dart_defines.insert("KEY".to_string(), "value".to_string());

        save_launch_configs(
            temp.path(),
            &[LaunchConfig {
                name: "Dev".to_string(),
                dart_defines,
                ..Default::default()
            }],
        )
        .unwrap();

        // Clear dart_defines with empty string
        update_launch_config_dart_defines(temp.path(), "Dev", "").unwrap();

        let loaded = load_launch_configs(temp.path());
        assert!(loaded[0].config.dart_defines.is_empty());
    }

    #[test]
    fn test_update_launch_config_dart_defines_not_found() {
        let temp = tempdir().unwrap();

        save_launch_configs(
            temp.path(),
            &[LaunchConfig {
                name: "Dev".to_string(),
                ..Default::default()
            }],
        )
        .unwrap();

        let result = update_launch_config_dart_defines(temp.path(), "NonExistent", "KEY=value");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Config 'NonExistent' not found"));
    }

    // ─────────────────────────────────────────────────────────────────────────
    // extra_args validation tests (Task 02-validate-extra-args)
    // ─────────────────────────────────────────────────────────────────────────

    /// Well-formed flags must pass through build_flutter_args unchanged.
    #[test]
    fn test_extra_args_valid_flags_pass_through() {
        let config = LaunchConfig {
            name: "Test".to_string(),
            extra_args: vec![
                "--obfuscate".to_string(),
                "--split-debug-info=build/symbols".to_string(),
                "-t".to_string(),
                "lib/main.dart".to_string(), // value token following the -t flag (split syntax)
            ],
            ..Default::default()
        };

        let args = config.build_flutter_args("device-id");

        // Valid dash-prefixed flags must be present.
        assert!(
            args.contains(&"--obfuscate".to_string()),
            "--obfuscate should pass through"
        );
        assert!(
            args.contains(&"--split-debug-info=build/symbols".to_string()),
            "--split-debug-info should pass through"
        );
        assert!(args.contains(&"-t".to_string()), "-t should pass through");

        // The value token following a split-syntax flag must be preserved.
        assert!(
            args.contains(&"lib/main.dart".to_string()),
            "value following -t must be preserved (split flag/value syntax)"
        );
    }

    /// A split flag/value pair (e.g. VS Code `toolArgs` `["--web-port", "8080"]`)
    /// must keep the value token even though it does not start with '-'.
    #[test]
    fn test_extra_args_split_flag_value_preserved() {
        let config = LaunchConfig {
            name: "Test".to_string(),
            extra_args: vec!["--web-port".to_string(), "8080".to_string()],
            ..Default::default()
        };

        let args = config.build_flutter_args("device-id");

        // Both tokens present, in order, immediately adjacent.
        let pos = args
            .windows(2)
            .position(|w| w[0] == "--web-port" && w[1] == "8080");
        assert!(
            pos.is_some(),
            "--web-port 8080 must be preserved as an adjacent flag/value pair, got {args:?}"
        );
    }

    /// A flag with an inline `=value` does not consume the following token; a
    /// subsequent bare positional with no preceding flag is still dropped.
    #[test]
    fn test_extra_args_inline_value_does_not_consume_next() {
        let config = LaunchConfig {
            name: "Test".to_string(),
            extra_args: vec![
                "--split-debug-info=build/symbols".to_string(),
                "stray".to_string(), // bare positional, no preceding split flag
            ],
            ..Default::default()
        };

        let args = config.build_flutter_args("device-id");

        assert!(args.contains(&"--split-debug-info=build/symbols".to_string()));
        assert!(
            !args.contains(&"stray".to_string()),
            "bare positional after an inline-value flag must be dropped"
        );
    }

    /// A value token is accepted after a flag even if the value itself would fail
    /// the leading-dash rule, but it must still pass the NUL/length checks.
    #[test]
    fn test_extra_args_value_token_still_length_checked() {
        let long_value = "x".repeat(MAX_EXTRA_ARG_LEN + 1);
        let config = LaunchConfig {
            name: "Test".to_string(),
            extra_args: vec!["--web-port".to_string(), long_value.clone()],
            ..Default::default()
        };

        let args = config.build_flutter_args("device-id");

        assert!(args.contains(&"--web-port".to_string()));
        assert!(
            !args.contains(&long_value),
            "over-length value token must still be dropped"
        );
    }

    /// An extra_arg containing a NUL byte must be dropped.
    #[test]
    fn test_extra_args_nul_byte_dropped() {
        let config = LaunchConfig {
            name: "Test".to_string(),
            extra_args: vec!["--flag\0injected".to_string()],
            ..Default::default()
        };

        let args = config.build_flutter_args("device-id");

        assert!(
            !args.iter().any(|a| a.contains('\0')),
            "NUL-containing arg must not reach the command"
        );
    }

    /// An extra_arg that exceeds MAX_EXTRA_ARG_LEN must be dropped.
    #[test]
    fn test_extra_args_over_length_dropped() {
        let long_arg = format!("--{}", "x".repeat(MAX_EXTRA_ARG_LEN));
        assert!(
            long_arg.len() > MAX_EXTRA_ARG_LEN,
            "precondition: arg is over limit"
        );

        let config = LaunchConfig {
            name: "Test".to_string(),
            extra_args: vec![long_arg.clone()],
            ..Default::default()
        };

        let args = config.build_flutter_args("device-id");

        assert!(!args.contains(&long_arg), "over-length arg must be dropped");
    }

    /// A free-text extra_arg not starting with '-' must be dropped.
    #[test]
    fn test_extra_args_non_dash_free_text_dropped() {
        let config = LaunchConfig {
            name: "Test".to_string(),
            extra_args: vec!["some_free_text_value".to_string()],
            ..Default::default()
        };

        let args = config.build_flutter_args("device-id");

        assert!(
            !args.contains(&"some_free_text_value".to_string()),
            "non-dash free-text arg must be dropped"
        );
    }
}
