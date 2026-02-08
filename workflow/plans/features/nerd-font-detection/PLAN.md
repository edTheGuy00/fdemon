# Plan: Dynamic Nerd Font Detection with Unicode Fallback

## TL;DR

Add a configuration-driven icon mode (`icons` setting in `UiSettings`) that defaults to safe Unicode but allows users to opt-in to Nerd Font icons via `config.toml` or environment variable. No reliable automatic font detection exists in terminal applications, so we follow the industry-standard pattern (starship, lazygit) of user configuration with sensible defaults.

---

## Background

The project currently has two parallel sets of icon constants in `icons.rs`:
- **`ICON_*`** constants: Safe Unicode characters (used in production)
- **`NERD_*`** constants: Nerd Font glyphs (dead code, reserved for future opt-in)

These were created during the phase 2 fixes (task `02-fix-nerd-font-icons`) which replaced Nerd Font glyphs with safe Unicode after they rendered as `?`/tofu in terminals without Nerd Fonts (e.g., Zed integrated terminal).

**Problem**: The `NERD_*` constants exist but there is no mechanism to use them. Users with Nerd Fonts installed get the degraded Unicode experience unnecessarily.

**Research Findings**: There is **no reliable programmatic way** to detect Nerd Font availability at runtime. Terminal emulators don't expose font information through standard query mechanisms. The Nerd Fonts maintainers [confirm](https://github.com/ryanoasis/nerd-fonts/discussions/829) there is "no general way" to detect availability. All major Rust TUI tools (starship, lazygit, eza, lsd) use **user configuration** rather than automatic detection.

---

## Affected Modules

- `crates/fdemon-app/src/config/types.rs` - Add `IconMode` enum and `icons` field to `UiSettings`
- `crates/fdemon-app/src/settings_items.rs` - Add settings panel entry for icon mode
- `crates/fdemon-tui/src/theme/icons.rs` - Replace static constants with `IconSet` struct and `icon()` accessor function
- `crates/fdemon-tui/src/theme/mod.rs` - Re-export `IconSet`
- `crates/fdemon-tui/src/widgets/header.rs` - Pass `IconSet` to `device_icon_for_platform()`
- `crates/fdemon-tui/src/widgets/log_view/mod.rs` - Pass `IconSet` to icon consumers
- `crates/fdemon-tui/src/theme/styles.rs` - Use `IconSet` for phase indicators (deduplicate inline literals)

---

## Design

### Icon Resolution Strategy

```
Priority order:
1. Environment variable: FDEMON_ICONS=nerd_fonts|unicode|ascii
2. Config file: .fdemon/config.toml → [ui] icons = "nerd_fonts"
3. Default: "unicode" (safe for all terminals)
```

### IconMode Enum (in `fdemon-app/config/types.rs`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IconMode {
    /// Nerd Font icons (requires Nerd Font installed in terminal)
    NerdFonts,
    /// Safe Unicode characters (works in all terminals)
    #[default]
    Unicode,
}
```

### IconSet Struct (in `fdemon-tui/theme/icons.rs`)

Replace the current static constants with a runtime-resolved struct:

```rust
pub struct IconSet {
    mode: IconMode,
}

impl IconSet {
    pub fn new(mode: IconMode) -> Self { Self { mode } }

    pub fn terminal(&self) -> &'static str {
        match self.mode {
            IconMode::NerdFonts => "\u{f120}",  // nf-fa-terminal
            IconMode::Unicode   => "❯",
        }
    }

    pub fn smartphone(&self) -> &'static str {
        match self.mode {
            IconMode::NerdFonts => "\u{f3cd}",  // nf-fa-mobile
            IconMode::Unicode   => "[M]",
        }
    }
    // ... etc for all icons
}
```

### Config Integration

```toml
# .fdemon/config.toml
[ui]
# Icon style: "unicode" (default, works everywhere) or "nerd_fonts" (requires Nerd Font)
icons = "nerd_fonts"
```

### Environment Variable Override

```bash
# Override config for this session
FDEMON_ICONS=nerd_fonts fdemon
```

---

## Development Phases

### Phase 1: Core Icon Infrastructure

**Goal**: Add `IconMode` to config, create `IconSet`, wire it through rendering.

#### Steps

1. **Add `IconMode` enum to config types**
   - Add `IconMode` enum to `crates/fdemon-app/src/config/types.rs`
   - Add `icons: IconMode` field to `UiSettings` with `#[serde(default)]`
   - Add env var override in settings loading (check `FDEMON_ICONS`)

2. **Create `IconSet` in `icons.rs`**
   - Replace dual `ICON_*`/`NERD_*` static constants with `IconSet` struct
   - Each icon is a method that returns `&'static str` based on the mode
   - Keep raw constants as private for the match arms
   - `IconSet` is `Clone`, `Copy`-friendly (it's just an enum wrapper)

3. **Wire `IconSet` through TUI rendering**
   - Create `IconSet` from `AppState.settings.ui.icons` at render time
   - Pass to `header.rs` `device_icon_for_platform()`
   - Pass to `log_view/mod.rs` icon consumers
   - Update `styles.rs` phase indicators to use `IconSet` (dedup inline literals)

4. **Add settings panel entry**
   - Add `ui.icons` to `settings_items.rs` as an Enum setting
   - Options: `["unicode", "nerd_fonts"]`

5. **Update tests**
   - Update existing icon tests to test both modes
   - Add tests for `IconSet` method correctness
   - Add config deserialization tests for `IconMode`

**Milestone**: Users can set `icons = "nerd_fonts"` in config.toml and see Nerd Font icons, or leave default for safe Unicode.

---

## Edge Cases & Risks

### Terminal Compatibility
- **Risk**: Nerd Font icons still render as tofu if user sets `nerd_fonts` without having fonts installed
- **Mitigation**: This is user opt-in only. Document the requirement clearly. Default is always safe Unicode.

### Width Calculations
- **Risk**: Nerd Font glyphs are single-width but some Unicode replacements like `[M]`, `[W]`, `[D]` are multi-character. Switching modes could affect layout.
- **Mitigation**: Nerd Font icons are all single-cell width. The multi-char brackets `[M]` are 3 chars. Width calculations that depend on icon length must use the actual icon string length, not assume a fixed width. Verify layout in both modes.

### Backward Compatibility
- **Risk**: Changing from static constants to `IconSet` methods breaks all import sites.
- **Mitigation**: Limited impact — only 2 files consume icons (`header.rs`, `log_view/mod.rs`). The refactor is mechanical.

### Nerd Fonts v2 vs v3
- **Risk**: Nerd Fonts v2 and v3 use different codepoints for some icons.
- **Mitigation**: Start with v3 codepoints only (current standard). v2 support can be added later if demanded. The existing `NERD_*` constants already use v3 codepoints.

---

## Configuration Additions

```toml
# .fdemon/config.toml
[ui]
# Icon style for the TUI
# Options: "unicode" (default), "nerd_fonts"
# "unicode" - Safe characters that work in all terminals
# "nerd_fonts" - Rich icons (requires a Nerd Font installed in your terminal)
icons = "unicode"
```

Environment variable override:
```bash
FDEMON_ICONS=nerd_fonts   # or "unicode"
```

---

## Success Criteria

### Phase 1 Complete When:
- [ ] `IconMode` enum exists in config types with serde support
- [ ] `IconSet` struct replaces dual static constants in `icons.rs`
- [ ] `icons = "nerd_fonts"` in config.toml activates Nerd Font glyphs
- [ ] `FDEMON_ICONS` env var overrides config setting
- [ ] Default behavior (no config) renders safe Unicode (unchanged from current)
- [ ] Settings panel shows icon mode as editable enum
- [ ] Phase indicators in `styles.rs` use `IconSet` (no more inline literals)
- [ ] All existing tests pass, new tests cover both modes
- [ ] `cargo check --workspace` passes
- [ ] `cargo clippy --workspace -- -D warnings` passes
- [ ] Visual verification: Nerd Font mode renders correctly in Ghostty
- [ ] Visual verification: Unicode mode renders correctly in Zed terminal

---

## Future Enhancements

- **Auto-detection heuristic**: An `"auto"` mode that checks `TERM_PROGRAM` to guess if the terminal likely has Nerd Fonts (e.g., Ghostty, WezTerm, Kitty → assume yes). This is unreliable but could be a convenience feature.
- **Nerd Fonts v2 support**: Add `nerd_fonts_v2` mode for users on older font versions.
- **Per-icon overrides**: Allow individual icon customization in config (advanced users).
- **Icon preview in settings**: Show a sample icon in the settings panel to verify rendering.

---

## References

- [Nerd Fonts: No general detection method](https://github.com/ryanoasis/nerd-fonts/discussions/829)
- [Starship: Config-driven presets, no auto-detection](https://starship.rs/presets/no-nerd-font)
- [lazygit: `nerdFontsVersion` config option](https://github.com/jesseduffield/lazygit/blob/master/docs/Config.md)
- Task `02-fix-nerd-font-icons.md`: Prior work replacing Nerd Fonts with safe Unicode
