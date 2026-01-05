## Task: Init Directory & Gitignore

**Objective**: Ensure `.fdemon` directory exists and `settings.local.toml` is added to `.gitignore`.

**Depends on**: 11-settings-persistence

**Estimated Time**: 0.5-1 hour

### Scope

- `src/config/settings.rs`: Enhance `init_config_dir()` to handle gitignore

### Details

#### 1. Enhanced Init Function

```rust
// src/config/settings.rs

const LOCAL_SETTINGS_FILENAME: &str = "settings.local.toml";
const GITIGNORE_ENTRY: &str = ".fdemon/settings.local.toml";

/// Initialize Flutter Demon configuration directory and files
///
/// Creates:
/// - `.fdemon/` directory if it doesn't exist
/// - `.fdemon/config.toml` with defaults if missing
/// - Adds `.fdemon/settings.local.toml` to `.gitignore` if not present
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
        // Exact match or with trailing slash/comment
        trimmed == entry ||
        trimmed.starts_with(&format!("{} ", entry)) ||
        trimmed.starts_with(&format!("{}#", entry)) ||
        // Also check for glob patterns that would match
        trimmed == ".fdemon/" ||
        trimmed == ".fdemon/*" ||
        trimmed == ".fdemon/**"
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
```

#### 2. Call Init on Startup

```rust
// In app/mod.rs or main.rs startup sequence

pub async fn run_with_project(project_path: PathBuf) -> Result<()> {
    // Initialize .fdemon directory
    if let Err(e) = init_fdemon_directory(&project_path) {
        warn!("Failed to initialize .fdemon directory: {}", e);
        // Non-fatal - continue with defaults
    }

    // ... rest of startup
}
```

#### 3. Visual Feedback (Optional)

When gitignore is modified, could show a brief message:

```rust
// In the settings panel or log view
if gitignore_modified {
    add_system_log(
        "Added .fdemon/settings.local.toml to .gitignore",
        LogLevel::Info
    );
}
```

### Acceptance Criteria

1. `.fdemon/` directory created on first run if missing
2. `config.toml` created with sensible defaults if missing
3. `.fdemon/settings.local.toml` added to `.gitignore`
4. Gitignore entry includes explanatory comment
5. Existing gitignore content preserved
6. No duplicate entries added to gitignore
7. Handles missing `.gitignore` file (creates it)
8. Handles various gitignore entry formats (trailing spaces, etc.)
9. Non-fatal errors don't crash the app
10. Unit tests for gitignore manipulation

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

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

        let gitignore = std::fs::read_to_string(
            temp.path().join(".gitignore")
        ).unwrap();

        assert!(gitignore.contains(".fdemon/settings.local.toml"));
        assert!(gitignore.contains("Flutter Demon"));
    }

    #[test]
    fn test_init_preserves_existing_gitignore() {
        let temp = tempdir().unwrap();

        // Create existing gitignore
        std::fs::write(
            temp.path().join(".gitignore"),
            "node_modules/\n.env\n"
        ).unwrap();

        init_fdemon_directory(temp.path()).unwrap();

        let gitignore = std::fs::read_to_string(
            temp.path().join(".gitignore")
        ).unwrap();

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
            ".fdemon/settings.local.toml\n"
        ).unwrap();

        init_fdemon_directory(temp.path()).unwrap();

        let gitignore = std::fs::read_to_string(
            temp.path().join(".gitignore")
        ).unwrap();

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
        let gitignore = std::fs::read_to_string(
            temp.path().join(".gitignore")
        ).unwrap();

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
            "[behavior]\nauto_start = true\n"
        ).unwrap();

        init_fdemon_directory(temp.path()).unwrap();

        // Should preserve custom config
        let content = std::fs::read_to_string(
            temp.path().join(".fdemon/config.toml")
        ).unwrap();

        assert!(content.contains("auto_start = true"));
    }
}
```

### Notes

- The gitignore entry targets specifically `settings.local.toml`, not the entire `.fdemon/` directory
- Project-shared files (`config.toml`, `launch.toml`) should be tracked
- The init function is idempotent - safe to call multiple times
- Consider adding `.fdemon/logs/` to gitignore for future log persistence feature

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
