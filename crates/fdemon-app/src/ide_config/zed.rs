//! Zed DAP client configuration generator.
//!
//! Generates or merges a `.zed/debug.json` entry that connects to fdemon's
//! DAP server via TCP. Uses the `Delve` adapter with `tcp_connection` so
//! Zed's debug panel recognises the entry and connects to the running DAP
//! server.
//!
//! # Format
//!
//! `.zed/debug.json` is a flat JSON **array** (no wrapping object), e.g.:
//!
//! ```json
//! [
//!   {
//!     "label": "Flutter Demon (TCP)",
//!     "adapter": "Delve",
//!     "request": "attach",
//!     "tcp_connection": { "host": "127.0.0.1", "port": 4711 }
//!   }
//! ]
//! ```

use super::{merge_json_array_entry, to_pretty_json, IdeConfigGenerator};
use fdemon_core::Result;
use std::path::{Path, PathBuf};

/// The label used to identify the fdemon DAP entry in Zed's `debug.json`.
const ZED_FDEMON_LABEL: &str = "Flutter Demon (TCP)";

/// The TCP host for the DAP server connection.
const ZED_DAP_HOST: &str = "127.0.0.1";

/// Zed DAP configuration generator.
///
/// Produces or updates a `.zed/debug.json` entry that connects to fdemon's
/// DAP server via TCP.
pub struct ZedGenerator;

impl ZedGenerator {
    /// Build the fdemon DAP entry for Zed's `debug.json`.
    ///
    /// # Adapter workaround
    ///
    /// Zed does not yet have a native Dart or Flutter DAP adapter type.
    /// As a workaround, this entry uses `"adapter": "Delve"` (the Go debugger
    /// adapter), which is one of the adapter names Zed's debug panel already
    /// recognises.  Fdemon's DAP server handles the actual Dart/Flutter
    /// protocol; Zed simply forwards the TCP connection to it.
    ///
    /// **Note:** If a future Zed release validates that the chosen adapter
    /// matches the project language, this workaround will break and will need
    /// to be updated to use a native Dart adapter name once Zed supports one.
    fn fdemon_entry(port: u16) -> serde_json::Value {
        serde_json::json!({
            "label": ZED_FDEMON_LABEL,
            "adapter": "Delve",
            "request": "attach",
            "tcp_connection": {
                "host": ZED_DAP_HOST,
                "port": port
            }
        })
    }
}

impl IdeConfigGenerator for ZedGenerator {
    /// Returns the path to Zed's debug config file within the project root.
    fn config_path(&self, project_root: &Path) -> PathBuf {
        project_root.join(".zed").join("debug.json")
    }

    /// Generate a fresh `.zed/debug.json` containing a single fdemon entry.
    fn generate(&self, port: u16, _project_root: &Path) -> Result<String> {
        let config = serde_json::json!([Self::fdemon_entry(port)]);
        Ok(to_pretty_json(&config))
    }

    /// Merge the fdemon DAP entry into an existing `.zed/debug.json`.
    ///
    /// Finds an existing fdemon entry by `"label" == "Flutter Demon (TCP)"` and
    /// updates its `tcp_connection.port`. If no matching entry is found, appends
    /// a new one. All non-fdemon entries are preserved unchanged.
    ///
    /// # Errors
    ///
    /// Returns [`fdemon_core::Error::Config`] if `existing` is not valid JSON or
    /// is not a JSON array.
    fn merge_config(&self, existing: &str, port: u16, _project_root: &Path) -> Result<String> {
        let mut array: Vec<serde_json::Value> = serde_json::from_str(existing)
            .map_err(|e| fdemon_core::Error::config(format!("invalid JSON in debug.json: {e}")))?;

        merge_json_array_entry(
            &mut array,
            "label",
            ZED_FDEMON_LABEL,
            Self::fdemon_entry(port),
        );

        Ok(to_pretty_json(&serde_json::Value::Array(array)))
    }

    /// Display name used in log messages.
    fn ide_name(&self) -> &'static str {
        "Zed"
    }
}

// ─────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // ── config_path ─────────────────────────────────────────────

    #[test]
    fn test_zed_config_path() {
        let gen = ZedGenerator;
        assert_eq!(
            gen.config_path(Path::new("/project")),
            PathBuf::from("/project/.zed/debug.json")
        );
    }

    // ── generate (fresh) ────────────────────────────────────────

    #[test]
    fn test_zed_fresh_generation() {
        let gen = ZedGenerator;
        let content = gen.generate(4711, Path::new("/project")).unwrap();
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0]["label"], "Flutter Demon (TCP)");
        assert_eq!(parsed[0]["tcp_connection"]["port"], 4711);
        assert_eq!(parsed[0]["tcp_connection"]["host"], "127.0.0.1");
        assert_eq!(parsed[0]["adapter"], "Delve");
    }

    #[test]
    fn test_zed_fresh_generation_embeds_correct_port() {
        let gen = ZedGenerator;
        let content = gen.generate(9999, Path::new("/project")).unwrap();
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed[0]["tcp_connection"]["port"], 9999);
    }

    #[test]
    fn test_zed_fresh_generation_has_request_attach() {
        let gen = ZedGenerator;
        let content = gen.generate(4711, Path::new("/project")).unwrap();
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed[0]["request"], "attach");
    }

    #[test]
    fn test_zed_fresh_generation_is_valid_json_array() {
        let gen = ZedGenerator;
        let content = gen.generate(4711, Path::new("/project")).unwrap();
        // Must parse as a JSON array (not an object)
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert!(parsed.is_array());
    }

    // ── merge_config ─────────────────────────────────────────────

    #[test]
    fn test_zed_merge_updates_existing_entry() {
        let existing = r#"[
            {"label": "Other", "adapter": "other"},
            {"label": "Flutter Demon (TCP)", "adapter": "Delve", "tcp_connection": {"host": "127.0.0.1", "port": 1234}}
        ]"#;
        let gen = ZedGenerator;
        let merged = gen.merge_config(existing, 5678, Path::new("")).unwrap();
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&merged).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0]["label"], "Other"); // preserved
        assert_eq!(parsed[1]["tcp_connection"]["port"], 5678); // updated
    }

    #[test]
    fn test_zed_merge_appends_when_no_fdemon_entry() {
        let existing = r#"[{"label": "Other", "adapter": "other"}]"#;
        let gen = ZedGenerator;
        let merged = gen.merge_config(existing, 4711, Path::new("")).unwrap();
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&merged).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[1]["label"], "Flutter Demon (TCP)");
    }

    #[test]
    fn test_zed_merge_empty_array() {
        let gen = ZedGenerator;
        let merged = gen.merge_config("[]", 4711, Path::new("")).unwrap();
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&merged).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0]["label"], "Flutter Demon (TCP)");
    }

    #[test]
    fn test_zed_merge_malformed_json_returns_error() {
        let gen = ZedGenerator;
        let result = gen.merge_config("not json", 4711, Path::new(""));
        assert!(result.is_err());
    }

    #[test]
    fn test_zed_merge_non_array_json_returns_error() {
        let gen = ZedGenerator;
        // A JSON object is not a valid debug.json (must be an array)
        let result = gen.merge_config(r#"{"key": "value"}"#, 4711, Path::new(""));
        assert!(result.is_err());
    }

    #[test]
    fn test_zed_merge_preserves_non_fdemon_configs() {
        let existing = r#"[
            {"label": "Config A", "adapter": "a"},
            {"label": "Config B", "adapter": "b"}
        ]"#;
        let gen = ZedGenerator;
        let merged = gen.merge_config(existing, 4711, Path::new("")).unwrap();
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&merged).unwrap();
        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[0]["label"], "Config A");
        assert_eq!(parsed[1]["label"], "Config B");
        assert_eq!(parsed[2]["label"], "Flutter Demon (TCP)");
    }

    #[test]
    fn test_zed_merge_updates_port_only_keeps_full_entry() {
        let existing = r#"[
            {"label": "Flutter Demon (TCP)", "adapter": "Delve", "request": "attach",
             "tcp_connection": {"host": "127.0.0.1", "port": 1111}}
        ]"#;
        let gen = ZedGenerator;
        let merged = gen.merge_config(existing, 2222, Path::new("")).unwrap();
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&merged).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0]["tcp_connection"]["port"], 2222);
        assert_eq!(parsed[0]["adapter"], "Delve");
    }

    // ── ide_name ─────────────────────────────────────────────────

    #[test]
    fn test_zed_ide_name() {
        assert_eq!(ZedGenerator.ide_name(), "Zed");
    }
}
