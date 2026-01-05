//! ANSI escape code handling utilities
//!
//! Provides functions to strip ANSI escape sequences from log messages.
//! The Flutter `logger` package and other logging libraries output ANSI codes
//! for terminal coloring that appear as garbage in the TUI.
//!
//! Flutter's `--machine` mode also escapes:
//! - Control characters using caret notation: ^[ for ESC
//! - Unicode box-drawing characters with backslashes: \┌ \│ \└ \├ \┄ \─

use regex::Regex;
use std::sync::LazyLock;

/// Regex pattern for ANSI escape sequences.
///
/// Covers:
/// - CSI sequences: ESC [ ... letter (colors, cursor, etc.)
/// - OSC sequences: ESC ] ... BEL or ST (hyperlinks, titles)
/// - Simple escapes: ESC letter
/// - Caret notation: ^[ ... (used by some terminals/tools instead of actual ESC byte)
static ANSI_ESCAPE_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    // Comprehensive pattern covering:
    // - CSI sequences: \x1b[ followed by params and command letter
    // - OSC sequences: \x1b] followed by content and terminator (BEL or ST)
    // - Simple escapes: \x1b followed by single letter
    // - Caret notation CSI: ^[[ followed by params and command letter (Flutter --machine mode)
    // - Caret notation simple: ^[ followed by single letter
    Regex::new(
        r"(?x)
        # Standard ANSI with ESC byte (0x1B)
        \x1b\[[0-9;?]*[A-Za-z]           # CSI sequences
        | \x1b\][^\x07\x1b]*(?:\x07|\x1b\\)  # OSC sequences
        | \x1b[A-Za-z]                   # Simple escapes

        # Caret notation (^[ = ESC) - Flutter --machine mode escapes control chars
        | \^[\[]\[[0-9;?]*[A-Za-z]       # ^[[ CSI sequences (note: ^[ then [ then params)
        | \^\[[0-9;?]*[A-Za-z]           # ^[ CSI sequences (^[ followed by params)
        ",
    )
    .expect("ANSI regex pattern is valid")
});

/// Regex pattern for backslash-escaped box-drawing characters.
///
/// Flutter's --machine mode escapes Unicode box-drawing characters with backslashes:
/// \┌ \│ \└ \├ \┄ \─
///
/// This pattern captures the backslash so we can remove it while keeping the character.
static BACKSLASH_BOX_DRAWING_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    // Match backslash before box-drawing characters
    // The box-drawing characters are: ┌ │ └ ├ ┄ ─
    Regex::new(r"\\([┌│└├┄─])").expect("Backslash box-drawing regex pattern is valid")
});

/// Regex pattern for trailing backslashes at end of content.
///
/// Flutter's --machine mode adds trailing backslashes that should be removed.
static TRAILING_BACKSLASH_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    // Match trailing backslash(es) optionally followed by whitespace at end of string
    Regex::new(r"\\+\s*$").expect("Trailing backslash regex pattern is valid")
});

/// Strip all ANSI escape sequences and Flutter escape patterns from a string.
///
/// Handles:
/// - Standard ANSI escape sequences (CSI, OSC, simple escapes)
/// - Caret notation (^[ = ESC) from Flutter --machine mode
/// - Backslash-escaped box-drawing characters (\┌ \│ \└ \├ \┄ \─)
/// - Trailing backslashes
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
///
/// // Backslash escapes removed
/// let input = r"\┌───────────\";
/// assert_eq!(strip_ansi_codes(input), "┌───────────");
/// ```
pub fn strip_ansi_codes(input: &str) -> String {
    // Step 1: Strip ANSI escape sequences (including caret notation)
    let without_ansi = ANSI_ESCAPE_PATTERN.replace_all(input, "");

    // Step 2: Remove backslashes before box-drawing characters (\┌ → ┌)
    let without_backslash_box = BACKSLASH_BOX_DRAWING_PATTERN.replace_all(&without_ansi, "$1");

    // Step 3: Remove trailing backslashes
    let result = TRAILING_BACKSLASH_PATTERN.replace_all(&without_backslash_box, "");

    result.into_owned()
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

    // Caret notation tests - Flutter --machine mode escapes ESC as ^[
    #[test]
    fn test_strip_caret_notation_color() {
        // Flutter --machine mode output with caret notation
        let input = "^[[38;5;196mError message^[[0m";
        let result = strip_ansi_codes(input);
        assert_eq!(result, "Error message");
    }

    #[test]
    fn test_strip_caret_notation_256_color() {
        // 256-color with caret notation (from user's log output)
        let input = "^[[38;5;12m│ Info^[[0m";
        let result = strip_ansi_codes(input);
        assert_eq!(result, "│ Info");
    }

    #[test]
    fn test_strip_caret_notation_mixed() {
        // Mixed caret notation and box drawing
        let input = "^[[38;5;244m┌───────────^[[0m";
        let result = strip_ansi_codes(input);
        assert_eq!(result, "┌───────────");
    }

    #[test]
    fn test_strip_caret_notation_multiline() {
        // Logger package output with caret notation
        let input = "^[[38;5;196m┌───────────^[[0m\n^[[38;5;196m│ ⛔ Error^[[0m\n^[[38;5;196m└───────────^[[0m";
        let result = strip_ansi_codes(input);
        assert_eq!(result, "┌───────────\n│ ⛔ Error\n└───────────");
    }

    #[test]
    fn test_contains_caret_notation() {
        assert!(contains_ansi_codes("^[[38;5;196mred^[[0m"));
        assert!(contains_ansi_codes("^[[31mtext^[[0m"));
    }

    #[test]
    fn test_caret_notation_simple_codes() {
        // Simple color codes with caret notation
        let input = "^[[31mRed^[[0m ^[[32mGreen^[[0m";
        let result = strip_ansi_codes(input);
        assert_eq!(result, "Red Green");
    }

    // Backslash escape tests - Flutter --machine mode escapes box-drawing chars
    #[test]
    fn test_strip_backslash_box_drawing_start() {
        // Flutter escapes ┌ as \┌
        let input = r"\┌───────────────────────────────────────";
        let result = strip_ansi_codes(input);
        assert_eq!(result, "┌───────────────────────────────────────");
    }

    #[test]
    fn test_strip_backslash_box_drawing_end() {
        // Flutter escapes └ as \└
        let input = r"\└───────────────────────────────────────";
        let result = strip_ansi_codes(input);
        assert_eq!(result, "└───────────────────────────────────────");
    }

    #[test]
    fn test_strip_backslash_box_drawing_pipe() {
        // Flutter escapes │ as \│
        let input = r"\│ Message content";
        let result = strip_ansi_codes(input);
        assert_eq!(result, "│ Message content");
    }

    #[test]
    fn test_strip_backslash_box_drawing_divider() {
        // Flutter escapes ├ as \├
        let input = r"\├┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄";
        let result = strip_ansi_codes(input);
        assert_eq!(result, "├┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄");
    }

    #[test]
    fn test_strip_trailing_backslash() {
        // Flutter adds trailing backslashes
        let input = r"┌───────────────────────────────────────\";
        let result = strip_ansi_codes(input);
        assert_eq!(result, "┌───────────────────────────────────────");
    }

    #[test]
    fn test_strip_trailing_backslash_with_spaces() {
        // Trailing backslash with spaces
        let input = "┌───────────────────────────────────────\\     ";
        let result = strip_ansi_codes(input);
        assert_eq!(result, "┌───────────────────────────────────────");
    }

    #[test]
    fn test_strip_both_backslash_escapes() {
        // Both leading \┌ and trailing \
        let input = r"\┌───────────────────────────────────────\";
        let result = strip_ansi_codes(input);
        assert_eq!(result, "┌───────────────────────────────────────");
    }

    #[test]
    fn test_strip_flutter_machine_mode_full_block() {
        // Full Logger block as Flutter --machine mode outputs it
        let lines = vec![
            r"\┌───────────────────────────────────────\",
            r"\│ Null check operator used on a null value\",
            r"\├┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄\",
            r"\│ #0   triggerNullError (package:...)\",
            r"\│ ⛔ Error triggered: Null Error\",
            r"\└───────────────────────────────────────\",
        ];

        let expected = vec![
            "┌───────────────────────────────────────",
            "│ Null check operator used on a null value",
            "├┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄",
            "│ #0   triggerNullError (package:...)",
            "│ ⛔ Error triggered: Null Error",
            "└───────────────────────────────────────",
        ];

        for (input, exp) in lines.iter().zip(expected.iter()) {
            assert_eq!(strip_ansi_codes(input), *exp);
        }
    }

    #[test]
    fn test_strip_combined_caret_and_backslash() {
        // Combined caret notation ANSI and backslash escapes
        let input = r"^[[38;5;196m\┌───────────^[[0m\";
        let result = strip_ansi_codes(input);
        assert_eq!(result, "┌───────────");
    }

    #[test]
    fn test_preserve_normal_backslashes() {
        // Regular backslashes in paths should be preserved
        let input = r"C:\Users\name\project";
        let result = strip_ansi_codes(input);
        assert_eq!(result, r"C:\Users\name\project");
    }

    #[test]
    fn test_block_detection_after_strip() {
        // Verify block detection works after stripping
        let input = r"\┌───────────────────────────────────────\";
        let result = strip_ansi_codes(input);
        assert!(result.trim_start().starts_with('┌'));
    }

    #[test]
    fn test_block_end_detection_after_strip() {
        // Verify block end detection works after stripping
        let input = r"\└───────────────────────────────────────\";
        let result = strip_ansi_codes(input);
        assert!(result.trim_start().starts_with('└'));
    }
}
