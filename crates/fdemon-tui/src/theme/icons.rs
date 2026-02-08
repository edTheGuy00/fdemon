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
            IconMode::NerdFonts => "\u{f120}", // nf-fa-terminal
            IconMode::Unicode => "\u{276f}",   // ❯
        }
    }

    pub fn smartphone(&self) -> &'static str {
        match self.mode {
            IconMode::NerdFonts => "\u{f3cd}", // nf-fa-mobile
            IconMode::Unicode => "[M]",
        }
    }

    pub fn globe(&self) -> &'static str {
        match self.mode {
            IconMode::NerdFonts => "\u{f0ac}", // nf-fa-globe
            IconMode::Unicode => "[W]",
        }
    }

    pub fn monitor(&self) -> &'static str {
        match self.mode {
            IconMode::NerdFonts => "\u{f108}", // nf-fa-desktop
            IconMode::Unicode => "[D]",
        }
    }

    pub fn activity(&self) -> &'static str {
        match self.mode {
            IconMode::NerdFonts => "\u{f0f1}", // nf-fa-heartbeat
            IconMode::Unicode => "~",
        }
    }

    pub fn alert(&self) -> &'static str {
        match self.mode {
            IconMode::NerdFonts => "\u{f071}", // nf-fa-warning
            IconMode::Unicode => "\u{26a0}",   // ⚠
        }
    }

    pub fn cpu(&self) -> &'static str {
        match self.mode {
            IconMode::NerdFonts => "\u{f2db}", // nf-fa-microchip
            IconMode::Unicode => "[C]",
        }
    }

    // --- Phase indicator icons ---

    pub fn dot(&self) -> &'static str {
        match self.mode {
            IconMode::NerdFonts => "\u{f444}", // nf-oct-dot_fill
            IconMode::Unicode => "\u{25cf}",   // ●
        }
    }

    pub fn circle(&self) -> &'static str {
        match self.mode {
            IconMode::NerdFonts => "\u{f06e}", // nf-fa-eye
            IconMode::Unicode => "\u{25cb}",   // ○
        }
    }

    pub fn refresh(&self) -> &'static str {
        match self.mode {
            IconMode::NerdFonts => "\u{f021}", // nf-fa-refresh
            IconMode::Unicode => "\u{21bb}",   // ↻
        }
    }

    pub fn close(&self) -> &'static str {
        match self.mode {
            IconMode::NerdFonts => "\u{f00d}", // nf-fa-close
            IconMode::Unicode => "\u{2717}",   // ✗
        }
    }

    // --- Reserved for future use ---

    pub fn play(&self) -> &'static str {
        match self.mode {
            IconMode::NerdFonts => "\u{f04b}", // nf-fa-play
            IconMode::Unicode => "\u{25b6}",   // ▶
        }
    }

    pub fn stop(&self) -> &'static str {
        match self.mode {
            IconMode::NerdFonts => "\u{f04d}", // nf-fa-stop
            IconMode::Unicode => "\u{25a0}",   // ■
        }
    }

    pub fn check(&self) -> &'static str {
        match self.mode {
            IconMode::NerdFonts => "\u{f00c}", // nf-fa-check
            IconMode::Unicode => "\u{2713}",   // ✓
        }
    }

    pub fn chevron_right(&self) -> &'static str {
        match self.mode {
            IconMode::NerdFonts => "\u{f054}", // nf-fa-chevron_right
            IconMode::Unicode => "\u{203a}",   // ›
        }
    }

    pub fn chevron_down(&self) -> &'static str {
        match self.mode {
            IconMode::NerdFonts => "\u{f078}", // nf-fa-chevron_down
            IconMode::Unicode => "\u{2304}",   // ⌄
        }
    }

    pub fn settings(&self) -> &'static str {
        match self.mode {
            IconMode::NerdFonts => "\u{f013}", // nf-fa-cog
            IconMode::Unicode => "\u{2699}",   // ⚙
        }
    }

    pub fn info(&self) -> &'static str {
        match self.mode {
            IconMode::NerdFonts => "\u{f05a}", // nf-fa-info_circle
            IconMode::Unicode => "\u{2139}",   // ℹ
        }
    }

    pub fn layers(&self) -> &'static str {
        match self.mode {
            IconMode::NerdFonts => "\u{f5fd}", // nf-mdi-layers
            IconMode::Unicode => "\u{2261}",   // ≡
        }
    }

    pub fn command(&self) -> &'static str {
        match self.mode {
            IconMode::NerdFonts => "\u{f120}", // nf-fa-terminal
            IconMode::Unicode => "$",
        }
    }
}

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
