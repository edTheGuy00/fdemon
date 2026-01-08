# Task F9: Document Testing Strategy

## Overview

Create comprehensive documentation for fdemon's E2E testing infrastructure, covering the testing pyramid, CI workflows, and contributor guidelines.

**Priority:** Medium
**Effort:** Low
**Depends On:** F1-F5 (ideally after Wave 1 & 2 complete)
**Status:** Pending

## Background

The E2E testing infrastructure spans multiple components:
- Mock daemon tests (Phase 1)
- Docker-based Flutter Linux desktop tests (Phase 2)
- Headless mode for output verification (F4-F5)

Documentation helps contributors understand how to run tests locally and add new test cases.

## Requirements

### Functional
- [ ] Testing pyramid documented
- [ ] Local development test commands documented
- [ ] CI workflow behavior explained
- [ ] New test contribution guidelines provided

### Documentation Location
- [ ] `docs/TESTING.md` - Main testing documentation

## Implementation

### Step 1: Create docs/TESTING.md

```markdown
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
│  - TestDaemon simulates Flutter daemon                      │
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
- `src/daemon/protocol.rs` - JSON-RPC parsing tests
- `src/core/stack_trace.rs` - Stack trace parsing tests

### Mock Daemon Tests

Location: `tests/e2e/mock/`

These tests use `TestDaemon` to simulate Flutter daemon behavior without requiring Flutter.

```bash
cargo test --test mock_daemon_tests
```

### Docker E2E Tests

Location: `tests/e2e/scripts/`

These run fdemon against real Flutter apps in Docker containers.

| Script | Purpose |
|--------|---------|
| `test_startup.sh` | Verify fdemon connects to Flutter and starts app |
| `test_hot_reload.sh` | Verify hot reload via stdin and file changes |
| `run_all_e2e.sh` | Orchestrate all E2E tests |

### Flutter Test Fixtures

Location: `tests/fixtures/`

| Fixture | Purpose |
|---------|---------|
| `simple_app/` | Minimal runnable Flutter app |
| `error_app/` | App with intentional compile errors |
| `plugin_with_example/` | Plugin structure with example app |
| `multi_module/` | Monorepo with multiple packages |

## Headless Mode

fdemon supports `--headless` mode for machine-readable output:

```bash
./target/release/fdemon --headless /path/to/flutter/app
```

Output format (NDJSON):
```json
{"event":"device_detected","device_id":"linux","platform":"linux","timestamp":1704700000}
{"event":"daemon_connected","device":"linux","timestamp":1704700001}
{"event":"app_started","session_id":"abc-123","device":"linux","timestamp":1704700005}
{"event":"hot_reload_completed","session_id":"abc-123","duration_ms":250,"timestamp":1704700010}
```

### Triggering Hot Reload

Send `r` to stdin:
```bash
echo "r" | ./target/release/fdemon --headless /path/to/app
```

Or use a pipe:
```bash
mkfifo /tmp/fdemon_input
./target/release/fdemon --headless /path/to/app < /tmp/fdemon_input &
echo "r" > /tmp/fdemon_input
```

## CI Workflows

### Unit Tests (Every Push)

File: `.github/workflows/ci.yml`

Runs on every push:
- `cargo fmt --check`
- `cargo clippy`
- `cargo test`

### E2E Tests (PR Merge)

File: `.github/workflows/e2e.yml`

Runs on PR merge to main:
- Builds Docker test image
- Runs all E2E scripts
- Uses BuildKit caching for faster builds

## Writing New Tests

### Adding a Unit Test

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

### Adding a Mock Daemon Test

See `tests/e2e/mock/` for examples using `TestDaemon`.

### Adding an E2E Script

1. Create `tests/e2e/scripts/test_<name>.sh`
2. Use headless mode: `fdemon --headless`
3. Parse JSON events with `jq`
4. Exit 0 for pass, non-zero for fail
5. Add to `run_all_e2e.sh` pattern match

Example:
```bash
#!/bin/bash
set -euo pipefail

source "$(dirname "$0")/lib/json_events.sh"
source "$(dirname "$0")/lib/xvfb.sh"

ensure_xvfb

OUTPUT="/tmp/my_test.jsonl"
./target/release/fdemon --headless tests/fixtures/simple_app > "$OUTPUT" &
PID=$!

if wait_for_event "app_started" $PID 60 "$OUTPUT"; then
    echo "PASS"
    kill $PID
    exit 0
else
    echo "FAIL"
    kill $PID
    exit 1
fi
```

### Adding a Flutter Fixture

1. Create directory: `tests/fixtures/<name>/`
2. Initialize: `cd tests/fixtures/<name> && flutter create .`
3. Add Linux platform: `flutter create --platforms=linux .`
4. Minimize dependencies for faster builds
5. Add to `.gitignore`: `build/`, `linux/flutter/ephemeral/`

## Known Limitations

### Docker E2E Tests

- **No Android/iOS devices**: Docker uses Flutter Linux desktop
- **GPU acceleration**: Not available, uses Mesa software rendering
- **First frame delay**: Linux desktop may take 2-3s for first frame

### Headless Mode

- **No TUI features**: Help popup, scrolling, etc. not testable
- **Log volume**: High log volume may require filtering

## Troubleshooting

### "No devices found"

In Docker, ensure:
1. Xvfb is running (`export DISPLAY=:99`)
2. Flutter Linux desktop is enabled (`flutter config --enable-linux-desktop`)
3. Fixture has `linux/` directory

### "Timeout waiting for event"

Increase timeout:
```bash
FDEMON_TEST_TIMEOUT=180 ./tests/e2e/scripts/test_startup.sh
```

### Docker build slow

Use BuildKit caching:
```bash
DOCKER_BUILDKIT=1 docker build -f Dockerfile.test -t fdemon-test .
```
```

## Verification

```bash
# Verify documentation renders correctly
# (Use any markdown previewer)

# Verify commands in documentation work
cargo test
docker build -f Dockerfile.test -t fdemon-test .
```

## Completion Checklist

- [ ] `docs/TESTING.md` created
- [ ] Testing pyramid explained
- [ ] All test commands documented
- [ ] Headless mode documented
- [ ] Contribution guidelines included
- [ ] CI workflows explained
- [ ] Troubleshooting section added
- [ ] Link added to main README.md (if appropriate)
