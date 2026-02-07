//! Color palette for the Cyber-Glass theme.
//!
//! Phase 1: Maps to existing named colors for zero visual regression.
//! Phase 2+: Will transition to RGB design token values.

// Allow dead_code since these are infrastructure constants for Phase 2 migration
#![allow(dead_code)]

use ratatui::style::Color;

// --- Background layers ---
pub const DEEPEST_BG: Color = Color::Black; // Terminal background (Phase 2: Rgb(10,12,16))
pub const CARD_BG: Color = Color::Black; // Panel/card backgrounds (Phase 2: Rgb(18,21,28))
pub const POPUP_BG: Color = Color::DarkGray; // Modal/popup backgrounds (Phase 2: Rgb(28,33,43))
pub const SURFACE: Color = Color::Black; // Elevated surface (Phase 2: Rgb(22,27,34))

// --- Borders ---
pub const BORDER_DIM: Color = Color::DarkGray; // Inactive borders (Phase 2: Rgb(45,51,59))
pub const BORDER_ACTIVE: Color = Color::Cyan; // Focused borders (Phase 2: Rgb(88,166,255))

// --- Accent ---
pub const ACCENT: Color = Color::Cyan; // Primary accent (Phase 2: Rgb(88,166,255))
pub const ACCENT_DIM: Color = Color::DarkGray; // Dimmed accent (Phase 2: Rgb(56,107,163))

// --- Text ---
pub const TEXT_PRIMARY: Color = Color::White; // Primary text (Phase 2: Rgb(201,209,217))
pub const TEXT_SECONDARY: Color = Color::Gray; // Secondary text (Phase 2: Rgb(125,133,144))
pub const TEXT_MUTED: Color = Color::DarkGray; // Muted text (Phase 2: Rgb(72,79,88))
pub const TEXT_BRIGHT: Color = Color::White; // Bright/emphasis text (Phase 2: Rgb(240,246,252))

// --- Status ---
pub const STATUS_GREEN: Color = Color::Green; // Running/success (Phase 2: Rgb(16,185,129))
pub const STATUS_RED: Color = Color::Red; // Error/stopped (Phase 2: Rgb(244,63,94))
pub const STATUS_YELLOW: Color = Color::Yellow; // Warning/reloading (Phase 2: Rgb(234,179,8))
pub const STATUS_BLUE: Color = Color::Blue; // Info (Phase 2: Rgb(56,189,248))
pub const STATUS_INDIGO: Color = Color::Magenta; // Flutter messages (Phase 2: Rgb(129,140,248))

// --- Effects ---
pub const SHADOW: Color = Color::Black; // Shadow color (Phase 2: Rgb(5,6,8))

// --- Gradients (approximate) ---
pub const GRADIENT_BLUE: Color = Color::Blue; // Button gradient start (Phase 2: Rgb(37,99,235))
pub const GRADIENT_INDIGO: Color = Color::Magenta; // Button gradient end (Phase 2: Rgb(99,102,241))

// --- Log level colors ---
pub const LOG_ERROR: Color = Color::Red;
pub const LOG_ERROR_MSG: Color = Color::LightRed;
pub const LOG_WARNING: Color = Color::Yellow;
pub const LOG_WARNING_MSG: Color = Color::Yellow;
pub const LOG_INFO: Color = Color::Green;
pub const LOG_INFO_MSG: Color = Color::White;
pub const LOG_DEBUG: Color = Color::DarkGray;
pub const LOG_DEBUG_MSG: Color = Color::DarkGray;

// --- Log source colors ---
pub const SOURCE_APP: Color = Color::Magenta;
pub const SOURCE_DAEMON: Color = Color::Yellow;
pub const SOURCE_FLUTTER: Color = Color::Blue;
pub const SOURCE_FLUTTER_ERROR: Color = Color::Red;
pub const SOURCE_WATCHER: Color = Color::Cyan;

// --- Search highlight ---
pub const SEARCH_HIGHLIGHT_FG: Color = Color::Black;
pub const SEARCH_HIGHLIGHT_BG: Color = Color::Yellow;
pub const SEARCH_CURRENT_FG: Color = Color::Black;
pub const SEARCH_CURRENT_BG: Color = Color::LightYellow;

// --- Stack trace ---
pub const STACK_FRAME_NUMBER: Color = Color::DarkGray;
pub const STACK_FUNCTION_PROJECT: Color = Color::White;
pub const STACK_FUNCTION_PACKAGE: Color = Color::DarkGray;
pub const STACK_FILE_PROJECT: Color = Color::Blue;
pub const STACK_FILE_PACKAGE: Color = Color::DarkGray;
pub const STACK_LOCATION_PROJECT: Color = Color::Cyan;
pub const STACK_LOCATION_PACKAGE: Color = Color::DarkGray;
pub const STACK_ASYNC_GAP: Color = Color::DarkGray;
pub const STACK_PUNCTUATION: Color = Color::DarkGray;

// --- Modal backgrounds (existing Rgb values preserved) ---
pub const MODAL_FUZZY_BG: Color = Color::Rgb(40, 40, 50);
pub const MODAL_FUZZY_QUERY_BG: Color = Color::Rgb(60, 60, 70);
pub const MODAL_DART_DEFINES_BG: Color = Color::Rgb(30, 30, 40);
pub const MODAL_DART_DEFINES_INPUT_ACTIVE_BG: Color = Color::Rgb(60, 60, 80);
pub const MODAL_DART_DEFINES_INPUT_INACTIVE_BG: Color = Color::Rgb(40, 40, 50);
pub const MODAL_DART_DEFINES_BUTTON_INACTIVE_BG: Color = Color::Rgb(50, 50, 60);
pub const MODAL_DART_DEFINES_CLEAR_BG: Color = Color::Rgb(20, 20, 30);
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
    fn test_modal_backgrounds_are_rgb() {
        // Modal backgrounds should preserve existing RGB values
        match MODAL_FUZZY_BG {
            Color::Rgb(_, _, _) => {}
            _ => panic!("MODAL_FUZZY_BG should be RGB"),
        }
        match MODAL_FUZZY_QUERY_BG {
            Color::Rgb(_, _, _) => {}
            _ => panic!("MODAL_FUZZY_QUERY_BG should be RGB"),
        }
    }
}
