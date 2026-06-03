//! # Streaming Child-Process Runner
//!
//! Provides [`run_streaming`], which spawns a child process, merges its stdout
//! and stderr streams into a single line sequence, and forwards each line to a
//! caller-supplied callback. The final exit status is returned to the caller
//! so that non-zero exits can be handled explicitly.
//!
//! ## Use Cases
//!
//! - `git clone` — progress lines are written to stderr; merging ensures they
//!   are not silently discarded.
//! - `flutter precache` — similar stderr progress.
//! - Phase 3: `sdkmanager` — a mixture of stdout and stderr output.
//!
//! ## Design Notes
//!
//! - stdout and stderr are read concurrently via two separate
//!   `tokio::io::BufReader` tasks that forward lines through an `mpsc` channel.
//!   This avoids the classic deadlock where a full pipe buffer blocks the child.
//! - The callback receives one `String` per line (newline stripped).
//! - A non-zero exit status is returned as-is; the caller decides how to
//!   surface it (e.g. as an error or as a warning).

use std::path::Path;
use std::process::{ExitStatus, Stdio};

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

use fdemon_core::{Error, Result};

// ── Constants ─────────────────────────────────────────────────────────────────

/// Buffer capacity for the internal line-forwarding channel.
/// Large enough to avoid back-pressure on fast-writing processes.
const LINE_CHANNEL_CAPACITY: usize = 256;

// ── Public API ────────────────────────────────────────────────────────────────

/// Run `program` with `args`, forwarding each stdout/stderr line to `on_line`,
/// and return the exit status.
///
/// Both stdout and stderr are captured and merged into a single stream. Lines
/// are delivered in arrival order (interleaved, not sorted by stream). The
/// callback receives the line text with the trailing newline stripped.
///
/// # Arguments
///
/// * `program` — The executable to run (looked up on `PATH` if not absolute).
/// * `args` — Slice of argument strings.
/// * `cwd` — Optional working directory for the child process; `None` inherits
///   the current working directory.
/// * `on_line` — Callback invoked once per output line.
///
/// # Returns
///
/// The [`ExitStatus`] of the child process. A non-zero status is not treated
/// as an error by this function; the caller is responsible for interpretation.
///
/// # Errors
///
/// Returns an error when the child process cannot be spawned, or when reading
/// from its stdout/stderr fails unexpectedly.
pub async fn run_streaming<F>(
    program: &str,
    args: &[&str],
    cwd: Option<&Path>,
    mut on_line: F,
) -> Result<ExitStatus>
where
    F: FnMut(String) + Send,
{
    let mut cmd = Command::new(program);
    cmd.args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| Error::process(format!("failed to spawn `{program}`: {e}")))?;

    // Take ownership of the piped stdout/stderr before awaiting the child.
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| Error::process(format!("could not capture stdout of `{program}`")))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| Error::process(format!("could not capture stderr of `{program}`")))?;

    // Channel to merge stdout and stderr lines.
    let (tx, mut rx) = mpsc::channel::<String>(LINE_CHANNEL_CAPACITY);

    let tx_out = tx.clone();
    let tx_err = tx;

    // Reader task for stdout.
    let stdout_task = tokio::spawn(async move {
        let mut reader = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            if tx_out.send(line).await.is_err() {
                break; // Receiver dropped; stop forwarding.
            }
        }
    });

    // Reader task for stderr.
    let stderr_task = tokio::spawn(async move {
        let mut reader = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            if tx_err.send(line).await.is_err() {
                break; // Receiver dropped; stop forwarding.
            }
        }
    });

    // Drain the merged line channel and forward to the callback.
    while let Some(line) = rx.recv().await {
        on_line(line);
    }

    // Wait for reader tasks to finish (they exit when the pipes close).
    let _ = stdout_task.await;
    let _ = stderr_task.await;

    let status = child
        .wait()
        .await
        .map_err(|e| Error::process(format!("failed to wait for `{program}`: {e}")))?;

    Ok(status)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Run a command that succeeds and collect its output lines.
    #[tokio::test]
    async fn test_run_streaming_captures_lines_and_success_status() {
        // Use a shell-independent approach: `echo` via sh on Unix, cmd on Windows.
        #[cfg(unix)]
        let (program, args): (&str, &[&str]) = ("sh", &["-c", "echo hello; echo world"]);
        #[cfg(windows)]
        let (program, args): (&str, &[&str]) = ("cmd", &["/C", "echo hello & echo world"]);

        let mut lines: Vec<String> = Vec::new();
        let status = run_streaming(program, args, None, |line| lines.push(line))
            .await
            .expect("run_streaming must not error");

        assert!(status.success(), "command must exit 0");
        // At least "hello" and "world" should appear in output.
        let joined = lines.join(" ");
        assert!(
            joined.contains("hello"),
            "output must contain 'hello': {joined:?}"
        );
        assert!(
            joined.contains("world"),
            "output must contain 'world': {joined:?}"
        );
    }

    /// A command that writes to both stdout and stderr should have both
    /// streams captured.
    #[tokio::test]
    async fn test_run_streaming_captures_stderr_as_well() {
        #[cfg(unix)]
        let (program, args): (&str, &[&str]) =
            ("sh", &["-c", "echo stdout_line; echo stderr_line >&2"]);
        #[cfg(windows)]
        // On Windows, write to stderr via 1>&2 redirect.
        let (program, args): (&str, &[&str]) =
            ("cmd", &["/C", "echo stdout_line & echo stderr_line 1>&2"]);

        let mut lines: Vec<String> = Vec::new();
        let status = run_streaming(program, args, None, |line| lines.push(line))
            .await
            .expect("must not error");

        assert!(status.success(), "command must exit 0");
        let joined = lines.join(" ");
        assert!(
            joined.contains("stdout_line"),
            "stdout line missing: {joined:?}"
        );
        assert!(
            joined.contains("stderr_line"),
            "stderr line missing: {joined:?}"
        );
    }

    /// A non-zero exit status must be returned to the caller, not swallowed.
    #[tokio::test]
    async fn test_run_streaming_returns_nonzero_exit_status() {
        #[cfg(unix)]
        let (program, args): (&str, &[&str]) = ("sh", &["-c", "exit 42"]);
        #[cfg(windows)]
        let (program, args): (&str, &[&str]) = ("cmd", &["/C", "exit 42"]);

        let mut lines: Vec<String> = Vec::new();
        let status = run_streaming(program, args, None, |line| lines.push(line))
            .await
            .expect("run_streaming must not error on non-zero exit");

        assert!(!status.success(), "exit 42 must not be success");
        #[cfg(unix)]
        assert_eq!(status.code(), Some(42));
    }

    /// Calling a non-existent program must return an error, not panic.
    #[tokio::test]
    async fn test_run_streaming_nonexistent_program_returns_error() {
        let err = run_streaming(
            "this_program_definitely_does_not_exist_fdemon_test",
            &[],
            None,
            |_| {},
        )
        .await
        .expect_err("must return error for non-existent program");

        assert!(
            matches!(err, fdemon_core::Error::Process { .. }),
            "expected Process error, got {err:?}"
        );
    }

    /// Working directory is forwarded to the child when `cwd` is `Some`.
    #[tokio::test]
    async fn test_run_streaming_respects_cwd() {
        let tmp = tempfile::TempDir::new().unwrap();

        // Write a marker file into the temp dir.
        let marker = tmp.path().join("marker.txt");
        std::fs::write(&marker, b"hello").unwrap();

        // Run `ls` (Unix) or `dir` (Windows) in the temp dir and check the
        // marker file appears in the output.
        #[cfg(unix)]
        let (program, args): (&str, &[&str]) = ("sh", &["-c", "ls"]);
        #[cfg(windows)]
        let (program, args): (&str, &[&str]) = ("cmd", &["/C", "dir /b"]);

        let mut lines: Vec<String> = Vec::new();
        let status = run_streaming(program, args, Some(tmp.path()), |line| lines.push(line))
            .await
            .expect("run_streaming must succeed");

        assert!(status.success());
        let joined = lines.join(" ");
        assert!(
            joined.contains("marker.txt"),
            "cwd listing must include marker.txt: {joined:?}"
        );
    }
}
