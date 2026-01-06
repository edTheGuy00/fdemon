## Task: Init Directory & Gitignore

**Objective**: Ensure `.fdemon` directory exists and `settings.local.toml` is added to `.gitignore`.

**Depends on**: 11-settings-persistence

**Estimated Time**: 0.5-1 hour

### Scope

- `src/config/settings.rs`: Enhance `init_config_dir()` to handle gitignore

**Related Module:**
```
tui/widgets/settings_panel/  # No changes needed, but uses config loaded by init
```

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

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/config/settings.rs` | Added `init_fdemon_directory()`, `ensure_gitignore_entry()`, `gitignore_contains_entry()`, and `generate_default_config()` functions; added 11 comprehensive unit tests |
| `src/config/mod.rs` | Exported `init_fdemon_directory` function |
| `src/tui/runner.rs` | Called `init_fdemon_directory()` at app startup (line 30-33) |

### Implementation Details

**Core Functionality:**
- `init_fdemon_directory()`: Main entry point that ensures `.fdemon/` directory exists, creates default `config.toml` if missing, and calls gitignore helper
- `ensure_gitignore_entry()`: Appends `.fdemon/settings.local.toml` to `.gitignore` with explanatory comment if not already present
- `gitignore_contains_entry()`: Intelligent detection that handles exact matches, trailing spaces/comments, and glob patterns (`.fdemon/`, `.fdemon/*`, `.fdemon/**`)
- `generate_default_config()`: Creates sensible default configuration with all settings including new `stack_trace_collapsed` and `stack_trace_max_frames` UI options

**Startup Integration:**
- Called in `tui/runner.rs::run_with_project()` before loading settings
- Wrapped in non-fatal error handling - logs warning but doesn't crash app
- Ensures directory exists before any config operations

**Testing Coverage:**
- 11 new unit tests covering all acceptance criteria
- Tests idempotency, preserving existing content, no duplicates, various gitignore formats
- Tests empty file creation, missing directory creation, config preservation
- All 1064 tests pass including new tests

### Testing Performed

- `cargo fmt` - Passed
- `cargo check` - Passed
- `cargo clippy -- -D warnings` - Passed (no warnings)
- `cargo test --lib settings::tests` - Passed (55 tests)
- `cargo test --lib` - Passed (1064 tests, 0 failed, 3 ignored)

### Notable Decisions/Tradeoffs

1. **Non-fatal initialization**: Init errors are logged but don't crash the app. This ensures fdemon works even in read-only filesystems or when git isn't initialized
2. **Smart gitignore detection**: Handles various edge cases (trailing spaces, comments, glob patterns) to avoid duplicate entries
3. **Idempotent design**: Function can be called multiple times safely - won't overwrite existing config or duplicate gitignore entries
4. **Explanatory comments**: Gitignore entry includes comment explaining it's for user preferences
5. **Reused existing `init_config_dir()`**: The new function supersedes but doesn't replace the existing `init_config_dir()` - both remain for backward compatibility
6. **Default config includes all settings**: Generated default includes stack trace settings from Phase 3

### Risks/Limitations

1. **Gitignore pattern detection**: The glob pattern detection (`.fdemon/`, `.fdemon/*`) might not work in all git versions, but this is a safe fallback that prevents duplicates in common cases
2. **File system permissions**: If `.fdemon/` can't be created or `.gitignore` can't be written, function returns error (logged as warning)
3. **No git validation**: Doesn't check if directory is a git repo before modifying `.gitignore` - creates the file regardless
