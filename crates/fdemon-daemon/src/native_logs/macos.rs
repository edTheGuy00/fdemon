//! macOS unified log capture backend.
//!
//! Implements [`NativeLogCapture`] by spawning `log stream` with process-name
//! filtering, parsing the syslog-style output, and emitting [`NativeLogEvent`]s.
//!
//! All code in this module is gated behind `#[cfg(target_os = "macos")]` via
//! the conditional module declaration in `mod.rs`.

use std::process::Stdio;
use std::sync::LazyLock;

use regex::Regex;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::{mpsc, watch};

use fdemon_core::NativeLogPriority;

use super::{MacOsLogConfig, NativeLogCapture, NativeLogEvent, NativeLogHandle};

/// Number of header lines emitted by `log stream --style syslog` before data.
///
/// Line 1: `Filtering the log data using "..."`
/// Line 2: Column header: `Timestamp  Thread  Type  Activity  PID  TTL`
const LOG_STREAM_HEADER_LINES: usize = 2;

/// Regex for macOS `log stream --style syslog` output.
///
/// Format: `timestamp  thread  type  activity  pid  ttl  process: (subsystem) [category] message`
///
/// The subsystem `(...)` and category `[...]` parts are both optional.
static SYSLOG_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(\d{4}-\d{2}-\d{2}\s+\d{2}:\d{2}:\d{2}\.\d+[-+]\d{4})\s+\S+\s+(\w+)\s+\S+\s+\d+\s+\d+\s+\S+:\s*(?:\(([^)]*)\))?\s*(?:\[([^\]]*)\])?\s*(.*)$"
    ).expect("syslog regex is valid")
});

/// A parsed line from `log stream --style syslog` output.
#[derive(Debug, Clone)]
pub struct SyslogLine {
    /// Raw timestamp string from the log line (e.g., `"2024-03-10 14:30:00.123456-0700"`).
    pub timestamp: String,
    /// macOS log level string (e.g., `"Default"`, `"Info"`, `"Debug"`, `"Error"`, `"Fault"`).
    pub level: String,
    /// Optional subsystem name (e.g., `"MyPlugin"`, `"Foundation"`).
    pub subsystem: Option<String>,
    /// Optional category string (e.g., `"com.example.plugin:default"`).
    pub category: Option<String>,
    /// The log message content.
    pub message: String,
}

/// Parse a single `log stream --style syslog` data line.
///
/// Returns `None` for header lines, blank lines, or lines that do not match
/// the expected syslog format.
pub fn parse_syslog_line(line: &str) -> Option<SyslogLine> {
    let caps = SYSLOG_RE.captures(line)?;
    Some(SyslogLine {
        timestamp: caps[1].to_string(),
        level: caps[2].to_string(),
        subsystem: caps.get(3).map(|m| m.as_str().to_string()),
        category: caps.get(4).map(|m| m.as_str().to_string()),
        message: caps[5].to_string(),
    })
}

/// Derive a tag string from a parsed syslog line.
///
/// Priority:
/// 1. Category base (e.g., `"com.example.plugin:default"` → `"com.example.plugin"`)
/// 2. Subsystem name (e.g., `"Foundation"`)
/// 3. `"native"` fallback when neither is present
pub fn derive_tag(line: &SyslogLine) -> String {
    // Prefer category, stripping the ":category" suffix if present
    if let Some(ref cat) = line.category {
        if let Some(base) = cat.split(':').next() {
            if !base.is_empty() {
                return base.to_string();
            }
        }
    }
    // Fall back to subsystem
    if let Some(ref sub) = line.subsystem {
        if !sub.is_empty() {
            return sub.to_string();
        }
    }
    // No tag information available
    "native".to_string()
}

/// Convert a parsed syslog line to a [`NativeLogEvent`].
pub fn syslog_line_to_event(line: &SyslogLine) -> NativeLogEvent {
    let priority =
        NativeLogPriority::from_macos_level(&line.level).unwrap_or(NativeLogPriority::Info);
    let tag = derive_tag(line);

    NativeLogEvent {
        tag,
        level: priority.to_log_level(),
        message: line.message.clone(),
        timestamp: Some(line.timestamp.clone()),
    }
}

/// Decide whether a tag should be included based on the capture configuration.
///
/// - If `include_tags` is non-empty, only those tags pass.
/// - Otherwise, any tag not in `exclude_tags` passes.
fn should_include_tag(config: &MacOsLogConfig, tag: &str) -> bool {
    if !config.include_tags.is_empty() {
        return config
            .include_tags
            .iter()
            .any(|t| t.eq_ignore_ascii_case(tag));
    }
    !config
        .exclude_tags
        .iter()
        .any(|t| t.eq_ignore_ascii_case(tag))
}

/// Build the `log stream` [`Command`] from the given configuration.
fn build_log_stream_command(config: &MacOsLogConfig) -> Command {
    let mut cmd = Command::new("log");
    cmd.arg("stream");

    // Filter by process name via --predicate
    let predicate = format!("process == \"{}\"", config.process_name);
    cmd.arg("--predicate").arg(&predicate);

    // Map min_level to the closest valid `log stream --level` argument.
    // macOS accepts: "default", "info", "debug". There is no "warning" level.
    let level = match config.min_level.to_lowercase().as_str() {
        "verbose" | "debug" => "debug",
        "info" => "info",
        "warning" | "error" => "error",
        _ => "info",
    };
    cmd.arg("--level").arg(level);

    // Use syslog style for structured, parseable output
    cmd.arg("--style").arg("syslog");

    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::null());
    cmd.kill_on_drop(true);

    cmd
}

/// Background task that runs the `log stream` process and forwards parsed events.
///
/// Skips the two header lines emitted before data, then parses each subsequent
/// line and sends matching [`NativeLogEvent`]s to `event_tx`. Exits when the
/// process ends, an I/O error occurs, the receiver is dropped, or `shutdown_rx`
/// signals `true`.
async fn run_log_stream_capture(
    config: MacOsLogConfig,
    event_tx: mpsc::Sender<NativeLogEvent>,
    mut shutdown_rx: watch::Receiver<bool>,
) {
    let mut cmd = build_log_stream_command(&config);
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Failed to spawn log stream: {}", e);
            return;
        }
    };

    let stdout = match child.stdout.take() {
        Some(s) => s,
        None => {
            tracing::warn!("log stream stdout not available");
            return;
        }
    };

    let mut reader = BufReader::new(stdout).lines();
    let mut header_lines_remaining = LOG_STREAM_HEADER_LINES;

    loop {
        tokio::select! {
            biased;

            _ = shutdown_rx.changed() => {
                tracing::debug!("macOS log stream capture shutdown signal received");
                let _ = child.kill().await;
                break;
            }

            line = reader.next_line() => {
                match line {
                    Ok(Some(line)) => {
                        // Skip the fixed header lines before data begins
                        if header_lines_remaining > 0 {
                            header_lines_remaining -= 1;
                            continue;
                        }

                        if let Some(parsed) = parse_syslog_line(&line) {
                            let tag = derive_tag(&parsed);

                            if !should_include_tag(&config, &tag) {
                                continue;
                            }

                            let event = syslog_line_to_event(&parsed);
                            if event_tx.send(event).await.is_err() {
                                // Receiver dropped — stop capture
                                break;
                            }
                        }
                    }
                    Ok(None) => {
                        tracing::debug!("log stream process exited (EOF)");
                        break;
                    }
                    Err(e) => {
                        tracing::warn!("Error reading log stream output: {}", e);
                        break;
                    }
                }
            }
        }
    }
}

/// macOS unified log capture backend.
///
/// Spawns `log stream --process <name> --style syslog` and parses the output
/// into [`NativeLogEvent`] values that are forwarded via an async channel.
pub struct MacOsLogCapture {
    config: MacOsLogConfig,
}

impl MacOsLogCapture {
    /// Create a new macOS log capture backend with the given configuration.
    pub fn new(config: MacOsLogConfig) -> Self {
        Self { config }
    }
}

impl NativeLogCapture for MacOsLogCapture {
    fn spawn(&self) -> Option<NativeLogHandle> {
        let config = MacOsLogConfig {
            process_name: self.config.process_name.clone(),
            exclude_tags: self.config.exclude_tags.clone(),
            include_tags: self.config.include_tags.clone(),
            min_level: self.config.min_level.clone(),
        };
        let (event_tx, event_rx) = mpsc::channel::<NativeLogEvent>(256);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        let task_handle = tokio::spawn(async move {
            run_log_stream_capture(config, event_tx, shutdown_rx).await;
        });

        Some(NativeLogHandle {
            event_rx,
            shutdown_tx,
            task_handle,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fdemon_core::LogLevel;

    #[test]
    fn test_parse_syslog_line_with_subsystem_and_category() {
        let line = "2024-03-10 14:30:00.123456-0700  0x1234     Info        0x0                  5678   0    my_app: (MyPlugin) [com.example.plugin:default] Hello from plugin";
        let parsed = parse_syslog_line(line).unwrap();
        assert_eq!(parsed.level, "Info");
        assert_eq!(parsed.subsystem, Some("MyPlugin".into()));
        assert_eq!(parsed.category, Some("com.example.plugin:default".into()));
        assert_eq!(parsed.message, "Hello from plugin");
    }

    #[test]
    fn test_parse_syslog_line_without_subsystem() {
        let line = "2024-03-10 14:30:00.123456-0700  0x1234     Error       0x0                  5678   0    my_app: NSLog message here";
        let parsed = parse_syslog_line(line).unwrap();
        assert_eq!(parsed.level, "Error");
        assert!(parsed.subsystem.is_none());
        assert!(parsed.category.is_none());
        assert_eq!(parsed.message, "NSLog message here");
    }

    #[test]
    fn test_derive_tag_from_category() {
        let line = SyslogLine {
            timestamp: "".into(),
            level: "Info".into(),
            subsystem: Some("MyPlugin".into()),
            category: Some("com.example.plugin:default".into()),
            message: "test".into(),
        };
        assert_eq!(derive_tag(&line), "com.example.plugin");
    }

    #[test]
    fn test_derive_tag_from_subsystem_fallback() {
        let line = SyslogLine {
            timestamp: "".into(),
            level: "Info".into(),
            subsystem: Some("Foundation".into()),
            category: None,
            message: "test".into(),
        };
        assert_eq!(derive_tag(&line), "Foundation");
    }

    #[test]
    fn test_derive_tag_native_fallback() {
        let line = SyslogLine {
            timestamp: "".into(),
            level: "Info".into(),
            subsystem: None,
            category: None,
            message: "test".into(),
        };
        assert_eq!(derive_tag(&line), "native");
    }

    #[test]
    fn test_syslog_line_to_event_level_mapping() {
        let make_line = |level: &str| SyslogLine {
            timestamp: "2024-03-10 14:30:00.123456-0700".into(),
            level: level.into(),
            subsystem: Some("Test".into()),
            category: None,
            message: "msg".into(),
        };

        assert_eq!(
            syslog_line_to_event(&make_line("Debug")).level,
            LogLevel::Debug
        );
        assert_eq!(
            syslog_line_to_event(&make_line("Info")).level,
            LogLevel::Info
        );
        assert_eq!(
            syslog_line_to_event(&make_line("Default")).level,
            LogLevel::Info
        );
        assert_eq!(
            syslog_line_to_event(&make_line("Error")).level,
            LogLevel::Error
        );
        assert_eq!(
            syslog_line_to_event(&make_line("Fault")).level,
            LogLevel::Error
        );
    }

    #[test]
    fn test_header_line_does_not_parse() {
        assert!(
            parse_syslog_line("Filtering the log data using \"process == \\\"my_app\\\"\"")
                .is_none()
        );
        assert!(parse_syslog_line("Timestamp                       Thread     Type        Activity             PID    TTL").is_none());
    }

    #[test]
    fn test_should_include_tag_no_filter_passes_all() {
        let config = MacOsLogConfig {
            process_name: "my_app".into(),
            exclude_tags: vec![],
            include_tags: vec![],
            min_level: "info".into(),
        };
        assert!(should_include_tag(&config, "anything"));
    }

    #[test]
    fn test_should_include_tag_exclude_filter() {
        let config = MacOsLogConfig {
            process_name: "my_app".into(),
            exclude_tags: vec!["Flutter".into()],
            include_tags: vec![],
            min_level: "info".into(),
        };
        assert!(!should_include_tag(&config, "flutter"));
        assert!(should_include_tag(&config, "GoLog"));
    }

    #[test]
    fn test_should_include_tag_include_filter_overrides_exclude() {
        let config = MacOsLogConfig {
            process_name: "my_app".into(),
            exclude_tags: vec!["Flutter".into()],
            include_tags: vec!["GoLog".into()],
            min_level: "info".into(),
        };
        assert!(should_include_tag(&config, "GoLog"));
        assert!(!should_include_tag(&config, "Flutter"));
        assert!(!should_include_tag(&config, "other"));
    }

    #[test]
    fn test_derive_tag_category_with_no_colon_suffix() {
        let line = SyslogLine {
            timestamp: "".into(),
            level: "Info".into(),
            subsystem: None,
            category: Some("com.example.plugin".into()),
            message: "test".into(),
        };
        assert_eq!(derive_tag(&line), "com.example.plugin");
    }

    #[test]
    fn test_parse_syslog_line_captures_timestamp() {
        let line = "2024-03-10 14:30:00.123456-0700  0x1234     Info        0x0                  5678   0    my_app: hello";
        let parsed = parse_syslog_line(line).unwrap();
        assert_eq!(parsed.timestamp, "2024-03-10 14:30:00.123456-0700");
    }

    #[test]
    fn test_syslog_line_to_event_uses_subsystem_as_tag() {
        let line = SyslogLine {
            timestamp: "2024-03-10 14:30:00.123456-0700".into(),
            level: "Info".into(),
            subsystem: Some("Foundation".into()),
            category: None,
            message: "Some Foundation log".into(),
        };
        let event = syslog_line_to_event(&line);
        assert_eq!(event.tag, "Foundation");
        assert_eq!(event.message, "Some Foundation log");
        assert_eq!(
            event.timestamp,
            Some("2024-03-10 14:30:00.123456-0700".into())
        );
    }

    #[test]
    fn test_macos_log_capture_new_stores_config() {
        let config = MacOsLogConfig {
            process_name: "test_app".into(),
            exclude_tags: vec!["flutter".into()],
            include_tags: vec![],
            min_level: "debug".into(),
        };
        let capture = MacOsLogCapture::new(config);
        assert_eq!(capture.config.process_name, "test_app");
    }
}
