#!/usr/bin/env bash
# Simulates a microservice emitting structured JSON logs.
# Used to test fdemon's custom log source feature with format = "json".

LEVELS=("debug" "info" "info" "info" "warning" "error")
TAGS=("auth" "auth" "http" "http" "db" "scheduler")
MESSAGES=(
  "Token refresh for user 42"
  "Login successful: admin@example.com"
  "GET /api/users 200 12ms"
  "POST /api/orders 201 45ms"
  "Connection pool exhausted, waiting"
  "Cron job missed deadline: cleanup"
)

i=0
while true; do
  idx=$((i % ${#MESSAGES[@]}))
  level="${LEVELS[$idx]}"
  tag="${TAGS[$idx]}"
  msg="${MESSAGES[$idx]}"
  ts=$(date -u '+%Y-%m-%dT%H:%M:%SZ')
  echo "{\"timestamp\": \"$ts\", \"level\": \"$level\", \"tag\": \"$tag\", \"message\": \"$msg\"}"
  i=$((i + 1))
  sleep 3
done
