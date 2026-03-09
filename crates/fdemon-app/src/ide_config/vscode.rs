//! VS Code DAP configuration generator.
//!
//! Generates or merges a `.vscode/launch.json` entry so that VS Code,
//! VS Code Insiders, and Cursor can attach to the fdemon DAP server via
//! the `debugServer` field (a VS Code-internal mechanism that redirects
//! the debug-adapter transport to an existing TCP server).

use std::path::{Path, PathBuf};

use fdemon_core::Result;
use serde_json::json;

use super::{
    merge::{clean_jsonc, merge_json_array_entry, to_pretty_json, FDEMON_CONFIG_NAME},
    IdeConfigGenerator,
};

/// Generates `.vscode/launch.json` DAP config for VS Code, VS Code Insiders, and Cursor.
///
/// Uses the `debugServer` field which tells VS Code to connect to an already-running
/// DAP server on the given port instead of spawning a debug adapter process.
/// The Dart extension must be installed (provides `"type": "dart"`).
pub struct VSCodeGenerator;

impl VSCodeGenerator {
    /// Build the fdemon launch configuration entry for a given port.
    fn fdemon_entry(port: u16) -> serde_json::Value {
        json!({
            "name": FDEMON_CONFIG_NAME,
            "type": "dart",
            "request": "attach",
            "debugServer": port,
            "cwd": "${workspaceFolder}",
            "fdemon-managed": true
        })
    }
}

impl IdeConfigGenerator for VSCodeGenerator {
    fn config_path(&self, project_root: &Path) -> PathBuf {
        project_root.join(".vscode").join("launch.json")
    }

    fn generate(&self, port: u16, _project_root: &Path) -> Result<String> {
        let config = json!({
            "version": "0.2.0",
            "configurations": [
                Self::fdemon_entry(port)
            ]
        });
        Ok(to_pretty_json(&config))
    }

    fn merge_config(&self, existing: &str, port: u16, _project_root: &Path) -> Result<String> {
        // Treat an empty/whitespace-only file as a fresh generation.
        if existing.trim().is_empty() {
            return self.generate(port, Path::new(""));
        }

        // Strip JSONC comments and trailing commas before parsing.
        let clean = clean_jsonc(existing);
        let mut root: serde_json::Value = serde_json::from_str(&clean)?;

        // Ensure `configurations` array exists.
        if root.get("configurations").is_none() {
            root["configurations"] = json!([]);
        }

        let configurations = root["configurations"]
            .as_array_mut()
            .ok_or_else(|| fdemon_core::Error::config("`configurations` is not an array"))?;

        merge_json_array_entry(
            configurations,
            "name",
            FDEMON_CONFIG_NAME,
            Self::fdemon_entry(port),
        );

        Ok(to_pretty_json(&root))
    }

    fn ide_name(&self) -> &'static str {
        "VS Code"
    }
}

// ─────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_vscode_config_path() {
        let gen = VSCodeGenerator;
        assert_eq!(
            gen.config_path(Path::new("/project")),
            PathBuf::from("/project/.vscode/launch.json")
        );
    }

    #[test]
    fn test_vscode_fresh_generation() {
        let gen = VSCodeGenerator;
        let content = gen.generate(4711, Path::new("/project")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed["version"], "0.2.0");
        let configs = parsed["configurations"].as_array().unwrap();
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0]["name"], "Flutter (fdemon)");
        assert_eq!(configs[0]["debugServer"], 4711);
        assert_eq!(configs[0]["type"], "dart");
        assert_eq!(configs[0]["request"], "attach");
        assert_eq!(configs[0]["fdemon-managed"], true);
    }

    #[test]
    fn test_vscode_fresh_generation_port_substitution() {
        let gen = VSCodeGenerator;
        let content = gen.generate(9999, Path::new("/project")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed["configurations"][0]["debugServer"], 9999);
    }

    #[test]
    fn test_vscode_merge_updates_existing_entry() {
        let existing = r#"{
            "version": "0.2.0",
            "configurations": [
                {"name": "Dart", "type": "dart", "request": "launch"},
                {"name": "Flutter (fdemon)", "type": "dart", "debugServer": 1234, "fdemon-managed": true}
            ]
        }"#;
        let gen = VSCodeGenerator;
        let merged = gen.merge_config(existing, 5678, Path::new("")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&merged).unwrap();
        let configs = parsed["configurations"].as_array().unwrap();
        assert_eq!(configs.len(), 2);
        assert_eq!(configs[0]["name"], "Dart"); // preserved
        assert_eq!(configs[1]["debugServer"], 5678); // updated
    }

    #[test]
    fn test_vscode_merge_appends_when_no_fdemon_entry() {
        let existing = r#"{
            "version": "0.2.0",
            "configurations": [
                {"name": "Dart", "type": "dart", "request": "launch"}
            ]
        }"#;
        let gen = VSCodeGenerator;
        let merged = gen.merge_config(existing, 4711, Path::new("")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&merged).unwrap();
        let configs = parsed["configurations"].as_array().unwrap();
        assert_eq!(configs.len(), 2);
        assert_eq!(configs[1]["name"], "Flutter (fdemon)");
    }

    #[test]
    fn test_vscode_merge_handles_jsonc_comments() {
        let existing = r#"{
            // This is a comment
            "version": "0.2.0",
            "configurations": [
                {"name": "Dart", "type": "dart"}
            ]
        }"#;
        let gen = VSCodeGenerator;
        let result = gen.merge_config(existing, 4711, Path::new(""));
        assert!(result.is_ok());
    }

    #[test]
    fn test_vscode_merge_malformed_json_returns_error() {
        let gen = VSCodeGenerator;
        let result = gen.merge_config("not json at all {{{", 4711, Path::new(""));
        assert!(result.is_err());
    }

    #[test]
    fn test_vscode_merge_preserves_version() {
        let existing = r#"{"version": "0.2.0", "configurations": []}"#;
        let gen = VSCodeGenerator;
        let merged = gen.merge_config(existing, 4711, Path::new("")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&merged).unwrap();
        assert_eq!(parsed["version"], "0.2.0");
    }

    #[test]
    fn test_vscode_merge_no_configurations_key() {
        let existing = r#"{"version": "0.2.0"}"#;
        let gen = VSCodeGenerator;
        let merged = gen.merge_config(existing, 4711, Path::new("")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&merged).unwrap();
        assert!(parsed["configurations"].is_array());
    }

    #[test]
    fn test_vscode_merge_empty_file_acts_as_fresh_generation() {
        let gen = VSCodeGenerator;
        let result = gen.merge_config("", 4711, Path::new(""));
        assert!(result.is_ok());
        let parsed: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(parsed["configurations"][0]["debugServer"], 4711);
    }

    #[test]
    fn test_vscode_merge_preserves_other_configurations() {
        let existing = r#"{
            "version": "0.2.0",
            "configurations": [
                {"name": "Config A", "type": "dart"},
                {"name": "Flutter (fdemon)", "debugServer": 1000, "fdemon-managed": true},
                {"name": "Config B", "type": "dart"}
            ]
        }"#;
        let gen = VSCodeGenerator;
        let merged = gen.merge_config(existing, 4711, Path::new("")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&merged).unwrap();
        let configs = parsed["configurations"].as_array().unwrap();
        assert_eq!(configs.len(), 3);
        assert_eq!(configs[0]["name"], "Config A");
        assert_eq!(configs[1]["debugServer"], 4711);
        assert_eq!(configs[2]["name"], "Config B");
    }

    #[test]
    fn test_vscode_ide_name() {
        assert_eq!(VSCodeGenerator.ide_name(), "VS Code");
    }
}
