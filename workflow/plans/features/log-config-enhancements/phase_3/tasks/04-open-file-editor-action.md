## Task: Open File in Editor Action

**Objective**: Implement the `o` key action to open the currently focused file reference in the user's configured editor at the correct line and column.

**Depends on**: 
- [02-editor-configuration](02-editor-configuration.md) - Editor settings and pattern resolution
- [03-cursor-file-reference-tracking](03-cursor-file-reference-tracking.md) - Focused file reference tracking

### Scope

- `src/app/message.rs`: Add `OpenFileAtCursor` message variant
- `src/app/handler/keys.rs`: Add `o` key handler
- `src/tui/editor.rs`: **NEW** - Editor command execution logic
- `src/tui/mod.rs`: Export editor module
- `src/tui/spawn.rs` or `src/app/handler/actions.rs`: Execute editor command

### Background

With file reference tracking (Task 03) and editor configuration (Task 02) in place, we can now implement the action that opens a file in the editor. When the user presses `o`:
1. Get the currently focused file reference from log view state
2. Resolve the file path (convert package: to absolute path)
3. Substitute variables in the editor pattern
4. Execute the editor command
5. Show feedback (success or error)

### Implementation Details

#### 1. Message Variant

```rust
// In src/app/message.rs

pub enum Message {
    // ... existing variants ...
    
    /// Open the currently focused file in the configured editor
    OpenFileAtCursor,
    
    /// Result of attempting to open a file
    OpenFileResult {
        success: bool,
        message: String,
    },
}
```

#### 2. Editor Execution Module

```rust
// src/tui/editor.rs

use crate::config::EditorSettings;
use crate::tui::hyperlinks::FileReference;
use std::path::{Path, PathBuf};
use std::process::Command;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EditorError {
    #[error("No editor configured or detected")]
    NoEditor,
    
    #[error("File not found: {0}")]
    FileNotFound(PathBuf),
    
    #[error("Failed to execute editor: {0}")]
    ExecutionFailed(#[from] std::io::Error),
    
    #[error("Could not resolve file path: {0}")]
    PathResolutionFailed(String),
}

/// Result of opening a file in an editor
pub struct OpenResult {
    pub success: bool,
    pub editor: String,
    pub file: String,
    pub line: u32,
}

/// Open a file reference in the configured editor
pub fn open_in_editor(
    file_ref: &FileReference,
    settings: &EditorSettings,
    project_root: &Path,
) -> Result<OpenResult, EditorError> {
    // 1. Resolve editor command and pattern
    let (editor, pattern) = settings
        .resolve()
        .ok_or(EditorError::NoEditor)?;
    
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
        file: resolved_path.display().to_string(),
        line: file_ref.line,
    })
}

/// Resolve a file path, handling package: URIs
fn resolve_file_path(path: &str, project_root: &Path) -> Result<PathBuf, EditorError> {
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
        return Err(EditorError::PathResolutionFailed(
            format!("Cannot open SDK file: {}", path)
        ));
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

/// Substitute pattern variables with actual values
fn substitute_pattern(
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

/// Execute the editor command
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

/// Sanitize file path for security
/// Prevents path traversal and other injection attacks
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
```

#### 3. Key Handler

```rust
// In src/app/handler/keys.rs

use crate::app::Message;
use crossterm::event::KeyCode;

impl AppState {
    pub fn handle_key(&mut self, key: KeyEvent) -> Option<Message> {
        match key.code {
            // ... existing handlers ...
            
            // Open file at cursor in editor
            KeyCode::Char('o') => {
                // Only in Normal mode, not when searching or in dialogs
                if matches!(self.ui_mode, UiMode::Normal) {
                    return Some(Message::OpenFileAtCursor);
                }
                None
            }
            
            // ... rest of handlers ...
        }
    }
}
```

#### 4. Message Handler

```rust
// In src/app/handler/actions.rs or src/app/update.rs

impl AppState {
    fn handle_message(&mut self, msg: Message) -> UpdateResult {
        match msg {
            // ... existing handlers ...
            
            Message::OpenFileAtCursor => {
                self.open_file_at_cursor()
            }
            
            Message::OpenFileResult { success, message } => {
                // Could show a status message or notification
                if !success {
                    tracing::warn!("Failed to open file: {}", message);
                }
                UpdateResult::default()
            }
            
            // ... rest of handlers ...
        }
    }
    
    fn open_file_at_cursor(&mut self) -> UpdateResult {
        // Get focused file reference from current session
        let file_ref = if let Some(handle) = self.session_manager.selected() {
            handle.session.log_view_state.focused_file_ref().cloned()
        } else {
            None
        };
        
        let Some(file_ref) = file_ref else {
            // No file reference at cursor
            return UpdateResult::default();
        };
        
        // Sanitize path
        if sanitize_path(&file_ref.path).is_none() {
            tracing::warn!("Rejected suspicious file path: {}", file_ref.path);
            return UpdateResult::default();
        }
        
        // Open in editor
        match open_in_editor(&file_ref, &self.settings.editor, &self.project_path) {
            Ok(result) => {
                tracing::info!(
                    "Opened {}:{} in {}",
                    result.file,
                    result.line,
                    result.editor
                );
            }
            Err(e) => {
                tracing::warn!("Failed to open file: {}", e);
            }
        }
        
        UpdateResult::default()
    }
}
```

#### 5. Update tui/mod.rs

```rust
// Add to src/tui/mod.rs

pub mod editor;

pub use editor::{open_in_editor, EditorError, OpenResult};
```

### Status Message Feedback (Optional Enhancement)

Show a brief status message when opening files:

```rust
// In status bar or as a transient notification

// On success:
"Opened main.dart:42 in VS Code"

// On error:
"Could not open file: No editor configured"
"File not found: lib/missing.dart"
```

### Acceptance Criteria

1. [ ] `Message::OpenFileAtCursor` variant added
2. [ ] `o` key triggers `OpenFileAtCursor` in Normal mode
3. [ ] `editor.rs` module implements `open_in_editor()`
4. [ ] Pattern substitution handles $EDITOR, $FILE, $LINE, $COLUMN
5. [ ] Package: URI resolution works (maps to lib/)
6. [ ] Absolute paths handled correctly
7. [ ] Relative paths resolved against project root
8. [ ] File existence checked before opening
9. [ ] Path sanitization prevents security issues
10. [ ] Editor command spawns without blocking TUI
11. [ ] Errors logged appropriately
12. [ ] dart: URIs rejected gracefully (can't open SDK files)

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    #[test]
    fn test_substitute_pattern_vscode() {
        let result = substitute_pattern(
            "code --goto $FILE:$LINE:$COLUMN",
            "code",
            Path::new("/path/to/file.dart"),
            42,
            10,
        );
        assert_eq!(result, "code --goto /path/to/file.dart:42:10");
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
    fn test_resolve_file_path_package() {
        let temp_dir = TempDir::new().unwrap();
        let lib_dir = temp_dir.path().join("lib").join("src");
        fs::create_dir_all(&lib_dir).unwrap();
        let file_path = lib_dir.join("utils.dart");
        fs::write(&file_path, "// test").unwrap();
        
        let resolved = resolve_file_path(
            "package:my_app/src/utils.dart",
            temp_dir.path(),
        ).unwrap();
        
        assert_eq!(resolved, file_path);
    }

    #[test]
    fn test_resolve_file_path_absolute() {
        let result = resolve_file_path(
            "/absolute/path/file.dart",
            Path::new("/project"),
        ).unwrap();
        
        assert_eq!(result, PathBuf::from("/absolute/path/file.dart"));
    }

    #[test]
    fn test_resolve_file_path_relative() {
        let result = resolve_file_path(
            "lib/main.dart",
            Path::new("/project"),
        ).unwrap();
        
        assert_eq!(result, PathBuf::from("/project/lib/main.dart"));
    }

    #[test]
    fn test_resolve_file_path_dart_uri_fails() {
        let result = resolve_file_path(
            "dart:core/list.dart",
            Path::new("/project"),
        );
        
        assert!(result.is_err());
    }

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
}
```

### Manual Testing

1. Configure editor in `.fdemon/config.toml`:
   ```toml
   [editor]
   command = "code"
   open_pattern = "code --goto $FILE:$LINE:$COLUMN"
   ```

2. Run Flutter Demon with sample app
3. Trigger an error with a stack trace
4. Scroll to the stack trace
5. Press `o` key
6. Verify editor opens at correct file and line

### Edge Cases

| Scenario | Expected Behavior |
|----------|-------------------|
| No file reference at cursor | No action (silent) |
| Editor not configured/detected | Log warning, no action |
| File doesn't exist | Log warning with path |
| Package file outside lib/ | Try alternative paths |
| dart: SDK file | Log "cannot open SDK file" |
| Path with spaces | Correctly quoted/escaped |
| Very long file paths | Handle without truncation |
| Non-existent editor command | Log spawn error |

### Security Considerations

1. **Path Traversal**: Reject paths containing `..`
2. **Shell Injection**: Reject paths with shell metacharacters
3. **Null Bytes**: Reject paths containing `\0`
4. **Command Injection**: Use `Command::new()` with separate args, not shell execution
5. **Symlink Following**: Consider resolving symlinks and checking final destination

### Notes

- The editor is spawned as a child process but we don't wait for it (non-blocking)
- Some editors (VS Code) may reuse existing window, others open new window
- Consider adding a config option to specify whether to wait for editor to close
- Future enhancement: Show editor name in status bar when file is opened

### Estimated Time

3-4 hours

### Files Modified

| File | Changes |
|------|---------|
| `src/app/message.rs` | Add `OpenFileAtCursor` message variant |
| `src/app/handler/keys.rs` | Add `o` key handler |
| `src/tui/editor.rs` | **NEW** - Complete editor execution module |
| `src/tui/mod.rs` | Export editor module |
| `src/app/handler/actions.rs` or `update.rs` | Handle `OpenFileAtCursor` message |