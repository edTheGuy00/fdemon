#!/bin/bash
# Start Xvfb virtual display for headless Flutter testing

export DISPLAY=:99

# Kill any existing Xvfb
pkill -9 Xvfb 2>/dev/null || true

# Start Xvfb with 1920x1080 display
Xvfb :99 -screen 0 1920x1080x24 &
XVFB_PID=$!

# Wait for Xvfb to be ready
sleep 2

# Verify display is available
if ! xdpyinfo -display :99 >/dev/null 2>&1; then
    echo "ERROR: Xvfb failed to start"
    exit 1
fi

echo "Xvfb started on display :99 (PID: $XVFB_PID)"
echo $XVFB_PID > /tmp/xvfb.pid
