## Task: Tier 2 — Docker Windows Tests (Wine)

**Objective**: Verify that fdemon's Windows-specific SDK detection logic works correctly, including `.bat` file detection and `FlutterExecutable::WindowsBatch` variant creation, using a Wine-based Docker container with a cross-compiled Windows binary.

**Depends on**: 04-docker-infrastructure

### Scope

- `tests/docker/windows-wine.Dockerfile`: Wine64 + cross-compiled fdemon.exe + simulated Windows Flutter layout
- `tests/sdk_detection/tier2_windows.rs`: Wine-based Windows detection tests

### Details

#### Approach

Since macOS Docker Desktop only runs Linux containers, we use **Wine in a Linux container** to execute a cross-compiled Windows binary of fdemon. This tests the Windows code paths (`cfg(target_os = "windows")` and `.bat` detection) in a controlled environment.

**Limitations:**
- Wine is not a perfect Windows emulator — filesystem semantics differ in subtle ways
- Wine does not support Windows-specific APIs like `where.exe` or `cmd /c` perfectly
- This is a **smoke test**, not a comprehensive Windows test — real Windows CI should be added later
- Wine's PATH handling may differ from native Windows

#### Dockerfile

```dockerfile
# tests/docker/windows-wine.Dockerfile

# Stage 1: Cross-compile fdemon for Windows
FROM rust:1.83-bookworm AS builder
RUN apt-get update && apt-get install -y gcc-mingw-w64-x86-64
RUN rustup target add x86_64-pc-windows-gnu
WORKDIR /build
COPY . .
RUN cargo build --release --target x86_64-pc-windows-gnu
# Binary at /build/target/x86_64-pc-windows-gnu/release/fdemon.exe

# Stage 2: Wine runtime with simulated Flutter environment
FROM debian:bookworm-slim

# Install Wine
RUN dpkg --add-architecture amd64 && \
    apt-get update && \
    apt-get install -y --no-install-recommends wine64 xvfb && \
    rm -rf /var/lib/apt/lists/*

# Initialize Wine prefix (suppress first-run dialogs)
ENV WINEPREFIX="/root/.wine"
ENV WINEDEBUG="-all"
RUN wineboot --init 2>/dev/null || true

# Create Windows-like Flutter SDK layout
# Simulates: C:\flutter\ installed manually on Windows
RUN mkdir -p /flutter/bin/cache/dart-sdk && \
    printf '@echo off\r\necho Flutter 3.22.0\r\n' > /flutter/bin/flutter.bat && \
    printf '3.22.0' > /flutter/VERSION && \
    mkdir -p /flutter/.git && \
    printf 'ref: refs/heads/stable\n' > /flutter/.git/HEAD

# Also create a Unix-style flutter script (some Windows installs have both)
RUN printf '#!/bin/sh\necho Flutter 3.22.0\n' > /flutter/bin/flutter && \
    chmod +x /flutter/bin/flutter

# Create test project
RUN mkdir -p /test-project && \
    printf 'name: test_project\r\ndescription: Test\r\nenvironment:\r\n  sdk: ">=3.0.0 <4.0.0"\r\n' \
    > /test-project/pubspec.yaml

# Copy cross-compiled binary
COPY --from=builder /build/target/x86_64-pc-windows-gnu/release/fdemon.exe /app/fdemon.exe

# Wine needs the Flutter path in its PATH
# Map the Linux paths to Wine's Z: drive (which maps / to Z:\)
ENV WINEPATH="Z:\\flutter\\bin"
```

#### Test Structure

```rust
// tests/sdk_detection/tier2_windows.rs
use super::docker_helpers::*;
use super::assertions::*;

#[test]
#[ignore = "requires Docker + Wine (cross-compiled Windows binary)"]
fn test_windows_bat_detection_via_wine() {
    if !docker_available() { return; }

    let root = project_root();
    docker_build(
        "tests/docker/windows-wine.Dockerfile",
        "fdemon-test-windows",
        &root,
    ).unwrap();

    // Run fdemon.exe via Wine in headless mode
    // Note: Wine maps Linux / to Z:\ drive, so /flutter → Z:\flutter
    let result = docker_exec(
        "fdemon-test-windows",
        &[
            "wine64", "/app/fdemon.exe", "--headless",
            "Z:\\test-project",
        ],
    ).unwrap();

    // Check that fdemon started (may fail due to Wine limitations, but should not crash)
    eprintln!("Wine stdout: {}", result.stdout);
    eprintln!("Wine stderr: {}", result.stderr);

    // Primary assertion: binary didn't crash/panic
    // Wine exit codes may differ — check stderr for Rust panic messages
    assert!(
        !result.stderr.contains("thread 'main' panicked"),
        "fdemon.exe panicked under Wine:\n{}", result.stderr
    );
}

#[test]
#[ignore = "requires Docker + Wine"]
fn test_windows_flutter_bat_exists_in_layout() {
    if !docker_available() { return; }

    // Verify the Docker image's Flutter layout has .bat file
    let result = docker_exec(
        "fdemon-test-windows",
        &["ls", "-la", "/flutter/bin/"],
    ).unwrap();

    assert!(result.stdout.contains("flutter.bat"),
        "flutter.bat not found in /flutter/bin/:\n{}", result.stdout);
}

#[test]
#[ignore = "requires Docker + Wine"]
fn test_windows_sdk_version_readable() {
    if !docker_available() { return; }

    let result = docker_exec(
        "fdemon-test-windows",
        &["cat", "/flutter/VERSION"],
    ).unwrap();

    assert!(result.stdout.trim() == "3.22.0",
        "VERSION file content unexpected: '{}'", result.stdout.trim());
}

/// Test that the cross-compilation itself succeeds and produces
/// a valid PE executable
#[test]
#[ignore = "requires Docker"]
fn test_windows_cross_compilation_produces_valid_exe() {
    if !docker_available() { return; }

    let result = docker_exec(
        "fdemon-test-windows",
        &["file", "/app/fdemon.exe"],
    ).unwrap();

    assert!(result.stdout.contains("PE32+") || result.stdout.contains("Windows"),
        "fdemon.exe is not a valid Windows executable:\n{}", result.stdout);
}
```

#### What We Can and Cannot Test

| Aspect | Testable via Wine? | Notes |
|--------|-------------------|-------|
| Cross-compilation succeeds | Yes | `file fdemon.exe` confirms PE format |
| Binary doesn't panic | Yes | Check for Rust panic in stderr |
| `.bat` file detection in code | Partially | Wine's filesystem may not distinguish `.bat` files the same way |
| `FlutterExecutable::WindowsBatch` variant | Partially | Depends on Wine's `cfg(target_os)` resolution |
| `cmd /c flutter.bat` execution | No | Wine's `cmd.exe` emulation is unreliable |
| Windows PATH resolution | Partially | Wine uses `WINEPATH` differently from real Windows `PATH` |
| Windows home dir (`%APPDATA%`) | Partially | Wine provides emulated registry but paths differ |

#### Alternative: Tempdir-Based Windows Path Logic Tests (Complementary)

Since Wine testing has limitations, also add tempdir tests for Windows-specific path logic:

```rust
#[test]
fn test_validate_sdk_path_finds_bat_file() {
    let tmp = TempDir::new().unwrap();
    let sdk = tmp.path().join("flutter");
    fs::create_dir_all(sdk.join("bin/cache/dart-sdk")).unwrap();
    fs::write(sdk.join("bin/flutter.bat"), "@echo off\r\n").unwrap();
    fs::write(sdk.join("VERSION"), "3.22.0").unwrap();
    // Note: This test verifies that validate_sdk_path checks for .bat
    // when bin/flutter doesn't exist (the Windows-only code path)
}

#[test]
fn test_flutter_executable_windows_batch_variant() {
    let bat_path = PathBuf::from("C:\\flutter\\bin\\flutter.bat");
    let exec = FlutterExecutable::WindowsBatch(bat_path.clone());
    assert_eq!(exec.path(), &bat_path);
}
```

### Acceptance Criteria

1. Windows cross-compilation succeeds (`x86_64-pc-windows-gnu` target)
2. Docker image builds with Wine and Flutter `.bat` layout
3. fdemon.exe doesn't panic when run under Wine
4. Cross-compiled binary is a valid PE executable
5. Flutter `.bat` file and VERSION file are present in the container
6. Complementary tempdir tests verify `.bat` detection logic at the library level
7. All tests clearly document Wine limitations in test names/comments

### Testing

```bash
# Run Windows Docker tests
cargo test --test sdk_detection tier2_windows -- --ignored --nocapture
```

### Notes

- **Cross-compilation requirements**: The `x86_64-pc-windows-gnu` target uses MinGW. Some crates may not compile cleanly — if compilation fails, investigate linker errors (common: missing Windows system libraries, OpenSSL issues).
- **Wine is a smoke test only**: The primary purpose is to verify that (a) fdemon cross-compiles to Windows, and (b) the binary doesn't crash on basic startup. Full Windows testing requires a real Windows environment.
- **Consider `cross` crate**: If MinGW cross-compilation is problematic, the `cross` tool (https://github.com/cross-rs/cross) provides pre-built Docker images for cross-compilation that may be more reliable.
- **Build time**: Cross-compilation + Wine Docker image will be the slowest build. Consider making this a separate test that's only run when explicitly needed.
- **Wine's `cfg(target_os)`**: When Rust compiles for `x86_64-pc-windows-gnu`, `cfg(target_os = "windows")` is `true` in the binary. This means the Windows code paths (`.bat` detection, `cmd /c` wrapping) are actually compiled in, which is what we want to test.
