//! Helper utilities for the handler module

use fdemon_core::{contains_word, strip_ansi_codes, LogLevel};

// ─────────────────────────────────────────────────────────
// Logger Package Block Detection (Phase 2 Task 11)
// ─────────────────────────────────────────────────────────

/// Box-drawing characters used by the Logger package for structured output
/// Reference: https://github.com/simc/logger
///
/// | Character | Unicode | Name                                | Usage         |
/// |-----------|---------|-------------------------------------|---------------|
/// | `┌`       | U+250C  | Box Drawings Light Down and Right   | Block start   |
/// | `└`       | U+2514  | Box Drawings Light Up and Right     | Block end     |
/// | `│`       | U+2502  | Box Drawings Light Vertical         | Block content |
/// | `├`       | U+251C  | Box Drawings Light Vertical + Right | Section divider |
/// | `┄`       | U+2504  | Box Drawings Light Triple Dash Horiz| Dashed divider |
/// | `─`       | U+2500  | Box Drawings Light Horizontal       | Horizontal line |
///
/// Check if a line is part of a Logger package structured block
///
/// Strips ANSI codes before checking for box-drawing characters to ensure
/// reliable detection even if ANSI codes weren't stripped earlier in the pipeline.
pub fn is_logger_block_line(message: &str) -> bool {
    // Strip ANSI codes first for reliable detection
    let cleaned = strip_ansi_codes(message);
    let trimmed = cleaned.trim_start();
    trimmed.starts_with('┌')
        || trimmed.starts_with('│')
        || trimmed.starts_with('├')
        || trimmed.starts_with('└')
        || trimmed.starts_with('┄')
        || trimmed.starts_with('─')
}

/// Check if a line is the start of a Logger block (┌)
///
/// Strips ANSI codes before checking for box-drawing characters.
pub fn is_block_start(message: &str) -> bool {
    let cleaned = strip_ansi_codes(message);
    cleaned.trim_start().starts_with('┌')
}

/// Check if a line is the end of a Logger block (└)
///
/// Strips ANSI codes before checking for box-drawing characters.
pub fn is_block_end(message: &str) -> bool {
    let cleaned = strip_ansi_codes(message);
    cleaned.trim_start().starts_with('└')
}

// ─────────────────────────────────────────────────────────
// Log Level Detection
// ─────────────────────────────────────────────────────────

/// Detect log level from raw (non-JSON) output line
///
/// Handles Android logcat format and content-based detection.
/// ANSI codes are automatically stripped before detection.
/// The "flutter: " prefix is stripped to avoid duplicate source indicators
/// (e.g., `[flutter] flutter: message` becomes `[flutter] message`).
pub fn detect_raw_line_level(line: &str) -> (LogLevel, String) {
    // Strip ANSI escape codes first (from Logger package, etc.)
    let cleaned = strip_ansi_codes(line);
    let trimmed = cleaned.trim();

    // Strip "flutter: " prefix if present (matches parse_flutter_log behavior in protocol.rs)
    // This prevents duplicate source indicators like "[flutter] flutter: message"
    let message = trimmed.strip_prefix("flutter: ").unwrap_or(trimmed);

    // Android logcat format: E/, W/, I/, D/, V/
    if message.starts_with("E/") {
        return (LogLevel::Error, message.to_string());
    }
    if message.starts_with("W/") {
        return (LogLevel::Warning, message.to_string());
    }
    if message.starts_with("I/") {
        return (LogLevel::Info, message.to_string());
    }
    if message.starts_with("D/") || message.starts_with("V/") {
        return (LogLevel::Debug, message.to_string());
    }

    // Use content-based detection for everything else
    let level = detect_log_level_from_content(message);
    (level, message.to_string())
}

/// Check if a line is a stack trace frame
///
/// Stack trace lines have patterns like:
/// - `#0   ClassName.methodName (file:line)`
/// - `│ #0   ClassName.methodName (file:line)`
/// - `#1   _functionName (file:line)`
///
/// These should not trigger keyword-based error detection because they
/// often contain class names like "ErrorTestingPage" or "ExceptionHandler".
fn is_stack_trace_line(message: &str) -> bool {
    // Strip leading box-drawing characters and whitespace
    let trimmed = message.trim_start_matches(|c: char| c.is_whitespace() || "│├└┌─┄".contains(c));

    // Check for stack frame pattern: #<digit>
    if let Some(rest) = trimmed.strip_prefix('#') {
        // Must be followed by a digit
        rest.starts_with(|c: char| c.is_ascii_digit())
    } else {
        false
    }
}

/// Content-based log level detection
///
/// Supports:
/// - Logger package: emoji indicators (🔥⛔⚠️💡🐛) and prefixes (Trace:, Debug:, etc.)
/// - Talker package: bracketed prefixes ([verbose], [debug], [info], etc.)
/// - Gradle/Xcode build errors
/// - General keywords (but NOT for stack trace lines to avoid false positives)
fn detect_log_level_from_content(message: &str) -> LogLevel {
    // ─────────────────────────────────────────────────────────
    // Emoji-based detection (Logger package uses these)
    // Check emojis first - they're unambiguous indicators
    // ─────────────────────────────────────────────────────────

    // Fatal/Critical indicators (check first - highest priority)
    if message.contains('🔥') || message.contains('💀') {
        return LogLevel::Error;
    }

    // Error indicators
    if message.contains('⛔') || message.contains('❌') || message.contains('🚫') {
        return LogLevel::Error;
    }

    // Warning indicators
    if message.contains('⚠') || message.contains('⚡') {
        return LogLevel::Warning;
    }

    // Info indicators
    if message.contains('💡') || message.contains('ℹ') {
        return LogLevel::Info;
    }

    // Debug indicators
    if message.contains('🐛') || message.contains('🔍') {
        return LogLevel::Debug;
    }

    // ─────────────────────────────────────────────────────────
    // Stack trace lines - default to Info, skip keyword detection
    // This prevents false positives from class names like "ErrorHandler"
    // ─────────────────────────────────────────────────────────

    if is_stack_trace_line(message) {
        return LogLevel::Info;
    }

    let lower = message.to_lowercase();

    // ─────────────────────────────────────────────────────────
    // Build system errors (Gradle/Xcode)
    // ─────────────────────────────────────────────────────────

    if lower.contains("failure:") || lower.contains("build failed") {
        return LogLevel::Error;
    }

    // ─────────────────────────────────────────────────────────
    // Prefix-based detection (Logger/Talker package formats)
    // ─────────────────────────────────────────────────────────

    // Logger package prefixes (with colon)
    if lower.contains("fatal:") || lower.contains("critical:") {
        return LogLevel::Error;
    }
    if lower.contains("error:") || lower.contains("exception:") {
        return LogLevel::Error;
    }
    if lower.contains("warning:") || lower.contains("warn:") {
        return LogLevel::Warning;
    }
    if lower.contains("info:") {
        return LogLevel::Info;
    }
    if lower.contains("debug:") || lower.contains("trace:") {
        return LogLevel::Debug;
    }

    // Talker package format (bracketed)
    if lower.contains("[critical]") || lower.contains("[fatal]") {
        return LogLevel::Error;
    }
    if lower.contains("[error]") || lower.contains("[exception]") {
        return LogLevel::Error;
    }
    if lower.contains("[warning]") || lower.contains("[warn]") {
        return LogLevel::Warning;
    }
    if lower.contains("[info]") {
        return LogLevel::Info;
    }
    if lower.contains("[debug]") || lower.contains("[verbose]") || lower.contains("[trace]") {
        return LogLevel::Debug;
    }

    // ─────────────────────────────────────────────────────────
    // Build progress (often noise, show as debug)
    // ─────────────────────────────────────────────────────────

    if message.starts_with("Running ")
        || message.starts_with("Building ")
        || message.starts_with("Compiling ")
        || message.contains("...")
    {
        return LogLevel::Debug;
    }

    // ─────────────────────────────────────────────────────────
    // Dart exception type detection
    // Handles CamelCase exception types like RangeError, TypeError, FormatException
    // Pattern: "SomethingError (params):" or "SomethingError: message"
    // ─────────────────────────────────────────────────────────

    // Check for Dart exception patterns (TypeNameError or TypeNameException)
    // These are CamelCase but indicate real errors
    if lower.contains("error (") || lower.contains("exception (") {
        return LogLevel::Error;
    }

    // ─────────────────────────────────────────────────────────
    // Word boundary detection (prevents false positives)
    // Uses word boundaries to avoid matching identifiers like
    // "ErrorTestingPage", "handleError", "errorCount"
    // ─────────────────────────────────────────────────────────

    // Error keywords - must be at word boundaries
    // Include common word variations (crashed, crashing, etc.)
    if contains_word(message, "error")
        || contains_word(message, "exception")
        || contains_word(message, "failed")
        || contains_word(message, "failure")
        || contains_word(message, "fatal")
        || contains_word(message, "crash")
        || contains_word(message, "crashed")
        || contains_word(message, "crashing")
    {
        return LogLevel::Error;
    }

    // Warning keywords - must be at word boundaries
    if contains_word(message, "warning")
        || contains_word(message, "deprecated")
        || contains_word(message, "caution")
    {
        return LogLevel::Warning;
    }

    // Debug keywords
    if lower.starts_with("debug") || contains_word(message, "verbose") {
        return LogLevel::Debug;
    }

    LogLevel::Info
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─────────────────────────────────────────────────────────
    // Android Logcat Format Tests
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_detect_raw_line_level_android() {
        let (level, _) = detect_raw_line_level("E/flutter: error message");
        assert_eq!(level, LogLevel::Error);

        let (level, _) = detect_raw_line_level("W/flutter: warning");
        assert_eq!(level, LogLevel::Warning);

        let (level, _) = detect_raw_line_level("I/flutter: info");
        assert_eq!(level, LogLevel::Info);

        let (level, _) = detect_raw_line_level("D/flutter: debug");
        assert_eq!(level, LogLevel::Debug);

        let (level, _) = detect_raw_line_level("V/flutter: verbose");
        assert_eq!(level, LogLevel::Debug);
    }

    #[test]
    fn test_detect_raw_line_level_gradle() {
        let (level, _) = detect_raw_line_level("FAILURE: Build failed");
        assert_eq!(level, LogLevel::Error);

        let (level, _) = detect_raw_line_level("BUILD FAILED in 10s");
        assert_eq!(level, LogLevel::Error);
    }

    #[test]
    fn test_detect_raw_line_level_xcode() {
        let (level, _) = detect_raw_line_level("❌ Build failed");
        assert_eq!(level, LogLevel::Error);
    }

    #[test]
    fn test_detect_raw_line_level_default() {
        let (level, _) = detect_raw_line_level("Some random output");
        assert_eq!(level, LogLevel::Info);
    }

    #[test]
    fn test_detect_raw_line_level_build_progress() {
        let (level, _) = detect_raw_line_level("Running pod install...");
        assert_eq!(level, LogLevel::Debug);

        let (level, _) = detect_raw_line_level("Building iOS app...");
        assert_eq!(level, LogLevel::Debug);

        let (level, _) = detect_raw_line_level("Compiling sources...");
        assert_eq!(level, LogLevel::Debug);
    }

    #[test]
    fn test_detect_raw_line_level_trims_whitespace() {
        let (level, msg) = detect_raw_line_level("   E/flutter: error   ");
        assert_eq!(level, LogLevel::Error);
        assert_eq!(msg, "E/flutter: error");
    }

    // ─────────────────────────────────────────────────────────
    // Logger Package Tests (via detect_log_level_from_content)
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_logger_trace_prefix() {
        assert_eq!(
            detect_log_level_from_content("Trace: Very detailed info"),
            LogLevel::Debug
        );
        assert_eq!(
            detect_log_level_from_content("│  Trace: message"),
            LogLevel::Debug
        );
    }

    #[test]
    fn test_logger_debug_emoji() {
        assert_eq!(
            detect_log_level_from_content("🐛 Debug: Debugging info"),
            LogLevel::Debug
        );
        assert_eq!(
            detect_log_level_from_content("│ 🐛  Debug: message"),
            LogLevel::Debug
        );
    }

    #[test]
    fn test_logger_info_emoji() {
        assert_eq!(
            detect_log_level_from_content("💡 Info: General info"),
            LogLevel::Info
        );
        assert_eq!(
            detect_log_level_from_content("│ 💡  Info: message"),
            LogLevel::Info
        );
    }

    #[test]
    fn test_logger_warning_emoji() {
        assert_eq!(
            detect_log_level_from_content("⚠️ Warning: Something wrong"),
            LogLevel::Warning
        );
        assert_eq!(
            detect_log_level_from_content("│ ⚠  Warning: message"),
            LogLevel::Warning
        );
    }

    #[test]
    fn test_logger_error_emoji() {
        assert_eq!(
            detect_log_level_from_content("⛔ Error: Something failed"),
            LogLevel::Error
        );
        assert_eq!(
            detect_log_level_from_content("│ ⛔  Error: message"),
            LogLevel::Error
        );
        assert_eq!(
            detect_log_level_from_content("❌ Error: failure"),
            LogLevel::Error
        );
    }

    #[test]
    fn test_logger_fatal_emoji() {
        assert_eq!(
            detect_log_level_from_content("🔥 Fatal: Critical failure"),
            LogLevel::Error
        );
        assert_eq!(
            detect_log_level_from_content("│ 🔥  Fatal: message"),
            LogLevel::Error
        );
    }

    // ─────────────────────────────────────────────────────────
    // Talker Package Tests
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_talker_verbose() {
        assert_eq!(
            detect_log_level_from_content("[verbose] Detailed message"),
            LogLevel::Debug
        );
    }

    #[test]
    fn test_talker_debug() {
        assert_eq!(
            detect_log_level_from_content("[debug] Debug message"),
            LogLevel::Debug
        );
    }

    #[test]
    fn test_talker_info() {
        assert_eq!(
            detect_log_level_from_content("[info] Info message"),
            LogLevel::Info
        );
    }

    #[test]
    fn test_talker_warning() {
        assert_eq!(
            detect_log_level_from_content("[warning] Warning message"),
            LogLevel::Warning
        );
        assert_eq!(
            detect_log_level_from_content("[warn] Warning message"),
            LogLevel::Warning
        );
    }

    #[test]
    fn test_talker_error() {
        assert_eq!(
            detect_log_level_from_content("[error] Error message"),
            LogLevel::Error
        );
        assert_eq!(
            detect_log_level_from_content("[exception] Exception occurred"),
            LogLevel::Error
        );
    }

    #[test]
    fn test_talker_critical() {
        assert_eq!(
            detect_log_level_from_content("[critical] Critical failure"),
            LogLevel::Error
        );
        assert_eq!(
            detect_log_level_from_content("[fatal] Fatal error"),
            LogLevel::Error
        );
    }

    // ─────────────────────────────────────────────────────────
    // Edge Cases
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_plain_message_is_info() {
        assert_eq!(
            detect_log_level_from_content("Just a regular message"),
            LogLevel::Info
        );
    }

    #[test]
    fn test_box_drawing_with_level() {
        // Logger package wraps messages in boxes
        assert_eq!(
            detect_log_level_from_content("│ 💡  Info: Login successful"),
            LogLevel::Info
        );
        assert_eq!(
            detect_log_level_from_content("│ 🐛  Debug: User data loaded"),
            LogLevel::Debug
        );
    }

    #[test]
    fn test_case_insensitive_prefixes() {
        assert_eq!(
            detect_log_level_from_content("ERROR: something failed"),
            LogLevel::Error
        );
        assert_eq!(
            detect_log_level_from_content("Warning: be careful"),
            LogLevel::Warning
        );
        assert_eq!(
            detect_log_level_from_content("DEBUG: verbose output"),
            LogLevel::Debug
        );
    }

    #[test]
    fn test_info_colon_prefix() {
        assert_eq!(
            detect_log_level_from_content("Info: Application started"),
            LogLevel::Info
        );
    }

    // ─────────────────────────────────────────────────────────
    // Flutter Prefix Stripping Tests
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_strip_flutter_prefix() {
        let (level, msg) = detect_raw_line_level("flutter: Hello World");
        assert_eq!(level, LogLevel::Info);
        assert_eq!(msg, "Hello World");
    }

    #[test]
    fn test_strip_flutter_prefix_with_box_drawing() {
        let (_, msg) = detect_raw_line_level("flutter: │ Stack trace info");
        assert_eq!(msg, "│ Stack trace info");
    }

    #[test]
    fn test_strip_flutter_prefix_with_emoji() {
        let (level, msg) = detect_raw_line_level("flutter: 💡 Info: message");
        assert_eq!(level, LogLevel::Info);
        assert_eq!(msg, "💡 Info: message");
    }

    #[test]
    fn test_strip_flutter_prefix_error() {
        let (level, msg) = detect_raw_line_level("flutter: ⛔ Error: failed");
        assert_eq!(level, LogLevel::Error);
        assert_eq!(msg, "⛔ Error: failed");
    }

    #[test]
    fn test_no_flutter_prefix() {
        let (_, msg) = detect_raw_line_level("Plain message without prefix");
        assert_eq!(msg, "Plain message without prefix");
    }

    #[test]
    fn test_android_logcat_after_strip() {
        // flutter: prefix should be stripped first, then logcat detection should work
        let (level, msg) = detect_raw_line_level("flutter: E/flutter: error message");
        assert_eq!(level, LogLevel::Error);
        assert_eq!(msg, "E/flutter: error message");
    }

    #[test]
    fn test_android_logcat_without_flutter_prefix() {
        let (level, msg) = detect_raw_line_level("E/flutter: error message");
        assert_eq!(level, LogLevel::Error);
        assert_eq!(msg, "E/flutter: error message");
    }

    #[test]
    fn test_strip_flutter_prefix_with_ansi() {
        // ANSI codes stripped first, then flutter: prefix
        let (_, msg) = detect_raw_line_level("\x1b[38;5;244mflutter: │ message\x1b[0m");
        assert_eq!(msg, "│ message");
    }

    #[test]
    fn test_strip_flutter_prefix_warning() {
        let (level, msg) = detect_raw_line_level("flutter: ⚠ Warning: deprecated");
        assert_eq!(level, LogLevel::Warning);
        assert_eq!(msg, "⚠ Warning: deprecated");
    }

    #[test]
    fn test_strip_flutter_prefix_debug() {
        let (level, msg) = detect_raw_line_level("flutter: 🐛 Debug: verbose info");
        assert_eq!(level, LogLevel::Debug);
        assert_eq!(msg, "🐛 Debug: verbose info");
    }

    #[test]
    fn test_flutter_prefix_case_sensitive() {
        // Only lowercase "flutter: " should be stripped
        let (_, msg) = detect_raw_line_level("Flutter: Message");
        assert_eq!(msg, "Flutter: Message"); // Not stripped

        let (_, msg) = detect_raw_line_level("FLUTTER: Message");
        assert_eq!(msg, "FLUTTER: Message"); // Not stripped
    }

    #[test]
    fn test_flutter_prefix_needs_space() {
        // Must have space after colon to strip
        let (_, msg) = detect_raw_line_level("flutter:NoSpace");
        assert_eq!(msg, "flutter:NoSpace"); // Not stripped
    }

    #[test]
    fn test_flutter_prefix_empty_after_strip() {
        // Edge case: "flutter: message" with actual content after
        let (level, msg) = detect_raw_line_level("flutter: message");
        assert_eq!(level, LogLevel::Info);
        assert_eq!(msg, "message");

        // Edge case: "flutter:  " with just spaces after - trailing spaces get trimmed
        // so "flutter:  " becomes "flutter:" after trim, and prefix doesn't match
        let (level, msg) = detect_raw_line_level("flutter:  ");
        assert_eq!(level, LogLevel::Info);
        assert_eq!(msg, "flutter:"); // trailing spaces trimmed, no match for "flutter: "
    }

    #[test]
    fn test_double_flutter_prefix() {
        // Only first occurrence stripped
        let (_, msg) = detect_raw_line_level("flutter: flutter: message");
        assert_eq!(msg, "flutter: message");
    }

    // ─────────────────────────────────────────────────────────
    // Logger Block Detection Tests (Phase 2 Task 11)
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_is_logger_block_line() {
        // Block start
        assert!(is_logger_block_line(
            "┌───────────────────────────────────────"
        ));
        // Block end
        assert!(is_logger_block_line(
            "└───────────────────────────────────────"
        ));
        // Block content
        assert!(is_logger_block_line("│ Message content"));
        // Section divider
        assert!(is_logger_block_line(
            "├┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄"
        ));
        // Dashed line
        assert!(is_logger_block_line(
            "┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄"
        ));
        // Horizontal line
        assert!(is_logger_block_line(
            "─────────────────────────────────────────"
        ));

        // With leading whitespace
        assert!(is_logger_block_line("   ┌───────────"));
        assert!(is_logger_block_line("\t│ Message"));

        // Regular messages
        assert!(!is_logger_block_line("Regular message"));
        assert!(!is_logger_block_line("Error: something failed"));
        assert!(!is_logger_block_line(""));
        assert!(!is_logger_block_line("   "));
    }

    #[test]
    fn test_is_block_start() {
        assert!(is_block_start("┌───────────────────────────────────────"));
        assert!(is_block_start("  ┌─────────────")); // with whitespace

        assert!(!is_block_start("│ Message"));
        assert!(!is_block_start("└───────────────────────────────────────"));
        assert!(!is_block_start("├┄┄┄┄┄┄┄┄"));
        assert!(!is_block_start("Regular message"));
    }

    #[test]
    fn test_is_block_end() {
        assert!(is_block_end("└───────────────────────────────────────"));
        assert!(is_block_end("  └─────────────")); // with whitespace

        assert!(!is_block_end("│ Message"));
        assert!(!is_block_end("┌───────────────────────────────────────"));
        assert!(!is_block_end("├┄┄┄┄┄┄┄┄"));
        assert!(!is_block_end("Regular message"));
    }

    #[test]
    fn test_block_detection_with_logger_output() {
        // Simulate actual Logger package output
        let lines = vec![
            "┌───────────────────────────────────────────────────────────────",
            "│ RangeError (length): Invalid value: Not in inclusive range 0..2: 10",
            "├┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄",
            "│ #0   List.[] (dart:core-patch/growable_array.dart)",
            "│ #1   triggerRangeError (package:flutter_deamon/errors/...)",
            "├┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄",
            "│ 11:57:11.960 (+0:05:46.971300)",
            "├┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄",
            "│ ⛔┄ Error triggered: Range Error",
            "└───────────────────────────────────────────────────────────────",
        ];

        assert!(is_block_start(lines[0]));
        for line in &lines[1..lines.len() - 1] {
            assert!(is_logger_block_line(line));
            assert!(!is_block_start(line));
            assert!(!is_block_end(line));
        }
        assert!(is_block_end(lines[lines.len() - 1]));
    }

    // ─────────────────────────────────────────────────────────
    // Block Detection with ANSI Codes (Regression test for Phase 2 fix)
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_block_start_with_ansi_codes() {
        // Logger package output with 256-color ANSI codes
        assert!(is_block_start(
            "\x1b[38;5;12m┌───────────────────────────────────────\x1b[0m"
        ));
        assert!(is_block_start(
            "\x1b[38;5;196m┌───────────────────────────────────────\x1b[0m"
        ));
        assert!(is_block_start(
            "\x1b[38;5;208m┌───────────────────────────────────────\x1b[0m"
        ));
        // With RGB colors
        assert!(is_block_start(
            "\x1b[38;2;255;100;50m┌───────────────────────────────────────\x1b[0m"
        ));
    }

    #[test]
    fn test_block_end_with_ansi_codes() {
        // Logger package output with 256-color ANSI codes
        assert!(is_block_end(
            "\x1b[38;5;12m└───────────────────────────────────────\x1b[0m"
        ));
        assert!(is_block_end(
            "\x1b[38;5;196m└───────────────────────────────────────\x1b[0m"
        ));
        assert!(is_block_end(
            "\x1b[38;5;208m└───────────────────────────────────────\x1b[0m"
        ));
    }

    #[test]
    fn test_is_logger_block_line_with_ansi_codes() {
        // All box-drawing characters with ANSI codes
        assert!(is_logger_block_line(
            "\x1b[38;5;12m┌───────────────────────────────────────\x1b[0m"
        ));
        assert!(is_logger_block_line(
            "\x1b[38;5;12m│ Message content\x1b[0m"
        ));
        assert!(is_logger_block_line(
            "\x1b[38;5;12m├┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄\x1b[0m"
        ));
        assert!(is_logger_block_line(
            "\x1b[38;5;12m└───────────────────────────────────────\x1b[0m"
        ));
        assert!(is_logger_block_line("\x1b[38;5;12m┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄\x1b[0m"));
        assert!(is_logger_block_line(
            "\x1b[38;5;12m───────────────────\x1b[0m"
        ));
    }

    #[test]
    fn test_block_detection_with_ansi_prefixed_logger_output() {
        // Simulate actual Logger package output with ANSI codes (as Flutter daemon sends it)
        let lines = vec![
            "\x1b[38;5;196m┌───────────────────────────────────────────────────────────────\x1b[0m",
            "\x1b[38;5;196m│ RangeError (length): Invalid value: Not in inclusive range 0..2: 10\x1b[0m",
            "\x1b[38;5;196m├┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄\x1b[0m",
            "\x1b[38;5;196m│ #0   List.[] (dart:core-patch/growable_array.dart)\x1b[0m",
            "\x1b[38;5;196m│ #1   triggerRangeError (package:flutter_deamon/errors/...)\x1b[0m",
            "\x1b[38;5;196m├┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄\x1b[0m",
            "\x1b[38;5;196m│ ⛔┄ Error triggered: Range Error\x1b[0m",
            "\x1b[38;5;196m└───────────────────────────────────────────────────────────────\x1b[0m",
        ];

        assert!(is_block_start(lines[0]));
        for line in &lines[1..lines.len() - 1] {
            assert!(is_logger_block_line(line));
            assert!(!is_block_start(line));
            assert!(!is_block_end(line));
        }
        assert!(is_block_end(lines[lines.len() - 1]));
    }

    // ─────────────────────────────────────────────────────────
    // Block Detection with Backslash Escapes (Flutter --machine mode)
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_block_start_with_backslash_escape() {
        // Flutter --machine mode escapes box-drawing with backslashes
        assert!(is_block_start(
            r"\┌───────────────────────────────────────\"
        ));
        assert!(is_block_start(r"\┌───────────────────────────────────────"));
    }

    #[test]
    fn test_block_end_with_backslash_escape() {
        assert!(is_block_end(r"\└───────────────────────────────────────\"));
        assert!(is_block_end(r"\└───────────────────────────────────────"));
    }

    #[test]
    fn test_is_logger_block_line_with_backslash_escape() {
        assert!(is_logger_block_line(
            r"\┌───────────────────────────────────────\"
        ));
        assert!(is_logger_block_line(r"\│ Message content\"));
        assert!(is_logger_block_line(r"\├┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄\"));
        assert!(is_logger_block_line(
            r"\└───────────────────────────────────────\"
        ));
    }

    #[test]
    fn test_block_detection_with_flutter_machine_mode_output() {
        // Full Logger block as Flutter --machine mode outputs it (with backslash escapes)
        let lines = vec![
            r"\┌───────────────────────────────────────\",
            r"\│ Null check operator used on a null value\",
            r"\├┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄\",
            r"\│ #0   triggerNullError (package:...)\",
            r"\│ ⛔ Error triggered: Null Error\",
            r"\└───────────────────────────────────────\",
        ];

        assert!(is_block_start(lines[0]));
        for line in &lines[1..lines.len() - 1] {
            assert!(
                is_logger_block_line(line),
                "Line should be block line: {}",
                line
            );
            assert!(!is_block_start(line));
            assert!(!is_block_end(line));
        }
        assert!(is_block_end(lines[lines.len() - 1]));
    }

    #[test]
    fn test_block_detection_with_combined_escapes() {
        // Combined caret notation ANSI and backslash escapes
        assert!(is_block_start(r"^[[38;5;196m\┌───────────^[[0m\"));
        assert!(is_block_end(r"^[[38;5;196m\└───────────^[[0m\"));
        assert!(is_logger_block_line(r"^[[38;5;196m\│ Message^[[0m\"));
    }

    // ─────────────────────────────────────────────────────────
    // Stack Trace Detection Tests
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_is_stack_trace_line() {
        // Standard Dart stack trace formats
        assert!(is_stack_trace_line(
            "#0   main (file:///path/main.dart:10:5)"
        ));
        assert!(is_stack_trace_line(
            "#1   _runMain (dart:ui/hooks.dart:159:15)"
        ));
        assert!(is_stack_trace_line(
            "#10  SomeClass.method (package:app/file.dart:42:10)"
        ));

        // With box-drawing prefix (Logger package output)
        assert!(is_stack_trace_line(
            "│ #0   ErrorTestingPage._spamLoggerLogs (package:flutter_deamon/main.dart:208:18)"
        ));
        assert!(is_stack_trace_line("│ #1   ErrorTestingPage.build.<anonymous closure> (package:flutter_deamon/main.dart:113:45)"));
        assert!(is_stack_trace_line(
            "├ #2   SomeWidget.build (file.dart:10:5)"
        ));

        // With whitespace
        assert!(is_stack_trace_line("  #0   main (file.dart:1:1)"));
        assert!(is_stack_trace_line(
            "    │ #5   ClassName.methodName (file.dart:1:1)"
        ));

        // Not stack traces
        assert!(!is_stack_trace_line("Regular message"));
        assert!(!is_stack_trace_line("Error: something failed"));
        assert!(!is_stack_trace_line("│ Error message"));
        assert!(!is_stack_trace_line("│ 🐛 Debug message"));
        assert!(!is_stack_trace_line("# Not a stack trace"));
        assert!(!is_stack_trace_line("#NotAStackTrace"));
    }

    #[test]
    fn test_stack_trace_does_not_trigger_error_detection() {
        // Stack traces with "Error" in class names should NOT be detected as errors
        let (level, _) = detect_raw_line_level(
            "│ #0   ErrorTestingPage._spamLoggerLogs (package:flutter_deamon/main.dart:208:18)",
        );
        assert_eq!(level, LogLevel::Info);

        let (level, _) = detect_raw_line_level("#0   ExceptionHandler.handle (file.dart:10:5)");
        assert_eq!(level, LogLevel::Info);

        let (level, _) = detect_raw_line_level(
            "│ #1   triggerNullError (package:flutter_deamon/errors/sync_errors.dart:14:23)",
        );
        assert_eq!(level, LogLevel::Info);
    }

    #[test]
    fn test_real_error_messages_still_detected() {
        // Real error messages (not stack traces) should still be detected
        let (level, _) = detect_raw_line_level("Error: Null check operator used on a null value");
        assert_eq!(level, LogLevel::Error);

        let (level, _) = detect_raw_line_level("│ ⛔ Error triggered: Null Error");
        assert_eq!(level, LogLevel::Error);

        let (level, _) = detect_raw_line_level("RangeError (length): Invalid value");
        assert_eq!(level, LogLevel::Error);
    }

    // ─────────────────────────────────────────────────────────
    // False Positive Prevention Tests (Bug Fix: Task 02)
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_class_name_not_error() {
        // Class names should NOT trigger Error level
        let (level, _) = detect_raw_line_level("ErrorTestingPage");
        assert_eq!(
            level,
            LogLevel::Info,
            "ErrorTestingPage should be Info, not Error"
        );

        let (level, _) = detect_raw_line_level("MyErrorHandler");
        assert_eq!(level, LogLevel::Info, "MyErrorHandler should be Info");

        let (level, _) = detect_raw_line_level("ErrorBoundary widget loaded");
        assert_eq!(level, LogLevel::Info, "ErrorBoundary should be Info");
    }

    #[test]
    fn test_method_name_not_error() {
        // Method names should NOT trigger Error level
        let (level, _) = detect_raw_line_level("handleError called");
        assert_eq!(level, LogLevel::Info, "handleError should be Info");

        let (level, _) = detect_raw_line_level("onErrorCallback triggered");
        assert_eq!(level, LogLevel::Info, "onErrorCallback should be Info");

        let (level, _) = detect_raw_line_level("throwError()");
        assert_eq!(level, LogLevel::Info, "throwError() should be Info");
    }

    #[test]
    fn test_variable_name_not_error() {
        // Variable names should NOT trigger Error level
        let (level, _) = detect_raw_line_level("errorCount: 5");
        assert_eq!(level, LogLevel::Info, "errorCount should be Info");

        let (level, _) = detect_raw_line_level("hasError = false");
        assert_eq!(level, LogLevel::Info, "hasError should be Info");

        let (level, _) = detect_raw_line_level("isErrorState check");
        assert_eq!(level, LogLevel::Info, "isErrorState should be Info");
    }

    #[test]
    fn test_camel_case_not_error() {
        // CamelCase identifiers should NOT trigger Error level
        let (level, _) = detect_raw_line_level("NetworkError");
        assert_eq!(level, LogLevel::Info, "NetworkError should be Info");

        let (level, _) = detect_raw_line_level("ValidationError");
        assert_eq!(level, LogLevel::Info, "ValidationError should be Info");

        let (level, _) = detect_raw_line_level("TimeoutError");
        assert_eq!(level, LogLevel::Info, "TimeoutError should be Info");
    }

    #[test]
    fn test_valid_error_detection_still_works() {
        // Valid error patterns should still trigger Error level
        let (level, _) = detect_raw_line_level("Error: something failed");
        assert_eq!(level, LogLevel::Error, "Error: should trigger Error");

        let (level, _) = detect_raw_line_level("An error occurred");
        assert_eq!(
            level,
            LogLevel::Error,
            "standalone error should trigger Error"
        );

        let (level, _) = detect_raw_line_level("[error] message");
        assert_eq!(level, LogLevel::Error, "[error] should trigger Error");

        let (level, _) = detect_raw_line_level("fatal error");
        assert_eq!(level, LogLevel::Error, "fatal error should trigger Error");
    }

    #[test]
    fn test_warning_class_name_not_warning() {
        // Warning class names should NOT trigger Warning level
        let (level, _) = detect_raw_line_level("WarningDialog");
        assert_eq!(level, LogLevel::Info, "WarningDialog should be Info");

        let (level, _) = detect_raw_line_level("ShowWarningBanner");
        assert_eq!(level, LogLevel::Info, "ShowWarningBanner should be Info");
    }

    #[test]
    fn test_valid_warning_detection_still_works() {
        // Valid warning patterns should still trigger Warning level
        let (level, _) = detect_raw_line_level("Warning: deprecated API");
        assert_eq!(level, LogLevel::Warning, "Warning: should trigger Warning");

        let (level, _) = detect_raw_line_level("[warning] check this");
        assert_eq!(level, LogLevel::Warning, "[warning] should trigger Warning");

        let (level, _) = detect_raw_line_level("⚠ Warning message");
        assert_eq!(
            level,
            LogLevel::Warning,
            "emoji warning should trigger Warning"
        );
    }

    #[test]
    fn test_build_failure_still_detected() {
        // Build failures should still trigger Error level
        let (level, _) = detect_raw_line_level("FAILURE: Build failed");
        assert_eq!(
            level,
            LogLevel::Error,
            "FAILURE: Build failed should be Error"
        );

        let (level, _) = detect_raw_line_level("Build failed with errors");
        assert_eq!(level, LogLevel::Error, "Build failed should be Error");
    }

    #[test]
    fn test_emoji_error_still_detected() {
        // Emoji-based errors should still trigger Error level
        let (level, _) = detect_raw_line_level("⛔ Error: failed");
        assert_eq!(level, LogLevel::Error);

        let (level, _) = detect_raw_line_level("❌ Build failed");
        assert_eq!(level, LogLevel::Error);

        let (level, _) = detect_raw_line_level("🔥 Fatal error");
        assert_eq!(level, LogLevel::Error);
    }
}
