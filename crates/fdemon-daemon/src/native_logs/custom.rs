//! Generic custom source process runner for native log capture.
//!
//! Implements [`NativeLogCapture`] by spawning a user-defined command and
//! parsing its stdout through the configured [`OutputFormat`] parser.
//!
//! ## Design decisions
//!
//! - **No shell expansion**: Uses [`tokio::process::Command::new`] with explicit
//!   args — never `sh -c`. This avoids injection risks and makes behavior
//!   predictable across platforms.
//! - **No auto-restart**: If the process exits, a warning is logged and the
//!   capture stops. Users must fix their command configuration. This avoids
//!   runaway process spawning from misconfigured commands.
//! - **stderr captured but not parsed**: stderr is piped to avoid orphaned pipe
//!   errors, but its output is not forwarded as log events.
//! - **Tag filtering**: Reuses [`super::should_include_tag`] with the configured
//!   `include_tags`/`exclude_tags` lists.

use std::collections::HashMap;

use fdemon_core::OutputFormat;
use tokio::io::AsyncBufReadExt;
use tokio::process::Command;
use tokio::sync::{mpsc, oneshot, watch};

use super::formats::parse_line;
use super::{NativeLogCapture, NativeLogEvent, NativeLogHandle, EVENT_CHANNEL_CAPACITY};

/// Configuration for a generic custom log source process.
///
/// This is a daemon-layer type, separate from the app-layer config type in
/// `fdemon-app`. It follows the same pattern as [`super::AndroidLogConfig`],
/// [`super::MacOsLogConfig`], and [`super::IosLogConfig`]: a plain struct
/// without serde derives, constructed by the app layer from its own config.
#[derive(Clone)]
pub struct CustomSourceConfig {
    /// Human-readable name for this source (used as log tag fallback and in
    /// diagnostic messages).
    pub name: String,
    /// The command to execute (absolute path or name resolved via `$PATH`).
    /// Never passed through a shell.
    pub command: String,
    /// Arguments to pass to the command.
    pub args: Vec<String>,
    /// Output format used to parse each line of stdout.
    pub format: OutputFormat,
    /// Optional working directory for the spawned process.
    pub working_dir: Option<String>,
    /// Additional environment variables to set for the spawned process.
    pub env: HashMap<String, String>,
    /// Tags to exclude from output. Ignored when `include_tags` is non-empty.
    pub exclude_tags: Vec<String>,
    /// If non-empty, only events with these tags are forwarded (whitelist mode).
    pub include_tags: Vec<String>,
    /// Optional regex pattern to match against stdout lines for readiness signaling.
    /// When set, each stdout line is checked against this pattern. On first match,
    /// the `ready_tx` sender (passed to [`CustomLogCapture::spawn_with_readiness`])
    /// is fired and pattern matching stops.
    pub ready_pattern: Option<String>,
}

/// Custom log capture backend.
///
/// Spawns a user-defined command and parses its stdout through the configured
/// [`OutputFormat`] parser, forwarding matching events as [`NativeLogEvent`]s.
pub struct CustomLogCapture {
    config: CustomSourceConfig,
}

impl CustomLogCapture {
    /// Create a new custom log capture backend with the given configuration.
    pub fn new(config: CustomSourceConfig) -> Self {
        Self { config }
    }

    /// Spawn with an optional readiness signal sender.
    ///
    /// If `ready_tx` is provided alongside a `ready_pattern` in the config,
    /// the capture loop will fire `ready_tx` when the pattern first matches
    /// a stdout line.
    ///
    /// If the process exits before the pattern matches, `ready_tx` is dropped,
    /// causing the receiver to get a `RecvError` — the app layer interprets
    /// this as "ready check failed".
    pub fn spawn_with_readiness(
        &self,
        ready_tx: Option<oneshot::Sender<()>>,
    ) -> Option<NativeLogHandle> {
        let (event_tx, event_rx) = mpsc::channel::<NativeLogEvent>(EVENT_CHANNEL_CAPACITY);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        let config = self.config.clone();

        let task_handle = tokio::spawn(async move {
            run_custom_capture(config, event_tx, shutdown_rx, ready_tx).await;
        });

        Some(NativeLogHandle {
            event_rx,
            shutdown_tx,
            task_handle,
        })
    }
}

impl NativeLogCapture for CustomLogCapture {
    fn spawn(&self) -> Option<NativeLogHandle> {
        self.spawn_with_readiness(None)
    }
}

/// Create a boxed [`CustomLogCapture`] from a [`CustomSourceConfig`].
///
/// This is separate from [`super::create_native_log_capture`] (which dispatches
/// by platform) because custom sources are user-defined and there can be multiple
/// per session.
pub fn create_custom_log_capture(config: CustomSourceConfig) -> Box<dyn NativeLogCapture> {
    Box::new(CustomLogCapture::new(config))
}

/// Background capture loop: spawns the process, reads stdout line-by-line,
/// parses each line through the configured format, applies tag filtering, and
/// emits events.
///
/// If `ready_tx` is `Some` and `config.ready_pattern` is set, each line is
/// checked against the compiled pattern. On first match, `ready_tx` is fired
/// and pattern matching stops. If the process exits before a match, `ready_tx`
/// is dropped, signaling failure to the receiver.
async fn run_custom_capture(
    config: CustomSourceConfig,
    event_tx: mpsc::Sender<NativeLogEvent>,
    mut shutdown_rx: watch::Receiver<bool>,
    mut ready_tx: Option<oneshot::Sender<()>>,
) {
    // Compile ready pattern if provided. An invalid pattern logs a warning and
    // drops ready_tx immediately (receiver will get RecvError, treated as failure).
    let mut ready_regex = config.ready_pattern.as_ref().and_then(|p| {
        match regex::Regex::new(p) {
            Ok(r) => Some(r),
            Err(e) => {
                tracing::warn!(
                    "Custom source '{}': invalid ready pattern '{}': {}",
                    config.name,
                    p,
                    e
                );
                // Dropping ready_tx signals failure to the receiver.
                drop(ready_tx.take());
                None
            }
        }
    });

    let mut cmd = Command::new(&config.command);
    cmd.args(&config.args);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    // Ensure the child is killed if the task is dropped.
    cmd.kill_on_drop(true);

    // Set working directory if specified.
    if let Some(ref dir) = config.working_dir {
        cmd.current_dir(dir);
    }

    // Set environment variables.
    for (key, value) in &config.env {
        cmd.env(key, value);
    }

    // Spawn the process — failure is a warning, not a panic.
    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(e) => {
            tracing::warn!(
                "Failed to spawn custom log source '{}' (command: '{}'): {}",
                config.name,
                config.command,
                e
            );
            return;
        }
    };

    tracing::debug!(
        "Custom log source '{}' started (pid: {:?})",
        config.name,
        child.id()
    );

    let stdout = match child.stdout.take() {
        Some(s) => s,
        None => {
            tracing::warn!("Custom log source '{}' stdout not available", config.name);
            return;
        }
    };

    let mut reader = tokio::io::BufReader::new(stdout).lines();

    loop {
        tokio::select! {
            biased;

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
                        // Check stdout readiness pattern BEFORE log parsing.
                        // Both ready_regex and ready_tx must be Some for a check to occur.
                        if let (Some(re), Some(_)) = (&ready_regex, &ready_tx) {
                            if re.is_match(&line) {
                                if let Some(tx) = ready_tx.take() {
                                    let _ = tx.send(());
                                }
                                // Clear regex — no further matching needed after first match.
                                ready_regex = None;
                                tracing::debug!(
                                    "Custom source '{}': stdout ready pattern matched",
                                    config.name
                                );
                            }
                        }

                        if let Some(event) = parse_line(&config.format, &line, &config.name) {
                            // Apply tag filtering.
                            if !super::should_include_tag(
                                &config.include_tags,
                                &config.exclude_tags,
                                &event.tag,
                            ) {
                                continue;
                            }
                            if event_tx.send(event).await.is_err() {
                                // Receiver dropped — stop silently.
                                break;
                            }
                        }
                    }
                    Ok(None) => {
                        // EOF — process stdout closed (process exited).
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
                            config.name,
                            e
                        );
                        break;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{timeout, Duration};

    fn make_config(command: &str, args: Vec<&str>, format: OutputFormat) -> CustomSourceConfig {
        CustomSourceConfig {
            name: "test-source".to_string(),
            command: command.to_string(),
            args: args.into_iter().map(|s| s.to_string()).collect(),
            format,
            working_dir: None,
            env: HashMap::new(),
            exclude_tags: vec![],
            include_tags: vec![],
            ready_pattern: None,
        }
    }

    // ── Basic functionality ────────────────────────────────────────────────

    #[tokio::test]
    async fn test_custom_capture_with_echo_command() {
        // `printf` outputs "hello\nworld\n" without a trailing extra newline issue.
        let config = make_config("printf", vec!["hello\\nworld\\n"], OutputFormat::Raw);
        let capture = CustomLogCapture::new(config);
        let handle = capture.spawn().expect("handle should be Some");

        let mut event_rx = handle.event_rx;
        let mut events = Vec::new();

        // Collect up to 2 events with a timeout.
        for _ in 0..2 {
            match timeout(Duration::from_secs(2), event_rx.recv()).await {
                Ok(Some(event)) => events.push(event),
                Ok(None) => break,
                Err(_) => break,
            }
        }

        // We should have received at least 1 event (printf output).
        // `printf "hello\nworld\n"` may differ by platform; accept 1 or 2.
        assert!(!events.is_empty(), "expected at least 1 log event");
        assert_eq!(events[0].tag, "test-source");
    }

    #[tokio::test]
    async fn test_custom_capture_process_exit() {
        // `echo` outputs one line and exits immediately.
        let config = make_config("echo", vec!["done"], OutputFormat::Raw);
        let capture = CustomLogCapture::new(config);
        let mut handle = capture.spawn().expect("handle should be Some");

        // Receive the single event.
        let event = timeout(Duration::from_secs(2), handle.event_rx.recv())
            .await
            .expect("timed out waiting for event")
            .expect("channel closed before event");

        assert_eq!(event.message, "done");
        assert_eq!(event.tag, "test-source");

        // After process exits the task should complete.
        let _ = timeout(Duration::from_secs(2), handle.task_handle)
            .await
            .expect("task did not complete after process exit");
    }

    #[tokio::test]
    async fn test_custom_capture_shutdown() {
        // `yes` produces infinite output — we must shut it down.
        let config = make_config("yes", vec![], OutputFormat::Raw);
        let capture = CustomLogCapture::new(config);
        let handle = capture.spawn().expect("handle should be Some");

        // Receive at least one event to confirm it's running.
        let mut event_rx = handle.event_rx;
        let first = timeout(Duration::from_secs(2), event_rx.recv())
            .await
            .expect("timed out waiting for first event");
        assert!(first.is_some());

        // Send the shutdown signal.
        handle
            .shutdown_tx
            .send(true)
            .expect("shutdown_tx should not be closed");

        // The background task should complete within a reasonable timeout.
        let _ = timeout(Duration::from_secs(3), handle.task_handle)
            .await
            .expect("task did not complete after shutdown signal");
    }

    #[tokio::test]
    async fn test_custom_capture_invalid_command() {
        // A non-existent command: spawn() returns Some (handle created) but the
        // background task logs a warning and exits without emitting any events.
        let config = make_config("/nonexistent/binary/command_xyz", vec![], OutputFormat::Raw);
        let capture = CustomLogCapture::new(config);
        let handle = capture.spawn();

        // spawn() must always return Some — failure is internal to the task.
        assert!(
            handle.is_some(),
            "spawn() must return Some even for invalid commands"
        );

        let handle = handle.unwrap();

        // The background task should exit quickly (spawn failure).
        let _ = timeout(Duration::from_secs(2), handle.task_handle)
            .await
            .expect("task did not complete after spawn failure");

        // No events should have been emitted.
        let mut event_rx = handle.event_rx;
        assert!(
            event_rx.try_recv().is_err(),
            "no events should be emitted for an invalid command"
        );
    }

    #[tokio::test]
    async fn test_custom_capture_with_env() {
        // `printenv MY_VAR` prints the value of MY_VAR.
        let mut env = HashMap::new();
        env.insert("MY_CUSTOM_VAR_XYZ".to_string(), "hello_env".to_string());

        let config = CustomSourceConfig {
            name: "env-test".to_string(),
            command: "printenv".to_string(),
            args: vec!["MY_CUSTOM_VAR_XYZ".to_string()],
            format: OutputFormat::Raw,
            working_dir: None,
            env,
            exclude_tags: vec![],
            include_tags: vec![],
            ready_pattern: None,
        };

        let capture = CustomLogCapture::new(config);
        let mut handle = capture.spawn().expect("handle should be Some");

        let event = timeout(Duration::from_secs(2), handle.event_rx.recv())
            .await
            .expect("timed out")
            .expect("channel closed");

        assert_eq!(event.message, "hello_env");
        assert_eq!(event.tag, "env-test");
    }

    #[tokio::test]
    async fn test_custom_capture_tag_filtering_exclude() {
        // Use JSON format. Output two lines: one with tag "filtered", one "allowed".
        // The shell approach is avoided — we use a script file or printf with JSON.
        // Since we cannot use `sh -c`, we use `printf` with newline-separated JSON.
        let json_line1 = r#"{"tag":"filtered","message":"should not appear"}"#;
        let json_line2 = r#"{"tag":"allowed","message":"should appear"}"#;
        let combined = format!("{}\\n{}", json_line1, json_line2);

        let config = CustomSourceConfig {
            name: "filter-test".to_string(),
            command: "printf".to_string(),
            args: vec![combined],
            format: OutputFormat::Json,
            working_dir: None,
            env: HashMap::new(),
            exclude_tags: vec!["filtered".to_string()],
            include_tags: vec![],
            ready_pattern: None,
        };

        let capture = CustomLogCapture::new(config);
        let mut handle = capture.spawn().expect("handle should be Some");

        let mut events = Vec::new();
        // Collect all events with a short timeout.
        loop {
            match timeout(Duration::from_millis(500), handle.event_rx.recv()).await {
                Ok(Some(event)) => events.push(event),
                Ok(None) | Err(_) => break,
            }
        }

        // Only the "allowed" tag should pass.
        assert!(
            events.iter().all(|e| e.tag != "filtered"),
            "filtered tag should not appear in events"
        );
        assert!(
            events.iter().any(|e| e.tag == "allowed"),
            "allowed tag should appear in events"
        );
    }

    #[tokio::test]
    async fn test_custom_capture_tag_filtering_include() {
        // Only "allowed" tag in include list; "other" should be dropped.
        let json_line1 = r#"{"tag":"allowed","message":"pass"}"#;
        let json_line2 = r#"{"tag":"other","message":"blocked"}"#;
        let combined = format!("{}\\n{}", json_line1, json_line2);

        let config = CustomSourceConfig {
            name: "include-test".to_string(),
            command: "printf".to_string(),
            args: vec![combined],
            format: OutputFormat::Json,
            working_dir: None,
            env: HashMap::new(),
            exclude_tags: vec![],
            include_tags: vec!["allowed".to_string()],
            ready_pattern: None,
        };

        let capture = CustomLogCapture::new(config);
        let mut handle = capture.spawn().expect("handle should be Some");

        let mut events = Vec::new();
        loop {
            match timeout(Duration::from_millis(500), handle.event_rx.recv()).await {
                Ok(Some(event)) => events.push(event),
                Ok(None) | Err(_) => break,
            }
        }

        assert!(
            events.iter().all(|e| e.tag != "other"),
            "non-included tag should not appear in events"
        );
    }

    #[tokio::test]
    async fn test_custom_capture_working_dir() {
        // `pwd` prints the current working directory; verify it matches working_dir.
        let config = CustomSourceConfig {
            name: "pwd-test".to_string(),
            command: "pwd".to_string(),
            args: vec![],
            format: OutputFormat::Raw,
            working_dir: Some("/tmp".to_string()),
            env: HashMap::new(),
            exclude_tags: vec![],
            include_tags: vec![],
            ready_pattern: None,
        };

        let capture = CustomLogCapture::new(config);
        let mut handle = capture.spawn().expect("handle should be Some");

        let event = timeout(Duration::from_secs(2), handle.event_rx.recv())
            .await
            .expect("timed out")
            .expect("channel closed");

        // On macOS /tmp is a symlink to /private/tmp — accept either.
        assert!(
            event.message == "/tmp" || event.message.starts_with("/private/tmp"),
            "expected /tmp or /private/tmp, got: {}",
            event.message
        );
    }

    // ── Factory function ───────────────────────────────────────────────────

    #[tokio::test]
    async fn test_create_custom_log_capture_returns_box() {
        let config = make_config("echo", vec!["test"], OutputFormat::Raw);
        let capture = create_custom_log_capture(config);
        // Verify spawn() returns Some — actual output is tested elsewhere.
        let handle = capture.spawn();
        assert!(
            handle.is_some(),
            "factory-created capture should return a handle"
        );
    }

    // ── Edge cases (Phase 3 Task 05) ──────────────────────────────────────

    /// stderr output must not produce events — only stdout is parsed.
    #[cfg(unix)]
    #[tokio::test]
    async fn test_custom_capture_stderr_does_not_produce_events() {
        // `sh -c` would violate the no-shell rule; instead use a command that
        // writes exclusively to stderr.  `printf` writes to stdout, but we can
        // rely on the well-known `ls` behaviour: listing a nonexistent path
        // writes its error to stderr and exits with a nonzero code, producing
        // no stdout.  On all Unix platforms this is guaranteed.
        let config = CustomSourceConfig {
            name: "stderr-test".to_string(),
            command: "ls".to_string(),
            // A path guaranteed not to exist: UUID-like name under /nonexistent.
            args: vec!["/nonexistent/__fdemon_test_xyz_does_not_exist__".to_string()],
            format: OutputFormat::Raw,
            working_dir: None,
            env: HashMap::new(),
            exclude_tags: vec![],
            include_tags: vec![],
            ready_pattern: None,
        };

        let capture = CustomLogCapture::new(config);
        let handle = capture.spawn().expect("handle should be Some");

        let mut event_rx = handle.event_rx;

        // The process exits quickly; wait up to 2 s for the task to finish.
        let _ = timeout(Duration::from_secs(2), handle.task_handle).await;

        // No events should have been produced from stderr output.
        assert!(
            event_rx.try_recv().is_err(),
            "stderr output must not produce log events"
        );
    }

    /// Spawning a capture and immediately sending shutdown must not panic or
    /// deadlock regardless of process startup timing.
    #[cfg(unix)]
    #[tokio::test]
    async fn test_custom_capture_concurrent_shutdown() {
        // Use `yes` to produce infinite output so the process is definitely
        // running when the shutdown signal arrives.
        let config = make_config("yes", vec![], OutputFormat::Raw);
        let capture = CustomLogCapture::new(config);
        let handle = capture.spawn().expect("handle should be Some");

        // Send shutdown immediately without waiting for any events.
        handle
            .shutdown_tx
            .send(true)
            .expect("shutdown_tx should not be closed");

        // The background task must complete cleanly within a reasonable timeout.
        let result = timeout(Duration::from_secs(5), handle.task_handle).await;
        assert!(
            result.is_ok(),
            "task should complete after concurrent shutdown signal"
        );
    }

    // ── Stdout readiness signaling ─────────────────────────────────────────

    /// When the ready_pattern matches a stdout line, ready_tx is fired.
    #[tokio::test]
    async fn test_stdout_ready_pattern_fires_on_match() {
        let config = CustomSourceConfig {
            name: "ready-test".to_string(),
            command: "printf".to_string(),
            args: vec!["starting\\nServer ready on port 8080\\nhandling requests\\n".to_string()],
            format: OutputFormat::Raw,
            working_dir: None,
            env: HashMap::new(),
            exclude_tags: vec![],
            include_tags: vec![],
            ready_pattern: Some("Server ready".to_string()),
        };

        let capture = CustomLogCapture::new(config);
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let _handle = capture.spawn_with_readiness(Some(ready_tx));

        // ready_rx should resolve (not timeout)
        let result = timeout(Duration::from_secs(2), ready_rx).await;
        assert!(result.is_ok(), "ready signal should fire on pattern match");
        assert!(
            result.unwrap().is_ok(),
            "ready_rx should receive Ok(()) on pattern match"
        );
    }

    /// When the process exits before the pattern matches, ready_tx is dropped
    /// and the receiver gets a RecvError.
    #[tokio::test]
    async fn test_stdout_ready_pattern_no_match_drops_tx() {
        let config = CustomSourceConfig {
            name: "no-match-test".to_string(),
            command: "echo".to_string(),
            args: vec!["no match here".to_string()],
            format: OutputFormat::Raw,
            working_dir: None,
            env: HashMap::new(),
            exclude_tags: vec![],
            include_tags: vec![],
            ready_pattern: Some("will not match".to_string()),
        };

        let capture = CustomLogCapture::new(config);
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let _handle = capture.spawn_with_readiness(Some(ready_tx));

        // Process exits quickly without matching — ready_rx should error.
        let result = timeout(Duration::from_secs(2), ready_rx).await;
        assert!(
            matches!(result, Ok(Err(_))),
            "ready_rx should get RecvError when process exits without matching"
        );
    }

    /// When no ready_pattern is set, ready_tx is dropped when the process exits.
    #[tokio::test]
    async fn test_stdout_ready_pattern_none_no_signal() {
        let config = CustomSourceConfig {
            name: "no-pattern".to_string(),
            command: "echo".to_string(),
            args: vec!["hello".to_string()],
            format: OutputFormat::Raw,
            working_dir: None,
            env: HashMap::new(),
            exclude_tags: vec![],
            include_tags: vec![],
            ready_pattern: None,
        };

        let capture = CustomLogCapture::new(config);
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let _handle = capture.spawn_with_readiness(Some(ready_tx));

        // No pattern to match — ready_tx dropped when process exits.
        let result = timeout(Duration::from_secs(2), ready_rx).await;
        assert!(
            matches!(result, Ok(Err(_))),
            "ready_rx should get RecvError when no pattern is set"
        );
    }

    /// Log events continue flowing normally during and after pattern matching.
    #[tokio::test]
    async fn test_stdout_ready_logs_still_flow_after_match() {
        let config = CustomSourceConfig {
            name: "logs-after-ready".to_string(),
            command: "printf".to_string(),
            args: vec!["ready\\nlog after ready\\n".to_string()],
            format: OutputFormat::Raw,
            working_dir: None,
            env: HashMap::new(),
            exclude_tags: vec![],
            include_tags: vec![],
            ready_pattern: Some("ready".to_string()),
        };

        let capture = CustomLogCapture::new(config);
        let (ready_tx, _ready_rx) = tokio::sync::oneshot::channel();
        let handle = capture.spawn_with_readiness(Some(ready_tx)).unwrap();

        let mut event_rx = handle.event_rx;
        let mut events = Vec::new();
        loop {
            match timeout(Duration::from_millis(500), event_rx.recv()).await {
                Ok(Some(event)) => events.push(event),
                _ => break,
            }
        }

        // Both lines should appear as log events (pattern matching doesn't consume lines).
        assert!(
            events.len() >= 2,
            "both lines should be forwarded as log events, got: {}",
            events.len()
        );
    }

    /// spawn_with_readiness(None) behaves identically to spawn().
    #[tokio::test]
    async fn test_spawn_with_readiness_none_behaves_like_spawn() {
        let config = make_config("echo", vec!["hello"], OutputFormat::Raw);
        let capture = CustomLogCapture::new(config);
        let handle = capture
            .spawn_with_readiness(None)
            .expect("handle should be Some");

        let mut event_rx = handle.event_rx;
        let event = timeout(Duration::from_secs(2), event_rx.recv())
            .await
            .expect("timed out")
            .expect("channel closed");

        assert_eq!(event.message, "hello");
    }

    /// Invalid regex in ready_pattern drops ready_tx immediately (before process runs).
    #[tokio::test]
    async fn test_stdout_ready_invalid_regex_drops_tx() {
        let config = CustomSourceConfig {
            name: "invalid-regex-test".to_string(),
            command: "echo".to_string(),
            args: vec!["hello".to_string()],
            format: OutputFormat::Raw,
            working_dir: None,
            env: HashMap::new(),
            exclude_tags: vec![],
            include_tags: vec![],
            // Intentionally invalid regex pattern.
            ready_pattern: Some("[invalid regex".to_string()),
        };

        let capture = CustomLogCapture::new(config);
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let _handle = capture.spawn_with_readiness(Some(ready_tx));

        // Invalid regex causes ready_tx to be dropped — receiver should error.
        let result = timeout(Duration::from_secs(2), ready_rx).await;
        assert!(
            matches!(result, Ok(Err(_))),
            "ready_rx should get RecvError when regex is invalid"
        );
    }
}
