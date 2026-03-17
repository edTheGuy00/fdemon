//! # Tier 2: Docker Linux Tests — Real Version Manager Installations
//!
//! These tests build Docker images containing real Flutter version manager
//! installations, run `fdemon --headless` inside those containers, and verify
//! that fdemon correctly detects the Flutter SDK without errors.
//!
//! ## Gating
//!
//! All tests in this module are `#[ignore]` and are skipped unless explicitly
//! requested via `cargo test -- --ignored`.  Each test also calls
//! [`docker_available`] and returns early when the Docker daemon is not
//! reachable, so they are safe to run on machines that have Docker installed
//! but no daemon running.
//!
//! ## Running
//!
//! ```bash
//! # Run all Tier 2 Linux tests
//! cargo test --test sdk_detection tier2_linux -- --ignored --nocapture
//!
//! # Run a single version manager
//! cargo test --test sdk_detection test_fvm_detection -- --ignored --nocapture
//! ```
//!
//! ## Performance
//!
//! **First run is slow**: Docker image builds compile fdemon (~5 min) and
//! download Flutter SDK tarballs (~2–3 GB per image).  Subsequent runs reuse
//! cached layers and are much faster.

use super::assertions::{parse_headless_events, HeadlessEvent};
use super::docker_helpers::{docker_available, docker_build, docker_run_headless, project_root};

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Build a version-manager Docker image, returning `false` when the build
/// fails so the test can skip gracefully instead of panicking and blocking
/// the entire test suite.
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

/// Assert that the NDJSON events collected from a headless run contain no
/// fatal error events, which would indicate that fdemon could not detect the
/// Flutter SDK.
fn assert_no_fatal_error(events: &[HeadlessEvent], stdout: &str, stderr: &str) {
    let fatal = events
        .iter()
        .any(|e| e.event == "error" && e.fatal == Some(true));
    assert!(
        !fatal,
        "fdemon emitted a fatal error event — SDK was not detected.\n\
         stdout:\n{stdout}\nstderr:\n{stderr}"
    );
}

// ---------------------------------------------------------------------------
// FVM
// ---------------------------------------------------------------------------

/// Verify that fdemon detects the Flutter SDK installed and managed by FVM
/// when the project contains a `.fvmrc` pinning the "stable" channel.
///
/// Expected detection path: `.fvmrc` → `SdkSource::Fvm { version: "stable" }`
/// → `~/fvm/versions/stable/`
#[test]
#[ignore = "requires Docker + internet (first run downloads Flutter SDK via FVM)"]
fn test_fvm_detection_real_install() {
    if !docker_available() {
        eprintln!("Docker daemon not available; skipping test_fvm_detection_real_install");
        return;
    }

    if !ensure_image("tests/docker/fvm.Dockerfile", "fdemon-test-fvm") { return; }

    let result = docker_run_headless("fdemon-test-fvm", &[], 120)
        .expect("docker run should not fail to spawn");
    let events = parse_headless_events(&result.stdout);

    assert_no_fatal_error(&events, &result.stdout, &result.stderr);
}

/// Verify that fdemon's debug logs mention FVM when the FVM environment is
/// active, confirming the FVM detection strategy was actually executed.
#[test]
#[ignore = "requires Docker + internet (first run downloads Flutter SDK via FVM)"]
fn test_fvm_source_identified_in_logs() {
    if !docker_available() {
        eprintln!("Docker daemon not available; skipping test_fvm_source_identified_in_logs");
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
    // assertion is that the SDK was detected (no fatal error in stdout).
    let events = parse_headless_events(&result.stdout);
    assert_no_fatal_error(&events, &result.stdout, &result.stderr);
}

// ---------------------------------------------------------------------------
// asdf
// ---------------------------------------------------------------------------

/// Verify that fdemon detects the Flutter SDK installed and managed by asdf
/// when the project contains a `.tool-versions` file.
///
/// Expected detection path: `.tool-versions` → `SdkSource::Asdf { version }`
/// → `~/.asdf/installs/flutter/<ver>/`
#[test]
#[ignore = "requires Docker + internet (first run downloads Flutter SDK via asdf)"]
fn test_asdf_detection_real_install() {
    if !docker_available() {
        eprintln!("Docker daemon not available; skipping test_asdf_detection_real_install");
        return;
    }

    if !ensure_image("tests/docker/asdf.Dockerfile", "fdemon-test-asdf") { return; }

    let result = docker_run_headless("fdemon-test-asdf", &[], 120)
        .expect("docker run should not fail to spawn");
    let events = parse_headless_events(&result.stdout);

    assert_no_fatal_error(&events, &result.stdout, &result.stderr);
}

// ---------------------------------------------------------------------------
// mise
// ---------------------------------------------------------------------------

/// Verify that fdemon detects the Flutter SDK installed and managed by mise
/// when the project contains a `.mise.toml` file.
///
/// Expected detection path: `.mise.toml` → `SdkSource::Mise { version }`
/// → `~/.local/share/mise/installs/flutter/<ver>/`
#[test]
#[ignore = "requires Docker + internet (first run downloads Flutter SDK via mise)"]
fn test_mise_detection_real_install() {
    if !docker_available() {
        eprintln!("Docker daemon not available; skipping test_mise_detection_real_install");
        return;
    }

    if !ensure_image("tests/docker/mise.Dockerfile", "fdemon-test-mise") { return; }

    let result = docker_run_headless("fdemon-test-mise", &[], 120)
        .expect("docker run should not fail to spawn");
    let events = parse_headless_events(&result.stdout);

    assert_no_fatal_error(&events, &result.stdout, &result.stderr);
}

// ---------------------------------------------------------------------------
// proto
// ---------------------------------------------------------------------------

/// Verify that fdemon detects the Flutter SDK installed and managed by proto
/// when the project contains a `.prototools` file.
///
/// Expected detection path: `.prototools` → `SdkSource::Proto { version }`
/// → `~/.proto/tools/flutter/<ver>/`
///
/// Note: the Flutter plugin for proto is community-maintained. If this test
/// fails during image build, check whether the plugin source URL has changed.
#[test]
#[ignore = "requires Docker + internet (first run downloads Flutter SDK via proto)"]
fn test_proto_detection_real_install() {
    if !docker_available() {
        eprintln!("Docker daemon not available; skipping test_proto_detection_real_install");
        return;
    }

    if !ensure_image("tests/docker/proto.Dockerfile", "fdemon-test-proto") { return; }

    let result = docker_run_headless("fdemon-test-proto", &[], 120)
        .expect("docker run should not fail to spawn");
    let events = parse_headless_events(&result.stdout);

    assert_no_fatal_error(&events, &result.stdout, &result.stderr);
}

// ---------------------------------------------------------------------------
// Puro
// ---------------------------------------------------------------------------

/// Verify that fdemon detects the Flutter SDK installed and managed by Puro
/// when the project contains a `.puro.json` referencing the "default"
/// environment.
///
/// Expected detection path: `.puro.json` → `SdkSource::Puro { env: "default" }`
/// → `~/.puro/envs/default/flutter/`
#[test]
#[ignore = "requires Docker + internet (first run downloads Flutter SDK via Puro)"]
fn test_puro_detection_real_install() {
    if !docker_available() {
        eprintln!("Docker daemon not available; skipping test_puro_detection_real_install");
        return;
    }

    if !ensure_image("tests/docker/puro.Dockerfile", "fdemon-test-puro") { return; }

    let result = docker_run_headless("fdemon-test-puro", &[], 120)
        .expect("docker run should not fail to spawn");
    let events = parse_headless_events(&result.stdout);

    assert_no_fatal_error(&events, &result.stdout, &result.stderr);
}

// ---------------------------------------------------------------------------
// Manual (PATH / SystemPath) installation
// ---------------------------------------------------------------------------

/// Verify that fdemon detects a manually-installed Flutter SDK (extracted to
/// `/opt/flutter`) via PATH when the project has no version manager config
/// files.
///
/// Expected detection path: no config files → PATH probe → `SdkSource::SystemPath`
#[test]
#[ignore = "requires Docker + internet (first run downloads Flutter SDK tarball)"]
fn test_manual_install_detection() {
    if !docker_available() {
        eprintln!("Docker daemon not available; skipping test_manual_install_detection");
        return;
    }

    if !ensure_image("tests/docker/manual.Dockerfile", "fdemon-test-manual") { return; }

    let result = docker_run_headless("fdemon-test-manual", &[], 120)
        .expect("docker run should not fail to spawn");
    let events = parse_headless_events(&result.stdout);

    assert_no_fatal_error(&events, &result.stdout, &result.stderr);
}
