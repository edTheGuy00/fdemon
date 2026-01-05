## Task: Local Settings File Structure

**Objective**: Implement the local settings file (`settings.local.toml`) for user-specific preferences that should not be tracked in git.

**Depends on**: None (can be done in parallel with Task 01)

**Estimated Time**: 1-1.5 hours

### Scope

- `src/config/settings.rs`: Add loading/saving for local settings
- `src/config/mod.rs`: Export new types

### Details

#### 1. File Location & Format

```
.fdemon/
├── config.toml          # Shared project settings (existing)
├── launch.toml          # Shared launch configs (existing)
└── settings.local.toml  # NEW: User-specific preferences
```

Example `settings.local.toml`:

```toml
# User-specific preferences (not tracked in git)
# These override values from config.toml

[editor]
# Override project editor with personal preference
command = "nvim"
open_pattern = "nvim +$LINE $FILE"

# UI preferences
theme = "dark"

# Last used values (auto-populated)
last_device = "iPhone 15 Pro"
last_config = "Development"

[window]
# Preferred terminal size (if supported)
width = 120
height = 40
```

#### 2. Load Function

```rust
const LOCAL_SETTINGS_FILENAME: &str = "settings.local.toml";

/// Load user preferences from .fdemon/settings.local.toml
///
/// Returns None if file doesn't exist (not an error - first run)
pub fn load_user_preferences(project_path: &Path) -> Option<UserPreferences> {
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
```

#### 3. Save Function

```rust
/// Save user preferences to .fdemon/settings.local.toml
///
/// Creates the file if it doesn't exist.
/// Uses atomic write (temp file + rename) for safety.
pub fn save_user_preferences(project_path: &Path, prefs: &UserPreferences) -> Result<()> {
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
```

#### 4. Merge Function

```rust
/// Merge user preferences into settings (user prefs override project settings)
pub fn merge_preferences(settings: &mut Settings, prefs: &UserPreferences) {
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
```

### Acceptance Criteria

1. `load_user_preferences()` loads from `.fdemon/settings.local.toml`
2. Returns `None` gracefully if file doesn't exist (not an error)
3. `save_user_preferences()` writes atomically (temp + rename)
4. Atomic writes prevent corruption if interrupted
5. `merge_preferences()` correctly overrides project settings
6. File includes header comment explaining purpose
7. Unit tests for load/save/merge operations

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_load_user_preferences_missing_file() {
        let temp = tempdir().unwrap();
        let prefs = load_user_preferences(temp.path());
        assert!(prefs.is_none());
    }

    #[test]
    fn test_save_and_load_user_preferences() {
        let temp = tempdir().unwrap();
        std::fs::create_dir_all(temp.path().join(".fdemon")).unwrap();

        let prefs = UserPreferences {
            theme: Some("dark".to_string()),
            last_device: Some("iPhone 15".to_string()),
            ..Default::default()
        };

        save_user_preferences(temp.path(), &prefs).unwrap();

        let loaded = load_user_preferences(temp.path()).unwrap();
        assert_eq!(loaded.theme, Some("dark".to_string()));
        assert_eq!(loaded.last_device, Some("iPhone 15".to_string()));
    }

    #[test]
    fn test_save_creates_directory() {
        let temp = tempdir().unwrap();
        // Don't create .fdemon dir - save should create it

        let prefs = UserPreferences::default();
        save_user_preferences(temp.path(), &prefs).unwrap();

        assert!(temp.path().join(".fdemon/settings.local.toml").exists());
    }

    #[test]
    fn test_merge_preferences_overrides() {
        let mut settings = Settings::default();
        settings.editor.command = "code".to_string();

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
        assert_eq!(settings.ui.theme, "monokai");
    }

    #[test]
    fn test_merge_preferences_partial() {
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
        let temp = tempdir().unwrap();
        std::fs::create_dir_all(temp.path().join(".fdemon")).unwrap();

        let prefs = UserPreferences::default();
        save_user_preferences(temp.path(), &prefs).unwrap();

        let content = std::fs::read_to_string(
            temp.path().join(".fdemon/settings.local.toml")
        ).unwrap();

        assert!(content.contains("User-specific preferences"));
        assert!(content.contains("not tracked in git"));
    }
}
```

### Notes

- The local settings file is intentionally simple at first
- More override options can be added as needed
- Consider supporting `~/.config/flutter-demon/settings.toml` for global user defaults (future)
- The merge function is called after loading project settings

---

## Completion Summary

**Status:** (Not Started)

**Files Modified:**
- (To be filled after implementation)

**Implementation Details:**

(To be filled after implementation)

**Testing Performed:**
- `cargo fmt` -
- `cargo check` -
- `cargo clippy -- -D warnings` -
- `cargo test settings` -

**Notable Decisions:**
- (To be filled after implementation)
