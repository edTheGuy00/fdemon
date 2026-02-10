//! Color palette for the Cyber-Glass theme.
//!
//! Cyber-Glass design tokens using true-color RGB values.
//!
//! Requires terminal with true-color support. On terminals without true-color,
//! ratatui/crossterm auto-fallback to nearest 256-color match.

use ratatui::style::Color;

// --- Background layers ---
pub const DEEPEST_BG: Color = Color::Rgb(10, 12, 16); // Terminal background
pub const CARD_BG: Color = Color::Rgb(18, 21, 28); // Panel/card backgrounds
pub const POPUP_BG: Color = Color::Rgb(28, 33, 43); // Modal/popup backgrounds
pub const SURFACE: Color = Color::Rgb(22, 27, 34); // Elevated surface

// --- Borders ---
pub const BORDER_DIM: Color = Color::Rgb(45, 51, 59); // Inactive borders
pub const BORDER_ACTIVE: Color = Color::Rgb(88, 166, 255); // Focused borders

// --- Accent ---
pub const ACCENT: Color = Color::Rgb(88, 166, 255); // Primary accent
pub const ACCENT_DIM: Color = Color::Rgb(56, 107, 163); // Dimmed accent

// --- Text ---
pub const TEXT_PRIMARY: Color = Color::Rgb(201, 209, 217); // Primary text
pub const TEXT_SECONDARY: Color = Color::Rgb(125, 133, 144); // Secondary text
pub const TEXT_MUTED: Color = Color::Rgb(72, 79, 88); // Muted text
pub const TEXT_BRIGHT: Color = Color::Rgb(240, 246, 252); // Bright/emphasis text

// --- Status ---
pub const STATUS_GREEN: Color = Color::Rgb(16, 185, 129); // Running/success
pub const STATUS_RED: Color = Color::Rgb(244, 63, 94); // Error/stopped
pub const STATUS_YELLOW: Color = Color::Rgb(234, 179, 8); // Warning/reloading
pub const STATUS_BLUE: Color = Color::Rgb(56, 189, 248); // Info
pub const STATUS_INDIGO: Color = Color::Rgb(129, 140, 248); // Flutter messages

// --- Effects ---
pub const SHADOW: Color = Color::Rgb(5, 6, 8); // Shadow color
pub const CONTRAST_FG: Color = Color::Rgb(0, 0, 0); // High contrast foreground on accent bg

// --- Selected Row ---
/// Subtle accent-tinted background for selected rows and info banners.
/// Approximates ACCENT at 10% opacity on CARD_BG.
pub const SELECTED_ROW_BG: Color = Color::Rgb(17, 25, 40); // #111928

// --- Gradients ---
pub const GRADIENT_BLUE: Color = Color::Rgb(37, 99, 235); // Button gradient start
#[allow(dead_code)]
pub const GRADIENT_INDIGO: Color = Color::Rgb(99, 102, 241); // Button gradient end

// --- Log level colors ---
pub const LOG_ERROR: Color = Color::Rgb(244, 63, 94);
pub const LOG_ERROR_MSG: Color = Color::Rgb(251, 113, 133);
pub const LOG_WARNING: Color = Color::Rgb(234, 179, 8);
pub const LOG_WARNING_MSG: Color = Color::Rgb(250, 204, 21);
pub const LOG_INFO: Color = Color::Rgb(16, 185, 129);
pub const LOG_INFO_MSG: Color = Color::Rgb(201, 209, 217);
pub const LOG_DEBUG: Color = Color::Rgb(72, 79, 88);
pub const LOG_DEBUG_MSG: Color = Color::Rgb(100, 116, 139);

// --- Log source colors ---
pub const SOURCE_APP: Color = STATUS_GREEN; // App logs use green
pub const SOURCE_DAEMON: Color = STATUS_YELLOW; // Daemon logs use yellow
pub const SOURCE_FLUTTER: Color = STATUS_INDIGO; // Flutter logs use indigo
pub const SOURCE_FLUTTER_ERROR: Color = STATUS_RED; // Flutter error logs use red
pub const SOURCE_WATCHER: Color = STATUS_BLUE; // Watcher logs use blue

// --- Search highlight ---
pub const SEARCH_HIGHLIGHT_FG: Color = Color::Rgb(0, 0, 0);
pub const SEARCH_HIGHLIGHT_BG: Color = Color::Rgb(234, 179, 8);
pub const SEARCH_CURRENT_FG: Color = Color::Rgb(0, 0, 0);
pub const SEARCH_CURRENT_BG: Color = Color::Rgb(250, 204, 21);

// --- Stack trace ---
pub const STACK_FRAME_NUMBER: Color = Color::Rgb(72, 79, 88);
pub const STACK_FUNCTION_PROJECT: Color = Color::Rgb(201, 209, 217);
pub const STACK_FUNCTION_PACKAGE: Color = Color::Rgb(72, 79, 88);
pub const STACK_FILE_PROJECT: Color = Color::Rgb(56, 189, 248);
pub const STACK_FILE_PACKAGE: Color = Color::Rgb(72, 79, 88);
pub const STACK_LOCATION_PROJECT: Color = Color::Rgb(88, 166, 255);
pub const STACK_LOCATION_PACKAGE: Color = Color::Rgb(72, 79, 88);
pub const STACK_ASYNC_GAP: Color = Color::Rgb(72, 79, 88);
pub const STACK_PUNCTUATION: Color = Color::Rgb(72, 79, 88);

// --- Legacy modal backgrounds (kept for backward compat) ---
pub const LINK_BAR_BG: Color = Color::Rgb(30, 30, 30);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_palette_constants_are_valid() {
        // Verify a few representative constants compile and are the expected type
        let _: Color = ACCENT;
        let _: Color = DEEPEST_BG;
        let _: Color = STATUS_GREEN;
    }

    #[test]
    fn test_background_layers_defined() {
        // All background layer constants should be defined
        let _: Color = DEEPEST_BG;
        let _: Color = CARD_BG;
        let _: Color = POPUP_BG;
        let _: Color = SURFACE;
    }

    #[test]
    fn test_log_level_colors_complete() {
        // All log levels should have colors
        let _: Color = LOG_ERROR;
        let _: Color = LOG_ERROR_MSG;
        let _: Color = LOG_WARNING;
        let _: Color = LOG_WARNING_MSG;
        let _: Color = LOG_INFO;
        let _: Color = LOG_INFO_MSG;
        let _: Color = LOG_DEBUG;
        let _: Color = LOG_DEBUG_MSG;
    }

    #[test]
    fn test_popup_bg_is_rgb() {
        // Popup background should be RGB
        match POPUP_BG {
            Color::Rgb(28, 33, 43) => {}
            _ => panic!("POPUP_BG should be Rgb(28, 33, 43)"),
        }
    }

    #[test]
    fn test_design_tokens_are_rgb() {
        // Verify representative constants use Color::Rgb variant
        match DEEPEST_BG {
            Color::Rgb(10, 12, 16) => {}
            _ => panic!("DEEPEST_BG should be Rgb(10, 12, 16)"),
        }
        match ACCENT {
            Color::Rgb(88, 166, 255) => {}
            _ => panic!("ACCENT should be Rgb(88, 166, 255)"),
        }
        match TEXT_PRIMARY {
            Color::Rgb(201, 209, 217) => {}
            _ => panic!("TEXT_PRIMARY should be Rgb(201, 209, 217)"),
        }
        match STATUS_GREEN {
            Color::Rgb(16, 185, 129) => {}
            _ => panic!("STATUS_GREEN should be Rgb(16, 185, 129)"),
        }
    }
}
