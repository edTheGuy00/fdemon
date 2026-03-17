# tests/docker/base.Dockerfile
#
# Multi-stage Dockerfile for SDK detection Tier 2 tests.
#
# Stage 1: Build the fdemon binary inside a Rust container so the resulting
# binary is a Linux x86_64 ELF (no cross-compilation required on macOS hosts).
#
# Stage 2: Minimal Debian runtime with common tooling.  Each version-manager
# Dockerfile (fvm.Dockerfile, asdf.Dockerfile, etc.) extends the `runtime`
# stage and installs its manager on top.

# ---------------------------------------------------------------------------
# Stage 1 – Rust builder
# ---------------------------------------------------------------------------
FROM rust:1.88-bookworm AS builder

WORKDIR /build

# Copy the full workspace source tree (honoured .dockerignore keeps this small).
COPY . .

# Build the release binary.  The layer is cached as long as source files are
# unchanged; the .dockerignore file excludes target/, .git/, docs, etc.
RUN cargo build --release

# Binary is at /build/target/release/fdemon

# ---------------------------------------------------------------------------
# Stage 2 – Minimal runtime base
# ---------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

# Install tools commonly required by version managers (curl, git, unzip, xz).
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    git \
    unzip \
    xz-utils \
    && rm -rf /var/lib/apt/lists/*

# Create a minimal Flutter project for fdemon to operate on.
# The pubspec.yaml is the minimum required by fdemon's project discovery.
RUN mkdir -p /test-project/linux && \
    printf 'name: test_project\ndescription: Test project for SDK detection\ndependencies:\n  flutter:\n    sdk: flutter\nenvironment:\n  sdk: ">=3.0.0 <4.0.0"\n' \
    > /test-project/pubspec.yaml

# Copy the fdemon binary from the builder stage.
COPY --from=builder /build/target/release/fdemon /usr/local/bin/fdemon
