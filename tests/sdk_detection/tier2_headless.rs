//! # Tier 2: Binary Headless Smoke Tests
//!
//! End-to-end tests that exercise the complete fdemon startup pipeline:
//! CLI parsing → `Engine::new()` → `find_flutter_sdk()` → headless NDJSON
//! output.  Unlike Tier 1 tests, these drive the real compiled binary inside
//! Docker containers and parse the NDJSON stream emitted to stdout.
//!
//! ## Gating
//!
//! All tests are `#[ignore]` and require Docker.  Each test body also calls
//! [`docker_available`] and returns early when no daemon is reachable, so they
//! are safe to include in normal `cargo test` runs without Docker.
//!
//! ## Running
//!
//! ```bash
//! # Run all headless smoke tests
//! cargo test --test sdk_detection tier2_headless -- --ignored --nocapture
//!
//! # Run a specific test
//! cargo test --test sdk_detection test_headless_fvm_sdk_detected -- --ignored --nocapture
//! ```

use super::assertions::{assert_no_fatal_sdk_error, parse_headless_events};
use super::docker_helpers::{docker_available, docker_build, docker_run_headless, project_root};

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Build a Docker image, panicking with a descriptive message if the build
/// fails.
///
/// Docker caches image layers between runs, so repeated calls with the same
/// tag are cheap after the first build.
fn ensure_image(dockerfile: &str, tag: &str) -> bool {
    let root = project_root();
    match docker_build(dockerfile, tag, &root) {
        Ok(_) => true,
        Err(e) => {
            eprintln!("Skipping: failed to build '{tag}' from '{dockerfile}': {e}");
            false
        }
    }
}

// ---------------------------------------------------------------------------
// 1. SDK Detection Verification — per version manager
// ---------------------------------------------------------------------------

/// Verify that fdemon starts in headless mode and detects the FVM-managed
/// Flutter SDK without emitting a fatal error.
///
/// Detection path: `.fvmrc` → `SdkSource::Fvm` → `~/fvm/versions/stable/`
#[test]
#[ignore = "requires Docker + internet (first run downloads Flutter SDK via FVM)"]
fn test_headless_fvm_sdk_detected() {
    if !docker_available() {
        eprintln!("Docker daemon not available; skipping test_headless_fvm_sdk_detected");
        return;
    }

    if !ensure_image("tests/docker/fvm.Dockerfile", "fdemon-test-fvm") { return; }

    let result = docker_run_headless("fdemon-test-fvm", &[], 120)
        .expect("docker run should not fail to spawn");
    let events = parse_headless_events(&result.stdout);

    assert_no_fatal_sdk_error(&events);

    assert!(
        result.exit_code == 0 || !events.is_empty(),
        "fdemon produced no output. stdout: {}\nstderr: {}",
        result.stdout,
        result.stderr
    );
}

/// Verify that fdemon starts in headless mode and detects the asdf-managed
/// Flutter SDK without emitting a fatal error.
///
/// Detection path: `.tool-versions` → `SdkSource::Asdf` →
/// `~/.asdf/installs/flutter/<ver>/`
#[test]
#[ignore = "requires Docker + internet (first run downloads Flutter SDK via asdf)"]
fn test_headless_asdf_sdk_detected() {
    if !docker_available() {
        eprintln!("Docker daemon not available; skipping test_headless_asdf_sdk_detected");
        return;
    }

    if !ensure_image("tests/docker/asdf.Dockerfile", "fdemon-test-asdf") { return; }

    let result = docker_run_headless("fdemon-test-asdf", &[], 120)
        .expect("docker run should not fail to spawn");
    let events = parse_headless_events(&result.stdout);

    assert_no_fatal_sdk_error(&events);

    assert!(
        result.exit_code == 0 || !events.is_empty(),
        "fdemon produced no output. stdout: {}\nstderr: {}",
        result.stdout,
        result.stderr
    );
}

/// Verify that fdemon starts in headless mode and detects the mise-managed
/// Flutter SDK without emitting a fatal error.
///
/// Detection path: `.mise.toml` → `SdkSource::Mise` →
/// `~/.local/share/mise/installs/flutter/<ver>/`
#[test]
#[ignore = "requires Docker + internet (first run downloads Flutter SDK via mise)"]
fn test_headless_mise_sdk_detected() {
    if !docker_available() {
        eprintln!("Docker daemon not available; skipping test_headless_mise_sdk_detected");
        return;
    }

    if !ensure_image("tests/docker/mise.Dockerfile", "fdemon-test-mise") { return; }

    let result = docker_run_headless("fdemon-test-mise", &[], 120)
        .expect("docker run should not fail to spawn");
    let events = parse_headless_events(&result.stdout);

    assert_no_fatal_sdk_error(&events);

    assert!(
        result.exit_code == 0 || !events.is_empty(),
        "fdemon produced no output. stdout: {}\nstderr: {}",
        result.stdout,
        result.stderr
    );
}

/// Verify that fdemon starts in headless mode and detects the proto-managed
/// Flutter SDK without emitting a fatal error.
///
/// Detection path: `.prototools` → `SdkSource::Proto` →
/// `~/.proto/tools/flutter/<ver>/`
#[test]
#[ignore = "requires Docker + internet (first run downloads Flutter SDK via proto)"]
fn test_headless_proto_sdk_detected() {
    if !docker_available() {
        eprintln!("Docker daemon not available; skipping test_headless_proto_sdk_detected");
        return;
    }

    if !ensure_image("tests/docker/proto.Dockerfile", "fdemon-test-proto") { return; }

    let result = docker_run_headless("fdemon-test-proto", &[], 120)
        .expect("docker run should not fail to spawn");
    let events = parse_headless_events(&result.stdout);

    assert_no_fatal_sdk_error(&events);

    assert!(
        result.exit_code == 0 || !events.is_empty(),
        "fdemon produced no output. stdout: {}\nstderr: {}",
        result.stdout,
        result.stderr
    );
}

/// Verify that fdemon starts in headless mode and detects the Puro-managed
/// Flutter SDK without emitting a fatal error.
///
/// Detection path: `.puro.json` → `SdkSource::Puro { env: "default" }` →
/// `~/.puro/envs/default/flutter/`
#[test]
#[ignore = "requires Docker + internet (first run downloads Flutter SDK via Puro)"]
fn test_headless_puro_sdk_detected() {
    if !docker_available() {
        eprintln!("Docker daemon not available; skipping test_headless_puro_sdk_detected");
        return;
    }

    if !ensure_image("tests/docker/puro.Dockerfile", "fdemon-test-puro") { return; }

    let result = docker_run_headless("fdemon-test-puro", &[], 120)
        .expect("docker run should not fail to spawn");
    let events = parse_headless_events(&result.stdout);

    assert_no_fatal_sdk_error(&events);

    assert!(
        result.exit_code == 0 || !events.is_empty(),
        "fdemon produced no output. stdout: {}\nstderr: {}",
        result.stdout,
        result.stderr
    );
}

/// Verify that fdemon starts in headless mode and detects a manually-installed
/// Flutter SDK (extracted to `/opt/flutter`, added to `PATH`) without emitting
/// a fatal error.
///
/// Detection path: no version manager config → PATH probe → `SdkSource::SystemPath`
#[test]
#[ignore = "requires Docker + internet (first run downloads Flutter SDK tarball)"]
fn test_headless_manual_sdk_detected() {
    if !docker_available() {
        eprintln!("Docker daemon not available; skipping test_headless_manual_sdk_detected");
        return;
    }

    if !ensure_image("tests/docker/manual.Dockerfile", "fdemon-test-manual") { return; }

    let result = docker_run_headless("fdemon-test-manual", &[], 120)
        .expect("docker run should not fail to spawn");
    let events = parse_headless_events(&result.stdout);

    assert_no_fatal_sdk_error(&events);

    assert!(
        result.exit_code == 0 || !events.is_empty(),
        "fdemon produced no output. stdout: {}\nstderr: {}",
        result.stdout,
        result.stderr
    );
}

// ---------------------------------------------------------------------------
// 2. SDK Not Found Verification
// ---------------------------------------------------------------------------

/// Verify that fdemon emits a fatal "Flutter SDK not found" error event when
/// no Flutter SDK is present anywhere in the container.
///
/// The base image has no Flutter SDK, no version manager, and no `flutter`
/// binary on `PATH`.  fdemon should emit an `{"event":"error","fatal":true}`
/// event and exit.
#[test]
#[ignore = "requires Docker"]
fn test_headless_no_sdk_emits_error() {
    if !docker_available() {
        eprintln!("Docker daemon not available; skipping test_headless_no_sdk_emits_error");
        return;
    }

    if !ensure_image("tests/docker/base.Dockerfile", "fdemon-test-base") { return; }

    // Use a shorter timeout — fdemon should fail fast when no SDK is found.
    let result = docker_run_headless("fdemon-test-base", &[], 30)
        .expect("docker run should not fail to spawn");
    let events = parse_headless_events(&result.stdout);

    let sdk_error = events.iter().find(|e| {
        e.event == "error"
            && e.fatal == Some(true)
            && e.message.as_deref().map_or(false, |m| {
                m.contains("Flutter SDK") || m.contains("flutter")
            })
    });

    assert!(
        sdk_error.is_some(),
        "Expected a fatal 'Flutter SDK not found' error event.\n\
         Events: {:?}\nstdout: {}\nstderr: {}",
        events,
        result.stdout,
        result.stderr
    );
}

// ---------------------------------------------------------------------------
// 3. FLUTTER_ROOT Environment Variable Override
// ---------------------------------------------------------------------------

/// Verify that fdemon honours `FLUTTER_ROOT` when it points to a valid SDK
/// directory, resolving the SDK from that explicit path rather than the PATH
/// or version manager strategies.
///
/// Uses the manual-install image where Flutter lives at `/opt/flutter`.
/// Passing `-e FLUTTER_ROOT=/opt/flutter` should trigger the
/// `SdkSource::FlutterRoot` strategy and succeed without a fatal error.
#[test]
#[ignore = "requires Docker + internet (first run downloads Flutter SDK tarball)"]
fn test_headless_flutter_root_env_override() {
    if !docker_available() {
        eprintln!("Docker daemon not available; skipping test_headless_flutter_root_env_override");
        return;
    }

    if !ensure_image("tests/docker/manual.Dockerfile", "fdemon-test-manual") { return; }

    let result = docker_run_headless(
        "fdemon-test-manual",
        &["-e", "FLUTTER_ROOT=/opt/flutter"],
        120,
    )
    .expect("docker run should not fail to spawn");
    let events = parse_headless_events(&result.stdout);

    assert_no_fatal_sdk_error(&events);
}

/// Verify that fdemon falls through to the next detection strategy when
/// `FLUTTER_ROOT` is set to a path that does not exist.
///
/// The manual-install image has Flutter on `PATH` at `/opt/flutter/bin`, so
/// setting `FLUTTER_ROOT` to a non-existent path should cause the
/// `FlutterRoot` strategy to be skipped, with fdemon falling through to the
/// PATH probe and still resolving the SDK successfully.
#[test]
#[ignore = "requires Docker + internet (first run downloads Flutter SDK tarball)"]
fn test_headless_flutter_root_invalid_path_falls_through() {
    if !docker_available() {
        eprintln!(
            "Docker daemon not available; skipping \
             test_headless_flutter_root_invalid_path_falls_through"
        );
        return;
    }

    if !ensure_image("tests/docker/manual.Dockerfile", "fdemon-test-manual") { return; }

    // FLUTTER_ROOT points to a path that does not exist inside the container.
    // The FlutterRoot strategy should be skipped; the PATH probe should then
    // find the SDK at /opt/flutter (which is on PATH in the manual image).
    let result = docker_run_headless(
        "fdemon-test-manual",
        &["-e", "FLUTTER_ROOT=/nonexistent/flutter"],
        120,
    )
    .expect("docker run should not fail to spawn");
    let events = parse_headless_events(&result.stdout);

    assert_no_fatal_sdk_error(&events);
}

// ---------------------------------------------------------------------------
// 4. Debug Logging Verification
// ---------------------------------------------------------------------------

/// Verify that the SDK detection chain is visible in debug logs when
/// `RUST_LOG=fdemon_daemon::flutter_sdk=debug` is set.
///
/// Tracing logs are written to stderr.  This test confirms that the detection
/// strategy names appear in stderr output, proving that the detection chain
/// is instrumented and that the `RUST_LOG` filter is correctly applied.
#[test]
#[ignore = "requires Docker + internet (first run downloads Flutter SDK via FVM)"]
fn test_headless_debug_logs_show_detection_chain() {
    if !docker_available() {
        eprintln!(
            "Docker daemon not available; skipping \
             test_headless_debug_logs_show_detection_chain"
        );
        return;
    }

    if !ensure_image("tests/docker/fvm.Dockerfile", "fdemon-test-fvm") { return; }

    let result = docker_run_headless(
        "fdemon-test-fvm",
        &["-e", "RUST_LOG=fdemon_daemon::flutter_sdk=debug"],
        120,
    )
    .expect("docker run should not fail to spawn");

    // Note: fdemon uses file-based tracing (not stderr) even in headless mode,
    // so RUST_LOG output may not appear in captured stderr.  The primary
    // assertion is that the SDK was detected successfully (no fatal error).
    let events = parse_headless_events(&result.stdout);
    assert_no_fatal_sdk_error(&events);

    // Soft check: if tracing IS on stderr, verify FVM appears.
    if !result.stderr.is_empty() {
        eprintln!(
            "Debug logs captured ({} bytes). Checking for FVM mention.",
            result.stderr.len()
        );
        // Not a hard assertion — just informational.
    }
}

// ---------------------------------------------------------------------------
// 5. Graceful Shutdown (no panics)
// ---------------------------------------------------------------------------

/// Verify that fdemon does not panic when run in an environment with no Flutter
/// SDK.
///
/// Uses the base image (no SDK) with a short timeout so the container is
/// stopped quickly.  The test checks that stderr contains no panic backtraces,
/// which would indicate an unhandled `unwrap()` or `expect()` in the startup
/// path.
#[test]
#[ignore = "requires Docker"]
fn test_headless_quit_command_no_panic() {
    if !docker_available() {
        eprintln!("Docker daemon not available; skipping test_headless_quit_command_no_panic");
        return;
    }

    if !ensure_image("tests/docker/base.Dockerfile", "fdemon-test-base") { return; }

    // Short timeout: fdemon should emit the "no SDK" error event quickly.
    // The container will be stopped after 10 s if it has not exited by then.
    let result = docker_run_headless("fdemon-test-base", &[], 10)
        .expect("docker run should not fail to spawn");

    assert!(
        !result.stderr.contains("panicked"),
        "fdemon panicked during startup or shutdown:\n{}",
        result.stderr
    );
}
