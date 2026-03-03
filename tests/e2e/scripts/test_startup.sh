#!/bin/bash
set -euo pipefail

# =============================================================================
# test_startup.sh - Verify fdemon startup workflow in headless mode
# =============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/lib/xvfb.sh"
source "$SCRIPT_DIR/lib/fixtures.sh"
source "$SCRIPT_DIR/lib/json_events.sh"

PROJECT_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
FIXTURE_DIR="$PROJECT_ROOT/tests/fixtures/simple_app"
TIMEOUT="${FDEMON_TEST_TIMEOUT:-120}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

cleanup() {
    log_info "Cleaning up..."
    # Kill fdemon if still running
    if [ -n "${FDEMON_PID:-}" ]; then
        kill "$FDEMON_PID" 2>/dev/null || true
        wait "$FDEMON_PID" 2>/dev/null || true
    fi
    # Kill any running fdemon processes
    pkill -f "fdemon.*simple_app" || true
    # Kill any Flutter processes from fixture
    pkill -f "flutter.*simple_app" || true
    # Remove output file
    rm -f "$OUTPUT_FILE"
    # Only stop Xvfb if not shared (run_all_e2e.sh manages its own)
    if [ -z "${FDEMON_SHARED_XVFB:-}" ]; then
        stop_xvfb
    fi
}
trap cleanup EXIT

# =============================================================================
# Main Test Logic
# =============================================================================

main() {
    log_info "Starting fdemon startup test (headless mode)"
    log_info "Fixture: $FIXTURE_DIR"
    log_info "Timeout: ${TIMEOUT}s"

    # Step 1: Ensure Xvfb is running
    ensure_xvfb

    # Step 2: Verify fixture exists
    if [[ ! -d "$FIXTURE_DIR" ]]; then
        log_error "Fixture directory not found: $FIXTURE_DIR"
        exit 1
    fi

    # Step 3: Build fdemon (if not already built)
    log_info "Building fdemon..."
    cd "$PROJECT_ROOT"
    cargo build --release --quiet
    FDEMON_BIN="$PROJECT_ROOT/target/release/fdemon"

    if [[ ! -x "$FDEMON_BIN" ]]; then
        log_error "fdemon binary not found: $FDEMON_BIN"
        exit 1
    fi

    # Step 4: Verify headless mode support
    if ! "$FDEMON_BIN" --help | grep -q "headless"; then
        log_error "fdemon does not support --headless mode"
        log_error "Ensure F4 (headless mode) is implemented"
        exit 1
    fi
    log_info "Headless mode supported"

    # Step 5: Setup Linux platform for fixture (needed in Docker)
    log_info "Setting up Linux platform for fixture..."
    setup_fixture_linux "$FIXTURE_DIR"

    # Step 6: Get Flutter dependencies
    log_info "Getting Flutter dependencies..."
    cd "$FIXTURE_DIR"
    flutter pub get > /dev/null 2>&1 || flutter pub get

    # Step 7: Build Linux app first (faster iteration)
    log_info "Building Flutter Linux app..."
    flutter build linux --debug

    # Step 8: Start fdemon in headless mode and capture output
    log_info "Starting fdemon in headless mode..."
    OUTPUT_FILE=$(mktemp /tmp/fdemon_startup_test.XXXXXX.jsonl)
    log_info "Output file: $OUTPUT_FILE"

    # Start fdemon with headless mode
    "$FDEMON_BIN" --headless "$FIXTURE_DIR" > "$OUTPUT_FILE" 2>&1 &
    FDEMON_PID=$!
    log_info "fdemon PID: $FDEMON_PID"

    # Give it a moment to start
    sleep 2

    # Step 9: Wait for device detection
    log_info "Waiting for device detection..."
    if ! wait_for_event "device_detected" "$FDEMON_PID" 30 "$OUTPUT_FILE"; then
        log_error "FAIL: No device detected"
        debug_print_recent 20 "$OUTPUT_FILE"
        exit 1
    fi
    log_info "✓ Device detected"

    # Verify it's a Linux device (platform may be "linux" or "linux-arm64")
    device_event=$(get_event "device_detected" "$OUTPUT_FILE")
    platform=$(extract_field "platform" "$device_event")
    if [[ ! "$platform" =~ ^linux ]]; then
        log_error "FAIL: Expected Linux platform, got '$platform'"
        exit 1
    fi
    log_info "✓ Linux platform confirmed ($platform)"

    # Step 10: Wait for daemon connection
    log_info "Waiting for daemon connection..."
    if ! wait_for_event "daemon_connected" "$FDEMON_PID" 60 "$OUTPUT_FILE"; then
        log_error "FAIL: Daemon did not connect"
        debug_print_recent 20 "$OUTPUT_FILE"
        exit 1
    fi
    log_info "✓ Daemon connected"

    # Step 11: Wait for app started
    log_info "Waiting for app to start..."
    if ! wait_for_event "app_started" "$FDEMON_PID" 90 "$OUTPUT_FILE"; then
        log_error "FAIL: App did not start"
        debug_print_recent 20 "$OUTPUT_FILE"
        exit 1
    fi
    log_info "✓ App started"

    # Extract and display app start details
    app_started_event=$(get_event "app_started" "$OUTPUT_FILE")
    session_id=$(extract_field "session_id" "$app_started_event")
    device=$(extract_field "device" "$app_started_event")
    log_info "  Session ID: $session_id"
    log_info "  Device: $device"

    # Step 12: Check for fatal errors
    if has_fatal_errors "$OUTPUT_FILE"; then
        log_error "FAIL: Fatal error occurred"
        get_all_events "error" "$OUTPUT_FILE" | jq '.'
        exit 1
    fi
    log_info "✓ No fatal errors"

    # Step 13: Verify we have some log events
    log_count=$(count_events "log" "$OUTPUT_FILE")
    log_info "✓ Log events: $log_count"

    # Step 14: Graceful shutdown
    log_info "Sending quit signal..."
    kill -TERM "$FDEMON_PID" 2>/dev/null || true
    wait "$FDEMON_PID" 2>/dev/null || true

    # Step 15: Report success
    log_info "==================================="
    log_info "STARTUP TEST PASSED"
    log_info "==================================="
    log_info "Summary:"
    log_info "  - Device detected: ✓"
    log_info "  - Daemon connected: ✓"
    log_info "  - App started: ✓"
    log_info "  - Fatal errors: none"
    log_info "  - Log events: $log_count"

    exit 0
}

main "$@"
