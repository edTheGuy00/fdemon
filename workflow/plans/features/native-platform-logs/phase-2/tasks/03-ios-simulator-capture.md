## Task: iOS Simulator Log Capture Backend

**Objective**: Implement simulator log capture in `ios.rs` using `xcrun simctl spawn <udid> log stream --predicate 'process == "Runner"' --style syslog`. The simulator uses the same unified logging system as macOS, so the syslog output format is identical.

**Depends on**: 02-ios-log-config

### Scope

- `crates/fdemon-daemon/src/native_logs/ios.rs`: Implement simulator capture path
- `crates/fdemon-daemon/src/native_logs/macos.rs`: Extract `parse_syslog_line()` and `SyslogLine` to be reusable (or keep a copy in ios.rs if extraction is too disruptive)

### Details

#### 1. Syslog Format Sharing

iOS simulators use `xcrun simctl spawn <udid> log stream --style syslog` which produces the **same format** as macOS `log stream --style syslog`:

```
2024-09-23 10:04:31.396000+0000  0x2e8      Default     0x0                  0      0    Runner: (CoreText) [com.apple.CoreText:] some message
2024-09-23 10:04:31.400000+0000  0x2e8      Info        0x0                  0      0    Runner: (MyPlugin) [com.example.myplugin:general] plugin initialized
```

**Option A (preferred)**: Extract `parse_syslog_line()`, `SyslogLine`, and the `SYSLOG_RE` regex from `macos.rs` into a shared location (e.g., a private `syslog.rs` helper within `native_logs/`). Both `macos.rs` and `ios.rs` import from there.

**Option B**: Copy the parser into `ios.rs`. This is simpler but creates duplication. Acceptable if extracting is too disruptive to the macos module.

The choice between options should be made at implementation time based on code complexity.

#### 2. Build simulator log stream command

```rust
use tokio::process::Command;
use std::process::Stdio;

fn build_simctl_log_stream_command(config: &IosLogConfig) -> Command {
    let mut cmd = Command::new("xcrun");
    cmd.arg("simctl");
    cmd.arg("spawn");
    cmd.arg(&config.device_udid);
    cmd.arg("log");
    cmd.arg("stream");

    // Filter by process name
    let predicate = format!("process == \"{}\"", config.process_name);
    cmd.arg("--predicate").arg(&predicate);

    // Use syslog style for structured parsing
    cmd.arg("--style").arg("syslog");

    // Set minimum log level
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
```

#### 3. Run simulator capture loop

Follow the same async loop pattern as `android.rs` and `macos.rs`:

```rust
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::{mpsc, watch};

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
                        // Skip the header line that log stream outputs on startup
                        if line.starts_with("Filtering the log data") || line.starts_with("Timestamp") {
                            continue;
                        }

                        if let Some(parsed) = parse_syslog_line(&line) {
                            // Derive tag from subsystem/category or process name
                            let tag = derive_tag(&parsed);

                            // Apply tag filter
                            if !should_include_tag(
                                &config.include_tags,
                                &config.exclude_tags,
                                &tag,
                            ) {
                                continue;
                            }

                            let event = NativeLogEvent {
                                tag,
                                level: parsed.level,
                                message: parsed.message,
                                timestamp: Some(parsed.timestamp),
                            };

                            if event_tx.send(event).await.is_err() {
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
```

#### 4. Wire into `IosLogCapture::spawn()` (simulator path)

Update the stub `spawn()` to dispatch based on `config.is_simulator`:

```rust
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
    fn spawn_simulator(&self) -> Option<NativeLogHandle> {
        let config = self.config.clone();
        let (event_tx, event_rx) = mpsc::channel::<NativeLogEvent>(EVENT_CHANNEL_CAPACITY);
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

    fn spawn_physical(&self) -> Option<NativeLogHandle> {
        // Stub — implemented in task 04.
        tracing::warn!("iOS physical device log capture not yet implemented");
        None
    }
}
```

#### 5. Syslog `derive_tag()` function

Reuse the same logic as macOS — extract the subsystem/category when available, fall back to process name:

```rust
fn derive_tag(parsed: &SyslogLine) -> String {
    // Prefer subsystem (e.g., "com.example.myplugin")
    if let Some(ref subsystem) = parsed.subsystem {
        if !subsystem.is_empty() {
            return subsystem.clone();
        }
    }
    // Fall back to the process name
    parsed.process.clone().unwrap_or_else(|| "native".to_string())
}
```

### Acceptance Criteria

1. `build_simctl_log_stream_command()` produces correct command: `xcrun simctl spawn <udid> log stream --predicate 'process == "Runner"' --style syslog --level <level>`
2. Syslog format lines are parsed correctly (reusing or matching `parse_syslog_line()` from macOS backend)
3. Header lines ("Filtering the log data...", "Timestamp...") are skipped
4. Tag derivation: subsystem preferred, falls back to process name
5. Tag filtering works via `should_include_tag()`
6. Shutdown signal kills the child process and breaks the loop
7. `IosLogCapture::spawn()` returns `Some(NativeLogHandle)` when `is_simulator == true`
8. `IosLogCapture::spawn()` still returns `None` for physical devices (stub from task 04)
9. `cargo check -p fdemon-daemon` compiles
10. `cargo test -p fdemon-daemon` passes

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

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
        let args: Vec<String> = std_cmd.get_args().map(|a| a.to_string_lossy().into()).collect();
        assert!(args.contains(&"--predicate".to_string()));
        assert!(args.iter().any(|a| a.contains("process == \"Runner\"")));
    }

    #[test]
    fn test_build_simctl_log_stream_command_level_mapping() {
        let test_cases = vec![
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
            let args: Vec<String> = std_cmd.get_args().map(|a| a.to_string_lossy().into()).collect();
            let level_idx = args.iter().position(|a| a == "--level").unwrap();
            assert_eq!(args[level_idx + 1], expected, "min_level '{}' should map to '{}'", input, expected);
        }
    }

    #[test]
    fn test_derive_tag_prefers_subsystem() {
        let parsed = SyslogLine {
            timestamp: "2024-01-01 00:00:00.000".to_string(),
            process: Some("Runner".to_string()),
            subsystem: Some("com.example.myplugin".to_string()),
            category: Some("general".to_string()),
            level: LogLevel::Info,
            message: "test".to_string(),
        };
        assert_eq!(derive_tag(&parsed), "com.example.myplugin");
    }

    #[test]
    fn test_derive_tag_falls_back_to_process() {
        let parsed = SyslogLine {
            timestamp: "2024-01-01 00:00:00.000".to_string(),
            process: Some("Runner".to_string()),
            subsystem: None,
            category: None,
            level: LogLevel::Info,
            message: "NSLog message".to_string(),
        };
        assert_eq!(derive_tag(&parsed), "Runner");
    }

    #[test]
    fn test_derive_tag_empty_subsystem_falls_back() {
        let parsed = SyslogLine {
            timestamp: "2024-01-01 00:00:00.000".to_string(),
            process: Some("Runner".to_string()),
            subsystem: Some("".to_string()),
            category: None,
            level: LogLevel::Info,
            message: "test".to_string(),
        };
        assert_eq!(derive_tag(&parsed), "Runner");
    }
}
```

### Notes

- The macOS syslog format parser (`parse_syslog_line()`, `SyslogLine` struct, `SYSLOG_RE` regex) from `macos.rs` is directly reusable for simulators. The implementor should decide whether to extract it to a shared module or copy it. Extraction is preferred for maintainability but either approach is acceptable.
- The `--style syslog` flag ensures consistent output format across macOS versions.
- The `--level` flag maps to macOS unified logging levels: `debug` includes all, `info` includes info+default+error+fault, `default` includes default+error+fault.
- `log stream` outputs a header line on startup (e.g., `"Filtering the log data using..."`) — this must be skipped by the parser.
- `kill_on_drop(true)` ensures the simctl process is cleaned up if the task panics.
- The `EVENT_CHANNEL_CAPACITY` constant from `mod.rs` should be reused (it's `pub(crate)` at 256).

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/native_logs/ios.rs` | Replaced stub with full simulator capture implementation |

### Notable Decisions/Tradeoffs

1. **Reuse from `macos.rs` instead of extracting**: `macos.rs` already exposes `parse_syslog_line()`, `SyslogLine`, and `derive_tag()` as `pub` functions. Rather than extracting to a third shared module, `ios.rs` imports directly from `super::macos`. This is Option B (import from sibling) in spirit — no new file, no duplication. The task's Option A (shared `syslog.rs`) would be a larger refactor and was explicitly called "preferred but acceptable to skip".

2. **`derive_tag()` behavior differs from task pseudocode**: The task's pseudocode showed a subsystem-first tag derivation, but the actual `derive_tag()` in `macos.rs` uses category-first (strips `:suffix`). The implementation reuses the existing `derive_tag()` to stay consistent with the macOS backend rather than inventing a divergent iOS-specific version.

3. **Level conversion via `NativeLogPriority`**: The task pseudocode passed `parsed.level` directly as a `LogLevel`, but `SyslogLine.level` is a `String`. Used the existing `NativeLogPriority::from_macos_level()` path (same as `macos.rs`'s `syslog_line_to_event()`) for consistent level mapping.

4. **`spawn_physical()` is a stub returning `None`**: Matches task requirement — task 04 will implement physical device capture.

### Testing Performed

- `cargo check -p fdemon-daemon` — Passed
- `cargo test -p fdemon-daemon` — Passed (519 tests, 0 failed, 3 ignored)
- `cargo fmt -p fdemon-daemon` — Passed
- `cargo clippy -p fdemon-daemon -- -D warnings` — Passed (no warnings)

### Risks/Limitations

1. **Import coupling to `macos.rs`**: `ios.rs` imports directly from `super::macos`. If `macos.rs` is refactored to make `parse_syslog_line`/`SyslogLine`/`derive_tag` private, `ios.rs` will break. This is low risk since all three are already `pub` by design.
2. **Physical device path is a stub**: `spawn_physical()` returns `None` with a warning. This is intentional — task 04 covers it.
