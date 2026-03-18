# tests/docker/windows-wine.Dockerfile
#
# Multi-stage Dockerfile for Windows SDK detection tests via Wine.
#
# Stage 1: Cross-compile fdemon for Windows (x86_64-pc-windows-gnu target) using
#          MinGW inside a Rust builder container.  This exercises the Windows
#          conditional-compilation paths (cfg(target_os = "windows")) without
#          requiring a real Windows host.
#
# Stage 2: Debian bookworm-slim with Wine64 installed and a simulated Windows
#          Flutter SDK layout.  fdemon.exe is run under wine64 as a smoke test.
#
# Limitations:
# - Wine is not a perfect Windows emulator; filesystem semantics differ subtly.
# - Wine does not emulate `cmd /c`, `where.exe`, or the Windows registry reliably.
# - This is a SMOKE TEST only: it verifies that the cross-compiled binary starts
#   without panicking, not that all Windows code paths behave correctly.
# - Real Windows CI should be added separately for comprehensive coverage.
#
# Cross-compilation notes:
# - Uses the x86_64-pc-windows-gnu target (MinGW toolchain) — no MSVC.
# - Some crates with native C dependencies (e.g. OpenSSL) may require feature
#   flags or replacement crates to compile cleanly for this target.  If the
#   build fails here, investigate linker errors and consult the task notes.

# ---------------------------------------------------------------------------
# Stage 1 – Cross-compile fdemon for Windows
# ---------------------------------------------------------------------------
FROM rust:1.88-bookworm AS builder

# Install the MinGW cross-compiler toolchain required for the
# x86_64-pc-windows-gnu Rust target.
RUN apt-get update && \
    apt-get install -y --no-install-recommends gcc-mingw-w64-x86-64 && \
    rm -rf /var/lib/apt/lists/*

# Add the Windows GNU target to the toolchain installed in this image.
RUN rustup target add x86_64-pc-windows-gnu

WORKDIR /build

# Copy the full workspace source tree.
# .dockerignore keeps the build context lean by excluding target/, .git/, docs/, etc.
COPY . .

# Cross-compile the release binary for Windows.
# The resulting .exe is at /build/target/x86_64-pc-windows-gnu/release/fdemon.exe
RUN cargo build --release --target x86_64-pc-windows-gnu

# ---------------------------------------------------------------------------
# Stage 2 – Wine runtime with simulated Windows Flutter SDK layout
# ---------------------------------------------------------------------------
FROM debian:bookworm-slim

# Install Wine64 and Xvfb.
# wine64   — runs 64-bit Windows PE executables on Linux
# xvfb     — virtual framebuffer; Wine may probe for a display even in headless mode
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        file \
        wine64 \
        xvfb \
    && rm -rf /var/lib/apt/lists/*

# Suppress Wine debug spam so test output is readable.
ENV WINEDEBUG="-all"
ENV WINEPREFIX="/root/.wine"

# Initialise the Wine prefix (creates the registry hive and drive stubs).
# The `|| true` prevents the build from failing if wineboot returns non-zero
# on first run in a headless environment.
RUN wineboot --init 2>/dev/null || true

# ---------------------------------------------------------------------------
# Simulated Windows Flutter SDK layout
#
# Wine maps the Linux root filesystem to the Z:\ drive, so the Flutter SDK
# placed at /flutter/ is accessible inside the .exe as Z:\flutter\.
#
# Layout mirrors a typical manual Windows Flutter installation:
#   /flutter/
#     bin/
#       flutter.bat   — Windows batch launcher (the file fdemon.exe looks for)
#       flutter       — Unix shell script (may coexist on some installs)
#     VERSION         — Flutter version string
#     .git/HEAD       — git HEAD encoding the release channel
#     bin/cache/dart-sdk/  — (empty) Dart SDK cache directory stub
# ---------------------------------------------------------------------------
RUN mkdir -p /flutter/bin/cache/dart-sdk && \
    printf '@echo off\r\necho Flutter 3.22.0\r\n' > /flutter/bin/flutter.bat && \
    printf '3.22.0' > /flutter/VERSION && \
    mkdir -p /flutter/.git && \
    printf 'ref: refs/heads/stable\n' > /flutter/.git/HEAD

# Some Windows Flutter installs also include the Unix shell script alongside
# the .bat file.  Create it so the container layout is realistic.
RUN printf '#!/bin/sh\necho Flutter 3.22.0\n' > /flutter/bin/flutter && \
    chmod +x /flutter/bin/flutter

# ---------------------------------------------------------------------------
# Minimal Flutter test project
# ---------------------------------------------------------------------------
RUN mkdir -p /test-project/linux && \
    printf 'name: test_project\r\ndescription: Test\r\ndependencies:\r\n  flutter:\r\n    sdk: flutter\r\nenvironment:\r\n  sdk: ">=3.0.0 <4.0.0"\r\n' \
    > /test-project/pubspec.yaml

# ---------------------------------------------------------------------------
# Copy the cross-compiled Windows binary
# ---------------------------------------------------------------------------
COPY --from=builder /build/target/x86_64-pc-windows-gnu/release/fdemon.exe /app/fdemon.exe

# Wine PATH: map /flutter/bin to Z:\flutter\bin so fdemon.exe can find it.
# Wine uses Z:\ as the mount point for the Linux root filesystem.
ENV WINEPATH="Z:\\flutter\\bin"
