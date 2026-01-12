//! Core domain type definitions

use std::collections::VecDeque;

use chrono::{DateTime, Local};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::core::ansi::strip_ansi_codes;
use crate::core::stack_trace::ParsedStackTrace;

/// Application state enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppPhase {
    /// Application is initializing
    #[default]
    Initializing,
    /// Flutter process is running
    Running,
    /// Application is reloading
    Reloading,
    /// Application has stopped
    Stopped,
    /// Application is shutting down
    Quitting,
}

/// Counter for generating unique log entry IDs
static LOG_ENTRY_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Represents a log entry with timestamp
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: DateTime<Local>,
    pub level: LogLevel,
    pub source: LogSource,
    pub message: String,
    /// Parsed stack trace, if this is an error with a trace
    pub stack_trace: Option<ParsedStackTrace>,
    /// Unique ID for this entry (for collapse state tracking)
    pub id: u64,
}

impl LogEntry {
    /// Create a new log entry with current timestamp
    ///
    /// ANSI escape codes are automatically stripped from the message.
    pub fn new(level: LogLevel, source: LogSource, message: impl Into<String>) -> Self {
        // Strip ANSI escape codes from log messages (from Logger package, etc.)
        let cleaned_message = strip_ansi_codes(&message.into());
        Self {
            timestamp: Local::now(),
            level,
            source,
            message: cleaned_message,
            stack_trace: None,
            id: LOG_ENTRY_COUNTER.fetch_add(1, Ordering::Relaxed),
        }
    }

    /// Create a new log entry with a stack trace
    pub fn with_stack_trace(
        level: LogLevel,
        source: LogSource,
        message: impl Into<String>,
        trace: ParsedStackTrace,
    ) -> Self {
        let mut entry = Self::new(level, source, message);
        entry.stack_trace = Some(trace);
        entry
    }

    /// Create an info log entry
    pub fn info(source: LogSource, message: impl Into<String>) -> Self {
        Self::new(LogLevel::Info, source, message)
    }

    /// Create an error log entry
    pub fn error(source: LogSource, message: impl Into<String>) -> Self {
        Self::new(LogLevel::Error, source, message)
    }

    /// Create a warning log entry
    pub fn warn(source: LogSource, message: impl Into<String>) -> Self {
        Self::new(LogLevel::Warning, source, message)
    }

    /// Check if this entry has a stack trace
    pub fn has_stack_trace(&self) -> bool {
        self.stack_trace.is_some()
    }

    /// Get stack trace frame count
    pub fn stack_trace_frame_count(&self) -> usize {
        self.stack_trace
            .as_ref()
            .map(|t| t.frames.len())
            .unwrap_or(0)
    }

    /// Format timestamp for display
    pub fn formatted_time(&self) -> String {
        self.timestamp.format("%H:%M:%S").to_string()
    }

    /// Format for single-line display (without wrapping)
    pub fn display_line(&self) -> String {
        format!(
            "{} {} [{}] {}",
            self.formatted_time(),
            self.level.prefix(),
            self.source.prefix(),
            self.message
        )
    }

    /// Check if this is an error-level entry
    pub fn is_error(&self) -> bool {
        self.level == LogLevel::Error
    }

    /// Check if this is from Flutter
    pub fn is_flutter(&self) -> bool {
        matches!(self.source, LogSource::Flutter | LogSource::FlutterError)
    }
}

/// Log severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
}

impl LogLevel {
    /// Get display prefix for log level
    pub fn prefix(&self) -> &'static str {
        match self {
            LogLevel::Debug => "DBG",
            LogLevel::Info => "INF",
            LogLevel::Warning => "WRN",
            LogLevel::Error => "ERR",
        }
    }

    /// Get numeric severity value for comparison
    /// Higher values indicate more severe levels
    pub fn severity(&self) -> u8 {
        match self {
            LogLevel::Debug => 0,
            LogLevel::Info => 1,
            LogLevel::Warning => 2,
            LogLevel::Error => 3,
        }
    }

    /// Compare severity levels
    /// Returns true if this level is more severe than other
    pub fn is_more_severe_than(&self, other: &LogLevel) -> bool {
        self.severity() > other.severity()
    }

    /// Get the more severe of two log levels
    pub fn max_severity(self, other: LogLevel) -> LogLevel {
        if self.severity() >= other.severity() {
            self
        } else {
            other
        }
    }
}

/// Filter for log levels - controls which severity levels are displayed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LogLevelFilter {
    /// Show all log levels
    #[default]
    All,
    /// Show only errors
    Errors,
    /// Show warnings and errors
    Warnings,
    /// Show info, warnings, and errors
    Info,
    /// Show all levels (same as All, for consistency)
    Debug,
}

impl LogLevelFilter {
    /// Cycle to the next filter option (wraps around)
    pub fn cycle(self) -> Self {
        match self {
            LogLevelFilter::All => LogLevelFilter::Errors,
            LogLevelFilter::Errors => LogLevelFilter::Warnings,
            LogLevelFilter::Warnings => LogLevelFilter::Info,
            LogLevelFilter::Info => LogLevelFilter::Debug,
            LogLevelFilter::Debug => LogLevelFilter::All,
        }
    }

    /// Check if a log level passes this filter
    pub fn matches(&self, level: &LogLevel) -> bool {
        match self {
            LogLevelFilter::All | LogLevelFilter::Debug => true,
            LogLevelFilter::Errors => *level == LogLevel::Error,
            LogLevelFilter::Warnings => {
                matches!(level, LogLevel::Warning | LogLevel::Error)
            }
            LogLevelFilter::Info => {
                matches!(level, LogLevel::Info | LogLevel::Warning | LogLevel::Error)
            }
        }
    }

    /// Get a user-friendly display name for the filter
    pub fn display_name(&self) -> &'static str {
        match self {
            LogLevelFilter::All => "All levels",
            LogLevelFilter::Errors => "Errors only",
            LogLevelFilter::Warnings => "Warnings+",
            LogLevelFilter::Info => "Info+",
            LogLevelFilter::Debug => "Debug+",
        }
    }
}

/// Source of log messages
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogSource {
    /// Application/system messages
    App,
    /// Flutter daemon infrastructure messages
    Daemon,
    /// Flutter daemon stdout
    Flutter,
    /// Flutter daemon stderr
    FlutterError,
    /// File watcher
    Watcher,
}

impl LogSource {
    pub fn prefix(&self) -> &'static str {
        match self {
            LogSource::App => "app",
            LogSource::Daemon => "daemon",
            LogSource::Flutter => "flutter",
            LogSource::FlutterError => "flutter",
            LogSource::Watcher => "watch",
        }
    }
}

/// Filter for log sources - controls which sources are displayed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LogSourceFilter {
    /// Show all log sources
    #[default]
    All,
    /// Show only app logs
    App,
    /// Show only daemon logs
    Daemon,
    /// Show Flutter logs (includes Flutter and FlutterError)
    Flutter,
    /// Show only watcher logs
    Watcher,
}

impl LogSourceFilter {
    /// Cycle to the next filter option (wraps around)
    pub fn cycle(self) -> Self {
        match self {
            LogSourceFilter::All => LogSourceFilter::App,
            LogSourceFilter::App => LogSourceFilter::Daemon,
            LogSourceFilter::Daemon => LogSourceFilter::Flutter,
            LogSourceFilter::Flutter => LogSourceFilter::Watcher,
            LogSourceFilter::Watcher => LogSourceFilter::All,
        }
    }

    /// Check if a log source passes this filter
    pub fn matches(&self, source: &LogSource) -> bool {
        match self {
            LogSourceFilter::All => true,
            LogSourceFilter::App => *source == LogSource::App,
            LogSourceFilter::Daemon => *source == LogSource::Daemon,
            LogSourceFilter::Flutter => {
                matches!(source, LogSource::Flutter | LogSource::FlutterError)
            }
            LogSourceFilter::Watcher => *source == LogSource::Watcher,
        }
    }

    /// Get a user-friendly display name for the filter
    pub fn display_name(&self) -> &'static str {
        match self {
            LogSourceFilter::All => "All sources",
            LogSourceFilter::App => "App logs",
            LogSourceFilter::Daemon => "Daemon logs",
            LogSourceFilter::Flutter => "Flutter logs",
            LogSourceFilter::Watcher => "Watcher logs",
        }
    }
}

/// Combined filter state for both level and source filtering
#[derive(Debug, Clone, Default)]
pub struct FilterState {
    /// Filter by log level
    pub level_filter: LogLevelFilter,
    /// Filter by log source
    pub source_filter: LogSourceFilter,
}

impl FilterState {
    /// Reset all filters to their default (All) state
    pub fn reset(&mut self) {
        self.level_filter = LogLevelFilter::All;
        self.source_filter = LogSourceFilter::All;
    }

    /// Check if any filter is active (not set to All)
    pub fn is_active(&self) -> bool {
        self.level_filter != LogLevelFilter::All || self.source_filter != LogSourceFilter::All
    }

    /// Check if a log entry passes both filters
    pub fn matches(&self, entry: &LogEntry) -> bool {
        self.level_filter.matches(&entry.level) && self.source_filter.matches(&entry.source)
    }
}

/// Represents a single search match within a log entry
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchMatch {
    /// Index of the log entry containing the match
    pub entry_index: usize,
    /// Byte offset of match start within the message
    pub start: usize,
    /// Byte offset of match end within the message
    pub end: usize,
}

impl SearchMatch {
    /// Create a new search match
    pub fn new(entry_index: usize, start: usize, end: usize) -> Self {
        Self {
            entry_index,
            start,
            end,
        }
    }

    /// Get the length of the matched text
    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    /// Check if the match is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// State for log search functionality
#[derive(Debug, Clone, Default)]
pub struct SearchState {
    /// The current search query string
    pub query: String,
    /// Whether search mode is active (showing search input)
    pub is_active: bool,
    /// The validated regex pattern string (None if query is empty or invalid)
    pub pattern: Option<String>,
    /// Whether the current pattern is valid regex
    pub is_valid: bool,
    /// All matches found in the current log buffer
    pub matches: Vec<SearchMatch>,
    /// Current match index (for n/N navigation)
    pub current_match: Option<usize>,
    /// Error message if regex compilation failed
    pub error: Option<String>,
}

impl SearchState {
    /// Create a new default search state
    pub fn new() -> Self {
        Self::default()
    }

    /// Clear query, matches, and deactivate search
    pub fn clear(&mut self) {
        self.query.clear();
        self.is_active = false;
        self.pattern = None;
        self.is_valid = false;
        self.matches.clear();
        self.current_match = None;
        self.error = None;
    }

    /// Enter search mode
    pub fn activate(&mut self) {
        self.is_active = true;
    }

    /// Exit search mode but keep query and matches
    pub fn deactivate(&mut self) {
        self.is_active = false;
    }

    /// Set the search query and validate as regex
    pub fn set_query(&mut self, query: &str) {
        self.query = query.to_string();

        if query.is_empty() {
            self.pattern = None;
            self.is_valid = false;
            self.error = None;
            self.matches.clear();
            self.current_match = None;
            return;
        }

        // Validate the regex pattern
        match regex::Regex::new(query) {
            Ok(_) => {
                self.pattern = Some(query.to_string());
                self.is_valid = true;
                self.error = None;
            }
            Err(e) => {
                self.pattern = None;
                self.is_valid = false;
                self.error = Some(e.to_string());
            }
        }
    }

    /// Check if there are any matches
    pub fn has_matches(&self) -> bool {
        !self.matches.is_empty()
    }

    /// Get the number of matches
    pub fn match_count(&self) -> usize {
        self.matches.len()
    }

    /// Get the current match index (1-based for display)
    pub fn current_match_index(&self) -> Option<usize> {
        self.current_match.map(|i| i + 1)
    }

    /// Get the current match
    pub fn current_match(&self) -> Option<&SearchMatch> {
        self.current_match.and_then(|i| self.matches.get(i))
    }

    /// Move to the next match (wraps around)
    pub fn next_match(&mut self) {
        if self.matches.is_empty() {
            self.current_match = None;
            return;
        }

        self.current_match = Some(match self.current_match {
            Some(i) => (i + 1) % self.matches.len(),
            None => 0,
        });
    }

    /// Move to the previous match (wraps around)
    pub fn prev_match(&mut self) {
        if self.matches.is_empty() {
            self.current_match = None;
            return;
        }

        self.current_match = Some(match self.current_match {
            Some(0) => self.matches.len() - 1,
            Some(i) => i - 1,
            None => self.matches.len() - 1,
        });
    }

    /// Jump to a match by entry index (finds first match at or after the given entry)
    pub fn jump_to_match(&mut self, entry_index: usize) {
        if self.matches.is_empty() {
            self.current_match = None;
            return;
        }

        // Find the first match at or after the given entry index
        for (i, m) in self.matches.iter().enumerate() {
            if m.entry_index >= entry_index {
                self.current_match = Some(i);
                return;
            }
        }

        // If no match found at or after, wrap to first match
        self.current_match = Some(0);
    }

    /// Update the match list (typically called after search is performed)
    pub fn update_matches(&mut self, matches: Vec<SearchMatch>) {
        self.matches = matches;
        // Reset current match if it's now out of bounds
        if let Some(i) = self.current_match {
            if i >= self.matches.len() {
                self.current_match = if self.matches.is_empty() {
                    None
                } else {
                    Some(0)
                };
            }
        }
    }

    /// Format the search status for display
    pub fn display_status(&self) -> String {
        if self.query.is_empty() {
            return String::new();
        }

        if self.matches.is_empty() {
            return "[No matches]".to_string();
        }

        match self.current_match {
            Some(i) => format!("[{}/{} matches]", i + 1, self.matches.len()),
            None => format!("[{} matches]", self.matches.len()),
        }
    }

    /// Execute search against log entries and update matches
    /// Returns true if the match list changed
    pub fn execute_search(&mut self, logs: &VecDeque<LogEntry>) -> bool {
        // Clear if no query
        if self.query.is_empty() {
            let changed = !self.matches.is_empty();
            self.matches.clear();
            self.current_match = None;
            return changed;
        }

        // Try to compile regex (case-insensitive by default)
        let pattern = format!("(?i){}", &self.query);
        let regex = match regex::Regex::new(&pattern) {
            Ok(r) => {
                self.is_valid = true;
                self.error = None;
                self.pattern = Some(self.query.clone());
                r
            }
            Err(e) => {
                self.is_valid = false;
                self.error = Some(format!("Invalid regex: {}", e));
                self.matches.clear();
                self.current_match = None;
                return true;
            }
        };

        // Find all matches
        let mut new_matches = Vec::new();
        for (entry_index, entry) in logs.iter().enumerate() {
            for mat in regex.find_iter(&entry.message) {
                new_matches.push(SearchMatch {
                    entry_index,
                    start: mat.start(),
                    end: mat.end(),
                });
            }
        }

        let changed = new_matches != self.matches;
        self.matches = new_matches;

        // Update current match
        if self.matches.is_empty() {
            self.current_match = None;
        } else if self.current_match.is_none() {
            self.current_match = Some(0);
        } else if let Some(idx) = self.current_match {
            // Keep current if still valid, otherwise reset to 0
            if idx >= self.matches.len() {
                self.current_match = Some(0);
            }
        }

        changed
    }

    /// Get the log entry index of the current match (for scrolling)
    pub fn current_match_entry_index(&self) -> Option<usize> {
        self.current_match
            .and_then(|idx| self.matches.get(idx))
            .map(|m| m.entry_index)
    }

    /// Get all matches for a specific log entry index
    pub fn matches_for_entry(&self, entry_index: usize) -> Vec<&SearchMatch> {
        self.matches
            .iter()
            .filter(|m| m.entry_index == entry_index)
            .collect()
    }

    /// Check if a specific match is the current one
    pub fn is_current_match(&self, match_ref: &SearchMatch) -> bool {
        if let Some(current_idx) = self.current_match {
            if let Some(current) = self.matches.get(current_idx) {
                return current == match_ref;
            }
        }
        false
    }
}

/// Platform type for bootable devices
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    IOS,
    Android,
}

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Platform::IOS => write!(f, "iOS"),
            Platform::Android => write!(f, "Android"),
        }
    }
}

/// State of a bootable device
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DeviceState {
    #[default]
    Shutdown,
    Booted,
    Booting,
    ShuttingDown,
    Unknown,
}

impl std::fmt::Display for DeviceState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeviceState::Shutdown => write!(f, "Shutdown"),
            DeviceState::Booted => write!(f, "Booted"),
            DeviceState::Booting => write!(f, "Booting"),
            DeviceState::ShuttingDown => write!(f, "Shutting Down"),
            DeviceState::Unknown => write!(f, "Unknown"),
        }
    }
}

/// A bootable device (offline simulator or AVD)
///
/// Unlike `Device` which represents connected/running devices from Flutter,
/// this represents devices that can be booted but aren't currently running.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootableDevice {
    /// Unique identifier (UDID for iOS, AVD name for Android)
    pub id: String,

    /// Human-readable name
    pub name: String,

    /// Platform (iOS or Android)
    pub platform: Platform,

    /// Runtime version (e.g., "iOS 17.2", "API 33")
    pub runtime: String,

    /// Current state (Shutdown, Booted, etc.)
    pub state: DeviceState,
}

impl BootableDevice {
    /// Create a new bootable device
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        platform: Platform,
        runtime: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            platform,
            runtime: runtime.into(),
            state: DeviceState::Shutdown,
        }
    }

    /// Set the device state (builder pattern)
    pub fn with_state(mut self, state: DeviceState) -> Self {
        self.state = state;
        self
    }

    /// Check if this device can be booted
    pub fn can_boot(&self) -> bool {
        matches!(self.state, DeviceState::Shutdown | DeviceState::Unknown)
    }

    /// Get display string for the device
    pub fn display_string(&self) -> String {
        format!("{} ({})", self.name, self.runtime)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_entry_creation() {
        let entry = LogEntry::info(LogSource::App, "Test message");
        assert_eq!(entry.level, LogLevel::Info);
        assert_eq!(entry.source, LogSource::App);
        assert_eq!(entry.message, "Test message");
    }

    #[test]
    fn test_log_entry_formatted_time() {
        let entry = LogEntry::info(LogSource::App, "Test");
        let time = entry.formatted_time();
        // Should be in HH:MM:SS format
        assert_eq!(time.len(), 8);
        assert!(time.contains(':'));
    }

    #[test]
    fn test_log_level_prefix() {
        assert_eq!(LogLevel::Info.prefix(), "INF");
        assert_eq!(LogLevel::Error.prefix(), "ERR");
        assert_eq!(LogLevel::Warning.prefix(), "WRN");
        assert_eq!(LogLevel::Debug.prefix(), "DBG");
    }

    #[test]
    fn test_log_source_prefix() {
        assert_eq!(LogSource::App.prefix(), "app");
        assert_eq!(LogSource::Flutter.prefix(), "flutter");
        assert_eq!(LogSource::Watcher.prefix(), "watch");
    }

    #[test]
    fn test_display_line_format() {
        let entry = LogEntry::info(LogSource::App, "Test message");
        let line = entry.display_line();

        // Should contain all expected components
        assert!(line.contains("INF"));
        assert!(line.contains("[app]"));
        assert!(line.contains("Test message"));
        // Timestamp is 8 chars (HH:MM:SS)
        assert!(line.len() > 20);
    }

    #[test]
    fn test_is_error() {
        let error = LogEntry::error(LogSource::App, "error");
        let info = LogEntry::info(LogSource::App, "info");
        let warn = LogEntry::warn(LogSource::App, "warn");

        assert!(error.is_error());
        assert!(!info.is_error());
        assert!(!warn.is_error());
    }

    #[test]
    fn test_is_flutter() {
        let flutter = LogEntry::info(LogSource::Flutter, "test");
        let flutter_err = LogEntry::error(LogSource::FlutterError, "test");
        let app = LogEntry::info(LogSource::App, "test");
        let watcher = LogEntry::info(LogSource::Watcher, "test");

        assert!(flutter.is_flutter());
        assert!(flutter_err.is_flutter());
        assert!(!app.is_flutter());
        assert!(!watcher.is_flutter());
    }

    // ─────────────────────────────────────────────────────────
    // Stack trace integration tests (Phase 2 Task 4)
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_log_entry_with_stack_trace() {
        let trace = ParsedStackTrace::parse("#0 main (package:app/main.dart:15:3)");
        let entry = LogEntry::with_stack_trace(
            LogLevel::Error,
            LogSource::App,
            "Something went wrong",
            trace,
        );

        assert!(entry.has_stack_trace());
        assert_eq!(entry.stack_trace_frame_count(), 1);
        assert_eq!(entry.level, LogLevel::Error);
        assert_eq!(entry.message, "Something went wrong");
    }

    #[test]
    fn test_log_entry_without_stack_trace() {
        let entry = LogEntry::new(LogLevel::Info, LogSource::App, "Hello");

        assert!(!entry.has_stack_trace());
        assert_eq!(entry.stack_trace_frame_count(), 0);
    }

    #[test]
    fn test_log_entry_id_uniqueness() {
        let entry1 = LogEntry::new(LogLevel::Info, LogSource::App, "First");
        let entry2 = LogEntry::new(LogLevel::Info, LogSource::App, "Second");
        let entry3 = LogEntry::new(LogLevel::Info, LogSource::App, "Third");

        assert_ne!(entry1.id, entry2.id);
        assert_ne!(entry2.id, entry3.id);
        assert_ne!(entry1.id, entry3.id);
    }

    #[test]
    fn test_backward_compatibility_convenience_constructors() {
        // These should all compile and work without stack traces
        let info = LogEntry::info(LogSource::App, "Info message");
        assert!(!info.has_stack_trace());
        assert_eq!(info.level, LogLevel::Info);

        let error = LogEntry::error(LogSource::App, "Error message");
        assert!(!error.has_stack_trace());
        assert_eq!(error.level, LogLevel::Error);

        let warn = LogEntry::warn(LogSource::App, "Warning message");
        assert!(!warn.has_stack_trace());
        assert_eq!(warn.level, LogLevel::Warning);
    }

    #[test]
    fn test_stack_trace_frame_count_multiple_frames() {
        let trace_str = r#"
#0      main (package:app/main.dart:15:3)
#1      runApp (package:flutter/src/widgets/binding.dart:100:5)
#2      _startIsolate (dart:isolate-patch/isolate_patch.dart:307:19)
"#;
        let trace = ParsedStackTrace::parse(trace_str);
        let entry = LogEntry::with_stack_trace(
            LogLevel::Error,
            LogSource::FlutterError,
            "Crash occurred",
            trace,
        );

        assert_eq!(entry.stack_trace_frame_count(), 3);
    }

    #[test]
    fn test_log_entry_with_empty_stack_trace() {
        let trace = ParsedStackTrace::parse(""); // No parseable frames
        let entry = LogEntry::with_stack_trace(
            LogLevel::Error,
            LogSource::App,
            "Error with empty trace",
            trace,
        );

        // has_stack_trace returns true because stack_trace is Some
        assert!(entry.has_stack_trace());
        // But frame count is 0
        assert_eq!(entry.stack_trace_frame_count(), 0);
    }

    #[test]
    fn test_log_entry_strips_ansi_codes() {
        // LogEntry::new should automatically strip ANSI escape codes
        let entry = LogEntry::new(
            LogLevel::Info,
            LogSource::Flutter,
            "\x1b[38;5;196m│ Null check operator used\x1b[0m",
        );
        assert_eq!(entry.message, "│ Null check operator used");
        assert!(!entry.message.contains("\x1b"));

        // Test via convenience constructors
        let error_entry = LogEntry::error(
            LogSource::App,
            "\x1b[1m\x1b[31mError: Something failed\x1b[0m",
        );
        assert_eq!(error_entry.message, "Error: Something failed");

        // Test that box-drawing and emojis are preserved
        let logger_entry = LogEntry::info(
            LogSource::Flutter,
            "\x1b[38;5;244m┌──────────\x1b[0m\n\x1b[38;5;244m│ 🐛 Debug\x1b[0m",
        );
        assert!(logger_entry.message.contains("┌──────────"));
        assert!(logger_entry.message.contains("🐛"));
        assert!(!logger_entry.message.contains("\x1b"));
    }

    // LogLevelFilter tests

    #[test]
    fn test_level_filter_cycle() {
        let mut f = LogLevelFilter::All;
        f = f.cycle();
        assert_eq!(f, LogLevelFilter::Errors);
        f = f.cycle();
        assert_eq!(f, LogLevelFilter::Warnings);
        f = f.cycle();
        assert_eq!(f, LogLevelFilter::Info);
        f = f.cycle();
        assert_eq!(f, LogLevelFilter::Debug);
        f = f.cycle();
        assert_eq!(f, LogLevelFilter::All); // wrap around
    }

    #[test]
    fn test_level_filter_all_matches_everything() {
        let filter = LogLevelFilter::All;
        assert!(filter.matches(&LogLevel::Debug));
        assert!(filter.matches(&LogLevel::Info));
        assert!(filter.matches(&LogLevel::Warning));
        assert!(filter.matches(&LogLevel::Error));
    }

    #[test]
    fn test_level_filter_errors_only() {
        let filter = LogLevelFilter::Errors;
        assert!(filter.matches(&LogLevel::Error));
        assert!(!filter.matches(&LogLevel::Warning));
        assert!(!filter.matches(&LogLevel::Info));
        assert!(!filter.matches(&LogLevel::Debug));
    }

    #[test]
    fn test_level_filter_warnings_includes_errors() {
        let filter = LogLevelFilter::Warnings;
        assert!(filter.matches(&LogLevel::Error));
        assert!(filter.matches(&LogLevel::Warning));
        assert!(!filter.matches(&LogLevel::Info));
        assert!(!filter.matches(&LogLevel::Debug));
    }

    #[test]
    fn test_level_filter_info_includes_warnings_and_errors() {
        let filter = LogLevelFilter::Info;
        assert!(filter.matches(&LogLevel::Error));
        assert!(filter.matches(&LogLevel::Warning));
        assert!(filter.matches(&LogLevel::Info));
        assert!(!filter.matches(&LogLevel::Debug));
    }

    #[test]
    fn test_level_filter_debug_matches_everything() {
        let filter = LogLevelFilter::Debug;
        assert!(filter.matches(&LogLevel::Debug));
        assert!(filter.matches(&LogLevel::Info));
        assert!(filter.matches(&LogLevel::Warning));
        assert!(filter.matches(&LogLevel::Error));
    }

    #[test]
    fn test_level_filter_display_names() {
        assert_eq!(LogLevelFilter::All.display_name(), "All levels");
        assert_eq!(LogLevelFilter::Errors.display_name(), "Errors only");
        assert_eq!(LogLevelFilter::Warnings.display_name(), "Warnings+");
        assert_eq!(LogLevelFilter::Info.display_name(), "Info+");
        assert_eq!(LogLevelFilter::Debug.display_name(), "Debug+");
    }

    #[test]
    fn test_level_filter_default() {
        let filter = LogLevelFilter::default();
        assert_eq!(filter, LogLevelFilter::All);
    }

    // LogLevel severity tests (Phase 2 Task 11)

    #[test]
    fn test_log_level_severity() {
        assert_eq!(LogLevel::Debug.severity(), 0);
        assert_eq!(LogLevel::Info.severity(), 1);
        assert_eq!(LogLevel::Warning.severity(), 2);
        assert_eq!(LogLevel::Error.severity(), 3);
    }

    #[test]
    fn test_log_level_is_more_severe_than() {
        assert!(LogLevel::Error.is_more_severe_than(&LogLevel::Warning));
        assert!(LogLevel::Error.is_more_severe_than(&LogLevel::Info));
        assert!(LogLevel::Error.is_more_severe_than(&LogLevel::Debug));

        assert!(LogLevel::Warning.is_more_severe_than(&LogLevel::Info));
        assert!(LogLevel::Warning.is_more_severe_than(&LogLevel::Debug));
        assert!(!LogLevel::Warning.is_more_severe_than(&LogLevel::Error));

        assert!(LogLevel::Info.is_more_severe_than(&LogLevel::Debug));
        assert!(!LogLevel::Info.is_more_severe_than(&LogLevel::Warning));
        assert!(!LogLevel::Info.is_more_severe_than(&LogLevel::Error));

        assert!(!LogLevel::Debug.is_more_severe_than(&LogLevel::Debug));
        assert!(!LogLevel::Debug.is_more_severe_than(&LogLevel::Info));
    }

    #[test]
    fn test_log_level_max_severity() {
        assert_eq!(
            LogLevel::Error.max_severity(LogLevel::Warning),
            LogLevel::Error
        );
        assert_eq!(
            LogLevel::Warning.max_severity(LogLevel::Error),
            LogLevel::Error
        );
        assert_eq!(LogLevel::Info.max_severity(LogLevel::Debug), LogLevel::Info);
        assert_eq!(LogLevel::Debug.max_severity(LogLevel::Info), LogLevel::Info);
        assert_eq!(
            LogLevel::Error.max_severity(LogLevel::Error),
            LogLevel::Error
        );
        assert_eq!(LogLevel::Info.max_severity(LogLevel::Info), LogLevel::Info);
    }

    // LogSourceFilter tests

    #[test]
    fn test_source_filter_cycle() {
        let mut f = LogSourceFilter::All;
        f = f.cycle();
        assert_eq!(f, LogSourceFilter::App);
        f = f.cycle();
        assert_eq!(f, LogSourceFilter::Daemon);
        f = f.cycle();
        assert_eq!(f, LogSourceFilter::Flutter);
        f = f.cycle();
        assert_eq!(f, LogSourceFilter::Watcher);
        f = f.cycle();
        assert_eq!(f, LogSourceFilter::All); // wrap around
    }

    #[test]
    fn test_source_filter_all_matches_everything() {
        let filter = LogSourceFilter::All;
        assert!(filter.matches(&LogSource::App));
        assert!(filter.matches(&LogSource::Daemon));
        assert!(filter.matches(&LogSource::Flutter));
        assert!(filter.matches(&LogSource::FlutterError));
        assert!(filter.matches(&LogSource::Watcher));
    }

    #[test]
    fn test_source_filter_app() {
        let filter = LogSourceFilter::App;
        assert!(filter.matches(&LogSource::App));
        assert!(!filter.matches(&LogSource::Daemon));
        assert!(!filter.matches(&LogSource::Flutter));
        assert!(!filter.matches(&LogSource::FlutterError));
        assert!(!filter.matches(&LogSource::Watcher));
    }

    #[test]
    fn test_source_filter_daemon() {
        let filter = LogSourceFilter::Daemon;
        assert!(!filter.matches(&LogSource::App));
        assert!(filter.matches(&LogSource::Daemon));
        assert!(!filter.matches(&LogSource::Flutter));
        assert!(!filter.matches(&LogSource::FlutterError));
        assert!(!filter.matches(&LogSource::Watcher));
    }

    #[test]
    fn test_source_filter_flutter_includes_flutter_error() {
        let filter = LogSourceFilter::Flutter;
        assert!(!filter.matches(&LogSource::App));
        assert!(!filter.matches(&LogSource::Daemon));
        assert!(filter.matches(&LogSource::Flutter));
        assert!(filter.matches(&LogSource::FlutterError));
        assert!(!filter.matches(&LogSource::Watcher));
    }

    #[test]
    fn test_source_filter_watcher() {
        let filter = LogSourceFilter::Watcher;
        assert!(!filter.matches(&LogSource::App));
        assert!(!filter.matches(&LogSource::Daemon));
        assert!(!filter.matches(&LogSource::Flutter));
        assert!(!filter.matches(&LogSource::FlutterError));
        assert!(filter.matches(&LogSource::Watcher));
    }

    #[test]
    fn test_source_filter_display_names() {
        assert_eq!(LogSourceFilter::All.display_name(), "All sources");
        assert_eq!(LogSourceFilter::App.display_name(), "App logs");
        assert_eq!(LogSourceFilter::Daemon.display_name(), "Daemon logs");
        assert_eq!(LogSourceFilter::Flutter.display_name(), "Flutter logs");
        assert_eq!(LogSourceFilter::Watcher.display_name(), "Watcher logs");
    }

    #[test]
    fn test_source_filter_default() {
        let filter = LogSourceFilter::default();
        assert_eq!(filter, LogSourceFilter::All);
    }

    // FilterState tests

    #[test]
    fn test_filter_state_default() {
        let state = FilterState::default();
        assert_eq!(state.level_filter, LogLevelFilter::All);
        assert_eq!(state.source_filter, LogSourceFilter::All);
    }

    #[test]
    fn test_filter_state_is_active() {
        let default = FilterState::default();
        assert!(!default.is_active());

        let with_level = FilterState {
            level_filter: LogLevelFilter::Errors,
            ..Default::default()
        };
        assert!(with_level.is_active());

        let with_source = FilterState {
            source_filter: LogSourceFilter::Flutter,
            ..Default::default()
        };
        assert!(with_source.is_active());

        let with_both = FilterState {
            level_filter: LogLevelFilter::Errors,
            source_filter: LogSourceFilter::Flutter,
        };
        assert!(with_both.is_active());
    }

    #[test]
    fn test_filter_state_reset() {
        let mut state = FilterState {
            level_filter: LogLevelFilter::Errors,
            source_filter: LogSourceFilter::Flutter,
        };
        assert!(state.is_active());

        state.reset();
        assert!(!state.is_active());
        assert_eq!(state.level_filter, LogLevelFilter::All);
        assert_eq!(state.source_filter, LogSourceFilter::All);
    }

    #[test]
    fn test_filter_state_matches_both_filters() {
        let state = FilterState {
            level_filter: LogLevelFilter::Errors,
            source_filter: LogSourceFilter::Flutter,
        };

        // Error from Flutter - should pass both filters
        let entry = LogEntry::error(LogSource::Flutter, "test");
        assert!(state.matches(&entry));

        // Error from FlutterError - should also pass (Flutter filter includes FlutterError)
        let entry_flutter_err = LogEntry::error(LogSource::FlutterError, "test");
        assert!(state.matches(&entry_flutter_err));

        // Info from Flutter - wrong level
        let entry_wrong_level = LogEntry::info(LogSource::Flutter, "test");
        assert!(!state.matches(&entry_wrong_level));

        // Error from App - wrong source
        let entry_wrong_source = LogEntry::error(LogSource::App, "test");
        assert!(!state.matches(&entry_wrong_source));

        // Warning from Daemon - wrong both
        let entry_wrong_both = LogEntry::warn(LogSource::Daemon, "test");
        assert!(!state.matches(&entry_wrong_both));
    }

    #[test]
    fn test_filter_state_matches_with_default() {
        let state = FilterState::default();

        // Default filter should match everything
        let entries = vec![
            LogEntry::error(LogSource::Flutter, "test"),
            LogEntry::info(LogSource::App, "test"),
            LogEntry::warn(LogSource::Daemon, "test"),
            LogEntry::new(LogLevel::Debug, LogSource::Watcher, "test"),
        ];

        for entry in &entries {
            assert!(
                state.matches(entry),
                "Default filter should match all entries"
            );
        }
    }

    // SearchMatch tests

    #[test]
    fn test_search_match_new() {
        let m = SearchMatch::new(5, 10, 15);
        assert_eq!(m.entry_index, 5);
        assert_eq!(m.start, 10);
        assert_eq!(m.end, 15);
    }

    #[test]
    fn test_search_match_len() {
        let m = SearchMatch::new(0, 5, 10);
        assert_eq!(m.len(), 5);

        let m2 = SearchMatch::new(0, 0, 0);
        assert_eq!(m2.len(), 0);
        assert!(m2.is_empty());
    }

    // SearchState tests

    #[test]
    fn test_search_state_default() {
        let state = SearchState::default();
        assert!(state.query.is_empty());
        assert!(!state.is_active);
        assert!(!state.has_matches());
        assert!(state.pattern.is_none());
        assert!(!state.is_valid);
        assert!(state.error.is_none());
    }

    #[test]
    fn test_search_state_new() {
        let state = SearchState::new();
        assert!(state.query.is_empty());
        assert!(!state.is_active);
    }

    #[test]
    fn test_search_state_activate_deactivate() {
        let mut state = SearchState::default();
        assert!(!state.is_active);

        state.activate();
        assert!(state.is_active);

        state.deactivate();
        assert!(!state.is_active);
    }

    #[test]
    fn test_search_state_set_valid_query() {
        let mut state = SearchState::default();
        state.set_query("error");
        assert_eq!(state.query, "error");
        assert!(state.is_valid);
        assert_eq!(state.pattern, Some("error".to_string()));
        assert!(state.error.is_none());
    }

    #[test]
    fn test_search_state_set_valid_regex() {
        let mut state = SearchState::default();
        state.set_query("error|warn");
        assert!(state.is_valid);
        assert_eq!(state.pattern, Some("error|warn".to_string()));
    }

    #[test]
    fn test_search_state_set_invalid_regex() {
        let mut state = SearchState::default();
        state.set_query("[invalid");
        assert_eq!(state.query, "[invalid");
        assert!(!state.is_valid);
        assert!(state.pattern.is_none());
        assert!(state.error.is_some());
    }

    #[test]
    fn test_search_state_set_empty_query() {
        let mut state = SearchState::default();
        state.set_query("test");
        state.update_matches(vec![SearchMatch::new(0, 0, 4)]);
        assert!(state.has_matches());

        state.set_query("");
        assert!(state.query.is_empty());
        assert!(!state.is_valid);
        assert!(state.pattern.is_none());
        assert!(state.error.is_none());
        assert!(!state.has_matches());
    }

    #[test]
    fn test_search_state_clear() {
        let mut state = SearchState::default();
        state.set_query("test");
        state.activate();
        state.update_matches(vec![SearchMatch::new(0, 0, 4)]);
        state.current_match = Some(0);

        state.clear();

        assert!(state.query.is_empty());
        assert!(!state.is_active);
        assert!(state.matches.is_empty());
        assert!(state.current_match.is_none());
        assert!(state.pattern.is_none());
        assert!(!state.is_valid);
        assert!(state.error.is_none());
    }

    #[test]
    fn test_search_state_has_matches() {
        let mut state = SearchState::default();
        assert!(!state.has_matches());
        assert_eq!(state.match_count(), 0);

        state.update_matches(vec![SearchMatch::new(0, 0, 4)]);
        assert!(state.has_matches());
        assert_eq!(state.match_count(), 1);
    }

    #[test]
    fn test_search_navigation_next() {
        let mut state = SearchState::default();
        state.set_query("test");
        state.update_matches(vec![
            SearchMatch::new(0, 0, 4),
            SearchMatch::new(2, 5, 9),
            SearchMatch::new(5, 0, 4),
        ]);
        state.current_match = Some(0);

        state.next_match();
        assert_eq!(state.current_match, Some(1));

        state.next_match();
        assert_eq!(state.current_match, Some(2));

        state.next_match(); // wrap around
        assert_eq!(state.current_match, Some(0));
    }

    #[test]
    fn test_search_navigation_next_from_none() {
        let mut state = SearchState::default();
        state.update_matches(vec![SearchMatch::new(0, 0, 4)]);
        assert!(state.current_match.is_none());

        state.next_match();
        assert_eq!(state.current_match, Some(0));
    }

    #[test]
    fn test_search_navigation_next_empty() {
        let mut state = SearchState::default();
        state.next_match();
        assert!(state.current_match.is_none());
    }

    #[test]
    fn test_search_navigation_prev() {
        let mut state = SearchState::default();
        state.set_query("test");
        state.update_matches(vec![
            SearchMatch::new(0, 0, 4),
            SearchMatch::new(2, 5, 9),
            SearchMatch::new(5, 0, 4),
        ]);
        state.current_match = Some(0);

        state.prev_match(); // wrap around
        assert_eq!(state.current_match, Some(2));

        state.prev_match();
        assert_eq!(state.current_match, Some(1));

        state.prev_match();
        assert_eq!(state.current_match, Some(0));
    }

    #[test]
    fn test_search_navigation_prev_from_none() {
        let mut state = SearchState::default();
        state.update_matches(vec![SearchMatch::new(0, 0, 4), SearchMatch::new(1, 0, 4)]);
        assert!(state.current_match.is_none());

        state.prev_match();
        assert_eq!(state.current_match, Some(1)); // goes to last
    }

    #[test]
    fn test_search_jump_to_match() {
        let mut state = SearchState::default();
        state.update_matches(vec![
            SearchMatch::new(0, 0, 4),
            SearchMatch::new(5, 0, 4),
            SearchMatch::new(10, 0, 4),
        ]);

        state.jump_to_match(3);
        assert_eq!(state.current_match, Some(1)); // entry 5 is first >= 3

        state.jump_to_match(10);
        assert_eq!(state.current_match, Some(2)); // exact match

        state.jump_to_match(20); // beyond all matches
        assert_eq!(state.current_match, Some(0)); // wrap to first
    }

    #[test]
    fn test_search_current_match_index() {
        let mut state = SearchState::default();
        assert!(state.current_match_index().is_none());

        state.update_matches(vec![SearchMatch::new(0, 0, 4)]);
        state.current_match = Some(0);
        assert_eq!(state.current_match_index(), Some(1)); // 1-based
    }

    #[test]
    fn test_search_current_match_getter() {
        let mut state = SearchState::default();
        assert!(state.current_match().is_none());

        let m = SearchMatch::new(5, 10, 15);
        state.update_matches(vec![m.clone()]);
        state.current_match = Some(0);

        let current = state.current_match().unwrap();
        assert_eq!(current.entry_index, 5);
        assert_eq!(current.start, 10);
        assert_eq!(current.end, 15);
    }

    #[test]
    fn test_search_update_matches_resets_out_of_bounds() {
        let mut state = SearchState::default();
        state.update_matches(vec![
            SearchMatch::new(0, 0, 4),
            SearchMatch::new(1, 0, 4),
            SearchMatch::new(2, 0, 4),
        ]);
        state.current_match = Some(2);

        // Reduce matches - current should reset
        state.update_matches(vec![SearchMatch::new(0, 0, 4)]);
        assert_eq!(state.current_match, Some(0));

        // Clear all matches
        state.current_match = Some(0);
        state.update_matches(vec![]);
        assert!(state.current_match.is_none());
    }

    #[test]
    fn test_display_status_with_matches() {
        let mut state = SearchState::default();
        state.set_query("test");
        state.update_matches(vec![SearchMatch::new(0, 0, 4), SearchMatch::new(2, 5, 9)]);
        state.current_match = Some(0);

        assert_eq!(state.display_status(), "[1/2 matches]");

        state.next_match();
        assert_eq!(state.display_status(), "[2/2 matches]");
    }

    #[test]
    fn test_display_status_matches_no_current() {
        let mut state = SearchState::default();
        state.set_query("test");
        state.update_matches(vec![SearchMatch::new(0, 0, 4), SearchMatch::new(2, 5, 9)]);
        // current_match is None

        assert_eq!(state.display_status(), "[2 matches]");
    }

    #[test]
    fn test_display_status_no_matches() {
        let mut state = SearchState::default();
        state.set_query("nonexistent");
        state.update_matches(vec![]);

        assert_eq!(state.display_status(), "[No matches]");
    }

    #[test]
    fn test_display_status_empty_query() {
        let state = SearchState::default();
        assert_eq!(state.display_status(), "");
    }

    // ─────────────────────────────────────────────────────────
    // execute_search tests (Task 6)
    // ─────────────────────────────────────────────────────────

    #[test]
    fn test_execute_search_finds_matches() {
        let logs = VecDeque::from(vec![
            LogEntry::info(LogSource::App, "Hello world"),
            LogEntry::error(LogSource::App, "Error occurred"),
            LogEntry::info(LogSource::App, "Another hello"),
        ]);

        let mut state = SearchState::default();
        state.set_query("hello");
        state.execute_search(&logs);

        assert_eq!(state.matches.len(), 2);
        assert_eq!(state.matches[0].entry_index, 0);
        assert_eq!(state.matches[1].entry_index, 2);
    }

    #[test]
    fn test_execute_search_case_insensitive() {
        let logs = VecDeque::from(vec![
            LogEntry::info(LogSource::App, "ERROR in caps"),
            LogEntry::error(LogSource::App, "error lowercase"),
        ]);

        let mut state = SearchState::default();
        state.set_query("error");
        state.execute_search(&logs);

        assert_eq!(state.matches.len(), 2);
    }

    #[test]
    fn test_execute_search_regex() {
        let logs = VecDeque::from(vec![
            LogEntry::info(LogSource::App, "Took 150ms"),
            LogEntry::info(LogSource::App, "Took 2500ms"),
            LogEntry::info(LogSource::App, "No timing here"),
        ]);

        let mut state = SearchState::default();
        state.set_query(r"\d+ms");
        state.execute_search(&logs);

        assert_eq!(state.matches.len(), 2);
    }

    #[test]
    fn test_execute_search_invalid_regex() {
        let logs = VecDeque::from(vec![LogEntry::info(LogSource::App, "test")]);

        let mut state = SearchState::default();
        state.set_query("[invalid");
        state.execute_search(&logs);

        assert!(!state.is_valid);
        assert!(state.error.is_some());
        assert!(state.matches.is_empty());
    }

    #[test]
    fn test_execute_search_empty_query_clears_matches() {
        let logs = VecDeque::from(vec![LogEntry::info(LogSource::App, "test")]);

        let mut state = SearchState::default();
        state.set_query("test");
        state.execute_search(&logs);
        assert_eq!(state.matches.len(), 1);

        // Manually set query to empty and call execute_search
        // Note: set_query("") already clears matches, so we directly modify query
        state.query = String::new();
        let changed = state.execute_search(&logs);

        assert!(changed);
        assert!(state.matches.is_empty());
        assert!(state.current_match.is_none());
    }

    #[test]
    fn test_execute_search_sets_current_match() {
        let logs = VecDeque::from(vec![
            LogEntry::info(LogSource::App, "first test"),
            LogEntry::info(LogSource::App, "second test"),
        ]);

        let mut state = SearchState::default();
        state.set_query("test");
        state.execute_search(&logs);

        // Should auto-select first match
        assert_eq!(state.current_match, Some(0));
    }

    #[test]
    fn test_execute_search_preserves_current_match() {
        let logs = VecDeque::from(vec![
            LogEntry::info(LogSource::App, "test one"),
            LogEntry::info(LogSource::App, "test two"),
            LogEntry::info(LogSource::App, "test three"),
        ]);

        let mut state = SearchState::default();
        state.set_query("test");
        state.execute_search(&logs);
        state.current_match = Some(1); // Move to second match

        // Re-execute same search
        state.execute_search(&logs);

        // Current match should be preserved
        assert_eq!(state.current_match, Some(1));
    }

    #[test]
    fn test_matches_for_entry() {
        let logs = VecDeque::from(vec![
            LogEntry::info(LogSource::App, "test one test"),
            LogEntry::info(LogSource::App, "no match"),
            LogEntry::info(LogSource::App, "test two"),
        ]);

        let mut state = SearchState::default();
        state.set_query("test");
        state.execute_search(&logs);

        let matches_0 = state.matches_for_entry(0);
        assert_eq!(matches_0.len(), 2); // "test" appears twice

        let matches_1 = state.matches_for_entry(1);
        assert!(matches_1.is_empty());

        let matches_2 = state.matches_for_entry(2);
        assert_eq!(matches_2.len(), 1);
    }

    #[test]
    fn test_current_match_entry_index() {
        let logs = VecDeque::from(vec![
            LogEntry::info(LogSource::App, "first"),
            LogEntry::info(LogSource::App, "test"),
            LogEntry::info(LogSource::App, "last"),
        ]);

        let mut state = SearchState::default();
        state.set_query("test");
        state.execute_search(&logs);

        assert_eq!(state.current_match_entry_index(), Some(1));

        state.next_match(); // Wrap to 0 since only 1 match
        assert_eq!(state.current_match_entry_index(), Some(1));
    }

    #[test]
    fn test_current_match_entry_index_no_matches() {
        let state = SearchState::default();
        assert!(state.current_match_entry_index().is_none());
    }

    #[test]
    fn test_is_current_match() {
        let logs = VecDeque::from(vec![
            LogEntry::info(LogSource::App, "test one"),
            LogEntry::info(LogSource::App, "test two"),
        ]);

        let mut state = SearchState::default();
        state.set_query("test");
        state.execute_search(&logs);

        // First match is current
        assert!(state.is_current_match(&state.matches[0].clone()));
        assert!(!state.is_current_match(&state.matches[1].clone()));

        // Move to second match
        state.next_match();
        assert!(!state.is_current_match(&state.matches[0].clone()));
        assert!(state.is_current_match(&state.matches[1].clone()));
    }

    #[test]
    fn test_is_current_match_no_current() {
        let logs = VecDeque::from(vec![LogEntry::info(LogSource::App, "test")]);

        let mut state = SearchState::default();
        state.set_query("test");
        state.execute_search(&logs);
        state.current_match = None;

        let match_ref = &state.matches[0];
        assert!(!state.is_current_match(match_ref));
    }

    #[test]
    fn test_execute_search_multiple_matches_per_entry() {
        let logs = VecDeque::from(vec![LogEntry::info(
            LogSource::App,
            "test abc test def test",
        )]);

        let mut state = SearchState::default();
        state.set_query("test");
        state.execute_search(&logs);

        assert_eq!(state.matches.len(), 3);
        assert_eq!(state.matches[0].start, 0);
        assert_eq!(state.matches[0].end, 4);
        assert_eq!(state.matches[1].start, 9);
        assert_eq!(state.matches[1].end, 13);
        assert_eq!(state.matches[2].start, 18);
        assert_eq!(state.matches[2].end, 22);
    }

    // BootableDevice tests

    #[test]
    fn test_bootable_device_can_boot() {
        let device = BootableDevice::new("id", "iPhone 15", Platform::IOS, "iOS 17.2");
        assert!(device.can_boot());

        let mut booted = device.clone();
        booted.state = DeviceState::Booted;
        assert!(!booted.can_boot());
    }

    #[test]
    fn test_display_string() {
        let device = BootableDevice::new("id", "Pixel 8", Platform::Android, "API 34");
        assert_eq!(device.display_string(), "Pixel 8 (API 34)");
    }
}
