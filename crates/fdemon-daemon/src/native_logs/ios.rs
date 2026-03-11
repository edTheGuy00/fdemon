//! # iOS Native Log Capture
//!
//! Captures native platform logs from iOS devices (physical and simulator).
//!
//! ## Simulator
//!
//! Uses `xcrun simctl spawn <udid> log stream --predicate 'process == "Runner"' --style syslog`
//! to capture the unified logging stream. Output format matches macOS `log stream` syslog format.
//!
//! ## Physical Device
//!
//! Uses `idevicesyslog -u <udid> -p Runner` to relay the device's syslog stream.
//! Output format: `Mon DD HH:MM:SS DeviceName Process(Framework)[PID] <Level>: message`
//!
//! ## Tool Availability
//!
//! - Simulator: requires `xcrun simctl` (checked via `ToolAvailability::xcrun_simctl`)
//! - Physical: requires `idevicesyslog` (checked via `ToolAvailability::idevicesyslog`)
//! - Both tools only available on macOS (this entire module is `#[cfg(target_os = "macos")]`)

use std::process::Stdio;
use std::sync::LazyLock;

use fdemon_core::LogLevel;
use regex::Regex;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::{mpsc, watch};

use super::{
    macos::{derive_tag, parse_syslog_line},
    IosLogConfig, NativeLogCapture, NativeLogEvent, NativeLogHandle,
};

// ── BSD syslog (idevicesyslog) parser ─────────────────────────────────────────

/// Regex for `idevicesyslog` output (BSD syslog format).
///
/// Format: `MMM DD HH:MM:SS DeviceName Process(Framework)[PID] <Level>: message`
///
/// Examples:
/// ```text
/// Mar 15 12:34:56 iPhone Runner(Flutter)[2037] <Notice>: flutter: Hello from Dart
/// Mar 15 12:35:01 Eds-iPhone Runner(MyPlugin)[2037] <Warning>: Plugin timeout after 5s
/// Mar 15 12:35:03 iPhone Runner(libsystem_network.dylib)[2037] <Debug>: nw_protocol_get_quic_image_block_invoke
/// ```
static IDEVICESYSLOG_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(\w{3}\s+\d{1,2}\s+\d{2}:\d{2}:\d{2})\s+\S+\s+(\w+)\(([^)]*)\)\[(\d+)\]\s+<(\w+)>:\s*(.*)$",
    )
    .expect("idevicesyslog regex is valid")
});

/// A parsed line from `idevicesyslog` output (BSD syslog format).
#[derive(Debug, Clone)]
pub struct IdevicesyslogLine {
    /// Timestamp in BSD syslog format (e.g., `"Mar 15 12:34:56"`).
    pub timestamp: String,
    /// Process name (e.g., `"Runner"`).
    pub process: String,
    /// Framework/library name (e.g., `"Flutter"`, `"MyPlugin"`, `"CoreText"`).
    pub framework: String,
    /// Process ID.
    pub pid: u32,
    /// Syslog level string (e.g., `"Notice"`, `"Warning"`, `"Error"`).
    pub level_str: String,
    /// Log message content.
    pub message: String,
}

/// Parse a single line of `idevicesyslog` output.
///
/// Returns `None` for non-matching lines such as connection messages, separator lines,
/// blank lines, or any line that does not match the BSD syslog format.
pub fn parse_idevicesyslog_line(line: &str) -> Option<IdevicesyslogLine> {
    let caps = IDEVICESYSLOG_RE.captures(line)?;
    Some(IdevicesyslogLine {
        timestamp: caps[1].to_string(),
        process: caps[2].to_string(),
        framework: caps[3].to_string(),
        pid: caps[4].parse().ok()?,
        level_str: caps[5].to_string(),
        message: caps[6].to_string(),
    })
}

/// Map a BSD syslog level string to [`LogLevel`].
///
/// BSD syslog levels in severity order (highest to lowest):
/// `Emergency`, `Alert`, `Critical`, `Error`, `Warning`, `Notice`, `Info`, `Debug`
///
/// This differs from macOS unified logging which uses `Default`/`Info`/`Debug`/`Error`/`Fault`.
pub fn bsd_syslog_level_to_log_level(level: &str) -> LogLevel {
    match level.to_lowercase().as_str() {
        "emergency" | "alert" | "critical" => LogLevel::Error,
        "error" => LogLevel::Error,
        "warning" => LogLevel::Warning,
        "notice" | "info" => LogLevel::Info,
        "debug" => LogLevel::Debug,
        _ => LogLevel::Info, // Default for unrecognized levels
    }
}

/// Convert a parsed `idevicesyslog` line to a [`NativeLogEvent`].
///
/// The framework field (e.g., `"Flutter"`, `"MyPlugin"`, `"CoreText"`) is used
/// as the tag rather than the process name (always `"Runner"`) — framework is
/// more informative for filtering.
fn idevicesyslog_line_to_event(line: &IdevicesyslogLine) -> NativeLogEvent {
    // Use the framework name as the tag (more useful than "Runner")
    let tag = line.framework.clone();
    let level = bsd_syslog_level_to_log_level(&line.level_str);

    NativeLogEvent {
        tag,
        level,
        message: line.message.clone(),
        timestamp: Some(line.timestamp.clone()),
    }
}

/// Parse `min_level` string into a [`LogLevel`] for downstream filtering.
///
/// Returns `None` for unrecognized strings (meaning no minimum filter is applied).
pub fn parse_min_level(level: &str) -> Option<LogLevel> {
    match level.to_lowercase().as_str() {
        "verbose" | "debug" => Some(LogLevel::Debug),
        "info" => Some(LogLevel::Info),
        "warning" => Some(LogLevel::Warning),
        "error" => Some(LogLevel::Error),
        _ => None,
    }
}

// ── Physical device command builder ───────────────────────────────────────────

/// Build the `idevicesyslog` [`Command`] for physical device capture.
pub fn build_idevicesyslog_command(config: &IosLogConfig) -> Command {
    let mut cmd = Command::new("idevicesyslog");

    // Target specific device by UDID
    cmd.arg("-u").arg(&config.device_udid);

    // Filter to the Runner process (the Flutter iOS host process)
    cmd.arg("-p").arg(&config.process_name);

    // Suppress kernel messages (noisy, rarely relevant to Flutter development)
    cmd.arg("-K");

    // Disable ANSI color codes for clean regex parsing
    cmd.arg("--no-colors");

    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::null());
    cmd.kill_on_drop(true);

    cmd
}

/// Background task that runs `idevicesyslog` and forwards parsed events.
///
/// Exits when the process ends, an I/O error occurs, the receiver is dropped,
/// or `shutdown_rx` signals `true`.
async fn run_idevicesyslog_capture(
    config: IosLogConfig,
    event_tx: mpsc::Sender<NativeLogEvent>,
    mut shutdown_rx: watch::Receiver<bool>,
) {
    let min_level = parse_min_level(&config.min_level);

    let mut cmd = build_idevicesyslog_command(&config);
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Failed to spawn idevicesyslog: {}", e);
            return;
        }
    };

    let stdout = match child.stdout.take() {
        Some(s) => s,
        None => {
            tracing::warn!("idevicesyslog stdout not available");
            return;
        }
    };

    let mut reader = BufReader::new(stdout).lines();

    loop {
        tokio::select! {
            biased;

            _ = shutdown_rx.changed() => {
                tracing::debug!("iOS physical device log capture shutdown signal received");
                let _ = child.kill().await;
                break;
            }

            line = reader.next_line() => {
                match line {
                    Ok(Some(line)) => {
                        if let Some(parsed) = parse_idevicesyslog_line(&line) {
                            let event = idevicesyslog_line_to_event(&parsed);

                            // Apply tag filter
                            if !super::should_include_tag(
                                &config.include_tags,
                                &config.exclude_tags,
                                &event.tag,
                            ) {
                                continue;
                            }

                            // Apply level filter
                            if let Some(min) = min_level {
                                if event.level.severity() < min.severity() {
                                    continue;
                                }
                            }

                            if event_tx.send(event).await.is_err() {
                                // Receiver dropped — stop silently.
                                break;
                            }
                        }
                    }
                    Ok(None) => {
                        tracing::debug!("idevicesyslog exited (EOF)");
                        break;
                    }
                    Err(e) => {
                        tracing::warn!("Error reading idevicesyslog: {}", e);
                        break;
                    }
                }
            }
        }
    }
}

// ── Simulator command builder ──────────────────────────────────────────────────

/// Build the `xcrun simctl spawn log stream` [`Command`] for simulator capture.
fn build_simctl_log_stream_command(config: &IosLogConfig) -> Command {
    let mut cmd = Command::new("xcrun");
    cmd.arg("simctl");
    cmd.arg("spawn");
    cmd.arg(&config.device_udid);
    cmd.arg("log");
    cmd.arg("stream");

    // Filter by process name via --predicate
    let predicate = format!("process == \"{}\"", config.process_name);
    cmd.arg("--predicate").arg(&predicate);

    // Use syslog style for structured, parseable output (same format as macOS)
    cmd.arg("--style").arg("syslog");

    // Map min_level to the closest valid `log stream --level` argument.
    // macOS / simctl log stream only accepts: "default", "info", "debug".
    let level = match config.min_level.to_lowercase().as_str() {
        "verbose" | "debug" => "debug",
        "info" => "info",
        _ => "default",
    };
    cmd.arg("--level").arg(level);

    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::null());
    cmd.kill_on_drop(true);

    cmd
}

/// Background task that runs `xcrun simctl spawn log stream` and forwards parsed events.
///
/// Skips the header line emitted on startup, then parses syslog-style lines using
/// the same parser as the macOS backend. Exits when the process ends, an I/O error
/// occurs, the receiver is dropped, or `shutdown_rx` signals `true`.
async fn run_simctl_log_capture(
    config: IosLogConfig,
    event_tx: mpsc::Sender<NativeLogEvent>,
    mut shutdown_rx: watch::Receiver<bool>,
) {
    let mut cmd = build_simctl_log_stream_command(&config);
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Failed to spawn simctl log stream: {}", e);
            return;
        }
    };

    let stdout = match child.stdout.take() {
        Some(s) => s,
        None => {
            tracing::warn!("simctl log stream stdout not available");
            return;
        }
    };

    let mut reader = BufReader::new(stdout).lines();

    loop {
        tokio::select! {
            biased;

            _ = shutdown_rx.changed() => {
                tracing::debug!("iOS simulator log capture shutdown signal received");
                let _ = child.kill().await;
                break;
            }

            line = reader.next_line() => {
                match line {
                    Ok(Some(line)) => {
                        // Skip the header lines that log stream outputs on startup
                        if line.starts_with("Filtering the log data") || line.starts_with("Timestamp") {
                            continue;
                        }

                        if let Some(parsed) = parse_syslog_line(&line) {
                            let tag = derive_tag(&parsed);

                            if !super::should_include_tag(
                                &config.include_tags,
                                &config.exclude_tags,
                                &tag,
                            ) {
                                continue;
                            }

                            let event = NativeLogEvent {
                                tag,
                                level: fdemon_core::NativeLogPriority::from_macos_level(&parsed.level)
                                    .unwrap_or(fdemon_core::NativeLogPriority::Info)
                                    .to_log_level(),
                                message: parsed.message,
                                timestamp: Some(parsed.timestamp),
                            };

                            if event_tx.send(event).await.is_err() {
                                // Receiver dropped — stop silently.
                                break;
                            }
                        }
                    }
                    Ok(None) => {
                        tracing::debug!("simctl log stream exited (EOF)");
                        break;
                    }
                    Err(e) => {
                        tracing::warn!("Error reading simctl log stream: {}", e);
                        break;
                    }
                }
            }
        }
    }
}

// ── IosLogCapture struct ───────────────────────────────────────────────────────

/// iOS native log capture backend.
///
/// Dispatches to `xcrun simctl spawn log stream` (simulator) or
/// `idevicesyslog` (physical device) based on `config.is_simulator`.
pub struct IosLogCapture {
    config: IosLogConfig,
}

impl IosLogCapture {
    /// Create a new iOS log capture backend with the given configuration.
    pub fn new(config: IosLogConfig) -> Self {
        Self { config }
    }
}

impl NativeLogCapture for IosLogCapture {
    fn spawn(&self) -> Option<NativeLogHandle> {
        if self.config.is_simulator {
            self.spawn_simulator()
        } else {
            self.spawn_physical()
        }
    }
}

impl IosLogCapture {
    /// Spawn log capture for an iOS simulator using `xcrun simctl spawn log stream`.
    fn spawn_simulator(&self) -> Option<NativeLogHandle> {
        let config = self.config.clone();
        let (event_tx, event_rx) = mpsc::channel::<NativeLogEvent>(super::EVENT_CHANNEL_CAPACITY);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        let task_handle = tokio::spawn(async move {
            run_simctl_log_capture(config, event_tx, shutdown_rx).await;
        });

        Some(NativeLogHandle {
            event_rx,
            shutdown_tx,
            task_handle,
        })
    }

    /// Spawn log capture for a physical iOS device using `idevicesyslog`.
    fn spawn_physical(&self) -> Option<NativeLogHandle> {
        let config = self.config.clone();
        let (event_tx, event_rx) = mpsc::channel::<NativeLogEvent>(super::EVENT_CHANNEL_CAPACITY);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        let task_handle = tokio::spawn(async move {
            run_idevicesyslog_capture(config, event_tx, shutdown_rx).await;
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

    // ── parse_idevicesyslog_line tests ─────────────────────────────────────

    #[test]
    fn test_parse_idevicesyslog_standard_line() {
        let line =
            "Mar 15 12:34:56 iPhone Runner(Flutter)[2037] <Notice>: flutter: Hello from Dart";
        let parsed = parse_idevicesyslog_line(line).unwrap();
        assert_eq!(parsed.timestamp, "Mar 15 12:34:56");
        assert_eq!(parsed.process, "Runner");
        assert_eq!(parsed.framework, "Flutter");
        assert_eq!(parsed.pid, 2037);
        assert_eq!(parsed.level_str, "Notice");
        assert_eq!(parsed.message, "flutter: Hello from Dart");
    }

    #[test]
    fn test_parse_idevicesyslog_plugin_line() {
        let line =
            "Mar 15 12:35:01 iPhone Runner(MyPlugin)[2037] <Warning>: Plugin timeout after 5s";
        let parsed = parse_idevicesyslog_line(line).unwrap();
        assert_eq!(parsed.framework, "MyPlugin");
        assert_eq!(parsed.level_str, "Warning");
        assert_eq!(parsed.message, "Plugin timeout after 5s");
    }

    #[test]
    fn test_parse_idevicesyslog_error_line() {
        let line = "Mar 15 12:35:02 iPhone Runner(CoreText)[2037] <Error>: Missing font descriptor";
        let parsed = parse_idevicesyslog_line(line).unwrap();
        assert_eq!(parsed.framework, "CoreText");
        assert_eq!(parsed.level_str, "Error");
    }

    #[test]
    fn test_parse_idevicesyslog_debug_line() {
        let line = "Mar 15 12:35:03 iPhone Runner(libsystem_network.dylib)[2037] <Debug>: nw_protocol_get_quic_image_block_invoke dlopen libquic";
        let parsed = parse_idevicesyslog_line(line).unwrap();
        assert_eq!(parsed.framework, "libsystem_network.dylib");
        assert_eq!(parsed.level_str, "Debug");
    }

    #[test]
    fn test_parse_idevicesyslog_device_name_with_hyphen() {
        let line = "Mar 15 12:34:56 Eds-iPhone Runner(Flutter)[1234] <Notice>: test";
        let parsed = parse_idevicesyslog_line(line).unwrap();
        assert_eq!(parsed.process, "Runner");
        assert_eq!(parsed.pid, 1234);
    }

    #[test]
    fn test_parse_idevicesyslog_non_matching_lines() {
        assert!(parse_idevicesyslog_line("").is_none());
        assert!(parse_idevicesyslog_line("Connected to device").is_none());
        assert!(parse_idevicesyslog_line("---").is_none());
    }

    #[test]
    fn test_parse_idevicesyslog_message_with_colons() {
        let line = "Mar 15 12:34:56 iPhone Runner(Flutter)[2037] <Notice>: key: value: nested";
        let parsed = parse_idevicesyslog_line(line).unwrap();
        assert_eq!(parsed.message, "key: value: nested");
    }

    // ── bsd_syslog_level_to_log_level tests ───────────────────────────────

    #[test]
    fn test_bsd_syslog_level_mapping() {
        assert_eq!(bsd_syslog_level_to_log_level("Emergency"), LogLevel::Error);
        assert_eq!(bsd_syslog_level_to_log_level("Alert"), LogLevel::Error);
        assert_eq!(bsd_syslog_level_to_log_level("Critical"), LogLevel::Error);
        assert_eq!(bsd_syslog_level_to_log_level("Error"), LogLevel::Error);
        assert_eq!(bsd_syslog_level_to_log_level("Warning"), LogLevel::Warning);
        assert_eq!(bsd_syslog_level_to_log_level("Notice"), LogLevel::Info);
        assert_eq!(bsd_syslog_level_to_log_level("Info"), LogLevel::Info);
        assert_eq!(bsd_syslog_level_to_log_level("Debug"), LogLevel::Debug);
    }

    #[test]
    fn test_bsd_syslog_level_case_insensitive() {
        assert_eq!(bsd_syslog_level_to_log_level("EMERGENCY"), LogLevel::Error);
        assert_eq!(bsd_syslog_level_to_log_level("notice"), LogLevel::Info);
        assert_eq!(bsd_syslog_level_to_log_level("WARNING"), LogLevel::Warning);
    }

    #[test]
    fn test_bsd_syslog_level_unknown_maps_to_info() {
        assert_eq!(bsd_syslog_level_to_log_level("Unknown"), LogLevel::Info);
        assert_eq!(bsd_syslog_level_to_log_level(""), LogLevel::Info);
    }

    // ── idevicesyslog_line_to_event tests ─────────────────────────────────

    #[test]
    fn test_idevicesyslog_line_to_event() {
        let line = IdevicesyslogLine {
            timestamp: "Mar 15 12:34:56".into(),
            process: "Runner".into(),
            framework: "MyPlugin".into(),
            pid: 2037,
            level_str: "Warning".into(),
            message: "connection timeout".into(),
        };
        let event = idevicesyslog_line_to_event(&line);
        assert_eq!(event.tag, "MyPlugin");
        assert_eq!(event.level, LogLevel::Warning);
        assert_eq!(event.message, "connection timeout");
        assert_eq!(event.timestamp, Some("Mar 15 12:34:56".into()));
    }

    #[test]
    fn test_idevicesyslog_line_to_event_uses_framework_not_process() {
        let line = IdevicesyslogLine {
            timestamp: "Mar 15 12:34:56".into(),
            process: "Runner".into(),
            framework: "Flutter".into(),
            pid: 1234,
            level_str: "Notice".into(),
            message: "dart message".into(),
        };
        let event = idevicesyslog_line_to_event(&line);
        // Tag must be framework ("Flutter"), not process ("Runner")
        assert_eq!(event.tag, "Flutter");
    }

    // ── build_idevicesyslog_command tests ──────────────────────────────────

    #[test]
    fn test_build_idevicesyslog_command() {
        let config = IosLogConfig {
            device_udid: "00008030-0000111ABC000DEF".to_string(),
            is_simulator: false,
            process_name: "Runner".to_string(),
            exclude_tags: vec![],
            include_tags: vec![],
            min_level: "info".to_string(),
        };
        let cmd = build_idevicesyslog_command(&config);
        let std_cmd = cmd.as_std();
        let args: Vec<String> = std_cmd
            .get_args()
            .map(|a| a.to_string_lossy().into())
            .collect();
        assert!(args.contains(&"-u".to_string()));
        assert!(args.contains(&"00008030-0000111ABC000DEF".to_string()));
        assert!(args.contains(&"-p".to_string()));
        assert!(args.contains(&"Runner".to_string()));
        assert!(args.contains(&"-K".to_string()));
        assert!(args.contains(&"--no-colors".to_string()));
    }

    // ── parse_min_level tests ──────────────────────────────────────────────

    #[test]
    fn test_parse_min_level() {
        assert_eq!(parse_min_level("debug"), Some(LogLevel::Debug));
        assert_eq!(parse_min_level("verbose"), Some(LogLevel::Debug));
        assert_eq!(parse_min_level("info"), Some(LogLevel::Info));
        assert_eq!(parse_min_level("warning"), Some(LogLevel::Warning));
        assert_eq!(parse_min_level("error"), Some(LogLevel::Error));
        assert_eq!(parse_min_level("invalid"), None);
        assert_eq!(parse_min_level(""), None);
    }

    #[test]
    fn test_parse_min_level_case_insensitive() {
        assert_eq!(parse_min_level("DEBUG"), Some(LogLevel::Debug));
        assert_eq!(parse_min_level("INFO"), Some(LogLevel::Info));
        assert_eq!(parse_min_level("WARNING"), Some(LogLevel::Warning));
        assert_eq!(parse_min_level("ERROR"), Some(LogLevel::Error));
    }

    // ── build_simctl_log_stream_command tests ──────────────────────────────

    #[test]
    fn test_build_simctl_log_stream_command_basic() {
        let config = IosLogConfig {
            device_udid: "AAAAAAAA-BBBB-CCCC-DDDD-EEEEEEEEEEEE".to_string(),
            is_simulator: true,
            process_name: "Runner".to_string(),
            exclude_tags: vec![],
            include_tags: vec![],
            min_level: "debug".to_string(),
        };
        let cmd = build_simctl_log_stream_command(&config);
        let std_cmd = cmd.as_std();
        let args: Vec<&std::ffi::OsStr> = std_cmd.get_args().collect();
        assert_eq!(args[0], "simctl");
        assert_eq!(args[1], "spawn");
        assert_eq!(args[2], "AAAAAAAA-BBBB-CCCC-DDDD-EEEEEEEEEEEE");
        assert_eq!(args[3], "log");
        assert_eq!(args[4], "stream");
    }

    #[test]
    fn test_build_simctl_log_stream_command_predicate() {
        let config = IosLogConfig {
            device_udid: "test-uuid".to_string(),
            is_simulator: true,
            process_name: "Runner".to_string(),
            exclude_tags: vec![],
            include_tags: vec![],
            min_level: "info".to_string(),
        };
        let cmd = build_simctl_log_stream_command(&config);
        let std_cmd = cmd.as_std();
        let args: Vec<String> = std_cmd
            .get_args()
            .map(|a| a.to_string_lossy().into())
            .collect();
        assert!(args.contains(&"--predicate".to_string()));
        assert!(args.iter().any(|a| a.contains("process == \"Runner\"")));
        assert!(args.contains(&"--style".to_string()));
        assert!(args.contains(&"syslog".to_string()));
    }

    #[test]
    fn test_build_simctl_log_stream_command_level_mapping() {
        let test_cases = [
            ("debug", "debug"),
            ("verbose", "debug"),
            ("info", "info"),
            ("warning", "default"),
            ("error", "default"),
        ];
        for (input, expected) in test_cases {
            let config = IosLogConfig {
                device_udid: "test".to_string(),
                is_simulator: true,
                process_name: "Runner".to_string(),
                exclude_tags: vec![],
                include_tags: vec![],
                min_level: input.to_string(),
            };
            let cmd = build_simctl_log_stream_command(&config);
            let std_cmd = cmd.as_std();
            let args: Vec<String> = std_cmd
                .get_args()
                .map(|a| a.to_string_lossy().into())
                .collect();
            let level_idx = args
                .iter()
                .position(|a| a == "--level")
                .expect("--level arg must be present");
            assert_eq!(
                args[level_idx + 1],
                expected,
                "min_level '{}' should map to '{}'",
                input,
                expected
            );
        }
    }

    // ── IosLogCapture struct tests ─────────────────────────────────────────

    #[test]
    fn test_ios_log_capture_new_stores_config() {
        let config = IosLogConfig {
            device_udid: "test-udid".to_string(),
            is_simulator: true,
            process_name: "Runner".to_string(),
            exclude_tags: vec!["flutter".to_string()],
            include_tags: vec![],
            min_level: "info".to_string(),
        };
        let capture = IosLogCapture::new(config);
        assert_eq!(capture.config.device_udid, "test-udid");
        assert!(capture.config.is_simulator);
    }
}
