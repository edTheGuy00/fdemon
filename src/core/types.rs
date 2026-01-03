//! Core domain type definitions

use chrono::{DateTime, Local};

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
    /// Application is shutting down
    Quitting,
}

/// Represents a log entry with timestamp
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: DateTime<Local>,
    pub level: LogLevel,
    pub source: LogSource,
    pub message: String,
}

impl LogEntry {
    /// Create a new log entry with current timestamp
    pub fn new(level: LogLevel, source: LogSource, message: impl Into<String>) -> Self {
        Self {
            timestamp: Local::now(),
            level,
            source,
            message: message.into(),
        }
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
}

/// Source of log messages
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogSource {
    /// Application/system messages
    App,
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
            LogSource::Flutter => "flutter",
            LogSource::FlutterError => "flutter",
            LogSource::Watcher => "watch",
        }
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
}
