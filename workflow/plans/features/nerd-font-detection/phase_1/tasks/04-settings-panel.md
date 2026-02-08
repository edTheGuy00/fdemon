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

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/settings_items.rs` | Added `ui.icons` setting item to `project_settings_items()` function in the UI section, placed after `ui.theme` (lines 132-147) |
| `crates/fdemon-app/src/handler/settings.rs` | Added `"ui.icons"` case to `apply_project_setting()` function to parse enum value and apply `IconMode` (lines 65-72) |
| `crates/fdemon-app/src/handler/settings_handlers.rs` | Updated `get_item_count_for_tab()` to reflect the new setting item count (16 -> 17) for Project tab |

### Notable Decisions/Tradeoffs

1. **Placement in UI section**: The `ui.icons` setting was placed immediately after `ui.theme` in the UI section, which groups all visual presentation settings together and makes the icon setting easily discoverable near the theme setting.

2. **Enum pattern matching**: The handler uses explicit pattern matching (`"nerd_fonts"` -> `IconMode::NerdFonts`, `"unicode"` -> `IconMode::Unicode`) with a fallback to `Unicode` for unknown values, ensuring safe degradation if an invalid value is encountered.

3. **Item count update**: Updated the hardcoded item count in `get_item_count_for_tab()` to maintain accurate navigation bounds in the settings panel.

### Testing Performed

- `cargo check -p fdemon-app` - Passed
- `cargo clippy -p fdemon-app -- -D warnings` - Passed (no warnings)
- `cargo test -p fdemon-app --lib` - Passed (747 tests passed, 0 failed, 5 ignored)

All acceptance criteria met:
1. `ui.icons` setting item is added to `project_settings_items()`
2. Setting value is derived from `settings.ui.icons.to_string()`
3. Enum cycling is handled by existing enum handler logic
4. Changes are applied via `apply_project_setting()` and persist through `save_settings()`
5. Setting takes effect immediately (no restart required) since widgets construct `IconSet` from live state
6. All compilation and lint checks pass

### Risks/Limitations

1. **Test isolation**: During testing, encountered one transient test failure related to environment variable isolation (`test_fdemon_icons_env_var_case_insensitive`). The test passes when run in isolation and consistently passes on subsequent full test runs, suggesting a minor test ordering sensitivity. This does not affect production behavior and is a known pattern with environment variable tests.

2. **No validation for Nerd Font installation**: The setting allows users to select `nerd_fonts` without verifying that a Nerd Font is actually installed. This is by design (matching the task requirements), but users who select this option without the proper font will see broken glyph rendering. The description text warns users about this requirement.
