//! # Docker Test Helpers
//!
//! Utilities for building Docker images and running `fdemon` inside containers
//! from Rust integration tests.
//!
//! All public functions in this module are intentionally synchronous; they
//! drive the `docker` CLI as a child process so that tests remain simple and
//! do not require a Tokio runtime.
//!
//! ## Gating
//!
//! Docker tests must carry `#[ignore = "requires Docker"]` so that they are
//! skipped in CI environments that do not have a Docker daemon.  Each test
//! body should additionally call [`docker_available`] and return early when
//! the daemon is not reachable:
//!
//! ```rust,no_run
//! #[test]
//! #[ignore = "requires Docker — run with `cargo test -- --ignored`"]
//! fn test_something_with_docker() {
//!     if !docker_available() {
//!         eprintln!("Docker not available, skipping");
//!         return;
//!     }
//!     // ... test body
//! }
//! ```
//!
//! To run Docker tests explicitly:
//!
//! ```bash
//! cargo test --test sdk_detection -- --ignored
//! ```

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// Monotonically increasing counter for unique Docker container names.
static CONTAINER_COUNTER: AtomicU64 = AtomicU64::new(0);

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Captured output from a Docker container run.
#[derive(Debug)]
pub struct DockerTestResult {
    /// Exit code reported by the container process.
    /// `-1` when the process was killed by the timeout watchdog.
    pub exit_code: i32,
    /// Bytes collected from the container's stdout, decoded as UTF-8 (lossy).
    pub stdout: String,
    /// Bytes collected from the container's stderr, decoded as UTF-8 (lossy).
    pub stderr: String,
}

// ---------------------------------------------------------------------------
// Docker availability
// ---------------------------------------------------------------------------

/// Returns `true` when the Docker daemon is reachable.
///
/// Runs `docker info` as a quick liveness probe.  The test suite uses this to
/// skip Docker-backed tests gracefully when no daemon is available.
pub fn docker_available() -> bool {
    Command::new("docker")
        .args(["info", "--format", "{{.ServerVersion}}"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Project root discovery
// ---------------------------------------------------------------------------

/// Returns the workspace root path.
///
/// Uses the `CARGO_MANIFEST_DIR` environment variable that Cargo sets when
/// running integration tests.  For the binary crate this points at the
/// workspace root itself, which is the correct Docker build context for
/// `COPY . .` in the Dockerfile.
pub fn project_root() -> PathBuf {
    let manifest_dir =
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set by Cargo");
    PathBuf::from(manifest_dir)
}

// ---------------------------------------------------------------------------
// docker build
// ---------------------------------------------------------------------------

/// Build a Docker image from a Dockerfile and return the image tag.
///
/// # Arguments
///
/// * `dockerfile` — Relative path from `project_root` to the Dockerfile,
///   e.g. `"tests/docker/base.Dockerfile"`.
/// * `tag` — Docker image tag to assign, e.g. `"fdemon-test-base"`.
/// * `project_root` — Build context directory (passed as `.` to `docker build`).
///   Must contain the full workspace source tree so that `COPY . .` works.
///
/// # Errors
///
/// Returns a human-readable `String` describing the failure when `docker build`
/// exits with a non-zero status or when the process cannot be spawned.
pub fn docker_build(dockerfile: &str, tag: &str, project_root: &Path) -> Result<String, String> {
    // Skip build if the image already exists (avoids redundant builds when
    // multiple tests share the same image and run in parallel).
    let exists = Command::new("docker")
        .args(["image", "inspect", tag])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if exists {
        return Ok(tag.to_string());
    }

    // Force linux/amd64 because Flutter SDK only publishes x86_64 Linux tarballs.
    // On Apple Silicon hosts this uses Rosetta 2 (OrbStack) or QEMU emulation.
    //
    // Timeout: Docker builds under QEMU emulation can take 10+ minutes for the
    // Rust compilation stage alone.  We cap at 10 minutes to avoid indefinitely
    // blocking the test suite when a version-manager installer hangs.
    const BUILD_TIMEOUT_SECS: u64 = 600;

    let child = Command::new("docker")
        .args([
            "build",
            "--platform",
            "linux/amd64",
            "-f",
            dockerfile,
            "-t",
            tag,
            ".",
        ])
        .current_dir(project_root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn `docker build`: {e}"))?;

    let child_id = child.id();

    // Watchdog: kill the build after the timeout.
    let timed_out = Arc::new(Mutex::new(false));
    let timed_out_clone = Arc::clone(&timed_out);
    let watchdog = thread::spawn(move || {
        thread::sleep(Duration::from_secs(BUILD_TIMEOUT_SECS));
        let mut flag = timed_out_clone.lock().expect("mutex poisoned");
        if !*flag {
            *flag = true;
            // Kill the docker build process.
            let _ = Command::new("kill")
                .arg(child_id.to_string())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
    });

    let output = child
        .wait_with_output()
        .map_err(|e| format!("Failed to wait for `docker build`: {e}"))?;

    // Signal the watchdog that we're done.
    {
        let mut flag = timed_out.lock().expect("mutex poisoned");
        *flag = true;
    }
    drop(watchdog);

    if output.status.success() {
        Ok(tag.to_string())
    } else {
        Err(format!(
            "`docker build` failed (exit {}):\n{}",
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

// ---------------------------------------------------------------------------
// docker run (headless fdemon)
// ---------------------------------------------------------------------------

/// Run `fdemon --headless /test-project` inside a Docker container and capture
/// its output.
///
/// The container is started with `--rm --init` so that:
/// - `--rm` removes the container automatically when it exits.
/// - `--init` ensures `fdemon` receives signals correctly (PID 1 reaping).
///
/// A watchdog thread kills the container after `timeout_secs` to prevent
/// indefinitely hung containers from blocking the test suite.
///
/// # Arguments
///
/// * `image_tag` — Docker image to run.
/// * `extra_args` — Additional `docker run` arguments inserted before the
///   image name, e.g. `&["-e", "MY_VAR=value"]`.
/// * `timeout_secs` — Maximum number of seconds to wait before forcibly
///   stopping the container.
///
/// # Errors
///
/// Returns a human-readable `String` when the `docker run` process cannot be
/// spawned.  A timeout does *not* return an error; the captured output up to
/// that point is returned with exit code `-1`.
pub fn docker_run_headless(
    image_tag: &str,
    extra_args: &[&str],
    timeout_secs: u64,
) -> Result<DockerTestResult, String> {
    // Assign a unique container name so parallel tests using the same image
    // don't collide.  The watchdog thread uses this name to `docker stop`.
    let seq = CONTAINER_COUNTER.fetch_add(1, Ordering::Relaxed);
    let container_name = format!(
        "fdemon-run-{}-{}-{}",
        image_tag.replace(':', "-"),
        std::process::id(),
        seq,
    );

    let mut cmd = Command::new("docker");
    cmd.args([
        "run",
        "--rm",
        "--init",
        "--platform",
        "linux/amd64",
        "--name",
        &container_name,
    ]);

    for arg in extra_args {
        cmd.arg(arg);
    }

    cmd.args([image_tag, "fdemon", "--headless", "/test-project"]);

    let child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn `docker run`: {e}"))?;

    run_with_timeout(child, &container_name, timeout_secs)
}

// ---------------------------------------------------------------------------
// docker run (arbitrary command)
// ---------------------------------------------------------------------------

/// Run an arbitrary command inside a Docker container and capture its output.
///
/// Useful for inspecting the container filesystem from tests, e.g. checking
/// that a version manager binary is on `$PATH`:
///
/// ```rust,no_run
/// let result = docker_exec("fdemon-test-fvm", &["which", "fvm"]).unwrap();
/// assert_eq!(result.exit_code, 0);
/// assert!(result.stdout.trim().ends_with("fvm"));
/// ```
///
/// # Arguments
///
/// * `image_tag` — Docker image to run.
/// * `command` — Command and its arguments to execute inside the container.
///
/// # Errors
///
/// Returns a human-readable `String` when the `docker run` process cannot be
/// spawned.
pub fn docker_exec(image_tag: &str, command: &[&str]) -> Result<DockerTestResult, String> {
    let seq = CONTAINER_COUNTER.fetch_add(1, Ordering::Relaxed);
    let container_name = format!(
        "fdemon-exec-{}-{}-{}",
        image_tag.replace(':', "-"),
        std::process::id(),
        seq,
    );

    let mut cmd = Command::new("docker");
    cmd.args([
        "run",
        "--rm",
        "--init",
        "--platform",
        "linux/amd64",
        "--name",
        &container_name,
    ]);
    cmd.arg(image_tag);
    cmd.args(command);

    let child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn `docker run` for exec: {e}"))?;

    // Use a generous default timeout for exec commands.
    const EXEC_TIMEOUT_SECS: u64 = 30;
    run_with_timeout(child, &container_name, EXEC_TIMEOUT_SECS)
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Wait for a Docker child process to finish, killing the named container after
/// `timeout_secs` if it has not exited naturally by then.
///
/// Returns a [`DockerTestResult`] with exit code `-1` when the timeout fires.
fn run_with_timeout(
    child: Child,
    container_name: &str,
    timeout_secs: u64,
) -> Result<DockerTestResult, String> {
    // Share the child process ID with the watchdog thread so it can send a kill
    // signal via `docker stop`.
    let container_name = container_name.to_string();

    // The watchdog uses a flag to avoid killing a container that already exited.
    let killed = Arc::new(Mutex::new(false));
    let killed_clone = Arc::clone(&killed);

    // Spawn a watchdog thread that stops the container after the timeout.
    let watchdog = thread::spawn(move || {
        thread::sleep(Duration::from_secs(timeout_secs));
        let mut killed_guard = killed_clone.lock().expect("mutex poisoned");
        if !*killed_guard {
            *killed_guard = true;
            // `docker stop` sends SIGTERM then SIGKILL; use a short grace period.
            let _ = Command::new("docker")
                .args(["stop", "--time", "2", &container_name])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
    });

    let output = child
        .wait_with_output()
        .map_err(|e| format!("Failed to wait for Docker container: {e}"))?;

    // Signal the watchdog that we no longer need it to fire.
    {
        let mut killed_guard = killed.lock().expect("mutex poisoned");
        *killed_guard = true;
    }

    // The watchdog thread will exit on its own; we don't need to join it.
    drop(watchdog);

    let exit_code = output.status.code().unwrap_or(-1);
    Ok(DockerTestResult {
        exit_code,
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

// ---------------------------------------------------------------------------
// Smoke test (ignored by default — requires Docker)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that the Docker infrastructure plumbing works end-to-end.
    ///
    /// Builds the base image (no Flutter SDK installed) and runs `fdemon
    /// --headless`, expecting an error event for "No Flutter SDK found".
    ///
    /// Run with:
    /// ```bash
    /// cargo test --test sdk_detection docker_infrastructure -- --ignored
    /// ```
    #[test]
    #[ignore = "requires Docker — run with `cargo test -- --ignored`"]
    fn test_docker_infrastructure_works() {
        if !docker_available() {
            eprintln!("Docker daemon not available; skipping");
            return;
        }

        let root = project_root();
        let tag = "fdemon-test-base";

        docker_build("tests/docker/base.Dockerfile", tag, &root)
            .expect("base Dockerfile should build successfully");

        let result =
            docker_run_headless(tag, &[], 60).expect("docker run should not fail to start");

        // fdemon exits non-zero when no Flutter SDK is found, so we accept any
        // exit code here; we just need to inspect the NDJSON events.
        let has_error_event = result.stdout.lines().any(|line| {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                v.get("event").and_then(|e| e.as_str()) == Some("error")
            } else {
                false
            }
        });

        assert!(
            has_error_event,
            "Expected an 'error' NDJSON event in stdout.\nstdout:\n{}\nstderr:\n{}",
            result.stdout, result.stderr,
        );
    }

    #[test]
    fn test_project_root_returns_valid_path() {
        let root = project_root();
        assert!(
            root.exists(),
            "project_root() should return a path that exists: {root:?}"
        );
        assert!(
            root.join("Cargo.toml").exists(),
            "project_root() should point at the workspace root (Cargo.toml not found): {root:?}"
        );
    }

    #[test]
    fn test_docker_available_does_not_panic() {
        // We can't assert a specific value because Docker may or may not be
        // installed in the current environment.  We just verify it doesn't panic.
        let _ = docker_available();
    }
}
