//! Helper utilities for the handler module

use crate::core::LogLevel;

/// Detect log level from raw (non-JSON) output line
pub fn detect_raw_line_level(line: &str) -> (LogLevel, String) {
    let trimmed = line.trim();

    // Android logcat format: E/, W/, I/, D/
    if trimmed.starts_with("E/") {
        return (LogLevel::Error, trimmed.to_string());
    }
    if trimmed.starts_with("W/") {
        return (LogLevel::Warning, trimmed.to_string());
    }

    // Gradle/build errors
    if trimmed.contains("FAILURE:")
        || trimmed.contains("BUILD FAILED")
        || trimmed.contains("error:")
    {
        return (LogLevel::Error, trimmed.to_string());
    }

    // Xcode errors
    if trimmed.contains("❌") {
        return (LogLevel::Error, trimmed.to_string());
    }

    // Warnings
    if trimmed.contains("warning:") || trimmed.contains("⚠") {
        return (LogLevel::Warning, trimmed.to_string());
    }

    // Build progress (often noise, show as debug)
    if trimmed.starts_with("Running ")
        || trimmed.starts_with("Building ")
        || trimmed.starts_with("Compiling ")
        || trimmed.contains("...")
    {
        return (LogLevel::Debug, trimmed.to_string());
    }

    (LogLevel::Info, trimmed.to_string())
}
