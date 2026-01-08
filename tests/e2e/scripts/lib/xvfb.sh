#!/bin/bash
# Xvfb helper functions for headless testing

XVFB_DISPLAY="${XVFB_DISPLAY:-:99}"
XVFB_RESOLUTION="${XVFB_RESOLUTION:-1920x1080x24}"
XVFB_PID_FILE="/tmp/xvfb.pid"

start_xvfb() {
    export DISPLAY="$XVFB_DISPLAY"

    # Kill any existing Xvfb
    pkill -9 Xvfb 2>/dev/null || true

    # Start Xvfb
    Xvfb "$XVFB_DISPLAY" -screen 0 "$XVFB_RESOLUTION" &
    local pid=$!
    echo $pid > "$XVFB_PID_FILE"

    # Wait for display to be ready
    local retries=10
    while ! xdpyinfo -display "$XVFB_DISPLAY" >/dev/null 2>&1; do
        retries=$((retries - 1))
        if [ $retries -le 0 ]; then
            echo "ERROR: Xvfb failed to start"
            return 1
        fi
        sleep 0.5
    done

    echo "Xvfb started on $XVFB_DISPLAY (PID: $pid)"
}

stop_xvfb() {
    if [ -f "$XVFB_PID_FILE" ]; then
        kill "$(cat $XVFB_PID_FILE)" 2>/dev/null || true
        rm -f "$XVFB_PID_FILE"
    fi
    pkill -9 Xvfb 2>/dev/null || true
}

ensure_xvfb() {
    if ! xdpyinfo -display "$XVFB_DISPLAY" >/dev/null 2>&1; then
        start_xvfb
    fi
}
