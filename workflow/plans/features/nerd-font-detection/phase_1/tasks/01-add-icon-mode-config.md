## Task: Add IconMode to Config Types

**Objective**: Add an `IconMode` enum and `icons` field to `UiSettings` so users can configure icon rendering via `config.toml` or the `FDEMON_ICONS` environment variable.

**Depends on**: None

### Scope

- `crates/fdemon-app/src/config/types.rs`: Add `IconMode` enum, add `icons` field to `UiSettings`
- `crates/fdemon-app/src/config/settings.rs`: Add env var override in `load_settings()`, update default config template

### Details

**1. Add `IconMode` enum to `types.rs`**

Place the enum near `UiSettings` (after line 195):

```rust
/// Icon rendering mode for the TUI.
///
/// Controls whether icons use safe Unicode characters (works in all terminals)
/// or Nerd Font glyphs (requires a Nerd Font installed in the terminal).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IconMode {
    /// Safe Unicode characters that work in all terminals (default)
    #[default]
    Unicode,
    /// Nerd Font glyphs — requires a Nerd Font installed in the terminal
    NerdFonts,
}

impl std::fmt::Display for IconMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IconMode::Unicode => write!(f, "unicode"),
            IconMode::NerdFonts => write!(f, "nerd_fonts"),
        }
    }
}
```

**2. Add `icons` field to `UiSettings`**

In the `UiSettings` struct (line 197-222), add:

```rust
/// Icon mode: "unicode" (default) or "nerd_fonts"
#[serde(default)]
pub icons: IconMode,
```

Update the `Default` impl to include `icons: IconMode::default()`.

**3. Add env var override in `settings.rs`**

In `load_settings()` (line 308-332), after loading settings from TOML, check the `FDEMON_ICONS` env var:

```rust
pub fn load_settings(project_path: &Path) -> Settings {
    let mut settings = /* existing load logic */;

    // Environment variable override for icon mode
    if let Ok(val) = std::env::var("FDEMON_ICONS") {
        match val.to_lowercase().as_str() {
            "nerd_fonts" | "nerd" => settings.ui.icons = IconMode::NerdFonts,
            "unicode" => settings.ui.icons = IconMode::Unicode,
            other => warn!("Unknown FDEMON_ICONS value: {:?}, ignoring", other),
        }
    }

    settings
}
```

**4. Update default config templates**

In both `generate_default_config()` (line 523) and `init_config_dir()` (line 335), add to the `[ui]` section:

```toml
[ui]
# ...existing fields...
# Icon style: "unicode" (default) or "nerd_fonts"
# "nerd_fonts" requires a Nerd Font installed in your terminal
icons = "unicode"
```

**5. Export `IconMode` from `config/mod.rs`**

Ensure `IconMode` is re-exported so `fdemon-tui` can import it.

### Acceptance Criteria

1. `IconMode` enum exists with `Unicode` (default) and `NerdFonts` variants
2. `UiSettings.icons` field serializes/deserializes correctly with TOML
3. `FDEMON_ICONS=nerd_fonts` overrides the config file value
4. `FDEMON_ICONS=unicode` overrides back to default
5. Unknown `FDEMON_ICONS` values are ignored with a warning
6. Default settings produce `IconMode::Unicode`
7. `IconMode` is re-exported from `fdemon-app` for downstream crates
8. Default config template includes the `icons` field
9. `cargo check -p fdemon-app` passes
10. `cargo clippy -p fdemon-app -- -D warnings` passes

### Testing

```rust
#[test]
fn test_icon_mode_default() {
    assert_eq!(IconMode::default(), IconMode::Unicode);
}

#[test]
fn test_icon_mode_display() {
    assert_eq!(IconMode::Unicode.to_string(), "unicode");
    assert_eq!(IconMode::NerdFonts.to_string(), "nerd_fonts");
}

#[test]
fn test_icon_mode_deserialize() {
    let toml = r#"icons = "nerd_fonts""#;
    #[derive(Deserialize)]
    struct W { icons: IconMode }
    let w: W = toml::from_str(toml).unwrap();
    assert_eq!(w.icons, IconMode::NerdFonts);
}

#[test]
fn test_settings_with_icons_field() {
    let toml = r#"
[ui]
icons = "nerd_fonts"
"#;
    let settings: Settings = toml::from_str(toml).unwrap();
    assert_eq!(settings.ui.icons, IconMode::NerdFonts);
}

#[test]
fn test_settings_without_icons_field_defaults() {
    let toml = r#"
[ui]
theme = "default"
"#;
    let settings: Settings = toml::from_str(toml).unwrap();
    assert_eq!(settings.ui.icons, IconMode::Unicode);
}
```

### Notes

- `IconMode` must derive `Serialize` + `Deserialize` so it roundtrips through `save_settings()`/`load_settings()`
- Use `#[serde(rename_all = "snake_case")]` so `NerdFonts` serializes as `"nerd_fonts"`
- The env var override must happen after TOML loading to take precedence
- `IconMode` lives in `fdemon-app` (config layer), not `fdemon-tui` (presentation layer)
