## Task: Open File in Editor Action

**Objective**: Implement the `o` key action to open the currently focused file reference in the user's configured editor at the correct line and column. When running inside an IDE's integrated terminal, open files in the **current IDE instance** rather than spawning a new window.

**Depends on**: 
- [02-editor-configuration](02-editor-configuration.md) - Editor settings, pattern resolution, and parent IDE detection
- [03-cursor-file-reference-tracking](03-cursor-file-reference-tracking.md) - Focused file reference tracking

### Prerequisites: Existing Infrastructure

The bug fix work and Phase 2 provide useful infrastructure:

| Component | Location | Purpose |
|-----------|----------|---------|
| `Session::focused_entry()` | `session.rs:757-760` | Get currently focused LogEntry |
| `LogViewState::focus_info` | `log_view.rs` (Task 03) | Extracted FileReference from focused entry |
| `LogViewState::focused_file_ref()` | `log_view.rs` (Task 03) | Convenience accessor for file reference |
| `detect_parent_ide()` | `config/settings.rs` (Task 02) | Detect if running in an IDE terminal |
| `editor_config_for_ide()` | `config/settings.rs` (Task 02) | Get editor config for detected IDE |
| `ParentIde` enum | `config/types.rs` (Task 02) | Represents detected parent IDE |

### Scope

- `src/app/message.rs`: Add `OpenFileAtCursor` message variant
- `src/app/handler/keys.rs`: Add `o` key handler
- `src/tui/editor.rs`: **NEW** - Editor command execution logic
- `src/tui/mod.rs`: Export editor module
- `src/tui/spawn.rs` or `src/app/handler/actions.rs`: Execute editor command

### Background

With file reference tracking (Task 03) and editor configuration (Task 02) in place, we can now implement the action that opens a file in the editor. When the user presses `o`:
1. **Detect parent IDE** - Check if running in VS Code, Cursor, Zed, IntelliJ, etc.
2. Get the currently focused file reference from `LogViewState::focus_info` (set during render)
3. Resolve the file path (convert package: to absolute path)
4. **Use IDE-specific command with reuse flags** if parent IDE detected
5. Otherwise, use configured editor pattern
6. Execute the editor command
7. Show feedback (success or error)

> **Note**: The focused file reference is stored in `LogViewState::focus_info.file_ref`, updated during each render pass by Task 03. This leverages the existing virtualization - only visible entries are processed.

### Instance Reuse Priority

When running inside an IDE's terminal, we want to open files in **that IDE instance**:

| Scenario | Behavior |
|----------|----------|
| Running in VS Code terminal | Use `code --reuse-window --goto file:line:col` |
| Running in Cursor terminal | Use `cursor --reuse-window --goto file:line:col` |
| Running in Zed terminal | Use `zed file:line` (reuses by default) |
| Running in IntelliJ terminal | Use `idea --line N file` |
| Running in plain terminal | Use configured editor or auto-detect |

This ensures a seamless experience where `o` opens the file exactly where the user expects it.

### Implementation Details

#### 1. Message Variant

```rust
// In src/app/message.rs

pub enum Message {
    // ... existing variants ...
    
    /// Open the currently focused file in the configured editor
    /// If running in an IDE terminal, opens in that IDE instance
    OpenFileAtCursor,
    
    /// Result of attempting to open a file
    OpenFileResult {
        success: bool,
        message: String,
        /// Which editor/IDE was used (for status display)
        editor_name: Option<String>,
    },
}
```

#### 2. Editor Execution Module

```rust
// src/tui/editor.rs

use crate::config::{EditorSettings, ParentIde, detect_parent_ide, editor_config_for_ide};
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
    pub editor_display_name: String,
    pub file: String,
    pub line: u32,
    /// Whether we used the parent IDE (running in its terminal)
    pub used_parent_ide: bool,
}

/// Open a file reference in the configured editor
/// 
/// Priority order:
/// 1. Parent IDE (if running in an IDE's integrated terminal)
/// 2. Configured editor from settings
/// 3. Auto-detected editor ($VISUAL, $EDITOR, PATH search)
pub fn open_in_editor(
    file_ref: &FileReference,
    settings: &EditorSettings,
    project_root: &Path,
) -> Result<OpenResult, EditorError> {
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
        // Get focused file reference from current session's LogViewState
        // This was set during render by Task 03's focus tracking
        let file_ref = if let Some(handle) = self.session_manager.selected_mut() {
            // focus_info is updated during each render pass
            handle.session.log_view_state.focus_info.file_ref.clone()
        } else {
            None
        };
        
        let Some(file_ref) = file_ref else {
            // No file reference at cursor
            tracing::debug!("No file reference at cursor position");
            return UpdateResult::default();
        };
        
        // Sanitize path
        if sanitize_path(&file_ref.path).is_none() {
            tracing::warn!("Rejected suspicious file path: {}", file_ref.path);
            return UpdateResult::default();
        }
        
        // Open in editor (parent IDE detection happens inside open_in_editor)
        match open_in_editor(&file_ref, &self.settings.editor, &self.project_path) {
            Ok(result) => {
                if result.used_parent_ide {
                    tracing::info!(
                        "Opened {}:{} in {} (current instance)",
                        result.file,
                        result.line,
                        result.editor_display_name
                    );
                } else {
                    tracing::info!(
                        "Opened {}:{} in {}",
                        result.file,
                        result.line,
                        result.editor_display_name
                    );
                }
                
                // Could show a brief status message in the UI
                // e.g., "Opened main.dart:42 in VS Code"
            }
            Err(e) => {
                tracing::warn!("Failed to open file: {}", e);
                // Could show error in status bar
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
            "code --reuse-window --goto $FILE:$LINE:$COLUMN",
            "code",
            Path::new("/path/to/file.dart"),
            42,
            10,
        );
        assert_eq!(result, "code --reuse-window --goto /path/to/file.dart:42:10");
    }
    
    #[test]
    fn test_open_result_tracks_parent_ide() {
        // When parent IDE is detected, used_parent_ide should be true
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
| Running in VS Code terminal | Opens in current VS Code instance |
| Running in Zed terminal | Opens in current Zed instance |
| Running in IntelliJ terminal | Opens in current IntelliJ instance |

### Security Considerations

1. **Path Traversal**: Reject paths containing `..`
2. **Shell Injection**: Reject paths with shell metacharacters
3. **Null Bytes**: Reject paths containing `\0`
4. **Command Injection**: Use `Command::new()` with separate args, not shell execution
5. **Symlink Following**: Consider resolving symlinks and checking final destination

### Notes

- The editor is spawned as a child process but we don't wait for it (non-blocking)
- **Parent IDE detection is checked first** - if running in VS Code terminal, opens in that VS Code instance
- `--reuse-window` flag is used for VS Code/Cursor to ensure instance reuse
- Zed and JetBrains IDEs reuse by default without special flags
- Neovim uses RPC (`--server $NVIM --remote-send`) when running inside `:terminal`
- Consider adding a config option to specify whether to wait for editor to close
- Future enhancement: Show editor name in status bar when file is opened
- **Existing Infrastructure**: `Session::focused_entry()` provides basic focus tracking; Task 03 extends this with file reference extraction in `LogViewState::focus_info`
- **VecDeque Compatibility**: Log storage is `VecDeque<LogEntry>` (from bug fix) but this doesn't affect file opening logic

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

---

## Completion Summary

**Status:** ✅ Done

**Date Completed:** 2026-01-05

### Files Modified

| File | Changes |
|------|---------|
| `src/app/message.rs` | Added `Message::OpenFileAtCursor` variant |
| `src/app/handler/keys.rs` | Added `o` key handler in `handle_key_normal()` |
| `src/tui/editor.rs` | **NEW** - Complete editor execution module with `open_in_editor()`, `resolve_file_path()`, `substitute_pattern()`, `sanitize_path()` |
| `src/tui/mod.rs` | Added `editor` module export and re-exports for `open_in_editor`, `EditorError`, `OpenResult` |
| `src/app/handler/update.rs` | Added handler for `Message::OpenFileAtCursor` |

### Implementation Details

1. **Editor Module (`src/tui/editor.rs`)**:
   - `EditorError` enum for error handling (NoEditor, FileNotFound, ExecutionFailed, PathResolutionFailed, PathRejected)
   - `OpenResult` struct to capture operation results including parent IDE detection
   - `open_in_editor()` - main API that checks parent IDE first, then configured editor
   - `resolve_file_path()` - handles package:, dart:, absolute, and relative paths
   - `substitute_pattern()` - replaces $EDITOR, $FILE, $LINE, $COLUMN variables
   - `execute_command()` - spawns editor without blocking (non-blocking)
   - `sanitize_path()` - security check for path traversal and shell injection

2. **Message Variant**:
   - Added `Message::OpenFileAtCursor` to message.rs

3. **Key Handler**:
   - `o` key in Normal mode triggers `Message::OpenFileAtCursor`

4. **Update Handler**:
   - Gets focused file reference from `LogViewState::focus_info.file_ref`
   - Sanitizes path before opening
   - Calls `open_in_editor()` with editor settings and project path
   - Logs success/failure with editor display name

### Testing Performed

- `cargo check` - Compiles successfully (1 warning for unused import from Task 03, expected)
- `cargo test --lib tui::editor` - 17 tests passed
- `cargo test --lib` - 933 tests passed, 3 ignored

### Notable Decisions/Tradeoffs

1. **Security First**: Path sanitization rejects paths with `..`, shell metacharacters, and null bytes
2. **Parent IDE Priority**: When running in an IDE terminal (VS Code, Cursor, Zed, IntelliJ), opens in that IDE instance first
3. **Non-blocking**: Editor is spawned without waiting, so TUI continues to run
4. **Graceful Degradation**: If no editor configured/detected, logs warning and takes no action (no user-visible error)
5. **Focus Info Integration**: Relies on Task 03's `FocusInfo` struct - file reference is populated during render

### Acceptance Criteria Checklist

- [x] `Message::OpenFileAtCursor` variant added
- [x] `o` key triggers `OpenFileAtCursor` in Normal mode
- [x] `editor.rs` module implements `open_in_editor()`
- [x] Pattern substitution handles $EDITOR, $FILE, $LINE, $COLUMN
- [x] Package: URI resolution works (maps to lib/)
- [x] Absolute paths handled correctly
- [x] Relative paths resolved against project root
- [x] File existence checked before opening
- [x] Path sanitization prevents security issues
- [x] Editor command spawns without blocking TUI
- [x] Errors logged appropriately
- [x] dart: URIs rejected gracefully (can't open SDK files)

### Integration with Task 03

This task uses infrastructure from Task 03:
- `LogViewState::focus_info.file_ref` - the currently focused file reference
- File reference is set during render pass by focus tracking logic

**Note**: The focus_info.file_ref will be populated when the render implementation updates it. Currently, pressing `o` will log "No file reference at cursor position" until render sets the focus info.

### Next Steps

For full functionality:
1. The render pass needs to update `LogViewState::focus_info.file_ref` based on the visible/focused entry
2. Task 05/06 (experimental) would add clickable OSC 8 hyperlinks