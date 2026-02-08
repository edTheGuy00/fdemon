## Task: Update Configuration Documentation

**Objective**: Update `docs/CONFIGURATION.md` to document the new `icons` setting under `[ui]`, including the `FDEMON_ICONS` environment variable override, so users can discover and configure Nerd Font support.

**Depends on**: 01-add-icon-mode-config, 04-settings-panel

### Scope

- `docs/CONFIGURATION.md`: Add `icons` to UI Settings section, examples, and settings panel docs

### Details

#### 1. Update the UI Settings TOML example block

Add the `icons` field to the `[ui]` code block in the "UI Settings" section:

```toml
[ui]
icons = "unicode"               # Icon style: "unicode" (default) or "nerd_fonts"
log_buffer_size = 10000
# ... existing fields
```

#### 2. Add row to UI Settings property table

Add a new row to the `| Property | Type | Default | Description |` table:

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `icons` | `string` | `"unicode"` | Icon style for the TUI. `"unicode"` uses safe characters that work in all terminals. `"nerd_fonts"` uses Nerd Font glyphs (requires a [Nerd Font](https://www.nerdfonts.com/) installed in your terminal). |

#### 3. Add environment variable override note

After the UI Settings table, add a note:

> **Environment variable override:** Set `FDEMON_ICONS=nerd_fonts` or `FDEMON_ICONS=unicode` to override the config file setting for the current session.

#### 4. Update the complete config.toml example

In the "Complete `.fdemon/config.toml` Example" section, add `icons = "unicode"` to the `[ui]` block.

#### 5. Update the Settings Panel — Editing Settings — Enums section

In the Enums editing section, add `icons` to the example list:

> Example: `mode` (debug/profile/release), `theme`, `icons` (unicode/nerd_fonts)

#### 6. Update the User Preferences available overrides

Add `icons` to the list of user-overridable settings if applicable (users may want different icon settings per machine depending on their terminal font).

### Acceptance Criteria

1. The `icons` setting is documented in the UI Settings section with correct type, default, and description
2. The `FDEMON_ICONS` environment variable override is documented
3. The complete `config.toml` example includes the `icons` field
4. The settings panel documentation mentions `icons` as an Enum type setting
5. All existing documentation links and cross-references remain intact
6. Documentation accurately reflects the implementation from tasks 01 and 04

### Notes

- This task should be done **after** tasks 01 (config type) and 04 (settings panel) so the documentation matches the actual implementation
- Keep the documentation style consistent with existing sections (TOML blocks, property tables, notes)
- The `FDEMON_ICONS` env var is an important discoverability surface — make sure it's prominent

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `docs/CONFIGURATION.md` | Added `icons` field to UI Settings TOML example, property table row, environment variable override note, complete config.toml example, Settings Panel Enum editing section, and User Preferences available overrides list |

### Notable Decisions/Tradeoffs

1. **Environment Variable Prominence**: Placed the `FDEMON_ICONS` environment variable override note immediately after the UI Settings property table, making it highly visible as requested in the task.
2. **User Preferences Addition**: Added `icons` to the User Preferences available overrides list, as users may want different icon settings per machine depending on their terminal font capabilities (exactly as noted in the task).
3. **Documentation Style Consistency**: Maintained exact formatting patterns from existing sections (TOML blocks, property tables, blockquote notes) to ensure consistency throughout the documentation.
4. **Nerd Font Link**: Included clickable link to nerdfonts.com in the description to help users discover and install Nerd Fonts.

### Testing Performed

- Verified all documentation additions match the actual implementation from tasks 01 and 04
- Checked `IconMode` enum implementation in `crates/fdemon-app/src/config/types.rs` (lines 196-217)
- Verified `FDEMON_ICONS` environment variable override in `crates/fdemon-app/src/config/settings.rs` (line 334)
- Confirmed settings panel implementation includes `icons` as an Enum setting
- Verified all existing documentation links and cross-references remain intact
- Confirmed TOML example syntax is valid

### Risks/Limitations

None. All changes are documentation-only and accurately reflect the existing implementation.
