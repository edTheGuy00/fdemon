//! Editor command execution for opening files at specific locations.
//!
//! This module handles opening files in the user's configured editor at the
//! correct line and column. When running inside an IDE's integrated terminal,
//! it opens files in the current IDE instance rather than spawning a new window.

use crate::config::{detect_parent_ide, editor_config_for_ide, EditorSettings};
use crate::hyperlinks::FileReference;
use std::path::{Path, PathBuf};
use std::process::Command;
use thiserror::Error;

// ─────────────────────────────────────────────────────────────────────────────
// Error Types
// ─────────────────────────────────────────────────────────────────────────────

/// Errors that can occur when opening a file in an editor.
#[derive(Debug, Error)]
pub enum EditorError {
    /// No editor configured or detected
    #[error("No editor configured or detected")]
    NoEditor,

    /// File not found at the specified path
    #[error("File not found: {0}")]
    FileNotFound(PathBuf),

    /// Failed to execute the editor command
    #[error("Failed to execute editor: {0}")]
    ExecutionFailed(#[from] std::io::Error),

    /// Could not resolve the file path (e.g., invalid package: URI)
    #[error("Could not resolve file path: {0}")]
    PathResolutionFailed(String),

    /// Path contains suspicious patterns (security check failed)
    #[error("Path rejected for security: {0}")]
    PathRejected(String),
}

// ─────────────────────────────────────────────────────────────────────────────
// Result Types
// ─────────────────────────────────────────────────────────────────────────────

/// Result of opening a file in an editor.
#[derive(Debug)]
pub struct OpenResult {
    /// Whether the operation succeeded
    pub success: bool,
    /// Editor command that was used
    pub editor: String,
    /// Display name of the editor (e.g., "Visual Studio Code")
    pub editor_display_name: String,
    /// File path that was opened
    pub file: String,
    /// Line number
    pub line: u32,
    /// Whether we used the parent IDE (running in its terminal)
    pub used_parent_ide: bool,
}

// ─────────────────────────────────────────────────────────────────────────────
// Main API
// ─────────────────────────────────────────────────────────────────────────────

/// Open a file reference in the configured editor.
///
/// Priority order:
/// 1. Parent IDE (if running in an IDE's integrated terminal)
/// 2. Configured editor from settings
/// 3. Auto-detected editor ($VISUAL, $EDITOR, PATH search)
///
/// # Arguments
///
/// * `file_ref` - The file reference containing path, line, and column
/// * `settings` - Editor settings from configuration
/// * `project_root` - Project root path for resolving relative paths
///
/// # Returns
///
/// `Ok(OpenResult)` on success, `Err(EditorError)` on failure.
pub fn open_in_editor(
    file_ref: &FileReference,
    settings: &EditorSettings,
    project_root: &Path,
) -> Result<OpenResult, EditorError> {
    // 0. Sanitize path first (security check)
    if sanitize_path(&file_ref.path).is_none() {
        return Err(EditorError::PathRejected(file_ref.path.clone()));
    }

    // 1. Check for parent IDE first (instance reuse)
    let (editor, pattern, display_name, used_parent_ide) = if let Some(ide) = detect_parent_ide() {
        let config = editor_config_for_ide(ide);
        (
            config.command.to_string(),
            config.pattern.to_string(),
            config.display_name.to_string(),
            true,
        )
    } else {
        // Fall back to configured/detected editor
        let (cmd, pat) = settings.resolve().ok_or(EditorError::NoEditor)?;
        let name = settings.editor_display_name();
        (cmd, pat, name, false)
    };

    // 2. Resolve file path
    let resolved_path = resolve_file_path(&file_ref.path, project_root)?;

    // 3. Verify file exists
    if !resolved_path.exists() {
        return Err(EditorError::FileNotFound(resolved_path));
    }

    // 4. Substitute pattern variables
    let command_line = substitute_pattern(
        &pattern,
        &editor,
        &resolved_path,
        file_ref.line,
        file_ref.column,
    );

    // 5. Execute command
    execute_command(&command_line)?;

    Ok(OpenResult {
        success: true,
        editor,
        editor_display_name: display_name,
        file: resolved_path.display().to_string(),
        line: file_ref.line,
        used_parent_ide,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Path Resolution
// ─────────────────────────────────────────────────────────────────────────────

/// Resolve a file path, handling package: URIs.
///
/// # Path Types
///
/// - `package:app/src/main.dart` - Resolved to `lib/src/main.dart` under project root
/// - `dart:core/list.dart` - SDK files, cannot be opened (returns error)
/// - `/absolute/path/file.dart` - Returned as-is
/// - `lib/main.dart` - Resolved relative to project root
pub fn resolve_file_path(path: &str, project_root: &Path) -> Result<PathBuf, EditorError> {
    // Handle package: URIs
    if path.starts_with("package:") {
        let package_path = path.strip_prefix("package:").unwrap();

        // Extract package name and relative path
        // e.g., "my_app/src/main.dart" -> package="my_app", rel="src/main.dart"
        if let Some((_package, rel_path)) = package_path.split_once('/') {
            // Map to lib/ directory (common convention)
            let resolved = project_root.join("lib").join(rel_path);
            if resolved.exists() {
                return Ok(resolved);
            }

            // Try without lib/ prefix (might be in project root)
            let alt_resolved = project_root.join(rel_path);
            if alt_resolved.exists() {
                return Ok(alt_resolved);
            }

            // Return the lib/ path even if it doesn't exist (let the error handling catch it)
            return Ok(resolved);
        }

        return Err(EditorError::PathResolutionFailed(path.to_string()));
    }

    // Handle dart: URIs (SDK files - generally not openable)
    if path.starts_with("dart:") {
        return Err(EditorError::PathResolutionFailed(format!(
            "Cannot open SDK file: {}",
            path
        )));
    }

    // Handle relative paths
    let path_buf = PathBuf::from(path);
    if path_buf.is_absolute() {
        Ok(path_buf)
    } else {
        // Relative to project root
        Ok(project_root.join(path))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Pattern Substitution
// ─────────────────────────────────────────────────────────────────────────────

/// Substitute pattern variables with actual values.
///
/// # Variables
///
/// - `$EDITOR` - Editor command
/// - `$FILE` - File path
/// - `$LINE` - Line number
/// - `$COLUMN` - Column number (minimum 1)
pub fn substitute_pattern(
    pattern: &str,
    editor: &str,
    file_path: &Path,
    line: u32,
    column: u32,
) -> String {
    let file_str = file_path.display().to_string();

    pattern
        .replace("$EDITOR", editor)
        .replace("$FILE", &file_str)
        .replace("$LINE", &line.to_string())
        .replace("$COLUMN", &column.max(1).to_string())
}

// ─────────────────────────────────────────────────────────────────────────────
// Command Execution
// ─────────────────────────────────────────────────────────────────────────────

/// Execute the editor command.
///
/// The command is spawned as a child process but we don't wait for it
/// (non-blocking). This allows the TUI to continue running while the
/// editor opens in the background.
fn execute_command(command_line: &str) -> Result<(), EditorError> {
    // Split command line into command and args
    let parts: Vec<&str> = command_line.split_whitespace().collect();

    if parts.is_empty() {
        return Err(EditorError::NoEditor);
    }

    let (cmd, args) = parts.split_first().unwrap();

    // Execute command without waiting (editor runs in background)
    Command::new(cmd)
        .args(args)
        .spawn()
        .map_err(EditorError::ExecutionFailed)?;

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Security
// ─────────────────────────────────────────────────────────────────────────────

/// Sanitize file path for security.
///
/// Prevents path traversal and other injection attacks by rejecting paths with:
/// - `..` (path traversal)
/// - Null bytes
/// - Shell metacharacters
///
/// # Returns
///
/// `Some(path)` if the path is safe, `None` if it contains suspicious patterns.
pub fn sanitize_path(path: &str) -> Option<String> {
    // Reject paths with suspicious patterns
    if path.contains("..") || path.contains('\0') {
        return None;
    }

    // Reject shell metacharacters
    let dangerous_chars = ['|', '&', ';', '$', '`', '(', ')', '{', '}', '<', '>'];
    if path.chars().any(|c| dangerous_chars.contains(&c)) {
        return None;
    }

    Some(path.to_string())
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    // ─────────────────────────────────────────────────────────────────────────
    // Pattern Substitution Tests
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_substitute_pattern_vscode() {
        let result = substitute_pattern(
            "code --reuse-window --goto $FILE:$LINE:$COLUMN",
            "code",
            Path::new("/path/to/file.dart"),
            42,
            10,
        );
        assert_eq!(
            result,
            "code --reuse-window --goto /path/to/file.dart:42:10"
        );
    }

    #[test]
    fn test_substitute_pattern_nvim() {
        let result = substitute_pattern(
            "nvim +$LINE $FILE",
            "nvim",
            Path::new("/path/to/file.dart"),
            42,
            0,
        );
        assert_eq!(result, "nvim +42 /path/to/file.dart");
    }

    #[test]
    fn test_substitute_pattern_zero_column() {
        let result = substitute_pattern(
            "$EDITOR $FILE:$LINE:$COLUMN",
            "code",
            Path::new("file.dart"),
            10,
            0, // Zero column should become 1
        );
        assert!(result.contains(":10:1"));
    }

    #[test]
    fn test_substitute_pattern_with_editor_var() {
        let result = substitute_pattern(
            "$EDITOR --goto $FILE:$LINE",
            "zed",
            Path::new("/file.dart"),
            15,
            1,
        );
        assert_eq!(result, "zed --goto /file.dart:15");
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Path Resolution Tests
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_resolve_file_path_package() {
        let temp_dir = TempDir::new().unwrap();
        let lib_dir = temp_dir.path().join("lib").join("src");
        fs::create_dir_all(&lib_dir).unwrap();
        let file_path = lib_dir.join("utils.dart");
        fs::write(&file_path, "// test").unwrap();

        let resolved = resolve_file_path("package:my_app/src/utils.dart", temp_dir.path()).unwrap();

        assert_eq!(resolved, file_path);
    }

    #[test]
    fn test_resolve_file_path_package_fallback() {
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().join("src");
        fs::create_dir_all(&src_dir).unwrap();
        let file_path = src_dir.join("utils.dart");
        fs::write(&file_path, "// test").unwrap();

        // File exists at project_root/src/utils.dart, not lib/src/utils.dart
        let resolved = resolve_file_path("package:my_app/src/utils.dart", temp_dir.path()).unwrap();

        assert_eq!(resolved, file_path);
    }

    #[test]
    fn test_resolve_file_path_absolute() {
        let result = resolve_file_path("/absolute/path/file.dart", Path::new("/project")).unwrap();

        assert_eq!(result, PathBuf::from("/absolute/path/file.dart"));
    }

    #[test]
    fn test_resolve_file_path_relative() {
        let result = resolve_file_path("lib/main.dart", Path::new("/project")).unwrap();

        assert_eq!(result, PathBuf::from("/project/lib/main.dart"));
    }

    #[test]
    fn test_resolve_file_path_dart_uri_fails() {
        let result = resolve_file_path("dart:core/list.dart", Path::new("/project"));

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, EditorError::PathResolutionFailed(_)));
    }

    #[test]
    fn test_resolve_file_path_invalid_package_uri() {
        let result = resolve_file_path("package:", Path::new("/project"));

        assert!(result.is_err());
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Sanitize Path Tests
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_sanitize_path_valid() {
        assert!(sanitize_path("/path/to/file.dart").is_some());
        assert!(sanitize_path("lib/main.dart").is_some());
        assert!(sanitize_path("package:app/main.dart").is_some());
    }

    #[test]
    fn test_sanitize_path_traversal() {
        assert!(sanitize_path("../../../etc/passwd").is_none());
        assert!(sanitize_path("/path/../secret").is_none());
    }

    #[test]
    fn test_sanitize_path_shell_injection() {
        assert!(sanitize_path("file.dart; rm -rf /").is_none());
        assert!(sanitize_path("file.dart | cat /etc/passwd").is_none());
        assert!(sanitize_path("$(whoami).dart").is_none());
        assert!(sanitize_path("`id`.dart").is_none());
    }

    #[test]
    fn test_sanitize_path_null_byte() {
        assert!(sanitize_path("file\0.dart").is_none());
    }

    #[test]
    fn test_sanitize_path_braces() {
        assert!(sanitize_path("file{1,2}.dart").is_none());
        assert!(sanitize_path("file$(cmd).dart").is_none());
    }

    // ─────────────────────────────────────────────────────────────────────────
    // OpenResult Tests
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_open_result_tracks_parent_ide() {
        let result = OpenResult {
            success: true,
            editor: "code".to_string(),
            editor_display_name: "Visual Studio Code".to_string(),
            file: "/path/to/file.dart".to_string(),
            line: 42,
            used_parent_ide: true,
        };
        assert!(result.used_parent_ide);
    }

    #[test]
    fn test_open_result_no_parent_ide() {
        let result = OpenResult {
            success: true,
            editor: "nvim".to_string(),
            editor_display_name: "Neovim".to_string(),
            file: "/path/to/file.dart".to_string(),
            line: 10,
            used_parent_ide: false,
        };
        assert!(!result.used_parent_ide);
    }
}
