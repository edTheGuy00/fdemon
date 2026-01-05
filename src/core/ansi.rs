//! ANSI escape code handling utilities
//!
//! Provides functions to strip ANSI escape sequences from log messages.
//! The Flutter `logger` package and other logging libraries output ANSI codes
//! for terminal coloring that appear as garbage in the TUI.

use regex::Regex;
use std::sync::LazyLock;

/// Regex pattern for ANSI escape sequences.
///
/// Covers:
/// - CSI sequences: ESC [ ... letter (colors, cursor, etc.)
/// - OSC sequences: ESC ] ... BEL or ST (hyperlinks, titles)
/// - Simple escapes: ESC letter
static ANSI_ESCAPE_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    // Comprehensive pattern covering:
    // - CSI sequences: \x1b[ followed by params and command letter
    // - OSC sequences: \x1b] followed by content and terminator (BEL or ST)
    // - Simple escapes: \x1b followed by single letter
    Regex::new(r"\x1b\[[0-9;?]*[A-Za-z]|\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)|\x1b[A-Za-z]")
        .expect("ANSI regex pattern is valid")
});

/// Strip all ANSI escape sequences from a string.
///
/// Preserves:
/// - Unicode box-drawing characters: ┌ │ └ ├ ┄ ─
/// - Emoji characters: 🐛 💡 ⚠️ ⛔ 🔥
/// - All other visible text content
///
/// # Examples
///
/// ```
/// use flutter_demon::core::strip_ansi_codes;
///
/// let input = "\x1b[31mred text\x1b[0m";
/// assert_eq!(strip_ansi_codes(input), "red text");
///
/// // Box-drawing and emojis preserved
/// let input = "│ 🐛 Debug message";
/// assert_eq!(strip_ansi_codes(input), input);
/// ```
pub fn strip_ansi_codes(input: &str) -> String {
    ANSI_ESCAPE_PATTERN.replace_all(input, "").into_owned()
}

/// Check if a string contains ANSI escape sequences.
///
/// # Examples
///
/// ```
/// use flutter_demon::core::contains_ansi_codes;
///
/// assert!(contains_ansi_codes("\x1b[31mred\x1b[0m"));
/// assert!(!contains_ansi_codes("plain text"));
/// ```
pub fn contains_ansi_codes(input: &str) -> bool {
    ANSI_ESCAPE_PATTERN.is_match(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_simple_color_codes() {
        let input = "\x1b[31mred text\x1b[0m";
        assert_eq!(strip_ansi_codes(input), "red text");
    }

    #[test]
    fn test_strip_256_color_codes() {
        let input = "\x1b[38;5;244m│ Trace: message\x1b[0m";
        assert_eq!(strip_ansi_codes(input), "│ Trace: message");
    }

    #[test]
    fn test_strip_multiple_codes() {
        let input = "\x1b[1m\x1b[38;5;12mBold blue\x1b[0m";
        assert_eq!(strip_ansi_codes(input), "Bold blue");
    }

    #[test]
    fn test_strip_rgb_color_codes() {
        let input = "\x1b[38;2;255;100;50mRGB color\x1b[0m";
        assert_eq!(strip_ansi_codes(input), "RGB color");
    }

    #[test]
    fn test_strip_background_color() {
        let input = "\x1b[48;5;234mBackground\x1b[0m";
        assert_eq!(strip_ansi_codes(input), "Background");
    }

    #[test]
    fn test_preserve_box_drawing() {
        let input = "┌─────────┐\n│ Message │\n└─────────┘";
        assert_eq!(strip_ansi_codes(input), input);
    }

    #[test]
    fn test_preserve_emojis() {
        let input = "🐛 Debug: message";
        assert_eq!(strip_ansi_codes(input), input);
    }

    #[test]
    fn test_preserve_all_logger_emojis() {
        let inputs = ["🐛 Debug", "💡 Info", "⚠️ Warning", "⛔ Error", "🔥 Fatal"];
        for input in inputs {
            assert_eq!(strip_ansi_codes(input), input);
        }
    }

    #[test]
    fn test_mixed_content() {
        let input = "\x1b[38;5;244m┌───────────\x1b[0m\n\x1b[38;5;244m│ 🐛 Debug\x1b[0m";
        assert_eq!(strip_ansi_codes(input), "┌───────────\n│ 🐛 Debug");
    }

    #[test]
    fn test_no_codes() {
        let input = "Plain text with no codes";
        assert_eq!(strip_ansi_codes(input), input);
    }

    #[test]
    fn test_empty_string() {
        assert_eq!(strip_ansi_codes(""), "");
    }

    #[test]
    fn test_contains_ansi_codes_true() {
        assert!(contains_ansi_codes("\x1b[31mred\x1b[0m"));
        assert!(contains_ansi_codes("\x1b[38;5;244mtext\x1b[0m"));
        assert!(contains_ansi_codes("prefix\x1b[1mbold\x1b[0msuffix"));
    }

    #[test]
    fn test_contains_ansi_codes_false() {
        assert!(!contains_ansi_codes("plain text"));
        assert!(!contains_ansi_codes("│ box drawing"));
        assert!(!contains_ansi_codes("🐛 emoji"));
        assert!(!contains_ansi_codes(""));
    }

    #[test]
    fn test_logger_package_output() {
        // Real Logger package output sample
        let input = "\x1b[38;5;244m│  Trace: Very detailed debugging info\x1b[0m";
        let result = strip_ansi_codes(input);
        assert_eq!(result, "│  Trace: Very detailed debugging info");
        assert!(result.contains("Trace:"));
    }

    #[test]
    fn test_logger_package_multiline() {
        // Logger package typically outputs multi-line blocks
        let input = "\x1b[38;5;244m┌───────────────────────────────────────────────────────────────────────────────\x1b[0m\n\x1b[38;5;244m│ 🐛 Debug message from logger\x1b[0m\n\x1b[38;5;244m└───────────────────────────────────────────────────────────────────────────────\x1b[0m";
        let result = strip_ansi_codes(input);
        assert!(result.contains("┌───────────"));
        assert!(result.contains("🐛 Debug"));
        assert!(result.contains("└───────────"));
        assert!(!result.contains("\x1b"));
    }

    #[test]
    fn test_bold_and_color_combined() {
        let input = "\x1b[1;31mBold Red Error\x1b[0m";
        assert_eq!(strip_ansi_codes(input), "Bold Red Error");
    }

    #[test]
    fn test_cursor_movement_codes() {
        // Cursor movement codes should also be stripped
        let input = "\x1b[2Jclear\x1b[Hmove";
        assert_eq!(strip_ansi_codes(input), "clearmove");
    }

    #[test]
    fn test_simple_escape_sequences() {
        // Simple ESC + letter sequences
        let input = "\x1bcReset\x1bM";
        let result = strip_ansi_codes(input);
        assert_eq!(result, "Reset");
    }

    #[test]
    fn test_partial_escape_not_stripped() {
        // Incomplete escape sequences should remain (defensive)
        // Note: This tests that we don't over-match
        let input = "text with \\x1b in it";
        assert_eq!(strip_ansi_codes(input), input);
    }

    #[test]
    fn test_osc_hyperlink() {
        // OSC 8 hyperlink format
        let input = "\x1b]8;;https://example.com\x1b\\Link Text\x1b]8;;\x1b\\";
        let result = strip_ansi_codes(input);
        assert_eq!(result, "Link Text");
    }

    #[test]
    fn test_osc_with_bel_terminator() {
        // OSC with BEL (^G) terminator
        let input = "\x1b]0;Window Title\x07Normal text";
        let result = strip_ansi_codes(input);
        assert_eq!(result, "Normal text");
    }
}
