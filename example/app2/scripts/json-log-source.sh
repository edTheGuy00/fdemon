#!/usr/bin/env bash
# Simulates a backend API server emitting structured JSON logs.
# Used to test fdemon's custom log source feature with format = "json".

LEVELS=("debug" "info" "info" "info" "warning" "warning" "error")
TAGS=("api-server" "api-server" "api-server" "middleware" "middleware" "api-server" "api-server")
MESSAGES=(
  "Route registered: /api/v2/devices"
  "GET /api/v2/devices 200 8ms"
  "POST /api/v2/push 201 23ms"
  "Rate limiter: 847/1000 requests this window"
  "Slow upstream: payment-service responded in 2100ms"
  "Retry attempt 2/3 for notification-service"
  "Unhandled exception in /api/v2/webhook: timeout"
)

i=0
while true; do
  idx=$((i % ${#MESSAGES[@]}))
  level="${LEVELS[$idx]}"
  tag="${TAGS[$idx]}"
  msg="${MESSAGES[$idx]}"
  ts=$(date -u '+%Y-%m-%dT%H:%M:%SZ')
  echo "{\"timestamp\": \"$ts\", \"level\": \"$level\", \"source\": \"$tag\", \"message\": \"$msg\"}"
  i=$((i + 1))
  sleep 3
done
