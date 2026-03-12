## Task: macOS Unified Logging Capture Backend

**Objective**: Implement `MacOsLogCapture` to spawn `log stream` with process-name filtering, parse the syslog-style output, map macOS log levels to `LogLevel`, and emit `NativeLogEvent`s. Gate all code behind `#[cfg(target_os = "macos")]`.

**Depends on**: 04-shared-native-infra

### Scope

- `crates/fdemon-daemon/src/native_logs/macos.rs`: Replace stub with full implementation

### Details

#### 1. Spawn `log stream` process

macOS `log stream` captures unified logging output in real-time:

```rust
#[cfg(target_os = "macos")]
fn build_log_stream_command(config: &MacOsLogConfig) -> Command {
    let mut cmd = Command::new("log");
    cmd.arg("stream");

    // Filter by process name
    let predicate = format!("process == \"{}\"", config.process_name);
    cmd.arg("--predicate").arg(&predicate);

    // Set minimum log level
    let level = match config.min_level.to_lowercase().as_str() {
        "verbose" | "debug" => "debug",
        "info" => "info",
        "warning" | "error" => "error", // macOS doesn't have a "warning" level for --level
        _ => "info",
    };
    cmd.arg("--level").arg(level);

    // Use syslog style for more structured output
    cmd.arg("--style").arg("syslog");

    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::null());
    cmd.kill_on_drop(true);

    cmd
}
```

**Note on `--level`**: macOS `log stream --level` accepts `default`, `info`, `debug`. There is no `warning` level — macOS unified logging levels are: Default, Info, Debug, Error, Fault. The `--level` flag sets the minimum: `debug` shows everything, `info` shows Info+Default+Error+Fault, `default` shows Default+Error+Fault.

#### 2. Parse syslog-style output

The `--style syslog` output format:

```
Filtering the log data using "process == "my_app""
Timestamp                       Thread     Type        Activity             PID    TTL
2024-03-10 14:30:00.123456-0700  0x1234     Info        0x0                  5678   0    my_app: (MyPlugin) [com.example.plugin:default] Hello from plugin
2024-03-10 14:30:00.456789-0700  0x1235     Error       0x0                  5678   0    my_app: (Foundation) NSLog message here
```

The first two lines are headers. Parse data lines:

```rust
use regex::Regex;
use std::sync::LazyLock;

/// Regex for macOS `log stream --style syslog` output.
/// Format: timestamp  thread  type  activity  pid  ttl  process: (subsystem) [category] message
/// The subsystem/category parts are optional.
#[cfg(target_os = "macos")]
static SYSLOG_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(\d{4}-\d{2}-\d{2}\s+\d{2}:\d{2}:\d{2}\.\d+[-+]\d{4})\s+\S+\s+(\w+)\s+\S+\s+\d+\s+\d+\s+\S+:\s*(?:\(([^)]*)\))?\s*(?:\[([^\]]*)\])?\s*(.*)$"
    ).expect("syslog regex is valid")
});

#[cfg(target_os = "macos")]
#[derive(Debug, Clone)]
pub struct SyslogLine {
    pub timestamp: String,
    pub level: String,       // "Default", "Info", "Debug", "Error", "Fault"
    pub subsystem: Option<String>,  // e.g., "MyPlugin", "Foundation"
    pub category: Option<String>,   // e.g., "com.example.plugin:default"
    pub message: String,
}

#[cfg(target_os = "macos")]
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
```

**Tag derivation**: Use subsystem or category as the tag when available. Fall back to `"native"` for untagged messages:

```rust
#[cfg(target_os = "macos")]
fn derive_tag(line: &SyslogLine) -> String {
    // Prefer category (e.g., "com.example.plugin:default" → "com.example.plugin")
    if let Some(ref cat) = line.category {
        // Strip the ":category" suffix if present
        if let Some(base) = cat.split(':').next() {
            if !base.is_empty() {
                return base.to_string();
            }
        }
    }
    // Fall back to subsystem (e.g., "MyPlugin", "Foundation")
    if let Some(ref sub) = line.subsystem {
        if !sub.is_empty() {
            return sub.to_string();
        }
    }
    // No tag information available
    "native".to_string()
}
```

#### 3. Convert to `NativeLogEvent`

```rust
#[cfg(target_os = "macos")]
fn syslog_line_to_event(line: &SyslogLine) -> NativeLogEvent {
    let priority = NativeLogPriority::from_macos_level(&line.level)
        .unwrap_or(NativeLogPriority::Info);
    let tag = derive_tag(line);

    NativeLogEvent {
        tag,
        level: priority.to_log_level(),
        message: line.message.clone(),
        timestamp: Some(line.timestamp.clone()),
    }
}
```

#### 4. Implement `NativeLogCapture` trait

```rust
#[cfg(target_os = "macos")]
pub struct MacOsLogCapture {
    config: MacOsLogConfig,
}

#[cfg(target_os = "macos")]
impl MacOsLogCapture {
    pub fn new(config: MacOsLogConfig) -> Self {
        Self { config }
    }
}

#[cfg(target_os = "macos")]
impl NativeLogCapture for MacOsLogCapture {
    fn spawn(&self) -> Option<NativeLogHandle> {
        let config = self.config.clone();
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
```

#### 5. Background task loop

```rust
#[cfg(target_os = "macos")]
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
    let mut header_lines_remaining = 2; // Skip "Filtering..." and column header lines

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
                        // Skip header lines
                        if header_lines_remaining > 0 {
                            header_lines_remaining -= 1;
                            continue;
                        }

                        if let Some(parsed) = parse_syslog_line(&line) {
                            let tag = derive_tag(&parsed);

                            // Apply tag filter
                            if !should_include_tag_macos(&config, &tag) {
                                continue;
                            }

                            let event = syslog_line_to_event(&parsed);
                            if event_tx.send(event).await.is_err() {
                                break; // Receiver dropped
                            }
                        }
                    }
                    Ok(None) => {
                        tracing::debug!("log stream process exited (EOF)");
                        break;
                    }
                    Err(e) => {
                        tracing::warn!("Error reading log stream: {}", e);
                        break;
                    }
                }
            }
        }
    }
}

#[cfg(target_os = "macos")]
fn should_include_tag_macos(config: &MacOsLogConfig, tag: &str) -> bool {
    if !config.include_tags.is_empty() {
        return config.include_tags.iter().any(|t| t.eq_ignore_ascii_case(tag));
    }
    !config.exclude_tags.iter().any(|t| t.eq_ignore_ascii_case(tag))
}
```

#### 6. Ensure all code is `#[cfg(target_os = "macos")]` gated

Every public type, function, and impl block in `macos.rs` must be gated. The module declaration in `mod.rs` should also be gated:

```rust
// In native_logs/mod.rs:
#[cfg(target_os = "macos")]
pub mod macos;
```

### Acceptance Criteria

1. On macOS: `parse_syslog_line()` correctly parses a standard syslog-style line
2. On macOS: Header lines (first 2 lines) are skipped
3. On macOS: `derive_tag()` extracts subsystem/category correctly, falls back to `"native"`
4. On macOS: macOS levels (Default/Info/Debug/Error/Fault) map correctly to `LogLevel`
5. On macOS: Tag filtering respects `exclude_tags` and `include_tags`
6. On macOS: `MacOsLogCapture::spawn()` returns `Some(NativeLogHandle)`
7. On macOS: Shutdown signal stops the capture task
8. On non-macOS: All macOS code is excluded by `#[cfg]` — no compilation impact
9. `cargo check --workspace` compiles on macOS
10. `cargo check --workspace` compiles on Linux (macOS code excluded)

### Testing

```rust
#[cfg(test)]
#[cfg(target_os = "macos")]
mod tests {
    use super::*;

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

        assert_eq!(syslog_line_to_event(&make_line("Debug")).level, LogLevel::Debug);
        assert_eq!(syslog_line_to_event(&make_line("Info")).level, LogLevel::Info);
        assert_eq!(syslog_line_to_event(&make_line("Default")).level, LogLevel::Info);
        assert_eq!(syslog_line_to_event(&make_line("Error")).level, LogLevel::Error);
        assert_eq!(syslog_line_to_event(&make_line("Fault")).level, LogLevel::Error);
    }

    #[test]
    fn test_header_line_does_not_parse() {
        assert!(parse_syslog_line("Filtering the log data using \"process == \\\"my_app\\\"\"").is_none());
        assert!(parse_syslog_line("Timestamp                       Thread     Type        Activity             PID    TTL").is_none());
    }
}
```

### Notes

- **macOS `log stream` requires no special permissions** for user processes. It may require `sudo` for system-level logs, but we only need app-level logs.
- **The syslog format is complex and varies by macOS version.** The regex may need adjustment based on real-world output. Ensure the regex is tested against actual `log stream --style syslog` output from different macOS versions (13+).
- **Header skipping**: The `log stream` command outputs 2 header lines before data. The counter approach is simple but fragile — consider also checking if a line starts with a timestamp as a more robust approach.
- **Process name derivation**: The `process_name` comes from the app layer (task 07). For Flutter macOS apps, this is typically the product name from the Xcode project, which often matches the Flutter project directory name. The app layer resolves this.
- **Unlike Android logcat**, macOS `log stream` doesn't have a `--pid` flag in all versions. Process name filtering via `--predicate` is the standard approach.
- **The `--style syslog` flag** provides more structured output than the default style, making it easier to parse. Alternative: `--style json` exists on newer macOS versions and would be even easier to parse, but `syslog` style is more widely available.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/native_logs/macos.rs` | Replaced stub with full implementation: `SYSLOG_RE` regex, `SyslogLine` struct, `parse_syslog_line()`, `derive_tag()`, `syslog_line_to_event()`, `should_include_tag()`, `build_log_stream_command()`, `run_log_stream_capture()` async task, `MacOsLogCapture` struct + `NativeLogCapture` impl, and 14 unit tests |

### Notable Decisions/Tradeoffs

1. **Entire file under macOS cfg gate**: The module declaration in `mod.rs` already gates the entire file with `#[cfg(target_os = "macos")]`, so individual items within the file don't need explicit `#[cfg]` attributes. This matches the task's requirement while keeping the file cleaner.

2. **`MacOsLogConfig` clone via field-by-field copy**: `MacOsLogConfig` doesn't derive `Clone` (it was not added in task 04). The `spawn()` method clones the config into the async task by constructing a new `MacOsLogConfig` from each field individually. This avoids needing to modify the shared infrastructure.

3. **Named constant for header lines**: `LOG_STREAM_HEADER_LINES: usize = 2` replaces the magic number `2` in the loop, following the code standards requirement for named constants.

4. **`LazyLock` without `#[cfg]` attribute on static**: Since the entire file is cfg-gated, the `SYSLOG_RE` static doesn't need its own `#[cfg]` attribute — it is only compiled on macOS.

5. **Tag derivation duplicated in loop**: `derive_tag()` is called twice in `run_log_stream_capture` (once for tag filtering, once inside `syslog_line_to_event()`). This is a minor inefficiency accepted in exchange for keeping `syslog_line_to_event()` a clean pure function matching the spec.

### Testing Performed

- `cargo check -p fdemon-daemon` — Passed
- `cargo test -p fdemon-daemon -- native_logs::macos` — Passed (14 tests)
- `cargo test -p fdemon-daemon` — Passed (483 tests, 3 ignored)
- `cargo clippy -p fdemon-daemon -- -D warnings` — Passed
- `cargo fmt --all && cargo check --workspace` — Passed

### Risks/Limitations

1. **Syslog format variation**: The regex was derived from the documented format. Real-world macOS log output may vary across macOS versions (13/14/15) or when process names contain special characters. The regex is well-tested against the documented format but not against live `log stream` output.

2. **Header line count**: The counter-based approach skips exactly 2 lines. If `log stream` emits extra diagnostic lines before data (e.g., for permission issues), those would be silently consumed. A more robust approach would be checking that each line starts with a timestamp pattern.

3. **`MacOsLogConfig` lacks `Clone`**: The spawn method copies field-by-field. If `MacOsLogConfig` gains new fields in future tasks, the copy in `spawn()` will cause a compilation error, which is a safe failure mode (won't silently drop fields).
