## Task: Editor Configuration Settings

**Objective**: Add configuration settings for editor integration, allowing users to specify their preferred editor and customize the command pattern used to open files at specific lines.

**Depends on**: None

### Scope

- `src/config/types.rs`: Add `EditorSettings` struct with command and pattern fields
- `src/config/settings.rs`: Add default editor detection logic

### Background

To open files from stack traces in the user's editor, we need configurable settings:
1. Which editor to use (VS Code, Zed, Neovim, etc.)
2. The command pattern for opening at a specific line/column

Different editors have different command-line syntax:
- VS Code: `code --goto file:line:column`
- Zed: `zed file:line`
- Neovim: `nvim +line file`
- Vim: `vim +line file`
- Emacs: `emacs +line:column file`
- Sublime Text: `subl file:line:column`
- JetBrains IDEs: `idea --line line file`

### Implementation Details

#### 1. EditorSettings Struct

```rust
// In src/config/types.rs

/// Editor integration settings
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EditorSettings {
    /// Editor command or name (e.g., "code", "zed", "nvim")
    /// If empty, attempts auto-detection
    #[serde(default)]
    pub command: String,

    /// Pattern for opening file at line/column
    /// Variables: $EDITOR, $FILE, $LINE, $COLUMN
    /// Example: "$EDITOR --goto $FILE:$LINE:$COLUMN"
    #[serde(default = "default_open_pattern")]
    pub open_pattern: String,
}

impl Default for EditorSettings {
    fn default() -> Self {
        Self {
            command: String::new(), // Auto-detect
            open_pattern: default_open_pattern(),
        }
    }
}

fn default_open_pattern() -> String {
    "$EDITOR $FILE:$LINE".to_string()
}
```

#### 2. Add to Settings Struct

```rust
// In src/config/types.rs

/// Application settings (.fdemon/config.toml)
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Settings {
    #[serde(default)]
    pub behavior: BehaviorSettings,

    #[serde(default)]
    pub watcher: WatcherSettings,

    #[serde(default)]
    pub ui: UiSettings,

    #[serde(default)]
    pub devtools: DevToolsSettings,

    #[serde(default)]
    pub editor: EditorSettings,  // NEW
}
```

#### 3. Editor Detection Logic

```rust
// In src/config/settings.rs

use std::env;
use std::process::Command;

/// Known editor configurations with their open patterns
pub struct EditorConfig {
    pub command: &'static str,
    pub pattern: &'static str,
    pub display_name: &'static str,
}

/// List of known editors with their file:line patterns
pub const KNOWN_EDITORS: &[EditorConfig] = &[
    EditorConfig {
        command: "code",
        pattern: "code --goto $FILE:$LINE:$COLUMN",
        display_name: "Visual Studio Code",
    },
    EditorConfig {
        command: "zed",
        pattern: "zed $FILE:$LINE",
        display_name: "Zed",
    },
    EditorConfig {
        command: "nvim",
        pattern: "nvim +$LINE $FILE",
        display_name: "Neovim",
    },
    EditorConfig {
        command: "vim",
        pattern: "vim +$LINE $FILE",
        display_name: "Vim",
    },
    EditorConfig {
        command: "emacs",
        pattern: "emacs +$LINE:$COLUMN $FILE",
        display_name: "Emacs",
    },
    EditorConfig {
        command: "subl",
        pattern: "subl $FILE:$LINE:$COLUMN",
        display_name: "Sublime Text",
    },
    EditorConfig {
        command: "idea",
        pattern: "idea --line $LINE $FILE",
        display_name: "IntelliJ IDEA",
    },
    EditorConfig {
        command: "cursor",
        pattern: "cursor --goto $FILE:$LINE:$COLUMN",
        display_name: "Cursor",
    },
];

/// Detect the user's preferred editor
/// 
/// Detection order:
/// 1. $VISUAL environment variable
/// 2. $EDITOR environment variable  
/// 3. Check for common editors in PATH
pub fn detect_editor() -> Option<EditorConfig> {
    // Check environment variables first
    for var in ["VISUAL", "EDITOR"] {
        if let Ok(editor) = env::var(var) {
            // Extract command name from path
            let cmd = std::path::Path::new(&editor)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&editor);
            
            // Look for matching known editor
            if let Some(config) = find_editor_config(cmd) {
                return Some(config);
            }
        }
    }
    
    // Check for common editors in PATH
    for config in KNOWN_EDITORS {
        if is_command_available(config.command) {
            return Some(EditorConfig {
                command: config.command,
                pattern: config.pattern,
                display_name: config.display_name,
            });
        }
    }
    
    None
}

/// Find editor config by command name
fn find_editor_config(cmd: &str) -> Option<EditorConfig> {
    KNOWN_EDITORS.iter()
        .find(|e| cmd.contains(e.command))
        .map(|e| EditorConfig {
            command: e.command,
            pattern: e.pattern,
            display_name: e.display_name,
        })
}

/// Check if a command is available in PATH
fn is_command_available(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}
```

#### 4. Resolve Editor Settings

```rust
// In src/config/settings.rs

impl EditorSettings {
    /// Resolve the effective editor command and pattern
    /// Uses configured values or falls back to auto-detection
    pub fn resolve(&self) -> Option<(String, String)> {
        let command = if self.command.is_empty() {
            detect_editor().map(|e| e.command.to_string())?
        } else {
            self.command.clone()
        };
        
        let pattern = if self.open_pattern == default_open_pattern() {
            // If using default pattern, check for editor-specific pattern
            find_editor_config(&command)
                .map(|e| e.pattern.to_string())
                .unwrap_or_else(|| self.open_pattern.clone())
        } else {
            self.open_pattern.clone()
        };
        
        Some((command, pattern))
    }
    
    /// Get the display name of the configured editor
    pub fn editor_display_name(&self) -> String {
        if self.command.is_empty() {
            detect_editor()
                .map(|e| e.display_name.to_string())
                .unwrap_or_else(|| "None detected".to_string())
        } else {
            find_editor_config(&self.command)
                .map(|e| e.display_name.to_string())
                .unwrap_or_else(|| self.command.clone())
        }
    }
}
```

### Config File Example

```toml
# .fdemon/config.toml

[editor]
# Editor command (leave empty for auto-detection)
command = "zed"

# Pattern for opening file at line
# Variables: $EDITOR, $FILE, $LINE, $COLUMN
open_pattern = "zed $FILE:$LINE"
```

### Acceptance Criteria

1. `EditorSettings` struct added with `command` and `open_pattern` fields
2. Settings struct includes `editor: EditorSettings`
3. Default editor detection works for VS Code, Zed, Neovim, Vim
4. Detection respects $VISUAL and $EDITOR environment variables
5. `resolve()` method returns effective command and pattern
6. Config file parsing handles [editor] section
7. Deserialization works with partial config (missing fields use defaults)
8. All new code has unit tests

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_editor_settings_default() {
        let settings = EditorSettings::default();
        assert!(settings.command.is_empty());
        assert_eq!(settings.open_pattern, "$EDITOR $FILE:$LINE");
    }

    #[test]
    fn test_editor_settings_deserialize_partial() {
        let toml = r#"
[editor]
command = "code"
"#;
        let settings: Settings = toml::from_str(toml).unwrap();
        assert_eq!(settings.editor.command, "code");
        assert_eq!(settings.editor.open_pattern, "$EDITOR $FILE:$LINE");
    }

    #[test]
    fn test_editor_settings_deserialize_full() {
        let toml = r#"
[editor]
command = "nvim"
open_pattern = "nvim +$LINE $FILE"
"#;
        let settings: Settings = toml::from_str(toml).unwrap();
        assert_eq!(settings.editor.command, "nvim");
        assert_eq!(settings.editor.open_pattern, "nvim +$LINE $FILE");
    }

    #[test]
    fn test_find_editor_config_exact() {
        let config = find_editor_config("code").unwrap();
        assert_eq!(config.command, "code");
        assert!(config.pattern.contains("--goto"));
    }

    #[test]
    fn test_find_editor_config_partial() {
        // Should match "nvim" from "/usr/local/bin/nvim"
        let config = find_editor_config("/usr/local/bin/nvim").unwrap();
        assert_eq!(config.command, "nvim");
    }

    #[test]
    fn test_known_editors_patterns() {
        // Verify all patterns contain required variables
        for editor in KNOWN_EDITORS {
            assert!(
                editor.pattern.contains("$FILE"),
                "{} pattern missing $FILE",
                editor.command
            );
            assert!(
                editor.pattern.contains("$LINE"),
                "{} pattern missing $LINE",
                editor.command
            );
        }
    }

    #[test]
    fn test_editor_display_name() {
        let mut settings = EditorSettings::default();
        settings.command = "code".to_string();
        assert_eq!(settings.editor_display_name(), "Visual Studio Code");
    }
}
```

### Notes

- The `is_command_available` function uses `which` on Unix and would need modification for Windows (`where.exe`)
- Editor detection happens at runtime, not at config load time
- Caching detection results could improve performance if called frequently
- The pattern substitution is implemented in Task 04 (Open File Action)