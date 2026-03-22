//! Neovim (nvim-dap) DAP configuration generator.
//!
//! Neovim's nvim-dap plugin supports two configuration approaches:
//!
//! 1. **Primary**: `.vscode/launch.json` — loaded via
//!    `require("dap.ext.vscode").load_launchjs()`. This is the most common
//!    setup and is handled by delegating to [`super::vscode::VSCodeGenerator`].
//!
//! 2. **Secondary**: `.nvim-dap.lua` — a project-local Lua snippet that users
//!    can source directly. Written as an informational best-effort file; failure
//!    to write it never fails the overall config generation.

use std::path::{Path, PathBuf};

use fdemon_core::Result;

use super::{vscode::VSCodeGenerator, IdeConfigGenerator};

/// Generates DAP config for Neovim's nvim-dap plugin.
///
/// The primary config target is `.vscode/launch.json` (same format as VS Code),
/// because nvim-dap can load VS Code launch configs via `load_launchjs()`.
/// Additionally writes a `.nvim-dap.lua` snippet in the project root as an
/// informational alternative for users who prefer native nvim-dap configuration.
pub struct NeovimGenerator;

impl NeovimGenerator {
    /// Generate the Lua snippet content for `.nvim-dap.lua`.
    ///
    /// The snippet configures nvim-dap with an `fdemon` adapter that connects
    /// to the given `port` and appends a `dart` configuration.
    pub fn generate_lua_snippet(&self, port: u16) -> String {
        format!(
            r#"-- fdemon DAP configuration for Neovim (auto-generated)
--
-- Option 1: Source this file in your Neovim config:
--   dofile(vim.fn.getcwd() .. '/.nvim-dap.lua')
--
-- Option 2: Use load_launchjs() to read .vscode/launch.json:
--   require('dap.ext.vscode').load_launchjs()
--
-- Option 2 is recommended -- fdemon auto-generates .vscode/launch.json

local dap = require('dap')

dap.adapters.fdemon = {{
  type = 'server',
  host = '127.0.0.1',
  port = {port},
}}

dap.configurations.dart = dap.configurations.dart or {{}}
table.insert(dap.configurations.dart, {{
  type = 'fdemon',
  request = 'attach',
  name = 'Flutter (fdemon)',
  cwd = vim.fn.getcwd(),
}})
"#,
            port = port
        )
    }

    /// Write `.nvim-dap.lua` to the project root.
    ///
    /// This is a best-effort operation — errors are logged as warnings but do
    /// not propagate to the caller. The file is always overwritten (fdemon-owned).
    pub fn write_nvim_dap_lua(&self, port: u16, project_root: &Path) {
        let path = project_root.join(".nvim-dap.lua");
        let content = self.generate_lua_snippet(port);
        match std::fs::write(&path, content) {
            Ok(()) => {
                tracing::debug!("Wrote .nvim-dap.lua at {}", path.display());
            }
            Err(e) => {
                tracing::warn!("Failed to write .nvim-dap.lua: {}", e);
            }
        }
    }
}

impl IdeConfigGenerator for NeovimGenerator {
    /// Returns the path to `.vscode/launch.json` — the primary config target
    /// for nvim-dap via `load_launchjs()`.
    fn config_path(&self, project_root: &Path) -> PathBuf {
        project_root.join(".vscode").join("launch.json")
    }

    /// Generate a fresh `.vscode/launch.json`.
    ///
    /// The JSON content is identical to the VS Code generator output.
    /// The secondary `.nvim-dap.lua` file is written by [`post_write`] so
    /// that it is kept in sync on both create and merge paths via `run_generator`.
    fn generate(&self, port: u16, project_root: &Path) -> Result<String> {
        let vscode = VSCodeGenerator;
        vscode.generate(port, project_root)
    }

    /// Merge the fdemon entry into an existing `.vscode/launch.json`.
    ///
    /// Delegates entirely to the VS Code generator's merge logic.
    fn merge_config(&self, existing: &str, port: u16, _project_root: &Path) -> Result<String> {
        let vscode = VSCodeGenerator;
        vscode.merge_config(existing, port, Path::new(""))
    }

    /// Write (or overwrite) the secondary `.nvim-dap.lua` file.
    ///
    /// Called by [`super::run_generator`] after both fresh creation and merging
    /// so the Lua snippet is kept in sync with `.vscode/launch.json` on every
    /// DAP server start.  Errors are logged as warnings and do not propagate.
    fn post_write(&self, port: u16, project_root: &Path) -> Result<()> {
        self.write_nvim_dap_lua(port, project_root);
        Ok(())
    }

    fn ide_name(&self) -> &'static str {
        "Neovim"
    }
}

// ─────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};
    use tempfile::tempdir;

    #[test]
    fn test_neovim_config_path_is_vscode_launch_json() {
        let gen = NeovimGenerator;
        assert_eq!(
            gen.config_path(Path::new("/project")),
            PathBuf::from("/project/.vscode/launch.json")
        );
    }

    #[test]
    fn test_neovim_fresh_generation_produces_valid_launch_json() {
        let dir = tempdir().unwrap();
        let gen = NeovimGenerator;
        let content = gen.generate(4711, dir.path()).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed["configurations"][0]["debugServer"], 4711);
    }

    #[test]
    fn test_neovim_fresh_generation_has_required_fields() {
        let dir = tempdir().unwrap();
        let gen = NeovimGenerator;
        let content = gen.generate(4711, dir.path()).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        let cfg = &parsed["configurations"][0];
        assert_eq!(cfg["name"], "Flutter (fdemon)");
        assert_eq!(cfg["type"], "dart");
        assert_eq!(cfg["request"], "attach");
        assert!(cfg.get("fdemon-managed").is_none());
    }

    #[test]
    fn test_neovim_generation_writes_lua_snippet() {
        // The Lua snippet is now written by post_write() rather than generate(),
        // so that it is updated on both create and merge paths via run_generator.
        // Test via post_write() directly.
        let dir = tempdir().unwrap();
        let gen = NeovimGenerator;
        gen.post_write(4711, dir.path()).unwrap();
        let lua_path = dir.path().join(".nvim-dap.lua");
        assert!(lua_path.exists());
        let content = std::fs::read_to_string(&lua_path).unwrap();
        assert!(content.contains("port = 4711"));
        assert!(content.contains("dap.adapters.fdemon"));
        assert!(content.contains("type = 'server'"));
    }

    #[test]
    fn test_neovim_post_write_updates_lua_on_merge() {
        // Verify that post_write updates .nvim-dap.lua even when called after merge.
        let dir = tempdir().unwrap();
        let old_lua_path = dir.path().join(".nvim-dap.lua");
        std::fs::write(&old_lua_path, "old port = 1234").unwrap();
        let gen = NeovimGenerator;
        gen.post_write(5678, dir.path()).unwrap();
        let content = std::fs::read_to_string(&old_lua_path).unwrap();
        assert!(content.contains("port = 5678"));
        assert!(!content.contains("old port"));
    }

    #[test]
    fn test_neovim_lua_snippet_port_substitution() {
        let gen = NeovimGenerator;
        let lua = gen.generate_lua_snippet(9999);
        assert!(lua.contains("port = 9999"));
        assert!(!lua.contains("port = 4711"));
    }

    #[test]
    fn test_neovim_lua_snippet_contains_usage_instructions() {
        let gen = NeovimGenerator;
        let lua = gen.generate_lua_snippet(4711);
        assert!(lua.contains("load_launchjs"));
        assert!(lua.contains("dofile"));
        assert!(lua.contains("Option 1"));
        assert!(lua.contains("Option 2"));
    }

    #[test]
    fn test_neovim_lua_snippet_contains_adapter_config() {
        let gen = NeovimGenerator;
        let lua = gen.generate_lua_snippet(4711);
        assert!(lua.contains("dap.adapters.fdemon"));
        assert!(lua.contains("type = 'server'"));
        assert!(lua.contains("host = '127.0.0.1'"));
    }

    #[test]
    fn test_neovim_lua_snippet_contains_dart_configuration() {
        let gen = NeovimGenerator;
        let lua = gen.generate_lua_snippet(4711);
        assert!(lua.contains("dap.configurations.dart"));
        assert!(lua.contains("type = 'fdemon'"));
        assert!(lua.contains("request = 'attach'"));
        assert!(lua.contains("name = 'Flutter (fdemon)'"));
    }

    #[test]
    fn test_neovim_merge_delegates_to_vscode() {
        let existing = r#"{
            "version": "0.2.0",
            "configurations": [
                {"name": "Dart", "type": "dart", "request": "launch"}
            ]
        }"#;
        let gen = NeovimGenerator;
        let merged = gen.merge_config(existing, 4711, Path::new("")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&merged).unwrap();
        let configs = parsed["configurations"].as_array().unwrap();
        assert_eq!(configs.len(), 2);
    }

    #[test]
    fn test_neovim_merge_updates_existing_fdemon_entry() {
        let existing = r#"{
            "version": "0.2.0",
            "configurations": [
                {"name": "Flutter (fdemon)", "debugServer": 1234, "fdemon-managed": true}
            ]
        }"#;
        let gen = NeovimGenerator;
        let merged = gen.merge_config(existing, 5678, Path::new("")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&merged).unwrap();
        assert_eq!(parsed["configurations"][0]["debugServer"], 5678);
    }

    #[test]
    fn test_neovim_merge_malformed_json_returns_error() {
        let gen = NeovimGenerator;
        let result = gen.merge_config("not json {{{", 4711, Path::new(""));
        assert!(result.is_err());
    }

    #[test]
    fn test_neovim_ide_name() {
        assert_eq!(NeovimGenerator.ide_name(), "Neovim");
    }

    #[test]
    fn test_neovim_write_lua_snippet_creates_file() {
        let dir = tempdir().unwrap();
        let gen = NeovimGenerator;
        gen.write_nvim_dap_lua(4711, dir.path());
        assert!(dir.path().join(".nvim-dap.lua").exists());
    }

    #[test]
    fn test_neovim_write_lua_snippet_overwrites_existing() {
        let dir = tempdir().unwrap();
        let lua_path = dir.path().join(".nvim-dap.lua");

        // Write an initial file with old port.
        std::fs::write(&lua_path, "old content").unwrap();

        let gen = NeovimGenerator;
        gen.write_nvim_dap_lua(9999, dir.path());

        let content = std::fs::read_to_string(&lua_path).unwrap();
        assert!(content.contains("port = 9999"));
        assert!(!content.contains("old content"));
    }

    #[test]
    fn test_neovim_write_lua_snippet_failure_does_not_panic() {
        // Use a non-existent parent directory to trigger a write error.
        let gen = NeovimGenerator;
        // This should not panic — it logs a warning and continues.
        gen.write_nvim_dap_lua(4711, Path::new("/nonexistent/deep/path"));
    }
}
