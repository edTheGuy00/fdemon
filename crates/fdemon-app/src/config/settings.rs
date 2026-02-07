//! Settings parser for .fdemon/config.toml

use super::types::{EditorSettings, ParentIde, Settings};
use fdemon_core::prelude::*;
use std::path::Path;
use std::process::Command;

const CONFIG_FILENAME: &str = "config.toml";
const FDEMON_DIR: &str = ".fdemon";

// ─────────────────────────────────────────────────────────────────────────────
// Editor Detection
// ─────────────────────────────────────────────────────────────────────────────

/// Known editor configuration with command and open pattern.
#[derive(Debug, Clone)]
pub struct EditorConfig {
    pub command: &'static str,
    pub pattern: &'static str,
    pub display_name: &'static str,
}

/// List of known editors with their file:line patterns.
///
/// Note: Patterns include --reuse-window where applicable for IDE instance reuse.
pub const KNOWN_EDITORS: &[EditorConfig] = &[
    EditorConfig {
        command: "code",
        pattern: "code --reuse-window --goto $FILE:$LINE:$COLUMN",
        display_name: "Visual Studio Code",
    },
    EditorConfig {
        command: "cursor",
        pattern: "cursor --reuse-window --goto $FILE:$LINE:$COLUMN",
        display_name: "Cursor",
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
];

/// Detect if running inside an IDE's integrated terminal.
///
/// This is crucial for opening files in the CURRENT IDE instance
/// rather than spawning a new window.
pub fn detect_parent_ide() -> Option<ParentIde> {
    use std::env;

    // Check TERM_PROGRAM first (most reliable)
    if let Ok(term_program) = env::var("TERM_PROGRAM") {
        match term_program.as_str() {
            "vscode" => return Some(ParentIde::VSCode),
            "vscode-insiders" => return Some(ParentIde::VSCodeInsiders),
            "cursor" => return Some(ParentIde::Cursor),
            "Zed" => return Some(ParentIde::Zed),
            _ => {}
        }
    }

    // Check for Zed's terminal marker
    if env::var("ZED_TERM").is_ok() {
        return Some(ParentIde::Zed);
    }

    // Check for VS Code's IPC hook (backup detection)
    if env::var("VSCODE_IPC_HOOK_CLI").is_ok() {
        // Could be VS Code or a fork - check more specifically
        if env::var("TERM_PROGRAM")
            .map(|v| v == "cursor")
            .unwrap_or(false)
        {
            return Some(ParentIde::Cursor);
        }
        return Some(ParentIde::VSCode);
    }

    // Check for JetBrains terminal
    if let Ok(terminal_emulator) = env::var("TERMINAL_EMULATOR") {
        if terminal_emulator.starts_with("JetBrains") {
            // Try to distinguish between IntelliJ and Android Studio
            if let Ok(idea_dir) = env::var("IDEA_INITIAL_DIRECTORY") {
                if idea_dir.contains("AndroidStudio") {
                    return Some(ParentIde::AndroidStudio);
                }
            }
            return Some(ParentIde::IntelliJ);
        }
    }

    // Check for Neovim's socket (running inside :terminal)
    if env::var("NVIM").is_ok() {
        return Some(ParentIde::Neovim);
    }

    None
}

/// Get the editor config for a detected parent IDE.
pub fn editor_config_for_ide(ide: ParentIde) -> EditorConfig {
    match ide {
        ParentIde::VSCode => EditorConfig {
            command: "code",
            pattern: "code --reuse-window --goto $FILE:$LINE:$COLUMN",
            display_name: "Visual Studio Code",
        },
        ParentIde::VSCodeInsiders => EditorConfig {
            command: "code-insiders",
            pattern: "code-insiders --reuse-window --goto $FILE:$LINE:$COLUMN",
            display_name: "VS Code Insiders",
        },
        ParentIde::Cursor => EditorConfig {
            command: "cursor",
            pattern: "cursor --reuse-window --goto $FILE:$LINE:$COLUMN",
            display_name: "Cursor",
        },
        ParentIde::Zed => EditorConfig {
            command: "zed",
            pattern: "zed $FILE:$LINE",
            display_name: "Zed",
        },
        ParentIde::IntelliJ => EditorConfig {
            command: "idea",
            pattern: "idea --line $LINE $FILE",
            display_name: "IntelliJ IDEA",
        },
        ParentIde::AndroidStudio => EditorConfig {
            command: "studio",
            pattern: "studio --line $LINE $FILE",
            display_name: "Android Studio",
        },
        ParentIde::Neovim => EditorConfig {
            command: "nvim",
            pattern: "nvim --server $NVIM --remote-send '<Esc>:e +$LINE $FILE<CR>'",
            display_name: "Neovim",
        },
    }
}

/// Detect the user's preferred editor.
///
/// Detection order:
/// 1. **Parent IDE** - If running in an IDE's terminal, use that IDE
/// 2. $VISUAL environment variable
/// 3. $EDITOR environment variable
/// 4. Check for common editors in PATH
pub fn detect_editor() -> Option<EditorConfig> {
    use std::env;

    // Priority 1: Parent IDE (most important for instance reuse)
    if let Some(ide) = detect_parent_ide() {
        return Some(editor_config_for_ide(ide));
    }

    // Check environment variables
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

/// Find editor config by command name.
pub fn find_editor_config(cmd: &str) -> Option<EditorConfig> {
    KNOWN_EDITORS
        .iter()
        .find(|e| cmd.contains(e.command))
        .map(|e| EditorConfig {
            command: e.command,
            pattern: e.pattern,
            display_name: e.display_name,
        })
}

/// Check if a command is available in PATH.
fn is_command_available(cmd: &str) -> bool {
    #[cfg(unix)]
    {
        Command::new("which")
            .arg(cmd)
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    #[cfg(windows)]
    {
        Command::new("where.exe")
            .arg(cmd)
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// EditorSettings Implementation
// ─────────────────────────────────────────────────────────────────────────────

/// Default open pattern for settings.
fn default_open_pattern() -> String {
    "$EDITOR $FILE:$LINE".to_string()
}

impl EditorSettings {
    /// Resolve the effective editor command and pattern.
    ///
    /// Uses configured values or falls back to auto-detection.
    ///
    /// Priority order:
    /// 1. Explicitly configured command (if set)
    /// 2. Parent IDE detection (if running in an IDE terminal)
    /// 3. $VISUAL / $EDITOR environment variables
    /// 4. Common editors in PATH
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

    /// Get the display name of the configured editor.
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

    /// Check if we detected a parent IDE.
    pub fn detected_parent_ide(&self) -> Option<ParentIde> {
        detect_parent_ide()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Settings Loading
// ─────────────────────────────────────────────────────────────────────────────

/// Load settings from .fdemon/config.toml
///
/// Returns default settings if file doesn't exist or can't be parsed.
pub fn load_settings(project_path: &Path) -> Settings {
    let config_path = project_path.join(FDEMON_DIR).join(CONFIG_FILENAME);

    if !config_path.exists() {
        debug!("No config file at {:?}, using defaults", config_path);
        return Settings::default();
    }

    match std::fs::read_to_string(&config_path) {
        Ok(content) => match toml::from_str(&content) {
            Ok(settings) => {
                debug!("Loaded settings from {:?}", config_path);
                settings
            }
            Err(e) => {
                warn!("Failed to parse {:?}: {}", config_path, e);
                Settings::default()
            }
        },
        Err(e) => {
            warn!("Failed to read {:?}: {}", config_path, e);
            Settings::default()
        }
    }
}

/// Create default config files in .fdemon/ directory
pub fn init_config_dir(project_path: &Path) -> Result<()> {
    let fdemon_dir = project_path.join(FDEMON_DIR);

    if !fdemon_dir.exists() {
        std::fs::create_dir_all(&fdemon_dir)
            .map_err(|e| Error::config(format!("Failed to create .fdemon dir: {}", e)))?;
    }

    let config_path = fdemon_dir.join(CONFIG_FILENAME);
    if !config_path.exists() {
        let default_content = r#"# Flutter Demon Configuration
# See: https://github.com/example/flutter-demon#configuration

[behavior]
auto_start = false      # Set to true to skip device selection
confirm_quit = true     # Ask before quitting with running apps

[watcher]
paths = ["lib"]
debounce_ms = 500
auto_reload = true
extensions = ["dart"]

[ui]
log_buffer_size = 10000
show_timestamps = true
compact_logs = false
theme = "default"

[devtools]
auto_open = false
browser = ""            # Empty = system default

[editor]
# Editor command (leave empty for auto-detection)
# Auto-detected from: parent IDE, $VISUAL, $EDITOR, or common editors in PATH
command = ""
# Pattern for opening file at line/column
# Variables: $EDITOR, $FILE, $LINE, $COLUMN
# Examples:
#   VS Code:  "code --reuse-window --goto $FILE:$LINE:$COLUMN"
#   Zed:      "zed $FILE:$LINE"
#   Neovim:   "nvim +$LINE $FILE"
open_pattern = "$EDITOR $FILE:$LINE"
"#;
        std::fs::write(&config_path, default_content)
            .map_err(|e| Error::config(format!("Failed to write config.toml: {}", e)))?;
    }

    Ok(())
}

/// Save settings to .fdemon/config.toml
///
/// Uses atomic write (temp file + rename) for safety.
/// Preserves file structure by regenerating with header comments.
pub fn save_settings(project_path: &Path, settings: &Settings) -> Result<()> {
    let fdemon_dir = project_path.join(FDEMON_DIR);

    // Ensure directory exists
    if !fdemon_dir.exists() {
        std::fs::create_dir_all(&fdemon_dir)
            .map_err(|e| Error::config(format!("Failed to create .fdemon dir: {}", e)))?;
    }

    let config_path = fdemon_dir.join(CONFIG_FILENAME);
    let temp_path = fdemon_dir.join(".config.toml.tmp");

    // Generate TOML content with header
    let header = generate_config_header();
    let content = toml::to_string_pretty(settings)
        .map_err(|e| Error::config(format!("Failed to serialize settings: {}", e)))?;

    let full_content = format!("{}{}", header, content);

    // Atomic write: write to temp, then rename
    std::fs::write(&temp_path, &full_content)
        .map_err(|e| Error::config(format!("Failed to write temp file: {}", e)))?;

    std::fs::rename(&temp_path, &config_path)
        .map_err(|e| Error::config(format!("Failed to rename temp file: {}", e)))?;

    info!("Saved settings to {:?}", config_path);
    Ok(())
}

fn generate_config_header() -> String {
    r#"# Flutter Demon Configuration
# See: https://github.com/example/flutter-demon#configuration
# Generated by fdemon settings panel

"#
    .to_string()
}

// ─────────────────────────────────────────────────────────────────────────────
// Local Settings (User Preferences)
// ─────────────────────────────────────────────────────────────────────────────

const LOCAL_SETTINGS_FILENAME: &str = "settings.local.toml";
const GITIGNORE_ENTRY: &str = ".fdemon/settings.local.toml";

// ─────────────────────────────────────────────────────────────────────────────
// Init Directory & Gitignore
// ─────────────────────────────────────────────────────────────────────────────

/// Initialize Flutter Demon configuration directory and files
///
/// Creates:
/// - `.fdemon/` directory if it doesn't exist
/// - `.fdemon/config.toml` with defaults if missing
/// - Adds `.fdemon/settings.local.toml` to `.gitignore` if not present
///
/// This function is idempotent and non-fatal - errors are logged but don't crash the app.
pub fn init_fdemon_directory(project_path: &Path) -> Result<()> {
    let fdemon_dir = project_path.join(FDEMON_DIR);

    // Create .fdemon directory
    if !fdemon_dir.exists() {
        std::fs::create_dir_all(&fdemon_dir)
            .map_err(|e| Error::config(format!("Failed to create .fdemon dir: {}", e)))?;
        info!("Created .fdemon directory");
    }

    // Create config.toml if missing
    let config_path = fdemon_dir.join(CONFIG_FILENAME);
    if !config_path.exists() {
        let default_content = generate_default_config();
        std::fs::write(&config_path, default_content)
            .map_err(|e| Error::config(format!("Failed to write config.toml: {}", e)))?;
        info!("Created default config.toml");
    }

    // Ensure settings.local.toml is in .gitignore
    ensure_gitignore_entry(project_path)?;

    Ok(())
}

/// Ensure `.fdemon/settings.local.toml` is in `.gitignore`
fn ensure_gitignore_entry(project_path: &Path) -> Result<()> {
    let gitignore_path = project_path.join(".gitignore");

    // Read existing content or start fresh
    let existing_content = std::fs::read_to_string(&gitignore_path).unwrap_or_default();

    // Check if entry already exists
    if gitignore_contains_entry(&existing_content, GITIGNORE_ENTRY) {
        debug!("Gitignore already contains {}", GITIGNORE_ENTRY);
        return Ok(());
    }

    // Append entry with comment
    let entry_to_add = format!(
        "\n# Flutter Demon user preferences (not tracked)\n{}\n",
        GITIGNORE_ENTRY
    );

    let new_content = if existing_content.is_empty() {
        entry_to_add.trim_start().to_string()
    } else if existing_content.ends_with('\n') {
        format!("{}{}", existing_content, entry_to_add)
    } else {
        format!("{}\n{}", existing_content, entry_to_add)
    };

    std::fs::write(&gitignore_path, new_content)
        .map_err(|e| Error::config(format!("Failed to update .gitignore: {}", e)))?;

    info!("Added {} to .gitignore", GITIGNORE_ENTRY);
    Ok(())
}

/// Check if gitignore already contains the entry (handles various formats)
fn gitignore_contains_entry(content: &str, entry: &str) -> bool {
    content.lines().any(|line| {
        let trimmed = line.trim();
        // Exact match or with trailing comment/spaces
        trimmed == entry
            || trimmed.starts_with(&format!("{} ", entry))
            || trimmed.starts_with(&format!("{}#", entry))
            // Also check for glob patterns that would match
            || trimmed == ".fdemon/"
            || trimmed == ".fdemon/*"
            || trimmed == ".fdemon/**"
    })
}

fn generate_default_config() -> String {
    r#"# Flutter Demon Configuration
# See: https://github.com/example/flutter-demon#configuration

[behavior]
auto_start = false      # Set to true to skip device selection
confirm_quit = true     # Ask before quitting with running apps

[watcher]
paths = ["lib"]
debounce_ms = 500
auto_reload = true
extensions = ["dart"]

[ui]
log_buffer_size = 10000
show_timestamps = true
compact_logs = false
theme = "default"
stack_trace_collapsed = true
stack_trace_max_frames = 3

[devtools]
auto_open = false
browser = ""            # Empty = system default

[editor]
# Editor command (leave empty for auto-detection)
# Auto-detected from: parent IDE, $VISUAL, $EDITOR, or common editors in PATH
command = ""
# Pattern for opening file at line/column
# Variables: $EDITOR, $FILE, $LINE, $COLUMN
open_pattern = "$EDITOR $FILE:$LINE"
"#
    .to_string()
}

/// Load user preferences from .fdemon/settings.local.toml
///
/// Returns None if file doesn't exist (not an error - first run)
pub fn load_user_preferences(project_path: &Path) -> Option<super::types::UserPreferences> {
    let prefs_path = project_path.join(FDEMON_DIR).join(LOCAL_SETTINGS_FILENAME);

    if !prefs_path.exists() {
        debug!("No local settings file at {:?}", prefs_path);
        return None;
    }

    match std::fs::read_to_string(&prefs_path) {
        Ok(content) => match toml::from_str(&content) {
            Ok(prefs) => {
                debug!("Loaded user preferences from {:?}", prefs_path);
                Some(prefs)
            }
            Err(e) => {
                warn!("Failed to parse {:?}: {}", prefs_path, e);
                None
            }
        },
        Err(e) => {
            warn!("Failed to read {:?}: {}", prefs_path, e);
            None
        }
    }
}

/// Save user preferences to .fdemon/settings.local.toml
///
/// Creates the file if it doesn't exist.
/// Uses atomic write (temp file + rename) for safety.
pub fn save_user_preferences(
    project_path: &Path,
    prefs: &super::types::UserPreferences,
) -> Result<()> {
    let fdemon_dir = project_path.join(FDEMON_DIR);

    // Ensure directory exists
    if !fdemon_dir.exists() {
        std::fs::create_dir_all(&fdemon_dir)
            .map_err(|e| Error::config(format!("Failed to create .fdemon dir: {}", e)))?;
    }

    let prefs_path = fdemon_dir.join(LOCAL_SETTINGS_FILENAME);
    let temp_path = fdemon_dir.join(".settings.local.toml.tmp");

    // Serialize to TOML with header comment
    let header = "# User-specific preferences (not tracked in git)\n\
                  # These override values from config.toml\n\n";

    let content = toml::to_string_pretty(prefs)
        .map_err(|e| Error::config(format!("Failed to serialize preferences: {}", e)))?;

    let full_content = format!("{}{}", header, content);

    // Atomic write: write to temp, then rename
    std::fs::write(&temp_path, full_content)
        .map_err(|e| Error::config(format!("Failed to write temp file: {}", e)))?;

    std::fs::rename(&temp_path, &prefs_path)
        .map_err(|e| Error::config(format!("Failed to rename temp file: {}", e)))?;

    debug!("Saved user preferences to {:?}", prefs_path);
    Ok(())
}

/// Merge user preferences into settings (user prefs override project settings)
pub fn merge_preferences(settings: &mut Settings, prefs: &super::types::UserPreferences) {
    // Override editor settings
    if let Some(ref editor) = prefs.editor {
        if !editor.command.is_empty() {
            settings.editor.command = editor.command.clone();
        }
        if editor.open_pattern != "$EDITOR $FILE:$LINE" {
            settings.editor.open_pattern = editor.open_pattern.clone();
        }
    }

    // Override theme
    if let Some(ref theme) = prefs.theme {
        settings.ui.theme = theme.clone();
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Last Selection Auto-save
// ─────────────────────────────────────────────────────────────────────────────

/// Result of loading last selection
#[derive(Debug, Clone)]
pub struct LastSelection {
    pub config_name: Option<String>,
    pub device_id: Option<String>,
}

/// Validated selection with indices
#[derive(Debug, Clone)]
pub struct ValidatedSelection {
    pub config_idx: Option<usize>,
    pub device_idx: Option<usize>,
}

/// Load the user's last selection from settings.local.toml
///
/// Returns None if file doesn't exist or fields are not set.
pub fn load_last_selection(project_path: &Path) -> Option<LastSelection> {
    let prefs = load_user_preferences(project_path)?;

    // Only return if at least one field is set
    if prefs.last_config.is_none() && prefs.last_device.is_none() {
        return None;
    }

    Some(LastSelection {
        config_name: prefs.last_config,
        device_id: prefs.last_device,
    })
}

/// Save the user's selection to settings.local.toml
///
/// Preserves other preferences in the file.
pub fn save_last_selection(
    project_path: &Path,
    config_name: Option<&str>,
    device_id: Option<&str>,
) -> Result<()> {
    // Load existing preferences or create new
    let mut prefs = load_user_preferences(project_path).unwrap_or_default();

    // Update selection fields
    prefs.last_config = config_name.map(|s| s.to_string());
    prefs.last_device = device_id.map(|s| s.to_string());

    // Save back
    save_user_preferences(project_path, &prefs)
}

/// Clear the last selection (e.g., when user explicitly cancels)
pub fn clear_last_selection(project_path: &Path) -> Result<()> {
    if let Some(mut prefs) = load_user_preferences(project_path) {
        prefs.last_config = None;
        prefs.last_device = None;
        save_user_preferences(project_path, &prefs)
    } else {
        Ok(()) // Nothing to clear
    }
}

/// Check if last selection matches available configs and devices
///
/// Returns validated selection with indices, or None if not found.
pub fn validate_last_selection(
    selection: &LastSelection,
    configs: &super::priority::LoadedConfigs,
    devices: &[fdemon_daemon::Device],
) -> Option<ValidatedSelection> {
    let config_idx = selection
        .config_name
        .as_ref()
        .and_then(|name| configs.configs.iter().position(|c| c.config.name == *name));

    let device_idx = selection
        .device_id
        .as_ref()
        .and_then(|id| devices.iter().position(|d| d.id == *id));

    // Return only if device is valid (config is optional)
    if device_idx.is_some() {
        Some(ValidatedSelection {
            config_idx,
            device_idx,
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_load_settings_defaults() {
        let temp = tempdir().unwrap();
        let settings = load_settings(temp.path());

        assert!(!settings.behavior.auto_start);
        assert!(settings.behavior.confirm_quit);
        assert_eq!(settings.watcher.debounce_ms, 500);
        assert!(settings.watcher.auto_reload);
    }

    #[test]
    fn test_load_settings_custom() {
        let temp = tempdir().unwrap();
        let fdemon_dir = temp.path().join(".fdemon");
        std::fs::create_dir_all(&fdemon_dir).unwrap();

        let config = r#"
[behavior]
auto_start = true

[watcher]
debounce_ms = 1000
auto_reload = false
"#;
        std::fs::write(fdemon_dir.join("config.toml"), config).unwrap();

        let settings = load_settings(temp.path());

        assert!(settings.behavior.auto_start);
        assert_eq!(settings.watcher.debounce_ms, 1000);
        assert!(!settings.watcher.auto_reload);
    }

    #[test]
    fn test_load_settings_invalid_toml() {
        let temp = tempdir().unwrap();
        let fdemon_dir = temp.path().join(".fdemon");
        std::fs::create_dir_all(&fdemon_dir).unwrap();

        // Invalid TOML
        std::fs::write(fdemon_dir.join("config.toml"), "not valid toml {{{{").unwrap();

        // Should return defaults
        let settings = load_settings(temp.path());
        assert!(!settings.behavior.auto_start);
    }

    #[test]
    fn test_init_config_dir() {
        let temp = tempdir().unwrap();

        init_config_dir(temp.path()).unwrap();

        assert!(temp.path().join(".fdemon").exists());
        assert!(temp.path().join(".fdemon/config.toml").exists());

        // Content should be valid TOML
        let content = std::fs::read_to_string(temp.path().join(".fdemon/config.toml")).unwrap();
        let _: Settings = toml::from_str(&content).expect("Default config should be valid TOML");
    }

    #[test]
    fn test_init_config_dir_idempotent() {
        let temp = tempdir().unwrap();

        // First init
        init_config_dir(temp.path()).unwrap();

        // Modify the file
        let config_path = temp.path().join(".fdemon/config.toml");
        std::fs::write(&config_path, "[behavior]\nauto_start = true\n").unwrap();

        // Second init should not overwrite
        init_config_dir(temp.path()).unwrap();

        let content = std::fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("auto_start = true"));
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Editor Settings Tests
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_editor_settings_default() {
        let settings = EditorSettings::default();
        assert!(settings.command.is_empty());
        assert_eq!(settings.open_pattern, "$EDITOR $FILE:$LINE");
    }

    #[test]
    fn test_editor_settings_deserialize_partial() {
        use super::super::types::Settings;
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
        use super::super::types::Settings;
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
    fn test_find_editor_config_unknown() {
        let config = find_editor_config("unknown_editor");
        assert!(config.is_none());
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
    fn test_parent_ide_url_schemes() {
        assert_eq!(ParentIde::VSCode.url_scheme(), "vscode");
        assert_eq!(ParentIde::VSCodeInsiders.url_scheme(), "vscode-insiders");
        assert_eq!(ParentIde::Cursor.url_scheme(), "cursor");
        assert_eq!(ParentIde::Zed.url_scheme(), "zed");
        assert_eq!(ParentIde::IntelliJ.url_scheme(), "idea");
        assert_eq!(ParentIde::AndroidStudio.url_scheme(), "idea");
        assert_eq!(ParentIde::Neovim.url_scheme(), "file");
    }

    #[test]
    fn test_parent_ide_reuse_flags() {
        assert_eq!(ParentIde::VSCode.reuse_flag(), Some("--reuse-window"));
        assert_eq!(
            ParentIde::VSCodeInsiders.reuse_flag(),
            Some("--reuse-window")
        );
        assert_eq!(ParentIde::Cursor.reuse_flag(), Some("--reuse-window"));
        assert_eq!(ParentIde::Zed.reuse_flag(), None); // Zed reuses by default
        assert_eq!(ParentIde::IntelliJ.reuse_flag(), None);
        assert_eq!(ParentIde::Neovim.reuse_flag(), None);
    }

    #[test]
    fn test_parent_ide_display_names() {
        assert_eq!(ParentIde::VSCode.display_name(), "VS Code");
        assert_eq!(ParentIde::VSCodeInsiders.display_name(), "VS Code Insiders");
        assert_eq!(ParentIde::Cursor.display_name(), "Cursor");
        assert_eq!(ParentIde::Zed.display_name(), "Zed");
        assert_eq!(ParentIde::IntelliJ.display_name(), "IntelliJ IDEA");
        assert_eq!(ParentIde::AndroidStudio.display_name(), "Android Studio");
        assert_eq!(ParentIde::Neovim.display_name(), "Neovim");
    }

    #[test]
    fn test_editor_config_for_ide() {
        let config = editor_config_for_ide(ParentIde::VSCode);
        assert_eq!(config.command, "code");
        assert!(config.pattern.contains("--reuse-window"));

        let config = editor_config_for_ide(ParentIde::Zed);
        assert_eq!(config.command, "zed");
        assert!(!config.pattern.contains("--reuse-window")); // Zed reuses by default
    }

    #[test]
    fn test_editor_display_name_configured() {
        let mut settings = EditorSettings::default();
        settings.command = "code".to_string();
        assert_eq!(settings.editor_display_name(), "Visual Studio Code");
    }

    #[test]
    fn test_editor_display_name_unknown() {
        let mut settings = EditorSettings::default();
        settings.command = "my-custom-editor".to_string();
        // Falls back to command name
        assert_eq!(settings.editor_display_name(), "my-custom-editor");
    }

    #[test]
    fn test_editor_resolve_with_configured_command() {
        let mut settings = EditorSettings::default();
        settings.command = "code".to_string();

        let (cmd, pattern) = settings.resolve().unwrap();
        assert_eq!(cmd, "code");
        // Should get the editor-specific pattern since we're using default open_pattern
        assert!(pattern.contains("--goto"));
    }

    #[test]
    fn test_editor_resolve_with_custom_pattern() {
        let mut settings = EditorSettings::default();
        settings.command = "code".to_string();
        settings.open_pattern = "custom $FILE:$LINE".to_string();

        let (cmd, pattern) = settings.resolve().unwrap();
        assert_eq!(cmd, "code");
        // Should use custom pattern, not editor default
        assert_eq!(pattern, "custom $FILE:$LINE");
    }

    #[test]
    fn test_load_settings_with_editor() {
        let temp = tempdir().unwrap();
        let fdemon_dir = temp.path().join(".fdemon");
        std::fs::create_dir_all(&fdemon_dir).unwrap();

        let config = r#"
[editor]
command = "zed"
open_pattern = "zed $FILE:$LINE"
"#;
        std::fs::write(fdemon_dir.join("config.toml"), config).unwrap();

        let settings = load_settings(temp.path());

        assert_eq!(settings.editor.command, "zed");
        assert_eq!(settings.editor.open_pattern, "zed $FILE:$LINE");
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Local Settings (User Preferences) Tests
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_load_user_preferences_missing_file() {
        let temp = tempdir().unwrap();
        let prefs = load_user_preferences(temp.path());
        assert!(prefs.is_none());
    }

    #[test]
    fn test_save_and_load_user_preferences() {
        use super::super::types::{UserPreferences, WindowPrefs};

        let temp = tempdir().unwrap();
        std::fs::create_dir_all(temp.path().join(".fdemon")).unwrap();

        let prefs = UserPreferences {
            theme: Some("dark".to_string()),
            last_device: Some("iPhone 15".to_string()),
            last_config: Some("Development".to_string()),
            window: Some(WindowPrefs {
                width: Some(120),
                height: Some(40),
            }),
            ..Default::default()
        };

        save_user_preferences(temp.path(), &prefs).unwrap();

        let loaded = load_user_preferences(temp.path()).unwrap();
        assert_eq!(loaded.theme, Some("dark".to_string()));
        assert_eq!(loaded.last_device, Some("iPhone 15".to_string()));
        assert_eq!(loaded.last_config, Some("Development".to_string()));
        assert!(loaded.window.is_some());
        let window = loaded.window.unwrap();
        assert_eq!(window.width, Some(120));
        assert_eq!(window.height, Some(40));
    }

    #[test]
    fn test_save_creates_directory() {
        use super::super::types::UserPreferences;

        let temp = tempdir().unwrap();
        // Don't create .fdemon dir - save should create it

        let prefs = UserPreferences::default();
        save_user_preferences(temp.path(), &prefs).unwrap();

        assert!(temp.path().join(".fdemon/settings.local.toml").exists());
    }

    #[test]
    fn test_merge_preferences_overrides() {
        use super::super::types::UserPreferences;

        let mut settings = Settings::default();
        settings.editor.command = "code".to_string();
        settings.ui.theme = "default".to_string();

        let prefs = UserPreferences {
            editor: Some(EditorSettings {
                command: "nvim".to_string(),
                open_pattern: "nvim +$LINE $FILE".to_string(),
            }),
            theme: Some("monokai".to_string()),
            ..Default::default()
        };

        merge_preferences(&mut settings, &prefs);

        assert_eq!(settings.editor.command, "nvim");
        assert_eq!(settings.editor.open_pattern, "nvim +$LINE $FILE");
        assert_eq!(settings.ui.theme, "monokai");
    }

    #[test]
    fn test_merge_preferences_partial() {
        use super::super::types::UserPreferences;

        let mut settings = Settings::default();
        settings.editor.command = "code".to_string();
        settings.ui.theme = "default".to_string();

        // Only override theme, not editor
        let prefs = UserPreferences {
            editor: None,
            theme: Some("nord".to_string()),
            ..Default::default()
        };

        merge_preferences(&mut settings, &prefs);

        assert_eq!(settings.editor.command, "code"); // Unchanged
        assert_eq!(settings.ui.theme, "nord"); // Changed
    }

    #[test]
    fn test_local_settings_file_has_header() {
        use super::super::types::UserPreferences;

        let temp = tempdir().unwrap();
        std::fs::create_dir_all(temp.path().join(".fdemon")).unwrap();

        let prefs = UserPreferences::default();
        save_user_preferences(temp.path(), &prefs).unwrap();

        let content =
            std::fs::read_to_string(temp.path().join(".fdemon/settings.local.toml")).unwrap();

        assert!(content.contains("User-specific preferences"));
        assert!(content.contains("not tracked in git"));
    }

    #[test]
    fn test_merge_preferences_empty_editor_command() {
        use super::super::types::UserPreferences;

        let mut settings = Settings::default();
        settings.editor.command = "code".to_string();

        // Empty command should not override
        let prefs = UserPreferences {
            editor: Some(EditorSettings {
                command: "".to_string(),
                open_pattern: "custom $FILE:$LINE".to_string(),
            }),
            ..Default::default()
        };

        merge_preferences(&mut settings, &prefs);

        assert_eq!(settings.editor.command, "code"); // Not overridden
        assert_eq!(settings.editor.open_pattern, "custom $FILE:$LINE"); // Overridden
    }

    #[test]
    fn test_merge_preferences_default_pattern() {
        use super::super::types::UserPreferences;

        let mut settings = Settings::default();
        settings.editor.open_pattern = "code --goto $FILE:$LINE".to_string();

        // Default pattern should not override
        let prefs = UserPreferences {
            editor: Some(EditorSettings {
                command: "nvim".to_string(),
                open_pattern: "$EDITOR $FILE:$LINE".to_string(),
            }),
            ..Default::default()
        };

        merge_preferences(&mut settings, &prefs);

        assert_eq!(settings.editor.command, "nvim"); // Overridden
        assert_eq!(settings.editor.open_pattern, "code --goto $FILE:$LINE"); // Not overridden (default pattern)
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Settings Save Tests (Task 11)
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_save_settings_roundtrip() {
        let temp = tempdir().unwrap();

        let mut settings = Settings::default();
        settings.behavior.auto_start = true;
        settings.watcher.debounce_ms = 1000;
        settings.ui.theme = "dark".to_string();

        // Save
        save_settings(temp.path(), &settings).unwrap();

        // Load
        let loaded = load_settings(temp.path());

        assert!(loaded.behavior.auto_start);
        assert_eq!(loaded.watcher.debounce_ms, 1000);
        assert_eq!(loaded.ui.theme, "dark");
    }

    #[test]
    fn test_save_settings_creates_directory() {
        let temp = tempdir().unwrap();
        // Don't create .fdemon directory

        let settings = Settings::default();
        save_settings(temp.path(), &settings).unwrap();

        assert!(temp.path().join(".fdemon/config.toml").exists());
    }

    #[test]
    fn test_save_settings_atomic_write() {
        let temp = tempdir().unwrap();
        std::fs::create_dir_all(temp.path().join(".fdemon")).unwrap();

        // Write initial content
        let settings = Settings::default();
        save_settings(temp.path(), &settings).unwrap();

        // Verify no temp file left behind
        assert!(!temp.path().join(".fdemon/.config.toml.tmp").exists());
    }

    #[test]
    fn test_saved_settings_file_has_header() {
        let temp = tempdir().unwrap();
        let settings = Settings::default();

        save_settings(temp.path(), &settings).unwrap();

        let content = std::fs::read_to_string(temp.path().join(".fdemon/config.toml")).unwrap();

        assert!(content.contains("Flutter Demon Configuration"));
        assert!(content.contains("Generated by fdemon settings panel"));
        assert!(content.starts_with('#'));
    }

    #[test]
    fn test_save_settings_full_roundtrip() {
        let temp = tempdir().unwrap();

        let mut settings = Settings::default();
        settings.behavior.auto_start = true;
        settings.behavior.confirm_quit = false;
        settings.watcher.debounce_ms = 2000;
        settings.watcher.auto_reload = false;
        settings.watcher.paths = vec!["lib".into(), "test".into()];
        settings.watcher.extensions = vec!["dart".into(), "yaml".into()];
        settings.ui.log_buffer_size = 5000;
        settings.ui.show_timestamps = false;
        settings.ui.compact_logs = true;
        settings.ui.theme = "monokai".to_string();
        settings.ui.stack_trace_collapsed = false;
        settings.ui.stack_trace_max_frames = 5;
        settings.devtools.auto_open = true;
        settings.devtools.browser = "firefox".to_string();
        settings.editor.command = "nvim".to_string();
        settings.editor.open_pattern = "nvim +$LINE $FILE".to_string();

        save_settings(temp.path(), &settings).unwrap();
        let loaded = load_settings(temp.path());

        assert_eq!(loaded.behavior.auto_start, true);
        assert_eq!(loaded.behavior.confirm_quit, false);
        assert_eq!(loaded.watcher.debounce_ms, 2000);
        assert_eq!(loaded.watcher.auto_reload, false);
        assert_eq!(loaded.watcher.paths, vec!["lib", "test"]);
        assert_eq!(loaded.watcher.extensions, vec!["dart", "yaml"]);
        assert_eq!(loaded.ui.log_buffer_size, 5000);
        assert_eq!(loaded.ui.show_timestamps, false);
        assert_eq!(loaded.ui.compact_logs, true);
        assert_eq!(loaded.ui.theme, "monokai");
        assert_eq!(loaded.ui.stack_trace_collapsed, false);
        assert_eq!(loaded.ui.stack_trace_max_frames, 5);
        assert_eq!(loaded.devtools.auto_open, true);
        assert_eq!(loaded.devtools.browser, "firefox");
        assert_eq!(loaded.editor.command, "nvim");
        assert_eq!(loaded.editor.open_pattern, "nvim +$LINE $FILE");
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Init Directory & Gitignore Tests (Task 12)
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_init_creates_fdemon_dir() {
        let temp = tempdir().unwrap();

        init_fdemon_directory(temp.path()).unwrap();

        assert!(temp.path().join(".fdemon").exists());
        assert!(temp.path().join(".fdemon/config.toml").exists());
    }

    #[test]
    fn test_init_creates_gitignore_entry() {
        let temp = tempdir().unwrap();

        init_fdemon_directory(temp.path()).unwrap();

        let gitignore = std::fs::read_to_string(temp.path().join(".gitignore")).unwrap();

        assert!(gitignore.contains(".fdemon/settings.local.toml"));
        assert!(gitignore.contains("Flutter Demon"));
    }

    #[test]
    fn test_init_preserves_existing_gitignore() {
        let temp = tempdir().unwrap();

        // Create existing gitignore
        std::fs::write(temp.path().join(".gitignore"), "node_modules/\n.env\n").unwrap();

        init_fdemon_directory(temp.path()).unwrap();

        let gitignore = std::fs::read_to_string(temp.path().join(".gitignore")).unwrap();

        assert!(gitignore.contains("node_modules/"));
        assert!(gitignore.contains(".env"));
        assert!(gitignore.contains(".fdemon/settings.local.toml"));
    }

    #[test]
    fn test_init_no_duplicate_gitignore() {
        let temp = tempdir().unwrap();

        // Create existing gitignore with entry
        std::fs::write(
            temp.path().join(".gitignore"),
            ".fdemon/settings.local.toml\n",
        )
        .unwrap();

        init_fdemon_directory(temp.path()).unwrap();

        let gitignore = std::fs::read_to_string(temp.path().join(".gitignore")).unwrap();

        // Count occurrences
        let count = gitignore.matches(".fdemon/settings.local.toml").count();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_gitignore_contains_entry() {
        assert!(gitignore_contains_entry(
            ".fdemon/settings.local.toml\n",
            ".fdemon/settings.local.toml"
        ));

        // With trailing space
        assert!(gitignore_contains_entry(
            ".fdemon/settings.local.toml \n",
            ".fdemon/settings.local.toml"
        ));

        // With comment
        assert!(gitignore_contains_entry(
            ".fdemon/settings.local.toml # user prefs\n",
            ".fdemon/settings.local.toml"
        ));

        // Parent directory pattern
        assert!(gitignore_contains_entry(
            ".fdemon/\n",
            ".fdemon/settings.local.toml"
        ));

        // Wildcard patterns
        assert!(gitignore_contains_entry(
            ".fdemon/*\n",
            ".fdemon/settings.local.toml"
        ));

        assert!(gitignore_contains_entry(
            ".fdemon/**\n",
            ".fdemon/settings.local.toml"
        ));

        // Not present
        assert!(!gitignore_contains_entry(
            "node_modules/\n",
            ".fdemon/settings.local.toml"
        ));
    }

    #[test]
    fn test_init_idempotent() {
        let temp = tempdir().unwrap();

        // Run twice
        init_fdemon_directory(temp.path()).unwrap();
        init_fdemon_directory(temp.path()).unwrap();

        // Should still have only one entry
        let gitignore = std::fs::read_to_string(temp.path().join(".gitignore")).unwrap();

        let count = gitignore.matches(".fdemon/settings.local.toml").count();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_init_doesnt_overwrite_config() {
        let temp = tempdir().unwrap();
        std::fs::create_dir_all(temp.path().join(".fdemon")).unwrap();

        // Create custom config
        std::fs::write(
            temp.path().join(".fdemon/config.toml"),
            "[behavior]\nauto_start = true\n",
        )
        .unwrap();

        init_fdemon_directory(temp.path()).unwrap();

        // Should preserve custom config
        let content = std::fs::read_to_string(temp.path().join(".fdemon/config.toml")).unwrap();

        assert!(content.contains("auto_start = true"));
    }

    #[test]
    fn test_gitignore_handles_no_trailing_newline() {
        let temp = tempdir().unwrap();

        // Create gitignore without trailing newline
        std::fs::write(temp.path().join(".gitignore"), "node_modules/").unwrap();

        init_fdemon_directory(temp.path()).unwrap();

        let gitignore = std::fs::read_to_string(temp.path().join(".gitignore")).unwrap();

        assert!(gitignore.contains("node_modules/"));
        assert!(gitignore.contains(".fdemon/settings.local.toml"));
        // Verify proper formatting
        let lines: Vec<&str> = gitignore.lines().collect();
        assert!(lines.contains(&"node_modules/"));
        assert!(lines.contains(&".fdemon/settings.local.toml"));
    }

    #[test]
    fn test_generate_default_config_is_valid_toml() {
        let content = generate_default_config();
        let _: Settings =
            toml::from_str(&content).expect("Generated default config should be valid TOML");
    }

    #[test]
    fn test_gitignore_empty_file() {
        let temp = tempdir().unwrap();

        // Don't create .gitignore - let init create it
        init_fdemon_directory(temp.path()).unwrap();

        let gitignore = std::fs::read_to_string(temp.path().join(".gitignore")).unwrap();

        // Should start with comment, not newline
        assert!(gitignore.starts_with("# Flutter Demon"));
        assert!(gitignore.contains(".fdemon/settings.local.toml"));
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Last Selection Tests (Task 05)
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_load_last_selection_missing_file() {
        let temp = tempdir().unwrap();
        let result = load_last_selection(temp.path());
        assert!(result.is_none());
    }

    #[test]
    fn test_save_and_load_last_selection() {
        let temp = tempdir().unwrap();
        std::fs::create_dir_all(temp.path().join(".fdemon")).unwrap();

        save_last_selection(temp.path(), Some("Debug"), Some("iPhone-15")).unwrap();

        let selection = load_last_selection(temp.path()).unwrap();
        assert_eq!(selection.config_name, Some("Debug".to_string()));
        assert_eq!(selection.device_id, Some("iPhone-15".to_string()));
    }

    #[test]
    fn test_save_preserves_other_prefs() {
        use super::super::types::UserPreferences;

        let temp = tempdir().unwrap();
        std::fs::create_dir_all(temp.path().join(".fdemon")).unwrap();

        // Save initial prefs with theme
        let mut prefs = UserPreferences::default();
        prefs.theme = Some("dark".to_string());
        save_user_preferences(temp.path(), &prefs).unwrap();

        // Save selection
        save_last_selection(temp.path(), Some("Debug"), None).unwrap();

        // Verify theme preserved
        let loaded = load_user_preferences(temp.path()).unwrap();
        assert_eq!(loaded.theme, Some("dark".to_string()));
        assert_eq!(loaded.last_config, Some("Debug".to_string()));
    }

    #[test]
    fn test_clear_last_selection() {
        let temp = tempdir().unwrap();
        std::fs::create_dir_all(temp.path().join(".fdemon")).unwrap();

        // Save selection
        save_last_selection(temp.path(), Some("Debug"), Some("device-1")).unwrap();

        // Clear it
        clear_last_selection(temp.path()).unwrap();

        // Verify cleared
        let selection = load_last_selection(temp.path());
        assert!(selection.is_none());
    }

    #[test]
    fn test_validate_last_selection() {
        use super::super::priority::{LoadedConfigs, SourcedConfig};
        use super::super::types::{ConfigSource, LaunchConfig};
        use fdemon_daemon::Device;

        let selection = LastSelection {
            config_name: Some("Debug".to_string()),
            device_id: Some("iphone-15".to_string()),
        };

        let configs = LoadedConfigs {
            configs: vec![SourcedConfig {
                config: LaunchConfig {
                    name: "Debug".to_string(),
                    ..Default::default()
                },
                source: ConfigSource::FDemon,
                display_name: "Debug".to_string(),
            }],
            vscode_start_index: None,
            is_empty: false,
        };

        let devices = vec![Device {
            id: "iphone-15".to_string(),
            name: "iPhone 15".to_string(),
            platform: "ios".to_string(),
            emulator: false,
            emulator_id: None,
            ephemeral: false,
            category: None,
            platform_type: None,
        }];

        let validated = validate_last_selection(&selection, &configs, &devices).unwrap();
        assert_eq!(validated.config_idx, Some(0));
        assert_eq!(validated.device_idx, Some(0));
    }

    #[test]
    fn test_validate_requires_device() {
        use super::super::priority::LoadedConfigs;
        use fdemon_daemon::Device;

        let selection = LastSelection {
            config_name: Some("Debug".to_string()),
            device_id: Some("missing-device".to_string()),
        };

        let configs = LoadedConfigs::default();
        let devices: Vec<Device> = vec![];

        let validated = validate_last_selection(&selection, &configs, &devices);
        assert!(validated.is_none());
    }

    #[test]
    fn test_load_returns_none_if_fields_empty() {
        use super::super::types::UserPreferences;

        let temp = tempdir().unwrap();
        std::fs::create_dir_all(temp.path().join(".fdemon")).unwrap();

        // Save prefs without selection
        let prefs = UserPreferences::default();
        save_user_preferences(temp.path(), &prefs).unwrap();

        let selection = load_last_selection(temp.path());
        assert!(selection.is_none());
    }

    #[test]
    fn test_validate_selection_config_optional() {
        use super::super::priority::LoadedConfigs;
        use fdemon_daemon::Device;

        // Selection with device but no config
        let selection = LastSelection {
            config_name: None,
            device_id: Some("iphone-15".to_string()),
        };

        let configs = LoadedConfigs::default();
        let devices = vec![Device {
            id: "iphone-15".to_string(),
            name: "iPhone 15".to_string(),
            platform: "ios".to_string(),
            emulator: false,
            emulator_id: None,
            ephemeral: false,
            category: None,
            platform_type: None,
        }];

        let validated = validate_last_selection(&selection, &configs, &devices).unwrap();
        assert_eq!(validated.config_idx, None);
        assert_eq!(validated.device_idx, Some(0));
    }

    #[test]
    fn test_save_last_selection_creates_directory() {
        let temp = tempdir().unwrap();
        // Don't create .fdemon directory

        save_last_selection(temp.path(), Some("Debug"), Some("device-1")).unwrap();

        assert!(temp.path().join(".fdemon/settings.local.toml").exists());
    }

    #[test]
    fn test_clear_last_selection_no_file() {
        let temp = tempdir().unwrap();

        // Clearing when no file exists should succeed silently
        clear_last_selection(temp.path()).unwrap();
    }
}
