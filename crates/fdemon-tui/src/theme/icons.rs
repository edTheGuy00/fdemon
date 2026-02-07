//! Nerd Font glyph constants.
//!
//! Each icon has a Nerd Font variant and an ASCII fallback.
//! Use the `icon()` function to select based on configuration.

// Allow dead_code since these are infrastructure constants for Phase 2 migration
#![allow(dead_code)]

pub const ICON_TERMINAL: &str = "\u{f120}"; // nf-fa-terminal
pub const ICON_SMARTPHONE: &str = "\u{f3cd}"; // nf-fa-mobile
pub const ICON_GLOBE: &str = "\u{f0ac}"; // nf-fa-globe
pub const ICON_MONITOR: &str = "\u{f108}"; // nf-fa-desktop
pub const ICON_ACTIVITY: &str = "\u{f0f1}"; // nf-fa-heartbeat
pub const ICON_PLAY: &str = "\u{f04b}"; // nf-fa-play
pub const ICON_STOP: &str = "\u{f04d}"; // nf-fa-stop
pub const ICON_REFRESH: &str = "\u{f021}"; // nf-fa-refresh
pub const ICON_ALERT: &str = "\u{f071}"; // nf-fa-warning
pub const ICON_CHECK: &str = "\u{f00c}"; // nf-fa-check
pub const ICON_CLOSE: &str = "\u{f00d}"; // nf-fa-close
pub const ICON_CHEVRON_R: &str = "\u{f054}"; // nf-fa-chevron_right
pub const ICON_CHEVRON_D: &str = "\u{f078}"; // nf-fa-chevron_down
pub const ICON_DOT: &str = "\u{f444}"; // nf-oct-dot_fill
pub const ICON_LAYERS: &str = "\u{f5fd}"; // nf-mdi-layers
pub const ICON_CPU: &str = "\u{f2db}"; // nf-fa-microchip
pub const ICON_SETTINGS: &str = "\u{f013}"; // nf-fa-cog
pub const ICON_ZAP: &str = "\u{f0e7}"; // nf-fa-bolt
pub const ICON_EYE: &str = "\u{f06e}"; // nf-fa-eye
pub const ICON_CODE: &str = "\u{f121}"; // nf-fa-code
pub const ICON_USER: &str = "\u{f007}"; // nf-fa-user
pub const ICON_INFO: &str = "\u{f05a}"; // nf-fa-info_circle
pub const ICON_KEYBOARD: &str = "\u{f11c}"; // nf-fa-keyboard_o
pub const ICON_COMMAND: &str = "\u{f120}"; // nf-fa-terminal
pub const ICON_SAVE: &str = "\u{f0c7}"; // nf-fa-floppy_o

// --- ASCII fallbacks ---
pub const ASCII_TERMINAL: &str = ">";
pub const ASCII_SMARTPHONE: &str = "[M]";
pub const ASCII_GLOBE: &str = "[W]";
pub const ASCII_MONITOR: &str = "[D]";
pub const ASCII_ACTIVITY: &str = "~";
pub const ASCII_PLAY: &str = ">";
pub const ASCII_STOP: &str = "x";
pub const ASCII_REFRESH: &str = "@";
pub const ASCII_ALERT: &str = "!";
pub const ASCII_CHECK: &str = "*";
pub const ASCII_CLOSE: &str = "x";
pub const ASCII_CHEVRON_R: &str = ">";
pub const ASCII_CHEVRON_D: &str = "v";
pub const ASCII_DOT: &str = "*";
pub const ASCII_LAYERS: &str = "#";
pub const ASCII_CPU: &str = "[C]";
pub const ASCII_SETTINGS: &str = "*";
pub const ASCII_ZAP: &str = "!";
pub const ASCII_EYE: &str = "o";
pub const ASCII_CODE: &str = "</>";
pub const ASCII_USER: &str = "@";
pub const ASCII_INFO: &str = "(i)";
pub const ASCII_KEYBOARD: &str = "[K]";
pub const ASCII_COMMAND: &str = "$";
pub const ASCII_SAVE: &str = "[S]";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_icon_constants_are_non_empty() {
        assert!(!ICON_TERMINAL.is_empty());
        assert!(!ICON_SMARTPHONE.is_empty());
        assert!(!ICON_GLOBE.is_empty());
        assert!(!ICON_MONITOR.is_empty());
        assert!(!ICON_ACTIVITY.is_empty());
    }

    #[test]
    fn test_ascii_fallbacks_are_non_empty() {
        assert!(!ASCII_TERMINAL.is_empty());
        assert!(!ASCII_SMARTPHONE.is_empty());
        assert!(!ASCII_GLOBE.is_empty());
        assert!(!ASCII_MONITOR.is_empty());
        assert!(!ASCII_ACTIVITY.is_empty());
    }

    #[test]
    fn test_icon_play_stop_refresh_defined() {
        assert!(!ICON_PLAY.is_empty());
        assert!(!ICON_STOP.is_empty());
        assert!(!ICON_REFRESH.is_empty());
        assert!(!ASCII_PLAY.is_empty());
        assert!(!ASCII_STOP.is_empty());
        assert!(!ASCII_REFRESH.is_empty());
    }

    #[test]
    fn test_icon_navigation_defined() {
        assert!(!ICON_CHEVRON_R.is_empty());
        assert!(!ICON_CHEVRON_D.is_empty());
        assert!(!ASCII_CHEVRON_R.is_empty());
        assert!(!ASCII_CHEVRON_D.is_empty());
    }

    #[test]
    fn test_icon_status_defined() {
        assert!(!ICON_ALERT.is_empty());
        assert!(!ICON_CHECK.is_empty());
        assert!(!ICON_CLOSE.is_empty());
        assert!(!ASCII_ALERT.is_empty());
        assert!(!ASCII_CHECK.is_empty());
        assert!(!ASCII_CLOSE.is_empty());
    }

    #[test]
    fn test_all_icons_have_ascii_fallback() {
        // Verify that key icons have both Nerd Font and ASCII variants
        let icons = [
            (ICON_TERMINAL, ASCII_TERMINAL),
            (ICON_SMARTPHONE, ASCII_SMARTPHONE),
            (ICON_PLAY, ASCII_PLAY),
            (ICON_STOP, ASCII_STOP),
            (ICON_REFRESH, ASCII_REFRESH),
        ];

        for (nerd, ascii) in &icons {
            assert!(!nerd.is_empty(), "Nerd Font icon should not be empty");
            assert!(!ascii.is_empty(), "ASCII fallback should not be empty");
        }
    }
}
