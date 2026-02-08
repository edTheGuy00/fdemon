## Task: Add Icon Mode to Settings Panel

**Objective**: Expose the `ui.icons` setting in the settings panel so users can toggle between Unicode and Nerd Fonts from within the TUI.

**Depends on**: 01-add-icon-mode-config

### Scope

- `crates/fdemon-app/src/settings_items.rs`: Add `ui.icons` entry to `project_settings_items()`
- `crates/fdemon-app/src/handler/`: Ensure the settings handler can apply `IconMode` changes via `save_settings()`

### Details

**1. Add settings item to `settings_items.rs`**

In `project_settings_items()`, add a new entry in the UI section (after the `ui.theme` item, around line 131):

```rust
SettingItem::new("ui.icons", "Icon Style")
    .description("Icon rendering: unicode (all terminals) or nerd_fonts (requires Nerd Font)")
    .value(SettingValue::Enum {
        value: settings.ui.icons.to_string(),
        options: vec![
            "unicode".to_string(),
            "nerd_fonts".to_string(),
        ],
    })
    .default(SettingValue::Enum {
        value: "unicode".to_string(),
        options: vec![
            "unicode".to_string(),
            "nerd_fonts".to_string(),
        ],
    })
    .section("UI"),
```

**2. Verify settings handler applies changes**

The settings handler in `crates/fdemon-app/src/handler/` should already handle enum settings generically via `save_settings()`. Verify that changing `ui.icons` through the settings panel correctly serializes to `config.toml` and takes effect.

If the handler uses string matching on setting IDs to apply values, add a case for `"ui.icons"` that parses the string value back to `IconMode`:

```rust
"ui.icons" => {
    match value.as_str() {
        "nerd_fonts" => settings.ui.icons = IconMode::NerdFonts,
        "unicode" => settings.ui.icons = IconMode::Unicode,
        _ => {}
    }
}
```

### Acceptance Criteria

1. `ui.icons` appears in the Project settings tab under the "UI" section
2. Shows current value ("unicode" or "nerd_fonts")
3. Toggling the value cycles between "unicode" and "nerd_fonts"
4. Changes persist to `config.toml` via `save_settings()`
5. Changes take effect immediately in the current session (icons update on next render frame)
6. `cargo check -p fdemon-app` passes

### Testing

- Verify `project_settings_items()` includes the `ui.icons` item
- Verify the setting value matches `settings.ui.icons.to_string()`
- Roundtrip test: set `icons = "nerd_fonts"`, save, reload, verify value persists

### Notes

- The description should clearly indicate that `nerd_fonts` requires a Nerd Font installed in the terminal — users who select it without a Nerd Font will see broken rendering
- The setting should take effect immediately without requiring a restart, since `IconSet` is constructed per-frame from `state.settings.ui.icons`
