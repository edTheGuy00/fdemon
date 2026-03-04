## Task: Add DapSettings Config Struct, Settings Items, and Handler

**Objective**: Define the `DapSettings` configuration struct in `fdemon-app`, register DAP settings in the settings panel UI, and wire up the settings handler so DAP configuration can be persisted to `.fdemon/config.toml` and modified at runtime.

**Depends on**: None

### Scope

- `crates/fdemon-app/src/config/types.rs` — Add `DapSettings` struct, add `dap` field to `Settings`
- `crates/fdemon-app/src/config/mod.rs` — Re-export `DapSettings`
- `crates/fdemon-app/src/settings_items.rs` — Add DAP section to `project_settings_items()`
- `crates/fdemon-app/src/handler/settings.rs` — Add `dap.*` arms to `apply_project_setting()`

### Details

#### 1. DapSettings Struct (`config/types.rs`)

Follow the `DevToolsSettings` pattern (lines 278-365). Add after the `EditorSettings` section:

```rust
/// Configuration for the embedded DAP (Debug Adapter Protocol) server.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct DapSettings {
    /// Always enable DAP server on startup (overrides auto-detection).
    /// Can also use --dap CLI flag.
    #[serde(default)]
    pub enabled: bool,

    /// Auto-start DAP server when running inside a detected IDE terminal
    /// (VS Code, Neovim, Helix, Zed, Emacs). No effect if enabled = true.
    #[serde(default = "default_auto_start_in_ide")]
    pub auto_start_in_ide: bool,

    /// TCP port for DAP connections. 0 = auto-assign an available port.
    /// Use a fixed port for stable IDE configs across restarts.
    #[serde(default)]
    pub port: u16,

    /// Bind address for the DAP server.
    #[serde(default = "default_bind_address")]
    pub bind_address: String,

    /// Suppress auto-reload while debugger is paused at a breakpoint.
    #[serde(default = "default_suppress_reload")]
    pub suppress_reload_on_pause: bool,
}

fn default_auto_start_in_ide() -> bool { true }
fn default_bind_address() -> String { "127.0.0.1".to_string() }
fn default_suppress_reload() -> bool { true }

impl Default for DapSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            auto_start_in_ide: default_auto_start_in_ide(),
            port: 0,
            bind_address: default_bind_address(),
            suppress_reload_on_pause: default_suppress_reload(),
        }
    }
}
```

Add the `dap` field to `Settings` struct (after `editor`):

```rust
pub struct Settings {
    // ... existing fields ...
    #[serde(default)]
    pub dap: DapSettings,
}
```

#### 2. Re-export (`config/mod.rs`)

Add `DapSettings` to the `pub use types::{ ... }` re-export list at `config/mod.rs:32-36`.

#### 3. Settings Items (`settings_items.rs`)

Add a "DAP Server" section to `project_settings_items()`. Insert after the existing DevTools/Editor sections. Follow the existing builder pattern:

```rust
// ─────────────────────────────────────────────────────────
// DAP Server Section
// ─────────────────────────────────────────────────────────
items.push(
    SettingItem::new("dap.enabled", "Always Enabled")
        .description("Always enable DAP server on startup (ignores IDE detection)")
        .value(SettingValue::Bool(settings.dap.enabled))
        .default(SettingValue::Bool(false))
        .section("DAP Server"),
);
items.push(
    SettingItem::new("dap.auto_start_in_ide", "Auto-Start in IDE")
        .description("Auto-start DAP server when running inside a detected IDE terminal")
        .value(SettingValue::Bool(settings.dap.auto_start_in_ide))
        .default(SettingValue::Bool(true))
        .section("DAP Server"),
);
items.push(
    SettingItem::new("dap.port", "Port")
        .description("TCP port for DAP connections (0 = auto-assign)")
        .value(SettingValue::Number(settings.dap.port as i64))
        .default(SettingValue::Number(0))
        .section("DAP Server"),
);
items.push(
    SettingItem::new("dap.bind_address", "Bind Address")
        .description("Network address to bind the DAP server to")
        .value(SettingValue::String(settings.dap.bind_address.clone()))
        .default(SettingValue::String("127.0.0.1".to_string()))
        .section("DAP Server"),
);
items.push(
    SettingItem::new("dap.suppress_reload_on_pause", "Suppress Reload on Pause")
        .description("Suppress auto-reload while debugger is paused at a breakpoint")
        .value(SettingValue::Bool(settings.dap.suppress_reload_on_pause))
        .default(SettingValue::Bool(true))
        .section("DAP Server"),
);
```

#### 4. Settings Handler (`handler/settings.rs`)

Add `dap.*` arms to `apply_project_setting()` before the catch-all `_ =>`:

```rust
"dap.enabled" => {
    if let SettingValue::Bool(v) = &item.value {
        settings.dap.enabled = *v;
    }
}
"dap.auto_start_in_ide" => {
    if let SettingValue::Bool(v) = &item.value {
        settings.dap.auto_start_in_ide = *v;
    }
}
"dap.port" => {
    if let SettingValue::Number(v) = &item.value {
        settings.dap.port = *v as u16;
    }
}
"dap.bind_address" => {
    if let SettingValue::String(v) = &item.value {
        settings.dap.bind_address = v.clone();
    }
}
"dap.suppress_reload_on_pause" => {
    if let SettingValue::Bool(v) = &item.value {
        settings.dap.suppress_reload_on_pause = *v;
    }
}
```

### Acceptance Criteria

1. `DapSettings` compiles with `Debug`, `Clone`, `Serialize`, `Deserialize` derives
2. `Settings::default()` includes `dap: DapSettings::default()` with correct defaults (`enabled: false`, `auto_start_in_ide: true`, `port: 0`, `bind_address: "127.0.0.1"`, `suppress_reload_on_pause: true`)
3. A `.fdemon/config.toml` with a `[dap]` section deserializes correctly into `Settings.dap`
4. A `.fdemon/config.toml` with NO `[dap]` section still parses (defaults used via `#[serde(default)]`)
5. `project_settings_items()` returns items with `section = "DAP Server"` for all 5 DAP settings
6. Each DAP settings item has matching `id`, `description`, `value`, `default`, and `section`
7. `apply_project_setting()` correctly mutates `Settings.dap` for all 5 DAP setting IDs
8. Unknown DAP setting IDs fall through to the existing `_ =>` catch-all warning
9. `DapSettings` is re-exported from `fdemon_app::config::DapSettings`
10. `cargo check -p fdemon-app` passes
11. `cargo clippy -p fdemon-app -- -D warnings` clean

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dap_settings_defaults() {
        let settings = DapSettings::default();
        assert!(!settings.enabled);
        assert!(settings.auto_start_in_ide);
        assert_eq!(settings.port, 0);
        assert_eq!(settings.bind_address, "127.0.0.1");
        assert!(settings.suppress_reload_on_pause);
    }

    #[test]
    fn test_dap_settings_deserialize_from_toml() {
        let toml = r#"
            [dap]
            enabled = true
            port = 4711
            bind_address = "0.0.0.0"
        "#;
        // Parse as Settings and check dap fields
    }

    #[test]
    fn test_settings_without_dap_section_uses_defaults() {
        let toml = r#"
            [behavior]
            auto_start = true
        "#;
        let settings: Settings = toml::from_str(toml).unwrap();
        assert!(!settings.dap.enabled);
        assert!(settings.dap.auto_start_in_ide);
    }

    #[test]
    fn test_apply_dap_enabled_setting() {
        let mut settings = Settings::default();
        let item = SettingItem::new("dap.enabled", "Enabled")
            .value(SettingValue::Bool(true));
        apply_project_setting(&mut settings, &item);
        assert!(settings.dap.enabled);
    }

    #[test]
    fn test_apply_dap_port_setting() {
        let mut settings = Settings::default();
        let item = SettingItem::new("dap.port", "Port")
            .value(SettingValue::Number(8080));
        apply_project_setting(&mut settings, &item);
        assert_eq!(settings.dap.port, 8080);
    }
}
```

### Notes

- The `dap.bind_address` defaults to `"127.0.0.1"` for security — only localhost connections accepted. Users can set `"0.0.0.0"` for remote debugging (documented as a security risk).
- The `dap.port = 0` default means auto-assign — the OS picks an available port. This avoids port conflicts when multiple fdemon instances run simultaneously. Users wanting stable ports set a fixed value.
- `suppress_reload_on_pause` is a Phase 4 feature but the setting is defined now so the config schema is stable.
- Settings items use the `id` format `"section.field"` (e.g., `"dap.port"`) — this must match exactly between `settings_items.rs` and `handler/settings.rs`. There is no compile-time check; keep them in sync manually.
