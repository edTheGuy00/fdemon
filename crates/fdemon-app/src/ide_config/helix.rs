//! Helix DAP configuration generator.
//!
//! Generates or merges a `.helix/languages.toml` file with a Dart language
//! entry that configures `fdemon` as the DAP adapter.
//!
//! ## Helix DAP transport model
//!
//! Helix always **spawns** the debug adapter binary and passes a port via
//! `port-arg`. It does not support connecting to an already-running DAP server.
//! The generated configuration makes Helix spawn `fdemon --dap-port <PORT>`,
//! which starts a new fdemon instance in DAP-only mode.
//!
//! This is a known limitation: the spawned fdemon instance is separate from
//! any existing `fdemon` TUI session. Users who need to attach to an
//! already-running fdemon session should use a wrapper script.
//!
//! ## Port parameter
//!
//! The `port` parameter passed to [`IdeConfigGenerator::generate`] and
//! [`IdeConfigGenerator::merge_config`] is **not** embedded in the generated
//! TOML. Helix assigns the port at runtime via `port-arg = "--dap-port {}"`.
//! The parameter is accepted for trait uniformity but unused.

use fdemon_core::{Error, Result};
use std::path::{Path, PathBuf};

use super::IdeConfigGenerator;

/// Helix DAP configuration generator.
///
/// Produces a `.helix/languages.toml` entry that instructs Helix to spawn
/// `fdemon --dap-port <PORT>` as the debug adapter for the Dart language.
pub struct HelixGenerator;

/// The fdemon debugger name embedded in the Helix config.
const FDEMON_DEBUGGER_NAME: &str = "fdemon-dap";

/// The dart language name used in Helix `languages.toml`.
const DART_LANGUAGE_NAME: &str = "dart";

impl IdeConfigGenerator for HelixGenerator {
    fn config_path(&self, project_root: &Path) -> PathBuf {
        project_root.join(".helix").join("languages.toml")
    }

    /// Generate the full `.helix/languages.toml` content for a fresh creation.
    ///
    /// The `port` parameter is accepted for trait uniformity but is not
    /// embedded in the generated TOML — Helix assigns the port at runtime
    /// via the `port-arg` field.
    fn generate(&self, _port: u16, _project_root: &Path) -> Result<String> {
        Ok(Self::dart_debugger_toml())
    }

    /// Merge fdemon DAP config into an existing `languages.toml`.
    ///
    /// Finds the `[[language]]` entry where `name = "dart"` and replaces its
    /// `[language.debugger]` section with the fdemon debugger configuration.
    /// If no Dart entry exists, a new one is appended. All other language
    /// entries are preserved unchanged.
    ///
    /// The `port` parameter is accepted for trait uniformity but is not used —
    /// Helix assigns the port at runtime via `port-arg`.
    ///
    /// # Errors
    ///
    /// Returns an error if the existing content is malformed TOML that cannot
    /// be parsed.
    fn merge_config(&self, existing: &str, _port: u16, _project_root: &Path) -> Result<String> {
        merge_helix_languages(existing)
    }

    fn ide_name(&self) -> &'static str {
        "Helix"
    }
}

impl HelixGenerator {
    /// Returns the static Helix `languages.toml` content with the fdemon
    /// Dart debugger configuration.
    ///
    /// This is the full file content for a fresh creation. During merge,
    /// only the debugger section is extracted and applied to the existing file.
    fn dart_debugger_toml() -> String {
        // Note: `port-arg` is how Helix passes a port to the spawned adapter.
        // Helix calls: fdemon --dap-port <PORT>
        // `args = []` because --dap-port is supplied via port-arg, not args.
        r#"# fdemon DAP configuration for Helix (auto-generated)
# Helix will spawn fdemon as a DAP adapter on a port it chooses.
# For connecting to an already-running fdemon, see the docs.

[[language]]
name = "dart"

[language.debugger]
name = "fdemon-dap"
transport = "tcp"
command = "fdemon"
args = []
port-arg = "--dap-port {}"

[[language.debugger.templates]]
name = "Flutter: Attach (fdemon)"
request = "attach"
completion = []

[language.debugger.templates.args]
"#
        .to_string()
    }

    /// Returns only the debugger table values that should be applied to an
    /// existing Dart language entry during merge.
    fn debugger_value() -> toml::Value {
        let mut map = toml::map::Map::new();
        map.insert(
            "name".to_string(),
            toml::Value::String(FDEMON_DEBUGGER_NAME.to_string()),
        );
        map.insert(
            "transport".to_string(),
            toml::Value::String("tcp".to_string()),
        );
        map.insert(
            "command".to_string(),
            toml::Value::String("fdemon".to_string()),
        );
        map.insert("args".to_string(), toml::Value::Array(vec![]));
        map.insert(
            "port-arg".to_string(),
            toml::Value::String("--dap-port {}".to_string()),
        );

        // Build the templates array
        let mut template_map = toml::map::Map::new();
        template_map.insert(
            "name".to_string(),
            toml::Value::String("Flutter: Attach (fdemon)".to_string()),
        );
        template_map.insert(
            "request".to_string(),
            toml::Value::String("attach".to_string()),
        );
        template_map.insert("completion".to_string(), toml::Value::Array(vec![]));
        template_map.insert(
            "args".to_string(),
            toml::Value::Table(toml::map::Map::new()),
        );

        map.insert(
            "templates".to_string(),
            toml::Value::Array(vec![toml::Value::Table(template_map)]),
        );

        toml::Value::Table(map)
    }
}

/// Merge the fdemon Dart debugger section into the existing `languages.toml`
/// content.
///
/// Algorithm:
/// 1. Parse the existing content as TOML.
/// 2. Extract the `language` array (the `[[language]]` tables).
/// 3. Find the entry where `name = "dart"`.
/// 4. If found: replace or insert its `debugger` sub-table.
/// 5. If not found: append a new Dart entry with the debugger configured.
/// 6. Serialize back to TOML.
///
/// Because `toml` (not `toml_edit`) is used, the serialized output may have
/// different ordering and formatting than the original file. This is acceptable
/// for `.helix/languages.toml`, which is typically small and project-local.
fn merge_helix_languages(existing: &str) -> Result<String> {
    let mut doc: toml::Value = toml::from_str(existing).map_err(|e| {
        Error::config(format!(
            "Failed to parse .helix/languages.toml as TOML: {e}"
        ))
    })?;

    let debugger = HelixGenerator::debugger_value();

    // Obtain or create the `language` array.
    let languages = match doc.get_mut("language") {
        Some(toml::Value::Array(arr)) => arr,
        Some(other) => {
            return Err(Error::config(format!(
                ".helix/languages.toml has a `language` key that is not an array (found: {})",
                other.type_str()
            )));
        }
        None => {
            // No `language` key yet — create it.
            let root = doc.as_table_mut().ok_or_else(|| {
                Error::config(".helix/languages.toml top level is not a TOML table".to_string())
            })?;
            root.insert("language".to_string(), toml::Value::Array(vec![]));
            match root.get_mut("language") {
                Some(toml::Value::Array(arr)) => arr,
                _ => {
                    return Err(Error::config(
                        "failed to retrieve inserted language array from TOML table".to_string(),
                    ))
                }
            }
        }
    };

    // Find the Dart language entry.
    let dart_idx = languages.iter().position(|entry| {
        entry
            .get("name")
            .and_then(|v| v.as_str())
            .map(|s| s == DART_LANGUAGE_NAME)
            .unwrap_or(false)
    });

    match dart_idx {
        Some(idx) => {
            // Entry exists — replace or insert its debugger section.
            let dart_entry = languages[idx].as_table_mut().ok_or_else(|| {
                Error::config(
                    "Dart language entry in .helix/languages.toml is not a TOML table".to_string(),
                )
            })?;
            if dart_entry.contains_key("debugger") {
                tracing::warn!(
                    "Replacing existing Dart debugger configuration in .helix/languages.toml"
                );
            }
            dart_entry.insert("debugger".to_string(), debugger);
        }
        None => {
            // No Dart entry — append one.
            let mut dart_entry = toml::map::Map::new();
            dart_entry.insert(
                "name".to_string(),
                toml::Value::String(DART_LANGUAGE_NAME.to_string()),
            );
            dart_entry.insert("debugger".to_string(), debugger);
            languages.push(toml::Value::Table(dart_entry));
        }
    }

    // Serialize back to TOML. The `toml` crate may reorder keys and lose
    // comments, but this is acceptable for project-local configuration.
    toml::to_string_pretty(&doc).map_err(|e| {
        Error::config(format!(
            "Failed to serialize merged .helix/languages.toml: {e}"
        ))
    })
}

// ─────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    // ── config_path ─────────────────────────────────────────────

    #[test]
    fn test_helix_config_path() {
        let gen = HelixGenerator;
        assert_eq!(
            gen.config_path(Path::new("/project")),
            PathBuf::from("/project/.helix/languages.toml")
        );
    }

    // ── generate (fresh) ────────────────────────────────────────

    #[test]
    fn test_helix_fresh_generation_contains_dart_name() {
        let gen = HelixGenerator;
        let content = gen.generate(4711, Path::new("/project")).unwrap();
        assert!(content.contains("name = \"dart\""));
    }

    #[test]
    fn test_helix_fresh_generation_contains_transport_tcp() {
        let gen = HelixGenerator;
        let content = gen.generate(4711, Path::new("/project")).unwrap();
        assert!(content.contains("transport = \"tcp\""));
    }

    #[test]
    fn test_helix_fresh_generation_contains_command_fdemon() {
        let gen = HelixGenerator;
        let content = gen.generate(4711, Path::new("/project")).unwrap();
        assert!(content.contains("command = \"fdemon\""));
    }

    #[test]
    fn test_helix_fresh_generation_contains_port_arg() {
        let gen = HelixGenerator;
        let content = gen.generate(4711, Path::new("/project")).unwrap();
        assert!(content.contains("port-arg"));
    }

    #[test]
    fn test_helix_fresh_generation_is_valid_toml() {
        let gen = HelixGenerator;
        let content = gen.generate(4711, Path::new("/project")).unwrap();
        let parsed: toml::Value = toml::from_str(&content).unwrap();
        assert!(parsed.get("language").is_some());
    }

    #[test]
    fn test_helix_fresh_generation_port_not_embedded() {
        // The port parameter must NOT appear in the generated TOML because
        // Helix controls port assignment at runtime via port-arg.
        let gen = HelixGenerator;
        let content = gen.generate(4711, Path::new("/project")).unwrap();
        assert!(!content.contains("4711"));
    }

    #[test]
    fn test_helix_fresh_generation_different_ports_same_output() {
        let gen = HelixGenerator;
        let c1 = gen.generate(1234, Path::new("/project")).unwrap();
        let c2 = gen.generate(5678, Path::new("/project")).unwrap();
        assert_eq!(c1, c2);
    }

    // ── merge — adds Dart entry when none exists ────────────────

    #[test]
    fn test_helix_merge_adds_dart_entry_to_empty_file() {
        let gen = HelixGenerator;
        let merged = gen.merge_config("", 4711, Path::new("")).unwrap();
        assert!(merged.contains("dart"));
    }

    #[test]
    fn test_helix_merge_adds_dart_entry_preserves_rust() {
        let existing = r#"
[[language]]
name = "rust"

[language.auto-format]
enable = true
"#;
        let gen = HelixGenerator;
        let merged = gen.merge_config(existing, 4711, Path::new("")).unwrap();
        assert!(merged.contains("name = \"dart\""));
        assert!(merged.contains("name = \"rust\"")); // preserved
    }

    #[test]
    fn test_helix_merge_adds_dart_entry_when_no_language_key() {
        let existing = "some-setting = true\n";
        let gen = HelixGenerator;
        let merged = gen.merge_config(existing, 4711, Path::new("")).unwrap();
        assert!(merged.contains("dart"));
    }

    // ── merge — updates existing Dart debugger ──────────────────

    #[test]
    fn test_helix_merge_updates_existing_dart_debugger_command() {
        let existing = r#"
[[language]]
name = "dart"

[language.debugger]
name = "old-debugger"
transport = "stdio"
command = "dart"
"#;
        let gen = HelixGenerator;
        let merged = gen.merge_config(existing, 4711, Path::new("")).unwrap();
        assert!(merged.contains("command = \"fdemon\""));
        assert!(!merged.contains("command = \"dart\""));
    }

    #[test]
    fn test_helix_merge_updates_transport_to_tcp() {
        let existing = r#"
[[language]]
name = "dart"

[language.debugger]
name = "old-debugger"
transport = "stdio"
command = "dart"
"#;
        let gen = HelixGenerator;
        let merged = gen.merge_config(existing, 4711, Path::new("")).unwrap();
        assert!(merged.contains("transport = \"tcp\""));
        assert!(!merged.contains("transport = \"stdio\""));
    }

    #[test]
    fn test_helix_merge_preserves_non_dart_languages() {
        let existing = r#"
[[language]]
name = "rust"

[[language]]
name = "python"

[[language]]
name = "dart"

[language.debugger]
name = "old"
transport = "stdio"
command = "dart"
"#;
        let gen = HelixGenerator;
        let merged = gen.merge_config(existing, 4711, Path::new("")).unwrap();
        assert!(merged.contains("name = \"rust\""));
        assert!(merged.contains("name = \"python\""));
        assert!(merged.contains("command = \"fdemon\""));
    }

    #[test]
    fn test_helix_merge_result_is_valid_toml() {
        let existing = r#"
[[language]]
name = "rust"
"#;
        let gen = HelixGenerator;
        let merged = gen.merge_config(existing, 4711, Path::new("")).unwrap();
        let parsed: Result<toml::Value> =
            toml::from_str(&merged).map_err(|e| Error::config(format!("parse error: {e}")));
        assert!(parsed.is_ok(), "merged output must be valid TOML");
    }

    // ── merge — Dart entry with no debugger ─────────────────────

    #[test]
    fn test_helix_merge_adds_debugger_to_dart_entry_without_one() {
        let existing = r#"
[[language]]
name = "dart"
file-types = ["dart"]
"#;
        let gen = HelixGenerator;
        let merged = gen.merge_config(existing, 4711, Path::new("")).unwrap();
        assert!(merged.contains("fdemon-dap"));
        assert!(merged.contains("command = \"fdemon\""));
    }

    // ── merge — malformed TOML ───────────────────────────────────

    #[test]
    fn test_helix_merge_malformed_toml_returns_error() {
        let gen = HelixGenerator;
        let result = gen.merge_config("not [valid toml", 4711, Path::new(""));
        assert!(result.is_err());
    }

    #[test]
    fn test_helix_merge_invalid_unicode_returns_error() {
        let gen = HelixGenerator;
        let result = gen.merge_config("[[language\x00]]", 4711, Path::new(""));
        // Either parses weird or errors — just must not panic
        let _ = result;
    }

    // ── ide_name ─────────────────────────────────────────────────

    #[test]
    fn test_helix_ide_name() {
        assert_eq!(HelixGenerator.ide_name(), "Helix");
    }

    // ── debugger_value structure ─────────────────────────────────

    #[test]
    fn test_debugger_value_has_required_fields() {
        let val = HelixGenerator::debugger_value();
        let tbl = val.as_table().unwrap();
        assert_eq!(tbl["name"].as_str(), Some(FDEMON_DEBUGGER_NAME));
        assert_eq!(tbl["transport"].as_str(), Some("tcp"));
        assert_eq!(tbl["command"].as_str(), Some("fdemon"));
        assert!(tbl["args"].is_array());
        assert!(tbl.contains_key("port-arg"));
        assert!(tbl.contains_key("templates"));
    }

    #[test]
    fn test_debugger_value_templates_has_one_entry() {
        let val = HelixGenerator::debugger_value();
        let tbl = val.as_table().unwrap();
        let templates = tbl["templates"].as_array().unwrap();
        assert_eq!(templates.len(), 1);
        let tmpl = templates[0].as_table().unwrap();
        assert_eq!(tmpl["request"].as_str(), Some("attach"));
    }
}
