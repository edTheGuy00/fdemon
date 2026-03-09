## Task: Extend ParentIde Enum with Emacs and Helix Variants

**Objective**: Add `Emacs` and `Helix` variants to the `ParentIde` enum with environment variable detection, and add DAP-specific methods (`supports_dap_config`, `dap_config_path`) to enable IDE config auto-generation.

**Depends on**: None

**Estimated Time**: 2–3 hours

### Scope

- `crates/fdemon-app/src/config/types.rs`: Add `Emacs` and `Helix` variants to `ParentIde` enum; implement all existing trait methods (`url_scheme`, `reuse_flag`, `display_name`) for new variants; add `supports_dap_config()` and `dap_config_path()` methods
- `crates/fdemon-app/src/config/settings.rs`: Add detection logic for `$INSIDE_EMACS` and `$HELIX_RUNTIME` environment variables in `detect_parent_ide()`; add `editor_config_for_ide()` mappings for new variants

### Details

#### 1. Add enum variants (`types.rs:531-539`)

Add two new variants to the existing `ParentIde` enum:

```rust
pub enum ParentIde {
    VSCode,
    VSCodeInsiders,
    Cursor,
    Zed,
    IntelliJ,
    AndroidStudio,
    Neovim,
    Emacs,   // NEW
    Helix,   // NEW
}
```

#### 2. Implement existing methods for new variants (`types.rs:541-576`)

| Variant | `url_scheme()` | `reuse_flag()` | `display_name()` |
|---------|---------------|----------------|-------------------|
| `Emacs` | `"file"` | `None` | `"Emacs"` |
| `Helix` | `"file"` | `None` | `"Helix"` |

Neither Emacs nor Helix has a custom URL scheme for file opening, so they use the `"file"` fallback (same as Neovim).

#### 3. Add DAP-specific methods (`types.rs`)

```rust
impl ParentIde {
    /// Returns true if this IDE supports auto-generated DAP configuration.
    /// IntelliJ and Android Studio use proprietary debugging — no standard DAP path.
    pub fn supports_dap_config(&self) -> bool {
        !matches!(self, Self::IntelliJ | Self::AndroidStudio)
    }

    /// Returns the target config file path for DAP auto-configuration.
    /// Returns None for IDEs that don't support DAP config (IntelliJ, Android Studio).
    pub fn dap_config_path(&self, project_root: &Path) -> Option<PathBuf> {
        match self {
            Self::VSCode | Self::VSCodeInsiders | Self::Cursor | Self::Neovim => {
                Some(project_root.join(".vscode/launch.json"))
            }
            Self::Helix => Some(project_root.join(".helix/languages.toml")),
            Self::Zed => Some(project_root.join(".zed/debug.json")),
            Self::Emacs => Some(project_root.join(".fdemon/dap-emacs.el")),
            Self::IntelliJ | Self::AndroidStudio => None,
        }
    }
}
```

Note: Neovim's primary config path is `.vscode/launch.json` (via `load_launchjs()`), not a Neovim-specific file.

#### 4. Add detection logic (`settings.rs:73-123`)

Insert new detection steps into `detect_parent_ide()`. These should be lower priority than existing checks since they're less common:

```rust
// After step 5 (NVIM check) and before the final None:

// Step 6: Emacs detection via $INSIDE_EMACS
// Set by Emacs shell-mode, vterm, eshell, term-mode
if std::env::var("INSIDE_EMACS").is_ok() {
    return Some(ParentIde::Emacs);
}

// Step 7: Helix detection via $HELIX_RUNTIME
// Set when running inside Helix's :sh command
if std::env::var("HELIX_RUNTIME").is_ok() {
    return Some(ParentIde::Helix);
}
```

#### 5. Add editor config mappings (`settings.rs:159-197`)

Add entries to `editor_config_for_ide()` for the new variants:

| Variant | Command | Pattern |
|---------|---------|---------|
| `Emacs` | `emacsclient` | `emacsclient -n +$LINE:$COLUMN $FILE` |
| `Helix` | `hx` | `hx $FILE:$LINE` |

### Acceptance Criteria

1. `ParentIde::Emacs` detected when `$INSIDE_EMACS` is set
2. `ParentIde::Helix` detected when `$HELIX_RUNTIME` is set
3. `supports_dap_config()` returns `true` for all variants except IntelliJ and AndroidStudio
4. `dap_config_path()` returns correct paths for all supported IDEs
5. All existing `ParentIde` methods (`url_scheme`, `reuse_flag`, `display_name`) work for new variants
6. Existing detection order is preserved — new variants are appended, not inserted
7. `cargo check --workspace` — Pass
8. `cargo test -p fdemon-app` — Pass
9. `cargo clippy --workspace -- -D warnings` — Pass

### Testing

```rust
#[test]
fn test_emacs_detection_via_inside_emacs() {
    // Set INSIDE_EMACS env var, call detect_parent_ide(), assert Emacs
}

#[test]
fn test_helix_detection_via_helix_runtime() {
    // Set HELIX_RUNTIME env var, call detect_parent_ide(), assert Helix
}

#[test]
fn test_supports_dap_config_true_for_all_except_intellij() {
    assert!(ParentIde::VSCode.supports_dap_config());
    assert!(ParentIde::Emacs.supports_dap_config());
    assert!(ParentIde::Helix.supports_dap_config());
    assert!(!ParentIde::IntelliJ.supports_dap_config());
    assert!(!ParentIde::AndroidStudio.supports_dap_config());
}

#[test]
fn test_dap_config_path_vscode_family() {
    let root = Path::new("/project");
    assert_eq!(
        ParentIde::VSCode.dap_config_path(root),
        Some(root.join(".vscode/launch.json"))
    );
}

#[test]
fn test_dap_config_path_helix() {
    let root = Path::new("/project");
    assert_eq!(
        ParentIde::Helix.dap_config_path(root),
        Some(root.join(".helix/languages.toml"))
    );
}

#[test]
fn test_dap_config_path_none_for_intellij() {
    assert_eq!(ParentIde::IntelliJ.dap_config_path(Path::new("/p")), None);
}

#[test]
fn test_emacs_display_name() {
    assert_eq!(ParentIde::Emacs.display_name(), "Emacs");
}

#[test]
fn test_helix_display_name() {
    assert_eq!(ParentIde::Helix.display_name(), "Helix");
}
```

### Notes

- Environment variable detection is best-effort. `$INSIDE_EMACS` is not set in all Emacs terminal modes (some custom shell setups skip it). `$HELIX_RUNTIME` may not be set in all Helix versions. Users can fall back to `--dap-config emacs/helix` (Task 10).
- The detection priority puts Emacs and Helix after Neovim. If someone runs Neovim inside Emacs (unlikely but possible), Neovim wins — this is correct since the innermost editor should take precedence.
- `emacsclient -n` opens files without blocking; the `-n` flag is critical for non-blocking behavior.
