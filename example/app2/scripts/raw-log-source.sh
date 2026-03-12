#!/usr/bin/env bash
# Simulates a native sidecar process writing raw text logs.
# Used to test fdemon's custom log source feature with format = "raw".

MESSAGES=(
  "gRPC server listening on :50051"
  "Received RPC: GetUser(id=7)"
  "Cache warm-up complete: 1024 entries"
  "TLS handshake completed"
  "Stream opened: events/device-abc"
  "Keepalive ping sent"
  "Received RPC: ListDevices()"
  "Compacting log segments"
  "Metrics exported: 42 series"
  "Stream closed: events/device-abc"
  "Certificate renewal scheduled"
  "Received RPC: UpdateConfig(v=3)"
)

i=0
while true; do
  msg="${MESSAGES[$((i % ${#MESSAGES[@]}))]}"
  echo "$(date '+%H:%M:%S') $msg"
  i=$((i + 1))
  sleep 2
done
