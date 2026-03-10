//! Android logcat capture backend.
//!
//! Implements [`NativeLogCapture`] by spawning `adb logcat` with threadtime format
//! and parsing its output into [`NativeLogEvent`](super::NativeLogEvent) values.
//!
//! ## Format
//!
//! Android logcat `threadtime` format:
//! ```text
//! MM-DD HH:MM:SS.mmm  PID  TID PRIO TAG     : message
//! 03-10 14:30:00.123  1234  5678 I GoLog   : Hello from Go
//! ```

use std::process::Stdio;
use std::sync::LazyLock;

use fdemon_core::{LogLevel, NativeLogPriority};
use regex::Regex;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::{mpsc, watch};

use super::{AndroidLogConfig, NativeLogCapture, NativeLogEvent, NativeLogHandle};

/// Compiled regex for logcat threadtime format:
/// `MM-DD HH:MM:SS.mmm  PID  TID PRIO TAG     : message`
static THREADTIME_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(\d{2}-\d{2})\s+(\d{2}:\d{2}:\d{2}\.\d{3})\s+(\d+)\s+(\d+)\s+([VDIWEF])\s+([^:]+?)\s*:\s*(.*)$",
    )
    .expect("threadtime regex is valid")
});

/// A single parsed logcat line in threadtime format.
#[derive(Debug, Clone)]
pub struct LogcatLine {
    /// Date portion: `MM-DD`
    pub date: String,
    /// Time portion: `HH:MM:SS.mmm`
    pub time: String,
    /// Process ID
    pub pid: u32,
    /// Thread ID
    pub tid: u32,
    /// Priority character: V/D/I/W/E/F
    pub priority: char,
    /// Log tag (e.g., `"GoLog"`, `"AndroidRuntime"`)
    pub tag: String,
    /// Log message content
    pub message: String,
}

/// Parse a single logcat threadtime line.
///
/// Returns `None` for non-matching lines such as header lines (`--- beginning of system`),
/// blank lines, or any line that does not match the threadtime format.
pub fn parse_threadtime_line(line: &str) -> Option<LogcatLine> {
    let caps = THREADTIME_RE.captures(line)?;
    Some(LogcatLine {
        date: caps[1].to_string(),
        time: caps[2].to_string(),
        pid: caps[3].parse().ok()?,
        tid: caps[4].parse().ok()?,
        priority: caps[5].chars().next()?,
        tag: caps[6].trim().to_string(),
        message: caps[7].to_string(),
    })
}

/// Convert a parsed logcat line into a [`NativeLogEvent`].
///
/// Returns `None` if the priority character is not recognized.
fn logcat_line_to_event(line: &LogcatLine) -> Option<NativeLogEvent> {
    let priority = NativeLogPriority::from_logcat_char(line.priority)?;
    Some(NativeLogEvent {
        tag: line.tag.clone(),
        level: priority.to_log_level(),
        message: line.message.clone(),
        timestamp: Some(format!("{} {}", line.date, line.time)),
    })
}

/// Parse the `min_level` string into a [`NativeLogPriority`].
///
/// Returns `None` for unrecognized strings (which means "no minimum filter").
fn parse_min_priority(level: &str) -> Option<NativeLogPriority> {
    match level.to_lowercase().as_str() {
        "verbose" => Some(NativeLogPriority::Verbose),
        "debug" => Some(NativeLogPriority::Debug),
        "info" => Some(NativeLogPriority::Info),
        "warning" => Some(NativeLogPriority::Warning),
        "error" => Some(NativeLogPriority::Error),
        _ => None,
    }
}

/// Build the `adb logcat` command for the given configuration.
///
/// Uses threadtime format and starts from 1 second ago to avoid replaying
/// the full ring buffer on connect.
fn build_logcat_command(config: &AndroidLogConfig) -> Command {
    let mut cmd = Command::new("adb");
    cmd.arg("-s").arg(&config.device_serial);
    cmd.arg("logcat");

    // PID-based filtering: captures all tags from the app process only.
    if let Some(pid) = config.pid {
        cmd.arg(format!("--pid={pid}"));
    }

    // Use threadtime format for structured parsing.
    cmd.arg("-v").arg("threadtime");

    // Start from 1 second ago to avoid dumping the full ring buffer on connect.
    cmd.arg("-T").arg("1");

    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::null());
    // Ensure the child is killed if the task is dropped.
    cmd.kill_on_drop(true);

    cmd
}

/// Background capture loop: reads stdout line-by-line, parses, filters, and emits events.
async fn run_logcat_capture(
    config: AndroidLogConfig,
    event_tx: mpsc::Sender<NativeLogEvent>,
    mut shutdown_rx: watch::Receiver<bool>,
) {
    let min_priority = parse_min_priority(&config.min_level);
    let min_severity: Option<u8> = min_priority.map(|p| p.to_log_level().severity());

    let mut cmd = build_logcat_command(&config);
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Failed to spawn adb logcat: {}", e);
            return;
        }
    };

    let stdout = match child.stdout.take() {
        Some(s) => s,
        None => {
            tracing::warn!("adb logcat stdout not available");
            return;
        }
    };

    let mut reader = BufReader::new(stdout).lines();

    loop {
        tokio::select! {
            biased;

            _ = shutdown_rx.changed() => {
                tracing::debug!("Native log capture shutdown signal received");
                let _ = child.kill().await;
                break;
            }

            line = reader.next_line() => {
                match line {
                    Ok(Some(line)) => {
                        if let Some(parsed) = parse_threadtime_line(&line) {
                            // Apply tag filter.
                            if !super::should_include_tag(
                                &config.include_tags,
                                &config.exclude_tags,
                                &parsed.tag,
                            ) {
                                continue;
                            }
                            // Apply priority filter.
                            if let Some(min_sev) = min_severity {
                                let line_sev = NativeLogPriority::from_logcat_char(parsed.priority)
                                    .map(|p| p.to_log_level().severity())
                                    .unwrap_or(LogLevel::Debug.severity());
                                if line_sev < min_sev {
                                    continue;
                                }
                            }
                            // Convert and emit.
                            if let Some(event) = logcat_line_to_event(&parsed) {
                                if event_tx.send(event).await.is_err() {
                                    // Receiver dropped — stop silently.
                                    break;
                                }
                            }
                        }
                    }
                    Ok(None) => {
                        // EOF — adb logcat process exited.
                        tracing::debug!("adb logcat process exited (EOF)");
                        break;
                    }
                    Err(e) => {
                        tracing::warn!("Error reading adb logcat output: {}", e);
                        break;
                    }
                }
            }
        }
    }
}

/// Android logcat capture backend.
///
/// Spawns `adb -s <serial> logcat` with optional PID filtering and parses
/// the output into [`NativeLogEvent`](super::NativeLogEvent) values.
pub struct AndroidLogCapture {
    config: AndroidLogConfig,
}

impl AndroidLogCapture {
    /// Create a new Android logcat capture backend with the given configuration.
    pub fn new(config: AndroidLogConfig) -> Self {
        Self { config }
    }
}

impl NativeLogCapture for AndroidLogCapture {
    fn spawn(&self) -> Option<NativeLogHandle> {
        let config = self.config.clone();

        let (event_tx, event_rx) = mpsc::channel::<NativeLogEvent>(super::EVENT_CHANNEL_CAPACITY);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        let task_handle = tokio::spawn(async move {
            run_logcat_capture(config, event_tx, shutdown_rx).await;
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

    #[test]
    fn test_parse_threadtime_standard_line() {
        let line = "03-10 14:30:00.123  1234  5678 I GoLog   : Hello from Go";
        let parsed = parse_threadtime_line(line).unwrap();
        assert_eq!(parsed.date, "03-10");
        assert_eq!(parsed.time, "14:30:00.123");
        assert_eq!(parsed.pid, 1234);
        assert_eq!(parsed.tid, 5678);
        assert_eq!(parsed.priority, 'I');
        assert_eq!(parsed.tag, "GoLog");
        assert_eq!(parsed.message, "Hello from Go");
    }

    #[test]
    fn test_parse_threadtime_error_priority() {
        let line = "03-10 14:30:00.123  1234  5678 E AndroidRuntime: FATAL EXCEPTION: main";
        let parsed = parse_threadtime_line(line).unwrap();
        assert_eq!(parsed.priority, 'E');
        assert_eq!(parsed.tag, "AndroidRuntime");
    }

    #[test]
    fn test_parse_threadtime_wide_pid_tid() {
        let line = "03-10 14:30:00.123 12345 67890 W OkHttp  : Connection pool timeout";
        let parsed = parse_threadtime_line(line).unwrap();
        assert_eq!(parsed.pid, 12345);
        assert_eq!(parsed.tid, 67890);
    }

    #[test]
    fn test_parse_threadtime_header_lines() {
        assert!(parse_threadtime_line("--------- beginning of system").is_none());
        assert!(parse_threadtime_line("--------- beginning of main").is_none());
        assert!(parse_threadtime_line("").is_none());
    }

    #[test]
    fn test_parse_threadtime_empty_message() {
        let line = "03-10 14:30:00.123  1234  5678 D MyTag   : ";
        let parsed = parse_threadtime_line(line).unwrap();
        assert_eq!(parsed.message, "");
    }

    #[test]
    fn test_parse_threadtime_message_with_colons() {
        let line = "03-10 14:30:00.123  1234  5678 I GoLog   : key: value: nested";
        let parsed = parse_threadtime_line(line).unwrap();
        assert_eq!(parsed.message, "key: value: nested");
    }

    #[test]
    fn test_parse_threadtime_all_priority_chars() {
        for (ch, _expected) in [
            ('V', NativeLogPriority::Verbose),
            ('D', NativeLogPriority::Debug),
            ('I', NativeLogPriority::Info),
            ('W', NativeLogPriority::Warning),
            ('E', NativeLogPriority::Error),
            ('F', NativeLogPriority::Fatal),
        ] {
            let line = format!("03-10 14:30:00.123  1234  5678 {ch} SomeTag  : msg");
            let parsed = parse_threadtime_line(&line).unwrap();
            assert_eq!(parsed.priority, ch);
        }
    }

    #[test]
    fn test_logcat_line_to_event() {
        let line = LogcatLine {
            date: "03-10".into(),
            time: "14:30:00.123".into(),
            pid: 1234,
            tid: 5678,
            priority: 'W',
            tag: "GoLog".into(),
            message: "test".into(),
        };
        let event = logcat_line_to_event(&line).unwrap();
        assert_eq!(event.tag, "GoLog");
        assert_eq!(event.level, LogLevel::Warning);
        assert_eq!(event.message, "test");
        assert_eq!(event.timestamp, Some("03-10 14:30:00.123".into()));
    }

    #[test]
    fn test_logcat_line_to_event_verbose_maps_to_debug() {
        let line = LogcatLine {
            date: "03-10".into(),
            time: "14:30:00.123".into(),
            pid: 1234,
            tid: 5678,
            priority: 'V',
            tag: "VerboseTag".into(),
            message: "verbose msg".into(),
        };
        let event = logcat_line_to_event(&line).unwrap();
        assert_eq!(event.level, LogLevel::Debug);
    }

    #[test]
    fn test_logcat_line_to_event_fatal_maps_to_error() {
        let line = LogcatLine {
            date: "03-10".into(),
            time: "14:30:00.123".into(),
            pid: 1234,
            tid: 5678,
            priority: 'F',
            tag: "FatalTag".into(),
            message: "fatal msg".into(),
        };
        let event = logcat_line_to_event(&line).unwrap();
        assert_eq!(event.level, LogLevel::Error);
    }

    #[test]
    fn test_parse_min_priority() {
        assert_eq!(
            parse_min_priority("verbose"),
            Some(NativeLogPriority::Verbose)
        );
        assert_eq!(parse_min_priority("debug"), Some(NativeLogPriority::Debug));
        assert_eq!(parse_min_priority("info"), Some(NativeLogPriority::Info));
        assert_eq!(
            parse_min_priority("warning"),
            Some(NativeLogPriority::Warning)
        );
        assert_eq!(parse_min_priority("error"), Some(NativeLogPriority::Error));
        assert_eq!(parse_min_priority("invalid"), None);
        assert_eq!(parse_min_priority(""), None);
    }

    #[test]
    fn test_parse_min_priority_case_insensitive() {
        assert_eq!(parse_min_priority("INFO"), Some(NativeLogPriority::Info));
        assert_eq!(
            parse_min_priority("Warning"),
            Some(NativeLogPriority::Warning)
        );
    }

    #[test]
    fn test_logcat_line_to_event_unknown_priority_returns_none() {
        let line = LogcatLine {
            date: "03-10".into(),
            time: "14:30:00.123".into(),
            pid: 1234,
            tid: 5678,
            priority: 'X', // unknown priority
            tag: "SomeTag".into(),
            message: "msg".into(),
        };
        assert!(logcat_line_to_event(&line).is_none());
    }
}
