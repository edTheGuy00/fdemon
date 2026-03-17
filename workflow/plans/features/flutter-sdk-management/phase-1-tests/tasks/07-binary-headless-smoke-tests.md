## Task: Tier 2 — Binary Headless Smoke Tests

**Objective**: Verify the full fdemon startup → SDK detection → headless output pipeline by running the compiled binary in Docker containers and parsing the NDJSON output to confirm correct SDK resolution.

**Depends on**: 01-shared-test-infrastructure, 04-docker-infrastructure

### Scope

- `tests/sdk_detection/tier2_headless.rs`: End-to-end binary tests

### Details

These tests exercise the **complete code path**: CLI parsing → `Engine::new()` → `find_flutter_sdk()` → `state.resolved_sdk` population → headless NDJSON output. Unlike Tier 1 tests (which call library functions directly), these test the real binary.

#### Test Categories

##### 1. SDK Detection Verification via Headless Output

For each version manager Docker image (built in Task 05), run fdemon headless and verify the NDJSON events indicate successful SDK detection:

```rust
#[test]
#[ignore = "requires Docker + internet"]
fn test_headless_fvm_sdk_detected() {
    if !docker_available() { return; }
    ensure_image("tests/docker/fvm.Dockerfile", "fdemon-test-fvm");

    let result = docker_run_headless("fdemon-test-fvm", &[], 60).unwrap();
    let events = parse_headless_events(&result.stdout);

    // Should NOT have a fatal "No Flutter SDK found" error
    assert_no_fatal_sdk_error(&events);

    // Should have device discovery events (proves SDK was resolved)
    // Device discovery requires a working flutter binary, which
    // proves the SDK was found and is functional
    assert!(result.exit_code == 0 || events.len() > 0,
        "fdemon produced no output. stdout: {}\nstderr: {}",
        result.stdout, result.stderr);
}

#[test]
#[ignore = "requires Docker + internet"]
fn test_headless_asdf_sdk_detected() { ... }

#[test]
#[ignore = "requires Docker + internet"]
fn test_headless_mise_sdk_detected() { ... }

#[test]
#[ignore = "requires Docker + internet"]
fn test_headless_proto_sdk_detected() { ... }

#[test]
#[ignore = "requires Docker + internet"]
fn test_headless_puro_sdk_detected() { ... }

#[test]
#[ignore = "requires Docker + internet"]
fn test_headless_manual_sdk_detected() { ... }
```

##### 2. SDK Not Found Verification

```rust
#[test]
#[ignore = "requires Docker"]
fn test_headless_no_sdk_emits_error() {
    if !docker_available() { return; }
    ensure_image("tests/docker/base.Dockerfile", "fdemon-test-base");

    let result = docker_run_headless("fdemon-test-base", &[], 30).unwrap();
    let events = parse_headless_events(&result.stdout);

    // Should emit a fatal error about SDK not found
    let sdk_error = events.iter().find(|e|
        e.event == "error" &&
        e.fatal == Some(true) &&
        e.message.as_deref().map_or(false, |m| m.contains("Flutter SDK"))
    );
    assert!(sdk_error.is_some(),
        "Expected fatal 'Flutter SDK not found' error.\nEvents: {:?}\nstdout: {}\nstderr: {}",
        events, result.stdout, result.stderr);
}
```

##### 3. FLUTTER_ROOT Override in Docker

```rust
#[test]
#[ignore = "requires Docker + internet"]
fn test_headless_flutter_root_env_override() {
    if !docker_available() { return; }
    // Use the manual install image but override FLUTTER_ROOT
    ensure_image("tests/docker/manual.Dockerfile", "fdemon-test-manual");

    // FLUTTER_ROOT is set to the known SDK location
    let result = docker_run_headless(
        "fdemon-test-manual",
        &["-e", "FLUTTER_ROOT=/opt/flutter"],
        60,
    ).unwrap();
    let events = parse_headless_events(&result.stdout);

    assert_no_fatal_sdk_error(&events);
}

#[test]
#[ignore = "requires Docker + internet"]
fn test_headless_flutter_root_invalid_path() {
    if !docker_available() { return; }
    ensure_image("tests/docker/manual.Dockerfile", "fdemon-test-manual");

    // FLUTTER_ROOT set to non-existent path — should fall through to PATH
    let result = docker_run_headless(
        "fdemon-test-manual",
        &["-e", "FLUTTER_ROOT=/nonexistent/flutter"],
        60,
    ).unwrap();
    let events = parse_headless_events(&result.stdout);

    // Should still find SDK via PATH (manual install adds to PATH)
    assert_no_fatal_sdk_error(&events);
}
```

##### 4. Debug Logging Verification

```rust
#[test]
#[ignore = "requires Docker + internet"]
fn test_headless_debug_logs_show_detection_chain() {
    if !docker_available() { return; }
    ensure_image("tests/docker/fvm.Dockerfile", "fdemon-test-fvm");

    let result = docker_run_headless(
        "fdemon-test-fvm",
        &["-e", "RUST_LOG=fdemon_daemon::flutter_sdk=debug"],
        60,
    ).unwrap();

    // Debug logs go to stderr (via tracing)
    // Should see the detection chain with strategy names
    assert!(result.stderr.contains("Trying strategy") || result.stderr.contains("flutter_sdk"),
        "Expected SDK detection chain in debug logs.\nstderr: {}", result.stderr);

    // Should show which strategy succeeded
    assert!(result.stderr.to_lowercase().contains("fvm") || result.stderr.to_lowercase().contains("resolved"),
        "Expected FVM resolution in debug logs.\nstderr: {}", result.stderr);
}
```

##### 5. Graceful Shutdown

```rust
#[test]
#[ignore = "requires Docker"]
fn test_headless_quit_command() {
    if !docker_available() { return; }
    ensure_image("tests/docker/base.Dockerfile", "fdemon-test-base");

    // Run with a timeout — fdemon should exit cleanly
    // (base image has no SDK, so headless emits error and the event loop
    // continues until quit. The docker_run_headless timeout handles this.)
    let result = docker_run_headless("fdemon-test-base", &[], 10).unwrap();

    // Should not have crashed
    assert!(
        !result.stderr.contains("panicked"),
        "fdemon panicked:\n{}", result.stderr
    );
}
```

#### Assertion Helpers

Add to `assertions.rs`:

```rust
/// Assert that no fatal SDK-related error was emitted
pub fn assert_no_fatal_sdk_error(events: &[HeadlessEvent]) {
    let fatal_errors: Vec<_> = events.iter()
        .filter(|e| e.event == "error" && e.fatal == Some(true))
        .collect();

    assert!(fatal_errors.is_empty(),
        "Unexpected fatal errors: {:?}", fatal_errors);
}
```

### Acceptance Criteria

1. All 6 version manager environments produce successful SDK detection in headless mode
2. Base image (no Flutter) correctly emits "No Flutter SDK found" fatal error
3. `FLUTTER_ROOT` override works when passed as Docker env var
4. Invalid `FLUTTER_ROOT` falls through to next strategy
5. Debug logging shows the detection chain when `RUST_LOG` is set
6. fdemon doesn't panic in any scenario
7. Tests complete within 60-second timeout per container
8. All tests pass with `cargo test -- --ignored`

### Testing

```bash
# Run all headless smoke tests
cargo test --test sdk_detection tier2_headless -- --ignored --nocapture

# Run a specific test
cargo test --test sdk_detection test_headless_fvm -- --ignored --nocapture

# Run with verbose output to see Docker build progress
cargo test --test sdk_detection tier2_headless -- --ignored --nocapture 2>&1
```

### Notes

- **These tests reuse Docker images from Task 05** — the `ensure_image()` calls should be no-ops if the images are already built. Consider a shared setup that builds all images once.
- **Headless output parsing**: fdemon's headless mode emits NDJSON to stdout and tracing logs to stderr. The `parse_headless_events()` function parses stdout lines as JSON objects.
- **Timeout behavior**: fdemon headless enters an event loop after startup. Without a Flutter process running, it waits for stdin commands. The Docker container will need to be killed after the timeout. Implement this in `docker_run_headless()` using `docker run --stop-timeout` or a manual timer.
- **Test ordering**: Docker image builds should happen first. If Task 05 images aren't built yet, these tests will build them (Dockerfiles are self-contained). But this means first-run of headless tests also triggers image builds.
- **False positives**: A test passing (no fatal error) doesn't guarantee the *correct* SDK was detected — just that *some* SDK was found. The debug log verification tests (category 4) add confidence that the expected strategy was used.
- **Consider a dedicated headless output for SDK info**: Currently fdemon headless doesn't emit an explicit "sdk_resolved" event. A future enhancement could add `{"event":"sdk_resolved","source":"fvm","version":"3.22.0","path":"/root/fvm/versions/stable"}` to make assertions more precise. For now, we assert on the absence of errors and presence of downstream events (device discovery).
