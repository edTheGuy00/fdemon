//! Emacs DAP config generator for `dap-mode`.
//!
//! Generates a `.fdemon/dap-emacs.el` Elisp snippet that registers fdemon as
//! a `dap-mode` debug provider. Because the target file lives inside
//! fdemon's own `.fdemon/` directory, the file is always overwritten rather
//! than merged — there is no user-managed content to preserve.
//!
//! ## Usage
//!
//! After generation the user must manually load the file:
//!
//! ```text
//! M-x load-file RET /path/to/.fdemon/dap-emacs.el RET
//! ```
//!
//! Or add the following to their Emacs init:
//!
//! ```elisp
//! (load-file "/path/to/.fdemon/dap-emacs.el")
//! ```

use std::path::{Path, PathBuf};

use fdemon_core::Result;

use super::IdeConfigGenerator;

// ─────────────────────────────────────────────────────────────────
// EmacsGenerator
// ─────────────────────────────────────────────────────────────────

/// Generates an Elisp snippet for Emacs `dap-mode` integration.
///
/// The output file `.fdemon/dap-emacs.el` is always overwritten because it
/// lives entirely within fdemon's own `.fdemon/` directory — no user content
/// needs to be preserved.
pub struct EmacsGenerator;

impl IdeConfigGenerator for EmacsGenerator {
    /// Returns `.fdemon/dap-emacs.el` relative to `project_root`.
    fn config_path(&self, project_root: &Path) -> PathBuf {
        project_root.join(".fdemon").join("dap-emacs.el")
    }

    /// Generate the full Elisp file content, embedding the absolute path of
    /// the generated file in the loading instructions.
    fn generate(&self, port: u16, project_root: &Path) -> Result<String> {
        let path = self.config_path(project_root);
        Ok(generate_elisp(port, path.display().to_string()))
    }

    /// Regenerate the file from scratch (overwrite semantics).
    ///
    /// Because `.fdemon/dap-emacs.el` is fdemon-owned, the existing content is
    /// always discarded. The absolute path embedded in the loading instructions
    /// is derived from `project_root` so the user can copy-paste a working path.
    fn merge_config(&self, _existing: &str, port: u16, project_root: &Path) -> Result<String> {
        let path = self.config_path(project_root);
        Ok(generate_elisp(port, path.display().to_string()))
    }

    /// Display name used in log messages.
    fn ide_name(&self) -> &'static str {
        "Emacs"
    }
}

// ─────────────────────────────────────────────────────────────────
// Elisp generation
// ─────────────────────────────────────────────────────────────────

/// Produce the full Elisp file content.
///
/// `file_path_display` is embedded verbatim in the loading instructions so
/// the user can copy-paste the correct path.
fn generate_elisp(port: u16, file_path_display: String) -> String {
    format!(
        r#";; fdemon DAP configuration for Emacs dap-mode (auto-generated)
;;
;; Load this file to register fdemon as a DAP provider:
;;
;;   M-x load-file RET {file_path} RET
;;
;; Or add to your Emacs config:
;;
;;   (load-file "{file_path}")

(require 'dap-mode)

(dap-register-debug-provider
  "fdemon"
  (lambda (conf)
    (plist-put conf :debugServer {port})
    (plist-put conf :host "localhost")
    conf))

(dap-register-debug-template
  "Flutter :: fdemon"
  (list :type "fdemon"
        :request "attach"
        :name "Flutter (fdemon DAP)"))
"#,
        file_path = file_path_display,
        port = port,
    )
}

// ─────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_emacs_config_path() {
        let gen = EmacsGenerator;
        assert_eq!(
            gen.config_path(Path::new("/project")),
            PathBuf::from("/project/.fdemon/dap-emacs.el")
        );
    }

    #[test]
    fn test_emacs_fresh_generation() {
        let gen = EmacsGenerator;
        let content = gen.generate(4711, Path::new("/project")).unwrap();
        assert!(content.contains("dap-register-debug-provider"));
        assert!(content.contains(":debugServer 4711"));
        assert!(content.contains("dap-register-debug-template"));
        assert!(content.contains(":request \"attach\""));
        assert!(content.contains("require 'dap-mode"));
    }

    #[test]
    fn test_emacs_port_substitution() {
        let gen = EmacsGenerator;
        let content = gen.generate(9999, Path::new("/project")).unwrap();
        assert!(content.contains(":debugServer 9999"));
        assert!(!content.contains(":debugServer 4711"));
    }

    #[test]
    fn test_emacs_merge_overwrites() {
        let gen = EmacsGenerator;
        let old_content = ";; old content";
        let new_content = gen
            .merge_config(old_content, 5678, Path::new("/project"))
            .unwrap();
        assert!(new_content.contains(":debugServer 5678"));
        assert!(!new_content.contains("old content"));
    }

    #[test]
    fn test_emacs_includes_loading_instructions() {
        let gen = EmacsGenerator;
        let content = gen.generate(4711, Path::new("/project")).unwrap();
        assert!(content.contains("load-file"));
        assert!(content.contains("M-x"));
    }

    #[test]
    fn test_emacs_ide_name() {
        assert_eq!(EmacsGenerator.ide_name(), "Emacs");
    }

    #[test]
    fn test_emacs_elisp_parens_balanced() {
        let gen = EmacsGenerator;
        let content = gen.generate(4711, Path::new("/project")).unwrap();
        // Simple paren balance check (ignoring strings/comments)
        let open = content.chars().filter(|c| *c == '(').count();
        let close = content.chars().filter(|c| *c == ')').count();
        assert_eq!(open, close, "Unbalanced parentheses in generated Elisp");
    }

    #[test]
    fn test_emacs_generate_embeds_absolute_path() {
        let gen = EmacsGenerator;
        let content = gen.generate(4711, Path::new("/my/flutter/app")).unwrap();
        assert!(content.contains("/my/flutter/app/.fdemon/dap-emacs.el"));
    }

    #[test]
    fn test_emacs_merge_uses_absolute_path() {
        let gen = EmacsGenerator;
        let content = gen
            .merge_config("", 4711, Path::new("/my/flutter/app"))
            .unwrap();
        // merge_config now uses the absolute path derived from project_root
        assert!(content.contains("/my/flutter/app/.fdemon/dap-emacs.el"));
    }

    #[test]
    fn test_emacs_merge_produces_absolute_path() {
        use tempfile::tempdir;
        let dir = tempdir().unwrap();
        let gen = EmacsGenerator;
        let existing = "(some old elisp)";
        let result = gen.merge_config(existing, 12345, dir.path()).unwrap();
        let expected_path = dir.path().join(".fdemon/dap-emacs.el");
        assert!(
            result.contains(&expected_path.display().to_string()),
            "expected absolute path '{}' in merged output",
            expected_path.display()
        );
        // Ensure the old relative placeholder is gone
        assert!(
            !result.contains("\".fdemon/dap-emacs.el\""),
            "merged output must not contain the relative placeholder"
        );
    }

    #[test]
    fn test_emacs_config_exists_false_for_temp_path() {
        let gen = EmacsGenerator;
        // A path that definitely does not exist
        assert!(!gen.config_exists(Path::new("/nonexistent/path/for/testing")));
    }

    #[test]
    fn test_emacs_uses_debug_server_not_debug_port() {
        let gen = EmacsGenerator;
        let content = gen.generate(4711, Path::new("/project")).unwrap();
        assert!(content.contains(":debugServer 4711"));
        assert!(!content.contains(":debugPort"));
    }
}
