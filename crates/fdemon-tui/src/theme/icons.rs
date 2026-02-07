//! Icon constants for the TUI.
//!
//! All `ICON_*` constants use universally-supported Unicode characters.
//! Original Nerd Font glyphs are preserved as `NERD_*` constants for future opt-in.

// --- Safe Unicode Icons (default) ---
pub const ICON_TERMINAL: &str = "❯"; // Terminal prompt indicator
pub const ICON_SMARTPHONE: &str = "[M]"; // Mobile device
pub const ICON_GLOBE: &str = "[W]"; // Web device
pub const ICON_MONITOR: &str = "[D]"; // Desktop device
pub const ICON_ACTIVITY: &str = "~"; // Uptime/activity indicator

// The following icons are kept for future config opt-in (user preference to show more UI icons).
// Once a settings panel allows "show icons for phase indicators", these will be consumed.
#[allow(dead_code)]
pub const ICON_PLAY: &str = "▶"; // Play/running
#[allow(dead_code)]
pub const ICON_STOP: &str = "■"; // Stopped
#[allow(dead_code)]
pub const ICON_REFRESH: &str = "↻"; // Reload/refresh
pub const ICON_ALERT: &str = "⚠"; // Warning/error
#[allow(dead_code)]
pub const ICON_CHECK: &str = "✓"; // Success
#[allow(dead_code)]
pub const ICON_CLOSE: &str = "✗"; // Close/error
#[allow(dead_code)]
pub const ICON_CHEVRON_R: &str = "›"; // Right chevron
#[allow(dead_code)]
pub const ICON_CHEVRON_D: &str = "⌄"; // Down chevron
#[allow(dead_code)]
pub const ICON_DOT: &str = "●"; // Dot indicator
#[allow(dead_code)]
pub const ICON_LAYERS: &str = "≡"; // Layers/stack
pub const ICON_CPU: &str = "[C]"; // Generic device fallback
#[allow(dead_code)]
pub const ICON_SETTINGS: &str = "⚙"; // Settings/config
#[allow(dead_code)]
pub const ICON_ZAP: &str = "!"; // Lightning/fast (using safe single-width)
#[allow(dead_code)]
pub const ICON_EYE: &str = "○"; // Visibility/watch (using safe single-width)
#[allow(dead_code)]
pub const ICON_CODE: &str = "</>"; // Code/source
#[allow(dead_code)]
pub const ICON_USER: &str = "@"; // User/profile (using safe single-width)
#[allow(dead_code)]
pub const ICON_INFO: &str = "ℹ"; // Information
#[allow(dead_code)]
pub const ICON_KEYBOARD: &str = "[K]"; // Keyboard input
#[allow(dead_code)]
pub const ICON_COMMAND: &str = "$"; // Command/shell prompt (distinct from TERMINAL)
#[allow(dead_code)]
pub const ICON_SAVE: &str = "[S]"; // Save/disk

// --- Nerd Font Icons (for future opt-in) ---
// All NERD_* constants are intentionally unused until a user config option enables Nerd Fonts.
// This preserves the full icon set for future Phase 2+ opt-in (e.g., `use_nerd_fonts = true`).
#[allow(dead_code)]
pub const NERD_TERMINAL: &str = "\u{f120}"; // nf-fa-terminal
#[allow(dead_code)]
pub const NERD_SMARTPHONE: &str = "\u{f3cd}"; // nf-fa-mobile
#[allow(dead_code)]
pub const NERD_GLOBE: &str = "\u{f0ac}"; // nf-fa-globe
#[allow(dead_code)]
pub const NERD_MONITOR: &str = "\u{f108}"; // nf-fa-desktop
#[allow(dead_code)]
pub const NERD_ACTIVITY: &str = "\u{f0f1}"; // nf-fa-heartbeat
#[allow(dead_code)]
pub const NERD_PLAY: &str = "\u{f04b}"; // nf-fa-play
#[allow(dead_code)]
pub const NERD_STOP: &str = "\u{f04d}"; // nf-fa-stop
#[allow(dead_code)]
pub const NERD_REFRESH: &str = "\u{f021}"; // nf-fa-refresh
#[allow(dead_code)]
pub const NERD_ALERT: &str = "\u{f071}"; // nf-fa-warning
#[allow(dead_code)]
pub const NERD_CHECK: &str = "\u{f00c}"; // nf-fa-check
#[allow(dead_code)]
pub const NERD_CLOSE: &str = "\u{f00d}"; // nf-fa-close
#[allow(dead_code)]
pub const NERD_CHEVRON_R: &str = "\u{f054}"; // nf-fa-chevron_right
#[allow(dead_code)]
pub const NERD_CHEVRON_D: &str = "\u{f078}"; // nf-fa-chevron_down
#[allow(dead_code)]
pub const NERD_DOT: &str = "\u{f444}"; // nf-oct-dot_fill
#[allow(dead_code)]
pub const NERD_LAYERS: &str = "\u{f5fd}"; // nf-mdi-layers
#[allow(dead_code)]
pub const NERD_CPU: &str = "\u{f2db}"; // nf-fa-microchip
#[allow(dead_code)]
pub const NERD_SETTINGS: &str = "\u{f013}"; // nf-fa-cog
#[allow(dead_code)]
pub const NERD_ZAP: &str = "\u{f0e7}"; // nf-fa-bolt
#[allow(dead_code)]
pub const NERD_EYE: &str = "\u{f06e}"; // nf-fa-eye
#[allow(dead_code)]
pub const NERD_CODE: &str = "\u{f121}"; // nf-fa-code
#[allow(dead_code)]
pub const NERD_USER: &str = "\u{f007}"; // nf-fa-user
#[allow(dead_code)]
pub const NERD_INFO: &str = "\u{f05a}"; // nf-fa-info_circle
#[allow(dead_code)]
pub const NERD_KEYBOARD: &str = "\u{f11c}"; // nf-fa-keyboard_o
#[allow(dead_code)]
pub const NERD_COMMAND: &str = "\u{f120}"; // nf-fa-terminal (same as terminal)
#[allow(dead_code)]
pub const NERD_SAVE: &str = "\u{f0c7}"; // nf-fa-floppy_o

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
    fn test_nerd_font_constants_are_non_empty() {
        assert!(!NERD_TERMINAL.is_empty());
        assert!(!NERD_SMARTPHONE.is_empty());
        assert!(!NERD_GLOBE.is_empty());
        assert!(!NERD_MONITOR.is_empty());
        assert!(!NERD_ACTIVITY.is_empty());
    }

    #[test]
    fn test_icon_play_stop_refresh_defined() {
        assert!(!ICON_PLAY.is_empty());
        assert!(!ICON_STOP.is_empty());
        assert!(!ICON_REFRESH.is_empty());
        assert!(!NERD_PLAY.is_empty());
        assert!(!NERD_STOP.is_empty());
        assert!(!NERD_REFRESH.is_empty());
    }

    #[test]
    fn test_icon_navigation_defined() {
        assert!(!ICON_CHEVRON_R.is_empty());
        assert!(!ICON_CHEVRON_D.is_empty());
        assert!(!NERD_CHEVRON_R.is_empty());
        assert!(!NERD_CHEVRON_D.is_empty());
    }

    #[test]
    fn test_icon_status_defined() {
        assert!(!ICON_ALERT.is_empty());
        assert!(!ICON_CHECK.is_empty());
        assert!(!ICON_CLOSE.is_empty());
        assert!(!NERD_ALERT.is_empty());
        assert!(!NERD_CHECK.is_empty());
        assert!(!NERD_CLOSE.is_empty());
    }

    #[test]
    fn test_all_icons_have_nerd_font_variant() {
        // Verify that key icons have both Unicode and Nerd Font variants
        let icons = [
            (ICON_TERMINAL, NERD_TERMINAL),
            (ICON_SMARTPHONE, NERD_SMARTPHONE),
            (ICON_PLAY, NERD_PLAY),
            (ICON_STOP, NERD_STOP),
            (ICON_REFRESH, NERD_REFRESH),
        ];

        for (unicode, nerd) in &icons {
            assert!(!unicode.is_empty(), "Unicode icon should not be empty");
            assert!(!nerd.is_empty(), "Nerd Font icon should not be empty");
        }
    }

    #[test]
    fn test_terminal_and_command_are_distinct() {
        // ICON_TERMINAL and ICON_COMMAND should be visually distinct
        assert_ne!(
            ICON_TERMINAL, ICON_COMMAND,
            "ICON_TERMINAL and ICON_COMMAND must be different"
        );
    }
}
