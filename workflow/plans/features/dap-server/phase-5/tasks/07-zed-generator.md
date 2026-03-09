## Task: Zed Config Generator

**Objective**: Implement the Zed DAP config generator that creates or merges a `.zed/debug.json` entry with `tcp_connection` for connecting to fdemon's DAP server.

**Depends on**: 01-extend-parent-ide, 02-ide-config-trait

**Estimated Time**: 2–3 hours

### Scope

- `crates/fdemon-app/src/ide_config/zed.rs`: **CREATE** — `ZedGenerator` struct implementing `IdeConfigGenerator` with JSON generation and merge logic
- `crates/fdemon-app/src/ide_config/mod.rs`: Add `pub mod zed;` declaration

### Details

#### 1. Zed debug.json format

Zed uses `.zed/debug.json` in the project root. The file is a JSON array of debug configurations:

```json
[
    {
        "label": "Flutter (fdemon DAP)",
        "adapter": "fdemon-dap",
        "request": "attach",
        "tcp_connection": {
            "host": "127.0.0.1",
            "port": 4711
        },
        "cwd": "$ZED_WORKTREE_ROOT",
        "fdemon-managed": true
    }
]
```

Key fields:
- `"tcp_connection"` — tells Zed to connect to an existing TCP server rather than spawning a new adapter process
- `"adapter": "fdemon-dap"` — adapter identifier (Zed needs a registered adapter type)
- `"cwd": "$ZED_WORKTREE_ROOT"` — Zed's variable for the workspace root
- `"fdemon-managed": true` — marker for identifying auto-generated entries during merge

#### 2. Zed Dart/Flutter caveat

As of March 2026, Dart/Flutter are not natively supported by Zed's debugger. A community Zed debugger extension for Dart would need to exist for this config to work. The generator should:
- Generate the config anyway (forward-compatible)
- Log a warning at `tracing::warn!` level that Dart debug support in Zed may not be available

#### 3. Fresh generation

When no `.zed/debug.json` exists, generate the JSON array with a single fdemon entry.

#### 4. Merge logic

`.zed/debug.json` is a JSON **array** (not an object like `launch.json`):

1. Parse existing content as a JSON array
2. Find existing fdemon entry by `"label" == "Flutter (fdemon DAP)"`
3. If found: update `tcp_connection.port`
4. If not found: append new entry
5. Serialize back with pretty-printing

Uses the `merge_json_array_entry()` utility from `merge.rs` with `field = "label"`.

#### 5. Trait implementation

```rust
pub struct ZedGenerator;

impl IdeConfigGenerator for ZedGenerator {
    fn config_path(&self, project_root: &Path) -> PathBuf {
        project_root.join(".zed").join("debug.json")
    }

    fn generate(&self, port: u16, _project_root: &Path) -> crate::Result<String> {
        let config = serde_json::json!([
            Self::fdemon_entry(port)
        ]);
        Ok(to_pretty_json(&config))
    }

    fn merge_config(&self, existing: &str, port: u16) -> crate::Result<String> {
        let mut array: Vec<serde_json::Value> = serde_json::from_str(existing)
            .map_err(|e| crate::Error::config(format!("invalid JSON in debug.json: {}", e)))?;
        merge_json_array_entry(&mut array, "label", "Flutter (fdemon DAP)", Self::fdemon_entry(port));
        Ok(to_pretty_json(&serde_json::Value::Array(array)))
    }

    fn ide_name(&self) -> &'static str {
        "Zed"
    }
}
```

### Acceptance Criteria

1. Fresh generation produces valid JSON array with `tcp_connection`, `adapter`, `request`, `label`, and `fdemon-managed` fields
2. Port number is correctly embedded in `tcp_connection.port`
3. Merge finds existing fdemon entry by label and updates port
4. Merge appends new entry when no fdemon entry exists
5. Merge preserves all non-fdemon configurations unchanged
6. Malformed JSON returns an error
7. Empty array `[]` is handled correctly (append entry)
8. `cargo check --workspace` — Pass
9. `cargo test -p fdemon-app` — Pass
10. `cargo clippy --workspace -- -D warnings` — Pass

### Testing

```rust
#[test]
fn test_zed_config_path() {
    let gen = ZedGenerator;
    assert_eq!(
        gen.config_path(Path::new("/project")),
        PathBuf::from("/project/.zed/debug.json")
    );
}

#[test]
fn test_zed_fresh_generation() {
    let gen = ZedGenerator;
    let content = gen.generate(4711, Path::new("/project")).unwrap();
    let parsed: Vec<serde_json::Value> = serde_json::from_str(&content).unwrap();
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0]["label"], "Flutter (fdemon DAP)");
    assert_eq!(parsed[0]["tcp_connection"]["port"], 4711);
    assert_eq!(parsed[0]["tcp_connection"]["host"], "127.0.0.1");
    assert_eq!(parsed[0]["adapter"], "fdemon-dap");
    assert_eq!(parsed[0]["fdemon-managed"], true);
}

#[test]
fn test_zed_merge_updates_existing_entry() {
    let existing = r#"[
        {"label": "Other", "adapter": "other"},
        {"label": "Flutter (fdemon DAP)", "tcp_connection": {"host": "127.0.0.1", "port": 1234}, "fdemon-managed": true}
    ]"#;
    let gen = ZedGenerator;
    let merged = gen.merge_config(existing, 5678).unwrap();
    let parsed: Vec<serde_json::Value> = serde_json::from_str(&merged).unwrap();
    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[0]["label"], "Other"); // preserved
    assert_eq!(parsed[1]["tcp_connection"]["port"], 5678); // updated
}

#[test]
fn test_zed_merge_appends_when_no_fdemon_entry() {
    let existing = r#"[{"label": "Other", "adapter": "other"}]"#;
    let gen = ZedGenerator;
    let merged = gen.merge_config(existing, 4711).unwrap();
    let parsed: Vec<serde_json::Value> = serde_json::from_str(&merged).unwrap();
    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[1]["label"], "Flutter (fdemon DAP)");
}

#[test]
fn test_zed_merge_empty_array() {
    let gen = ZedGenerator;
    let merged = gen.merge_config("[]", 4711).unwrap();
    let parsed: Vec<serde_json::Value> = serde_json::from_str(&merged).unwrap();
    assert_eq!(parsed.len(), 1);
}

#[test]
fn test_zed_merge_malformed_json_returns_error() {
    let gen = ZedGenerator;
    let result = gen.merge_config("not json", 4711);
    assert!(result.is_err());
}

#[test]
fn test_zed_ide_name() {
    assert_eq!(ZedGenerator.ide_name(), "Zed");
}
```

### Notes

- Zed's debug config format is simpler than VS Code's since `.zed/debug.json` is a flat JSON array (no wrapping object with `version` field).
- The `tcp_connection` field is the key mechanism — it tells Zed to connect to an existing TCP server rather than spawning a new adapter. This is analogous to VS Code's `debugServer` field.
- The `fdemon-managed: true` marker is non-standard but harmless — Zed ignores unknown fields in debug configs.
- Dart/Flutter debugger support in Zed depends on a community extension. The generated config is forward-compatible — it will work once Dart debugging is available in Zed.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/ide_config/zed.rs` | Created — `ZedGenerator` struct implementing `IdeConfigGenerator` with JSON generation, merge logic, and 14 unit tests |
| `crates/fdemon-app/src/ide_config/mod.rs` | Added `pub mod zed;` declaration; updated `generate_ide_config()` to dispatch `ParentIde::Zed` to `run_generator(&zed::ZedGenerator, ...)` |

### Notable Decisions/Tradeoffs

1. **Named constants for label/host**: Used `ZED_FDEMON_LABEL` and `ZED_DAP_HOST` constants instead of inline string literals, per CODE_STANDARDS.md anti-magic-numbers guidance.

2. **fdemon_core::Error directly**: The trait signature uses `fdemon_core::Result` and the task spec's `crate::Error` resolved to `fdemon_core::Error::config(...)` — used it directly, consistent with `plugin.rs` patterns in the codebase.

3. **mod.rs was already partially updated**: When this task ran, the `mod.rs` had been updated by concurrent tasks (04–06, 08) to include `run_generator`, `vscode`, `helix`, `neovim`, `emacs` modules, and their dispatch arms — but the Zed arm was still stubbed as `Ok(None)`. This task added `pub mod zed;` and replaced the stub with `run_generator(&zed::ZedGenerator, ...)`. The fully-wired dispatch now handles all 5 IDEs.

4. **Non-array JSON returns error**: Added an extra test (`test_zed_merge_non_array_json_returns_error`) beyond the task spec to cover the case where a JSON object is passed instead of an array — `serde_json::from_str::<Vec<Value>>` correctly fails on objects.

### Testing Performed

- `cargo check --workspace` — Passed
- `cargo test -p fdemon-app ide_config::zed` — Passed (14 tests)
- `cargo test -p fdemon-app` — Passed (1,439 tests, 0 failed)
- `cargo clippy --workspace -- -D warnings` — Passed
- `cargo fmt --all` — Clean

### Risks/Limitations

1. **Zed Dart/Flutter support**: As documented in the task, Dart/Flutter debugging in Zed requires a community extension not yet available (March 2026). The generator emits a `tracing::warn!` and produces the config anyway for forward compatibility.
