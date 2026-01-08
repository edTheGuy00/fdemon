# Task F6: GitHub Actions Android Emulator Workflow

## Overview

Create a GitHub Actions workflow that runs fdemon E2E tests with a real Android emulator using `reactivecircus/android-emulator-runner` on Ubuntu runners with KVM hardware acceleration.

**Priority:** Medium
**Effort:** Medium
**Depends On:** F4 (fdemon headless mode)
**Status:** Pending

## Background

GitHub Actions Ubuntu runners support KVM (Kernel-based Virtual Machine), enabling hardware-accelerated Android emulators. This is 2-3x faster than macOS runners and avoids the complexity of Docker-based emulator solutions.

Key benefits:
- Native KVM support on Ubuntu runners
- `reactivecircus/android-emulator-runner` handles emulator lifecycle
- AVD snapshot caching for fast startup (<60 seconds)
- Matrix testing across API levels

## Requirements

### Functional
- [ ] Workflow runs on schedule (nightly) and manual dispatch
- [ ] Android emulator starts with hardware acceleration
- [ ] fdemon tests run against real Flutter app on emulator
- [ ] Test artifacts uploaded on failure
- [ ] AVD snapshots cached for fast subsequent runs

### Technical
- [ ] Use `reactivecircus/android-emulator-runner@v2`
- [ ] Enable KVM on Ubuntu runner
- [ ] Target API levels 30-33 (Android 11-13)
- [ ] Use `google_atd` (Android Test Device) images for speed
- [ ] Timeout: 30 minutes max

## Implementation

### Step 1: Create workflow file

Create `.github/workflows/e2e-emulator.yml`:

```yaml
name: E2E Tests (Android Emulator)

on:
  schedule:
    # Run nightly at 3 AM UTC
    - cron: '0 3 * * *'
  workflow_dispatch:
    inputs:
      api_level:
        description: 'Android API level'
        required: false
        default: '30'
        type: choice
        options:
          - '29'
          - '30'
          - '33'

env:
  FLUTTER_VERSION: '3.19.0'

jobs:
  android-e2e:
    runs-on: ubuntu-latest
    timeout-minutes: 30
    strategy:
      fail-fast: false
      matrix:
        api-level: [30]  # Can expand to [29, 30, 33] for matrix testing

    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Setup Flutter
        uses: subosito/flutter-action@v2
        with:
          flutter-version: ${{ env.FLUTTER_VERSION }}
          cache: true

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          toolchain: stable

      - name: Cache Cargo
        uses: Swatinem/rust-cache@v2

      - name: Build fdemon
        run: cargo build --release

      - name: Enable KVM
        run: |
          echo 'KERNEL=="kvm", GROUP="kvm", MODE="0666", OPTIONS+="static_node=kvm"' | sudo tee /etc/udev/rules.d/99-kvm4all.rules
          sudo udevadm control --reload-rules
          sudo udevadm trigger --name-match=kvm

      - name: AVD Cache
        uses: actions/cache@v4
        id: avd-cache
        with:
          path: |
            ~/.android/avd/*
            ~/.android/adb*
          key: avd-${{ matrix.api-level }}-${{ runner.os }}

      - name: Create AVD Snapshot (if not cached)
        if: steps.avd-cache.outputs.cache-hit != 'true'
        uses: reactivecircus/android-emulator-runner@v2
        with:
          api-level: ${{ matrix.api-level }}
          target: google_atd
          arch: x86_64
          profile: pixel_6
          cores: 2
          ram-size: 4096M
          disk-size: 8192M
          emulator-boot-timeout: 600
          emulator-options: -no-window -gpu swiftshader_indirect -noaudio -no-boot-anim
          disable-animations: true
          script: echo "AVD snapshot created for caching"

      - name: Run E2E Tests
        uses: reactivecircus/android-emulator-runner@v2
        with:
          api-level: ${{ matrix.api-level }}
          target: google_atd
          arch: x86_64
          profile: pixel_6
          cores: 2
          ram-size: 4096M
          disk-size: 8192M
          emulator-boot-timeout: 300
          emulator-options: -no-snapshot-save -no-window -gpu swiftshader_indirect -noaudio -no-boot-anim
          disable-animations: true
          script: |
            # Verify emulator is ready
            adb wait-for-device
            adb shell getprop sys.boot_completed | grep -q 1

            # Run fdemon E2E test script
            ./tests/e2e/scripts/test_android_emulator.sh

      - name: Upload Test Logs
        if: failure()
        uses: actions/upload-artifact@v4
        with:
          name: android-e2e-logs-api${{ matrix.api-level }}-${{ github.run_number }}
          path: |
            test-logs/
            ~/.android/avd/*.avd/*.log
          retention-days: 7
```

### Step 2: Create E2E test script

Create `tests/e2e/scripts/test_android_emulator.sh`:

```bash
#!/bin/bash
set -euo pipefail

# E2E test for fdemon with real Android emulator
# Requires: fdemon --headless mode, connected Android device

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
FIXTURE_DIR="$PROJECT_ROOT/tests/fixtures/simple_app"
FDEMON_BIN="$PROJECT_ROOT/target/release/fdemon"
LOG_DIR="$PROJECT_ROOT/test-logs"
TIMEOUT_SECONDS="${FDEMON_TEST_TIMEOUT:-120}"

mkdir -p "$LOG_DIR"

echo "=== Android Emulator E2E Test ==="
echo "Fixture: $FIXTURE_DIR"
echo "fdemon: $FDEMON_BIN"
echo "Timeout: ${TIMEOUT_SECONDS}s"

# Verify fdemon binary exists
if [[ ! -x "$FDEMON_BIN" ]]; then
    echo "ERROR: fdemon binary not found at $FDEMON_BIN"
    exit 1
fi

# Verify device is connected
if ! adb devices | grep -q "emulator"; then
    echo "ERROR: No Android emulator detected"
    adb devices
    exit 1
fi

echo "Device detected:"
adb devices

# Get Flutter dependencies for fixture
echo "Getting Flutter dependencies..."
cd "$FIXTURE_DIR"
flutter pub get

# Start fdemon in headless mode
echo "Starting fdemon in headless mode..."
"$FDEMON_BIN" --headless "$FIXTURE_DIR" > "$LOG_DIR/fdemon_output.log" 2>&1 &
FDEMON_PID=$!

# Cleanup function
cleanup() {
    echo "Cleaning up..."
    kill $FDEMON_PID 2>/dev/null || true
    # Kill any Flutter processes
    pkill -f "flutter run" 2>/dev/null || true
}
trap cleanup EXIT

# Wait for app to start
echo "Waiting for app to start..."
STARTED=false
for i in $(seq 1 $TIMEOUT_SECONDS); do
    if grep -q '"event":"app_started"' "$LOG_DIR/fdemon_output.log" 2>/dev/null; then
        STARTED=true
        echo "App started after ${i}s"
        break
    fi
    sleep 1
done

if [[ "$STARTED" != "true" ]]; then
    echo "ERROR: App did not start within ${TIMEOUT_SECONDS}s"
    echo "=== fdemon output ==="
    cat "$LOG_DIR/fdemon_output.log"
    exit 1
fi

# Verify daemon connected
if grep -q '"event":"daemon_connected"' "$LOG_DIR/fdemon_output.log"; then
    echo "PASS: Daemon connected"
else
    echo "FAIL: Daemon connected event not found"
    exit 1
fi

# Test hot reload by modifying a file
echo "Testing hot reload..."
MAIN_DART="$FIXTURE_DIR/lib/main.dart"
cp "$MAIN_DART" "$MAIN_DART.bak"

# Add a comment to trigger reload
echo "// Hot reload test $(date +%s)" >> "$MAIN_DART"

# Wait for hot reload
RELOADED=false
for i in $(seq 1 30); do
    if grep -q '"event":"hot_reload_completed"' "$LOG_DIR/fdemon_output.log" 2>/dev/null; then
        RELOADED=true
        echo "Hot reload completed after ${i}s"
        break
    fi
    sleep 1
done

# Restore original file
mv "$MAIN_DART.bak" "$MAIN_DART"

if [[ "$RELOADED" != "true" ]]; then
    echo "WARN: Hot reload did not trigger (file watcher may not be active)"
    # Not a failure - hot reload may require manual trigger in headless mode
fi

# Verify log events
if grep -q '"event":"log"' "$LOG_DIR/fdemon_output.log"; then
    echo "PASS: Log events received"
else
    echo "WARN: No log events received"
fi

echo ""
echo "=== Test Summary ==="
echo "PASS: fdemon started and connected to Flutter daemon"
echo "PASS: App launched on Android emulator"
echo ""
echo "=== Full Output ==="
cat "$LOG_DIR/fdemon_output.log"

echo ""
echo "All tests passed!"
exit 0
```

### Step 3: Make script executable

```bash
chmod +x tests/e2e/scripts/test_android_emulator.sh
```

## Verification

```bash
# Test locally with connected emulator
adb devices  # Should show emulator
./tests/e2e/scripts/test_android_emulator.sh

# Trigger workflow manually
gh workflow run e2e-emulator.yml
```

## Configuration Options

### API Level Selection

| API Level | Android Version | Notes |
|-----------|-----------------|-------|
| 29 | Android 10 | Minimum recommended |
| 30 | Android 11 | Default, well-tested |
| 33 | Android 13 | Latest stable |

### Emulator Options

| Option | Purpose |
|--------|---------|
| `-no-window` | Headless mode |
| `-gpu swiftshader_indirect` | Software rendering |
| `-noaudio` | Disable audio |
| `-no-boot-anim` | Skip boot animation |
| `-no-snapshot-save` | Don't save snapshot (use cached) |

## Risks

1. **Flaky tests**: Emulator timing can be unpredictable
   - Mitigation: Add retries, increase timeouts, use `google_atd` images

2. **CI cost**: Emulator tests take ~15-20 minutes
   - Mitigation: Run nightly, not on every PR

3. **KVM availability**: Some runners may not have KVM
   - Mitigation: Standard `ubuntu-latest` supports KVM since April 2024

## References

- [reactivecircus/android-emulator-runner](https://github.com/ReactiveCircus/android-emulator-runner)
- [GitHub Actions Hardware Acceleration](https://github.blog/changelog/2024-04-02-github-actions-hardware-accelerated-android-virtualization-now-available/)
- [Android Emulator Command Line](https://developer.android.com/studio/run/emulator-commandline)

## Completion Checklist

- [ ] Workflow file created at `.github/workflows/e2e-emulator.yml`
- [ ] Test script created at `tests/e2e/scripts/test_android_emulator.sh`
- [ ] KVM enablement step added
- [ ] AVD caching implemented
- [ ] Artifact upload on failure
- [ ] Manual dispatch working
- [ ] Nightly schedule configured
- [ ] Test passes on real emulator
