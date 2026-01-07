## Task: Create Dockerfile.test

**Objective**: Create a Docker image that provides a complete Flutter + Rust environment for running E2E tests against a real Flutter daemon.

**Depends on**: None

### Scope

- `Dockerfile.test`: **NEW** - Multi-stage Docker build for test environment

### Details

Create a Dockerfile that:
1. Uses `ghcr.io/cirruslabs/flutter:stable` as the base image (includes Flutter SDK)
2. Installs Rust toolchain via rustup
3. Configures headless Flutter environment (no display required)
4. Optimizes layer caching for fast rebuilds
5. Sets up working directory and entrypoint

#### Dockerfile Structure

```dockerfile
# Stage 1: Rust toolchain
FROM ghcr.io/cirruslabs/flutter:stable AS builder

# Install Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# Pre-cache Rust dependencies (optional optimization)
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release || true
RUN rm -rf src

# Stage 2: Test runner
FROM ghcr.io/cirruslabs/flutter:stable

# Copy Rust from builder
COPY --from=builder /root/.cargo /root/.cargo
COPY --from=builder /root/.rustup /root/.rustup
ENV PATH="/root/.cargo/bin:${PATH}"

# Configure Flutter for headless operation
RUN flutter config --no-analytics
RUN flutter precache --linux

# Set working directory
WORKDIR /app

# Default command
CMD ["./tests/e2e/scripts/run_all_e2e.sh"]
```

#### Key Considerations

1. **Base Image**: Use Cirrus Labs Flutter image which is well-maintained and includes Android SDK
2. **Rust Installation**: Install via rustup for version flexibility
3. **Layer Caching**:
   - Cargo.toml/Cargo.lock copied early for dependency caching
   - Source code copied last
4. **Headless Flutter**:
   - Disable analytics
   - Precache Linux platform tools
5. **No Android Emulator** (Phase 2): Tests use `flutter run --machine` without device for protocol testing

#### Environment Variables

```dockerfile
ENV FDEMON_TEST_TIMEOUT=60000
ENV FLUTTER_TEST=true
ENV CI=true
```

### Acceptance Criteria

1. `docker build -f Dockerfile.test -t fdemon-test .` completes successfully
2. Container has `flutter --version` working
3. Container has `cargo --version` working
4. Container can compile fdemon: `cargo build --release`
5. Build time <10 minutes on cold cache
6. Rebuild time <2 minutes when only source changes (layer caching working)

### Testing

```bash
# Build the image
docker build -f Dockerfile.test -t fdemon-test .

# Verify Flutter
docker run --rm fdemon-test flutter --version

# Verify Rust
docker run --rm fdemon-test cargo --version

# Verify fdemon builds
docker run --rm -v $(pwd):/app fdemon-test cargo build --release
```

### Notes

- Consider using `--mount=type=cache` for cargo registry in CI
- Image size will be large (~3-4GB) due to Flutter SDK - this is expected
- May need to add `--platform linux/amd64` for ARM Mac users
- Android SDK is included but emulator not configured (future enhancement)

---

## Completion Summary

**Status:** Not Started
