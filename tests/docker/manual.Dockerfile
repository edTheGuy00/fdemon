# tests/docker/manual.Dockerfile
#
# Tier 2 SDK detection test image: manual Flutter installation via tarball
#
# Stage 1: Build the fdemon binary from source (Linux x86_64 ELF).
# Stage 2: Debian bookworm-slim runtime with Flutter extracted to /opt/flutter
#          and added to PATH.  No version manager config files are present in
#          the test project, so fdemon must fall back to PATH-based detection
#          (SdkSource::SystemPath).
#
# The FLUTTER_VERSION build arg defaults to 3.22.0 (a stable release with a
# known tarball URL).  Override it when building to test a different version:
#
#   docker build --build-arg FLUTTER_VERSION=3.24.0 -f manual.Dockerfile .

# ---------------------------------------------------------------------------
# Stage 1 – Rust builder
# ---------------------------------------------------------------------------
FROM rust:1.88-bookworm AS builder

WORKDIR /build
COPY . .
RUN cargo build --release

# ---------------------------------------------------------------------------
# Stage 2 – Manual install runtime
# ---------------------------------------------------------------------------
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    git \
    unzip \
    xz-utils \
    && rm -rf /var/lib/apt/lists/*

# Download and extract a specific Flutter release tarball.
# Using a tarball is significantly faster than `git clone` for CI.
ARG FLUTTER_VERSION=3.22.0
RUN curl -fsSL \
    "https://storage.googleapis.com/flutter_infra_release/releases/stable/linux/flutter_linux_${FLUTTER_VERSION}-stable.tar.xz" \
    | tar -xJ -C /opt/

ENV PATH="/opt/flutter/bin:$PATH"

# Pre-cache the Dart SDK to avoid first-run delays during tests
RUN flutter precache \
    --no-android --no-ios --no-web --no-linux --no-macos --no-windows --no-fuchsia \
    || true

# Create test project without any version manager config files.
# fdemon must detect the SDK via PATH (SdkSource::SystemPath).
RUN mkdir -p /test-project/linux && \
    printf 'name: test_project\ndescription: Test project for SDK detection\ndependencies:\n  flutter:\n    sdk: flutter\nenvironment:\n  sdk: ">=3.0.0 <4.0.0"\n' \
    > /test-project/pubspec.yaml

COPY --from=builder /build/target/release/fdemon /usr/local/bin/fdemon

WORKDIR /test-project
