## Task: Create IconSet Struct

**Objective**: Replace the dual `ICON_*`/`NERD_*` static constants in `icons.rs` with an `IconSet` struct that resolves icons at runtime based on `IconMode`.

**Depends on**: None (uses `IconMode` from `fdemon-app`, but can be coded against it independently)

### Scope

- `crates/fdemon-tui/src/theme/icons.rs`: Replace all constants with `IconSet` struct
- `crates/fdemon-tui/src/theme/mod.rs`: Update module docstring

### Details

**1. Replace `icons.rs` with `IconSet` struct**

Replace the entire module with:

```rust
//! Icon set for the TUI.
//!
//! Provides `IconSet` which resolves icons at runtime based on `IconMode`.
//! - `IconMode::Unicode` — safe characters that work in all terminals
//! - `IconMode::NerdFonts` — rich Nerd Font glyphs (requires Nerd Font installed)

use fdemon_app::config::IconMode;

/// Runtime icon resolver.
///
/// Created from `IconMode`, returns the appropriate icon string for each
/// icon slot based on the configured mode.
#[derive(Debug, Clone, Copy)]
pub struct IconSet {
    mode: IconMode,
}

impl IconSet {
    pub fn new(mode: IconMode) -> Self {
        Self { mode }
    }

    pub fn terminal(&self) -> &'static str {
        match self.mode {
            IconMode::NerdFonts => "\u{f120}",  // nf-fa-terminal
            IconMode::Unicode   => "\u{276f}",  // ❯
        }
    }

    pub fn smartphone(&self) -> &'static str {
        match self.mode {
            IconMode::NerdFonts => "\u{f3cd}",  // nf-fa-mobile
            IconMode::Unicode   => "[M]",
        }
    }

    pub fn globe(&self) -> &'static str {
        match self.mode {
            IconMode::NerdFonts => "\u{f0ac}",  // nf-fa-globe
            IconMode::Unicode   => "[W]",
        }
    }

    pub fn monitor(&self) -> &'static str {
        match self.mode {
            IconMode::NerdFonts => "\u{f108}",  // nf-fa-desktop
            IconMode::Unicode   => "[D]",
        }
    }

    pub fn activity(&self) -> &'static str {
        match self.mode {
            IconMode::NerdFonts => "\u{f0f1}",  // nf-fa-heartbeat
            IconMode::Unicode   => "~",
        }
    }

    pub fn alert(&self) -> &'static str {
        match self.mode {
            IconMode::NerdFonts => "\u{f071}",  // nf-fa-warning
            IconMode::Unicode   => "\u{26a0}",  // ⚠
        }
    }

    pub fn cpu(&self) -> &'static str {
        match self.mode {
            IconMode::NerdFonts => "\u{f2db}",  // nf-fa-microchip
            IconMode::Unicode   => "[C]",
        }
    }

    // --- Phase indicator icons ---

    pub fn dot(&self) -> &'static str {
        match self.mode {
            IconMode::NerdFonts => "\u{f444}",  // nf-oct-dot_fill
            IconMode::Unicode   => "\u{25cf}",  // ●
        }
    }

    pub fn circle(&self) -> &'static str {
        match self.mode {
            IconMode::NerdFonts => "\u{f06e}",  // nf-fa-eye
            IconMode::Unicode   => "\u{25cb}",  // ○
        }
    }

    pub fn refresh(&self) -> &'static str {
        match self.mode {
            IconMode::NerdFonts => "\u{f021}",  // nf-fa-refresh
            IconMode::Unicode   => "\u{21bb}",  // ↻
        }
    }

    pub fn close(&self) -> &'static str {
        match self.mode {
            IconMode::NerdFonts => "\u{f00d}",  // nf-fa-close
            IconMode::Unicode   => "\u{2717}",  // ✗
        }
    }

    // --- Reserved for future use ---

    pub fn play(&self) -> &'static str {
        match self.mode {
            IconMode::NerdFonts => "\u{f04b}",  // nf-fa-play
            IconMode::Unicode   => "\u{25b6}",  // ▶
        }
    }

    pub fn stop(&self) -> &'static str {
        match self.mode {
            IconMode::NerdFonts => "\u{f04d}",  // nf-fa-stop
            IconMode::Unicode   => "\u{25a0}",  // ■
        }
    }

    pub fn check(&self) -> &'static str {
        match self.mode {
            IconMode::NerdFonts => "\u{f00c}",  // nf-fa-check
            IconMode::Unicode   => "\u{2713}",  // ✓
        }
    }

    pub fn chevron_right(&self) -> &'static str {
        match self.mode {
            IconMode::NerdFonts => "\u{f054}",  // nf-fa-chevron_right
            IconMode::Unicode   => "\u{203a}",  // ›
        }
    }

    pub fn chevron_down(&self) -> &'static str {
        match self.mode {
            IconMode::NerdFonts => "\u{f078}",  // nf-fa-chevron_down
            IconMode::Unicode   => "\u{2304}",  // ⌄
        }
    }

    pub fn settings(&self) -> &'static str {
        match self.mode {
            IconMode::NerdFonts => "\u{f013}",  // nf-fa-cog
            IconMode::Unicode   => "\u{2699}",  // ⚙
        }
    }

    pub fn info(&self) -> &'static str {
        match self.mode {
            IconMode::NerdFonts => "\u{f05a}",  // nf-fa-info_circle
            IconMode::Unicode   => "\u{2139}",  // ℹ
        }
    }

    pub fn layers(&self) -> &'static str {
        match self.mode {
            IconMode::NerdFonts => "\u{f5fd}",  // nf-mdi-layers
            IconMode::Unicode   => "\u{2261}",  // ≡
        }
    }

    pub fn command(&self) -> &'static str {
        match self.mode {
            IconMode::NerdFonts => "\u{f120}",  // nf-fa-terminal
            IconMode::Unicode   => "$",
        }
    }
}
```

**2. Update `theme/mod.rs` docstring**

Change line 6 from:
```
//! - `icons` — Nerd Font glyph constants with ASCII fallbacks
```
To:
```
//! - `icons` — `IconSet` for runtime icon resolution (Unicode/Nerd Font)
```

**Key design decisions:**
- `IconSet` is `Copy` + `Clone` — it's just an enum wrapper, cheap to pass around
- Methods return `&'static str` — no allocations, same as the old constants
- Method names are lowercase without `ICON_` prefix (e.g., `icons.terminal()` instead of `icons::ICON_TERMINAL`)
- All Nerd Font codepoints are v3 (matches existing `NERD_*` constants)
- Unicode values match the existing `ICON_*` constants exactly
- Dead-code items (play, stop, check, etc.) are kept as methods without `#[allow(dead_code)]` — they'll be consumed when widgets use them

### Acceptance Criteria

1. `IconSet::new(IconMode::Unicode)` returns safe Unicode for all methods
2. `IconSet::new(IconMode::NerdFonts)` returns Nerd Font glyphs for all methods
3. All existing icon values are preserved (Unicode values unchanged from current `ICON_*` constants)
4. All existing Nerd Font values are preserved (unchanged from current `NERD_*` constants)
5. `IconSet` derives `Debug`, `Clone`, `Copy`
6. No `ICON_*` or `NERD_*` static constants remain in the module
7. `cargo check -p fdemon-tui` passes (expect compile errors in `header.rs` and `log_view/mod.rs` — these are resolved in task 03)

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use fdemon_app::config::IconMode;

    #[test]
    fn test_unicode_icons_are_non_empty() {
        let icons = IconSet::new(IconMode::Unicode);
        assert!(!icons.terminal().is_empty());
        assert!(!icons.smartphone().is_empty());
        assert!(!icons.globe().is_empty());
        assert!(!icons.monitor().is_empty());
        assert!(!icons.activity().is_empty());
        assert!(!icons.alert().is_empty());
        assert!(!icons.cpu().is_empty());
    }

    #[test]
    fn test_nerd_font_icons_are_non_empty() {
        let icons = IconSet::new(IconMode::NerdFonts);
        assert!(!icons.terminal().is_empty());
        assert!(!icons.smartphone().is_empty());
        assert!(!icons.globe().is_empty());
        assert!(!icons.monitor().is_empty());
        assert!(!icons.activity().is_empty());
        assert!(!icons.alert().is_empty());
        assert!(!icons.cpu().is_empty());
    }

    #[test]
    fn test_unicode_and_nerd_font_differ() {
        let unicode = IconSet::new(IconMode::Unicode);
        let nerd = IconSet::new(IconMode::NerdFonts);
        // At least the main icons should differ between modes
        assert_ne!(unicode.terminal(), nerd.terminal());
        assert_ne!(unicode.smartphone(), nerd.smartphone());
        assert_ne!(unicode.alert(), nerd.alert());
    }

    #[test]
    fn test_terminal_and_command_are_distinct() {
        let icons = IconSet::new(IconMode::Unicode);
        assert_ne!(icons.terminal(), icons.command());
    }

    #[test]
    fn test_phase_indicator_icons() {
        let icons = IconSet::new(IconMode::Unicode);
        assert_eq!(icons.dot(), "●");
        assert_eq!(icons.circle(), "○");
        assert_eq!(icons.refresh(), "↻");
        assert_eq!(icons.close(), "✗");
    }

    #[test]
    fn test_icon_set_is_copy() {
        let icons = IconSet::new(IconMode::Unicode);
        let copy = icons;
        assert_eq!(icons.terminal(), copy.terminal());
    }
}
```

### Notes

- This task will cause compile errors in `header.rs` and `log_view/mod.rs` because they still reference `icons::ICON_*` constants. These are resolved in task 03.
- The `IconSet` depends on `IconMode` from `fdemon-app`. Since `fdemon-tui` already depends on `fdemon-app`, no new crate dependency is needed.
- Methods that are currently unused will get `#[allow(dead_code)]` removed — they exist as part of the `IconSet` API and may be used in future widgets.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/theme/icons.rs` | Replaced all `ICON_*` and `NERD_*` static constants with `IconSet` struct that resolves icons at runtime based on `IconMode`. Includes 6 unit tests. |
| `crates/fdemon-tui/src/theme/mod.rs` | Updated module docstring to describe `icons` as "`IconSet` for runtime icon resolution (Unicode/Nerd Font)" |
| `crates/fdemon-app/src/config/mod.rs` | Added `IconMode` to public exports (completed by task 01) |

### Notable Decisions/Tradeoffs

1. **IconSet is Copy + Clone**: Struct is just an enum wrapper (1 byte), making it cheap to pass around by value
2. **Methods return &'static str**: No allocations, same performance as the old constants
3. **Method names are lowercase**: Changed from `ICON_TERMINAL` constant to `icons.terminal()` method for better ergonomics
4. **All icon methods public without dead_code**: Removed `#[allow(dead_code)]` since they're part of the public API
5. **IconMode export location**: `IconMode` was already exported from `fdemon_app::config` by task 01, allowing proper import

### Testing Performed

- `cargo check -p fdemon-tui` - Expected compile errors in `header.rs` (4 errors) and `log_view/mod.rs` (5 errors) due to references to old `ICON_*` constants
- Unit tests written (6 tests) but cannot be executed until task 03 fixes the consumer errors
- Tests verify:
  - Unicode icons are non-empty
  - Nerd Font icons are non-empty
  - Unicode and Nerd Font modes differ
  - Terminal and command icons are distinct
  - Phase indicator icon values match original constants
  - IconSet is Copy

### Risks/Limitations

1. **Compile Errors Expected**: The crate won't compile until task 03 updates `header.rs` and `log_view/mod.rs` to use the new `IconSet` API
2. **Test Execution Blocked**: Unit tests cannot be run until consumers are updated
3. **No Breaking Changes to Values**: All Unicode and Nerd Font icon values are preserved exactly from the original constants
