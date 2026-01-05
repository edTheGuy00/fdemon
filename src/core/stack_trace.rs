//! Stack trace parsing types and utilities for Dart/Flutter stack traces.
//!
//! This module provides types for representing and working with parsed stack traces
//! from Flutter applications, including support for multiple Dart stack trace formats.

use regex::Regex;
use std::sync::LazyLock;

use super::ansi::strip_ansi_codes;

// ─────────────────────────────────────────────────────────────────────────────
// Regex Patterns
// ─────────────────────────────────────────────────────────────────────────────

/// Matches Dart VM stack trace format: `#0      main (package:app/main.dart:15:3)`
/// Captures: 1=frame_number, 2=function_name, 3=file_path, 4=line, 5=column
pub static DART_VM_FRAME_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"#(\d+)\s+(.+?)\s+\((.+?):(\d+):(\d+)\)").expect("Invalid DART_VM_FRAME_REGEX")
});

/// Matches Dart VM stack trace format without column: `#0      main (package:app/main.dart:15)`
/// Captures: 1=frame_number, 2=function_name, 3=file_path, 4=line
pub static DART_VM_FRAME_NO_COL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"#(\d+)\s+(.+?)\s+\((.+?):(\d+)\)").expect("Invalid DART_VM_FRAME_NO_COL_REGEX")
});

/// Matches the friendly/package_trace format: `package:app/main.dart 15:3  main`
/// Captures: 1=file_path, 2=line, 3=column, 4=function_name
pub static FRIENDLY_FRAME_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(.+?)\s+(\d+):(\d+)\s+(.+)$").expect("Invalid FRIENDLY_FRAME_REGEX")
});

/// Matches async suspension markers
pub static ASYNC_GAP_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<asynchronous suspension>").expect("Invalid ASYNC_GAP_REGEX"));

/// Matches package frame paths (SDK/pub cache packages to dim)
/// - `dart:` prefix (Dart SDK internals)
/// - `package:flutter/` (Flutter SDK)
/// - paths containing `.pub-cache` (pub packages)
pub static PACKAGE_PATH_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(dart:|package:flutter/)|\.pub-cache").expect("Invalid PACKAGE_PATH_REGEX")
});

// ─────────────────────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────────────────────

/// Stack trace format variants for parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StackTraceFormat {
    /// Standard Dart VM: `#0 function (file:line:col)`
    DartVm,

    /// Flutter/package format (same as DartVm but from Flutter framework)
    Flutter,

    /// Friendly format: `file line:col function`
    Friendly,

    /// Unknown/unparseable format
    #[default]
    Unknown,
}

/// Represents a single frame in a stack trace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StackFrame {
    /// Frame number (e.g., 0, 1, 2 from #0, #1, #2)
    pub frame_number: usize,

    /// Function/method name (e.g., "main", "State.setState")
    pub function_name: String,

    /// File path (e.g., "package:app/main.dart", "dart:isolate-patch/...")
    pub file_path: String,

    /// Line number (1-based)
    pub line: u32,

    /// Column number (1-based)
    pub column: u32,

    /// Whether this is a package/SDK frame (should be dimmed)
    pub is_package_frame: bool,

    /// Whether this is an async suspension marker
    pub is_async_gap: bool,
}

impl StackFrame {
    /// Create a new stack frame with all fields.
    pub fn new(
        frame_number: usize,
        function_name: impl Into<String>,
        file_path: impl Into<String>,
        line: u32,
        column: u32,
    ) -> Self {
        let file_path = file_path.into();
        let is_package_frame = is_package_path(&file_path);

        Self {
            frame_number,
            function_name: function_name.into(),
            file_path,
            line,
            column,
            is_package_frame,
            is_async_gap: false,
        }
    }

    /// Create an async gap marker frame.
    pub fn async_gap(frame_number: usize) -> Self {
        Self {
            frame_number,
            function_name: String::new(),
            file_path: String::new(),
            line: 0,
            column: 0,
            is_package_frame: false,
            is_async_gap: true,
        }
    }

    /// Returns true if this is a project frame (not package/SDK, not async gap).
    pub fn is_project_frame(&self) -> bool {
        !self.is_package_frame && !self.is_async_gap
    }

    /// Returns formatted location string: "file:line:col"
    pub fn display_location(&self) -> String {
        if self.is_async_gap {
            return "<asynchronous suspension>".to_string();
        }
        format!("{}:{}:{}", self.file_path, self.line, self.column)
    }

    /// Extracts just the filename from the full path.
    ///
    /// Examples:
    /// - `package:my_app/src/utils/helpers.dart` -> `helpers.dart`
    /// - `dart:isolate-patch/isolate_patch.dart` -> `isolate_patch.dart`
    /// - `/absolute/path/file.dart` -> `file.dart`
    pub fn short_path(&self) -> &str {
        if self.is_async_gap {
            return "";
        }

        // Find the last '/' and return everything after it
        self.file_path.rsplit('/').next().unwrap_or(&self.file_path)
    }
}

/// Represents a parsed stack trace with multiple frames.
#[derive(Debug, Clone, Default)]
pub struct ParsedStackTrace {
    /// Original raw stack trace string
    pub raw: String,

    /// Parsed frames
    pub frames: Vec<StackFrame>,

    /// Whether parsing was fully successful
    pub is_complete: bool,

    /// Detected format of the stack trace
    pub format: StackTraceFormat,
}

impl ParsedStackTrace {
    /// Create a new parsed stack trace with the raw string.
    pub fn new(raw: impl Into<String>) -> Self {
        Self {
            raw: raw.into(),
            frames: Vec::new(),
            is_complete: false,
            format: StackTraceFormat::Unknown,
        }
    }

    /// Add a parsed frame to the stack trace.
    pub fn add_frame(&mut self, frame: StackFrame) {
        self.frames.push(frame);
    }

    /// Returns true if the stack trace has any frames.
    pub fn has_frames(&self) -> bool {
        !self.frames.is_empty()
    }

    /// Returns the number of frames.
    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    /// Returns an iterator over non-package (project) frames.
    pub fn project_frames(&self) -> impl Iterator<Item = &StackFrame> {
        self.frames.iter().filter(|f| f.is_project_frame())
    }

    /// Returns the first N frames for collapsed view.
    pub fn visible_frames(&self, max: usize) -> impl Iterator<Item = &StackFrame> {
        self.frames.iter().take(max)
    }

    /// Returns the count of frames beyond max (hidden when collapsed).
    pub fn hidden_count(&self, max: usize) -> usize {
        self.frames.len().saturating_sub(max)
    }

    /// Returns the first project frame (if any).
    pub fn first_project_frame(&self) -> Option<&StackFrame> {
        self.project_frames().next()
    }

    /// Mark the stack trace as completely parsed.
    pub fn mark_complete(&mut self) {
        self.is_complete = true;
    }

    /// Parse a raw stack trace string into structured frames.
    ///
    /// Automatically detects the format (Dart VM or Friendly) and applies
    /// the appropriate parser. Handles edge cases like async gaps,
    /// missing column numbers, and whitespace.
    ///
    /// ANSI escape codes are automatically stripped from the input.
    pub fn parse(raw: &str) -> Self {
        // Strip ANSI escape codes first (from Logger package, etc.)
        let cleaned = strip_ansi_codes(raw);
        let mut trace = Self::new(&cleaned);
        trace.format = detect_format(&cleaned);

        // Track frame number for async gaps (which don't have their own number)
        let mut next_frame_number = 0usize;

        for line in cleaned.lines() {
            let line = line.trim();

            // Skip empty lines
            if line.is_empty() {
                continue;
            }

            // Check for async gap marker
            if ASYNC_GAP_REGEX.is_match(line) {
                trace.add_frame(StackFrame::async_gap(next_frame_number));
                next_frame_number += 1;
                continue;
            }

            // Try parsing based on detected format, with fallback
            let frame = match trace.format {
                StackTraceFormat::DartVm | StackTraceFormat::Flutter => parse_dart_vm_line(line)
                    .or_else(|| parse_friendly_line(line, next_frame_number)),
                StackTraceFormat::Friendly => parse_friendly_line(line, next_frame_number)
                    .or_else(|| parse_dart_vm_line(line)),
                StackTraceFormat::Unknown => {
                    // Try both formats
                    parse_dart_vm_line(line)
                        .or_else(|| parse_friendly_line(line, next_frame_number))
                }
            };

            if let Some(f) = frame {
                next_frame_number = f.frame_number + 1;
                trace.add_frame(f);
            }
        }

        // Mark complete if we parsed at least one frame
        if trace.has_frames() {
            trace.mark_complete();
        }

        trace
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Parsing Functions
// ─────────────────────────────────────────────────────────────────────────────

/// Detect the format of a stack trace string by examining its content.
///
/// Returns the detected format or `Unknown` if the format cannot be determined.
pub fn detect_format(trace: &str) -> StackTraceFormat {
    // Find the first non-empty, non-async-gap line
    for line in trace.lines() {
        let line = line.trim();
        if line.is_empty() || ASYNC_GAP_REGEX.is_match(line) {
            continue;
        }

        // Check for Dart VM format: starts with #N
        if line.starts_with('#') && DART_VM_FRAME_REGEX.is_match(line) {
            // Check if it's from Flutter framework
            if line.contains("package:flutter/") {
                return StackTraceFormat::Flutter;
            }
            return StackTraceFormat::DartVm;
        }

        // Also check for Dart VM format without column
        if line.starts_with('#') && DART_VM_FRAME_NO_COL_REGEX.is_match(line) {
            if line.contains("package:flutter/") {
                return StackTraceFormat::Flutter;
            }
            return StackTraceFormat::DartVm;
        }

        // Check for friendly format
        if FRIENDLY_FRAME_REGEX.is_match(line) {
            return StackTraceFormat::Friendly;
        }

        // If first meaningful line doesn't match any format, return Unknown
        return StackTraceFormat::Unknown;
    }

    StackTraceFormat::Unknown
}

/// Parse a single line in Dart VM format.
///
/// Returns `Some(StackFrame)` if the line matches, `None` otherwise.
fn parse_dart_vm_line(line: &str) -> Option<StackFrame> {
    // Try with column first
    if let Some(caps) = DART_VM_FRAME_REGEX.captures(line) {
        let frame_number = caps[1].parse().unwrap_or(0);
        let function_name = caps[2].to_string();
        let file_path = caps[3].to_string();
        let line_num = caps[4].parse().unwrap_or(0);
        let column = caps[5].parse().unwrap_or(0);

        return Some(StackFrame::new(
            frame_number,
            function_name,
            file_path,
            line_num,
            column,
        ));
    }

    // Try without column
    if let Some(caps) = DART_VM_FRAME_NO_COL_REGEX.captures(line) {
        let frame_number = caps[1].parse().unwrap_or(0);
        let function_name = caps[2].to_string();
        let file_path = caps[3].to_string();
        let line_num = caps[4].parse().unwrap_or(0);

        return Some(StackFrame::new(
            frame_number,
            function_name,
            file_path,
            line_num,
            0, // Default column to 0
        ));
    }

    None
}

/// Parse a single line in friendly format.
///
/// Returns `Some(StackFrame)` if the line matches, `None` otherwise.
/// The `fallback_frame_number` is used since friendly format doesn't include frame numbers.
fn parse_friendly_line(line: &str, fallback_frame_number: usize) -> Option<StackFrame> {
    if let Some(caps) = FRIENDLY_FRAME_REGEX.captures(line) {
        let file_path = caps[1].to_string();
        let line_num = caps[2].parse().unwrap_or(0);
        let column = caps[3].parse().unwrap_or(0);
        let function_name = caps[4].to_string();

        return Some(StackFrame::new(
            fallback_frame_number,
            function_name,
            file_path,
            line_num,
            column,
        ));
    }

    None
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper Functions
// ─────────────────────────────────────────────────────────────────────────────

/// Determines if a file path represents a package/SDK frame that should be dimmed.
///
/// Package frames include:
/// - `dart:` prefix (Dart SDK internals)
/// - `package:flutter/` (Flutter SDK)
/// - Paths containing `.pub-cache` (pub packages)
///
/// Project frames include:
/// - `package:app_name/` where app_name matches project
/// - Paths with `lib/` or `test/` directories
pub fn is_package_path(path: &str) -> bool {
    // Match dart: prefix (SDK internals)
    if path.starts_with("dart:") {
        return true;
    }

    // Match package:flutter/ (Flutter SDK)
    if path.starts_with("package:flutter/") {
        return true;
    }

    // Match .pub-cache paths (pub packages)
    if path.contains(".pub-cache") {
        return true;
    }

    // Additional common SDK packages that should be dimmed
    if path.starts_with("package:flutter_test/")
        || path.starts_with("package:test/")
        || path.starts_with("package:test_api/")
        || path.starts_with("package:async/")
        || path.starts_with("package:stream_channel/")
        || path.starts_with("package:matcher/")
    {
        return true;
    }

    false
}

/// Determines if a file path represents a project frame (not SDK/package).
///
/// This is the inverse of `is_package_path`.
pub fn is_project_path(path: &str) -> bool {
    !is_package_path(path)
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // StackFrame tests

    #[test]
    fn test_stack_frame_creation() {
        let frame = StackFrame::new(0, "main", "package:app/main.dart", 15, 3);

        assert_eq!(frame.frame_number, 0);
        assert_eq!(frame.function_name, "main");
        assert_eq!(frame.file_path, "package:app/main.dart");
        assert_eq!(frame.line, 15);
        assert_eq!(frame.column, 3);
        assert!(!frame.is_package_frame); // project package
        assert!(!frame.is_async_gap);
    }

    #[test]
    fn test_stack_frame_display_location() {
        let frame = StackFrame::new(0, "main", "package:app/main.dart", 15, 3);
        assert_eq!(frame.display_location(), "package:app/main.dart:15:3");
    }

    #[test]
    fn test_stack_frame_async_gap_display() {
        let frame = StackFrame::async_gap(1);
        assert_eq!(frame.display_location(), "<asynchronous suspension>");
    }

    #[test]
    fn test_short_path_package_format() {
        let frame = StackFrame::new(0, "main", "package:my_app/src/utils/helpers.dart", 10, 5);
        assert_eq!(frame.short_path(), "helpers.dart");
    }

    #[test]
    fn test_short_path_dart_sdk() {
        let frame = StackFrame::new(0, "run", "dart:isolate-patch/isolate_patch.dart", 307, 1);
        assert_eq!(frame.short_path(), "isolate_patch.dart");
    }

    #[test]
    fn test_short_path_no_slash() {
        let frame = StackFrame::new(0, "main", "main.dart", 1, 1);
        assert_eq!(frame.short_path(), "main.dart");
    }

    #[test]
    fn test_short_path_async_gap() {
        let frame = StackFrame::async_gap(0);
        assert_eq!(frame.short_path(), "");
    }

    #[test]
    fn test_is_project_frame() {
        // Project frame
        let project = StackFrame::new(0, "main", "package:my_app/main.dart", 1, 1);
        assert!(project.is_project_frame());

        // Package frame (Flutter SDK)
        let flutter = StackFrame::new(
            0,
            "setState",
            "package:flutter/src/widgets/framework.dart",
            1187,
            9,
        );
        assert!(!flutter.is_project_frame());

        // Async gap
        let async_gap = StackFrame::async_gap(1);
        assert!(!async_gap.is_project_frame());
    }

    #[test]
    fn test_async_gap_frame() {
        let frame = StackFrame::async_gap(2);
        assert_eq!(frame.frame_number, 2);
        assert!(frame.is_async_gap);
        assert!(!frame.is_project_frame());
        assert!(frame.function_name.is_empty());
        assert!(frame.file_path.is_empty());
    }

    // Package detection tests

    #[test]
    fn test_is_package_path_dart_sdk() {
        assert!(is_package_path("dart:isolate-patch/isolate_patch.dart"));
        assert!(is_package_path("dart:async/future.dart"));
        assert!(is_package_path("dart:core/object.dart"));
    }

    #[test]
    fn test_is_package_path_flutter_sdk() {
        assert!(is_package_path(
            "package:flutter/src/widgets/framework.dart"
        ));
        assert!(is_package_path("package:flutter/material.dart"));
    }

    #[test]
    fn test_is_package_path_pub_cache() {
        assert!(is_package_path(
            "/Users/dev/.pub-cache/hosted/pub.dev/http-0.13.0/lib/http.dart"
        ));
        assert!(is_package_path(
            "~/.pub-cache/hosted/pub.dev/provider/lib/provider.dart"
        ));
    }

    #[test]
    fn test_is_package_path_common_packages() {
        assert!(is_package_path("package:flutter_test/flutter_test.dart"));
        assert!(is_package_path("package:test/test.dart"));
        assert!(is_package_path("package:async/async.dart"));
    }

    #[test]
    fn test_is_package_path_project_packages() {
        // App packages should NOT be marked as package frames
        assert!(!is_package_path("package:my_app/main.dart"));
        assert!(!is_package_path("package:sample/src/widget.dart"));
        assert!(!is_package_path(
            "package:cool_app/features/login/login_page.dart"
        ));
    }

    #[test]
    fn test_is_project_path() {
        assert!(is_project_path("package:my_app/main.dart"));
        assert!(is_project_path("lib/main.dart"));
        assert!(!is_project_path("dart:core/object.dart"));
        assert!(!is_project_path("package:flutter/material.dart"));
    }

    // ParsedStackTrace tests

    #[test]
    fn test_parsed_stack_trace_new() {
        let trace = ParsedStackTrace::new("#0 main (package:app/main.dart:15:3)");
        assert_eq!(trace.raw, "#0 main (package:app/main.dart:15:3)");
        assert!(trace.frames.is_empty());
        assert!(!trace.is_complete);
        assert_eq!(trace.format, StackTraceFormat::Unknown);
    }

    #[test]
    fn test_parsed_stack_trace_add_frame() {
        let mut trace = ParsedStackTrace::new("test");
        assert!(!trace.has_frames());
        assert_eq!(trace.frame_count(), 0);

        trace.add_frame(StackFrame::new(0, "main", "package:app/main.dart", 1, 1));
        assert!(trace.has_frames());
        assert_eq!(trace.frame_count(), 1);
    }

    #[test]
    fn test_parsed_stack_trace_visible_frames() {
        let mut trace = ParsedStackTrace::new("test");
        for i in 0..10 {
            trace.add_frame(StackFrame::new(
                i,
                format!("func{i}"),
                "file.dart",
                i as u32,
                1,
            ));
        }

        assert_eq!(trace.visible_frames(5).count(), 5);
        assert_eq!(trace.visible_frames(15).count(), 10); // capped at actual count
    }

    #[test]
    fn test_parsed_stack_trace_hidden_count() {
        let mut trace = ParsedStackTrace::new("test");
        for i in 0..10 {
            trace.add_frame(StackFrame::new(
                i,
                format!("func{i}"),
                "file.dart",
                i as u32,
                1,
            ));
        }

        assert_eq!(trace.hidden_count(5), 5);
        assert_eq!(trace.hidden_count(10), 0);
        assert_eq!(trace.hidden_count(15), 0); // can't be negative
    }

    #[test]
    fn test_parsed_stack_trace_project_frames() {
        let mut trace = ParsedStackTrace::new("test");

        // Add project frame
        trace.add_frame(StackFrame::new(0, "main", "package:my_app/main.dart", 1, 1));

        // Add package frame
        trace.add_frame(StackFrame::new(
            1,
            "setState",
            "package:flutter/src/widgets/framework.dart",
            100,
            5,
        ));

        // Add async gap
        trace.add_frame(StackFrame::async_gap(2));

        // Add another project frame
        trace.add_frame(StackFrame::new(
            3,
            "build",
            "package:my_app/widget.dart",
            50,
            10,
        ));

        let project_frames: Vec<_> = trace.project_frames().collect();
        assert_eq!(project_frames.len(), 2);
        assert_eq!(project_frames[0].function_name, "main");
        assert_eq!(project_frames[1].function_name, "build");
    }

    #[test]
    fn test_parsed_stack_trace_first_project_frame() {
        let mut trace = ParsedStackTrace::new("test");

        // Add package frame first
        trace.add_frame(StackFrame::new(
            0,
            "setState",
            "package:flutter/src/widgets/framework.dart",
            100,
            5,
        ));

        // Then project frame
        trace.add_frame(StackFrame::new(1, "main", "package:my_app/main.dart", 1, 1));

        let first = trace.first_project_frame().unwrap();
        assert_eq!(first.function_name, "main");
    }

    #[test]
    fn test_parsed_stack_trace_no_project_frames() {
        let mut trace = ParsedStackTrace::new("test");
        trace.add_frame(StackFrame::new(
            0,
            "setState",
            "package:flutter/src/widgets/framework.dart",
            100,
            5,
        ));

        assert!(trace.first_project_frame().is_none());
    }

    #[test]
    fn test_parsed_stack_trace_mark_complete() {
        let mut trace = ParsedStackTrace::new("test");
        assert!(!trace.is_complete);

        trace.mark_complete();
        assert!(trace.is_complete);
    }

    // Regex pattern tests

    #[test]
    fn test_dart_vm_frame_regex_matches() {
        let line = "#0      main (package:app/main.dart:15:3)";
        let caps = DART_VM_FRAME_REGEX.captures(line).unwrap();

        assert_eq!(&caps[1], "0"); // frame number
        assert_eq!(&caps[2], "main"); // function name
        assert_eq!(&caps[3], "package:app/main.dart"); // file path
        assert_eq!(&caps[4], "15"); // line
        assert_eq!(&caps[5], "3"); // column
    }

    #[test]
    fn test_dart_vm_frame_regex_anonymous_closure() {
        let line = "#1      _startIsolate.<anonymous closure> (dart:isolate-patch/isolate_patch.dart:307:1)";
        let caps = DART_VM_FRAME_REGEX.captures(line).unwrap();

        assert_eq!(&caps[1], "1");
        assert_eq!(&caps[2], "_startIsolate.<anonymous closure>");
        assert_eq!(&caps[3], "dart:isolate-patch/isolate_patch.dart");
        assert_eq!(&caps[4], "307");
        assert_eq!(&caps[5], "1");
    }

    #[test]
    fn test_dart_vm_frame_regex_flutter_framework() {
        let line = "#0      State.setState.<anonymous closure> (package:flutter/src/widgets/framework.dart:1187:9)";
        let caps = DART_VM_FRAME_REGEX.captures(line).unwrap();

        assert_eq!(&caps[1], "0");
        assert_eq!(&caps[2], "State.setState.<anonymous closure>");
        assert_eq!(&caps[3], "package:flutter/src/widgets/framework.dart");
        assert_eq!(&caps[4], "1187");
        assert_eq!(&caps[5], "9");
    }

    #[test]
    fn test_friendly_frame_regex_matches() {
        let line = "package:app/main.dart 15:3                main";
        let caps = FRIENDLY_FRAME_REGEX.captures(line).unwrap();

        assert_eq!(&caps[1], "package:app/main.dart"); // file path
        assert_eq!(&caps[2], "15"); // line
        assert_eq!(&caps[3], "3"); // column
        assert_eq!(&caps[4], "main"); // function name
    }

    #[test]
    fn test_friendly_frame_regex_with_anonymous() {
        let line =
            "package:flutter/src/widgets/framework.dart 1187:9  State.setState.<anonymous closure>";
        let caps = FRIENDLY_FRAME_REGEX.captures(line).unwrap();

        assert_eq!(&caps[1], "package:flutter/src/widgets/framework.dart");
        assert_eq!(&caps[2], "1187");
        assert_eq!(&caps[3], "9");
        assert_eq!(&caps[4], "State.setState.<anonymous closure>");
    }

    #[test]
    fn test_async_gap_regex_matches() {
        let line = "<asynchronous suspension>";
        assert!(ASYNC_GAP_REGEX.is_match(line));
    }

    #[test]
    fn test_async_gap_regex_embedded() {
        // Async gap can appear on its own line
        let line = "  <asynchronous suspension>  ";
        assert!(ASYNC_GAP_REGEX.is_match(line));
    }

    // StackTraceFormat tests

    #[test]
    fn test_stack_trace_format_default() {
        assert_eq!(StackTraceFormat::default(), StackTraceFormat::Unknown);
    }

    #[test]
    fn test_stack_trace_format_equality() {
        assert_eq!(StackTraceFormat::DartVm, StackTraceFormat::DartVm);
        assert_ne!(StackTraceFormat::DartVm, StackTraceFormat::Friendly);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Parsing tests (Task 2)
    // ─────────────────────────────────────────────────────────────────────────

    const SAMPLE_DART_VM_TRACE: &str = r#"
#0      main (package:sample/main.dart:15:3)
#1      _startIsolate.<anonymous closure> (dart:isolate-patch/isolate_patch.dart:307:19)
#2      _RawReceivePort._handleMessage (dart:isolate-patch/isolate_patch.dart:174:12)
"#;

    const SAMPLE_ASYNC_TRACE: &str = r#"
#0      someAsyncFunction (package:app/utils.dart:23:7)
<asynchronous suspension>
#1      main (package:app/main.dart:10:3)
"#;

    const SAMPLE_FLUTTER_TRACE: &str = r#"
#0      State.setState.<anonymous closure> (package:flutter/src/widgets/framework.dart:1187:9)
#1      State.setState (package:flutter/src/widgets/framework.dart:1222:6)
#2      _MyHomePageState._incrementCounter (package:sample/main.dart:45:5)
"#;

    // detect_format tests

    #[test]
    fn test_detect_dart_vm_format() {
        let format = detect_format(SAMPLE_DART_VM_TRACE);
        assert_eq!(format, StackTraceFormat::DartVm);
    }

    #[test]
    fn test_detect_flutter_format() {
        let format = detect_format(SAMPLE_FLUTTER_TRACE);
        assert_eq!(format, StackTraceFormat::Flutter);
    }

    #[test]
    fn test_detect_friendly_format() {
        let trace = "package:app/main.dart 15:3  main\npackage:app/utils.dart 20:5  helper";
        let format = detect_format(trace);
        assert_eq!(format, StackTraceFormat::Friendly);
    }

    #[test]
    fn test_detect_unknown_format() {
        let format = detect_format("not a stack trace at all");
        assert_eq!(format, StackTraceFormat::Unknown);
    }

    #[test]
    fn test_detect_empty_trace() {
        let format = detect_format("");
        assert_eq!(format, StackTraceFormat::Unknown);
    }

    #[test]
    fn test_detect_format_skips_async_gap() {
        let trace = "<asynchronous suspension>\n#0      main (package:app/main.dart:15:3)";
        let format = detect_format(trace);
        assert_eq!(format, StackTraceFormat::DartVm);
    }

    // ParsedStackTrace::parse tests

    #[test]
    fn test_parse_dart_vm_trace() {
        let trace = ParsedStackTrace::parse(SAMPLE_DART_VM_TRACE);

        assert_eq!(trace.frames.len(), 3);
        assert!(trace.is_complete);
        assert_eq!(trace.format, StackTraceFormat::DartVm);

        let first = &trace.frames[0];
        assert_eq!(first.frame_number, 0);
        assert_eq!(first.function_name, "main");
        assert_eq!(first.file_path, "package:sample/main.dart");
        assert_eq!(first.line, 15);
        assert_eq!(first.column, 3);
        assert!(!first.is_package_frame);
    }

    #[test]
    fn test_parse_dart_vm_trace_package_detection() {
        let trace = ParsedStackTrace::parse(SAMPLE_DART_VM_TRACE);

        // First frame is project frame
        assert!(!trace.frames[0].is_package_frame);

        // Second and third are SDK frames
        assert!(trace.frames[1].is_package_frame);
        assert!(trace.frames[2].is_package_frame);
    }

    #[test]
    fn test_parse_async_trace() {
        let trace = ParsedStackTrace::parse(SAMPLE_ASYNC_TRACE);

        assert_eq!(trace.frames.len(), 3);

        // First frame is normal
        assert!(!trace.frames[0].is_async_gap);
        assert_eq!(trace.frames[0].function_name, "someAsyncFunction");

        // Second frame is async gap
        assert!(trace.frames[1].is_async_gap);

        // Third frame is normal
        assert!(!trace.frames[2].is_async_gap);
        assert_eq!(trace.frames[2].function_name, "main");
    }

    #[test]
    fn test_parse_flutter_trace_package_detection() {
        let trace = ParsedStackTrace::parse(SAMPLE_FLUTTER_TRACE);

        assert_eq!(trace.format, StackTraceFormat::Flutter);

        // Flutter framework frames are package frames
        assert!(trace.frames[0].is_package_frame);
        assert!(trace.frames[1].is_package_frame);

        // App frame is NOT a package frame
        assert!(!trace.frames[2].is_package_frame);
    }

    #[test]
    fn test_parse_anonymous_closure() {
        let line = "#0      State.setState.<anonymous closure> (package:flutter/src/widgets/framework.dart:1187:9)";
        let trace = ParsedStackTrace::parse(line);

        assert_eq!(trace.frames.len(), 1);
        assert_eq!(
            trace.frames[0].function_name,
            "State.setState.<anonymous closure>"
        );
    }

    #[test]
    fn test_parse_nested_closure() {
        let line =
            "#0      main.<anonymous closure>.<anonymous closure> (package:app/main.dart:15:3)";
        let trace = ParsedStackTrace::parse(line);

        assert_eq!(trace.frames.len(), 1);
        assert_eq!(
            trace.frames[0].function_name,
            "main.<anonymous closure>.<anonymous closure>"
        );
    }

    #[test]
    fn test_parse_empty_trace() {
        let trace = ParsedStackTrace::parse("");
        assert!(trace.frames.is_empty());
        assert!(!trace.is_complete);
    }

    #[test]
    fn test_parse_whitespace_handling() {
        let trace_with_whitespace = "  \n  #0      main (package:app/main.dart:15:3)  \n  \n";
        let trace = ParsedStackTrace::parse(trace_with_whitespace);

        assert_eq!(trace.frames.len(), 1);
        assert_eq!(trace.frames[0].function_name, "main");
    }

    #[test]
    fn test_parse_frame_without_column() {
        let line = "#0      main (package:app/main.dart:15)";
        let trace = ParsedStackTrace::parse(line);

        assert_eq!(trace.frames.len(), 1);
        assert_eq!(trace.frames[0].line, 15);
        assert_eq!(trace.frames[0].column, 0); // Default to 0
    }

    #[test]
    fn test_parse_friendly_format() {
        let trace_str = "package:app/main.dart 15:3  main";
        let trace = ParsedStackTrace::parse(trace_str);

        assert_eq!(trace.frames.len(), 1);
        assert_eq!(trace.format, StackTraceFormat::Friendly);
        assert_eq!(trace.frames[0].file_path, "package:app/main.dart");
        assert_eq!(trace.frames[0].line, 15);
        assert_eq!(trace.frames[0].column, 3);
        assert_eq!(trace.frames[0].function_name, "main");
    }

    #[test]
    fn test_parse_project_frames_iterator() {
        let trace = ParsedStackTrace::parse(SAMPLE_FLUTTER_TRACE);

        let project_frames: Vec<_> = trace.project_frames().collect();
        assert_eq!(project_frames.len(), 1);
        assert_eq!(
            project_frames[0].function_name,
            "_MyHomePageState._incrementCounter"
        );
    }

    #[test]
    fn test_dart_vm_no_col_regex_matches() {
        let line = "#0      main (package:app/main.dart:15)";
        let caps = DART_VM_FRAME_NO_COL_REGEX.captures(line).unwrap();

        assert_eq!(&caps[1], "0");
        assert_eq!(&caps[2], "main");
        assert_eq!(&caps[3], "package:app/main.dart");
        assert_eq!(&caps[4], "15");
    }

    #[test]
    fn test_parse_long_function_name() {
        let line = "#0      _SomeVeryLongPrivateClassName.someEvenLongerMethodName (package:app/file.dart:100:5)";
        let trace = ParsedStackTrace::parse(line);

        assert_eq!(trace.frames.len(), 1);
        assert_eq!(
            trace.frames[0].function_name,
            "_SomeVeryLongPrivateClassName.someEvenLongerMethodName"
        );
    }

    #[test]
    fn test_parse_file_uri_format() {
        // Some stack traces use file:// URIs
        let line = "#0      main (file:///path/to/file.dart:15:3)";
        let trace = ParsedStackTrace::parse(line);

        assert_eq!(trace.frames.len(), 1);
        assert_eq!(trace.frames[0].file_path, "file:///path/to/file.dart");
    }

    #[test]
    fn test_parse_preserves_raw_string() {
        let raw = "#0      main (package:app/main.dart:15:3)";
        let trace = ParsedStackTrace::parse(raw);

        assert_eq!(trace.raw, raw);
    }

    #[test]
    fn test_parse_large_frame_numbers() {
        let line = "#99      someFunction (package:app/file.dart:1000:50)";
        let trace = ParsedStackTrace::parse(line);

        assert_eq!(trace.frames.len(), 1);
        assert_eq!(trace.frames[0].frame_number, 99);
        assert_eq!(trace.frames[0].line, 1000);
        assert_eq!(trace.frames[0].column, 50);
    }

    #[test]
    fn test_parse_mixed_valid_invalid_lines() {
        let trace_str = r#"
#0      validFrame (package:app/file.dart:10:5)
this is not a valid frame
#1      anotherValid (package:app/other.dart:20:3)
"#;
        let trace = ParsedStackTrace::parse(trace_str);

        // Should only parse valid frames
        assert_eq!(trace.frames.len(), 2);
        assert_eq!(trace.frames[0].function_name, "validFrame");
        assert_eq!(trace.frames[1].function_name, "anotherValid");
    }

    #[test]
    fn test_parse_frame_number_continuity_with_async() {
        let trace = ParsedStackTrace::parse(SAMPLE_ASYNC_TRACE);

        // Frame numbers should be 0, 1, 2 (async gap gets its own number)
        assert_eq!(trace.frames[0].frame_number, 0);
        assert_eq!(trace.frames[1].frame_number, 1); // async gap
        assert_eq!(trace.frames[2].frame_number, 1); // from the trace itself
    }
}
