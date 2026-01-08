# Testing Guide

This document covers fdemon's testing strategy, from unit tests to end-to-end validation.

## Testing Pyramid

```
┌─────────────────────────────────────────────────────────────┐
│  Level 4: Real Device Testing (Future)                     │
│  - Android emulator on GitHub Actions                       │
│  - iOS simulator on macOS runners                           │
│  - ~15 min, nightly only                                    │
├─────────────────────────────────────────────────────────────┤
│  Level 3: Docker E2E with Flutter Linux Desktop            │
│  - fdemon --headless with real Flutter app                  │
│  - ~5-10 min, every PR merge                                │
├─────────────────────────────────────────────────────────────┤
│  Level 2: Mock Daemon Tests                                 │
│  - Simulated Flutter daemon behavior                        │
│  - Fast feedback, no Flutter required                       │
│  - <2 min, every push                                       │
├─────────────────────────────────────────────────────────────┤
│  Level 1: Unit + Integration Tests                         │
│  - Component logic, state transitions                       │
│  - TestBackend for TUI rendering                            │
│  - <30 sec, every commit                                    │
└─────────────────────────────────────────────────────────────┘
```

## Quick Start

### Run All Tests

```bash
# Unit and integration tests
cargo test

# Run with output
cargo test -- --nocapture

# Run specific test
cargo test hot_reload
```

### Run E2E Tests (Docker)

```bash
# Build test image
docker build -f Dockerfile.test -t fdemon-test .

# Run all E2E tests
docker-compose -f docker-compose.test.yml run --rm flutter-e2e-test

# Run specific test
docker-compose -f docker-compose.test.yml run --rm flutter-e2e-test \
    ./tests/e2e/scripts/test_startup.sh
```

### Run E2E Tests (Local - requires Flutter)

```bash
# Build fdemon
cargo build --release

# Run headless test against fixture
./target/release/fdemon --headless tests/fixtures/simple_app
```

## Test Categories

### Unit Tests

Location: `src/**/*.rs` (inline `#[cfg(test)]` modules)

```bash
cargo test --lib
```

Key test modules:
- `src/app/handler/tests.rs` - State transition tests
- `src/app/session/tests.rs` - Session management tests
- `src/daemon/protocol.rs` - JSON-RPC parsing tests
- `src/core/ansi.rs` - ANSI escape handling tests
- `src/tui/widgets/log_view/tests.rs` - Widget rendering tests

### Integration Tests

Location: `tests/` directory at project root

```bash
# Run all integration tests
cargo test --test '*'

# Run specific integration test
cargo test --test discovery_integration
```

Current integration tests:
- `tests/discovery_integration.rs` - Flutter project discovery
- `tests/fixture_parsing_test.rs` - Test fixture validation

### Mock Daemon Tests

Location: `tests/e2e/*.rs`

These tests use simulated Flutter daemon behavior without requiring Flutter.

```bash
# Run all mock tests
cargo test --test e2e

# Run specific mock test module
cargo test --test e2e hot_reload
```

Test modules:
- `tests/e2e/mock_daemon.rs` - Basic daemon lifecycle
- `tests/e2e/daemon_interaction.rs` - Daemon command/response flow
- `tests/e2e/hot_reload.rs` - Hot reload message handling
- `tests/e2e/session_management.rs` - Multi-session coordination

### Docker E2E Tests

Location: `tests/e2e/scripts/`

These run fdemon against real Flutter apps in Docker containers.

| Script | Purpose |
|--------|---------|
| `test_startup.sh` | Verify fdemon connects to Flutter and starts app |
| `test_hot_reload.sh` | Verify hot reload via stdin and file changes |
| `run_all_e2e.sh` | Orchestrate all E2E tests |

Helper libraries:
- `lib/json_events.sh` - Parse and wait for JSON events from headless mode
- `lib/xvfb.sh` - Manage virtual display for Linux desktop
- `lib/fixtures.sh` - Prepare Flutter test fixtures

### Flutter Test Fixtures

Location: `tests/fixtures/`

| Fixture | Purpose |
|---------|---------|
| `simple_app/` | Minimal runnable Flutter app |
| `error_app/` | App with intentional compile errors |
| `plugin_with_example/` | Plugin structure with example app |
| `multi_module/` | Monorepo with multiple packages |
| `daemon_responses/` | JSON response templates for mock tests |

## Headless Mode

fdemon supports `--headless` mode for machine-readable output:

```bash
./target/release/fdemon --headless /path/to/flutter/app
```

### Output Format (NDJSON)

Events are output as newline-delimited JSON, one event per line:

```json
{"event":"device_detected","device_id":"linux","device_name":"Linux Desktop","platform":"linux","timestamp":1704700000000}
{"event":"daemon_connected","device":"Linux Desktop","timestamp":1704700001000}
{"event":"app_started","session_id":"0","device":"Linux Desktop","timestamp":1704700005000}
{"event":"hot_reload_started","session_id":"0","timestamp":1704700010000}
{"event":"hot_reload_completed","session_id":"0","duration_ms":250,"timestamp":1704700011000}
{"event":"log","level":"info","message":"Flutter app initialized","session_id":"0","timestamp":1704700012000}
```

### Available Events

| Event | Fields | Description |
|-------|--------|-------------|
| `device_detected` | `device_id`, `device_name`, `platform`, `timestamp` | Device discovered |
| `daemon_connected` | `device`, `timestamp` | Flutter daemon connected |
| `daemon_disconnected` | `device`, `reason`, `timestamp` | Flutter daemon disconnected |
| `session_created` | `session_id`, `device`, `timestamp` | New session created |
| `session_removed` | `session_id`, `timestamp` | Session ended |
| `app_started` | `session_id`, `device`, `timestamp` | Flutter app started |
| `app_stopped` | `session_id`, `reason`, `timestamp` | Flutter app stopped |
| `hot_reload_started` | `session_id`, `timestamp` | Hot reload initiated |
| `hot_reload_completed` | `session_id`, `duration_ms`, `timestamp` | Hot reload succeeded |
| `hot_reload_failed` | `session_id`, `error`, `timestamp` | Hot reload failed |
| `log` | `level`, `message`, `session_id`, `timestamp` | Log entry from app |
| `error` | `message`, `fatal`, `timestamp` | Error occurred |

### Triggering Hot Reload

Send `r` to stdin:
```bash
echo "r" | ./target/release/fdemon --headless /path/to/app
```

Or use a named pipe:
```bash
mkfifo /tmp/fdemon_input
./target/release/fdemon --headless /path/to/app < /tmp/fdemon_input &
PID=$!
sleep 2
echo "r" > /tmp/fdemon_input
kill $PID
rm /tmp/fdemon_input
```

### Other Commands

- `r` or `reload` - Trigger hot reload
- `R` or `restart` - Trigger hot restart
- `q` or `quit` - Exit fdemon

## CI Workflows

### E2E Tests (PR Merge / Nightly)

File: `.github/workflows/e2e.yml`

Runs on:
- Push to `main` branch
- PR merge to `main`
- Nightly schedule (2 AM UTC)
- Manual trigger via `workflow_dispatch`

Jobs:
1. **Docker E2E Tests** - Builds Docker test image, runs all E2E scripts
2. **Mock Daemon Tests** - Runs `cargo test --test e2e` for fast feedback

Features:
- BuildKit caching for faster builds
- Test logs uploaded as artifacts (retained 7 days)
- Configurable timeout via workflow inputs
- Concurrent run cancellation

### Unit Tests (Not Yet Implemented)

The project currently runs all tests (including E2E mocks) in the E2E workflow. A future `ci.yml` workflow would run on every push:

```yaml
# Future: .github/workflows/ci.yml
name: CI

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: cargo fmt --check
      - run: cargo clippy -- -D warnings
      - run: cargo test --lib
```

## Writing New Tests

### Adding a Unit Test

Place tests inline with the code they test:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_your_feature() {
        // Arrange
        let state = AppState::default();

        // Act
        let result = your_function(&state);

        // Assert
        assert!(result.is_ok());
    }
}
```

For large test suites (100+ lines), use a separate `tests.rs` file:

```rust
// src/my_module/mod.rs
#[cfg(test)]
mod tests;

// src/my_module/tests.rs
use super::*;

#[test]
fn test_case_1() { /* ... */ }

#[test]
fn test_case_2() { /* ... */ }
```

### Adding a Mock Daemon Test

See `tests/e2e/` for examples. Mock tests verify state transitions and message handling without requiring Flutter:

```rust
// tests/e2e/my_feature.rs
use fdemon::app::message::Message;
use fdemon::app::state::AppState;
use fdemon::core::DaemonEvent;

#[tokio::test]
async fn test_my_feature() {
    let mut state = AppState::default();

    // Simulate daemon event
    let event = DaemonEvent::Stdout(/* ... */);
    let msg = Message::Daemon(event);

    // Process message
    let result = fdemon::app::handler::update(&mut state, msg);

    // Assert state changes
    assert_eq!(state.phase, AppPhase::Running);
}
```

### Adding a Docker E2E Script

1. Create `tests/e2e/scripts/test_<name>.sh`
2. Use headless mode: `fdemon --headless`
3. Parse JSON events with `jq` and helper functions
4. Exit 0 for pass, non-zero for fail
5. Add to `run_all_e2e.sh`

Example:
```bash
#!/bin/bash
set -euo pipefail

# Source helper libraries
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/lib/json_events.sh"
source "$SCRIPT_DIR/lib/xvfb.sh"
source "$SCRIPT_DIR/lib/fixtures.sh"

# Ensure Xvfb is running (Linux desktop requirement)
ensure_xvfb

# Prepare fixture
FIXTURE_DIR="$(prepare_fixture simple_app)"

# Output file for JSON events
OUTPUT_FILE="/tmp/test_my_feature_$$.jsonl"

# Build fdemon if not exists
FDEMON_BIN="${FDEMON_BIN:-./target/release/fdemon}"
if [[ ! -x "$FDEMON_BIN" ]]; then
    log_error "fdemon binary not found or not executable: $FDEMON_BIN"
    exit 1
fi

# Start fdemon in headless mode
log_info "Starting fdemon in headless mode"
"$FDEMON_BIN" --headless "$FIXTURE_DIR" > "$OUTPUT_FILE" 2>&1 &
FDEMON_PID=$!

# Wait for app_started event (timeout: 60 seconds)
if wait_for_event "app_started" "$FDEMON_PID" 60 "$OUTPUT_FILE"; then
    log_success "Test passed: app started successfully"
    kill "$FDEMON_PID" 2>/dev/null || true
    exit 0
else
    log_error "Test failed: app_started event not received"
    cat "$OUTPUT_FILE"
    kill "$FDEMON_PID" 2>/dev/null || true
    exit 1
fi
```

### Adding a Flutter Fixture

1. Create directory: `tests/fixtures/<name>/`
2. Initialize: `cd tests/fixtures/<name> && flutter create .`
3. Add Linux platform: `flutter create --platforms=linux .`
4. Minimize dependencies for faster builds
5. Add to `.gitignore`: `build/`, `linux/flutter/ephemeral/`

Example:
```bash
cd tests/fixtures
flutter create --platforms=linux my_test_app
cd my_test_app

# Minimize pubspec.yaml (remove unnecessary dependencies)
# Edit lib/main.dart for specific test scenario

# Ensure .gitignore excludes build artifacts
echo "build/" >> .gitignore
echo "linux/flutter/ephemeral/" >> .gitignore
```

## Known Limitations

### Docker E2E Tests

- **No Android/iOS devices**: Docker uses Flutter Linux desktop only
- **GPU acceleration**: Not available, uses Mesa software rendering
- **First frame delay**: Linux desktop may take 2-3s for first frame
- **ARM architecture**: `adb` binary is disabled in Docker (x86-only, crashes on ARM)

### Headless Mode

- **No TUI features**: Help popup, scrolling, visual feedback not testable
- **Log volume**: High log volume may require filtering or processing
- **Timing precision**: `duration_ms` for hot reload may be 0 in some cases

### Mock Daemon Tests

- **Not real Flutter**: Cannot catch Flutter-specific bugs
- **Protocol accuracy**: Mock responses must stay in sync with Flutter's JSON-RPC protocol

## Troubleshooting

### "No devices found"

In Docker, ensure:
1. Xvfb is running (`export DISPLAY=:99`)
2. Flutter Linux desktop is enabled (`flutter config --enable-linux-desktop`)
3. Fixture has `linux/` directory (`flutter create --platforms=linux .`)

Debug:
```bash
# Check Flutter devices
flutter devices

# Check Linux prerequisites
flutter doctor

# Verify Xvfb
ps aux | grep Xvfb
echo $DISPLAY
```

### "Timeout waiting for event"

Increase timeout:
```bash
FDEMON_TEST_TIMEOUT=180 ./tests/e2e/scripts/test_startup.sh
```

Or check if fdemon started successfully:
```bash
# Run test script with verbose output
bash -x ./tests/e2e/scripts/test_startup.sh

# Check fdemon logs
cat /tmp/test_*.jsonl
```

### Docker build slow

Use BuildKit caching:
```bash
DOCKER_BUILDKIT=1 docker build -f Dockerfile.test -t fdemon-test .
```

Or use docker-compose with volume caching:
```bash
docker-compose -f docker-compose.test.yml build
```

### "Permission denied" in Docker

Ensure scripts are executable:
```bash
chmod +x tests/e2e/scripts/*.sh
chmod +x tests/e2e/scripts/lib/*.sh
```

### Mock tests fail with "daemon response not found"

Check fixture responses in `tests/fixtures/daemon_responses/`:
```bash
ls -la tests/fixtures/daemon_responses/
```

Ensure JSON files match the protocol messages in `src/daemon/protocol.rs`.

### Integration test fixtures fail to build

Ensure Flutter is installed and Linux desktop is enabled:
```bash
flutter --version
flutter config --enable-linux-desktop
flutter doctor
```

Then rebuild fixtures:
```bash
cd tests/fixtures/simple_app
flutter pub get
flutter build linux
```

## Best Practices

### Test Isolation

- Each test should be independent
- Use `tempdir()` for file-based tests
- Clean up resources in test teardown
- Don't rely on test execution order

### Test Naming

Use descriptive names that explain the scenario:

```rust
// Good
#[test]
fn test_hot_reload_updates_reload_count() { /* ... */ }

// Bad
#[test]
fn test_reload() { /* ... */ }
```

### Error Messages

Make test failures actionable:

```rust
// Good
assert_eq!(
    state.reload_count, 1,
    "Expected reload count to increment after hot reload, but got {}",
    state.reload_count
);

// Bad
assert_eq!(state.reload_count, 1);
```

### Test Coverage

Aim to test:
- Happy path
- Error cases
- Edge cases (empty inputs, boundary conditions)
- Race conditions (for async code)

### CI-Friendly Tests

- Use timeouts to prevent hanging tests
- Provide verbose output on failure
- Upload artifacts (logs, screenshots) for debugging
- Make tests deterministic (no flaky tests)

## Performance Benchmarks

Typical test durations on CI (GitHub Actions, ubuntu-latest):

| Test Suite | Duration | Frequency |
|------------|----------|-----------|
| Unit tests (`cargo test --lib`) | ~10s | Every push (future) |
| Integration tests (`cargo test --test '*'`) | ~20s | Every push (future) |
| Mock E2E tests (`cargo test --test e2e`) | ~60s | Every push |
| Docker E2E tests (full suite) | ~8min | PR merge, nightly |

## Further Reading

- [ARCHITECTURE.md](ARCHITECTURE.md) - System architecture and TEA pattern
- [DEVELOPMENT.md](DEVELOPMENT.md) - Build commands and workflow
- [CODE_STANDARDS.md](CODE_STANDARDS.md) - Coding conventions

## Contributing

When adding new features:

1. Write unit tests for business logic
2. Add integration tests for cross-module behavior
3. Add mock E2E tests for state transitions
4. Add Docker E2E tests for end-user scenarios (if applicable)
5. Update this document if adding new test patterns

See [DEVELOPMENT.md](DEVELOPMENT.md) for quality gates and verification commands.
