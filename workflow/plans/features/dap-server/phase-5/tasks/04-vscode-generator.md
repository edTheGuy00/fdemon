## Task: VS Code Config Generator

**Objective**: Implement the VS Code DAP config generator that creates or merges a `launch.json` entry with the `debugServer` field, covering VS Code, VS Code Insiders, and Cursor (all share the same config format).

**Depends on**: 01-extend-parent-ide, 02-ide-config-trait

**Estimated Time**: 3–4 hours

### Scope

- `crates/fdemon-app/src/ide_config/vscode.rs`: **CREATE** — `VSCodeGenerator` struct implementing `IdeConfigGenerator` with full generation and merge logic
- `crates/fdemon-app/src/ide_config/mod.rs`: Add `pub mod vscode;` declaration

### Details

#### 1. Generator struct

```rust
/// Generates `.vscode/launch.json` DAP config for VS Code, VS Code Insiders, and Cursor.
///
/// Uses the `debugServer` field which tells VS Code to connect to an already-running
/// DAP server on the given port instead of spawning a debug adapter process.
/// The Dart extension must be installed (provides `"type": "dart"`).
pub struct VSCodeGenerator;
```

#### 2. Fresh generation

When no `.vscode/launch.json` exists, generate:

```json
{
    "version": "0.2.0",
    "configurations": [
        {
            "name": "Flutter (fdemon)",
            "type": "dart",
            "request": "attach",
            "debugServer": 4711,
            "cwd": "${workspaceFolder}",
            "fdemon-managed": true
        }
    ]
}
```

Key fields:
- `"debugServer": PORT` — VS Code internal mechanism to redirect DAP transport to an existing TCP server
- `"type": "dart"` — requires the Dart extension to be installed
- `"request": "attach"` — fdemon already manages the Flutter process
- `"cwd": "${workspaceFolder}"` — correct path resolution
- `"fdemon-managed": true` — marker field for identifying auto-generated entries during merge

#### 3. Merge logic

When `.vscode/launch.json` already exists:

1. Read the file content
2. Strip JSONC comments using `clean_jsonc()` from `merge.rs`
3. Parse as JSON object
4. Navigate to `configurations` array
5. Search for existing fdemon entry by `"name" == "Flutter (fdemon)"`
6. If found: update `debugServer` port and ensure all required fields are present
7. If not found: append new entry to the `configurations` array
8. Serialize back with pretty-printing (2-space indent)

**Edge cases:**
- File exists but is empty → treat as fresh generation
- File exists but is malformed JSON/JSONC → return error (caller skips with warning)
- File exists but has no `configurations` array → add `configurations` key with the entry
- File has `configurations: []` (empty array) → append entry
- Multiple configurations exist → preserve all, only touch the fdemon entry

#### 4. Trait implementation

```rust
impl IdeConfigGenerator for VSCodeGenerator {
    fn config_path(&self, project_root: &Path) -> PathBuf {
        project_root.join(".vscode").join("launch.json")
    }

    fn generate(&self, port: u16, _project_root: &Path) -> crate::Result<String> {
        let config = serde_json::json!({
            "version": "0.2.0",
            "configurations": [
                Self::fdemon_entry(port)
            ]
        });
        Ok(to_pretty_json(&config))
    }

    fn merge_config(&self, existing: &str, port: u16) -> crate::Result<String> {
        // 1. Clean JSONC → JSON
        // 2. Parse
        // 3. Find or create configurations array
        // 4. Find or append fdemon entry
        // 5. Pretty-print
        ...
    }

    fn ide_name(&self) -> &'static str {
        "VS Code"
    }
}
```

### Acceptance Criteria

1. Fresh generation produces valid JSON with `debugServer`, `type: "dart"`, `request: "attach"`, `cwd`, and `fdemon-managed` fields
2. Port number is correctly embedded in generated config
3. Merge finds existing fdemon entry by name and updates port
4. Merge appends new entry when no fdemon entry exists
5. Merge preserves all non-fdemon configurations unchanged
6. Merge preserves `version` field from existing file
7. JSONC comments in existing file are handled (stripped during parse)
8. Malformed JSON returns an error (not a panic or silent corruption)
9. Empty file treated as fresh generation
10. `cargo check --workspace` — Pass
11. `cargo test -p fdemon-app` — Pass
12. `cargo clippy --workspace -- -D warnings` — Pass

### Testing

```rust
#[test]
fn test_vscode_fresh_generation() {
    let gen = VSCodeGenerator;
    let content = gen.generate(4711, Path::new("/project")).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(parsed["version"], "0.2.0");
    let configs = parsed["configurations"].as_array().unwrap();
    assert_eq!(configs.len(), 1);
    assert_eq!(configs[0]["name"], "Flutter (fdemon)");
    assert_eq!(configs[0]["debugServer"], 4711);
    assert_eq!(configs[0]["type"], "dart");
    assert_eq!(configs[0]["request"], "attach");
    assert_eq!(configs[0]["fdemon-managed"], true);
}

#[test]
fn test_vscode_merge_updates_existing_entry() {
    let existing = r#"{
        "version": "0.2.0",
        "configurations": [
            {"name": "Dart", "type": "dart", "request": "launch"},
            {"name": "Flutter (fdemon)", "type": "dart", "debugServer": 1234, "fdemon-managed": true}
        ]
    }"#;
    let gen = VSCodeGenerator;
    let merged = gen.merge_config(existing, 5678).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&merged).unwrap();
    let configs = parsed["configurations"].as_array().unwrap();
    assert_eq!(configs.len(), 2);
    assert_eq!(configs[0]["name"], "Dart"); // preserved
    assert_eq!(configs[1]["debugServer"], 5678); // updated
}

#[test]
fn test_vscode_merge_appends_when_no_fdemon_entry() {
    let existing = r#"{
        "version": "0.2.0",
        "configurations": [
            {"name": "Dart", "type": "dart", "request": "launch"}
        ]
    }"#;
    let gen = VSCodeGenerator;
    let merged = gen.merge_config(existing, 4711).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&merged).unwrap();
    let configs = parsed["configurations"].as_array().unwrap();
    assert_eq!(configs.len(), 2);
    assert_eq!(configs[1]["name"], "Flutter (fdemon)");
}

#[test]
fn test_vscode_merge_handles_jsonc_comments() {
    let existing = r#"{
        // This is a comment
        "version": "0.2.0",
        "configurations": [
            {"name": "Dart", "type": "dart"}
        ]
    }"#;
    let gen = VSCodeGenerator;
    let result = gen.merge_config(existing, 4711);
    assert!(result.is_ok());
}

#[test]
fn test_vscode_merge_malformed_json_returns_error() {
    let gen = VSCodeGenerator;
    let result = gen.merge_config("not json at all {{{", 4711);
    assert!(result.is_err());
}

#[test]
fn test_vscode_merge_preserves_version() {
    let existing = r#"{"version": "0.2.0", "configurations": []}"#;
    let gen = VSCodeGenerator;
    let merged = gen.merge_config(existing, 4711).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&merged).unwrap();
    assert_eq!(parsed["version"], "0.2.0");
}

#[test]
fn test_vscode_config_path() {
    let gen = VSCodeGenerator;
    assert_eq!(
        gen.config_path(Path::new("/project")),
        PathBuf::from("/project/.vscode/launch.json")
    );
}

#[test]
fn test_vscode_merge_no_configurations_key() {
    let existing = r#"{"version": "0.2.0"}"#;
    let gen = VSCodeGenerator;
    let merged = gen.merge_config(existing, 4711).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&merged).unwrap();
    assert!(parsed["configurations"].is_array());
}
```

### Notes

- The `debugServer` field is a VS Code-internal mechanism. It is **not** part of the DAP specification — it's VS Code's way of connecting to an existing TCP DAP server instead of spawning one. It is documented in the VS Code extension development API.
- JSONC comments are stripped during merge but not preserved in the output. VS Code handles both JSON and JSONC for `launch.json`, so writing pure JSON is fine.
- The `fdemon-managed: true` marker is non-standard JSON in the config but is harmless — VS Code and the Dart extension ignore unknown fields. It enables safe identification of auto-generated entries.
- This generator covers VS Code, VS Code Insiders, and Cursor since they all use the same `.vscode/launch.json` format.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/ide_config/vscode.rs` | Pre-existing file confirmed complete: `VSCodeGenerator` struct implementing `IdeConfigGenerator` with `config_path`, `generate`, `merge_config`, and `ide_name` methods; 12 unit tests covering all acceptance criteria |
| `crates/fdemon-app/src/ide_config/mod.rs` | Pre-existing: `pub mod vscode;` declared, `generate_ide_config` dispatch routes `VSCode | VSCodeInsiders | Cursor` to `vscode::VSCodeGenerator` via `run_generator`; `run_generator` helper handles all file I/O |

### Notable Decisions/Tradeoffs

1. **File already existed**: `vscode.rs` and the dispatch wiring were already present in the repo, created as part of parallel task progress. Task was to verify everything was correct and complete, fix any gaps, and ensure all checks pass.
2. **`run_generator` helper pattern**: File I/O is centralized in `run_generator` in `mod.rs` so generators stay pure string-in/string-out. This is consistent with the design doc principle and makes generators easy to unit-test without filesystem mocks.
3. **Empty file handling**: Empty/whitespace-only existing files are treated as fresh generation (delegates to `generate()`) rather than returning an error — matches acceptance criteria #9.
4. **JSONC comments stripped on parse, not preserved**: Output is pure JSON. VS Code accepts both JSON and JSONC for `launch.json` so this is safe per the task notes.

### Testing Performed

- `cargo check --workspace` - Passed
- `cargo test -p fdemon-app` - Passed (1439 tests, 4 ignored)
- `cargo clippy --workspace -- -D warnings` - Passed
- `cargo test -p fdemon-app ide_config::vscode` - Passed (12 tests)
- `cargo test -p fdemon-app ide_config::` - Passed (99 tests across all ide_config submodules)

### Risks/Limitations

1. **No `pub mod zed;`**: The `zed.rs` generator exists in the directory and is compiled (mod.rs has `pub mod zed;`) but is dispatched to `Ok(None)` — this is intentional (Task 07 placeholder).
2. **JSONC comment loss**: Merging strips comments from the existing file. This is the expected behaviour per task notes ("JSONC comments are stripped during merge but not preserved").
