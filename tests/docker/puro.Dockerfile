# tests/docker/puro.Dockerfile
#
# Tier 2 SDK detection test image: Puro + default environment
#
# Stage 1: Build the fdemon binary from source (Linux x86_64 ELF).
# Stage 2: Debian bookworm-slim runtime with Puro installed, a "default"
#          Puro environment created with Flutter stable, and a `.puro.json`
#          config file in the test project.
#
# Note: PURO_ROOT is set explicitly because Puro's installer may not detect
#       $HOME correctly inside Docker (where UID 0 is used).

# ---------------------------------------------------------------------------
# Stage 1 – Rust builder
# ---------------------------------------------------------------------------
FROM rust:1.88-bookworm AS builder

WORKDIR /build
COPY . .
RUN cargo build --release

# ---------------------------------------------------------------------------
# Stage 2 – Puro runtime
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

# Install Puro with an explicit PURO_ROOT so the path is deterministic.
# Create a shell profile first — Puro's installer expects one to exist.
ENV HOME="/root"
ENV PURO_ROOT="/root/.puro"
RUN touch /root/.profile && \
    curl -o- https://puro.dev/install.sh | bash
ENV PATH="/root/.puro/bin:/root/.puro/envs/default/bin:$PATH"

# Create a "default" Puro environment with Flutter 3.22.0 (pinned — newer SDKs
# lack the VERSION file that fdemon requires).
RUN puro create default --flutter-version 3.22.0

# Pre-cache the Dart SDK to avoid first-run delays during tests
RUN /root/.puro/envs/default/flutter/bin/flutter precache \
    --no-android --no-ios --no-web --no-linux --no-macos --no-windows --no-fuchsia \
    || true

# Create test project with a .puro.json referencing the "default" environment
RUN mkdir -p /test-project/linux && \
    printf 'name: test_project\ndescription: Test project for SDK detection\ndependencies:\n  flutter:\n    sdk: flutter\nenvironment:\n  sdk: ">=3.0.0 <4.0.0"\n' \
    > /test-project/pubspec.yaml && \
    printf '{"env":"default"}\n' > /test-project/.puro.json

COPY --from=builder /build/target/release/fdemon /usr/local/bin/fdemon

WORKDIR /test-project
