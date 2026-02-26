//! Flutter process management

use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, oneshot, Notify};

use super::commands::{CommandSender, DaemonCommand, RequestTracker};
use fdemon_core::events::DaemonEvent;
use fdemon_core::prelude::*;

/// Manages a Flutter child process.
///
/// The `Child` handle is moved into a dedicated `wait_for_exit` background task that
/// calls `child.wait()`. This ensures the real exit code is captured and emitted as
/// `DaemonEvent::Exited { code: Some(N) }` rather than always `None`.
///
/// `FlutterProcess` retains a kill channel ([`kill_tx`]) to request a force-kill, an
/// atomic flag ([`exited`]) for synchronous `has_exited()` checks, and a [`Notify`]
/// handle so `shutdown()` can await graceful exit without holding a lock across `.await`.
pub struct FlutterProcess {
    /// Sender for stdin commands
    stdin_tx: mpsc::Sender<String>,
    /// Process ID for logging
    pid: Option<u32>,
    /// One-shot sender that tells the wait task to force-kill the process.
    /// Consumed on first use (or on drop).
    kill_tx: Option<oneshot::Sender<()>>,
    /// Set to `true` by the wait task once the child has exited.
    /// Allows synchronous `has_exited()` / `is_running()` checks.
    exited: Arc<AtomicBool>,
    /// Notified by the wait task immediately after the child exits.
    /// Used by `shutdown()` to await graceful termination without polling.
    exit_notify: Arc<Notify>,
}

impl FlutterProcess {
    /// Internal spawn implementation. All public methods delegate here.
    fn spawn_internal(
        args: &[String],
        project_path: &Path,
        event_tx: mpsc::Sender<DaemonEvent>,
    ) -> Result<Self> {
        // Validate project path
        let pubspec = project_path.join("pubspec.yaml");
        if !pubspec.exists() {
            return Err(Error::NoProject {
                path: project_path.to_path_buf(),
            });
        }

        info!("Spawning Flutter: flutter {}", args.join(" "));

        // Spawn the Flutter process
        let mut child = Command::new("flutter")
            .args(args)
            .current_dir(project_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true) // Critical: cleanup on drop
            .spawn()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    Error::FlutterNotFound
                } else {
                    Error::ProcessSpawn {
                        reason: e.to_string(),
                    }
                }
            })?;

        let pid = child.id();
        info!("Flutter process started with PID: {:?}", pid);

        // Take ownership of stdin and create command channel
        let stdin = child.stdin.take().expect("stdin was configured");
        let (stdin_tx, stdin_rx) = mpsc::channel::<String>(32);
        tokio::spawn(Self::stdin_writer(stdin, stdin_rx));

        // Spawn stdout reader task (no longer emits Exited — that's the wait task's job)
        let stdout = child.stdout.take().expect("stdout was configured");
        tokio::spawn(Self::stdout_reader(stdout, event_tx.clone()));

        // Spawn stderr reader task
        let stderr = child.stderr.take().expect("stderr was configured");
        tokio::spawn(Self::stderr_reader(stderr, event_tx.clone()));

        // Shared exit-state primitives
        let exited = Arc::new(AtomicBool::new(false));
        let exit_notify = Arc::new(Notify::new());

        // Kill channel: FlutterProcess holds the sender, wait task holds the receiver.
        let (kill_tx, kill_rx) = oneshot::channel::<()>();

        // Spawn the dedicated wait task — takes ownership of `child`.
        tokio::spawn(Self::wait_for_exit(
            child,
            kill_rx,
            event_tx,
            Arc::clone(&exited),
            Arc::clone(&exit_notify),
        ));

        Ok(Self {
            stdin_tx,
            pid,
            kill_tx: Some(kill_tx),
            exited,
            exit_notify,
        })
    }

    /// Background task: owns `child`, waits for it to exit, emits `DaemonEvent::Exited`.
    ///
    /// Two ways the task can end:
    /// 1. The Flutter process exits naturally — `child.wait()` resolves.
    /// 2. `kill_rx` fires — we kill the child first, then wait for it.
    async fn wait_for_exit(
        mut child: Child,
        kill_rx: oneshot::Receiver<()>,
        event_tx: mpsc::Sender<DaemonEvent>,
        exited: Arc<AtomicBool>,
        exit_notify: Arc<Notify>,
    ) {
        let code: Option<i32> = tokio::select! {
            // Natural exit path
            result = child.wait() => {
                match result {
                    Ok(status) => {
                        info!("Flutter process exited with status: {:?}", status);
                        status.code()
                    }
                    Err(e) => {
                        error!("Error waiting for Flutter process: {}", e);
                        None
                    }
                }
            }
            // Force-kill path: kill_tx was sent (by shutdown or drop)
            _ = kill_rx => {
                info!("Kill signal received, force-killing Flutter process");
                if let Err(e) = child.kill().await {
                    error!("Failed to kill Flutter process: {}", e);
                }
                match child.wait().await {
                    Ok(status) => {
                        info!("Flutter process killed, exit status: {:?}", status);
                        status.code()
                    }
                    Err(e) => {
                        error!("Error waiting after kill: {}", e);
                        None
                    }
                }
            }
        };

        // Mark process as exited and wake any waiters before sending the event.
        // This order ensures `has_exited()` is true before callers observe the event.
        exited.store(true, Ordering::Release);
        exit_notify.notify_waiters();

        debug!("Sending DaemonEvent::Exited {{ code: {:?} }}", code);
        let _ = event_tx.send(DaemonEvent::Exited { code }).await;
    }

    /// Spawn a new Flutter process in the given project directory
    ///
    /// Events are sent to `event_tx` for processing by the TUI event loop.
    pub async fn spawn(project_path: &Path, event_tx: mpsc::Sender<DaemonEvent>) -> Result<Self> {
        let args = vec!["run".to_string(), "--machine".to_string()];
        Self::spawn_internal(&args, project_path, event_tx)
    }

    /// Spawn a new Flutter process with a specific device
    ///
    /// Similar to `spawn()` but adds `-d <device_id>` argument.
    pub async fn spawn_with_device(
        project_path: &Path,
        device_id: &str,
        event_tx: mpsc::Sender<DaemonEvent>,
    ) -> Result<Self> {
        let args = vec![
            "run".to_string(),
            "--machine".to_string(),
            "-d".to_string(),
            device_id.to_string(),
        ];
        Self::spawn_internal(&args, project_path, event_tx)
    }

    /// Spawn a Flutter process with pre-built arguments
    ///
    /// The caller is responsible for building the complete argument list including
    /// `run`, `--machine`, `-d`, and all other flags.
    pub async fn spawn_with_args(
        project_path: &Path,
        args: Vec<String>,
        event_tx: mpsc::Sender<DaemonEvent>,
    ) -> Result<Self> {
        Self::spawn_internal(&args, project_path, event_tx)
    }

    /// Read lines from stdout and send as `DaemonEvent::Stdout`.
    ///
    /// Does NOT emit `DaemonEvent::Exited` — that is the responsibility of the
    /// `wait_for_exit` task, which captures the real exit code.
    async fn stdout_reader(stdout: tokio::process::ChildStdout, tx: mpsc::Sender<DaemonEvent>) {
        let mut reader = BufReader::new(stdout).lines();

        while let Ok(Some(line)) = reader.next_line().await {
            trace!("stdout: {}", line);

            if tx.send(DaemonEvent::Stdout(line)).await.is_err() {
                debug!("stdout channel closed");
                break;
            }
        }

        // Stdout EOF just means the pipe closed; the process may still be shutting down.
        // The wait_for_exit task will emit DaemonEvent::Exited with the real exit code.
        info!("stdout reader finished, process likely exiting");
    }

    /// Read lines from stderr and send as DaemonEvents
    async fn stderr_reader(stderr: tokio::process::ChildStderr, tx: mpsc::Sender<DaemonEvent>) {
        let mut reader = BufReader::new(stderr).lines();

        while let Ok(Some(line)) = reader.next_line().await {
            trace!("stderr: {}", line);

            if tx.send(DaemonEvent::Stderr(line)).await.is_err() {
                debug!("stderr channel closed");
                break;
            }
        }

        debug!("stderr reader finished");
    }

    /// Write commands to stdin
    async fn stdin_writer(mut stdin: tokio::process::ChildStdin, mut rx: mpsc::Receiver<String>) {
        while let Some(command) = rx.recv().await {
            debug!("Sending to daemon: {}", command);

            // Write command followed by newline
            if let Err(e) = stdin.write_all(command.as_bytes()).await {
                error!("Failed to write to stdin: {}", e);
                break;
            }
            if let Err(e) = stdin.write_all(b"\n").await {
                error!("Failed to write newline: {}", e);
                break;
            }
            if let Err(e) = stdin.flush().await {
                error!("Failed to flush stdin: {}", e);
                break;
            }
        }

        debug!("stdin writer finished");
    }

    /// Send a raw command to the Flutter process
    pub async fn send(&self, command: &str) -> Result<()> {
        self.stdin_tx
            .send(command.to_string())
            .await
            .map_err(|_| Error::channel_send("stdin channel closed"))
    }

    /// Send a JSON-RPC command (auto-wrapped in brackets)
    pub async fn send_json(&self, json: &str) -> Result<()> {
        let wrapped = format!("[{}]", json);
        self.send(&wrapped).await
    }

    /// Gracefully shutdown the Flutter process.
    ///
    /// Optimized for fast shutdown:
    /// 1. Early exit if process already dead (atomic check — no lock)
    /// 2. Send app.stop command with 1s timeout
    /// 3. Send daemon.shutdown command
    /// 4. Wait up to 2s for graceful exit via `exit_notify`
    /// 5. Send kill signal to the wait task if graceful exit times out
    pub async fn shutdown(
        &mut self,
        app_id: Option<&str>,
        cmd_sender: Option<&CommandSender>,
    ) -> Result<()> {
        use std::time::Duration;
        use tokio::time::timeout;

        // Fast path: if process already exited, we're done
        if self.has_exited() {
            info!("Flutter process already exited, skipping shutdown commands");
            return Ok(());
        }

        info!("Initiating Flutter process shutdown");

        // Step 1: Stop the app if we have an app_id and command sender
        // Reduced timeout from 5s to 1s for faster shutdown
        if let (Some(id), Some(sender)) = (app_id, cmd_sender) {
            info!("Stopping Flutter app: {}", id);
            match sender
                .send_with_timeout(
                    DaemonCommand::Stop {
                        app_id: id.to_string(),
                    },
                    Duration::from_secs(1),
                )
                .await
            {
                Ok(_) => info!("App stop command acknowledged"),
                Err(e) => {
                    // Check if process died while we were waiting
                    if self.has_exited() {
                        info!("Process exited during stop command");
                        return Ok(());
                    }
                    warn!("App stop command failed (continuing): {}", e);
                }
            }
        }

        // Step 2: Send daemon.shutdown command
        let shutdown_cmd = r#"{"method":"daemon.shutdown","id":9999}"#;
        let _ = self.send_json(shutdown_cmd).await;

        // Step 3: Wait up to 2 seconds for graceful exit.
        //
        // Race-free pattern: create the `notified()` future BEFORE the final
        // `has_exited()` check, so we cannot miss a notification that fires
        // between the check and the await.
        let notified = self.exit_notify.notified();
        if self.has_exited() {
            info!("Flutter process exited gracefully");
            return Ok(());
        }

        match timeout(Duration::from_secs(2), notified).await {
            Ok(()) => {
                info!("Flutter process exited gracefully");
                Ok(())
            }
            Err(_) => {
                warn!("Timeout waiting for graceful exit, force killing");
                self.force_kill()
            }
        }
    }

    /// Force kill the process by signalling the wait task.
    ///
    /// The wait task calls `child.kill()` and then `child.wait()`, ensuring the
    /// OS reaps the process correctly before emitting `DaemonEvent::Exited`.
    fn force_kill(&mut self) -> Result<()> {
        warn!("Force killing Flutter process via kill channel");
        if let Some(tx) = self.kill_tx.take() {
            // Ignore send error — the wait task may have already exited naturally.
            let _ = tx.send(());
        }
        Ok(())
    }

    /// Check if the process has already exited.
    ///
    /// This is a non-blocking, synchronous check backed by an atomic flag that is
    /// set by the `wait_for_exit` task.  Unlike the old `try_wait()` approach, this
    /// method takes `&self` (not `&mut self`) and never races with `child.wait()`.
    pub fn has_exited(&self) -> bool {
        self.exited.load(Ordering::Acquire)
    }

    /// Check if the process is still running.
    ///
    /// This is the logical complement of `has_exited()`.
    pub fn is_running(&self) -> bool {
        !self.has_exited()
    }

    /// Get the process ID
    pub fn id(&self) -> Option<u32> {
        self.pid
    }

    /// Get the stdin sender for creating a CommandSender
    pub fn stdin_sender(&self) -> mpsc::Sender<String> {
        self.stdin_tx.clone()
    }

    /// Create a command sender for this process
    pub fn command_sender(&self, tracker: Arc<RequestTracker>) -> CommandSender {
        CommandSender::new(self.stdin_tx.clone(), tracker)
    }
}

impl Drop for FlutterProcess {
    fn drop(&mut self) {
        if !self.has_exited() {
            warn!("FlutterProcess dropped while process may still be running");
            // Send kill signal so the wait task tears down the child cleanly.
            // If kill_tx was already consumed by shutdown(), this is a no-op.
            if let Some(tx) = self.kill_tx.take() {
                let _ = tx.send(());
            }
        }
        // kill_on_drop(true) on the Child is the final safety net if the
        // wait task hasn't had a chance to handle the kill yet.
        debug!("FlutterProcess dropped");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_spawn_no_project() {
        let (tx, _rx) = mpsc::channel(16);
        let result = FlutterProcess::spawn(Path::new("/nonexistent/path"), tx).await;

        assert!(matches!(result, Err(Error::NoProject { .. })));
    }

    #[tokio::test]
    async fn test_spawn_invalid_path() {
        let (tx, _rx) = mpsc::channel(16);
        let temp = std::env::temp_dir().join("fdemon-no-pubspec-test");
        std::fs::create_dir_all(&temp).ok();

        let result = FlutterProcess::spawn(&temp, tx).await;
        assert!(matches!(result, Err(Error::NoProject { .. })));

        std::fs::remove_dir_all(&temp).ok();
    }

    /// Helper: spawn a short-lived real process (not Flutter) using the internal machinery.
    ///
    /// We exercise only the wait task by bypassing `spawn_internal`'s pubspec check.
    /// We use `sh -c "exit N"` as a stand-in for a Flutter process.
    async fn spawn_test_process(
        exit_code: i32,
        event_tx: mpsc::Sender<DaemonEvent>,
    ) -> FlutterProcess {
        // Build a trivial child that exits immediately with the given code
        let mut child = Command::new("sh")
            .args(["-c", &format!("exit {}", exit_code)])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .expect("sh must be available in test environment");

        let pid = child.id();

        // Consume stdio so they don't block
        let stdin = child.stdin.take().expect("stdin");
        let (stdin_tx, stdin_rx) = mpsc::channel::<String>(4);
        tokio::spawn(FlutterProcess::stdin_writer(stdin, stdin_rx));

        let stdout = child.stdout.take().expect("stdout");
        tokio::spawn(FlutterProcess::stdout_reader(stdout, event_tx.clone()));

        let stderr = child.stderr.take().expect("stderr");
        tokio::spawn(FlutterProcess::stderr_reader(stderr, event_tx.clone()));

        let exited = Arc::new(AtomicBool::new(false));
        let exit_notify = Arc::new(Notify::new());
        let (kill_tx, kill_rx) = oneshot::channel::<()>();

        tokio::spawn(FlutterProcess::wait_for_exit(
            child,
            kill_rx,
            event_tx,
            Arc::clone(&exited),
            Arc::clone(&exit_notify),
        ));

        FlutterProcess {
            stdin_tx,
            pid,
            kill_tx: Some(kill_tx),
            exited,
            exit_notify,
        }
    }

    #[tokio::test]
    async fn test_exit_code_captured_on_normal_exit() {
        let (tx, mut rx) = mpsc::channel(16);
        let _process = spawn_test_process(0, tx).await;

        // Drain events until we find the Exited event
        let mut found = false;
        for _ in 0..50 {
            match tokio::time::timeout(std::time::Duration::from_millis(100), rx.recv()).await {
                Ok(Some(DaemonEvent::Exited { code })) => {
                    assert_eq!(code, Some(0), "expected exit code 0, got {:?}", code);
                    found = true;
                    break;
                }
                Ok(Some(_)) => continue,
                Ok(None) => break,
                Err(_) => break,
            }
        }
        assert!(found, "DaemonEvent::Exited was not received");
    }

    #[tokio::test]
    async fn test_exit_code_captured_on_error_exit() {
        let (tx, mut rx) = mpsc::channel(16);
        let _process = spawn_test_process(42, tx).await;

        let mut found = false;
        for _ in 0..50 {
            match tokio::time::timeout(std::time::Duration::from_millis(100), rx.recv()).await {
                Ok(Some(DaemonEvent::Exited { code })) => {
                    assert_eq!(code, Some(42), "expected exit code 42, got {:?}", code);
                    found = true;
                    break;
                }
                Ok(Some(_)) => continue,
                Ok(None) => break,
                Err(_) => break,
            }
        }
        assert!(found, "DaemonEvent::Exited was not received");
    }

    #[tokio::test]
    async fn test_stdout_reader_does_not_emit_exited_event() {
        // Spawn a process that immediately closes stdout; we should get exactly
        // one Exited event (from wait_for_exit), not two.
        let (tx, mut rx) = mpsc::channel(32);
        let _process = spawn_test_process(0, tx).await;

        let mut exited_count = 0usize;
        let deadline = tokio::time::sleep(std::time::Duration::from_millis(500));
        tokio::pin!(deadline);

        loop {
            tokio::select! {
                event = rx.recv() => {
                    match event {
                        Some(DaemonEvent::Exited { .. }) => exited_count += 1,
                        Some(_) => {}
                        None => break,
                    }
                }
                _ = &mut deadline => break,
            }
        }

        assert_eq!(
            exited_count, 1,
            "expected exactly one Exited event, got {}",
            exited_count
        );
    }

    #[tokio::test]
    async fn test_has_exited_becomes_true_after_exit() {
        let (tx, mut rx) = mpsc::channel(16);
        let process = spawn_test_process(0, tx).await;

        // Wait for the Exited event
        loop {
            match tokio::time::timeout(std::time::Duration::from_millis(200), rx.recv()).await {
                Ok(Some(DaemonEvent::Exited { .. })) => break,
                Ok(Some(_)) => continue,
                _ => panic!("did not receive Exited event in time"),
            }
        }

        // After the event, has_exited() must be true
        assert!(
            process.has_exited(),
            "has_exited() should be true after Exited event"
        );
        assert!(
            !process.is_running(),
            "is_running() should be false after Exited event"
        );
    }

    #[tokio::test]
    async fn test_shutdown_kills_long_running_process() {
        // Spawn a process that sleeps indefinitely
        let mut child = Command::new("sh")
            .args(["-c", "sleep 60"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .expect("sh must be available");

        let pid = child.id();
        let (event_tx, mut event_rx) = mpsc::channel::<DaemonEvent>(16);

        let stdin = child.stdin.take().expect("stdin");
        let (stdin_tx, stdin_rx) = mpsc::channel::<String>(4);
        tokio::spawn(FlutterProcess::stdin_writer(stdin, stdin_rx));

        let stdout = child.stdout.take().expect("stdout");
        tokio::spawn(FlutterProcess::stdout_reader(stdout, event_tx.clone()));

        let stderr = child.stderr.take().expect("stderr");
        tokio::spawn(FlutterProcess::stderr_reader(stderr, event_tx.clone()));

        let exited = Arc::new(AtomicBool::new(false));
        let exit_notify = Arc::new(Notify::new());
        let (kill_tx, kill_rx) = oneshot::channel::<()>();

        tokio::spawn(FlutterProcess::wait_for_exit(
            child,
            kill_rx,
            event_tx,
            Arc::clone(&exited),
            Arc::clone(&exit_notify),
        ));

        let mut process = FlutterProcess {
            stdin_tx,
            pid,
            kill_tx: Some(kill_tx),
            exited,
            exit_notify,
        };

        // Confirm it's running
        assert!(!process.has_exited());

        // Shutdown should succeed by sending the kill signal
        process
            .shutdown(None, None)
            .await
            .expect("shutdown should not error");

        // Wait for the Exited event
        let mut got_exited = false;
        for _ in 0..30 {
            match tokio::time::timeout(std::time::Duration::from_millis(100), event_rx.recv()).await
            {
                Ok(Some(DaemonEvent::Exited { .. })) => {
                    got_exited = true;
                    break;
                }
                Ok(Some(_)) => continue,
                _ => break,
            }
        }
        assert!(
            got_exited,
            "DaemonEvent::Exited should be received after shutdown"
        );
    }
}
