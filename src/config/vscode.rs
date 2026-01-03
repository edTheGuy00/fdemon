//! VSCode launch.json parser for Dart/Flutter compatibility

use super::types::{ConfigSource, FlutterMode, LaunchConfig, ResolvedLaunchConfig};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use tracing::{debug, warn};

const VSCODE_DIR: &str = ".vscode";
const LAUNCH_FILENAME: &str = "launch.json";

/// VSCode launch.json file structure
#[derive(Debug, Deserialize)]
struct VSCodeLaunchFile {
    #[serde(default)]
    #[allow(dead_code)]
    version: Option<String>,

    #[serde(default)]
    configurations: Vec<VSCodeConfiguration>,
}

/// A single VSCode launch configuration
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VSCodeConfiguration {
    /// Display name
    name: String,

    /// Type (must be "dart" for Flutter/Dart)
    #[serde(rename = "type")]
    config_type: String,

    /// Request type: "launch" or "attach"
    #[allow(dead_code)]
    request: String,

    /// Entry point program
    #[serde(default)]
    program: Option<String>,

    /// Target device ID
    #[serde(default)]
    device_id: Option<String>,

    /// Flutter mode: debug, profile, release
    #[serde(default)]
    flutter_mode: Option<String>,

    /// Additional tool arguments (passed to flutter run)
    #[serde(default)]
    tool_args: Vec<String>,

    /// Arguments passed to the app's main()
    #[serde(default)]
    #[allow(dead_code)]
    args: Vec<String>,

    /// Working directory
    #[serde(default)]
    #[allow(dead_code)]
    cwd: Option<String>,

    /// Environment variables
    #[serde(default)]
    #[allow(dead_code)]
    env: HashMap<String, String>,
}

/// Load launch configurations from .vscode/launch.json
///
/// Only Dart/Flutter configurations (type = "dart") are imported.
/// Returns empty vec if file doesn't exist or can't be parsed.
pub fn load_vscode_configs(project_path: &Path) -> Vec<ResolvedLaunchConfig> {
    let launch_path = project_path.join(VSCODE_DIR).join(LAUNCH_FILENAME);

    if !launch_path.exists() {
        debug!("No VSCode launch.json at {:?}", launch_path);
        return Vec::new();
    }

    match std::fs::read_to_string(&launch_path) {
        Ok(content) => parse_launch_json(&content, &launch_path),
        Err(e) => {
            warn!("Failed to read {:?}: {}", launch_path, e);
            Vec::new()
        }
    }
}

/// Parse the launch.json content
fn parse_launch_json(content: &str, path: &Path) -> Vec<ResolvedLaunchConfig> {
    // VSCode allows comments in JSON (JSONC), so we need to strip them
    let cleaned = strip_json_comments(content);

    match serde_json::from_str::<VSCodeLaunchFile>(&cleaned) {
        Ok(launch_file) => {
            let configs: Vec<_> = launch_file
                .configurations
                .into_iter()
                .filter(is_dart_config)
                .filter_map(convert_vscode_config)
                .collect();

            debug!(
                "Loaded {} Dart/Flutter configurations from {:?}",
                configs.len(),
                path
            );
            configs
        }
        Err(e) => {
            warn!("Failed to parse {:?}: {}", path, e);
            Vec::new()
        }
    }
}

/// Check if this is a Dart/Flutter configuration
fn is_dart_config(config: &VSCodeConfiguration) -> bool {
    config.config_type.to_lowercase() == "dart"
}

/// Convert VSCode configuration to internal LaunchConfig
fn convert_vscode_config(vscode: VSCodeConfiguration) -> Option<ResolvedLaunchConfig> {
    // Parse flutter mode
    let mode = vscode
        .flutter_mode
        .as_deref()
        .map(parse_flutter_mode)
        .unwrap_or(FlutterMode::Debug);

    // Extract dart-defines and flavor from toolArgs
    let (dart_defines, flavor, extra_args) = parse_tool_args(&vscode.tool_args);

    // Build entry point from program field
    let entry_point = vscode
        .program
        .filter(|p| !p.is_empty() && p != "lib/main.dart")
        .map(std::path::PathBuf::from);

    // Determine device from deviceId
    let device = vscode
        .device_id
        .filter(|d| !d.is_empty())
        .unwrap_or_else(|| "auto".to_string());

    let config = LaunchConfig {
        name: vscode.name,
        device,
        mode,
        flavor,
        entry_point,
        dart_defines,
        extra_args,
        auto_start: false, // VSCode imports never auto-start
    };

    Some(ResolvedLaunchConfig {
        config,
        source: ConfigSource::VSCode,
    })
}

/// Parse flutter mode string
fn parse_flutter_mode(mode: &str) -> FlutterMode {
    match mode.to_lowercase().as_str() {
        "profile" => FlutterMode::Profile,
        "release" => FlutterMode::Release,
        _ => FlutterMode::Debug,
    }
}

/// Parse toolArgs to extract dart-defines, flavor, and remaining args
fn parse_tool_args(
    args: &[String],
) -> (
    HashMap<String, String>, // dart_defines
    Option<String>,          // flavor
    Vec<String>,             // extra_args
) {
    let mut dart_defines = HashMap::new();
    let mut flavor = None;
    let mut extra_args = Vec::new();

    let mut iter = args.iter().peekable();

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--dart-define" => {
                if let Some(value) = iter.next() {
                    if let Some((key, val)) = value.split_once('=') {
                        dart_defines.insert(key.to_string(), val.to_string());
                    }
                }
            }
            "--flavor" => {
                if let Some(value) = iter.next() {
                    flavor = Some(value.clone());
                }
            }
            _ if arg.starts_with("--dart-define=") => {
                if let Some(rest) = arg.strip_prefix("--dart-define=") {
                    if let Some((key, val)) = rest.split_once('=') {
                        dart_defines.insert(key.to_string(), val.to_string());
                    }
                }
            }
            _ if arg.starts_with("--flavor=") => {
                if let Some(value) = arg.strip_prefix("--flavor=") {
                    flavor = Some(value.to_string());
                }
            }
            _ => {
                extra_args.push(arg.clone());
            }
        }
    }

    (dart_defines, flavor, extra_args)
}

/// Strip comments from JSON (JSONC support)
///
/// VSCode uses JSONC which allows // and /* */ comments
fn strip_json_comments(content: &str) -> String {
    let mut result = String::with_capacity(content.len());
    let mut chars = content.chars().peekable();
    let mut in_string = false;
    let mut escape_next = false;

    while let Some(c) = chars.next() {
        if escape_next {
            result.push(c);
            escape_next = false;
            continue;
        }

        if c == '\\' && in_string {
            result.push(c);
            escape_next = true;
            continue;
        }

        if c == '"' && !escape_next {
            in_string = !in_string;
            result.push(c);
            continue;
        }

        if !in_string && c == '/' {
            if let Some(&next) = chars.peek() {
                if next == '/' {
                    // Line comment - skip until newline
                    chars.next(); // consume second /
                    while let Some(&nc) = chars.peek() {
                        if nc == '\n' {
                            break;
                        }
                        chars.next();
                    }
                    continue;
                } else if next == '*' {
                    // Block comment - skip until */
                    chars.next(); // consume *
                    while let Some(nc) = chars.next() {
                        if nc == '*' {
                            if let Some(&'/') = chars.peek() {
                                chars.next(); // consume /
                                break;
                            }
                        }
                    }
                    continue;
                }
            }
        }

        result.push(c);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_load_vscode_configs_no_file() {
        let temp = tempdir().unwrap();
        let configs = load_vscode_configs(temp.path());
        assert!(configs.is_empty());
    }

    #[test]
    fn test_load_vscode_configs_basic() {
        let temp = tempdir().unwrap();
        let vscode_dir = temp.path().join(".vscode");
        std::fs::create_dir_all(&vscode_dir).unwrap();

        let content = r#"{
            "version": "0.2.0",
            "configurations": [
                {
                    "name": "Flutter Debug",
                    "type": "dart",
                    "request": "launch",
                    "flutterMode": "debug"
                }
            ]
        }"#;
        std::fs::write(vscode_dir.join("launch.json"), content).unwrap();

        let configs = load_vscode_configs(temp.path());

        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].config.name, "Flutter Debug");
        assert_eq!(configs[0].config.mode, FlutterMode::Debug);
        assert_eq!(configs[0].source, ConfigSource::VSCode);
    }

    #[test]
    fn test_load_vscode_configs_with_tool_args() {
        let temp = tempdir().unwrap();
        let vscode_dir = temp.path().join(".vscode");
        std::fs::create_dir_all(&vscode_dir).unwrap();

        let content = r#"{
            "configurations": [
                {
                    "name": "Dev",
                    "type": "dart",
                    "request": "launch",
                    "flutterMode": "debug",
                    "deviceId": "iphone",
                    "toolArgs": [
                        "--dart-define", "API_URL=https://dev.api.com",
                        "--flavor", "development",
                        "--verbose"
                    ]
                }
            ]
        }"#;
        std::fs::write(vscode_dir.join("launch.json"), content).unwrap();

        let configs = load_vscode_configs(temp.path());

        assert_eq!(configs.len(), 1);
        let config = &configs[0].config;

        assert_eq!(config.device, "iphone");
        assert_eq!(config.flavor, Some("development".to_string()));
        assert_eq!(
            config.dart_defines.get("API_URL"),
            Some(&"https://dev.api.com".to_string())
        );
        assert!(config.extra_args.contains(&"--verbose".to_string()));
    }

    #[test]
    fn test_load_vscode_filters_non_dart() {
        let temp = tempdir().unwrap();
        let vscode_dir = temp.path().join(".vscode");
        std::fs::create_dir_all(&vscode_dir).unwrap();

        let content = r#"{
            "configurations": [
                {
                    "name": "Node.js",
                    "type": "node",
                    "request": "launch"
                },
                {
                    "name": "Flutter",
                    "type": "dart",
                    "request": "launch"
                }
            ]
        }"#;
        std::fs::write(vscode_dir.join("launch.json"), content).unwrap();

        let configs = load_vscode_configs(temp.path());

        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].config.name, "Flutter");
    }

    #[test]
    fn test_strip_json_comments_line_comment() {
        let input = r#"{
            // This is a comment
            "key": "value"
        }"#;

        let result = strip_json_comments(input);
        assert!(!result.contains("This is a comment"));
        assert!(result.contains("\"key\": \"value\""));
    }

    #[test]
    fn test_strip_json_comments_block_comment() {
        let input = r#"{
            /* Block comment */
            "key": "value"
        }"#;

        let result = strip_json_comments(input);
        assert!(!result.contains("Block comment"));
        assert!(result.contains("\"key\": \"value\""));
    }

    #[test]
    fn test_strip_json_comments_preserves_strings() {
        let input = r#"{"url": "http://example.com"}"#;

        let result = strip_json_comments(input);
        assert_eq!(result, input);
    }

    #[test]
    fn test_strip_json_comments_preserves_slashes_in_strings() {
        let input = r#"{"path": "C:\\Users\\test", "url": "https://example.com/path"}"#;

        let result = strip_json_comments(input);
        assert_eq!(result, input);
    }

    #[test]
    fn test_parse_tool_args() {
        let args = vec![
            "--dart-define".to_string(),
            "KEY1=value1".to_string(),
            "--flavor".to_string(),
            "dev".to_string(),
            "--verbose".to_string(),
            "--dart-define=KEY2=value2".to_string(),
        ];

        let (defines, flavor, extra) = parse_tool_args(&args);

        assert_eq!(defines.get("KEY1"), Some(&"value1".to_string()));
        assert_eq!(defines.get("KEY2"), Some(&"value2".to_string()));
        assert_eq!(flavor, Some("dev".to_string()));
        assert!(extra.contains(&"--verbose".to_string()));
    }

    #[test]
    fn test_parse_tool_args_with_equals_flavor() {
        let args = vec!["--flavor=production".to_string()];

        let (_, flavor, _) = parse_tool_args(&args);

        assert_eq!(flavor, Some("production".to_string()));
    }

    #[test]
    fn test_parse_flutter_mode() {
        assert_eq!(parse_flutter_mode("debug"), FlutterMode::Debug);
        assert_eq!(parse_flutter_mode("Debug"), FlutterMode::Debug);
        assert_eq!(parse_flutter_mode("profile"), FlutterMode::Profile);
        assert_eq!(parse_flutter_mode("release"), FlutterMode::Release);
        assert_eq!(parse_flutter_mode("unknown"), FlutterMode::Debug);
    }

    #[test]
    fn test_vscode_config_with_program() {
        let temp = tempdir().unwrap();
        let vscode_dir = temp.path().join(".vscode");
        std::fs::create_dir_all(&vscode_dir).unwrap();

        let content = r#"{
            "configurations": [
                {
                    "name": "Custom Entry",
                    "type": "dart",
                    "request": "launch",
                    "program": "lib/custom_main.dart"
                }
            ]
        }"#;
        std::fs::write(vscode_dir.join("launch.json"), content).unwrap();

        let configs = load_vscode_configs(temp.path());

        assert_eq!(configs.len(), 1);
        assert_eq!(
            configs[0].config.entry_point,
            Some(std::path::PathBuf::from("lib/custom_main.dart"))
        );
    }

    #[test]
    fn test_vscode_config_default_program_ignored() {
        let temp = tempdir().unwrap();
        let vscode_dir = temp.path().join(".vscode");
        std::fs::create_dir_all(&vscode_dir).unwrap();

        let content = r#"{
            "configurations": [
                {
                    "name": "Default Entry",
                    "type": "dart",
                    "request": "launch",
                    "program": "lib/main.dart"
                }
            ]
        }"#;
        std::fs::write(vscode_dir.join("launch.json"), content).unwrap();

        let configs = load_vscode_configs(temp.path());

        assert_eq!(configs.len(), 1);
        // Default program should not be set as entry_point
        assert_eq!(configs[0].config.entry_point, None);
    }

    #[test]
    fn test_vscode_config_auto_start_always_false() {
        let temp = tempdir().unwrap();
        let vscode_dir = temp.path().join(".vscode");
        std::fs::create_dir_all(&vscode_dir).unwrap();

        let content = r#"{
            "configurations": [
                {
                    "name": "Test",
                    "type": "dart",
                    "request": "launch"
                }
            ]
        }"#;
        std::fs::write(vscode_dir.join("launch.json"), content).unwrap();

        let configs = load_vscode_configs(temp.path());

        assert_eq!(configs.len(), 1);
        assert!(!configs[0].config.auto_start);
    }

    #[test]
    fn test_vscode_config_empty_device_id_defaults_to_auto() {
        let temp = tempdir().unwrap();
        let vscode_dir = temp.path().join(".vscode");
        std::fs::create_dir_all(&vscode_dir).unwrap();

        let content = r#"{
            "configurations": [
                {
                    "name": "Test",
                    "type": "dart",
                    "request": "launch",
                    "deviceId": ""
                }
            ]
        }"#;
        std::fs::write(vscode_dir.join("launch.json"), content).unwrap();

        let configs = load_vscode_configs(temp.path());

        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].config.device, "auto");
    }

    #[test]
    fn test_vscode_invalid_json_returns_empty() {
        let temp = tempdir().unwrap();
        let vscode_dir = temp.path().join(".vscode");
        std::fs::create_dir_all(&vscode_dir).unwrap();

        std::fs::write(vscode_dir.join("launch.json"), "not valid json {{{{").unwrap();

        let configs = load_vscode_configs(temp.path());
        assert!(configs.is_empty());
    }

    #[test]
    fn test_vscode_import_integration() {
        let temp = tempdir().unwrap();
        let vscode_dir = temp.path().join(".vscode");
        std::fs::create_dir_all(&vscode_dir).unwrap();

        // Real-world example from a Flutter project
        let content = r#"{
            // Flutter launch configurations
            "version": "0.2.0",
            "configurations": [
                {
                    "name": "my_app (debug)",
                    "type": "dart",
                    "request": "launch",
                    "flutterMode": "debug",
                    "deviceId": "iphone",
                    "toolArgs": [
                        "--dart-define", "API_URL=https://dev.example.com",
                        "--dart-define", "SENTRY_DSN=",
                        "--flavor", "development"
                    ]
                },
                {
                    "name": "my_app (profile)",
                    "type": "dart",
                    "request": "launch",
                    "flutterMode": "profile"
                },
                {
                    /* Production build */
                    "name": "my_app (release)",
                    "type": "dart",
                    "request": "launch",
                    "flutterMode": "release",
                    "toolArgs": [
                        "--flavor", "production",
                        "--obfuscate",
                        "--split-debug-info=build/symbols"
                    ]
                }
            ]
        }"#;
        std::fs::write(vscode_dir.join("launch.json"), content).unwrap();

        let configs = load_vscode_configs(temp.path());

        assert_eq!(configs.len(), 3);

        // Debug config
        let debug = &configs[0].config;
        assert_eq!(debug.name, "my_app (debug)");
        assert_eq!(debug.mode, FlutterMode::Debug);
        assert_eq!(debug.device, "iphone");
        assert_eq!(debug.flavor, Some("development".to_string()));
        assert_eq!(debug.dart_defines.len(), 2);

        // Profile config
        let profile = &configs[1].config;
        assert_eq!(profile.mode, FlutterMode::Profile);
        assert_eq!(profile.device, "auto");

        // Release config
        let release = &configs[2].config;
        assert_eq!(release.mode, FlutterMode::Release);
        assert_eq!(release.flavor, Some("production".to_string()));
        assert!(release.extra_args.contains(&"--obfuscate".to_string()));
    }

    #[test]
    fn test_strip_json_comments_multiline_block() {
        let input = r#"{
            /*
             * Multi-line
             * block comment
             */
            "key": "value"
        }"#;

        let result = strip_json_comments(input);
        assert!(!result.contains("Multi-line"));
        assert!(!result.contains("block comment"));
        assert!(result.contains("\"key\": \"value\""));
    }

    #[test]
    fn test_strip_json_comments_comment_like_in_string() {
        let input = r#"{"comment": "This has // in it and /* too */"}"#;

        let result = strip_json_comments(input);
        // Should preserve the comment-like content inside the string
        assert!(result.contains("// in it"));
        assert!(result.contains("/* too */"));
    }
}
