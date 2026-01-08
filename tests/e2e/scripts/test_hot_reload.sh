#!/bin/bash
set -euo pipefail

# =============================================================================
# test_hot_reload.sh - Verify fdemon hot reload workflow in headless mode
# =============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/lib/xvfb.sh"
source "$SCRIPT_DIR/lib/fixtures.sh"
source "$SCRIPT_DIR/lib/json_events.sh"

PROJECT_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
FIXTURE_DIR="$PROJECT_ROOT/tests/fixtures/simple_app"
TIMEOUT="${FDEMON_TEST_TIMEOUT:-120}"
RELOAD_TIMEOUT="${FDEMON_RELOAD_TIMEOUT:-30}"

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
    # Close stdin pipe file descriptor
    exec 3>&- 2>/dev/null || true
    # Remove named pipe
    rm -f "$STDIN_PIPE"
    # Kill fdemon if still running
    if [ -n "${FDEMON_PID:-}" ]; then
        kill "$FDEMON_PID" 2>/dev/null || true
        wait "$FDEMON_PID" 2>/dev/null || true
    fi
    # Kill any running fdemon processes
    pkill -f "fdemon.*simple_app" || true
    # Kill any Flutter processes from fixture
    pkill -f "flutter.*simple_app" || true
    # Restore original main.dart
    if [[ -f "$FIXTURE_DIR/lib/main.dart.bak" ]]; then
        mv "$FIXTURE_DIR/lib/main.dart.bak" "$FIXTURE_DIR/lib/main.dart"
        log_info "Restored main.dart"
    fi
    # Remove output file
    rm -f "$OUTPUT_FILE"
    # Stop Xvfb
    stop_xvfb
}
trap cleanup EXIT

# =============================================================================
# Main Test Logic
# =============================================================================

main() {
    log_info "Starting fdemon hot reload test (headless mode)"
    log_info "Fixture: $FIXTURE_DIR"
    log_info "Timeout: ${TIMEOUT}s"
    log_info "Reload Timeout: ${RELOAD_TIMEOUT}s"

    # Step 1: Ensure Xvfb is running
    ensure_xvfb

    # Step 2: Verify fixture exists
    if [[ ! -d "$FIXTURE_DIR" ]]; then
        log_error "Fixture directory not found: $FIXTURE_DIR"
        exit 1
    fi

    # Step 3: Build fdemon
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
        exit 1
    fi

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

    # Step 8: Backup main.dart
    log_info "Backing up main.dart..."
    cp "$FIXTURE_DIR/lib/main.dart" "$FIXTURE_DIR/lib/main.dart.bak"

    # Step 9: Create named pipe for stdin commands
    STDIN_PIPE="/tmp/fdemon_stdin_$$.pipe"
    rm -f "$STDIN_PIPE"
    mkfifo "$STDIN_PIPE"
    log_info "Created stdin pipe: $STDIN_PIPE"

    # Step 10: Start fdemon in headless mode with stdin pipe
    log_info "Starting fdemon in headless mode..."
    OUTPUT_FILE=$(mktemp /tmp/fdemon_hot_reload_test.XXXXXX.jsonl)
    log_info "Output file: $OUTPUT_FILE"

    # Start fdemon with stdin from pipe
    "$FDEMON_BIN" --headless "$FIXTURE_DIR" < "$STDIN_PIPE" > "$OUTPUT_FILE" 2>&1 &
    FDEMON_PID=$!
    log_info "fdemon PID: $FDEMON_PID"

    # Open pipe for writing (keep it open)
    exec 3>"$STDIN_PIPE"

    # Give it a moment to start
    sleep 2

    # Step 11: Wait for app to be running
    log_info "Waiting for app to start..."
    if ! wait_for_event "app_started" "$FDEMON_PID" 90 "$OUTPUT_FILE"; then
        log_error "FAIL: App did not start"
        debug_print_recent 20 "$OUTPUT_FILE"
        exit 1
    fi
    log_info "✓ App started"

    # Give app time to stabilize
    sleep 3

    # Step 12: Record initial reload count
    initial_reloads=$(count_events "hot_reload_completed" "$OUTPUT_FILE")
    log_info "Initial hot reload count: $initial_reloads"

    # Step 13: Trigger hot reload via stdin
    log_info "Triggering hot reload via stdin (command 'r')..."
    echo "r" >&3

    # Step 14: Wait for hot reload to start
    log_info "Waiting for hot reload to start..."
    if ! wait_for_event "hot_reload_started" "$FDEMON_PID" 10 "$OUTPUT_FILE"; then
        log_warn "Did not detect hot_reload_started event (may not be emitted yet)"
    else
        log_info "✓ Hot reload started"
    fi

    # Step 15: Wait for hot reload completion
    log_info "Waiting for hot reload to complete..."
    sleep 5

    new_reloads=$(count_events "hot_reload_completed" "$OUTPUT_FILE")
    log_info "Hot reload count after trigger: $new_reloads"

    if [ "$new_reloads" -gt "$initial_reloads" ]; then
        log_info "✓ Hot reload completed"

        # Get reload event and check duration
        reload_event=$(get_event "hot_reload_completed" "$OUTPUT_FILE")
        duration=$(extract_field "duration_ms" "$reload_event")

        if [ -n "$duration" ]; then
            log_info "✓ Reload duration: ${duration}ms"

            # Assert duration is reasonable (not 0, not too long)
            if [ "$duration" -eq 0 ]; then
                log_warn "Warning: Reload duration is 0ms (unexpected)"
            elif [ "$duration" -gt 60000 ]; then
                log_warn "Warning: Reload duration > 60s (unusually slow)"
            fi
        fi
    else
        log_error "FAIL: Hot reload did not complete"
        log_error "Initial reloads: $initial_reloads, after trigger: $new_reloads"
        debug_print_recent 30 "$OUTPUT_FILE"
        exit 1
    fi

    # Step 16: Test file-triggered hot reload
    log_info ""
    log_info "Testing file-triggered hot reload..."
    MAIN_DART="$FIXTURE_DIR/lib/main.dart"

    # Record current reload count
    before_file_change=$(count_events "hot_reload_completed" "$OUTPUT_FILE")

    # Modify file
    TIMESTAMP=$(date +%s)
    echo "// Hot reload trigger: $TIMESTAMP" >> "$MAIN_DART"
    log_info "Modified main.dart with timestamp: $TIMESTAMP"

    # Wait for file watcher debounce + reload
    log_info "Waiting for file-triggered reload (10s)..."
    sleep 10

    file_reloads=$(count_events "hot_reload_completed" "$OUTPUT_FILE")

    # Cleanup modification (restore from backup)
    if [[ -f "$FIXTURE_DIR/lib/main.dart.bak" ]]; then
        cp "$FIXTURE_DIR/lib/main.dart.bak" "$FIXTURE_DIR/lib/main.dart"
    fi

    if [ "$file_reloads" -gt "$before_file_change" ]; then
        log_info "✓ File-triggered hot reload completed"
        log_info "  Reload count: $before_file_change → $file_reloads"
    else
        log_warn "WARN: File-triggered hot reload not detected"
        log_warn "  This may be expected if file watcher debounce is high or auto-reload is disabled"
        log_warn "  Reload count: $before_file_change → $file_reloads"
    fi

    # Step 17: Check for any fatal errors during the test
    if has_fatal_errors "$OUTPUT_FILE"; then
        log_error "FAIL: Fatal error occurred during test"
        get_all_events "error" "$OUTPUT_FILE" | jq '.'
        exit 1
    fi
    log_info "✓ No fatal errors"

    # Step 18: Graceful shutdown
    log_info "Sending quit signal..."
    echo "q" >&3
    sleep 2
    kill -TERM "$FDEMON_PID" 2>/dev/null || true
    wait "$FDEMON_PID" 2>/dev/null || true

    # Step 19: Report success
    log_info "==================================="
    log_info "HOT RELOAD TEST PASSED"
    log_info "==================================="
    log_info "Summary:"
    log_info "  - Manual hot reload (stdin): ✓"
    if [ "$file_reloads" -gt "$before_file_change" ]; then
        log_info "  - File-triggered hot reload: ✓"
    else
        log_info "  - File-triggered hot reload: ⚠ (not detected)"
    fi
    log_info "  - Total hot reloads: $file_reloads"
    log_info "  - Fatal errors: none"

    exit 0
}

main "$@"
