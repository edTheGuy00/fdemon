# tests/docker/proto.Dockerfile
#
# Tier 2 SDK detection test image: proto + flutter community plugin
#
# Stage 1: Build the fdemon binary from source (Linux x86_64 ELF).
# Stage 2: Debian bookworm-slim runtime with proto installed, the community
#          Flutter plugin registered, Flutter latest installed, and a
#          `.prototools` file pinned in the test project.
#
# Note: the Flutter plugin is community-maintained at
#       https://github.com/nickclaw/proto-flutter-plugin — check for updates
#       if `proto install flutter` begins to fail.

# ---------------------------------------------------------------------------
# Stage 1 – Rust builder
# ---------------------------------------------------------------------------
FROM rust:1.88-bookworm AS builder

WORKDIR /build
COPY . .
RUN cargo build --release

# ---------------------------------------------------------------------------
# Stage 2 – proto runtime
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

# Install proto
RUN curl -fsSL https://moonrepo.dev/install/proto.sh | bash
ENV PATH="/root/.proto/shims:/root/.proto/bin:$PATH"

# Register the community Flutter plugin and install Flutter 3.22.0 (pinned).
RUN proto plugin add flutter "github://nickclaw/proto-flutter-plugin" && \
    proto install flutter 3.22.0

# Pre-cache the Dart SDK to avoid first-run delays during tests
RUN flutter precache \
    --no-android --no-ios --no-web --no-linux --no-macos --no-windows --no-fuchsia \
    || true

# Create test project with a .prototools file pinning 3.22.0
RUN mkdir -p /test-project/linux && \
    printf 'name: test_project\ndescription: Test project for SDK detection\ndependencies:\n  flutter:\n    sdk: flutter\nenvironment:\n  sdk: ">=3.0.0 <4.0.0"\n' \
    > /test-project/pubspec.yaml && \
    cd /test-project && proto pin flutter 3.22.0

COPY --from=builder /build/target/release/fdemon /usr/local/bin/fdemon

WORKDIR /test-project
