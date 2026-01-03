//! Flutter process management

use std::path::Path;
use std::process::Stdio;

use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;

use super::commands::{CommandSender, RequestTracker};
use crate::common::prelude::*;
use crate::core::DaemonEvent;

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
    /// Spawn a new Flutter process in the given project directory
    ///
    /// Events are sent to `event_tx` for processing by the TUI event loop.
    pub async fn spawn(project_path: &Path, event_tx: mpsc::Sender<DaemonEvent>) -> Result<Self> {
        // Validate project path
        let pubspec = project_path.join("pubspec.yaml");
        if !pubspec.exists() {
            return Err(Error::NoProject {
                path: project_path.to_path_buf(),
            });
        }

        info!("Spawning Flutter process in: {}", project_path.display());

        // Spawn the Flutter process
        let mut child = Command::new("flutter")
            .args(["run", "--machine"])
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
        let stdout_tx = event_tx.clone();
        tokio::spawn(Self::stdout_reader(stdout, stdout_tx));

        // Spawn stderr reader task
        let stderr = child.stderr.take().expect("stderr was configured");
        let stderr_tx = event_tx.clone();
        tokio::spawn(Self::stderr_reader(stderr, stderr_tx));

        Ok(Self {
            child,
            stdin_tx,
            pid,
        })
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
    /// 1. Send daemon.shutdown command
    /// 2. Wait with timeout
    /// 3. Force kill if needed
    pub async fn shutdown(&mut self) -> Result<()> {
        use std::time::Duration;
        use tokio::time::timeout;

        info!("Initiating Flutter process shutdown");

        // Try graceful shutdown first
        let shutdown_cmd = r#"{"method":"daemon.shutdown","id":9999}"#;
        let _ = self.send_json(shutdown_cmd).await;

        // Wait up to 5 seconds for graceful exit
        match timeout(Duration::from_secs(5), self.child.wait()).await {
            Ok(Ok(status)) => {
                info!("Flutter process exited gracefully: {:?}", status);
                Ok(())
            }
            Ok(Err(e)) => {
                warn!("Error waiting for process: {}", e);
                self.force_kill().await
            }
            Err(_) => {
                warn!("Timeout waiting for graceful exit");
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
