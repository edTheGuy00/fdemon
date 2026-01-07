# E2E Integration Testing Strategy for Flutter Demon

**Date:** 2025-01-XX  
**Author:** Research & Planning  
**Status:** Recommendation

## Executive Summary

This document outlines a comprehensive strategy for implementing end-to-end (E2E) integration tests for Flutter Demon (`fdemon`), a terminal user interface (TUI) application for Flutter development. The recommended approach uses a multi-layered testing strategy combining mock daemon tests for fast feedback with Docker-based real Flutter daemon integration for comprehensive validation.

## Current Testing Landscape

### Existing Test Infrastructure

The project currently has:

1. **Unit Tests** - Using `#[cfg(test)]` modules throughout the codebase
   - Handler tests in `src/app/handler/tests.rs`
   - State management tests
   - Configuration parsing tests
   - Core utility tests

2. **Widget/Rendering Tests** - Using Ratatui's `TestBackend`
   - `src/tui/widgets/confirm_dialog.rs` - Dialog rendering
   - `src/tui/widgets/device_selector.rs` - Device selector UI
   - `src/tui/widgets/header.rs` - Header rendering
   - `src/tui/widgets/settings_panel/tests.rs` - Settings panel

3. **Integration Tests** - Located in `tests/` directory
   - `tests/discovery_integration.rs` - Flutter project discovery
   - Tests various project types (apps, plugins, packages)
   - Validates monorepo structures

### Gaps in Current Testing

- **No Flutter daemon interaction testing** - Critical functionality untested
- **No file watcher integration testing** - Hot reload triggers not validated
- **No multi-session workflow testing** - Session management edge cases
- **No end-to-end user workflow testing** - Complete user journeys untested
- **No CI integration with real Flutter tooling** - Environment-specific issues may slip through

## Recommended Multi-Layered Testing Strategy

```
┌─────────────────────────────────────────────────────┐
│ Layer 3: Full Docker E2E Tests                     │
│ - Real Flutter daemon interaction                   │
│ - Actual file watching                             │
│ - Complete user workflows                          │
│ - Run on: Pre-merge, Nightly, Release             │
└─────────────────────────────────────────────────────┘
                      ▼
┌─────────────────────────────────────────────────────┐
│ Layer 2: Mocked Daemon Integration Tests           │
│ - Mock Flutter daemon responses                     │
│ - Test state transitions                           │
│ - Verify message handling                          │
│ - Run on: Every commit                             │
└─────────────────────────────────────────────────────┘
                      ▼
┌─────────────────────────────────────────────────────┐
│ Layer 1: Widget/Unit Tests (Current)               │
│ - TestBackend rendering                            │
│ - Individual component logic                       │
│ - Run on: Every commit                             │
└─────────────────────────────────────────────────────┘
```

## Layer 1: Widget & Unit Tests (Existing)

**Status:** ✅ Already implemented

**Tools:**
- Ratatui's `TestBackend`
- Standard Rust `#[test]` attributes
- `tokio-test` for async tests

**Coverage:**
- Widget rendering verification
- State machine transitions
- Configuration parsing
- Project discovery logic

**Keep doing:**
- Continue writing unit tests for new features
- Use `TestBackend::new(width, height)` for UI tests
- Assert buffer contents with `assert_buffer_lines()`

## Layer 2: Mock Daemon Integration Tests

**Status:** 🔶 Recommended for immediate implementation

**Purpose:** Fast-feedback integration testing without requiring Flutter installation

### Architecture

Create a mock Flutter daemon that simulates the JSON-RPC protocol:

```rust
// tests/e2e/mock_daemon.rs
use tokio::sync::mpsc;
use serde_json::{json, Value};

/// Mock Flutter daemon for testing without real Flutter installation
pub struct MockFlutterDaemon {
    cmd_rx: mpsc::Receiver<String>,
    event_tx: mpsc::Sender<String>,
}

impl MockFlutterDaemon {
    pub fn new() -> (Self, DaemonHandle) {
        let (cmd_tx, cmd_rx) = mpsc::channel(100);
        let (event_tx, event_rx) = mpsc::channel(100);
        
        let daemon = MockFlutterDaemon {
            cmd_rx,
            event_tx,
        };
        
        let handle = DaemonHandle {
            cmd_tx,
            event_rx,
        };
        
        (daemon, handle)
    }
    
    /// Start mock daemon event loop
    pub async fn run(&mut self) {
        // Send initial daemon.connected
        self.send_event(json!({
            "event": "daemon.connected",
            "params": {"version": "0.0.1"}
        })).await;
        
        // Process commands
        while let Some(cmd) = self.cmd_rx.recv().await {
            self.handle_command(&cmd).await;
        }
    }
    
    async fn handle_command(&mut self, cmd: &str) {
        let parsed: Value = serde_json::from_str(cmd).unwrap();
        let method = parsed["method"].as_str().unwrap();
        
        match method {
            "device.getDevices" => {
                self.send_response(json!([
                    {
                        "id": "test-device-1",
                        "name": "Test Device",
                        "platform": "android"
                    }
                ])).await;
            }
            "app.start" => {
                // Simulate app startup
                self.send_event(json!({
                    "event": "app.started",
                    "params": {"appId": "test-app-1"}
                })).await;
            }
            "app.restart" => {
                // Simulate hot reload
                self.send_event(json!({
                    "event": "app.log",
                    "params": {"appId": "test-app-1", "log": "Performing hot reload..."}
                })).await;
                
                tokio::time::sleep(Duration::from_millis(100)).await;
                
                self.send_event(json!({
                    "event": "app.log",
                    "params": {"appId": "test-app-1", "log": "Reloaded 5 of 5 libraries"}
                })).await;
            }
            _ => {}
        }
    }
    
    async fn send_event(&mut self, event: Value) {
        let json_str = serde_json::to_string(&event).unwrap();
        self.event_tx.send(json_str).await.unwrap();
    }
    
    async fn send_response(&mut self, result: Value) {
        // Similar to send_event but with response format
        self.send_event(json!({"result": result})).await;
    }
}

pub struct DaemonHandle {
    pub cmd_tx: mpsc::Sender<String>,
    pub event_rx: mpsc::Receiver<String>,
}
```

### Test Examples

**Device Discovery Test:**

```rust
// tests/e2e/daemon_interaction.rs
#[tokio::test]
async fn test_device_discovery_flow() {
    let (mut mock_daemon, daemon_handle) = MockFlutterDaemon::new();
    let mut app_state = AppState::new();
    
    // Spawn mock daemon responder
    let daemon_task = tokio::spawn(async move {
        mock_daemon.run().await;
    });
    
    // Simulate device discovery message
    let message = Message::RequestDevices;
    let (new_state, action) = update(app_state, message);
    
    // Process daemon response
    if let Some(event_json) = daemon_handle.event_rx.recv().await {
        let message = Message::DaemonEvent(parse_event(&event_json));
        let (new_state, _) = update(new_state, message);
        
        assert_eq!(new_state.devices.len(), 1);
        assert_eq!(new_state.devices[0].id, "test-device-1");
    }
    
    daemon_task.abort();
}
```

**Hot Reload Test:**

```rust
#[tokio::test]
async fn test_hot_reload_triggers_all_sessions() {
    let (mut mock_daemon, daemon_handle) = MockFlutterDaemon::new();
    let mut app_state = AppState::new();
    
    // Setup: Create two running sessions
    let device1 = Device::new("device-1", "Device 1");
    let device2 = Device::new("device-2", "Device 2");
    let session1 = app_state.session_manager.create_session(&device1).unwrap();
    let session2 = app_state.session_manager.create_session(&device2).unwrap();
    
    // Mark sessions as running
    app_state.session_manager
        .get_mut(session1)
        .unwrap()
        .set_app_id("app-1".to_string());
    app_state.session_manager
        .get_mut(session2)
        .unwrap()
        .set_app_id("app-2".to_string());
    
    // Trigger hot reload
    let message = Message::TriggerHotReload;
    let (new_state, action) = update(app_state, message);
    
    // Verify both sessions received reload command
    assert!(matches!(action, Some(UpdateAction::ReloadAllSessions)));
    
    // Verify daemon received two reload commands
    let cmd1 = daemon_handle.cmd_tx.recv().await.unwrap();
    let cmd2 = daemon_handle.cmd_tx.recv().await.unwrap();
    
    assert!(cmd1.contains("app.restart"));
    assert!(cmd2.contains("app.restart"));
}
```

**Session Management Test:**

```rust
#[tokio::test]
async fn test_session_lifecycle() {
    let (mut mock_daemon, daemon_handle) = MockFlutterDaemon::new();
    let mut app_state = AppState::new();
    
    // 1. Create session
    let device = Device::new("device-1", "Device 1");
    let session_id = app_state.session_manager.create_session(&device).unwrap();
    assert_eq!(app_state.session_manager.session_count(), 1);
    
    // 2. Start app on session
    let message = Message::StartSession(session_id);
    let (new_state, action) = update(app_state, message);
    
    // Verify start command sent to daemon
    let cmd = daemon_handle.cmd_tx.recv().await.unwrap();
    assert!(cmd.contains("app.start"));
    
    // 3. Simulate app started event
    let message = Message::DaemonEvent(AppStartedEvent {
        app_id: "test-app-1".to_string(),
    });
    let (new_state, _) = update(new_state, message);
    
    assert!(new_state.session_manager.get(session_id).unwrap().is_running());
    
    // 4. Stop session
    let message = Message::StopSession(session_id);
    let (new_state, action) = update(new_state, message);
    
    // Verify stop command sent
    let cmd = daemon_handle.cmd_tx.recv().await.unwrap();
    assert!(cmd.contains("app.stop"));
    
    // 5. Remove session
    let message = Message::RemoveSession(session_id);
    let (new_state, _) = update(new_state, message);
    
    assert_eq!(new_state.session_manager.session_count(), 0);
}
```

### Test File Structure

```
tests/
├── e2e/
│   ├── mod.rs                    # Test utilities, MockFlutterDaemon
│   ├── daemon_interaction.rs     # Device discovery, daemon connection
│   ├── hot_reload.rs             # Hot reload workflows
│   ├── session_management.rs     # Session lifecycle tests
│   ├── log_parsing.rs            # Log entry parsing and filtering
│   └── error_handling.rs         # Error recovery scenarios
└── fixtures/
    └── daemon_responses/         # JSON files with recorded responses
        ├── device_list.json
        ├── app_started.json
        └── hot_reload_sequence.json
```

### Benefits

- ✅ **Fast execution** - No Flutter installation required
- ✅ **Deterministic** - No flaky network/timing issues
- ✅ **Easy debugging** - Full control over daemon behavior
- ✅ **CI-friendly** - Runs on any platform
- ✅ **Edge case testing** - Can simulate rare error conditions

### Implementation Timeline

**Week 1-2:**
1. Create `MockFlutterDaemon` infrastructure
2. Implement 5-10 core workflow tests
3. Add to CI pipeline

**Week 3-4:**
4. Expand test coverage to edge cases
5. Add recorded response fixtures
6. Document testing patterns

## Layer 3: Docker-Based E2E Tests

**Status:** 🔶 Recommended for Phase 2

**Purpose:** Validate real Flutter daemon interaction in isolated environment

### Docker Infrastructure

**Test Dockerfile:**

```dockerfile
# Dockerfile.test
FROM ghcr.io/cirruslabs/flutter:stable

# Install Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# Install system dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    git \
    xvfb \
    && rm -rf /var/lib/apt/lists/*

# Setup Flutter
ENV FLUTTER_ROOT=/opt/flutter
ENV PATH="$FLUTTER_ROOT/bin:$PATH"

# Verify Flutter installation
RUN flutter doctor -v

# Create test workspace
WORKDIR /workspace

# Copy test fixtures (minimal Flutter apps)
COPY tests/fixtures /workspace/fixtures

# Entry point for running tests
COPY tests/e2e/docker-entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

ENTRYPOINT ["/entrypoint.sh"]
```

**Docker Compose Configuration:**

```yaml
# docker-compose.test.yml
version: '3.8'

services:
  flutter-e2e-test:
    build:
      context: .
      dockerfile: Dockerfile.test
    volumes:
      # Mount compiled binary
      - ./target/debug/fdemon:/usr/local/bin/fdemon:ro
      # Mount test fixtures
      - ./tests/fixtures:/workspace/fixtures:ro
      # Mount test scripts
      - ./tests/e2e/scripts:/workspace/scripts:ro
    environment:
      - RUST_LOG=debug
      - FLUTTER_ROOT=/opt/flutter
      - DISPLAY=:99
    command: bash /workspace/scripts/run_all_e2e.sh

  # Optional: Run specific test scenarios
  hot-reload-test:
    extends: flutter-e2e-test
    command: bash /workspace/scripts/test_hot_reload.sh

  multi-session-test:
    extends: flutter-e2e-test
    command: bash /workspace/scripts/test_multi_session.sh
```

### Test Fixtures

Create minimal Flutter applications for testing:

```
tests/fixtures/
├── simple_app/                   # Minimal runnable Flutter app
│   ├── lib/
│   │   └── main.dart
│   ├── pubspec.yaml
│   ├── android/
│   └── ios/
├── plugin_with_example/          # Plugin structure
│   ├── lib/
│   ├── pubspec.yaml
│   └── example/
│       ├── lib/main.dart
│       └── pubspec.yaml
├── multi_module/                 # Monorepo structure
│   ├── apps/
│   │   ├── app1/
│   │   └── app2/
│   └── packages/
│       └── shared/
└── error_app/                    # App with intentional errors
    ├── lib/
    │   └── main.dart             # Contains syntax error
    └── pubspec.yaml
```

**Simple App Example:**

```dart
// tests/fixtures/simple_app/lib/main.dart
import 'package:flutter/material.dart';

void main() {
  runApp(const MyApp());
}

class MyApp extends StatelessWidget {
  const MyApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Test App',
      home: Scaffold(
        appBar: AppBar(title: const Text('Test App')),
        body: const Center(
          child: Text('Hello E2E Test'),
        ),
      ),
    );
  }
}
```

### E2E Test Scripts

**Hot Reload Test Script:**

```bash
#!/bin/bash
# tests/e2e/scripts/test_hot_reload.sh

set -e

echo "=== Hot Reload E2E Test ==="

cd /workspace/fixtures/simple_app

# Start Flutter daemon in background
flutter daemon --machine > /tmp/daemon.log 2>&1 &
DAEMON_PID=$!
sleep 2

# Start fdemon in background
fdemon . > /tmp/fdemon.log 2>&1 &
FDEMON_PID=$!
sleep 3

# Verify fdemon is running
if ! ps -p $FDEMON_PID > /dev/null; then
    echo "ERROR: fdemon failed to start"
    cat /tmp/fdemon.log
    exit 1
fi

# Modify a Dart file to trigger hot reload
echo "// Test comment" >> lib/main.dart

# Wait for hot reload
sleep 5

# Check logs for hot reload completion
if grep -q "Reloaded.*libraries" /tmp/fdemon.log; then
    echo "✓ Hot reload triggered successfully"
else
    echo "✗ Hot reload did not complete"
    cat /tmp/fdemon.log
    exit 1
fi

# Cleanup
kill $FDEMON_PID 2>/dev/null || true
kill $DAEMON_PID 2>/dev/null || true

echo "=== Test Passed ==="
```

**Multi-Session Test Script:**

```bash
#!/bin/bash
# tests/e2e/scripts/test_multi_session.sh

set -e

echo "=== Multi-Session E2E Test ==="

cd /workspace/fixtures/simple_app

# TODO: Implement multi-device testing
# This would require Flutter device emulators or simulators
# For now, test with multiple "headless" sessions

# Start fdemon
fdemon . &
FDEMON_PID=$!
sleep 3

# Send commands via stdin to create multiple sessions
# (This requires fdemon to support scripted input)

# Verify multiple sessions created
# Check logs or state

kill $FDEMON_PID

echo "=== Test Passed ==="
```

### PTY-Based Terminal Interaction Testing

For testing actual keyboard input and terminal output:

```rust
// tests/e2e/tui_interaction.rs
use std::process::{Command, Stdio};
use std::io::{Write, BufRead, BufReader};
use std::time::Duration;

#[test]
#[ignore] // Run with --ignored for full E2E
fn test_device_selector_keyboard_navigation() {
    let mut child = Command::new("fdemon")
        .arg("/workspace/fixtures/simple_app")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn fdemon");
    
    let stdin = child.stdin.as_mut().unwrap();
    let stdout = BufReader::new(child.stdout.as_mut().unwrap());
    
    // Wait for app startup
    std::thread::sleep(Duration::from_secs(2));
    
    // Send 'd' to open device selector
    stdin.write_all(b"d").unwrap();
    stdin.flush().unwrap();
    
    std::thread::sleep(Duration::from_millis(500));
    
    // Send arrow down
    stdin.write_all(b"\x1b[B").unwrap(); // Down arrow escape sequence
    stdin.flush().unwrap();
    
    // Send Enter to select device
    stdin.write_all(b"\r").unwrap();
    stdin.flush().unwrap();
    
    std::thread::sleep(Duration::from_secs(1));
    
    // Verify session was created (check logs or state file)
    
    // Cleanup
    child.kill().unwrap();
    child.wait().unwrap();
}
```

**Alternative: Use `expectrl` crate:**

```rust
use expectrl::{spawn, Regex, WaitStatus};

#[test]
#[ignore]
fn test_startup_flow_with_expect() {
    let mut process = spawn("fdemon /workspace/fixtures/simple_app").unwrap();
    
    // Wait for startup message
    process.expect(Regex("Flutter Demon.*ready")).unwrap();
    
    // Send device selector command
    process.send("d").unwrap();
    
    // Wait for device list
    process.expect(Regex("Device Selector")).unwrap();
    
    // Navigate and select
    process.send_line("").unwrap(); // Enter
    
    // Verify session started
    process.expect(Regex("Session.*started")).unwrap();
    
    // Quit
    process.send("q").unwrap();
    process.send("y").unwrap(); // Confirm quit
    
    assert_eq!(process.wait().unwrap(), WaitStatus::Exited(process.pid(), 0));
}
```

### Running Docker Tests

**Local execution:**

```bash
# Build the test image
docker-compose -f docker-compose.test.yml build

# Run all E2E tests
docker-compose -f docker-compose.test.yml run --rm flutter-e2e-test

# Run specific test
docker-compose -f docker-compose.test.yml run --rm hot-reload-test

# Interactive debugging
docker-compose -f docker-compose.test.yml run --rm flutter-e2e-test bash
```

**CI Integration:**

```yaml
# .github/workflows/e2e.yml
name: E2E Tests

on:
  push:
    branches: [main, develop]
  pull_request:
    branches: [main]

jobs:
  mock-integration:
    name: Mock Daemon Integration Tests
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Setup Rust
        uses: actions-rust-lang/setup-rust-toolchain@v1
      
      - name: Run mock integration tests
        run: cargo test --test e2e
        
      - name: Upload test logs
        if: failure()
        uses: actions/upload-artifact@v3
        with:
          name: mock-test-logs
          path: target/debug/test-*.log

  docker-e2e:
    name: Docker E2E Tests
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Setup Rust
        uses: actions-rust-lang/setup-rust-toolchain@v1
      
      - name: Build fdemon binary
        run: cargo build --release
      
      - name: Set up Docker Buildx
        uses: docker/setup-buildx-action@v2
      
      - name: Build test image
        run: docker-compose -f docker-compose.test.yml build
      
      - name: Run E2E tests
        run: |
          docker-compose -f docker-compose.test.yml run --rm flutter-e2e-test
      
      - name: Upload test results
        if: always()
        uses: actions/upload-artifact@v3
        with:
          name: e2e-test-logs
          path: |
            /tmp/fdemon.log
            /tmp/daemon.log

  docker-e2e-matrix:
    name: E2E Tests (Flutter ${{ matrix.flutter-version }})
    runs-on: ubuntu-latest
    strategy:
      matrix:
        flutter-version: [stable, beta]
    steps:
      - uses: actions/checkout@v3
      
      - name: Build with Flutter ${{ matrix.flutter-version }}
        run: |
          docker build -t fdemon-test:${{ matrix.flutter-version }} \
            --build-arg FLUTTER_VERSION=${{ matrix.flutter-version }} \
            -f Dockerfile.test .
      
      - name: Run E2E tests
        run: |
          docker run --rm \
            -v $PWD/target/release/fdemon:/usr/local/bin/fdemon:ro \
            fdemon-test:${{ matrix.flutter-version }} \
            bash /workspace/scripts/run_all_e2e.sh
```

### Benefits

- ✅ **Real Flutter daemon** - Tests actual integration
- ✅ **Reproducible** - Same environment every time
- ✅ **CI-ready** - Runs in GitHub Actions, GitLab CI, etc.
- ✅ **Multi-version testing** - Test against multiple Flutter versions
- ✅ **Isolated** - No interference with host system

### Challenges & Mitigations

| Challenge | Mitigation |
|-----------|------------|
| Slow test execution | Run only on pre-merge, nightly |
| Flaky network/timing | Add retry logic, increase timeouts |
| Debug difficulty | Volume mount logs, interactive debugging mode |
| Resource intensive | Use Docker layer caching, parallel execution |
| Device emulation | Use Flutter's headless test mode or mock devices |

## Testing Priority Matrix

| Test Type | Priority | Effort | Value | Timeline |
|-----------|----------|--------|-------|----------|
| Mock daemon - basic flows | 🔴 High | Medium | High | Week 1-2 |
| Mock daemon - error cases | 🟡 Medium | Low | High | Week 2-3 |
| Docker E2E - smoke tests | 🟡 Medium | High | High | Week 3-4 |
| Docker E2E - full coverage | 🟢 Low | High | Medium | Month 2 |
| PTY-based TUI interaction | 🟢 Low | Very High | Medium | Future |

## Recommended Implementation Phases

### Phase 1: Foundation (Weeks 1-2)

**Goals:**
- Set up mock daemon infrastructure
- Implement 10-15 core integration tests
- Add to CI pipeline

**Deliverables:**
- `tests/e2e/mod.rs` with `MockFlutterDaemon`
- Tests for device discovery, session lifecycle, hot reload
- CI workflow running mock tests on every commit

**Success Criteria:**
- Mock tests run in < 30 seconds
- 80% code coverage of `handler::update()` paths
- Zero flaky tests

### Phase 2: Docker Infrastructure (Weeks 3-4)

**Goals:**
- Create Docker test environment
- Implement basic E2E smoke tests
- Document test fixture creation

**Deliverables:**
- `Dockerfile.test` and `docker-compose.test.yml`
- 3-5 test fixtures (simple_app, plugin, error_app)
- 3-5 bash scripts for common workflows
- Documentation for running Docker tests

**Success Criteria:**
- Docker tests run in < 5 minutes
- Tests pass consistently (< 5% flake rate)
- Can run locally and in CI

### Phase 3: Comprehensive Coverage (Weeks 5-8)

**Goals:**
- Expand test coverage to edge cases
- Add multi-version Flutter testing
- Performance benchmarking

**Deliverables:**
- 20+ Docker E2E test scenarios
- Multi-Flutter-version CI matrix
- Performance regression tests
- Test maintenance documentation

**Success Criteria:**
- Cover all major user workflows
- Catch regressions before merge
- Documented test patterns for contributors

### Phase 4: Advanced Testing (Future)

**Goals:**
- PTY-based interaction testing
- Visual regression testing
- Chaos/fuzz testing

**Deliverables:**
- `expectrl`-based keyboard interaction tests
- Screenshot comparison tests
- Property-based testing for state machine

## Test Execution Strategy

### Local Development

```bash
# Quick feedback loop
cargo test                              # Unit + widget tests (fast)
cargo test --test e2e                   # Mock integration tests (medium)

# Pre-commit
cargo test --all                        # All Rust tests
cargo clippy                            # Lints
cargo fmt --check                       # Format check

# Pre-push
docker-compose -f docker-compose.test.yml run --rm flutter-e2e-test
```

### CI Pipeline

```
┌─────────────────────────────────────────────────────┐
│ On Every Commit                                     │
├─────────────────────────────────────────────────────┤
│ • Unit tests                                        │
│ • Widget tests                                      │
│ • Mock integration tests                            │
│ • Clippy lints                                      │
│ • Format check                                      │
│                                                     │
│ Duration: ~5 minutes                                │
└─────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────┐
│ On Pull Request / Pre-merge                         │
├─────────────────────────────────────────────────────┤
│ • All commit checks                                 │
│ • Docker E2E smoke tests (stable Flutter)          │
│ • Code coverage report                              │
│                                                     │
│ Duration: ~15 minutes                               │
└─────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────┐
│ Nightly / Scheduled                                 │
├─────────────────────────────────────────────────────┤
│ • All PR checks                                     │
│ • Full Docker E2E suite                             │
│ • Multi-Flutter-version matrix (stable, beta)      │
│ • Performance benchmarks                            │
│ • Memory leak detection                             │
│                                                     │
│ Duration: ~45 minutes                               │
└─────────────────────────────────────────────────────┘
```

## Key Testing Patterns

### Pattern 1: Arrange-Act-Assert with Mock Daemon

```rust
#[tokio::test]
async fn test_pattern_example() {
    // Arrange
    let (mut mock_daemon, handle) = MockFlutterDaemon::new();
    let mut state = AppState::new();
    tokio::spawn(async move { mock_daemon.run().await });
    
    // Act
    let message = Message::SomeAction;
    let (new_state, action) = update(state, message);
    
    // Assert
    assert_eq!(new_state.expected_field, expected_value);
    assert!(matches!(action, Some(UpdateAction::Expected(_))));
}
```

### Pattern 2: Recorded Response Fixtures

```rust
// Load pre-recorded daemon responses
let device_list = load_fixture("daemon_responses/device_list.json");
mock_daemon.set_response("device.getDevices", device_list);
```

### Pattern 3: Docker Test Script Template

```bash
#!/bin/bash
set -e
set -o pipefail

TEST_NAME="my_test"
echo "=== $TEST_NAME ==="

# Setup
cd /workspace/fixtures/simple_app
cleanup() {
    kill $FDEMON_PID 2>/dev/null || true
}
trap cleanup EXIT

# Execute
fdemon . > /tmp/fdemon.log 2>&1 &
FDEMON_PID=$!
sleep 3

# Verify
if grep -q "expected output" /tmp/fdemon.log; then
    echo "✓ $TEST_NAME passed"
    exit 0
else
    echo "✗ $TEST_NAME failed"
    cat /tmp/fdemon.log
    exit 1
fi
```

## Metrics & Success Criteria

### Coverage Goals

- **Unit test coverage:** > 80% for core logic
- **Integration test coverage:** > 60% for handler paths
- **E2E test coverage:** All critical user workflows

### Quality Metrics

- **Flake rate:** < 5% across all test runs
- **Test execution time:**
  - Unit tests: < 10 seconds
  - Mock integration: < 30 seconds
  - Docker E2E: < 5 minutes
- **CI feedback time:** < 15 minutes for PR checks

### Maintenance Goals

- **Test-to-code ratio:** ~1:2 (reasonable for Rust)
- **Test documentation:** Every complex test has explanation
- **Fixture maintenance:** Update fixtures with Flutter releases

## Tools & Dependencies

### New Dependencies to Add

```toml
[dev-dependencies]
# Existing
tokio-test = "0.4"
tempfile = "3"

# New for E2E testing
mockall = "0.12"           # Mock generation (if needed)
expectrl = "0.7"           # PTY interaction (Phase 4)
insta = "1.34"             # Snapshot testing (optional)
```

### External Tools

- **Docker** - Container runtime
- **docker-compose** - Multi-container orchestration
- **Flutter SDK** - In Docker image (ghcr.io/cirruslabs/flutter)
- **jq** - JSON processing in bash scripts

## Risk Assessment

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| Docker tests too slow | Medium | High | Use layer caching, parallel tests |
| Flutter version incompatibility | High | Medium | Multi-version CI matrix |
| Flaky timing-dependent tests | High | Medium | Use mock daemon for determinism |
| Test maintenance burden | Medium | High | Good documentation, test patterns |
| CI resource limits | Low | Low | Use self-hosted runners if needed |

## Open Questions & Future Considerations

1. **Device Emulation:** How to test multi-device scenarios without real devices?
   - **Option A:** Use Flutter's headless test mode
   - **Option B:** Mock device responses entirely
   - **Option C:** Use Android emulator in CI (resource-intensive)

2. **Visual Regression Testing:** Should we test TUI appearance?
   - Could use snapshot testing with `insta` crate
   - Capture terminal output and compare against golden files

3. **Performance Testing:** How to detect performance regressions?
   - Add criterion benchmarks for critical paths
   - Track test execution time in CI
   - Memory profiling for long-running sessions

4. **Chaos Testing:** Should we test against daemon crashes?
   - Randomly kill daemon during tests
   - Test reconnection logic
   - Verify state recovery

## Conclusion

The recommended multi-layered testing approach provides:

1. **Fast feedback** via mock daemon tests for everyday development
2. **High confidence** via Docker E2E tests for critical paths
3. **Gradual adoption** through phased implementation
4. **Maintainability** through clear patterns and documentation

**Next Steps:**
1. Review and approve this recommendation
2. Create GitHub issues for Phase 1 tasks
3. Set up project board for tracking
4. Begin implementation with mock daemon infrastructure

## References

- [Ratatui Testing Guide](https://docs.rs/ratatui/latest/ratatui/backend/struct.TestBackend.html)
- [Flutter Daemon Protocol](https://github.com/flutter/flutter/blob/master/packages/flutter_tools/doc/daemon.md)
- [Cirrus Labs Flutter Docker Images](https://github.com/cirruslabs/docker-images-flutter)
- [Rust Testing Best Practices](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [tokio Testing Utilities](https://docs.rs/tokio/latest/tokio/test/)

---

**Document Version:** 1.0  
**Last Updated:** 2025-01-XX  
**Next Review:** After Phase 1 completion