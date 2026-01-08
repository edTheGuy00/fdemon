## Task: Create test_hot_reload.sh Script

**Objective**: Create a bash script that verifies fdemon correctly triggers hot reload when files change and handles reload success/failure scenarios.

**Depends on**: 07-test-startup-script (reuses patterns)

### Scope

- `tests/e2e/scripts/test_hot_reload.sh`: **NEW** - Hot reload workflow verification script

### Details

Create a bash script that:
1. Starts fdemon with simple_app fixture
2. Waits for app to be running
3. Modifies a Dart file to trigger hot reload
4. Verifies reload completes successfully
5. Tests reload failure scenarios (optional)
6. Reports success/failure

#### Script Structure

```bash
#!/bin/bash
set -euo pipefail

# =============================================================================
# test_hot_reload.sh - Verify fdemon hot reload workflow
# =============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
FIXTURE_DIR="$PROJECT_ROOT/tests/fixtures/simple_app"
TIMEOUT="${FDEMON_TEST_TIMEOUT:-60}"
RELOAD_TIMEOUT="${FDEMON_RELOAD_TIMEOUT:-30}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

cleanup() {
    log_info "Cleaning up..."
    pkill -f "fdemon.*simple_app" || true
    pkill -f "flutter.*simple_app" || true
    # Restore original main.dart
    if [[ -f "$FIXTURE_DIR/lib/main.dart.bak" ]]; then
        mv "$FIXTURE_DIR/lib/main.dart.bak" "$FIXTURE_DIR/lib/main.dart"
    fi
}
trap cleanup EXIT

wait_for_pattern() {
    local file=$1
    local pattern=$2
    local timeout=$3
    local description=$4

    log_info "Waiting for: $description"
    for i in $(seq 1 "$timeout"); do
        if grep -q "$pattern" "$file" 2>/dev/null; then
            return 0
        fi
        sleep 1
    done
    return 1
}

# =============================================================================
# Main Test Logic
# =============================================================================

main() {
    log_info "Starting fdemon hot reload test"
    log_info "Fixture: $FIXTURE_DIR"

    # Step 1: Verify fixture exists
    if [[ ! -d "$FIXTURE_DIR" ]]; then
        log_error "Fixture directory not found: $FIXTURE_DIR"
        exit 1
    fi

    # Step 2: Build fdemon
    log_info "Building fdemon..."
    cd "$PROJECT_ROOT"
    cargo build --release --quiet
    FDEMON_BIN="$PROJECT_ROOT/target/release/fdemon"

    # Step 3: Get Flutter dependencies
    log_info "Getting Flutter dependencies..."
    cd "$FIXTURE_DIR"
    flutter pub get --quiet

    # Step 4: Backup main.dart
    cp "$FIXTURE_DIR/lib/main.dart" "$FIXTURE_DIR/lib/main.dart.bak"

    # Step 5: Start fdemon
    log_info "Starting fdemon..."
    OUTPUT_FILE=$(mktemp)
    timeout "$TIMEOUT" "$FDEMON_BIN" "$FIXTURE_DIR" 2>&1 | tee "$OUTPUT_FILE" &
    FDEMON_PID=$!

    # Step 6: Wait for app to be running
    if ! wait_for_pattern "$OUTPUT_FILE" "app.started\|Running" "$TIMEOUT" "app running"; then
        log_error "Timeout waiting for app to start"
        cat "$OUTPUT_FILE"
        exit 1
    fi
    log_info "App is running!"

    # Give app time to stabilize
    sleep 2

    # Step 7: Record current line count to detect reload completion
    BEFORE_LINES=$(wc -l < "$OUTPUT_FILE")

    # Step 8: Modify main.dart to trigger reload
    log_info "Modifying main.dart to trigger hot reload..."
    TIMESTAMP=$(date +%s)
    sed -i.tmp "s/Hello from simple_app/Hot Reload Test $TIMESTAMP/" \
        "$FIXTURE_DIR/lib/main.dart"
    rm -f "$FIXTURE_DIR/lib/main.dart.tmp"

    # Step 9: Wait for reload to complete
    log_info "Waiting for hot reload to complete..."
    RELOAD_DETECTED=false
    for i in $(seq 1 "$RELOAD_TIMEOUT"); do
        # Check for reload-related messages
        if tail -n +$BEFORE_LINES "$OUTPUT_FILE" | grep -qi "reload\|Reloaded\|app.progress"; then
            log_info "Reload activity detected!"
            RELOAD_DETECTED=true
            break
        fi
        sleep 1
    done

    if [[ "$RELOAD_DETECTED" != "true" ]]; then
        log_error "Timeout waiting for hot reload"
        log_error "Output after modification:"
        tail -n +$BEFORE_LINES "$OUTPUT_FILE"
        exit 1
    fi

    # Step 10: Wait a bit more for reload to complete
    sleep 3

    # Step 11: Verify reload success (no errors)
    if tail -n +$BEFORE_LINES "$OUTPUT_FILE" | grep -qi "reload.*fail\|error.*reload"; then
        log_error "Hot reload appears to have failed"
        tail -n +$BEFORE_LINES "$OUTPUT_FILE"
        exit 1
    fi

    # Step 12: Graceful shutdown
    log_info "Sending quit signal..."
    kill -TERM "$FDEMON_PID" 2>/dev/null || true
    wait "$FDEMON_PID" 2>/dev/null || true

    # Step 13: Report success
    log_info "==================================="
    log_info "HOT RELOAD TEST PASSED"
    log_info "==================================="

    rm -f "$OUTPUT_FILE"
    exit 0
}

main "$@"
```

#### Test Scenarios

1. **Basic Hot Reload**:
   - Modify Dart file
   - Verify reload triggers
   - Verify reload completes

2. **Reload After Error Recovery** (stretch):
   - Introduce syntax error
   - Verify error is reported
   - Fix error
   - Verify reload works

3. **Multiple Rapid Reloads** (stretch):
   - Make several changes quickly
   - Verify debouncing works
   - Verify final state is correct

### Key Considerations

1. **File Modification**:
   - Use `sed` for in-place modification
   - Backup original file
   - Restore on cleanup

2. **Reload Detection**:
   - Look for reload-related log messages
   - Track line count before/after
   - Handle different output formats

3. **Timing**:
   - Account for file watcher debounce
   - Allow time for reload to complete
   - Don't rush assertions

### Acceptance Criteria

1. Script exits 0 when hot reload succeeds
2. Script detects when reload is triggered
3. Script detects reload completion
4. Script restores original main.dart on exit
5. Script works in Docker environment
6. Script respects timeout environment variables

### Testing

```bash
# Make script executable
chmod +x tests/e2e/scripts/test_hot_reload.sh

# Run locally
./tests/e2e/scripts/test_hot_reload.sh

# Run with extended timeout
FDEMON_RELOAD_TIMEOUT=60 ./tests/e2e/scripts/test_hot_reload.sh

# Run in Docker
docker-compose -f docker-compose.test.yml run --rm flutter-e2e-hot-reload
```

### Notes

- Hot reload requires app to be in "running" state
- File watcher debounce affects timing
- Consider testing restart (`R`) vs reload (`r`)
- Error recovery testing is complex and may be a separate task

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `tests/e2e/scripts/test_hot_reload.sh` | Created new bash script for hot reload workflow verification |

### Notable Decisions/Tradeoffs

1. **Backup/Restore Pattern**: Used backup (.bak) and trap cleanup to ensure main.dart is always restored, even on script failure.
2. **Reload Detection**: Used line count tracking (BEFORE_LINES) to isolate reload-related output from startup output, preventing false positives.
3. **Sed Modification**: Used timestamp in modification to ensure uniqueness and verify change propagation.
4. **Pattern Matching**: Used grep with multiple patterns (reload|Reloaded|app.progress) to catch various reload indicators from Flutter daemon output.

### Testing Performed

- `bash -n tests/e2e/scripts/test_hot_reload.sh` - Passed (syntax check)
- `chmod +x tests/e2e/scripts/test_hot_reload.sh` - Passed (script is executable)
- Script structure follows test_startup.sh patterns (error handling, colors, cleanup)

### Risks/Limitations

1. **Flutter Environment Required**: Script requires Flutter SDK and working environment; will fail in minimal test containers without Flutter installed.
2. **Timing Sensitivity**: Reload detection relies on debounce timing and file watcher responsiveness; may need timeout adjustments in slower CI environments.
3. **Pattern Matching Fragility**: Relies on Flutter daemon output format; changes to daemon message format could break detection logic.
4. **Sed Platform Differences**: Uses `sed -i.tmp` syntax which is compatible with both GNU and BSD sed, but creates temporary .tmp files that are immediately removed.
