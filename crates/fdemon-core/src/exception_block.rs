//! Exception block parser for Flutter framework exceptions.
//!
//! Provides a line-by-line state machine parser that detects Flutter framework exception blocks
//! (══╡ EXCEPTION CAUGHT BY ╞══), accumulates their lines, and extracts structured information
//! including the library name, error message, error-causing widget, and stack trace.

use crate::ansi::strip_ansi_codes;
use crate::stack_trace::ParsedStackTrace;
use crate::types::{LogEntry, LogLevel, LogSource};

/// Maximum lines to buffer in an exception block before force-completing
const MAX_EXCEPTION_BLOCK_LINES: usize = 500;

// ─────────────────────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────────────────────

/// A parsed Flutter framework exception block
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExceptionBlock {
    /// The library that caught the exception (e.g., "WIDGETS LIBRARY", "RENDERING LIBRARY")
    pub library: String,

    /// The error description lines (between header and stack trace)
    pub description: String,

    /// The error-causing widget name, if identified
    pub widget_name: Option<String>,

    /// The error-causing widget source location, if identified
    pub widget_location: Option<String>,

    /// Raw stack trace text (all #N frame lines joined with newlines)
    pub stack_trace_text: String,

    /// Total lines in the original block (for diagnostics)
    pub line_count: usize,
}

impl ExceptionBlock {
    /// Convert to a LogEntry with parsed stack trace
    pub fn to_log_entry(&self) -> LogEntry {
        let message = self.format_summary();

        // Parse stack trace using existing infrastructure
        let stack_trace = if !self.stack_trace_text.is_empty() {
            let parsed = ParsedStackTrace::parse(&self.stack_trace_text);
            if parsed.has_frames() {
                Some(parsed)
            } else {
                None
            }
        } else {
            None
        };

        // Create LogEntry at Error level
        if let Some(trace) = stack_trace {
            LogEntry::with_stack_trace(LogLevel::Error, LogSource::Flutter, message, trace)
        } else {
            LogEntry::error(LogSource::Flutter, message)
        }
    }

    /// Format a compact summary for the log message
    pub fn format_summary(&self) -> String {
        // Extract the first meaningful line from description
        let first_line = self
            .description
            .lines()
            .map(|l| l.trim())
            .find(|l| !l.is_empty())
            .unwrap_or("");

        // Format: "[EXCEPTION] <LIBRARY>: <first line>"
        let mut summary = format!("[EXCEPTION] {}: {}", self.library, first_line);

        // Append widget name if available
        if let Some(widget) = &self.widget_name {
            summary.push_str(&format!(" — {}", widget));
        }

        // Truncate to ~120 chars for readability
        if summary.len() > 120 {
            summary.truncate(117);
            summary.push_str("...");
        }

        summary
    }
}

/// Result of feeding a line to the parser
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeedResult {
    /// Line was consumed and buffered (part of an exception block)
    Buffered,

    /// Line is not part of an exception block — caller should handle it normally
    NotConsumed,

    /// An exception block is complete
    Complete(ExceptionBlock),

    /// A "Another exception was thrown:" one-liner was detected
    OneLineException(String),
}

/// Parser states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParserState {
    /// Waiting for an exception block to start
    Idle,

    /// Inside the body (between header and stack trace)
    InBody,

    /// Inside the stack trace section
    InStackTrace,
}

// ─────────────────────────────────────────────────────────────────────────────
// Parser
// ─────────────────────────────────────────────────────────────────────────────

/// Line-by-line state machine parser for Flutter exception blocks
#[derive(Debug)]
pub struct ExceptionBlockParser {
    /// Current parser state
    state: ParserState,

    /// Library name from the header
    library: String,

    /// Accumulated description lines
    description_lines: Vec<String>,

    /// Accumulated stack trace lines
    stack_trace_lines: Vec<String>,

    /// Widget name, if identified
    widget_name: Option<String>,

    /// Widget location, if identified
    widget_location: Option<String>,

    /// Total lines accumulated
    line_count: usize,

    /// Flag to capture next line as widget name
    capture_widget_next: bool,
}

impl ExceptionBlockParser {
    /// Create a new parser in Idle state
    pub fn new() -> Self {
        Self {
            state: ParserState::Idle,
            library: String::new(),
            description_lines: Vec::new(),
            stack_trace_lines: Vec::new(),
            widget_name: None,
            widget_location: None,
            line_count: 0,
            capture_widget_next: false,
        }
    }

    /// Feed a line to the parser and get the result
    pub fn feed_line(&mut self, line: &str) -> FeedResult {
        // Strip ANSI codes, then strip "flutter: " prefix (present in app.log events)
        let stripped = strip_ansi_codes(line);
        let clean_line = stripped
            .strip_prefix("flutter: ")
            .or_else(|| stripped.strip_prefix("flutter:"))
            .unwrap_or(&stripped)
            .to_string();

        match self.state {
            ParserState::Idle => self.handle_idle(&clean_line),
            ParserState::InBody => self.handle_in_body(&clean_line),
            ParserState::InStackTrace => self.handle_in_stack_trace(&clean_line),
        }
    }

    /// Flush incomplete block and reset state
    pub fn flush(&mut self) -> Option<ExceptionBlock> {
        if self.state == ParserState::Idle {
            return None;
        }

        let block = self.build_block();
        self.reset();
        Some(block)
    }

    /// Reset parser to Idle state
    pub fn reset(&mut self) {
        self.state = ParserState::Idle;
        self.library.clear();
        self.description_lines.clear();
        self.stack_trace_lines.clear();
        self.widget_name = None;
        self.widget_location = None;
        self.line_count = 0;
        self.capture_widget_next = false;
    }

    // ─────────────────────────────────────────────────────────────────────────
    // State Handlers
    // ─────────────────────────────────────────────────────────────────────────

    fn handle_idle(&mut self, line: &str) -> FeedResult {
        // Check for "Another exception was thrown:" one-liner
        if let Some(msg) = is_another_exception(line) {
            return FeedResult::OneLineException(msg);
        }

        // Check for exception header
        if let Some(library) = extract_library_from_header(line) {
            self.state = ParserState::InBody;
            self.library = library;
            self.line_count = 1;
            return FeedResult::Buffered;
        }

        FeedResult::NotConsumed
    }

    fn handle_in_body(&mut self, line: &str) -> FeedResult {
        self.line_count += 1;

        // Check for new exception header (force-complete current block, start new one)
        if let Some(library) = extract_library_from_header(line) {
            let block = self.build_block();
            self.reset();
            // Start the new exception block
            self.state = ParserState::InBody;
            self.library = library;
            self.line_count = 1;
            return FeedResult::Complete(block);
        }

        // Check for footer (complete without stack trace)
        if is_exception_footer(line) {
            let block = self.build_block();
            self.reset();
            return FeedResult::Complete(block);
        }

        // Check for stack trace marker
        if is_stack_trace_marker(line) {
            self.state = ParserState::InStackTrace;
            return FeedResult::Buffered;
        }

        // Check for direct stack frame (# followed by digit)
        let trimmed_for_frame = line.trim_start();
        if trimmed_for_frame.starts_with('#')
            && trimmed_for_frame
                .chars()
                .nth(1)
                .is_some_and(|c| c.is_ascii_digit())
        {
            self.state = ParserState::InStackTrace;
            self.stack_trace_lines.push(line.to_string());
            return FeedResult::Buffered;
        }

        // Check for widget info marker
        if is_widget_info_marker(line) {
            self.capture_widget_next = true;
            return FeedResult::Buffered;
        }

        // Capture widget name and location
        if self.capture_widget_next {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                if self.widget_name.is_none() {
                    // First non-empty line after marker is widget name
                    self.widget_name = Some(trimmed.to_string());
                } else if trimmed.contains(':')
                    && trimmed.starts_with(self.widget_name.as_deref().unwrap_or(""))
                {
                    // Line starting with widget name and containing ':' is likely the location
                    self.widget_location = Some(trimmed.to_string());
                    self.capture_widget_next = false;
                } else {
                    // Doesn't match location pattern — stop capturing
                    self.capture_widget_next = false;
                }
            } else if self.widget_name.is_some() {
                // Empty line after widget name — widget section is done
                self.capture_widget_next = false;
            }
            return FeedResult::Buffered;
        }

        // Accumulate description line
        self.description_lines.push(line.to_string());

        // Check safety limit
        if self.line_count >= MAX_EXCEPTION_BLOCK_LINES {
            let block = self.build_block();
            self.reset();
            return FeedResult::Complete(block);
        }

        FeedResult::Buffered
    }

    fn handle_in_stack_trace(&mut self, line: &str) -> FeedResult {
        self.line_count += 1;

        // Check for new exception header (force-complete current block, start new one)
        if let Some(library) = extract_library_from_header(line) {
            let block = self.build_block();
            self.reset();
            self.state = ParserState::InBody;
            self.library = library;
            self.line_count = 1;
            return FeedResult::Complete(block);
        }

        // Check for footer (complete)
        if is_exception_footer(line) {
            let block = self.build_block();
            self.reset();
            return FeedResult::Complete(block);
        }

        // Accumulate stack trace line
        self.stack_trace_lines.push(line.to_string());

        // Check safety limit
        if self.line_count >= MAX_EXCEPTION_BLOCK_LINES {
            let block = self.build_block();
            self.reset();
            return FeedResult::Complete(block);
        }

        FeedResult::Buffered
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Helpers
    // ─────────────────────────────────────────────────────────────────────────

    fn build_block(&self) -> ExceptionBlock {
        ExceptionBlock {
            library: self.library.clone(),
            description: self.description_lines.join("\n"),
            widget_name: self.widget_name.clone(),
            widget_location: self.widget_location.clone(),
            stack_trace_text: self.stack_trace_lines.join("\n"),
            line_count: self.line_count,
        }
    }
}

impl Default for ExceptionBlockParser {
    fn default() -> Self {
        Self::new()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Detection Functions
// ─────────────────────────────────────────────────────────────────────────────

/// Check if a line is an exception header (used in tests)
#[cfg(test)]
fn is_exception_header(line: &str) -> bool {
    let clean = strip_ansi_codes(line);
    clean.contains("EXCEPTION CAUGHT BY") && clean.contains("══╡")
}

/// Extract library name from exception header
fn extract_library_from_header(line: &str) -> Option<String> {
    let clean = strip_ansi_codes(line);

    if !clean.contains("EXCEPTION CAUGHT BY") {
        return None;
    }

    // Find "EXCEPTION CAUGHT BY " and extract until " ╞"
    if let Some(start_idx) = clean.find("EXCEPTION CAUGHT BY ") {
        let after_prefix = &clean[start_idx + 20..]; // "EXCEPTION CAUGHT BY " is 20 chars
        if let Some(end_idx) = after_prefix.find(" ╞") {
            return Some(after_prefix[..end_idx].to_string());
        }
        // Fallback: take until end if no closing marker
        return Some(after_prefix.trim().to_string());
    }

    None
}

/// Check if a line is an exception footer (all ═ characters)
fn is_exception_footer(line: &str) -> bool {
    let clean = strip_ansi_codes(line).trim().to_string();

    // Must be at least 10 characters to avoid false positives
    if clean.len() < 10 {
        return false;
    }

    // Check if all characters are ═
    clean.chars().all(|c| c == '═')
}

/// Check if a line is "Another exception was thrown:" and extract message
fn is_another_exception(line: &str) -> Option<String> {
    let clean = strip_ansi_codes(line);
    let prefix = "Another exception was thrown: ";

    if let Some(idx) = clean.find(prefix) {
        let message = clean[idx + prefix.len()..].trim();
        if !message.is_empty() {
            return Some(message.to_string());
        }
    }

    None
}

/// Check if a line is the stack trace marker
fn is_stack_trace_marker(line: &str) -> bool {
    let clean = strip_ansi_codes(line);
    clean.contains("When the exception was thrown, this was the stack:")
}

/// Check if a line is the widget info marker
fn is_widget_info_marker(line: &str) -> bool {
    let clean = strip_ansi_codes(line);
    clean.contains("The relevant error-causing widget was:")
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ─────────────────────────────────────────────────────────────────────────
    // Header Detection
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_detects_exception_header() {
        assert!(is_exception_header(
            "══╡ EXCEPTION CAUGHT BY WIDGETS LIBRARY ╞═══════════"
        ));
    }

    #[test]
    fn test_detects_exception_header_with_ansi() {
        assert!(is_exception_header(
            "\x1b[38;5;196m══╡ EXCEPTION CAUGHT BY RENDERING LIBRARY ╞═══\x1b[0m"
        ));
    }

    #[test]
    fn test_non_exception_header() {
        assert!(!is_exception_header("Just a normal log line"));
        assert!(!is_exception_header("══════════════════════")); // no EXCEPTION CAUGHT BY
    }

    #[test]
    fn test_extract_library_from_header() {
        let line = "══╡ EXCEPTION CAUGHT BY WIDGETS LIBRARY ╞═══════════";
        assert_eq!(
            extract_library_from_header(line),
            Some("WIDGETS LIBRARY".to_string())
        );

        let line2 = "\x1b[38;5;196m══╡ EXCEPTION CAUGHT BY RENDERING LIBRARY ╞═══\x1b[0m";
        assert_eq!(
            extract_library_from_header(line2),
            Some("RENDERING LIBRARY".to_string())
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Footer Detection
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_detects_exception_footer() {
        assert!(is_exception_footer(
            "════════════════════════════════════════════════════════"
        ));
    }

    #[test]
    fn test_short_equals_not_footer() {
        assert!(!is_exception_footer("═══")); // too short
    }

    #[test]
    fn test_footer_with_ansi() {
        assert!(is_exception_footer(
            "\x1b[38;5;196m════════════════════════════════════════════════════════\x1b[0m"
        ));
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Another Exception One-Liner
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_detects_another_exception() {
        let result =
            is_another_exception("Another exception was thrown: RangeError (index): Invalid value");
        assert_eq!(
            result,
            Some("RangeError (index): Invalid value".to_string())
        );
    }

    #[test]
    fn test_another_exception_with_ansi() {
        let result =
            is_another_exception("\x1b[31mAnother exception was thrown: Some error\x1b[0m");
        assert_eq!(result, Some("Some error".to_string()));
    }

    #[test]
    fn test_not_another_exception() {
        assert_eq!(is_another_exception("Just a normal line"), None);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Full Block Parsing
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_parse_complete_exception_block() {
        let mut parser = ExceptionBlockParser::new();

        let lines = vec![
            "══╡ EXCEPTION CAUGHT BY WIDGETS LIBRARY ╞═══════════",
            "The following assertion was thrown building _CodeLine:",
            "'package:flutter/src/widgets/container.dart': Failed assertion: line 270",
            "",
            "The relevant error-causing widget was:",
            "  _CodeLine",
            "  _CodeLine:file:///Users/ed/.../ide_code_viewer.dart:72:22",
            "",
            "When the exception was thrown, this was the stack:",
            "#0      new Container (package:flutter/src/widgets/container.dart:270:15)",
            "#1      _CodeLine.build (package:zabin_app/.../ide_code_viewer.dart:141:16)",
            "(elided 2 frames from class _AssertionError)",
            "════════════════════════════════════════════════════════",
        ];

        let mut result = None;
        for line in &lines {
            match parser.feed_line(line) {
                FeedResult::Complete(block) => {
                    result = Some(block);
                    break;
                }
                FeedResult::Buffered => {}
                _ => panic!("unexpected result for line: {}", line),
            }
        }

        let block = result.expect("should have completed");
        assert_eq!(block.library, "WIDGETS LIBRARY");
        assert!(block.description.contains("Failed assertion"));
        assert_eq!(block.widget_name.as_deref(), Some("_CodeLine"));
        assert!(block.stack_trace_text.contains("#0"));
        assert!(block.stack_trace_text.contains("#1"));
    }

    #[test]
    fn test_parse_exception_block_to_log_entry() {
        // Parse a block, convert to LogEntry, verify stack trace is parsed
        let block = ExceptionBlock {
            library: "WIDGETS LIBRARY".to_string(),
            description: "Failed assertion: 'margin.isNonNegative'".to_string(),
            widget_name: Some("_CodeLine".to_string()),
            widget_location: Some("file:///Users/ed/.../file.dart:72:22".to_string()),
            stack_trace_text: "#0      new Container (package:flutter/src/widgets/container.dart:270:15)\n#1      _CodeLine.build (package:zabin_app/.../file.dart:141:16)".to_string(),
            line_count: 13,
        };

        let entry = block.to_log_entry();
        assert_eq!(entry.level, LogLevel::Error);
        assert!(entry.stack_trace.is_some());
        assert_eq!(entry.stack_trace.as_ref().unwrap().frame_count(), 2);
    }

    #[test]
    fn test_safety_limit_forces_completion() {
        let mut parser = ExceptionBlockParser::new();

        // Feed header
        parser.feed_line("══╡ EXCEPTION CAUGHT BY WIDGETS LIBRARY ╞═══════════");

        // Feed lines until safety limit
        for i in 0..MAX_EXCEPTION_BLOCK_LINES {
            let result = parser.feed_line(&format!("line {}", i));
            if matches!(result, FeedResult::Complete(_)) {
                // Should force-complete at the limit
                return;
            }
        }

        panic!("parser should have force-completed at line limit");
    }

    #[test]
    fn test_non_exception_lines_pass_through() {
        let mut parser = ExceptionBlockParser::new();

        assert!(matches!(
            parser.feed_line("Just a normal log line"),
            FeedResult::NotConsumed
        ));
        assert!(matches!(
            parser.feed_line("flutter: Hello World"),
            FeedResult::NotConsumed
        ));
    }

    #[test]
    fn test_parser_resets_after_completion() {
        let mut parser = ExceptionBlockParser::new();

        // Parse one block
        parser.feed_line("══╡ EXCEPTION CAUGHT BY WIDGETS LIBRARY ╞═══════════");
        parser.feed_line("Error description");
        let result = parser.feed_line("════════════════════════════════════════════════════════");
        assert!(matches!(result, FeedResult::Complete(_)));

        // Parser should be back to Idle
        assert!(matches!(
            parser.feed_line("Normal line after exception"),
            FeedResult::NotConsumed
        ));
    }

    #[test]
    fn test_flush_incomplete_returns_partial_block() {
        let mut parser = ExceptionBlockParser::new();

        parser.feed_line("══╡ EXCEPTION CAUGHT BY WIDGETS LIBRARY ╞═══════════");
        parser.feed_line("Error description");
        // Don't send footer — simulate session exit

        let partial = parser.flush();
        assert!(partial.is_some());
        let block = partial.unwrap();
        assert_eq!(block.library, "WIDGETS LIBRARY");
    }

    #[test]
    fn test_format_summary_basic() {
        let block = ExceptionBlock {
            library: "WIDGETS LIBRARY".to_string(),
            description: "The following assertion was thrown".to_string(),
            widget_name: None,
            widget_location: None,
            stack_trace_text: String::new(),
            line_count: 5,
        };

        let summary = block.format_summary();
        assert!(summary.contains("[EXCEPTION]"));
        assert!(summary.contains("WIDGETS LIBRARY"));
        assert!(summary.contains("assertion"));
    }

    #[test]
    fn test_format_summary_with_widget() {
        let block = ExceptionBlock {
            library: "WIDGETS LIBRARY".to_string(),
            description: "Error occurred".to_string(),
            widget_name: Some("MyWidget".to_string()),
            widget_location: None,
            stack_trace_text: String::new(),
            line_count: 5,
        };

        let summary = block.format_summary();
        assert!(summary.contains("[EXCEPTION]"));
        assert!(summary.contains("MyWidget"));
    }

    #[test]
    fn test_format_summary_truncates_long_text() {
        let long_desc = "A".repeat(150);
        let block = ExceptionBlock {
            library: "WIDGETS LIBRARY".to_string(),
            description: long_desc,
            widget_name: None,
            widget_location: None,
            stack_trace_text: String::new(),
            line_count: 5,
        };

        let summary = block.format_summary();
        assert!(summary.len() <= 120);
        assert!(summary.ends_with("..."));
    }

    #[test]
    fn test_one_line_exception() {
        let mut parser = ExceptionBlockParser::new();
        let result =
            parser.feed_line("Another exception was thrown: RangeError (index): Invalid value");

        match result {
            FeedResult::OneLineException(msg) => {
                assert_eq!(msg, "RangeError (index): Invalid value");
            }
            _ => panic!("Expected OneLineException"),
        }
    }

    #[test]
    fn test_exception_without_stack_trace() {
        let mut parser = ExceptionBlockParser::new();

        parser.feed_line("══╡ EXCEPTION CAUGHT BY WIDGETS LIBRARY ╞═══════════");
        parser.feed_line("Error description");
        let result = parser.feed_line("════════════════════════════════════════════════════════");

        match result {
            FeedResult::Complete(block) => {
                assert_eq!(block.library, "WIDGETS LIBRARY");
                assert!(block.stack_trace_text.is_empty());
            }
            _ => panic!("Expected Complete"),
        }
    }

    #[test]
    fn test_direct_stack_frame_transition() {
        let mut parser = ExceptionBlockParser::new();

        parser.feed_line("══╡ EXCEPTION CAUGHT BY WIDGETS LIBRARY ╞═══════════");
        parser.feed_line("Error description");
        // Direct stack frame without marker
        parser.feed_line("#0      main (package:app/main.dart:15:3)");
        let result = parser.feed_line("════════════════════════════════════════════════════════");

        match result {
            FeedResult::Complete(block) => {
                assert!(block.stack_trace_text.contains("#0"));
            }
            _ => panic!("Expected Complete"),
        }
    }

    #[test]
    fn test_multiline_description() {
        let mut parser = ExceptionBlockParser::new();

        parser.feed_line("══╡ EXCEPTION CAUGHT BY WIDGETS LIBRARY ╞═══════════");
        parser.feed_line("Line 1 of description");
        parser.feed_line("Line 2 of description");
        parser.feed_line("Line 3 of description");
        let result = parser.feed_line("════════════════════════════════════════════════════════");

        match result {
            FeedResult::Complete(block) => {
                assert!(block.description.contains("Line 1"));
                assert!(block.description.contains("Line 2"));
                assert!(block.description.contains("Line 3"));
            }
            _ => panic!("Expected Complete"),
        }
    }

    #[test]
    fn test_parse_exception_block_with_flutter_prefix() {
        let mut parser = ExceptionBlockParser::new();

        // Lines as they arrive via app.log events (with "flutter: " prefix)
        let lines = vec![
            "flutter: ══╡ EXCEPTION CAUGHT BY WIDGETS LIBRARY ╞═══════════",
            "flutter: The following assertion was thrown building _CodeLine:",
            "flutter: 'package:flutter/src/widgets/container.dart': Failed assertion: line 270",
            "flutter: ",
            "flutter: The relevant error-causing widget was:",
            "flutter:   _CodeLine",
            "flutter:   _CodeLine:file:///Users/ed/.../ide_code_viewer.dart:72:22",
            "flutter: ",
            "flutter: When the exception was thrown, this was the stack:",
            "flutter: #0      new Container (package:flutter/src/widgets/container.dart:270:15)",
            "flutter: #1      _CodeLine.build (package:zabin_app/.../ide_code_viewer.dart:141:16)",
            "flutter: ════════════════════════════════════════════════════════",
        ];

        let mut result = None;
        for line in &lines {
            match parser.feed_line(line) {
                FeedResult::Complete(block) => {
                    result = Some(block);
                    break;
                }
                FeedResult::Buffered => {}
                _ => panic!("unexpected result for line: {}", line),
            }
        }

        let block = result.expect("should have completed");
        assert_eq!(block.library, "WIDGETS LIBRARY");
        assert!(block.description.contains("Failed assertion"));
        assert_eq!(block.widget_name.as_deref(), Some("_CodeLine"));
        assert!(block.stack_trace_text.contains("#0"));
        assert!(block.stack_trace_text.contains("#1"));
    }

    #[test]
    fn test_flutter_prefix_footer_detected() {
        let mut parser = ExceptionBlockParser::new();

        parser.feed_line("flutter: ══╡ EXCEPTION CAUGHT BY WIDGETS LIBRARY ╞═══════════");
        parser.feed_line("flutter: Error description");
        let result =
            parser.feed_line("flutter: ════════════════════════════════════════════════════════");

        assert!(
            matches!(result, FeedResult::Complete(_)),
            "Footer with flutter: prefix should be detected"
        );
    }

    #[test]
    fn test_flutter_prefix_another_exception() {
        let mut parser = ExceptionBlockParser::new();

        let result = parser
            .feed_line("flutter: Another exception was thrown: RangeError (index): Invalid value");

        match result {
            FeedResult::OneLineException(msg) => {
                assert_eq!(msg, "RangeError (index): Invalid value");
            }
            _ => panic!("Expected OneLineException"),
        }
    }

    #[test]
    fn test_indented_stack_frame_detection() {
        let mut parser = ExceptionBlockParser::new();

        parser.feed_line("══╡ EXCEPTION CAUGHT BY WIDGETS LIBRARY ╞═══════════");
        parser.feed_line("Error description");
        // Indented stack frame (common in Flutter output)
        parser.feed_line("  #0      main (package:app/main.dart:15:3)");
        let result = parser.feed_line("════════════════════════════════════════════════════════");

        match result {
            FeedResult::Complete(block) => {
                assert!(
                    block.stack_trace_text.contains("#0"),
                    "Indented stack frames should be captured"
                );
            }
            _ => panic!("Expected Complete"),
        }
    }

    #[test]
    fn test_capture_widget_next_safety_reset() {
        let mut parser = ExceptionBlockParser::new();

        parser.feed_line("══╡ EXCEPTION CAUGHT BY WIDGETS LIBRARY ╞═══════════");
        parser.feed_line("Error description");
        parser.feed_line("The relevant error-causing widget was:");
        parser.feed_line("  MyWidget");
        // Next line doesn't match location pattern — should stop capturing
        parser.feed_line("  Some unrelated line");
        // This description line should be accumulated, not eaten by widget capture
        parser.feed_line("More description");
        let result = parser.feed_line("════════════════════════════════════════════════════════");

        match result {
            FeedResult::Complete(block) => {
                assert_eq!(block.widget_name.as_deref(), Some("MyWidget"));
                assert!(
                    block.description.contains("More description"),
                    "Lines after widget capture reset should be accumulated as description"
                );
            }
            _ => panic!("Expected Complete"),
        }
    }

    #[test]
    fn test_widget_extraction() {
        let mut parser = ExceptionBlockParser::new();

        parser.feed_line("══╡ EXCEPTION CAUGHT BY WIDGETS LIBRARY ╞═══════════");
        parser.feed_line("Error description");
        parser.feed_line("The relevant error-causing widget was:");
        parser.feed_line("  MyWidget");
        parser.feed_line("  MyWidget:file:///path/to/file.dart:42:10");
        let result = parser.feed_line("════════════════════════════════════════════════════════");

        match result {
            FeedResult::Complete(block) => {
                assert_eq!(block.widget_name.as_deref(), Some("MyWidget"));
                assert!(block.widget_location.is_some());
            }
            _ => panic!("Expected Complete"),
        }
    }

    #[test]
    fn test_empty_line_after_widget_name_resets_capture() {
        let mut parser = ExceptionBlockParser::new();

        parser.feed_line("══╡ EXCEPTION CAUGHT BY WIDGETS LIBRARY ╞═══════════");
        parser.feed_line("Error description");
        parser.feed_line("The relevant error-causing widget was:");
        parser.feed_line("  _CodeLine");
        // Empty line after widget name — must reset capture mode
        parser.feed_line("");
        // Stack trace marker should NOT be eaten by widget capture
        parser.feed_line("When the exception was thrown, this was the stack:");
        parser
            .feed_line("#0      new Container (package:flutter/src/widgets/container.dart:270:15)");
        let result = parser.feed_line("════════════════════════════════════════════════════════");

        match result {
            FeedResult::Complete(block) => {
                assert_eq!(block.widget_name.as_deref(), Some("_CodeLine"));
                assert!(
                    block.stack_trace_text.contains("#0"),
                    "Stack trace should be captured after empty line resets widget capture"
                );
            }
            _ => panic!("Expected Complete, got {:?}", result),
        }
    }

    #[test]
    fn test_empty_line_after_widget_footer_detected() {
        let mut parser = ExceptionBlockParser::new();

        parser.feed_line("══╡ EXCEPTION CAUGHT BY WIDGETS LIBRARY ╞═══════════");
        parser.feed_line("Error description");
        parser.feed_line("The relevant error-causing widget was:");
        parser.feed_line("  MyWidget");
        parser.feed_line(""); // empty line
                              // Footer should be detected, not eaten by widget capture
        let result = parser.feed_line("════════════════════════════════════════════════════════");

        assert!(
            matches!(result, FeedResult::Complete(_)),
            "Footer after empty line should complete the block"
        );
    }

    #[test]
    fn test_real_world_renderflex_overflow() {
        let mut parser = ExceptionBlockParser::new();

        let lines = vec![
            "══╡ EXCEPTION CAUGHT BY RENDERING LIBRARY ╞═════════════════════",
            "The following assertion was thrown during layout:",
            "A RenderFlex overflowed by 4028 pixels on the right.",
            "",
            "The relevant error-causing widget was:",
            "  Row",
            "",
            "When the exception was thrown, this was the stack:",
            "#0      RenderFlex.performLayout (package:flutter/src/rendering/flex.dart:999:15)",
            "#1      RenderObject.layout (package:flutter/src/rendering/object.dart:2521:7)",
            "════════════════════════════════════════════════════════════════════",
        ];

        let mut result = None;
        for line in &lines {
            match parser.feed_line(line) {
                FeedResult::Complete(block) => {
                    result = Some(block);
                    break;
                }
                FeedResult::Buffered => {}
                other => panic!("Unexpected result for line {:?}: {:?}", line, other),
            }
        }

        let block = result.expect("Block should have completed");
        assert_eq!(block.library, "RENDERING LIBRARY");
        assert!(block.description.contains("RenderFlex overflowed"));
        assert_eq!(block.widget_name.as_deref(), Some("Row"));
        assert!(block.stack_trace_text.contains("#0"));
        assert!(block.stack_trace_text.contains("#1"));
    }

    #[test]
    fn test_back_to_back_exceptions_without_footer() {
        let mut parser = ExceptionBlockParser::new();

        // First exception block — no footer before second header
        parser.feed_line("══╡ EXCEPTION CAUGHT BY WIDGETS LIBRARY ╞═══════════");
        parser.feed_line("First error description");

        // Second exception header should force-complete the first
        let result = parser.feed_line("══╡ EXCEPTION CAUGHT BY RENDERING LIBRARY ╞═══════════");

        match result {
            FeedResult::Complete(block) => {
                assert_eq!(block.library, "WIDGETS LIBRARY");
                assert!(block.description.contains("First error"));
            }
            _ => panic!("Expected Complete for first block"),
        }

        // Continue the second block
        parser.feed_line("Second error description");
        let result = parser.feed_line("════════════════════════════════════════════════════════");

        match result {
            FeedResult::Complete(block) => {
                assert_eq!(block.library, "RENDERING LIBRARY");
                assert!(block.description.contains("Second error"));
            }
            _ => panic!("Expected Complete for second block"),
        }
    }

    #[test]
    fn test_new_header_in_stack_trace_completes_block() {
        let mut parser = ExceptionBlockParser::new();

        parser.feed_line("══╡ EXCEPTION CAUGHT BY WIDGETS LIBRARY ╞═══════════");
        parser.feed_line("Error description");
        parser.feed_line("#0      main (package:app/main.dart:15:3)");

        // New header while in stack trace state
        let result = parser.feed_line("══╡ EXCEPTION CAUGHT BY RENDERING LIBRARY ╞═══════════");

        match result {
            FeedResult::Complete(block) => {
                assert_eq!(block.library, "WIDGETS LIBRARY");
                assert!(block.stack_trace_text.contains("#0"));
            }
            _ => panic!("Expected Complete"),
        }

        // Second block continues normally
        parser.feed_line("Second error");
        let result = parser.feed_line("════════════════════════════════════════════════════════");
        assert!(matches!(result, FeedResult::Complete(_)));
    }

    #[test]
    fn test_normal_lines_not_eaten_after_exception() {
        let mut parser = ExceptionBlockParser::new();

        // Complete exception with widget section and empty line
        parser.feed_line("══╡ EXCEPTION CAUGHT BY WIDGETS LIBRARY ╞═══════════");
        parser.feed_line("Error description");
        parser.feed_line("The relevant error-causing widget was:");
        parser.feed_line("  MyWidget");
        parser.feed_line(""); // empty line
        parser.feed_line("════════════════════════════════════════════════════════");

        // Normal lines after should NOT be consumed
        let result = parser.feed_line("flutter: Normal log message");
        assert!(
            matches!(result, FeedResult::NotConsumed),
            "Normal lines after completed exception should pass through"
        );
    }

    #[test]
    fn test_flutter_colon_no_space_stripped_as_empty() {
        // "flutter:" (no space) should be treated as empty line by the parser.
        // This happens when parse_flutter_log trims "flutter: " → "flutter:".
        let mut parser = ExceptionBlockParser::new();

        parser.feed_line("══╡ EXCEPTION CAUGHT BY WIDGETS LIBRARY ╞═══════════");
        parser.feed_line("Error description");
        parser.feed_line("The relevant error-causing widget was:");
        parser.feed_line("  MyWidget");
        // "flutter:" (no space) should be treated as empty line → reset capture
        parser.feed_line("flutter:");
        // Footer should be detected (not eaten by widget capture)
        let result = parser.feed_line("════════════════════════════════════════════════════════");

        assert!(
            matches!(result, FeedResult::Complete(_)),
            "flutter: without space should be stripped and treated as empty line"
        );
    }
}
