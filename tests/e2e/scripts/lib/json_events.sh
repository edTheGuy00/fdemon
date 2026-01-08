#!/bin/bash
# =============================================================================
# json_events.sh - JSON event parsing helpers for headless mode testing
# =============================================================================
#
# This library provides helper functions for parsing and asserting on
# NDJSON events output by fdemon in --headless mode.
#
# Dependencies: jq
#
# Usage:
#   source "$(dirname "$0")/lib/json_events.sh"

# Check if jq is available
if ! command -v jq &>/dev/null; then
    echo "ERROR: jq is required for headless mode testing"
    echo "Install with: apt-get install jq (Debian/Ubuntu) or brew install jq (macOS)"
    exit 1
fi

# =============================================================================
# Event Waiting Functions
# =============================================================================

# Wait for a specific event type to appear in the output file
# Usage: wait_for_event "app_started" $PID $TIMEOUT $OUTPUT_FILE
# Returns: 0 if event found, 1 on timeout or process death
wait_for_event() {
    local event_type="$1"
    local pid="$2"
    local timeout="${3:-60}"
    local output_file="${4:-/tmp/fdemon_output.jsonl}"

    local elapsed=0
    while [ $elapsed -lt $timeout ]; do
        # Check if event exists in output
        if grep -q "\"event\":\"$event_type\"" "$output_file" 2>/dev/null; then
            return 0
        fi

        sleep 1
        elapsed=$((elapsed + 1))

        # Check if process is still alive
        if ! kill -0 "$pid" 2>/dev/null; then
            echo "ERROR: fdemon process died while waiting for $event_type event"
            return 1
        fi
    done

    echo "ERROR: Timeout waiting for $event_type event (${timeout}s)"
    return 1
}

# =============================================================================
# Event Extraction Functions
# =============================================================================

# Get the most recent event of a specific type
# Usage: get_event "hot_reload_completed" /tmp/output.jsonl
# Returns: JSON event string (last matching event)
get_event() {
    local event_type="$1"
    local output_file="${2:-/tmp/fdemon_output.jsonl}"

    grep "\"event\":\"$event_type\"" "$output_file" 2>/dev/null | tail -1
}

# Get all events of a specific type
# Usage: get_all_events "log" /tmp/output.jsonl
# Returns: All matching JSON events (one per line)
get_all_events() {
    local event_type="$1"
    local output_file="${2:-/tmp/fdemon_output.jsonl}"

    grep "\"event\":\"$event_type\"" "$output_file" 2>/dev/null
}

# =============================================================================
# Event Counting Functions
# =============================================================================

# Count events of a specific type
# Usage: count_events "log" /tmp/output.jsonl
# Returns: Number of matching events (0 if none)
count_events() {
    local event_type="$1"
    local output_file="${2:-/tmp/fdemon_output.jsonl}"

    grep -c "\"event\":\"$event_type\"" "$output_file" 2>/dev/null || echo 0
}

# =============================================================================
# Error Detection Functions
# =============================================================================

# Check for any fatal error events
# Usage: has_fatal_errors /tmp/output.jsonl
# Returns: 0 (true) if fatal errors exist, 1 (false) otherwise
has_fatal_errors() {
    local output_file="${1:-/tmp/fdemon_output.jsonl}"

    if grep -q '"event":"error".*"fatal":true' "$output_file" 2>/dev/null; then
        return 0
    fi
    return 1
}

# Check for any error events (fatal or non-fatal)
# Usage: has_errors /tmp/output.jsonl
# Returns: 0 (true) if any errors exist, 1 (false) otherwise
has_errors() {
    local output_file="${1:-/tmp/fdemon_output.jsonl}"

    if grep -q '"event":"error"' "$output_file" 2>/dev/null; then
        return 0
    fi
    return 1
}

# =============================================================================
# JSON Field Extraction Functions
# =============================================================================

# Extract a field from a JSON event
# Usage: extract_field "duration_ms" "$event_json"
# Returns: Field value (or empty string if not found)
extract_field() {
    local field="$1"
    local json="$2"

    echo "$json" | jq -r ".$field // empty" 2>/dev/null
}

# Extract a nested field from a JSON event
# Usage: extract_nested_field "data.device.name" "$event_json"
# Returns: Nested field value (or empty string if not found)
extract_nested_field() {
    local field_path="$1"
    local json="$2"

    echo "$json" | jq -r ".$field_path // empty" 2>/dev/null
}

# =============================================================================
# Assertion Functions
# =============================================================================

# Assert that a field in a JSON event equals an expected value
# Usage: assert_field_equals "device" "linux" "$event_json"
# Returns: 0 if assertion passes, 1 if it fails
assert_field_equals() {
    local field="$1"
    local expected="$2"
    local json="$3"

    local actual
    actual=$(echo "$json" | jq -r ".$field // empty" 2>/dev/null)

    if [ "$actual" = "$expected" ]; then
        return 0
    else
        echo "ASSERT FAILED: Expected $field='$expected', got '$actual'"
        echo "Event JSON: $json"
        return 1
    fi
}

# Assert that a field exists in a JSON event
# Usage: assert_field_exists "duration_ms" "$event_json"
# Returns: 0 if field exists, 1 if it doesn't
assert_field_exists() {
    local field="$1"
    local json="$2"

    local value
    value=$(echo "$json" | jq -r ".$field // empty" 2>/dev/null)

    if [ -n "$value" ]; then
        return 0
    else
        echo "ASSERT FAILED: Field '$field' does not exist or is empty"
        echo "Event JSON: $json"
        return 1
    fi
}

# Assert that a numeric field is greater than a value
# Usage: assert_field_greater_than "duration_ms" 0 "$event_json"
# Returns: 0 if assertion passes, 1 if it fails
assert_field_greater_than() {
    local field="$1"
    local threshold="$2"
    local json="$3"

    local value
    value=$(echo "$json" | jq -r ".$field // 0" 2>/dev/null)

    if [ "$value" -gt "$threshold" ]; then
        return 0
    else
        echo "ASSERT FAILED: Expected $field > $threshold, got $value"
        echo "Event JSON: $json"
        return 1
    fi
}

# =============================================================================
# Debug Functions
# =============================================================================

# Print all events of a type with pretty formatting
# Usage: debug_print_events "log" /tmp/output.jsonl
debug_print_events() {
    local event_type="$1"
    local output_file="${2:-/tmp/fdemon_output.jsonl}"

    echo "=== Events of type '$event_type' ==="
    get_all_events "$event_type" "$output_file" | jq '.'
    echo "=== End of events ==="
}

# Print the last N lines of the output file with pretty formatting
# Usage: debug_print_recent 10 /tmp/output.jsonl
debug_print_recent() {
    local lines="${1:-20}"
    local output_file="${2:-/tmp/fdemon_output.jsonl}"

    echo "=== Last $lines events ==="
    tail -n "$lines" "$output_file" 2>/dev/null | jq '.' 2>/dev/null || tail -n "$lines" "$output_file"
    echo "=== End of events ==="
}
