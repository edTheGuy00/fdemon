## Task: Helix Config Generator

**Objective**: Implement the Helix DAP config generator that creates or merges a `.helix/languages.toml` entry with `transport = "tcp"` for the Dart language debugger section.

**Depends on**: 01-extend-parent-ide, 02-ide-config-trait

**Estimated Time**: 3–4 hours

### Scope

- `crates/fdemon-app/src/ide_config/helix.rs`: **CREATE** — `HelixGenerator` struct implementing `IdeConfigGenerator` with TOML generation and merge logic
- `crates/fdemon-app/src/ide_config/mod.rs`: Add `pub mod helix;` declaration
- `crates/fdemon-app/Cargo.toml`: Ensure `toml` crate is available (likely already a dependency for config parsing)

### Details

#### 1. Helix DAP transport model

Helix's DAP integration works differently from VS Code:
- Helix always **spawns** the debug adapter binary and passes a port via `port-arg`
- It does not support connecting to an already-running DAP server
- The workaround: configure `fdemon` as the command with `--dap-port {}` as the port arg, so Helix spawns a new fdemon DAP instance on its chosen port

This means the generated config makes Helix spawn a new fdemon process in DAP-only mode, which is different from the "connect to running fdemon" model used by VS Code. This is a known limitation documented in the plan.

#### 2. Fresh generation

When no `.helix/languages.toml` exists, generate:

```toml
# fdemon DAP configuration for Helix (auto-generated)
# Helix will spawn fdemon as a DAP adapter on a port it chooses.
# For connecting to an already-running fdemon, see the docs.

[[language]]
name = "dart"

[language.debugger]
name = "fdemon-dap"
transport = "tcp"
command = "fdemon"
args = ["--dap-stdio"]
port-arg = "--dap-port {}"

[[language.debugger.templates]]
name = "Flutter: Attach (fdemon)"
request = "attach"
completion = []

[language.debugger.templates.args]
```

**Note on transport**: The `--dap-stdio` in `args` combined with `port-arg` means Helix will call `fdemon --dap-stdio --dap-port <PORT>`. However, `--dap-stdio` and `--dap-port` are mutually exclusive in the current CLI. The correct approach is to use only `--dap-port {}` in `port-arg` and leave `args = []`:

```toml
command = "fdemon"
args = []
port-arg = "--dap-port {}"
```

Helix spawns `fdemon --dap-port <PORT>`, which starts a new fdemon instance with DAP on that port.

#### 3. Merge logic

TOML merge is more complex than JSON because `languages.toml` uses arrays of tables (`[[language]]`):

1. Parse existing TOML content using the `toml` crate
2. Find the `[[language]]` entry where `name = "dart"`
3. If found: update the `[language.debugger]` section, preserving all other language entries
4. If not found: append a new `[[language]]` entry for Dart
5. Serialize back to TOML

**Edge cases:**
- File exists but has no `[[language]]` entries → add the Dart entry
- File has a Dart entry with a different debugger → replace the debugger section (warn about overwrite)
- File has a Dart entry with no debugger → add the debugger section
- Malformed TOML → return error (caller skips with warning)
- File has other languages (e.g., Rust, Python) → preserve them completely

#### 4. TOML serialization challenge

The `toml` crate's serialization may reorder keys or change formatting. To minimize disruption to existing files:
- Use `toml_edit` crate if available (preserves formatting and comments)
- If only `toml` is available, accept that formatting may change during merge

Check if `toml_edit` is already a dependency. If not, evaluate whether to add it or accept `toml` crate's serialization.

#### 5. Trait implementation

```rust
pub struct HelixGenerator;

impl IdeConfigGenerator for HelixGenerator {
    fn config_path(&self, project_root: &Path) -> PathBuf {
        project_root.join(".helix").join("languages.toml")
    }

    fn generate(&self, port: u16, _project_root: &Path) -> crate::Result<String> {
        // Generate full languages.toml with Dart debugger config
        // Note: `port` is not embedded in the TOML — Helix picks its own port
        // via port-arg at runtime. The port parameter is unused here.
        Ok(Self::dart_debugger_toml())
    }

    fn merge_config(&self, existing: &str, _port: u16) -> crate::Result<String> {
        // Parse existing TOML, find/replace Dart debugger section
        ...
    }

    fn ide_name(&self) -> &'static str {
        "Helix"
    }
}
```

**Important**: The `port` parameter is not used in the generated TOML because Helix controls port assignment at runtime via `port-arg`. The generated config is static — it always points Helix to `fdemon --dap-port {}`.

### Acceptance Criteria

1. Fresh generation produces valid TOML with `[[language]]`, `[language.debugger]`, and `[[language.debugger.templates]]` sections
2. Generated TOML has `name = "dart"`, `transport = "tcp"`, `command = "fdemon"`
3. Merge finds existing Dart language entry and updates debugger section
4. Merge preserves all non-Dart language entries unchanged
5. Merge appends Dart entry when none exists
6. Malformed TOML returns an error (not a panic)
7. Generated config is syntactically valid and can be parsed by `toml::from_str()`
8. `cargo check --workspace` — Pass
9. `cargo test -p fdemon-app` — Pass
10. `cargo clippy --workspace -- -D warnings` — Pass

### Testing

```rust
#[test]
fn test_helix_config_path() {
    let gen = HelixGenerator;
    assert_eq!(
        gen.config_path(Path::new("/project")),
        PathBuf::from("/project/.helix/languages.toml")
    );
}

#[test]
fn test_helix_fresh_generation() {
    let gen = HelixGenerator;
    let content = gen.generate(4711, Path::new("/project")).unwrap();
    assert!(content.contains("name = \"dart\""));
    assert!(content.contains("transport = \"tcp\""));
    assert!(content.contains("command = \"fdemon\""));
    assert!(content.contains("port-arg"));
    // Verify it's valid TOML
    let parsed: toml::Value = toml::from_str(&content).unwrap();
    assert!(parsed.get("language").is_some());
}

#[test]
fn test_helix_merge_adds_dart_entry() {
    let existing = r#"
[[language]]
name = "rust"

[language.auto-format]
enable = true
"#;
    let gen = HelixGenerator;
    let merged = gen.merge_config(existing, 4711).unwrap();
    assert!(merged.contains("name = \"dart\""));
    assert!(merged.contains("name = \"rust\"")); // preserved
}

#[test]
fn test_helix_merge_updates_existing_dart_debugger() {
    let existing = r#"
[[language]]
name = "dart"

[language.debugger]
name = "old-debugger"
transport = "stdio"
command = "dart"
"#;
    let gen = HelixGenerator;
    let merged = gen.merge_config(existing, 4711).unwrap();
    assert!(merged.contains("command = \"fdemon\""));
    assert!(!merged.contains("command = \"dart\""));
}

#[test]
fn test_helix_merge_malformed_toml_returns_error() {
    let gen = HelixGenerator;
    let result = gen.merge_config("not [valid toml", 4711);
    assert!(result.is_err());
}

#[test]
fn test_helix_ide_name() {
    assert_eq!(HelixGenerator.ide_name(), "Helix");
}
```

### Notes

- The `port` parameter to `generate()` and `merge_config()` is effectively unused for Helix since Helix controls port assignment at runtime. The trait requires the parameter for uniformity across generators. Consider documenting this in the method body with a comment.
- Helix's `transport = "tcp"` model fundamentally requires spawning a new process. Users who want to debug against an already-running fdemon session should use a wrapper script or the `--dap-stdio` flag. This is documented as a known limitation.
- If `toml_edit` is not available as a dependency, the merge logic using the basic `toml` crate will reformat the file. This is acceptable since `.helix/languages.toml` is typically small and project-local.
- The `[[language.debugger.templates]]` uses `request = "attach"` because fdemon already manages the Flutter process. Helix doesn't have an equivalent of VS Code's `debugServer` — the spawned fdemon instance handles the connection.
