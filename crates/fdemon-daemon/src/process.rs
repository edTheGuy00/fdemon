//! Flutter process management

use std::path::Path;
use std::process::Stdio;

use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;

use super::commands::{CommandSender, DaemonCommand, RequestTracker};
use fdemon_core::events::DaemonEvent;
use fdemon_core::prelude::*;

/// Manages a Flutter child process
pub struct FlutterProcess {
    /// The child process handle
    child: Child,
    /// Sender for stdin commands
    stdin_tx: mpsc::Sender<String>,
    /// Process ID for logging
    pid: Option<u32>,
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

        // Spawn stdout reader task
        let stdout = child.stdout.take().expect("stdout was configured");
        tokio::spawn(Self::stdout_reader(stdout, event_tx.clone()));

        // Spawn stderr reader task
        let stderr = child.stderr.take().expect("stderr was configured");
        tokio::spawn(Self::stderr_reader(stderr, event_tx));

        Ok(Self {
            child,
            stdin_tx,
            pid,
        })
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

    /// Read lines from stdout and send as DaemonEvents
    async fn stdout_reader(stdout: tokio::process::ChildStdout, tx: mpsc::Sender<DaemonEvent>) {
        let mut reader = BufReader::new(stdout).lines();

        while let Ok(Some(line)) = reader.next_line().await {
            trace!("stdout: {}", line);

            if tx.send(DaemonEvent::Stdout(line)).await.is_err() {
                debug!("stdout channel closed");
                break;
            }
        }

        info!("stdout reader finished, process likely exited");
        // Send exit event when stdout closes
        let _ = tx.send(DaemonEvent::Exited { code: None }).await;
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

    /// Gracefully shutdown the Flutter process
    ///
    /// Optimized for fast shutdown:
    /// 1. Early exit if process already dead
    /// 2. Send app.stop command with 1s timeout (reduced from 5s)
    /// 3. Send daemon.shutdown command
    /// 4. Wait 2s for graceful exit (reduced from 5s)
    /// 5. Force kill if needed
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

        // Step 3: Wait up to 2 seconds for graceful exit (reduced from 5s)
        match timeout(Duration::from_secs(2), self.child.wait()).await {
            Ok(Ok(status)) => {
                info!("Flutter process exited gracefully: {:?}", status);
                Ok(())
            }
            Ok(Err(e)) => {
                warn!("Error waiting for process: {}", e);
                self.force_kill().await
            }
            Err(_) => {
                warn!("Timeout waiting for graceful exit, force killing");
                self.force_kill().await
            }
        }
    }

    /// Force kill the process
    async fn force_kill(&mut self) -> Result<()> {
        warn!("Force killing Flutter process");
        self.child
            .kill()
            .await
            .map_err(|e| Error::process(format!("Failed to kill: {}", e)))
    }

    /// Check if the process is still running
    pub fn is_running(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    /// Check if the process has already exited
    pub fn has_exited(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(Some(_)))
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
        if let Ok(None) = self.child.try_wait() {
            warn!("FlutterProcess dropped while still running");
        }
        // kill_on_drop(true) handles actual cleanup
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
}
