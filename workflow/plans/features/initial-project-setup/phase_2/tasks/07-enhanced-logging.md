## Task: Enhanced Logging

**Objective**: Parse `app.log` daemon events to extract Flutter print() output, color-code errors and warnings, and handle `daemon.logMessage` events for improved log display.

**Depends on**: 01-typed-protocol

---

### Scope

- `src/app/handler.rs`: MODIFY - Enhanced daemon event parsing using typed messages
- `src/core/types.rs`: MODIFY - Add log level detection utilities
- `src/tui/widgets/log_view.rs`: MODIFY - Enhance color coding for different message types
- `src/daemon/protocol.rs`: MODIFY - Add helper methods for log extraction

---

### Implementation Details

#### Problem Statement

Currently, daemon messages are displayed with minimal parsing:
- Raw JSON summaries are shown instead of clean log output
- No distinction between `flutter:` prefixed messages and system messages
- Error messages from the app aren't highlighted distinctly
- Progress messages clutter the log view

This task improves log parsing and display for better developer experience.

#### Enhanced Log Parsing

```rust
// src/daemon/protocol.rs - add helper methods to DaemonMessage

impl DaemonMessage {
    /// Extract a clean log message for display
    pub fn to_log_entry(&self) -> Option<LogEntryInfo> {
        match self {
            DaemonMessage::AppLog(log) => {
                let (level, message) = Self::parse_flutter_log(&log.log, log.error);
                Some(LogEntryInfo {
                    level,
                    source: LogSource::Flutter,
                    message,
                    stack_trace: log.stack_trace.clone(),
                })
            }
            DaemonMessage::DaemonLogMessage(msg) => {
                let level = match msg.level.as_str() {
                    "error" => LogLevel::Error,
                    "warning" => LogLevel::Warning,
                    "status" => LogLevel::Info,
                    _ => LogLevel::Debug,
                };
                Some(LogEntryInfo {
                    level,
                    source: LogSource::App,
                    message: msg.message.clone(),
                    stack_trace: msg.stack_trace.clone(),
                })
            }
            DaemonMessage::AppProgress(progress) => {
                // Only show progress messages that are meaningful
                if progress.finished {
                    progress.message.as_ref().map(|msg| LogEntryInfo {
                        level: LogLevel::Info,
                        source: LogSource::Flutter,
                        message: msg.clone(),
                        stack_trace: None,
                    })
                } else {
                    // Skip in-progress messages to reduce noise
                    None
                }
            }
            DaemonMessage::AppStart(start) => Some(LogEntryInfo {
                level: LogLevel::Info,
                source: LogSource::App,
                message: format!("App starting on {}", start.device_id),
                stack_trace: None,
            }),
            DaemonMessage::AppStarted(_) => Some(LogEntryInfo {
                level: LogLevel::Info,
                source: LogSource::App,
                message: "App started".to_string(),
                stack_trace: None,
            }),
            DaemonMessage::AppStop(stop) => {
                let message = if let Some(err) = &stop.error {
                    format!("App stopped with error: {}", err)
                } else {
                    "App stopped".to_string()
                };
                Some(LogEntryInfo {
                    level: if stop.error.is_some() { LogLevel::Error } else { LogLevel::Warning },
                    source: LogSource::App,
                    message,
                    stack_trace: None,
                })
            }
            DaemonMessage::AppDebugPort(debug) => Some(LogEntryInfo {
                level: LogLevel::Info,
                source: LogSource::App,
                message: format!("DevTools available at port {}", debug.port),
                stack_trace: None,
            }),
            DaemonMessage::DeviceAdded(device) => Some(LogEntryInfo {
                level: LogLevel::Debug,
                source: LogSource::App,
                message: format!("Device connected: {} ({})", device.name, device.platform),
                stack_trace: None,
            }),
            DaemonMessage::DeviceRemoved(device) => Some(LogEntryInfo {
                level: LogLevel::Debug,
                source: LogSource::App,
                message: format!("Device disconnected: {}", device.name),
                stack_trace: None,
            }),
            DaemonMessage::DaemonConnected(conn) => Some(LogEntryInfo {
                level: LogLevel::Debug,
                source: LogSource::App,
                message: format!("Daemon connected (v{}, pid {})", conn.version, conn.pid),
                stack_trace: None,
            }),
            _ => None, // UnknownEvent, Response handled separately
        }
    }

    /// Parse a flutter log message to extract level and clean message
    fn parse_flutter_log(raw: &str, is_error: bool) -> (LogLevel, String) {
        let message = raw.trim();

        // Check for error indicators
        if is_error {
            return (LogLevel::Error, message.to_string());
        }

        // Check for common patterns
        if message.starts_with("flutter: ") {
            let content = &message[9..]; // Strip "flutter: " prefix
            let level = Self::detect_log_level(content);
            return (level, content.to_string());
        }

        // Check for error patterns in content
        if message.contains("Exception:") 
            || message.contains("Error:") 
            || message.starts_with("E/") 
        {
            return (LogLevel::Error, message.to_string());
        }

        // Check for warning patterns
        if message.contains("Warning:") || message.starts_with("W/") {
            return (LogLevel::Warning, message.to_string());
        }

        // Default to info
        (LogLevel::Info, message.to_string())
    }

    /// Detect log level from message content
    fn detect_log_level(message: &str) -> LogLevel {
        let lower = message.to_lowercase();

        // Error indicators
        if lower.contains("error") 
            || lower.contains("exception")
            || lower.contains("failed")
            || lower.contains("fatal")
        {
            return LogLevel::Error;
        }

        // Warning indicators
        if lower.contains("warning") 
            || lower.contains("warn")
            || lower.contains("deprecated")
        {
            return LogLevel::Warning;
        }

        // Debug indicators
        if lower.starts_with("debug:")
            || lower.starts_with("[debug]")
            || lower.contains("verbose")
        {
            return LogLevel::Debug;
        }

        LogLevel::Info
    }
}

/// Intermediate log entry info from parsed daemon message
#[derive(Debug, Clone)]
pub struct LogEntryInfo {
    pub level: LogLevel,
    pub source: LogSource,
    pub message: String,
    pub stack_trace: Option<String>,
}
```

#### Updated Handler

```rust
// src/app/handler.rs - update handle_daemon_event

use crate::daemon::{DaemonMessage, LogEntryInfo};

fn handle_daemon_event(state: &mut AppState, event: DaemonEvent) {
    match event {
        DaemonEvent::Stdout(line) => {
            // Try to strip brackets and parse
            if let Some(json) = protocol::strip_brackets(&line) {
                if let Some(msg) = DaemonMessage::parse(json) {
                    // Handle responses separately
                    if let DaemonMessage::Response { id, result, error } = &msg {
                        // Route to request tracker (handled in event loop)
                        tracing::debug!("Response for request {}", id);
                        return;
                    }

                    // Convert to log entry if applicable
                    if let Some(entry_info) = msg.to_log_entry() {
                        let mut entry = LogEntry::new(
                            entry_info.level,
                            entry_info.source,
                            entry_info.message,
                        );

                        // Add stack trace as separate entries if present
                        state.add_log(entry);

                        if let Some(trace) = entry_info.stack_trace {
                            for line in trace.lines().take(10) { // Limit stack trace
                                state.add_log(LogEntry::new(
                                    LogLevel::Debug,
                                    LogSource::FlutterError,
                                    format!("    {}", line),
                                ));
                            }
                        }
                    } else {
                        // Unknown event type, log at debug level
                        tracing::debug!("Unhandled daemon message: {}", msg.summary());
                    }
                } else {
                    // Unparseable JSON - show raw
                    tracing::debug!("Unparseable daemon JSON: {}", json);
                }
            } else if !line.trim().is_empty() {
                // Non-JSON output (build progress, etc.)
                // Detect if it's an error or warning
                let (level, message) = detect_raw_line_level(&line);
                state.add_log(LogEntry::new(level, LogSource::Flutter, message));
            }
        }

        DaemonEvent::Stderr(line) => {
            if !line.trim().is_empty() {
                state.add_log(LogEntry::new(
                    LogLevel::Error,
                    LogSource::FlutterError,
                    line,
                ));
            }
        }

        DaemonEvent::Exited { code } => {
            let (level, message) = match code {
                Some(0) => (LogLevel::Info, "Flutter process exited normally".to_string()),
                Some(c) => (LogLevel::Warning, format!("Flutter process exited with code {}", c)),
                None => (LogLevel::Warning, "Flutter process exited".to_string()),
            };
            state.add_log(LogEntry::new(level, LogSource::App, message));
            state.phase = AppPhase::Initializing;
        }

        DaemonEvent::SpawnFailed { reason } => {
            state.add_log(LogEntry::error(
                LogSource::App,
                format!("Failed to start Flutter: {}", reason),
            ));
        }

        DaemonEvent::Message(msg) => {
            // Typed message variant (if using)
            if let Some(entry_info) = msg.to_log_entry() {
                state.add_log(LogEntry::new(
                    entry_info.level,
                    entry_info.source,
                    entry_info.message,
                ));
            }
        }
    }
}

/// Detect log level from raw (non-JSON) output line
fn detect_raw_line_level(line: &str) -> (LogLevel, String) {
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
    if trimmed.contains("error:") || trimmed.contains("❌") {
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
```

#### Enhanced Log View Colors

```rust
// src/tui/widgets/log_view.rs - enhanced color scheme

impl<'a> LogView<'a> {
    /// Get style for log level - returns (level_style, message_style)
    fn level_style(level: LogLevel) -> (Style, Style) {
        match level {
            LogLevel::Error => (
                Style::default()
                    .fg(Color::Red)
                    .add_modifier(Modifier::BOLD),
                Style::default().fg(Color::LightRed),
            ),
            LogLevel::Warning => (
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
                Style::default().fg(Color::Yellow),
            ),
            LogLevel::Info => (
                Style::default().fg(Color::Green),
                Style::default().fg(Color::White), // Brighter for better readability
            ),
            LogLevel::Debug => (
                Style::default().fg(Color::DarkGray),
                Style::default().fg(Color::DarkGray),
            ),
        }
    }

    /// Get style for log source
    fn source_style(source: LogSource) -> Style {
        match source {
            LogSource::App => Style::default().fg(Color::Magenta),
            LogSource::Flutter => Style::default().fg(Color::Cyan),
            LogSource::FlutterError => Style::default().fg(Color::Red),
            LogSource::Watcher => Style::default().fg(Color::Blue),
        }
    }

    /// Format message with inline highlighting
    fn format_message(message: &str, base_style: Style) -> Vec<Span<'static>> {
        let mut spans = Vec::new();

        // Simple highlighting for common patterns
        // This could be extended with regex for more sophisticated highlighting

        if message.contains("Reloaded") || message.contains("reloaded") {
            // Highlight reload success
            spans.push(Span::styled(
                message.to_string(),
                base_style.fg(Color::Green),
            ));
        } else if message.contains("Exception") || message.contains("Error") {
            // Highlight exceptions
            spans.push(Span::styled(
                message.to_string(),
                base_style.fg(Color::LightRed),
            ));
        } else if message.starts_with("    ") {
            // Stack trace lines (indented)
            spans.push(Span::styled(
                message.to_string(),
                Style::default().fg(Color::DarkGray),
            ));
        } else {
            spans.push(Span::styled(message.to_string(), base_style));
        }

        spans
    }

    /// Format a single log entry as a styled Line with enhanced colors
    fn format_entry_enhanced(&self, entry: &LogEntry) -> Line<'static> {
        let (level_style, msg_style) = Self::level_style(entry.level);
        let source_style = Self::source_style(entry.source);

        let mut spans = Vec::with_capacity(8);

        // Timestamp: "HH:MM:SS "
        if self.show_timestamps {
            spans.push(Span::styled(
                entry.formatted_time(),
                Style::default().fg(Color::DarkGray),
            ));
            spans.push(Span::raw(" "));
        }

        // Level indicator with color
        let level_icon = match entry.level {
            LogLevel::Error => "✗",
            LogLevel::Warning => "⚠",
            LogLevel::Info => "•",
            LogLevel::Debug => "·",
        };
        spans.push(Span::styled(
            format!("{} ", level_icon),
            level_style,
        ));

        // Source: "[flutter] " or "[app] "
        if self.show_source {
            spans.push(Span::styled(
                format!("[{}] ", entry.source.prefix()),
                source_style,
            ));
        }

        // Message with inline highlighting
        spans.extend(Self::format_message(&entry.message, msg_style));

        Line::from(spans)
    }
}
```

---

### Acceptance Criteria

1. [ ] `app.log` events parsed to extract clean Flutter print() output
2. [ ] "flutter: " prefix stripped from messages
3. [ ] Error logs from daemon show in red with ✗ icon
4. [ ] Warning logs show in yellow with ⚠ icon
5. [ ] Info logs show in white/green with • icon
6. [ ] Debug logs show in gray with · icon
7. [ ] `daemon.logMessage` events properly categorized by level field
8. [ ] Stack traces displayed with indentation (limited to 10 lines)
9. [ ] Progress messages filtered (only show finished ones)
10. [ ] Raw non-JSON output (build progress) properly categorized
11. [ ] Reload success messages highlighted in green
12. [ ] `LogEntryInfo` struct provides clean interface for log conversion
13. [ ] Unit tests for log level detection
14. [ ] Unit tests for flutter log parsing

---

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_flutter_log_basic() {
        let (level, msg) = DaemonMessage::parse_flutter_log("flutter: Hello World", false);
        assert_eq!(level, LogLevel::Info);
        assert_eq!(msg, "Hello World");
    }

    #[test]
    fn test_parse_flutter_log_error_flag() {
        let (level, msg) = DaemonMessage::parse_flutter_log("Some error occurred", true);
        assert_eq!(level, LogLevel::Error);
        assert_eq!(msg, "Some error occurred");
    }

    #[test]
    fn test_parse_flutter_log_exception_in_message() {
        let (level, msg) = DaemonMessage::parse_flutter_log(
            "flutter: Exception: Something went wrong",
            false
        );
        assert_eq!(level, LogLevel::Error);
    }

    #[test]
    fn test_parse_flutter_log_warning() {
        let (level, _) = DaemonMessage::parse_flutter_log(
            "flutter: Warning: deprecated API used",
            false
        );
        assert_eq!(level, LogLevel::Warning);
    }

    #[test]
    fn test_detect_log_level_error_patterns() {
        assert_eq!(DaemonMessage::detect_log_level("Error occurred"), LogLevel::Error);
        assert_eq!(DaemonMessage::detect_log_level("An exception was thrown"), LogLevel::Error);
        assert_eq!(DaemonMessage::detect_log_level("Build failed"), LogLevel::Error);
        assert_eq!(DaemonMessage::detect_log_level("Fatal error"), LogLevel::Error);
    }

    #[test]
    fn test_detect_log_level_warning_patterns() {
        assert_eq!(DaemonMessage::detect_log_level("Warning: check this"), LogLevel::Warning);
        assert_eq!(DaemonMessage::detect_log_level("This is deprecated"), LogLevel::Warning);
    }

    #[test]
    fn test_detect_log_level_debug_patterns() {
        assert_eq!(DaemonMessage::detect_log_level("debug: value is 5"), LogLevel::Debug);
        assert_eq!(DaemonMessage::detect_log_level("[debug] trace info"), LogLevel::Debug);
    }

    #[test]
    fn test_detect_log_level_default() {
        assert_eq!(DaemonMessage::detect_log_level("Normal message"), LogLevel::Info);
    }

    #[test]
    fn test_detect_raw_line_level_android() {
        let (level, _) = detect_raw_line_level("E/flutter: Error in app");
        assert_eq!(level, LogLevel::Error);

        let (level, _) = detect_raw_line_level("W/flutter: Warning message");
        assert_eq!(level, LogLevel::Warning);
    }

    #[test]
    fn test_detect_raw_line_level_gradle() {
        let (level, _) = detect_raw_line_level("FAILURE: Build failed");
        assert_eq!(level, LogLevel::Error);

        let (level, _) = detect_raw_line_level("BUILD FAILED in 5s");
        assert_eq!(level, LogLevel::Error);
    }

    #[test]
    fn test_detect_raw_line_level_xcode() {
        let (level, _) = detect_raw_line_level("error: cannot find module");
        assert_eq!(level, LogLevel::Error);
    }

    #[test]
    fn test_detect_raw_line_level_build_progress() {
        let (level, _) = detect_raw_line_level("Running Gradle task 'assembleDebug'...");
        assert_eq!(level, LogLevel::Debug);

        let (level, _) = detect_raw_line_level("Building flutter assets...");
        assert_eq!(level, LogLevel::Debug);
    }

    #[test]
    fn test_app_log_to_log_entry() {
        let app_log = AppLog {
            app_id: "test".to_string(),
            log: "flutter: Hello from app".to_string(),
            error: false,
            stack_trace: None,
        };
        
        let msg = DaemonMessage::AppLog(app_log);
        let entry = msg.to_log_entry().unwrap();
        
        assert_eq!(entry.level, LogLevel::Info);
        assert_eq!(entry.message, "Hello from app");
        assert!(matches!(entry.source, LogSource::Flutter));
    }

    #[test]
    fn test_daemon_log_message_to_log_entry() {
        let daemon_msg = DaemonLogMessage {
            level: "error".to_string(),
            message: "Something went wrong".to_string(),
            stack_trace: None,
        };

        let msg = DaemonMessage::DaemonLogMessage(daemon_msg);
        let entry = msg.to_log_entry().unwrap();

        assert_eq!(entry.level, LogLevel::Error);
        assert_eq!(entry.message, "Something went wrong");
    }

    #[test]
    fn test_app_progress_finished_only() {
        let progress_ongoing = AppProgress {
            app_id: "test".to_string(),
            id: "1".to_string(),
            progress_id: None,
            message: Some("Compiling...".to_string()),
            finished: false,
        };

        let msg_ongoing = DaemonMessage::AppProgress(progress_ongoing);
        assert!(msg_ongoing.to_log_entry().is_none()); // Skip ongoing

        let progress_finished = AppProgress {
            app_id: "test".to_string(),
            id: "1".to_string(),
            progress_id: None,
            message: Some("Compilation complete".to_string()),
            finished: true,
        };

        let msg_finished = DaemonMessage::AppProgress(progress_finished);
        assert!(msg_finished.to_log_entry().is_some()); // Show finished
    }

    #[test]
    fn test_app_stop_error_level() {
        let stop_normal = AppStop {
            app_id: "test".to_string(),
            error: None,
        };
        let entry = DaemonMessage::AppStop(stop_normal).to_log_entry().unwrap();
        assert_eq!(entry.level, LogLevel::Warning);

        let stop_error = AppStop {
            app_id: "test".to_string(),
            error: Some("Crash!".to_string()),
        };
        let entry = DaemonMessage::AppStop(stop_error).to_log_entry().unwrap();
        assert_eq!(entry.level, LogLevel::Error);
    }
}
```

---

### Log Display Examples

**Before (raw):**
```
12:34:56 INF [flutter] Event: app.log
12:34:57 INF [flutter] Event: app.progress
12:34:58 INF [flutter] Response #1: ok
```

**After (enhanced):**
```
12:34:56 • [flutter] Hello from main()
12:34:57 • [flutter] Button tapped
12:34:58 ✗ [flutter] Exception: Null check failed
           at package:my_app/main.dart:42
           at package:flutter/gestures.dart:1234
12:35:01 • [app] Reloaded in 245ms
```

---

### Notes

- Stack trace limiting (10 lines) prevents log buffer from filling with long traces
- Progress message filtering reduces noise during builds
- The "flutter: " prefix is stripped because it's redundant information
- Android logcat format (E/, W/, I/, D/) is detected for raw output
- Build output (Gradle, Xcode) errors are properly categorized
- Future enhancement: regex-based syntax highlighting for Dart code in errors
- Consider adding log filtering UI in Phase 3 (by level, source, pattern)

---

### Files to Create/Modify

| File | Action | Description |
|------|--------|-------------|
| `src/daemon/protocol.rs` | MODIFY | Add `to_log_entry()`, `parse_flutter_log()`, `LogEntryInfo` |
| `src/app/handler.rs` | MODIFY | Use enhanced parsing in `handle_daemon_event` |
| `src/tui/widgets/log_view.rs` | MODIFY | Add icons and enhanced color scheme |
| `src/core/types.rs` | MODIFY | Ensure LogLevel/LogSource support new patterns |

---

## Implementation Summary

**Status**: ✅ Done

**Date Completed**: 2026-01-03

### Files Modified

| File | Changes |
|------|---------|
| `src/daemon/protocol.rs` | Added `LogEntryInfo` struct, `to_log_entry()`, `parse_flutter_log()`, `detect_log_level()` methods to `DaemonMessage` |
| `src/daemon/mod.rs` | Exported `LogEntryInfo` |
| `src/app/handler.rs` | Enhanced `handle_daemon_event` with typed message parsing, added `detect_raw_line_level()` function, renamed `handle_daemon_message` to `handle_daemon_message_state` |
| `src/tui/widgets/log_view.rs` | Added `level_icon()`, `format_message()` methods, updated `format_entry()` to use icons (✗, ⚠, •, ·) and enhanced colors |

### Notable Decisions & Tradeoffs

1. **Source assignment**: `DaemonLogMessage` events use `LogSource::Daemon` instead of `LogSource::App` for clearer distinction
2. **Progress filtering**: In-progress messages are skipped (`to_log_entry()` returns `None`) to reduce log noise
3. **Stack trace limit**: 10 lines max to prevent buffer overflow with long traces
4. **Icon-based display**: Replaced text prefixes (ERR, INF, etc.) with icons for visual clarity
5. **Inline highlighting**: Reload success messages highlighted green, exceptions red

### Testing Performed

```bash
cargo check    # PASS
cargo test     # PASS (218 tests)
cargo clippy   # PASS (no warnings)
cargo fmt      # Applied
```

### New Tests Added

- `test_parse_flutter_log_basic`
- `test_parse_flutter_log_error_flag`
- `test_parse_flutter_log_exception_in_message`
- `test_parse_flutter_log_warning`
- `test_detect_log_level_error_patterns`
- `test_detect_log_level_warning_patterns`
- `test_detect_log_level_debug_patterns`
- `test_detect_log_level_default`
- `test_app_log_to_log_entry`
- `test_daemon_log_message_to_log_entry`
- `test_app_progress_finished_only`
- `test_app_stop_error_level`
- `test_app_log_strips_flutter_prefix`
- `test_app_log_with_stack_trace`
- `test_detect_raw_line_level_android`
- `test_detect_raw_line_level_gradle`
- `test_detect_raw_line_level_xcode`
- `test_detect_raw_line_level_build_progress`
- `test_detect_raw_line_level_default`
- `test_detect_raw_line_level_trims_whitespace`

### Risks & Limitations

1. **Pattern matching**: Log level detection uses string matching which may produce false positives for messages containing "error" or "warning" as substrings
2. **Xcode emoji check**: Uses emoji (❌) for Xcode error detection which may not render correctly on all terminals
3. **Stack trace parsing**: Simple line-based parsing may not handle all Flutter stack trace formats