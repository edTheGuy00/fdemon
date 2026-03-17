//! # Tier 2 — Docker Windows Tests via Wine
//!
//! Verifies that fdemon's Windows-specific SDK detection logic compiles and
//! executes correctly, using a Wine-based Docker container running a
//! cross-compiled `fdemon.exe`.
//!
//! ## Approach
//!
//! macOS Docker Desktop can only run Linux containers, so Windows execution is
//! tested via **Wine in a Linux container**.  The fdemon binary is
//! cross-compiled for `x86_64-pc-windows-gnu` (MinGW), which means
//! `cfg(target_os = "windows")` is `true` in the resulting `.exe`.  This
//! exercises the `.bat`-detection code paths that are compiled out on non-Windows
//! hosts.
//!
//! ## Test categories
//!
//! 1. **Docker/Wine tests** — require Docker with Wine; all carry `#[ignore]`.
//!    - Cross-compilation verification: `file fdemon.exe` should report PE32+.
//!    - Wine execution smoke test: binary must not panic on startup.
//!    - Flutter layout verification: `.bat` file and VERSION file must exist.
//!
//! 2. **Tempdir tests** — no Docker required; always run.
//!    - Verify `.bat` file detection logic at the library level.
//!    - Verify `FlutterExecutable::WindowsBatch` type behaviour.
//!
//! ## Wine limitations
//!
//! | Aspect                               | Testable via Wine? |
//! |--------------------------------------|--------------------|
//! | Cross-compilation succeeds           | Yes                |
//! | Binary doesn't panic on startup      | Yes                |
//! | `.bat` file detection (code level)   | Partially          |
//! | `cmd /c flutter.bat` execution       | No — Wine `cmd.exe` is unreliable |
//! | Windows PATH resolution              | Partially          |
//! | `%APPDATA%` / registry               | Partially          |
//!
//! ## Running
//!
//! ```bash
//! # Run only the Wine-based Docker tests (requires Docker + MinGW in PATH)
//! cargo test --test sdk_detection tier2_windows -- --ignored --nocapture
//!
//! # Run only the non-Docker tempdir tests
//! cargo test --test sdk_detection tier2_windows
//! ```

use std::fs;
use std::path::PathBuf;

use tempfile::TempDir;

use super::docker_helpers::{docker_available, docker_build, docker_exec, project_root};
use fdemon_daemon::flutter_sdk::{validate_sdk_path, FlutterExecutable};

// ─────────────────────────────────────────────────────────────────────────────
// Constants
// ─────────────────────────────────────────────────────────────────────────────

/// Docker image tag for the Wine-based Windows test image.
const WINDOWS_IMAGE_TAG: &str = "fdemon-test-windows";

/// Dockerfile path relative to the project root.
const WINDOWS_DOCKERFILE: &str = "tests/docker/windows-wine.Dockerfile";

// ─────────────────────────────────────────────────────────────────────────────
// Docker/Wine tests (require `--ignored` to run)
// ─────────────────────────────────────────────────────────────────────────────

/// Build the Wine Docker image.
///
/// Factored out of each test so the image is built once and reused.
/// Must be called inside each Docker test that needs the image because
/// tests run in arbitrary order and in parallel.
fn ensure_wine_image_built() -> Result<(), String> {
    let root = project_root();
    docker_build(WINDOWS_DOCKERFILE, WINDOWS_IMAGE_TAG, &root).map(|_| ())
}

/// Verify that the cross-compilation produced a valid 64-bit Windows PE
/// executable.
///
/// Uses `file(1)` inside the container to inspect the binary's ELF/PE magic.
/// A successful cross-compilation produces a file whose `file` output contains
/// "PE32+" or "Windows".
#[test]
#[ignore = "requires Docker + Wine (cross-compiled Windows binary)"]
fn test_windows_cross_compilation_produces_valid_exe() {
    if !docker_available() {
        eprintln!("Docker not available, skipping");
        return;
    }

    ensure_wine_image_built().expect("Wine Docker image should build successfully");

    let result = docker_exec(WINDOWS_IMAGE_TAG, &["file", "/app/fdemon.exe"])
        .expect("docker exec should not fail to spawn");

    eprintln!("file output: {}", result.stdout);

    assert!(
        result.stdout.contains("PE32+") || result.stdout.contains("Windows"),
        "fdemon.exe is not a valid Windows PE executable.\n\
         Expected 'PE32+' or 'Windows' in `file` output.\n\
         Got: {}",
        result.stdout,
    );
}

/// Smoke test: run `fdemon.exe` under Wine with `--headless` and verify that
/// the binary starts without triggering a Rust panic.
///
/// Wine limitations mean the binary may exit non-zero for many reasons
/// (missing DLLs, filesystem quirks, Wine incompatibilities).  The only hard
/// assertion here is the absence of a Rust panic message in stderr, which
/// would indicate a logic bug in the Windows code paths compiled into the .exe.
///
/// Wine maps the Linux root to `Z:\`, so `/test-project` → `Z:\test-project`.
#[test]
#[ignore = "requires Docker + Wine (cross-compiled Windows binary)"]
fn test_windows_bat_detection_via_wine() {
    if !docker_available() {
        eprintln!("Docker not available, skipping");
        return;
    }

    ensure_wine_image_built().expect("Wine Docker image should build successfully");

    // Run fdemon.exe inside Wine in headless mode.
    // The project path uses Wine's Z: drive mapping for the Linux filesystem.
    let result = docker_exec(
        WINDOWS_IMAGE_TAG,
        &[
            "wine64",
            "/app/fdemon.exe",
            "--headless",
            "Z:\\test-project",
        ],
    )
    .expect("docker exec should not fail to spawn");

    eprintln!("Wine stdout: {}", result.stdout);
    eprintln!("Wine stderr: {}", result.stderr);

    // Primary assertion: the Rust runtime must not have panicked.
    // Wine exit codes are unreliable, but a Rust panic always emits this
    // message to stderr regardless of platform.
    assert!(
        !result.stderr.contains("thread 'main' panicked"),
        "fdemon.exe panicked under Wine.\n\
         This indicates a bug in a Windows code path compiled into the binary.\n\
         stderr:\n{}",
        result.stderr,
    );
}

/// Verify that the Wine container's Flutter SDK layout contains `flutter.bat`.
///
/// The Dockerfile creates `/flutter/bin/flutter.bat` to simulate a Windows
/// Flutter installation.  This test confirms the file is present so that
/// subsequent Wine-execution tests can rely on it.
#[test]
#[ignore = "requires Docker + Wine"]
fn test_windows_flutter_bat_exists_in_layout() {
    if !docker_available() {
        eprintln!("Docker not available, skipping");
        return;
    }

    ensure_wine_image_built().expect("Wine Docker image should build successfully");

    let result = docker_exec(WINDOWS_IMAGE_TAG, &["ls", "-la", "/flutter/bin/"])
        .expect("docker exec should not fail to spawn");

    assert!(
        result.stdout.contains("flutter.bat"),
        "flutter.bat not found in /flutter/bin/.\n\
         This means the Dockerfile's Flutter SDK layout is incorrect.\n\
         ls output:\n{}",
        result.stdout,
    );
}

/// Verify that the Wine container's Flutter VERSION file contains the expected
/// version string.
#[test]
#[ignore = "requires Docker + Wine"]
fn test_windows_sdk_version_readable() {
    if !docker_available() {
        eprintln!("Docker not available, skipping");
        return;
    }

    ensure_wine_image_built().expect("Wine Docker image should build successfully");

    let result = docker_exec(WINDOWS_IMAGE_TAG, &["cat", "/flutter/VERSION"])
        .expect("docker exec should not fail to spawn");

    assert_eq!(
        result.stdout.trim(),
        "3.22.0",
        "VERSION file content is unexpected.\n\
         Expected '3.22.0', got '{}'",
        result.stdout.trim(),
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Tempdir tests (no Docker required — always run)
// ─────────────────────────────────────────────────────────────────────────────

/// Verify that `validate_sdk_path` reports a missing binary when `bin/flutter`
/// is absent and `bin/flutter.bat` is the only executable present.
///
/// On Unix, `validate_sdk_path` checks for `bin/flutter` (the shell script).
/// If only `bin/flutter.bat` exists, validation fails — this is the expected
/// behaviour because the library is compiled for Unix and should prefer the
/// Unix executable.  The `.bat` file is only checked on Windows builds
/// (`cfg(target_os = "windows")`).
///
/// This test documents that behaviour explicitly so the distinction is clear.
#[test]
fn test_validate_sdk_path_bat_only_fails_on_unix() {
    let tmp = TempDir::new().unwrap();
    let sdk = tmp.path().join("flutter");

    // Create a layout with ONLY a .bat file — no `bin/flutter` shell script.
    fs::create_dir_all(sdk.join("bin/cache/dart-sdk")).unwrap();
    fs::write(sdk.join("bin/flutter.bat"), "@echo off\r\n").unwrap();
    fs::write(sdk.join("VERSION"), "3.22.0").unwrap();

    // On Unix the function checks for `bin/flutter`, which is absent.
    // Expected: validation fails.
    #[cfg(not(target_os = "windows"))]
    {
        let result = validate_sdk_path(&sdk);
        assert!(
            result.is_err(),
            "validate_sdk_path should fail on Unix when only flutter.bat is present; \
             got Ok({:?})",
            result.unwrap().path(),
        );
    }

    // On Windows the function checks for `bin/flutter.bat`, which IS present.
    // Expected: validation succeeds.
    #[cfg(target_os = "windows")]
    {
        let result = validate_sdk_path(&sdk);
        assert!(
            result.is_ok(),
            "validate_sdk_path should succeed on Windows when flutter.bat is present; \
             got Err({:?})",
            result.unwrap_err(),
        );
        let exe = result.unwrap();
        assert!(
            matches!(exe, FlutterExecutable::WindowsBatch(_)),
            "Expected WindowsBatch variant on Windows, got {:?}",
            exe,
        );
    }
}

/// Verify that a tempdir-based SDK with both `bin/flutter` and `bin/flutter.bat`
/// passes `validate_sdk_path` on Unix and returns a `Direct` executable.
///
/// Some Windows Flutter installations include both the shell script and the
/// batch file.  On Unix fdemon should always use the shell script.
#[test]
fn test_validate_sdk_path_finds_shell_script_when_both_exist() {
    let tmp = TempDir::new().unwrap();
    let sdk = tmp.path().join("flutter");

    fs::create_dir_all(sdk.join("bin")).unwrap();
    // Both executables present (as in some cross-platform installs)
    fs::write(sdk.join("bin/flutter"), "#!/bin/sh\necho Flutter 3.22.0\n").unwrap();
    fs::write(sdk.join("bin/flutter.bat"), "@echo off\r\n").unwrap();
    fs::write(sdk.join("VERSION"), "3.22.0").unwrap();

    // On Unix validate_sdk_path should prefer bin/flutter and return Direct.
    #[cfg(not(target_os = "windows"))]
    {
        let result = validate_sdk_path(&sdk);
        assert!(
            result.is_ok(),
            "validate_sdk_path should succeed when bin/flutter is present; got Err({:?})",
            result.unwrap_err(),
        );
        let exe = result.unwrap();
        assert!(
            matches!(exe, FlutterExecutable::Direct(_)),
            "Expected Direct variant on Unix, got {:?}",
            exe,
        );
        assert_eq!(
            exe.path(),
            sdk.join("bin/flutter"),
            "Direct executable path should point to bin/flutter",
        );
    }
}

/// Type-level test: verify that `FlutterExecutable::WindowsBatch` stores the
/// path correctly and that `path()` returns the same path.
///
/// This does not require a real filesystem — it tests the type API only.
#[test]
fn test_flutter_executable_windows_batch_variant() {
    let bat_path = PathBuf::from("C:\\flutter\\bin\\flutter.bat");
    let exec = FlutterExecutable::WindowsBatch(bat_path.clone());

    assert_eq!(
        exec.path(),
        bat_path.as_path(),
        "FlutterExecutable::WindowsBatch::path() should return the stored path",
    );
}

/// Type-level test: verify that `FlutterExecutable::Direct` stores the path
/// correctly and that `path()` returns the same path.
///
/// Included here for symmetry with the WindowsBatch test above — both variants
/// share the same `path()` implementation.
#[test]
fn test_flutter_executable_direct_variant() {
    let bin_path = PathBuf::from("/usr/local/flutter/bin/flutter");
    let exec = FlutterExecutable::Direct(bin_path.clone());

    assert_eq!(
        exec.path(),
        bin_path.as_path(),
        "FlutterExecutable::Direct::path() should return the stored path",
    );
}

/// Verify that `FlutterExecutable::WindowsBatch` is not equal to
/// `FlutterExecutable::Direct` even when pointing to the same path string.
///
/// The two variants represent fundamentally different invocation strategies
/// (direct vs. `cmd /c` wrapper) and must not be considered equal.
#[test]
fn test_flutter_executable_variants_are_not_equal() {
    let path = PathBuf::from("/flutter/bin/flutter");
    let direct = FlutterExecutable::Direct(path.clone());
    let batch = FlutterExecutable::WindowsBatch(path.clone());

    assert_ne!(
        direct, batch,
        "Direct and WindowsBatch variants must not be equal even with the same path",
    );
}

/// Verify that creating a bat-file-only SDK layout at a tempdir path behaves
/// correctly with `validate_sdk_path` when a Unix shell script IS present.
///
/// This is the "happy path" for a complete SDK that includes both executables.
#[test]
fn test_validate_sdk_path_complete_sdk_passes() {
    let tmp = TempDir::new().unwrap();
    let sdk = tmp.path().join("flutter");

    fs::create_dir_all(sdk.join("bin/cache/dart-sdk")).unwrap();
    fs::write(sdk.join("bin/flutter"), "#!/bin/sh\n").unwrap();
    fs::write(sdk.join("bin/flutter.bat"), "@echo off\r\n").unwrap();
    fs::write(sdk.join("VERSION"), "3.22.0").unwrap();
    fs::create_dir_all(sdk.join(".git")).unwrap();
    fs::write(sdk.join(".git/HEAD"), "ref: refs/heads/stable\n").unwrap();

    let result = validate_sdk_path(&sdk);
    assert!(
        result.is_ok(),
        "validate_sdk_path should succeed for a complete SDK layout; got Err({:?})",
        result.unwrap_err(),
    );
}
