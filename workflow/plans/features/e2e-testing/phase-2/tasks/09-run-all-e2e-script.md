## Task: Create run_all_e2e.sh Orchestrator Script

**Objective**: Create a master script that orchestrates all E2E test scripts, provides summary reporting, and handles failures gracefully.

**Depends on**: 07-test-startup-script, 08-test-hot-reload-script

### Scope

- `tests/e2e/scripts/run_all_e2e.sh`: **NEW** - Master E2E test orchestrator

### Details

Create a bash script that:
1. Runs all E2E test scripts in sequence
2. Collects results from each test
3. Continues on failure (run all tests even if some fail)
4. Generates summary report
5. Exits with appropriate code for CI

#### Script Structure

```bash
#!/bin/bash
set -uo pipefail  # Note: no -e, we handle errors ourselves

# =============================================================================
# run_all_e2e.sh - Master E2E test orchestrator
# =============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
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

setup() {
    log_header "E2E Test Suite Setup"

    # Create log directory
    mkdir -p "$LOG_DIR"
    log_info "Log directory: $LOG_DIR"

    # Build fdemon once for all tests
    log_info "Building fdemon..."
    cd "$PROJECT_ROOT"
    if ! cargo build --release --quiet; then
        log_error "Failed to build fdemon"
        exit 1
    fi
    log_info "Build complete"

    # Verify Flutter is available
    if ! command -v flutter &> /dev/null; then
        log_error "Flutter not found in PATH"
        exit 1
    fi
    log_info "Flutter version: $(flutter --version --machine | head -1)"
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
    # Keep logs from last 5 runs
    log_info "Cleaning up old logs..."
    cd "$LOG_DIR" 2>/dev/null || return
    ls -t *.log 2>/dev/null | tail -n +50 | xargs rm -f 2>/dev/null || true
}

# =============================================================================
# Main
# =============================================================================

main() {
    log_header "Flutter Demon E2E Test Suite"
    log_info "Started at: $(date)"
    log_info "Project root: $PROJECT_ROOT"

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
```

### Key Considerations

1. **Error Handling**:
   - Don't use `set -e` - handle errors manually
   - Continue running tests even after failures
   - Track results for each test

2. **Logging**:
   - Each test gets its own timestamped log file
   - Show tail of log on failure
   - Clean up old logs

3. **Reporting**:
   - Clear visual summary
   - Pass/fail counts
   - Duration tracking
   - Exit code reflects overall success

4. **Extensibility**:
   - Easy to add new tests to TESTS array
   - Consistent interface for all test scripts

### Acceptance Criteria

1. Script runs all defined test scripts
2. Script continues after individual test failures
3. Script produces clear summary report
4. Script exits 0 only if all tests pass
5. Script creates timestamped log files
6. Script works in Docker environment
7. Logs are retained for debugging

### Testing

```bash
# Make script executable
chmod +x tests/e2e/scripts/run_all_e2e.sh

# Run all tests locally
./tests/e2e/scripts/run_all_e2e.sh

# Run with custom log directory
FDEMON_LOG_DIR=/tmp/fdemon-e2e-logs ./tests/e2e/scripts/run_all_e2e.sh

# Run in Docker
docker-compose -f docker-compose.test.yml run --rm flutter-e2e-test
```

### Notes

- This is the default entrypoint for Docker container
- Consider adding parallel test execution later
- May want to add --filter flag for running specific tests
- Log rotation prevents disk fill in CI

---

## Completion Summary

**Status:** Not Started
