#!/bin/bash
set -uo pipefail  # Note: no -e, we handle errors ourselves

# =============================================================================
# run_all_e2e.sh - Master E2E test orchestrator (headless mode)
# =============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/lib/xvfb.sh"
source "$SCRIPT_DIR/lib/fixtures.sh"

PROJECT_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
LOG_DIR="${FDEMON_LOG_DIR:-$PROJECT_ROOT/test-logs}"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_header() { echo -e "\n${BLUE}=== $1 ===${NC}\n"; }

# Test results tracking
declare -A TEST_RESULTS
declare -A TEST_DURATIONS
TESTS_PASSED=0
TESTS_FAILED=0

# =============================================================================
# Test Definitions
# =============================================================================

# Add tests here in order of execution
TESTS=(
    "startup:test_startup.sh:Verify fdemon startup workflow"
    "hot_reload:test_hot_reload.sh:Verify hot reload functionality"
    # Add more tests as they are created
    # "error_handling:test_errors.sh:Verify error handling"
    # "multi_session:test_sessions.sh:Verify multi-session support"
)

# =============================================================================
# Functions
# =============================================================================

check_dependencies() {
    log_header "Checking Dependencies"

    local missing=()

    # Check for jq (required for JSON parsing)
    if ! command -v jq &>/dev/null; then
        log_error "jq is required for headless mode tests"
        missing+=("jq")
    else
        log_info "✓ jq: $(jq --version)"
    fi

    # Check for Flutter
    if ! command -v flutter &>/dev/null; then
        log_error "Flutter not found in PATH"
        missing+=("flutter")
    else
        log_info "✓ Flutter: $(flutter --version | head -1)"
    fi

    # Check for Rust/Cargo
    if ! command -v cargo &>/dev/null; then
        log_error "Cargo not found in PATH"
        missing+=("cargo")
    else
        log_info "✓ Cargo: $(cargo --version)"
    fi

    if [ ${#missing[@]} -gt 0 ]; then
        log_error "Missing dependencies: ${missing[*]}"
        echo ""
        echo "Installation instructions:"
        for dep in "${missing[@]}"; do
            case "$dep" in
                jq)
                    echo "  - jq: apt-get install jq (Debian/Ubuntu) or brew install jq (macOS)"
                    ;;
                flutter)
                    echo "  - Flutter: https://flutter.dev/docs/get-started/install"
                    ;;
                cargo)
                    echo "  - Rust/Cargo: https://rustup.rs/"
                    ;;
            esac
        done
        return 1
    fi

    return 0
}

setup() {
    log_header "E2E Test Suite Setup"

    # Check dependencies first
    if ! check_dependencies; then
        exit 1
    fi

    # Start Xvfb once for all tests
    log_info "Starting Xvfb for headless testing..."
    start_xvfb

    # Create log directory
    mkdir -p "$LOG_DIR"
    log_info "Log directory: $LOG_DIR"

    # Build fdemon once for all tests
    log_info "Building fdemon..."
    cd "$PROJECT_ROOT"
    if ! cargo build --release --quiet; then
        log_error "Failed to build fdemon"
        stop_xvfb
        exit 1
    fi
    log_info "Build complete"

    # Ensure fdemon supports headless mode
    if ! "$PROJECT_ROOT/target/release/fdemon" --help | grep -q "headless"; then
        log_error "fdemon does not support --headless mode"
        log_error "Ensure F4 (fdemon headless mode) is implemented"
        stop_xvfb
        exit 1
    fi
    log_info "✓ Headless mode supported"

    # Setup Linux platform for fixtures (needed in Docker)
    log_info "Setting up Flutter fixtures for Linux..."
    setup_fixture_linux "$PROJECT_ROOT/tests/fixtures/simple_app"
}

run_test() {
    local test_name=$1
    local test_script=$2
    local test_description=$3
    local log_file="$LOG_DIR/${test_name}_${TIMESTAMP}.log"

    log_header "Running: $test_description"
    log_info "Script: $test_script"
    log_info "Log: $log_file"

    local start_time=$(date +%s)

    # Run test and capture output
    if "$SCRIPT_DIR/$test_script" > "$log_file" 2>&1; then
        local end_time=$(date +%s)
        local duration=$((end_time - start_time))

        TEST_RESULTS[$test_name]="PASSED"
        TEST_DURATIONS[$test_name]=$duration
        TESTS_PASSED=$((TESTS_PASSED + 1))

        log_info "${GREEN}PASSED${NC} (${duration}s)"
    else
        local exit_code=$?
        local end_time=$(date +%s)
        local duration=$((end_time - start_time))

        TEST_RESULTS[$test_name]="FAILED (exit $exit_code)"
        TEST_DURATIONS[$test_name]=$duration
        TESTS_FAILED=$((TESTS_FAILED + 1))

        log_error "${RED}FAILED${NC} (exit $exit_code, ${duration}s)"
        log_error "See log: $log_file"

        # Show last 20 lines of log on failure
        if [[ -f "$log_file" ]]; then
            echo ""
            echo "--- Last 20 lines of output ---"
            tail -20 "$log_file"
            echo "--- End of output ---"
            echo ""
        fi
    fi
}

print_summary() {
    log_header "E2E Test Summary"

    local total=$((TESTS_PASSED + TESTS_FAILED))

    echo "Results:"
    echo "--------"
    for test_info in "${TESTS[@]}"; do
        IFS=':' read -r name script desc <<< "$test_info"
        local result="${TEST_RESULTS[$name]:-NOT RUN}"
        local duration="${TEST_DURATIONS[$name]:-0}"

        if [[ "$result" == "PASSED" ]]; then
            echo -e "  ${GREEN}✓${NC} $desc (${duration}s)"
        elif [[ "$result" == "NOT RUN" ]]; then
            echo -e "  ${YELLOW}○${NC} $desc (not run)"
        else
            echo -e "  ${RED}✗${NC} $desc (${duration}s) - $result"
        fi
    done

    echo ""
    echo "--------"
    echo -e "Total: $total | ${GREEN}Passed: $TESTS_PASSED${NC} | ${RED}Failed: $TESTS_FAILED${NC}"
    echo "Logs: $LOG_DIR"
    echo ""

    if [[ $TESTS_FAILED -gt 0 ]]; then
        log_error "Some tests failed!"
        return 1
    else
        log_info "All tests passed!"
        return 0
    fi
}

cleanup_old_logs() {
    # Keep logs from last ~50 test runs
    log_info "Cleaning up old logs..."
    cd "$LOG_DIR" 2>/dev/null || return
    ls -t *.log 2>/dev/null | tail -n +51 | xargs rm -f 2>/dev/null || true
}

# =============================================================================
# Main
# =============================================================================

cleanup_xvfb() {
    log_info "Stopping Xvfb..."
    stop_xvfb
}

main() {
    log_header "Flutter Demon E2E Test Suite (Headless Mode)"
    log_info "Started at: $(date)"
    log_info "Project root: $PROJECT_ROOT"

    # Ensure Xvfb is stopped on exit
    trap cleanup_xvfb EXIT

    setup
    cleanup_old_logs

    # Run all tests
    for test_info in "${TESTS[@]}"; do
        IFS=':' read -r name script desc <<< "$test_info"
        run_test "$name" "$script" "$desc"
    done

    # Print summary and exit with appropriate code
    if print_summary; then
        exit 0
    else
        exit 1
    fi
}

main "$@"
