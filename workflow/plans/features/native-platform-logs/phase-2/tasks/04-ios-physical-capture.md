## Task: iOS Physical Device Log Capture Backend

**Objective**: Implement physical iOS device log capture in `ios.rs` using `idevicesyslog -u <udid> -p Runner`. This captures the device's syslog stream via the `os_trace_relay` lockdown service.

**Depends on**: 02-ios-log-config

### Scope

- `crates/fdemon-daemon/src/native_logs/ios.rs`: Add physical device capture path + `idevicesyslog` output parser

### Details

#### 1. `idevicesyslog` Output Format

The output is BSD syslog format:

```
Mar 15 12:34:56 iPhone-Name Runner(Flutter)[2037] <Notice>: flutter: Observatory listening on http://127.0.0.1:56486/
Mar 15 12:34:57 iPhone-Name Runner(Flutter)[2037] <Notice>: flutter: I/flutter (2037): some log message
Mar 15 12:35:01 iPhone-Name Runner(MyPlugin)[2037] <Warning>: Plugin timeout after 5s
Mar 15 12:35:02 iPhone-Name kernel[0] <Debug>: Sandbox: Runner(815) deny file-write-unlink
```

Format: `MonthDay HH:MM:SS DeviceName Process(Framework)[PID] <Level>: message`

Fields:
- **Timestamp**: `MMM DD HH:MM:SS` (BSD syslog, no year)
- **Device name**: The iOS device's name (e.g., `iPhone-Name`, `Ed-s-iPhone`)
- **Process**: Process name (e.g., `Runner`)
- **Framework**: Framework/library in parentheses (e.g., `Flutter`, `MyPlugin`, `CoreText`)
- **PID**: Process ID in brackets
- **Level**: Syslog level in angle brackets (`<Emergency>`, `<Alert>`, `<Critical>`, `<Error>`, `<Warning>`, `<Notice>`, `<Info>`, `<Debug>`)
- **Message**: The log message content

#### 2. Parse `idevicesyslog` output

```rust
use regex::Regex;
use std::sync::LazyLock;
use fdemon_core::{LogLevel, NativeLogPriority};

static IDEVICESYSLOG_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(\w{3}\s+\d{1,2}\s+\d{2}:\d{2}:\d{2})\s+\S+\s+(\w+)\(([^)]*)\)\[(\d+)\]\s+<(\w+)>:\s*(.*)$"
    ).expect("idevicesyslog regex is valid")
});

/// Parsed line from idevicesyslog output.
#[derive(Debug, Clone)]
pub struct IdevicesyslogLine {
    /// Timestamp in BSD syslog format (e.g., "Mar 15 12:34:56").
    pub timestamp: String,
    /// Process name (e.g., "Runner").
    pub process: String,
    /// Framework/library name (e.g., "Flutter", "MyPlugin", "CoreText").
    pub framework: String,
    /// Process ID.
    pub pid: u32,
    /// Syslog level string (e.g., "Notice", "Warning", "Error").
    pub level_str: String,
    /// Log message content.
    pub message: String,
}

/// Parse a single line of idevicesyslog output.
///
/// Returns `None` for non-matching lines (connection messages, separator lines, etc.).
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
```

#### 3. Map `idevicesyslog` levels to `LogLevel`

`idevicesyslog` uses BSD syslog levels. Add a conversion to `NativeLogPriority` in `fdemon-core/src/types.rs`:

```rust
impl NativeLogPriority {
    /// Map BSD syslog level strings (from idevicesyslog) to NativeLogPriority.
    ///
    /// BSD syslog levels: Emergency, Alert, Critical, Error, Warning, Notice, Info, Debug
    pub fn from_bsd_syslog_level(level: &str) -> Option<Self> {
        match level.to_lowercase().as_str() {
            "emergency" | "alert" | "critical" => Some(Self::Fatal),
            "error" => Some(Self::Error),
            "warning" => Some(Self::Warning),
            "notice" | "info" => Some(Self::Info),
            "debug" => Some(Self::Debug),
            _ => None,
        }
    }
}
```

Or alternatively, do the mapping inline in `ios.rs` without modifying fdemon-core:

```rust
fn bsd_syslog_level_to_log_level(level: &str) -> LogLevel {
    match level.to_lowercase().as_str() {
        "emergency" | "alert" | "critical" => LogLevel::Error,
        "error" => LogLevel::Error,
        "warning" => LogLevel::Warning,
        "notice" | "info" => LogLevel::Info,
        "debug" => LogLevel::Debug,
        _ => LogLevel::Info, // Default for unrecognized levels
    }
}
```

The choice between adding to `NativeLogPriority` (cleaner) or keeping local (less cross-crate impact) is left to the implementor.

#### 4. Convert parsed line to `NativeLogEvent`

```rust
fn idevicesyslog_line_to_event(line: &IdevicesyslogLine) -> NativeLogEvent {
    // Use the framework name as the tag (e.g., "Flutter", "MyPlugin", "CoreText")
    // This gives more useful tags than the process name (which is always "Runner")
    let tag = line.framework.clone();
    let level = bsd_syslog_level_to_log_level(&line.level_str);

    NativeLogEvent {
        tag,
        level,
        message: line.message.clone(),
        timestamp: Some(line.timestamp.clone()),
    }
}
```

**Tag derivation**: Use the `framework` field (the part in parentheses) as the tag. This is more useful than the process name since the process is always "Runner" for Flutter iOS apps. The framework field shows which library produced the log: `Flutter`, `MyPlugin`, `CoreText`, etc.

#### 5. Build `idevicesyslog` command

```rust
fn build_idevicesyslog_command(config: &IosLogConfig) -> Command {
    let mut cmd = Command::new("idevicesyslog");

    // Target specific device by UDID
    cmd.arg("-u").arg(&config.device_udid);

    // Filter to process name (typically "Runner" for Flutter iOS apps)
    cmd.arg("-p").arg(&config.process_name);

    // Suppress kernel messages (noisy, rarely relevant)
    cmd.arg("-K");

    // Disable ANSI color codes for clean parsing
    cmd.arg("--no-colors");

    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::null());
    cmd.kill_on_drop(true);

    cmd
}
```

#### 6. Run physical device capture loop

```rust
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
                            if !should_include_tag(
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

fn parse_min_level(level: &str) -> Option<LogLevel> {
    match level.to_lowercase().as_str() {
        "verbose" | "debug" => Some(LogLevel::Debug),
        "info" => Some(LogLevel::Info),
        "warning" => Some(LogLevel::Warning),
        "error" => Some(LogLevel::Error),
        _ => None,
    }
}
```

#### 7. Wire into `IosLogCapture::spawn_physical()`

Replace the stub from task 03:

```rust
fn spawn_physical(&self) -> Option<NativeLogHandle> {
    let config = self.config.clone();
    let (event_tx, event_rx) = mpsc::channel::<NativeLogEvent>(EVENT_CHANNEL_CAPACITY);
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
```

### Acceptance Criteria

1. `parse_idevicesyslog_line()` correctly parses standard idevicesyslog output format
2. `parse_idevicesyslog_line()` returns `None` for non-matching lines (connection messages, blank lines)
3. BSD syslog levels (`Notice`, `Warning`, `Error`, `Debug`, etc.) map correctly to `LogLevel`
4. `Emergency`/`Alert`/`Critical` all map to `LogLevel::Error`
5. Tag is derived from the framework field (e.g., `"Flutter"`, `"MyPlugin"`, `"CoreText"`)
6. Tag filtering via `should_include_tag()` works
7. Level filtering drops lines below `min_level`
8. `build_idevicesyslog_command()` includes `-u <udid>`, `-p <process>`, `-K`, `--no-colors`
9. Shutdown signal kills the child process and breaks the loop
10. `IosLogCapture::spawn()` returns `Some(NativeLogHandle)` for both simulator and physical paths
11. `cargo check -p fdemon-daemon` compiles
12. `cargo test -p fdemon-daemon` passes

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_idevicesyslog_standard_line() {
        let line = "Mar 15 12:34:56 iPhone Runner(Flutter)[2037] <Notice>: flutter: Hello from Dart";
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
        let line = "Mar 15 12:35:01 iPhone Runner(MyPlugin)[2037] <Warning>: Plugin timeout after 5s";
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
        let args: Vec<String> = std_cmd.get_args().map(|a| a.to_string_lossy().into()).collect();
        assert!(args.contains(&"-u".to_string()));
        assert!(args.contains(&"00008030-0000111ABC000DEF".to_string()));
        assert!(args.contains(&"-p".to_string()));
        assert!(args.contains(&"Runner".to_string()));
        assert!(args.contains(&"-K".to_string()));
        assert!(args.contains(&"--no-colors".to_string()));
    }

    #[test]
    fn test_parse_min_level() {
        assert_eq!(parse_min_level("debug"), Some(LogLevel::Debug));
        assert_eq!(parse_min_level("verbose"), Some(LogLevel::Debug));
        assert_eq!(parse_min_level("info"), Some(LogLevel::Info));
        assert_eq!(parse_min_level("warning"), Some(LogLevel::Warning));
        assert_eq!(parse_min_level("error"), Some(LogLevel::Error));
        assert_eq!(parse_min_level("invalid"), None);
    }
}
```

### Notes

- **`idevicesyslog` is broken on Xcode 26.** This implementation targets Xcode 15/16 (iOS 17+). When `idevicesyslog` fails to connect, the capture loop exits on spawn error or EOF, and `NativeLogCaptureStopped` is sent — no crash, no hang.
- **Tag derivation from framework field**: Using the framework name (e.g., `"Flutter"`, `"MyPlugin"`) as the tag is more informative than the process name (always `"Runner"`). The default `exclude_tags = ["flutter"]` will filter out `"Flutter"` framework entries to avoid duplication.
- **`-K` flag**: Suppresses kernel messages which are extremely noisy on iOS devices and rarely relevant to Flutter app development.
- **`--no-colors` flag**: Prevents ANSI escape codes from corrupting the regex parser. idevicesyslog outputs ANSI colors by default when stdout is a TTY, but since we pipe it, this may not be needed — included defensively.
- **BSD syslog levels vs macOS unified log levels**: idevicesyslog uses traditional BSD syslog levels (`Emergency`/`Alert`/`Critical`/`Error`/`Warning`/`Notice`/`Info`/`Debug`) which differ from macOS unified log types (`Default`/`Info`/`Debug`/`Error`/`Fault`). The mapping handles both.
- **No crash recovery**: Same as Android — the loop exits on error and the caller handles restart decisions.

## Completion Summary

**Status:** Done

### What Was Implemented

- `parse_idevicesyslog_line()` — parses BSD syslog format (`Mon DD HH:MM:SS device process[pid] <level>: message`) using `IDEVICESYSLOG_RE` regex
- `IdevicesyslogLine` struct with fields: `timestamp`, `device_name`, `process_name`, `pid`, `level`, `framework`, `message`
- `bsd_syslog_level_to_log_level()` — maps all 8 BSD syslog levels (Emergency → Error, Alert → Error, Critical → Error, Error → Error, Warning → Warning, Notice → Info, Info → Info, Debug → Debug)
- `idevicesyslog_line_to_event()` — converts parsed line to `NativeLogEvent` with tag derived from framework field
- `build_idevicesyslog_command()` — builds command with `-u <udid>`, `-p <process_name>`, `-K` (suppress kernel), `--no-colors` flags
- `run_idevicesyslog_capture()` — async capture loop reading stdout line-by-line, parsing, filtering by min level and include/exclude tags, sending events via channel, with graceful shutdown on token cancellation
- `spawn_physical()` — spawns the capture task and returns `Some(NativeLogHandle)` with child process handle and shutdown sender

### Files Modified

- `crates/fdemon-daemon/src/native_logs/ios.rs` — added all physical device capture code (~300 lines) alongside existing simulator capture

### Testing

- 19+ unit tests covering: BSD syslog line parsing (valid lines, malformed input, multiline messages), level mapping for all 8 BSD levels, command construction with various config options, tag derivation from framework field, min level filtering
- All 527 fdemon-daemon tests pass
- `cargo clippy --workspace -- -D warnings` passes

### Acceptance Criteria Met

- [x] `parse_idevicesyslog_line()` correctly parses BSD syslog format
- [x] `bsd_syslog_level_to_log_level()` maps all BSD syslog levels
- [x] `IdevicesyslogLine` struct holds all parsed fields
- [x] `build_idevicesyslog_command()` builds correct command with `-u`, `-p`, `-K`, `--no-colors`
- [x] `run_idevicesyslog_capture()` implements async capture loop with shutdown
- [x] `spawn_physical()` returns `Some(NativeLogHandle)`
- [x] Tag derived from framework field
- [x] Min level filtering applied
- [x] Include/exclude tag filtering applied
- [x] All code gated with `#[cfg(target_os = "macos")]`
