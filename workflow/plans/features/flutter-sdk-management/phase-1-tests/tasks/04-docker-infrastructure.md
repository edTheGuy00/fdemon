## Task: Docker Infrastructure

**Objective**: Create the Docker build/run infrastructure that Tier 2 tests use to spin up containers with real version manager installations. Includes a shared base Dockerfile, helper functions for building/running Docker images from Rust tests, and the gating mechanism.

**Depends on**: None

### Scope

- `tests/docker/base.Dockerfile`: Multi-stage Dockerfile with Rust builder + Debian runtime base
- `tests/sdk_detection/docker_helpers.rs`: Rust helpers for invoking `docker build` and `docker run` from tests
- Integration with `#[ignore]` gating

### Details

#### Base Dockerfile (Multi-Stage)

```dockerfile
# tests/docker/base.Dockerfile
# Stage 1: Build fdemon binary
FROM rust:1.83-bookworm AS builder
WORKDIR /build
COPY . .
RUN cargo build --release
# Binary at /build/target/release/fdemon

# Stage 2: Minimal runtime
FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl git unzip xz-utils \
    && rm -rf /var/lib/apt/lists/*

# Create test project structure
RUN mkdir -p /test-project && \
    printf 'name: test_project\ndescription: Test project for SDK detection\nenvironment:\n  sdk: ">=3.0.0 <4.0.0"\n' \
    > /test-project/pubspec.yaml

COPY --from=builder /build/target/release/fdemon /usr/local/bin/fdemon
```

Each version manager Dockerfile extends the `runtime` stage with manager-specific installation.

#### Docker Helper Functions

```rust
// tests/sdk_detection/docker_helpers.rs

use std::process::{Command, Output};
use std::path::Path;

/// Result of a Docker test run
pub struct DockerTestResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

/// Build a Docker image from a Dockerfile.
/// Uses `--cache-from` for layer reuse across runs.
/// Returns the image tag.
pub fn docker_build(
    dockerfile: &str,       // relative path from project root, e.g. "tests/docker/fvm.Dockerfile"
    tag: &str,              // e.g. "fdemon-test-fvm"
    project_root: &Path,    // build context = project root (for COPY . .)
) -> Result<String, String> {
    let output = Command::new("docker")
        .args(&["build", "-f", dockerfile, "-t", tag, "."])
        .current_dir(project_root)
        .output()
        .map_err(|e| format!("docker build failed to start: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "docker build failed (exit {}):\n{}",
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(tag.to_string())
}

/// Run fdemon headless in a Docker container and capture output.
/// The container runs `fdemon --headless /test-project` by default.
pub fn docker_run_headless(
    image_tag: &str,
    extra_args: &[&str],    // extra docker run args (e.g., env vars)
    timeout_secs: u64,
) -> Result<DockerTestResult, String> {
    let mut cmd = Command::new("docker");
    cmd.args(&["run", "--rm"]);
    for arg in extra_args {
        cmd.arg(arg);
    }
    cmd.args(&[image_tag, "fdemon", "--headless", "/test-project"]);

    // TODO: implement timeout using std::thread + process kill
    let output = cmd.output()
        .map_err(|e| format!("docker run failed to start: {}", e))?;

    Ok(DockerTestResult {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

/// Run an arbitrary command in a Docker container (not fdemon).
/// Useful for inspecting filesystem layout, running `which flutter`, etc.
pub fn docker_exec(
    image_tag: &str,
    command: &[&str],
) -> Result<DockerTestResult, String> { ... }

/// Check if Docker daemon is available.
/// Used to skip Docker tests gracefully when Docker isn't running.
pub fn docker_available() -> bool {
    Command::new("docker")
        .args(&["info", "--format", "{{.ServerVersion}}"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Get the project root path (for build context).
/// Walks up from CARGO_MANIFEST_DIR to find the workspace root.
pub fn project_root() -> PathBuf {
    // Use CARGO_MANIFEST_DIR env var set by cargo during tests
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR not set");
    PathBuf::from(manifest_dir)
}
```

#### Gating Mechanism

Docker tests use `#[ignore]` so they're skipped by default:

```rust
#[test]
#[ignore = "requires Docker — run with `cargo test -- --ignored`"]
fn test_fvm_detection_docker() {
    if !docker_available() {
        eprintln!("Docker not available, skipping");
        return;
    }
    // ... test body
}
```

To run Docker tests:
```bash
# Run only Docker tests
cargo test --test sdk_detection -- --ignored

# Run all tests including Docker
cargo test --test sdk_detection -- --include-ignored
```

#### Docker Image Caching

Docker layer caching handles this naturally:
- First build is slow (compiles fdemon + installs Flutter SDK)
- Subsequent builds reuse cached layers if source hasn't changed
- Each Dockerfile uses a `builder` stage that is shared across images (same base + same binary)

To force rebuild: `docker build --no-cache ...`

#### `.dockerignore` for Build Context

Create a `.dockerignore` at project root (or extend existing):

```
target/
.git/
*.md
workflow/
tests/docker/
.fdemon/
```

This keeps the build context small and speeds up `COPY . .`.

### Acceptance Criteria

1. `docker_build()` successfully builds the base Dockerfile
2. `docker_run_headless()` returns captured stdout/stderr from fdemon
3. `docker_available()` correctly detects Docker daemon presence
4. Docker tests are skipped by default (`cargo test` doesn't run them)
5. Docker tests run with `cargo test -- --ignored`
6. Build context is minimal (`.dockerignore` configured)
7. Helper functions provide clear error messages on Docker failures
8. Timeout mechanism prevents hung Docker containers

### Testing

Verify the infrastructure with a minimal smoke test:

```rust
#[test]
#[ignore = "requires Docker"]
fn test_docker_infrastructure_works() {
    if !docker_available() { return; }

    let root = project_root();
    // Build just the base image (no version manager)
    docker_build("tests/docker/base.Dockerfile", "fdemon-test-base", &root).unwrap();

    // Run fdemon — should get "No Flutter SDK found" since base has no Flutter
    let result = docker_run_headless("fdemon-test-base", &[], 30).unwrap();
    let events = parse_headless_events(&result.stdout);
    assert!(events.iter().any(|e| e.event == "error" && e.message.as_deref() == Some("No Flutter SDK found")));
}
```

### Notes

- **Build context is the project root** — Docker needs access to the full source tree for `cargo build` inside the container. The `.dockerignore` keeps it manageable.
- **Binary is compiled inside Docker** — this avoids cross-compilation issues (macOS → Linux). It's slower on first build but subsequent builds use cached layers.
- **No `testcontainers-rs` dependency** — raw `std::process::Command` with `docker` CLI is simpler and more transparent for this use case. The version manager images are built-and-run, not long-running services.
- **Timeout handling** is important — if fdemon hangs (e.g., waiting for Flutter process that can't start), the Docker container should be killed after a reasonable timeout. Implement via `std::thread::spawn` + `process.kill()` or `docker run --timeout`.
- **Consider `docker run --init`** — ensures fdemon receives signals properly inside the container.
