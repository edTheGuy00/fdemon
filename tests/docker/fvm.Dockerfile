# tests/docker/fvm.Dockerfile
#
# Tier 2 SDK detection test image: FVM v3 + Flutter stable
#
# Stage 1: Build the fdemon binary from source (Linux x86_64 ELF).
# Stage 2: Debian bookworm-slim runtime with FVM installed and a Flutter
#          project that carries a `.fvmrc` pinning the "stable" channel.

# ---------------------------------------------------------------------------
# Stage 1 – Rust builder
# ---------------------------------------------------------------------------
FROM rust:1.88-bookworm AS builder

WORKDIR /build
COPY . .
RUN cargo build --release

# ---------------------------------------------------------------------------
# Stage 2 – FVM runtime
# ---------------------------------------------------------------------------
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    git \
    unzip \
    xz-utils \
    tar \
    bash \
    && rm -rf /var/lib/apt/lists/*

# Install FVM
RUN curl -fsSL https://fvm.app/install.sh | bash
ENV PATH="/root/fvm/bin:$PATH"

# Install Flutter 3.22.0 via FVM (pinned — newer SDKs lack the VERSION file
# that fdemon's validate_sdk_path() requires).
RUN fvm install 3.22.0 --skip-pub-get

# Pre-cache the Dart SDK to avoid first-run delays during tests
RUN /root/fvm/versions/3.22.0/bin/flutter precache \
    --no-android --no-ios --no-web --no-linux --no-macos --no-windows --no-fuchsia \
    || true

# Create test project with .fvmrc pinning 3.22.0
RUN mkdir -p /test-project/linux && \
    printf 'name: test_project\ndescription: Test project for SDK detection\ndependencies:\n  flutter:\n    sdk: flutter\nenvironment:\n  sdk: ">=3.0.0 <4.0.0"\n' \
    > /test-project/pubspec.yaml && \
    printf '{"flutter":"3.22.0"}\n' > /test-project/.fvmrc

COPY --from=builder /build/target/release/fdemon /usr/local/bin/fdemon

WORKDIR /test-project
