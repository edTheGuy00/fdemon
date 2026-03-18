# tests/docker/mise.Dockerfile
#
# Tier 2 SDK detection test image: mise + Flutter
#
# Stage 1: Build the fdemon binary from source (Linux x86_64 ELF).
# Stage 2: Debian bookworm-slim runtime with mise installed, Flutter latest
#          set globally, and a `.mise.toml` in the test project directory.

# ---------------------------------------------------------------------------
# Stage 1 – Rust builder
# ---------------------------------------------------------------------------
FROM rust:1.88-bookworm AS builder

WORKDIR /build
COPY . .
RUN cargo build --release

# ---------------------------------------------------------------------------
# Stage 2 – mise runtime
# ---------------------------------------------------------------------------
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    git \
    unzip \
    xz-utils \
    bash \
    && rm -rf /var/lib/apt/lists/*

# Install mise
RUN curl https://mise.run | sh
ENV PATH="/root/.local/share/mise/shims:/root/.local/bin:$PATH"

# Install Flutter 3.22.0 via mise (pinned — newer SDKs lack the VERSION file).
# `mise use -g` has a template parsing bug in some versions, so we write the
# config file manually instead.
RUN mise install flutter@3.22.0

# Set up PATH so the installed flutter is available.
ENV PATH="/root/.local/share/mise/installs/flutter/3.22.0/bin:$PATH"

# Pre-cache the Dart SDK to avoid first-run delays during tests
RUN flutter precache \
    --no-android --no-ios --no-web --no-linux --no-macos --no-windows --no-fuchsia \
    || true

# Create test project with a .mise.toml pinning 3.22.0
RUN mkdir -p /test-project/linux && \
    printf 'name: test_project\ndescription: Test project for SDK detection\ndependencies:\n  flutter:\n    sdk: flutter\nenvironment:\n  sdk: ">=3.0.0 <4.0.0"\n' \
    > /test-project/pubspec.yaml && \
    printf '[tools]\nflutter = "3.22.0"\n' > /test-project/.mise.toml

COPY --from=builder /build/target/release/fdemon /usr/local/bin/fdemon

WORKDIR /test-project
