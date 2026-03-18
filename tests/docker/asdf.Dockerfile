# tests/docker/asdf.Dockerfile
#
# Tier 2 SDK detection test image: asdf + flutter plugin
#
# Stage 1: Build the fdemon binary from source (Linux x86_64 ELF).
# Stage 2: Debian bookworm-slim runtime with asdf v0.14.1 installed, the
#          asdf-flutter plugin added, and Flutter latest installed globally.
#          The test project carries a `.tool-versions` file.

# ---------------------------------------------------------------------------
# Stage 1 – Rust builder
# ---------------------------------------------------------------------------
FROM rust:1.88-bookworm AS builder

WORKDIR /build
COPY . .
RUN cargo build --release

# ---------------------------------------------------------------------------
# Stage 2 – asdf runtime
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

# Install asdf v0.14.1
RUN git clone https://github.com/asdf-vm/asdf.git /root/.asdf --branch v0.14.1
ENV PATH="/root/.asdf/shims:/root/.asdf/bin:$PATH"

# Add the asdf-flutter plugin and install Flutter 3.22.0.
# The asdf-flutter plugin requires the channel suffix (e.g. "3.22.0-stable")
# to distinguish stable releases from beta/dev.
RUN asdf plugin add flutter https://github.com/asdf-community/asdf-flutter.git && \
    asdf install flutter 3.22.0-stable && \
    asdf global flutter 3.22.0-stable

# Pre-cache the Dart SDK to avoid first-run delays during tests
RUN flutter precache \
    --no-android --no-ios --no-web --no-linux --no-macos --no-windows --no-fuchsia \
    || true

# Create test project with .tool-versions pinning 3.22.0-stable
RUN mkdir -p /test-project/linux && \
    printf 'name: test_project\ndescription: Test project for SDK detection\ndependencies:\n  flutter:\n    sdk: flutter\nenvironment:\n  sdk: ">=3.0.0 <4.0.0"\n' \
    > /test-project/pubspec.yaml && \
    printf 'flutter 3.22.0-stable\n' > /test-project/.tool-versions

COPY --from=builder /build/target/release/fdemon /usr/local/bin/fdemon

WORKDIR /test-project
