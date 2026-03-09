//! IDE-specific DAP client configuration generation.
//!
//! This module provides the [`IdeConfigGenerator`] trait and the top-level
//! [`generate_ide_config()`] dispatch function. Each IDE has its own submodule
//! (Tasks 04–08) that implements the trait for a specific config format.
//!
//! ## Design
//!
//! The dispatch function owns file I/O (read / mkdir / write) so that generator
//! implementations only need to produce or transform string content. This makes
//! generators pure and straightforward to unit-test without touching the file
//! system.
//!
//! ## Submodule status
//!
//! Per-IDE submodules are added incrementally in later tasks:
//!
//! | Module | IDE | Task |
//! |--------|-----|------|
//! | `vscode` | VS Code, Cursor, Neovim | Task 04 |
//! | `neovim` | Neovim (nvim-dap) | Task 05 |
//! | `helix`  | Helix | Task 06 |
//! | `zed`    | Zed | Task 07 |
//! | `emacs`  | Emacs (dap-mode) | Task 08 |

pub mod emacs;
pub mod helix;
pub(crate) mod merge;
pub mod neovim;
pub mod vscode;
pub mod zed;

use fdemon_core::Result;
use std::path::{Path, PathBuf};

// ─────────────────────────────────────────────────────────────────
// Public types
// ─────────────────────────────────────────────────────────────────

/// Result of an IDE config generation attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdeConfigResult {
    /// Path to the config file that was created or updated.
    pub path: PathBuf,
    /// What action was taken.
    pub action: ConfigAction,
}

/// What happened during config generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigAction {
    /// Config file was created (did not previously exist).
    Created,
    /// Existing config file was updated with a new/changed fdemon entry.
    Updated,
    /// Config generation was skipped (with reason).
    Skipped(String),
}

// ─────────────────────────────────────────────────────────────────
// IdeConfigGenerator trait
// ─────────────────────────────────────────────────────────────────

/// Trait for generating IDE-specific DAP client configuration files.
///
/// Each IDE has its own config format and file location. Implementations
/// handle both fresh creation and merging into existing config files.
///
/// The trait uses `&self` receivers so that generators can carry configuration
/// if needed, although initial implementations are unit structs.
pub trait IdeConfigGenerator {
    /// Returns the path where this IDE's config file should be written,
    /// relative to the project root.
    fn config_path(&self, project_root: &Path) -> PathBuf;

    /// Generate the full config file content for a fresh creation.
    fn generate(&self, port: u16, project_root: &Path) -> Result<String>;

    /// Check if a config file already exists at the expected path.
    ///
    /// The default implementation checks for file existence. Override only
    /// when a more sophisticated check is required.
    fn config_exists(&self, project_root: &Path) -> bool {
        self.config_path(project_root).exists()
    }

    /// Merge fdemon DAP config into an existing config file.
    ///
    /// Returns the merged content, or an error if the file cannot be parsed.
    ///
    /// `project_root` is the absolute path to the Flutter project root. Most
    /// implementations ignore it (pass `_project_root`), but generators that
    /// embed an absolute file path (e.g. Emacs) require it to avoid writing a
    /// relative placeholder.
    ///
    /// Implementations must:
    /// - Find existing fdemon entries (by marker) and update them
    /// - Append a new entry if no fdemon entry exists
    /// - Preserve all non-fdemon entries unchanged
    fn merge_config(&self, existing: &str, port: u16, project_root: &Path) -> Result<String>;

    /// Optional post-generation hook for secondary file writes.
    ///
    /// Called by [`run_generator`] after both fresh creation and merging, so
    /// secondary artifacts (e.g. Neovim's `.nvim-dap.lua`) are always kept in
    /// sync with the primary config file.
    ///
    /// The default implementation is a no-op. Override when the IDE requires
    /// additional files beyond the primary config path returned by
    /// [`IdeConfigGenerator::config_path`].
    fn post_write(&self, _port: u16, _project_root: &Path) -> Result<()> {
        Ok(())
    }

    /// The display name for this IDE (used in log messages).
    fn ide_name(&self) -> &'static str;
}

// ─────────────────────────────────────────────────────────────────
// File I/O helper
// ─────────────────────────────────────────────────────────────────

/// Execute a generator: handle file I/O and return an [`IdeConfigResult`].
///
/// This function is the single location that owns all filesystem operations
/// for IDE config generation. Generator implementations stay pure (string in,
/// string out) and are easy to unit-test without touching the filesystem.
///
/// Steps:
/// 1. Compute the target path via [`IdeConfigGenerator::config_path`].
/// 2. If the file already exists, read it and call [`IdeConfigGenerator::merge_config`].
/// 3. If the file does not exist, call [`IdeConfigGenerator::generate`] for fresh content.
/// 4. Ensure the parent directory exists (`create_dir_all`).
/// 5. Write the content and return an [`IdeConfigResult`].
/// 6. Call [`IdeConfigGenerator::post_write`] for any secondary file writes (e.g. Neovim Lua).
fn run_generator(
    generator: &dyn IdeConfigGenerator,
    port: u16,
    project_root: &Path,
) -> Result<Option<IdeConfigResult>> {
    let config_path = generator.config_path(project_root);

    let (content, action) = if generator.config_exists(project_root) {
        let existing = std::fs::read_to_string(&config_path)?;
        let merged = generator.merge_config(&existing, port, project_root)?;
        if merged == existing {
            tracing::info!(
                path = %config_path.display(),
                "{} DAP config skipped (content unchanged)",
                generator.ide_name(),
            );
            return Ok(Some(IdeConfigResult {
                path: config_path,
                action: ConfigAction::Skipped("content unchanged".to_string()),
            }));
        }
        (merged, ConfigAction::Updated)
    } else {
        let fresh = generator.generate(port, project_root)?;
        (fresh, ConfigAction::Created)
    };

    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&config_path, &content)?;

    // Call the post-write hook for secondary file writes (e.g. Neovim's .nvim-dap.lua).
    generator.post_write(port, project_root)?;

    let action_label = match &action {
        ConfigAction::Created => "created",
        ConfigAction::Updated => "updated",
        ConfigAction::Skipped(_) => "skipped",
    };
    tracing::info!(
        path = %config_path.display(),
        "{} DAP config {}",
        generator.ide_name(),
        action_label,
    );

    Ok(Some(IdeConfigResult {
        path: config_path,
        action,
    }))
}

// ─────────────────────────────────────────────────────────────────
// Dispatch function
// ─────────────────────────────────────────────────────────────────

/// Generate IDE-specific DAP config for the detected (or specified) IDE.
///
/// Returns `Ok(None)` when:
/// - `ide` is `None`
/// - The IDE doesn't support DAP config (e.g. IntelliJ, Android Studio)
///
/// On success returns an [`IdeConfigResult`] describing what was created or
/// updated.
pub fn generate_ide_config(
    ide: Option<crate::config::ParentIde>,
    port: u16,
    project_root: &Path,
) -> Result<Option<IdeConfigResult>> {
    use crate::config::ParentIde;

    let ide = match ide {
        Some(ide) if ide.supports_dap_config() => ide,
        _ => return Ok(None),
    };

    match ide {
        // VS Code, VS Code Insiders, and Cursor share the launch.json format (Task 04).
        ParentIde::VSCode | ParentIde::VSCodeInsiders | ParentIde::Cursor => {
            run_generator(&vscode::VSCodeGenerator, port, project_root)
        }
        // Neovim uses launch.json as primary (via load_launchjs) plus a Lua snippet (Task 05).
        ParentIde::Neovim => run_generator(&neovim::NeovimGenerator, port, project_root),
        // Helix uses .helix/languages.toml (Task 06).
        ParentIde::Helix => run_generator(&helix::HelixGenerator, port, project_root),
        // Emacs uses a .fdemon/dap-emacs.el Elisp snippet (Task 08).
        ParentIde::Emacs => run_generator(&emacs::EmacsGenerator, port, project_root),
        // Zed uses .zed/debug.json with tcp_connection (Task 07).
        ParentIde::Zed => run_generator(&zed::ZedGenerator, port, project_root),
        // These are already excluded by supports_dap_config() above,
        // but the compiler requires exhaustive coverage.
        ParentIde::IntelliJ | ParentIde::AndroidStudio => Ok(None),
    }
}

// ─────────────────────────────────────────────────────────────────
// IDE name parsing
// ─────────────────────────────────────────────────────────────────

/// Parse a CLI IDE name string to a [`crate::config::ParentIde`] variant.
///
/// Accepts common aliases in addition to canonical names (case-insensitive):
///
/// | Input | Result |
/// |-------|--------|
/// | `vscode`, `vs-code`, `code` | `ParentIde::VSCode` |
/// | `neovim`, `nvim` | `ParentIde::Neovim` |
/// | `helix`, `hx` | `ParentIde::Helix` |
/// | `zed` | `ParentIde::Zed` |
/// | `emacs` | `ParentIde::Emacs` |
///
/// Returns an error for unrecognised inputs.
pub fn parse_ide_name(name: &str) -> fdemon_core::Result<crate::config::ParentIde> {
    use crate::config::ParentIde;
    use fdemon_core::Error;

    match name.to_lowercase().as_str() {
        "vscode" | "vs-code" | "code" => Ok(ParentIde::VSCode),
        "neovim" | "nvim" => Ok(ParentIde::Neovim),
        "helix" | "hx" => Ok(ParentIde::Helix),
        "zed" => Ok(ParentIde::Zed),
        "emacs" => Ok(ParentIde::Emacs),
        _ => Err(Error::config(format!(
            "unknown IDE '{}'. Valid values: vscode, neovim, helix, zed, emacs",
            name
        ))),
    }
}

// ─────────────────────────────────────────────────────────────────
// Re-exports
// ─────────────────────────────────────────────────────────────────

pub(crate) use merge::{clean_jsonc, merge_json_array_entry, to_pretty_json};

// ─────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ParentIde;
    use merge::{clean_jsonc, find_json_entry_by_field};
    use serde_json::json;

    // ── parse_ide_name ───────────────────────────────────────────

    #[test]
    fn test_parse_ide_name_vscode_canonical() {
        assert_eq!(parse_ide_name("vscode").unwrap(), ParentIde::VSCode);
    }

    #[test]
    fn test_parse_ide_name_vscode_aliases() {
        assert_eq!(parse_ide_name("vs-code").unwrap(), ParentIde::VSCode);
        assert_eq!(parse_ide_name("code").unwrap(), ParentIde::VSCode);
    }

    #[test]
    fn test_parse_ide_name_vscode_case_insensitive() {
        assert_eq!(parse_ide_name("VSCODE").unwrap(), ParentIde::VSCode);
        assert_eq!(parse_ide_name("VsCode").unwrap(), ParentIde::VSCode);
    }

    #[test]
    fn test_parse_ide_name_neovim() {
        assert_eq!(parse_ide_name("neovim").unwrap(), ParentIde::Neovim);
        assert_eq!(parse_ide_name("nvim").unwrap(), ParentIde::Neovim);
    }

    #[test]
    fn test_parse_ide_name_helix() {
        assert_eq!(parse_ide_name("helix").unwrap(), ParentIde::Helix);
        assert_eq!(parse_ide_name("hx").unwrap(), ParentIde::Helix);
    }

    #[test]
    fn test_parse_ide_name_zed() {
        assert_eq!(parse_ide_name("zed").unwrap(), ParentIde::Zed);
    }

    #[test]
    fn test_parse_ide_name_emacs() {
        assert_eq!(parse_ide_name("emacs").unwrap(), ParentIde::Emacs);
    }

    #[test]
    fn test_parse_ide_name_invalid_returns_error() {
        assert!(parse_ide_name("sublime").is_err());
        assert!(parse_ide_name("").is_err());
        assert!(parse_ide_name("idea").is_err());
        assert!(parse_ide_name("android-studio").is_err());
    }

    #[test]
    fn test_parse_ide_name_error_message_lists_valid_values() {
        let err = parse_ide_name("sublime").unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("vscode"),
            "error should list valid values: {}",
            msg
        );
        assert!(
            msg.contains("neovim"),
            "error should list valid values: {}",
            msg
        );
    }

    #[test]
    fn test_standalone_config_generation_vscode() {
        use tempfile::tempdir;
        let dir = tempdir().unwrap();
        let result = generate_ide_config(Some(ParentIde::VSCode), 4711, dir.path()).unwrap();
        assert!(result.is_some());
        assert!(dir.path().join(".vscode/launch.json").exists());
    }

    // ── generate_ide_config — None / unsupported IDE paths ──────

    #[test]
    fn test_generate_ide_config_none_returns_none() {
        let result = generate_ide_config(None, 4711, Path::new("/tmp"));
        assert_eq!(result.unwrap(), None);
    }

    #[test]
    fn test_generate_ide_config_intellij_returns_none() {
        let result = generate_ide_config(Some(ParentIde::IntelliJ), 4711, Path::new("/tmp"));
        assert_eq!(result.unwrap(), None);
    }

    #[test]
    fn test_generate_ide_config_android_studio_returns_none() {
        let result = generate_ide_config(Some(ParentIde::AndroidStudio), 4711, Path::new("/tmp"));
        assert_eq!(result.unwrap(), None);
    }

    // ── ConfigAction — enum variants ────────────────────────────

    #[test]
    fn test_config_action_variants_are_eq() {
        assert_eq!(ConfigAction::Created, ConfigAction::Created);
        assert_eq!(ConfigAction::Updated, ConfigAction::Updated);
        assert_eq!(
            ConfigAction::Skipped("reason".to_string()),
            ConfigAction::Skipped("reason".to_string())
        );
        assert_ne!(ConfigAction::Created, ConfigAction::Updated);
    }

    // ── merge utilities (re-exported) ────────────────────────────

    #[test]
    fn test_find_json_entry_by_field_found_via_reexport() {
        let array = vec![
            json!({"name": "Dart", "type": "dart"}),
            json!({"name": "Flutter (fdemon)", "type": "dart"}),
        ];
        assert_eq!(
            find_json_entry_by_field(&array, "name", "Flutter (fdemon)"),
            Some(1)
        );
    }

    #[test]
    fn test_find_json_entry_by_field_not_found_via_reexport() {
        let array = vec![json!({"name": "Dart"})];
        assert_eq!(
            find_json_entry_by_field(&array, "name", "Flutter (fdemon)"),
            None
        );
    }

    #[test]
    fn test_merge_json_array_entry_replaces_existing_via_reexport() {
        let mut array = vec![
            json!({"name": "existing"}),
            json!({"name": "Flutter (fdemon)", "debugServer": 1234}),
        ];
        merge_json_array_entry(
            &mut array,
            "name",
            "Flutter (fdemon)",
            json!({"name": "Flutter (fdemon)", "debugServer": 5678}),
        );
        assert_eq!(array.len(), 2);
        assert_eq!(array[1]["debugServer"], 5678);
    }

    #[test]
    fn test_merge_json_array_entry_appends_new_via_reexport() {
        let mut array = vec![json!({"name": "existing"})];
        merge_json_array_entry(
            &mut array,
            "name",
            "Flutter (fdemon)",
            json!({"name": "Flutter (fdemon)"}),
        );
        assert_eq!(array.len(), 2);
    }

    #[test]
    fn test_clean_jsonc_strips_line_comments_via_reexport() {
        assert_eq!(
            clean_jsonc("{\n  // comment\n  \"key\": 1\n}"),
            "{\n  \n  \"key\": 1\n}"
        );
    }

    #[test]
    fn test_clean_jsonc_strips_trailing_commas_via_reexport() {
        let input = r#"{"items": [1, 2,]}"#;
        let cleaned = clean_jsonc(input);
        let parsed: serde_json::Value = serde_json::from_str(&cleaned).unwrap();
        assert!(parsed.is_object());
    }

    // ── run_generator: content comparison / skip behaviour ──────

    /// Verifies the create → skip → update sequence for run_generator.
    ///
    /// 1. First call: file does not exist → `ConfigAction::Created` and file written.
    /// 2. Second call with identical port: content unchanged → `ConfigAction::Skipped`.
    /// 3. Third call with different port: content changed → `ConfigAction::Updated`.
    #[test]
    fn test_run_generator_create_skip_update_sequence() {
        use tempfile::tempdir;

        let dir = tempdir().unwrap();

        // First run: file does not exist → Created.
        let result1 = run_generator(&vscode::VSCodeGenerator, 12345, dir.path())
            .unwrap()
            .unwrap();
        assert!(
            matches!(result1.action, ConfigAction::Created),
            "expected Created, got {:?}",
            result1.action
        );
        assert!(
            result1.path.exists(),
            "config file should have been written"
        );

        // Second run with same port: content is identical → Skipped.
        let result2 = run_generator(&vscode::VSCodeGenerator, 12345, dir.path())
            .unwrap()
            .unwrap();
        assert!(
            matches!(result2.action, ConfigAction::Skipped(ref reason) if reason == "content unchanged"),
            "expected Skipped(\"content unchanged\"), got {:?}",
            result2.action
        );

        // Third run with a different port: content changes → Updated.
        let result3 = run_generator(&vscode::VSCodeGenerator, 54321, dir.path())
            .unwrap()
            .unwrap();
        assert!(
            matches!(result3.action, ConfigAction::Updated),
            "expected Updated, got {:?}",
            result3.action
        );
    }

    /// Verifies that when content is unchanged the file modification time is not
    /// updated (i.e. no write occurs on skip).
    #[test]
    fn test_run_generator_skip_does_not_modify_file() {
        use std::time::Duration;
        use tempfile::tempdir;

        let dir = tempdir().unwrap();

        // Create the file.
        run_generator(&vscode::VSCodeGenerator, 12345, dir.path())
            .unwrap()
            .unwrap();

        let config_path = vscode::VSCodeGenerator.config_path(dir.path());
        let mtime_before = std::fs::metadata(&config_path).unwrap().modified().unwrap();

        // Give the clock a small window so that a spurious write would be detectable.
        std::thread::sleep(Duration::from_millis(10));

        // Second run with identical port: should skip.
        let result = run_generator(&vscode::VSCodeGenerator, 12345, dir.path())
            .unwrap()
            .unwrap();
        assert!(matches!(result.action, ConfigAction::Skipped(_)));

        let mtime_after = std::fs::metadata(&config_path).unwrap().modified().unwrap();

        assert_eq!(
            mtime_before, mtime_after,
            "file mtime should not change when content is unchanged"
        );
    }
}
