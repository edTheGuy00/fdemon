## Task: --dap-config CLI Flag for Manual Generation

**Objective**: Add a `--dap-config <IDE>` CLI flag that generates DAP config for a specific IDE without auto-detection, enabling manual config generation for terminals without IDE detection and for scripting use cases.

**Depends on**: 03-dap-settings-and-messages, 04-vscode-generator, 05-neovim-generator, 06-helix-generator, 07-zed-generator, 08-emacs-generator

**Estimated Time**: 2–3 hours

### Scope

- `flutter-demon/src/main.rs`: Add `--dap-config <IDE>` clap argument with enum value parsing
- `crates/fdemon-app/src/ide_config/mod.rs`: Add `generate_for_ide_name()` function that maps CLI string to `ParentIde` and calls `generate_ide_config()`
- `crates/fdemon-app/src/config/types.rs`: (optional) Add `DapConfigIde` enum for CLI value parsing

### Details

#### 1. CLI argument (`main.rs`)

Add a new optional argument using clap:

```rust
/// Generate DAP config for a specific IDE without auto-detection.
/// Can be used standalone: fdemon --dap-config vscode --dap-port 4711
/// Values: vscode, neovim, helix, zed, emacs
#[arg(long, value_name = "IDE")]
dap_config: Option<String>,
```

**Accepted values**: `vscode`, `neovim`, `helix`, `zed`, `emacs` (case-insensitive).

Invalid values should print a clear error message listing the accepted values.

#### 2. Standalone mode

When `--dap-config` is used, fdemon can operate in two modes:

1. **Standalone config generation**: `fdemon --dap-config vscode --dap-port 4711` — generates config and exits immediately. The TUI/Engine is never started. This is useful for scripting and CI.

2. **Combined with normal run**: `fdemon --dap-config vscode` — starts normally but overrides auto-detection with the specified IDE for config generation when DAP starts.

**Implementation for standalone mode:**

```rust
// In main(), after parsing args:
if let Some(ide_str) = &args.dap_config {
    let port = args.dap_port.unwrap_or(0);
    if port == 0 && !has_project_path {
        // Standalone mode needs a port
        eprintln!("Error: --dap-config requires --dap-port when used standalone");
        std::process::exit(1);
    }

    // If only --dap-config and --dap-port, generate and exit
    if is_standalone_config_mode(&args) {
        let ide = parse_ide_name(ide_str)?;
        let project_root = resolve_project_path(&args)?;
        match generate_ide_config(Some(ide), port, &project_root)? {
            Some(result) => {
                println!("Generated DAP config: {:?} at {}", result.action, result.path.display());
            }
            None => {
                println!("IDE '{}' does not support DAP config generation", ide_str);
            }
        }
        return Ok(());
    }
}
```

#### 3. IDE name parsing

Add a helper function (in `ide_config/mod.rs` or a dedicated location):

```rust
/// Parse a CLI IDE name string to a ParentIde variant.
pub fn parse_ide_name(name: &str) -> crate::Result<ParentIde> {
    match name.to_lowercase().as_str() {
        "vscode" | "vs-code" | "code" => Ok(ParentIde::VSCode),
        "neovim" | "nvim" => Ok(ParentIde::Neovim),
        "helix" | "hx" => Ok(ParentIde::Helix),
        "zed" => Ok(ParentIde::Zed),
        "emacs" => Ok(ParentIde::Emacs),
        _ => Err(crate::Error::config(format!(
            "unknown IDE '{}'. Valid values: vscode, neovim, helix, zed, emacs",
            name
        ))),
    }
}
```

#### 4. Override auto-detection

When `--dap-config` is used in combination with a normal run (not standalone), the specified IDE overrides `detect_parent_ide()` for config generation:

```rust
// In the GenerateIdeConfig action handler:
let ide = if let Some(override_ide) = cli_ide_override {
    Some(override_ide)
} else {
    detect_parent_ide()
};
```

This requires passing the CLI override through the Engine or state. Options:
- Store `cli_dap_config_ide: Option<ParentIde>` on `Engine` or `AppState`
- Pass it through `UpdateAction::GenerateIdeConfig { port, ide_override: Option<ParentIde> }`

The `UpdateAction` approach is cleaner — extend the variant:

```rust
UpdateAction::GenerateIdeConfig {
    port: u16,
    ide_override: Option<ParentIde>,
},
```

#### 5. Mutual exclusivity with `--dap-stdio`

`--dap-config` should be compatible with `--dap-port` but mutually exclusive with `--dap-stdio` (which runs as a subprocess adapter, not a server). Add a clap conflict:

```rust
#[arg(long, conflicts_with = "dap_stdio")]
dap_config: Option<String>,
```

### Acceptance Criteria

1. `fdemon --dap-config vscode --dap-port 4711` generates `.vscode/launch.json` and exits
2. `fdemon --dap-config neovim --dap-port 4711` generates both `.vscode/launch.json` and `.nvim-dap.lua`
3. `fdemon --dap-config helix --dap-port 4711` generates `.helix/languages.toml`
4. `fdemon --dap-config zed --dap-port 4711` generates `.zed/debug.json`
5. `fdemon --dap-config emacs --dap-port 4711` generates `.fdemon/dap-emacs.el`
6. Invalid IDE name prints clear error with valid options
7. `--dap-config` without `--dap-port` in standalone mode prints error
8. `--dap-config` with `--dap-stdio` is rejected by clap
9. Standalone mode exits cleanly without starting TUI or Engine
10. `cargo check --workspace` — Pass
11. `cargo test --workspace` — Pass
12. `cargo clippy --workspace -- -D warnings` — Pass

### Testing

```rust
#[test]
fn test_parse_ide_name_vscode() {
    assert_eq!(parse_ide_name("vscode").unwrap(), ParentIde::VSCode);
    assert_eq!(parse_ide_name("vs-code").unwrap(), ParentIde::VSCode);
    assert_eq!(parse_ide_name("code").unwrap(), ParentIde::VSCode);
    assert_eq!(parse_ide_name("VSCODE").unwrap(), ParentIde::VSCode); // case-insensitive
}

#[test]
fn test_parse_ide_name_neovim() {
    assert_eq!(parse_ide_name("neovim").unwrap(), ParentIde::Neovim);
    assert_eq!(parse_ide_name("nvim").unwrap(), ParentIde::Neovim);
}

#[test]
fn test_parse_ide_name_helix() {
    assert_eq!(parse_ide_name("helix").unwrap(), ParentIde::Helix);
    assert_eq!(parse_ide_name("hx").unwrap(), ParentIde::Helix);
}

#[test]
fn test_parse_ide_name_invalid() {
    assert!(parse_ide_name("sublime").is_err());
    assert!(parse_ide_name("").is_err());
}

#[test]
fn test_standalone_config_generation() {
    let dir = tempdir().unwrap();
    let result = generate_ide_config(Some(ParentIde::VSCode), 4711, dir.path()).unwrap();
    assert!(result.is_some());
    assert!(dir.path().join(".vscode/launch.json").exists());
}
```

Note: CLI integration testing (actual binary invocation with `--dap-config`) would be integration tests in the `tests/` directory, using `assert_cmd` or similar. These are optional for this task.

### Notes

- The standalone mode (`--dap-config` + `--dap-port` without starting the TUI) is a simple early-return path in `main()`. It avoids the complexity of Engine initialization just for config generation.
- Aliases (`code`, `nvim`, `hx`) are convenience shortcuts for common tool names that users are likely to type.
- When combined with a normal run, `--dap-config vscode` overrides auto-detection. This is useful when fdemon can't detect the IDE (e.g., running in tmux inside VS Code, where `$TERM_PROGRAM` is `tmux`).
- Consider using clap's `ValueEnum` derive macro for the IDE name parsing instead of manual string matching, for better help text and tab completion.
