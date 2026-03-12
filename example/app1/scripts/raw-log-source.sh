#!/usr/bin/env bash
# Simulates a background service writing raw text logs.
# Used to test fdemon's custom log source feature with format = "raw".

MESSAGES=(
  "Processing batch 1 of 10"
  "Cache miss for key user:1234"
  "Connecting to database"
  "Query executed in 12ms"
  "Cache hit for key session:abc"
  "Processing batch 2 of 10"
  "Slow query detected: 340ms"
  "Connection pool: 3/10 active"
  "Heartbeat OK"
  "Processing batch 3 of 10"
  "GC pause: 2ms"
  "Request completed: 200 OK"
  "Rate limit check passed"
  "Worker thread idle"
  "Flushing write buffer"
)

i=0
while true; do
  msg="${MESSAGES[$((i % ${#MESSAGES[@]}))]}"
  echo "$(date '+%H:%M:%S') $msg"
  i=$((i + 1))
  sleep 2
done
