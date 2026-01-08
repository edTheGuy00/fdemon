#!/bin/bash
# Run E2E tests with retry support

if command -v cargo-nextest &> /dev/null; then
    echo "Running with nextest (retry enabled)"
    cargo nextest run --test e2e "$@"
else
    echo "Running with cargo test (no retry)"
    echo "Install nextest for retry support: cargo install cargo-nextest"
    cargo test --test e2e "$@"
fi
