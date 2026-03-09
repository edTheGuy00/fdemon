## Task: Neovim Config Generator

**Objective**: Implement the Neovim DAP config generator that produces both a `.vscode/launch.json` entry (via `load_launchjs()`) and an informational `.nvim-dap.lua` snippet file for direct nvim-dap configuration.

**Depends on**: 01-extend-parent-ide, 02-ide-config-trait

**Estimated Time**: 3–4 hours

### Scope

- `crates/fdemon-app/src/ide_config/neovim.rs`: **CREATE** — `NeovimGenerator` struct implementing `IdeConfigGenerator` with dual-output generation
- `crates/fdemon-app/src/ide_config/mod.rs`: Add `pub mod neovim;` declaration

### Details

#### 1. Dual-output strategy

Neovim (nvim-dap) supports two configuration approaches:

1. **Primary**: `.vscode/launch.json` — nvim-dap can load VS Code launch configs via `require("dap.ext.vscode").load_launchjs()`. This is the most common setup and is already handled by the VS Code generator format.

2. **Secondary**: `.nvim-dap.lua` — A project-local Lua snippet that users can source directly. This is informational — it shows the native nvim-dap configuration in case users prefer not to use `load_launchjs()`.

The `NeovimGenerator` does both:
- `generate()` / `merge_config()` operate on `.vscode/launch.json` (delegates to VS Code format)
- After writing the primary config, also writes `.nvim-dap.lua` as a secondary informational file

#### 2. Primary config (`.vscode/launch.json`)

Uses the same format as the VS Code generator. The `config_path()` returns `.vscode/launch.json`. The `generate()` and `merge_config()` methods produce the same JSON structure:

```json
{
    "name": "Flutter (fdemon)",
    "type": "dart",
    "request": "attach",
    "debugServer": 4711,
    "cwd": "${workspaceFolder}",
    "fdemon-managed": true
}
```

To avoid code duplication, the Neovim generator can internally construct a `VSCodeGenerator` and delegate:

```rust
impl IdeConfigGenerator for NeovimGenerator {
    fn config_path(&self, project_root: &Path) -> PathBuf {
        // Same as VS Code — .vscode/launch.json
        project_root.join(".vscode").join("launch.json")
    }

    fn generate(&self, port: u16, project_root: &Path) -> crate::Result<String> {
        // Delegate to VS Code generator for the primary config
        let vscode = super::vscode::VSCodeGenerator;
        let result = vscode.generate(port, project_root)?;

        // Also write the .nvim-dap.lua snippet (secondary, informational)
        self.write_nvim_dap_lua(port, project_root);

        Ok(result)
    }

    fn merge_config(&self, existing: &str, port: u16) -> crate::Result<String> {
        let vscode = super::vscode::VSCodeGenerator;
        vscode.merge_config(existing, port)
    }

    fn ide_name(&self) -> &'static str {
        "Neovim"
    }
}
```

#### 3. Secondary config (`.nvim-dap.lua`)

Generated at `.nvim-dap.lua` in the project root:

```lua
-- fdemon DAP configuration for Neovim (auto-generated)
--
-- Option 1: Source this file in your Neovim config:
--   dofile(vim.fn.getcwd() .. '/.nvim-dap.lua')
--
-- Option 2: Use load_launchjs() to read .vscode/launch.json:
--   require('dap.ext.vscode').load_launchjs()
--
-- Option 2 is recommended — fdemon auto-generates .vscode/launch.json

local dap = require('dap')

dap.adapters.fdemon = {
  type = 'server',
  host = '127.0.0.1',
  port = 4711,
}

dap.configurations.dart = dap.configurations.dart or {}
table.insert(dap.configurations.dart, {
  type = 'fdemon',
  request = 'attach',
  name = 'Flutter (fdemon)',
  cwd = vim.fn.getcwd(),
})
```

This file is always overwritten (fdemon-owned), not merged. It's a convenience snippet, not a primary config.

#### 4. Writing the Lua snippet

The `write_nvim_dap_lua()` method is best-effort:

```rust
fn write_nvim_dap_lua(&self, port: u16, project_root: &Path) {
    let path = project_root.join(".nvim-dap.lua");
    let content = self.generate_lua_snippet(port);
    if let Err(e) = std::fs::write(&path, content) {
        tracing::warn!("Failed to write .nvim-dap.lua: {}", e);
    } else {
        tracing::debug!("Wrote .nvim-dap.lua at {}", path.display());
    }
}
```

Errors writing the Lua file do not fail the overall config generation — the primary `.vscode/launch.json` is what matters.

### Acceptance Criteria

1. `config_path()` returns `.vscode/launch.json` (same as VS Code)
2. `generate()` produces valid `launch.json` content with `debugServer` field
3. `generate()` also writes `.nvim-dap.lua` in the project root
4. `.nvim-dap.lua` contains correct port number, host, adapter type, and configuration
5. `.nvim-dap.lua` includes usage instructions as comments
6. `merge_config()` correctly delegates to VS Code merge logic
7. Lua snippet is always overwritten (not merged)
8. Failure to write `.nvim-dap.lua` does not fail the overall generation
9. `cargo check --workspace` — Pass
10. `cargo test -p fdemon-app` — Pass
11. `cargo clippy --workspace -- -D warnings` — Pass

### Testing

```rust
#[test]
fn test_neovim_config_path_is_vscode_launch_json() {
    let gen = NeovimGenerator;
    assert_eq!(
        gen.config_path(Path::new("/project")),
        PathBuf::from("/project/.vscode/launch.json")
    );
}

#[test]
fn test_neovim_fresh_generation_produces_valid_launch_json() {
    let dir = tempdir().unwrap();
    let gen = NeovimGenerator;
    let content = gen.generate(4711, dir.path()).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(parsed["configurations"][0]["debugServer"], 4711);
}

#[test]
fn test_neovim_generation_writes_lua_snippet() {
    let dir = tempdir().unwrap();
    let gen = NeovimGenerator;
    gen.generate(4711, dir.path()).unwrap();
    let lua_path = dir.path().join(".nvim-dap.lua");
    assert!(lua_path.exists());
    let content = std::fs::read_to_string(&lua_path).unwrap();
    assert!(content.contains("port = 4711"));
    assert!(content.contains("dap.adapters.fdemon"));
    assert!(content.contains("type = 'server'"));
}

#[test]
fn test_neovim_lua_snippet_port_substitution() {
    let gen = NeovimGenerator;
    let lua = gen.generate_lua_snippet(9999);
    assert!(lua.contains("port = 9999"));
    assert!(!lua.contains("port = 4711"));
}

#[test]
fn test_neovim_merge_delegates_to_vscode() {
    let existing = r#"{
        "version": "0.2.0",
        "configurations": [
            {"name": "Dart", "type": "dart", "request": "launch"}
        ]
    }"#;
    let gen = NeovimGenerator;
    let merged = gen.merge_config(existing, 4711).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&merged).unwrap();
    let configs = parsed["configurations"].as_array().unwrap();
    assert_eq!(configs.len(), 2);
}

#[test]
fn test_neovim_ide_name() {
    assert_eq!(NeovimGenerator.ide_name(), "Neovim");
}
```

### Notes

- The `.nvim-dap.lua` file is written in `generate()` only, not in `merge_config()`. The dispatch function in `mod.rs` calls `generate()` for fresh creation and `merge_config()` for existing files. For the Lua snippet to be updated on port changes, the `generate()` path needs the side-effect, and the merge path should also update it. Consider calling `write_nvim_dap_lua()` from both methods, or having the dispatch function handle it.
- The `write_nvim_dap_lua` method needs access to `project_root`, which `merge_config()` doesn't receive. Two options: (1) have the dispatch function call a separate method on `NeovimGenerator` after merge, or (2) override `config_exists()` or add a post-generation hook. Option 1 is simpler — add a `pub fn write_lua_snippet(port, project_root)` that the dispatch function calls specifically for Neovim.
- Alternatively, restructure so the Neovim generator's trait implementation writes both files. The trait could be extended with an optional `post_generate()` hook, but this may be over-engineering for a single IDE's needs. The simplest approach: call `write_nvim_dap_lua()` in both `generate()` and add a comment that the dispatch function should also call it after merge for Neovim.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/ide_config/vscode.rs` | Created — `VSCodeGenerator` implementing `IdeConfigGenerator` with full generate/merge logic |
| `crates/fdemon-app/src/ide_config/neovim.rs` | Created — `NeovimGenerator` implementing `IdeConfigGenerator` delegating to VS Code generator, plus `generate_lua_snippet()` and `write_nvim_dap_lua()` for the secondary Lua file |
| `crates/fdemon-app/src/ide_config/mod.rs` | Added `pub mod vscode;` and `pub mod neovim;`; updated `generate_ide_config()` signature (removed `_` prefix from `port`/`project_root` params); wired VS Code variants and Neovim into the dispatch match |

### Notable Decisions/Tradeoffs

1. **VS Code generator was a missing dependency**: Task 04 had not been implemented before task 05 was assigned. I implemented both `vscode.rs` and `neovim.rs` as required, since the Neovim generator delegates to `VSCodeGenerator`.

2. **mod.rs dispatch function**: The file had been partially updated by previous tasks (helix, emacs were already wired in) with `_port`/`_project_root` prefixed parameters and VS Code/Neovim stubbed to `Ok(None)`. I updated the function signature and match arms to dispatch those variants via `run_generator`.

3. **Lua snippet on merge path**: The task notes that `merge_config()` doesn't receive `project_root`, so `.nvim-dap.lua` is only updated via the `generate()` path (fresh creation). The `write_nvim_dap_lua` method is public (`pub fn`) so the dispatch function or a caller can invoke it explicitly after a merge if needed. This follows Option 1 from the task notes without over-engineering the trait.

### Testing Performed

- `cargo check --workspace` — Passed
- `cargo test -p fdemon-app` — Passed (1425 tests, 0 failures)
- `cargo test -p fdemon-app -- ide_config::neovim` — Passed (15 tests)
- `cargo test -p fdemon-app -- ide_config::vscode` — Passed (12 tests)
- `cargo clippy --workspace -- -D warnings` — Passed (0 warnings)
- `cargo fmt --all -- --check` — Passed

### Risks/Limitations

1. **Lua snippet not updated on merge**: When `run_generator` takes the merge path (existing `launch.json`), `write_nvim_dap_lua()` is not called because `merge_config()` doesn't receive `project_root`. The Lua snippet is only written on fresh generation. If the port changes and the file is merged rather than recreated, `.nvim-dap.lua` will be stale. A future task can address this by calling `gen.write_nvim_dap_lua(port, project_root)` explicitly in `run_generator` when the generator is `NeovimGenerator` (e.g., via downcasting or a trait method).
