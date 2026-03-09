## Task: IdeConfigGenerator Trait and Module Structure

**Objective**: Create the `ide_config` module in `fdemon-app` with the `IdeConfigGenerator` trait, shared merge utilities for JSON and TOML, and the top-level `generate_ide_config()` dispatch function.

**Depends on**: None

**Estimated Time**: 3–4 hours

### Scope

- `crates/fdemon-app/src/ide_config/mod.rs`: **CREATE** — Module root with `IdeConfigGenerator` trait, `IdeConfigResult`, `ConfigAction` enum, `generate_ide_config()` dispatch function, re-exports
- `crates/fdemon-app/src/ide_config/merge.rs`: **CREATE** — Shared JSON and TOML merge utilities used by per-IDE generators
- `crates/fdemon-app/src/lib.rs`: Add `pub mod ide_config;` declaration

### Details

#### 1. Module structure

```
crates/fdemon-app/src/ide_config/
├── mod.rs       ← trait, dispatch, result types, re-exports
├── merge.rs     ← shared JSON/TOML merge utilities
├── vscode.rs    ← (Task 04)
├── neovim.rs    ← (Task 05)
├── helix.rs     ← (Task 06)
├── zed.rs       ← (Task 07)
└── emacs.rs     ← (Task 08)
```

Only `mod.rs` and `merge.rs` are created in this task. The per-IDE generator submodules are declared in `mod.rs` but implemented in Tasks 04–08.

#### 2. Core types (`mod.rs`)

```rust
use std::path::{Path, PathBuf};

/// Result of an IDE config generation attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdeConfigResult {
    /// Path to the config file that was created/updated.
    pub path: PathBuf,
    /// What action was taken.
    pub action: ConfigAction,
}

/// What happened during config generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigAction {
    /// Config file was created (did not previously exist).
    Created,
    /// Existing config file was updated with new/changed fdemon entry.
    Updated,
    /// Config generation was skipped.
    Skipped(String),
}
```

#### 3. `IdeConfigGenerator` trait (`mod.rs`)

```rust
/// Trait for generating IDE-specific DAP client configuration files.
///
/// Each IDE has its own config format and file location. Implementations
/// handle both fresh creation and merging into existing config files.
pub trait IdeConfigGenerator {
    /// Returns the path where this IDE's config file should be written,
    /// relative to the project root.
    fn config_path(&self, project_root: &Path) -> PathBuf;

    /// Generate the full config file content for a fresh creation.
    fn generate(&self, port: u16, project_root: &Path) -> crate::Result<String>;

    /// Check if a config file already exists at the expected path.
    fn config_exists(&self, project_root: &Path) -> bool {
        self.config_path(project_root).exists()
    }

    /// Merge fdemon DAP config into an existing config file.
    /// Returns the merged content, or an error if the file cannot be parsed.
    ///
    /// Implementations must:
    /// - Find existing fdemon entries (by marker) and update them
    /// - Append a new entry if no fdemon entry exists
    /// - Preserve all non-fdemon entries unchanged
    fn merge_config(&self, existing: &str, port: u16) -> crate::Result<String>;

    /// The display name for this IDE (for logging).
    fn ide_name(&self) -> &'static str;
}
```

#### 4. Dispatch function (`mod.rs`)

```rust
use crate::config::ParentIde;

/// Generate IDE-specific DAP config for the detected (or specified) IDE.
///
/// If `ide` is `None`, returns `Ok(None)` — no config to generate.
/// If the IDE doesn't support DAP config, returns `Ok(None)`.
/// On success, returns the result describing what was created/updated.
pub fn generate_ide_config(
    ide: Option<ParentIde>,
    port: u16,
    project_root: &Path,
) -> crate::Result<Option<IdeConfigResult>> {
    let ide = match ide {
        Some(ide) if ide.supports_dap_config() => ide,
        _ => return Ok(None),
    };

    let generator: Box<dyn IdeConfigGenerator> = match ide {
        ParentIde::VSCode | ParentIde::VSCodeInsiders | ParentIde::Cursor => {
            Box::new(vscode::VSCodeGenerator)
        }
        ParentIde::Neovim => Box::new(neovim::NeovimGenerator),
        ParentIde::Helix => Box::new(helix::HelixGenerator),
        ParentIde::Zed => Box::new(zed::ZedGenerator),
        ParentIde::Emacs => Box::new(emacs::EmacsGenerator),
        ParentIde::IntelliJ | ParentIde::AndroidStudio => return Ok(None),
    };

    let path = generator.config_path(project_root);

    let result = if generator.config_exists(project_root) {
        // Merge into existing config
        let existing = std::fs::read_to_string(&path)
            .map_err(|e| crate::Error::config(format!("failed to read {}: {}", path.display(), e)))?;
        match generator.merge_config(&existing, port) {
            Ok(merged) => {
                std::fs::write(&path, &merged)
                    .map_err(|e| crate::Error::config(format!("failed to write {}: {}", path.display(), e)))?;
                IdeConfigResult { path, action: ConfigAction::Updated }
            }
            Err(e) => {
                // Don't clobber unparseable files — skip with warning
                tracing::warn!("Skipping DAP config merge for {}: {}", path.display(), e);
                IdeConfigResult {
                    path,
                    action: ConfigAction::Skipped(format!("merge failed: {}", e)),
                }
            }
        }
    } else {
        // Create parent directories and write new file
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| crate::Error::config(format!("failed to create {}: {}", parent.display(), e)))?;
        }
        let content = generator.generate(port, project_root)?;
        std::fs::write(&path, &content)
            .map_err(|e| crate::Error::config(format!("failed to write {}: {}", path.display(), e)))?;
        IdeConfigResult { path, action: ConfigAction::Created }
    };

    tracing::info!(
        "DAP config for {} — {:?} at {}",
        generator.ide_name(),
        result.action,
        result.path.display()
    );

    Ok(Some(result))
}
```

#### 5. Shared merge utilities (`merge.rs`)

Provide helper functions for JSON and TOML manipulation that multiple generators reuse:

```rust
/// Marker field name used to identify fdemon-managed entries in JSON configs.
pub const FDEMON_MARKER_FIELD: &str = "fdemon-managed";

/// Marker value for the fdemon config entry name field.
pub const FDEMON_CONFIG_NAME: &str = "Flutter (fdemon)";

/// Find an entry in a JSON array by a string field value.
/// Returns the index if found.
pub fn find_json_entry_by_field(
    array: &[serde_json::Value],
    field: &str,
    value: &str,
) -> Option<usize> { ... }

/// Merge a new entry into a JSON array, replacing an existing entry
/// matched by `field == value`, or appending if not found.
pub fn merge_json_array_entry(
    array: &mut Vec<serde_json::Value>,
    field: &str,
    value: &str,
    new_entry: serde_json::Value,
) { ... }

/// Clean JSONC (JSON with comments) to valid JSON.
/// Re-uses the approach from config/vscode.rs but exposed as a utility.
pub fn clean_jsonc(input: &str) -> String { ... }

/// Serialize JSON with consistent pretty-printing (2-space indent).
pub fn to_pretty_json(value: &serde_json::Value) -> String { ... }
```

The `clean_jsonc` function can delegate to or duplicate the logic from `crates/fdemon-app/src/config/vscode.rs::clean_jsonc()`. Since it's an internal utility, either approach is acceptable — but prefer importing if `clean_jsonc` is made `pub` in the existing module.

### Acceptance Criteria

1. `IdeConfigGenerator` trait compiles with all required methods
2. `generate_ide_config(None, port, root)` returns `Ok(None)`
3. `generate_ide_config(Some(IntelliJ), port, root)` returns `Ok(None)`
4. `ConfigAction` enum has `Created`, `Updated`, `Skipped(String)` variants
5. `merge_json_array_entry` correctly replaces existing entries and appends new ones
6. `find_json_entry_by_field` finds entries by name/label field
7. `clean_jsonc` strips comments and trailing commas from JSONC
8. Module is declared in `lib.rs` and compiles
9. `cargo check --workspace` — Pass
10. `cargo test -p fdemon-app` — Pass
11. `cargo clippy --workspace -- -D warnings` — Pass

### Testing

```rust
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
fn test_find_json_entry_by_field_found() {
    let array = vec![
        json!({"name": "Dart", "type": "dart"}),
        json!({"name": "Flutter (fdemon)", "type": "dart"}),
    ];
    assert_eq!(find_json_entry_by_field(&array, "name", "Flutter (fdemon)"), Some(1));
}

#[test]
fn test_find_json_entry_by_field_not_found() {
    let array = vec![json!({"name": "Dart"})];
    assert_eq!(find_json_entry_by_field(&array, "name", "Flutter (fdemon)"), None);
}

#[test]
fn test_merge_json_array_entry_replaces_existing() {
    let mut array = vec![
        json!({"name": "existing"}),
        json!({"name": "Flutter (fdemon)", "debugServer": 1234}),
    ];
    merge_json_array_entry(&mut array, "name", "Flutter (fdemon)", json!({"name": "Flutter (fdemon)", "debugServer": 5678}));
    assert_eq!(array.len(), 2);
    assert_eq!(array[1]["debugServer"], 5678);
}

#[test]
fn test_merge_json_array_entry_appends_new() {
    let mut array = vec![json!({"name": "existing"})];
    merge_json_array_entry(&mut array, "name", "Flutter (fdemon)", json!({"name": "Flutter (fdemon)"}));
    assert_eq!(array.len(), 2);
}

#[test]
fn test_clean_jsonc_strips_line_comments() {
    assert_eq!(clean_jsonc("{\n  // comment\n  \"key\": 1\n}"), "{\n  \n  \"key\": 1\n}");
}

#[test]
fn test_clean_jsonc_strips_trailing_commas() {
    let input = r#"{"items": [1, 2,]}"#;
    let cleaned = clean_jsonc(input);
    let parsed: serde_json::Value = serde_json::from_str(&cleaned).unwrap();
    assert!(parsed.is_object());
}
```

### Notes

- The trait uses `&self` receivers to allow generators to carry configuration, though the initial implementations are unit structs.
- `config_exists()` has a default implementation (check file existence) which all generators can use without overriding.
- The dispatch function handles the file I/O (read/write/mkdir) so generators only produce string content. This keeps generators pure and testable.
- The `clean_jsonc` function in `config/vscode.rs` is currently `pub(crate)`. If making it `pub` is undesirable, duplicate the ~30-line implementation in `merge.rs`.
- Per-IDE submodule declarations (`mod vscode;`, etc.) should be added to `mod.rs` with `#[allow(unused)]` until Tasks 04–08 create the implementations, or use conditional compilation. Alternatively, declare them only when the files exist.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/ide_config/mod.rs` | Created — `IdeConfigGenerator` trait, `IdeConfigResult`, `ConfigAction`, `generate_ide_config()` placeholder dispatch, re-exports, tests |
| `crates/fdemon-app/src/ide_config/merge.rs` | Created — `find_json_entry_by_field`, `merge_json_array_entry`, `clean_jsonc`, `to_pretty_json`, constants, tests |
| `crates/fdemon-app/src/lib.rs` | Added `pub mod ide_config;` in alphabetical order |
| `crates/fdemon-app/src/actions/mod.rs` | Added `UpdateAction::GenerateIdeConfig` arm that async-dispatches to `ide_config::generate_ide_config()` |
| `crates/fdemon-app/src/settings_items.rs` | Fixed pre-existing test: added missing `auto_configure_ide` field in `DapSettings` struct initializer |

### Notable Decisions/Tradeoffs

1. **Option 1 for dispatch**: Chose the placeholder approach — `generate_ide_config()` returns `Ok(None)` for all supported IDEs until Tasks 04–08 add generator modules. The match arm is exhaustive so the compiler enforces it gets updated when generators arrive.
2. **`clean_jsonc` duplication**: The function in `config/vscode.rs` is not `pub`, so the implementation was duplicated (~70 lines including `strip_json_comments` + `strip_trailing_commas`) in `merge.rs`. Both implementations are identical in logic.
3. **`Result` import**: Used `fdemon_core::Result` directly (not `crate::Result`) because `fdemon-app` does not re-export `Result` at its crate root.
4. **Pre-existing `settings_items.rs` fix**: The `DapSettings` struct initializer in a test was missing the `auto_configure_ide` field added by a prior task. Fixed as a side-effect since it prevented `cargo test` from compiling.
5. **`GenerateIdeConfig` action handler**: Added the missing match arm to `actions/mod.rs` which the compiler required for exhaustive coverage. The handler detects the parent IDE via `detect_parent_ide()` and delegates to `ide_config::generate_ide_config()` in an async task.

### Testing Performed

- `cargo check --workspace` — Passed
- `cargo test -p fdemon-app` — Passed (1367 unit tests + 1 doc test)
- `cargo test -p fdemon-app ide_config` — Passed (27 new tests)
- `cargo clippy --workspace -- -D warnings` — Passed (zero warnings)
- `cargo fmt --all` — Passed

### Risks/Limitations

1. **Placeholder dispatch**: `generate_ide_config()` currently returns `Ok(None)` for all IDEs. Tasks 04–08 must update the match to dispatch to real generators. The exhaustive match ensures the compiler reminds implementors.
2. **`clean_jsonc` duplication**: Two copies of the JSONC cleaning logic exist (`config/vscode.rs` and `ide_config/merge.rs`). If one is modified, the other should be updated too. Making `config/vscode.rs::clean_jsonc` `pub(crate)` would eliminate duplication, but that was not in scope for this task.
