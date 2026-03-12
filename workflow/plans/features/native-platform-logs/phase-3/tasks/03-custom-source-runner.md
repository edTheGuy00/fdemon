## Task: Generic Custom Source Process Runner

**Objective**: Implement `CustomLogCapture` struct that implements the `NativeLogCapture` trait, spawning user-defined commands and parsing their stdout through the configured format parser.

**Depends on**: 01-custom-source-config, 02-format-parsers

### Scope

- `crates/fdemon-daemon/src/native_logs/custom.rs` — **NEW** file
- `crates/fdemon-daemon/src/native_logs/mod.rs` — Add `pub mod custom;`, add `"custom"` to `create_native_log_capture` dispatch (or a new factory function)

### Details

```rust
use super::{NativeLogCapture, NativeLogHandle, NativeLogEvent, EVENT_CHANNEL_CAPACITY};
use super::formats::{parse_line, OutputFormat};
use tokio::process::Command;
use tokio::sync::{mpsc, watch};
use std::collections::HashMap;

pub struct CustomSourceConfig {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub format: OutputFormat,
    pub working_dir: Option<String>,
    pub env: HashMap<String, String>,
    pub exclude_tags: Vec<String>,
    pub include_tags: Vec<String>,
}

pub struct CustomLogCapture {
    config: CustomSourceConfig,
}

impl CustomLogCapture {
    pub fn new(config: CustomSourceConfig) -> Self {
        Self { config }
    }
}

impl NativeLogCapture for CustomLogCapture {
    fn spawn(&self) -> Option<NativeLogHandle> {
        let (event_tx, event_rx) = mpsc::channel(EVENT_CHANNEL_CAPACITY);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        let config = self.config.clone();

        let task_handle = tokio::spawn(async move {
            run_custom_capture(config, event_tx, shutdown_rx).await;
        });

        Some(NativeLogHandle {
            event_rx,
            shutdown_tx,
            task_handle,
        })
    }
}
```

#### Process Runner

```rust
async fn run_custom_capture(
    config: CustomSourceConfig,
    event_tx: mpsc::Sender<NativeLogEvent>,
    mut shutdown_rx: watch::Receiver<bool>,
) {
    let mut cmd = Command::new(&config.command);
    cmd.args(&config.args);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    // Set working directory if specified
    if let Some(ref dir) = config.working_dir {
        cmd.current_dir(dir);
    }

    // Set environment variables
    for (key, value) in &config.env {
        cmd.env(key, value);
    }

    // Spawn the process
    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(e) => {
            tracing::warn!(
                "Failed to spawn custom log source '{}' (command: '{}'): {}",
                config.name, config.command, e
            );
            return;
        }
    };

    tracing::debug!(
        "Custom log source '{}' started (pid: {:?})",
        config.name,
        child.id()
    );

    let stdout = child.stdout.take().expect("stdout was piped");
    let mut reader = tokio::io::BufReader::new(stdout).lines();

    loop {
        tokio::select! {
            _ = shutdown_rx.changed() => {
                if *shutdown_rx.borrow() {
                    tracing::debug!("Custom log source '{}' shutting down", config.name);
                    let _ = child.kill().await;
                    break;
                }
            }
            line = reader.next_line() => {
                match line {
                    Ok(Some(line)) => {
                        if let Some(event) = parse_line(&config.format, &line, &config.name) {
                            // Apply tag filtering (reuse should_include_tag from mod.rs)
                            if !super::should_include_tag(
                                &event.tag,
                                &config.exclude_tags,
                                &config.include_tags,
                            ) {
                                continue;
                            }
                            if event_tx.send(event).await.is_err() {
                                break; // receiver dropped
                            }
                        }
                    }
                    Ok(None) => {
                        // Process stdout closed — process exited
                        let status = child.wait().await;
                        tracing::warn!(
                            "Custom log source '{}' exited (status: {:?})",
                            config.name,
                            status.map(|s| s.code())
                        );
                        break;
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Error reading from custom log source '{}': {}",
                            config.name, e
                        );
                        break;
                    }
                }
            }
        }
    }
}
```

### Key Design Decisions

1. **No shell expansion**: `Command::new()` with explicit args — never `sh -c`. This avoids injection risks and makes behavior predictable across platforms.
2. **No auto-restart**: If the process exits, log a warning and stop. Users must fix their command. This avoids runaway process spawning from misconfigured commands.
3. **stderr captured but not parsed**: stderr is piped to avoid orphaned pipe errors, but its output is not parsed as log events. Could be logged at debug level for troubleshooting.
4. **Tag filtering applied in the capture loop**: Reuse `should_include_tag()` from `mod.rs` with the global `exclude_tags`/`include_tags`. This ensures custom source output respects the same filtering rules.

### Factory Function

Add a factory function or extend the existing `create_native_log_capture`:

```rust
// In mod.rs or custom.rs
pub fn create_custom_log_capture(config: CustomSourceConfig) -> Box<dyn NativeLogCapture> {
    Box::new(CustomLogCapture::new(config))
}
```

This is separate from `create_native_log_capture` (which dispatches by platform) because custom sources are user-defined and there can be multiple per session.

### Acceptance Criteria

1. `CustomLogCapture` implements `NativeLogCapture` trait
2. Spawns the configured command with correct args, env, and working_dir
3. Reads stdout line-by-line and parses through the configured format
4. Applies `should_include_tag()` filtering
5. Shutdown signal (`watch::Sender<bool>`) kills the child process
6. Process exit logged as warning with exit code
7. Spawn failure logged as warning (no panic, no error propagation)
8. `NativeLogHandle` returned with event_rx, shutdown_tx, task_handle

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_custom_capture_with_echo_command() {
        // Spawn: echo "hello\nworld"
        // Verify: 2 NativeLogEvent received with format=Raw
    }

    #[tokio::test]
    async fn test_custom_capture_shutdown() {
        // Spawn: yes (infinite output)
        // Send shutdown signal
        // Verify: process killed, task completes
    }

    #[tokio::test]
    async fn test_custom_capture_invalid_command() {
        // Spawn: /nonexistent/command
        // Verify: spawn returns Some (handle created) but no events, warning logged
    }

    #[tokio::test]
    async fn test_custom_capture_process_exit() {
        // Spawn: echo "done" (exits after one line)
        // Verify: event received, then handle task completes
    }

    #[tokio::test]
    async fn test_custom_capture_with_env() {
        // Spawn: printenv MY_VAR
        // Verify: event contains the env value
    }

    #[tokio::test]
    async fn test_custom_capture_tag_filtering() {
        // Spawn with json format, exclude_tags = ["filtered"]
        // Send JSON with tag="filtered" and tag="allowed"
        // Verify: only "allowed" events pass through
    }
}
```

### Notes

- `tokio::io::BufReader::lines()` requires `use tokio::io::AsyncBufReadExt;`
- The existing platform captures (Android, macOS, iOS) use a similar pattern — refer to `android.rs::run_logcat_capture` for the established loop structure
- Consider whether `CustomSourceConfig` in the daemon should be a separate type from the config type in `fdemon-app`, or whether to reuse the same type. If layers need to stay separate, the daemon config type should be a simpler struct without serde derives, constructed from the app config type. Check the pattern used by `AndroidLogConfig`/`MacOsLogConfig`/`IosLogConfig`.
