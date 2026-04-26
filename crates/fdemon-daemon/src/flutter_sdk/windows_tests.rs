//! Windows-only integration tests for the Flutter SDK locator and executable.
//!
//! These tests run only when `cargo test` is invoked on a `windows` target.
//! On Unix CI / local runs they are compiled out entirely.

#![cfg(all(test, target_os = "windows"))]

use std::fs;
use std::path::{Path, PathBuf};

use serial_test::serial;
use tempfile::TempDir;

use super::locator::{find_flutter_sdk, resolve_sdk_root_from_binary};
use super::types::{validate_sdk_path, FlutterExecutable, SdkSource};

/// Builds a fake Flutter SDK tree under `root` with a working `flutter.bat`
/// shim. The shim prints "FAKE_FLUTTER" followed by any arguments passed to it
/// (`%*`), then exits 0 — enough to verify the invocation path and argument
/// forwarding without needing a real Flutter install.
fn create_fake_sdk(root: &Path, version: &str) {
    fs::create_dir_all(root.join("bin").join("cache").join("dart-sdk"))
        .expect("create fake bin/cache/dart-sdk dir");
    fs::write(
        root.join("bin").join("flutter.bat"),
        "@echo off\r\necho FAKE_FLUTTER %*\r\n",
    )
    .expect("write fake flutter.bat");
    fs::write(root.join("VERSION"), version).expect("write fake VERSION file");
}

/// RAII guard that prepends `dir` to the PATH environment variable for the
/// lifetime of the guard, then restores the original PATH on drop.
///
/// Mirrors the `PathPrependGuard` in `locator.rs`'s test module; duplicated
/// here because `#[cfg(test)]` helpers are not accessible across modules.
struct PathPrependGuard {
    original: std::ffi::OsString,
}

impl PathPrependGuard {
    fn new(dir: &Path) -> Self {
        let original = std::env::var_os("PATH").unwrap_or_default();
        let mut new_path = std::ffi::OsString::from(dir);
        new_path.push(";");
        new_path.push(&original);
        std::env::set_var("PATH", new_path);
        Self { original }
    }
}

impl Drop for PathPrependGuard {
    fn drop(&mut self) {
        std::env::set_var("PATH", &self.original);
    }
}

/// Prepend `dir` to PATH for the lifetime of the returned guard.
fn path_prepend_guard(dir: &Path) -> PathPrependGuard {
    PathPrependGuard::new(dir)
}

// ─────────────────────────────────────────────────────────────────────────────
// FlutterExecutable / validate_sdk_path tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_validate_sdk_path_windows_returns_windows_batch_variant() {
    let tmp = TempDir::new().expect("create tempdir");
    create_fake_sdk(tmp.path(), "3.99.0");

    let exe = validate_sdk_path(tmp.path()).expect("validate_sdk_path should succeed");
    assert!(matches!(exe, FlutterExecutable::WindowsBatch(_)));
    assert!(exe.path().ends_with("flutter.bat"));
    assert!(exe.path().is_absolute());
}

#[test]
fn test_command_windows_batch_invokes_bat_directly_not_cmd() {
    let tmp = TempDir::new().expect("create tempdir");
    create_fake_sdk(tmp.path(), "3.99.0");
    let exe = validate_sdk_path(tmp.path()).expect("validate_sdk_path should succeed");

    let cmd = exe.command();
    // After the fix, command().get_program() must be the .bat path itself,
    // NOT "cmd". This is what proves we removed the cmd /c wrapper.
    assert_eq!(cmd.as_std().get_program(), exe.path().as_os_str());
}

#[test]
fn test_command_windows_batch_executes_bat_returns_zero() {
    let tmp = TempDir::new().expect("create tempdir");
    create_fake_sdk(tmp.path(), "3.99.0");
    let exe = validate_sdk_path(tmp.path()).expect("validate_sdk_path should succeed");

    // Verify Rust's stdlib correctly invokes the .bat via cmd internally.
    let rt = tokio::runtime::Runtime::new().expect("create tokio runtime");
    let output = rt.block_on(async {
        exe.command()
            .output()
            .await
            .expect("execute fake flutter.bat")
    });
    assert!(
        output.status.success(),
        "exit code: {:?}",
        output.status.code()
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("FAKE_FLUTTER"), "stdout was: {stdout}");
}

#[test]
fn test_command_windows_batch_path_with_spaces_passes_args_intact() {
    // This is the regression test for issues #32 / #34.
    let tmp = TempDir::new().expect("create tempdir");
    let root_with_spaces = tmp.path().join("Some Folder With Spaces");
    fs::create_dir_all(&root_with_spaces).expect("create dir with spaces in name");
    create_fake_sdk(&root_with_spaces, "3.99.0");
    let exe = validate_sdk_path(&root_with_spaces).expect("validate_sdk_path should succeed");

    let rt = tokio::runtime::Runtime::new().expect("create tokio runtime");
    let output = rt.block_on(async {
        exe.command()
            .arg("devices")
            .output()
            .await
            .expect("spawn flutter.bat with spaces in path")
    });
    assert!(
        output.status.success(),
        "exit code: {:?}",
        output.status.code()
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("FAKE_FLUTTER") && stdout.contains("devices"),
        "expected stdout to contain `FAKE_FLUTTER devices`, got: {stdout}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// which / dunce tests
// ─────────────────────────────────────────────────────────────────────────────

// PATH mutation — must run serially (set_var is process-wide and not thread-safe).
#[test]
#[serial]
fn test_which_finds_flutter_bat_via_pathext() {
    let tmp = TempDir::new().expect("create tempdir");
    create_fake_sdk(tmp.path(), "3.99.0");
    let bin_dir = tmp.path().join("bin");

    let _path_guard = path_prepend_guard(&bin_dir);
    let resolved = which::which("flutter").expect("which should resolve via PATHEXT");
    assert!(resolved.ends_with("flutter.bat"));
    assert!(resolved.is_absolute());
}

#[test]
fn test_dunce_canonicalize_strips_unc_prefix_on_windows() {
    let tmp = TempDir::new().expect("create tempdir");
    create_fake_sdk(tmp.path(), "3.99.0");
    let bat = tmp.path().join("bin").join("flutter.bat");

    let canonical = dunce::canonicalize(&bat).expect("dunce::canonicalize should succeed");
    let s = canonical.to_string_lossy();
    assert!(
        !s.starts_with(r"\\?\"),
        "dunce should strip the \\\\?\\ prefix; got: {s}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// resolve_sdk_root_from_binary tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_resolve_sdk_root_from_binary_walks_up_two_parents() {
    let tmp = TempDir::new().expect("create tempdir");
    create_fake_sdk(tmp.path(), "3.99.0");

    let bat = tmp.path().join("bin").join("flutter.bat");
    let resolved = resolve_sdk_root_from_binary(&bat);
    let expected = dunce::canonicalize(tmp.path()).ok();
    assert_eq!(resolved, expected);
}

// ─────────────────────────────────────────────────────────────────────────────
// find_flutter_sdk end-to-end test
// ─────────────────────────────────────────────────────────────────────────────

// PATH mutation — must run serially (set_var is process-wide and not thread-safe).
#[test]
#[serial]
fn test_find_flutter_sdk_via_path_returns_system_source() {
    let tmp = TempDir::new().expect("create tempdir");
    create_fake_sdk(tmp.path(), "3.99.0");
    let bin_dir = tmp.path().join("bin");

    let _path_guard = path_prepend_guard(&bin_dir);
    // No explicit config, no FLUTTER_ROOT, no version manager — should fall
    // through to strategy 10 (system PATH via which).
    std::env::remove_var("FLUTTER_ROOT");
    let project = TempDir::new().expect("create project tempdir");
    let sdk =
        find_flutter_sdk(project.path(), None).expect("locator should find the fake SDK on PATH");
    assert!(sdk.executable.path().ends_with("flutter.bat"));
    assert!(sdk.executable.path().is_absolute());
    // The executable path must NOT have a UNC prefix.
    let p = sdk.executable.path().to_string_lossy();
    assert!(
        !p.starts_with(r"\\?\"),
        "UNC prefix leaked into executable path: {p}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Strategy 12: shim-installer layout tests (scoop, winget)
// ─────────────────────────────────────────────────────────────────────────────

/// Helper: build a scoop-style shim layout under a tempdir.
/// scoop installs flutter shims at <root>/scoop/shims/flutter.bat — there is
/// no <root>/scoop/bin/, so strategies 10 and 11 must reject the inferred root
/// and Strategy 12 must fire.
fn create_scoop_shim_layout(root: &Path) -> PathBuf {
    let shims = root.join("scoop").join("shims");
    fs::create_dir_all(&shims).expect("create scoop shims dir");
    let bat = shims.join("flutter.bat");
    fs::write(&bat, "@echo off\r\necho FAKE_FLUTTER %*\r\n").expect("write scoop flutter.bat");
    bat
}

/// Helper: build a winget-style shim layout under a tempdir.
/// winget shims live at <root>/Links/flutter.bat with no surrounding SDK tree.
fn create_winget_shim_layout(root: &Path) -> PathBuf {
    let links = root.join("Links");
    fs::create_dir_all(&links).expect("create winget Links dir");
    let bat = links.join("flutter.bat");
    fs::write(&bat, "@echo off\r\necho FAKE_FLUTTER %*\r\n").expect("write winget flutter.bat");
    bat
}

// PATH mutation — must run serially (set_var is process-wide and not thread-safe).
#[test]
#[serial]
fn test_find_flutter_sdk_scoop_shim_resolves_via_strategy_12() {
    let temp = TempDir::new().expect("create tempdir");
    let bat = create_scoop_shim_layout(temp.path());
    let bin_dir = bat.parent().expect("bat parent (scoop/shims)");
    let _path_guard = path_prepend_guard(bin_dir);
    std::env::remove_var("FLUTTER_ROOT");
    let project = TempDir::new().expect("create project tempdir");

    let sdk =
        find_flutter_sdk(project.path(), None).expect("Strategy 12 should resolve scoop shim");
    assert_eq!(sdk.source, SdkSource::PathInferred);
    assert_eq!(sdk.version, "unknown");
    assert!(sdk.executable.path().ends_with("flutter.bat"));
}

// PATH mutation — must run serially (set_var is process-wide and not thread-safe).
#[test]
#[serial]
fn test_find_flutter_sdk_winget_shim_resolves_via_strategy_12() {
    let temp = TempDir::new().expect("create tempdir");
    let bat = create_winget_shim_layout(temp.path());
    let bin_dir = bat.parent().expect("bat parent (Links)");
    let _path_guard = path_prepend_guard(bin_dir);
    std::env::remove_var("FLUTTER_ROOT");
    let project = TempDir::new().expect("create project tempdir");

    let sdk =
        find_flutter_sdk(project.path(), None).expect("Strategy 12 should resolve winget shim");
    assert_eq!(sdk.source, SdkSource::PathInferred);
    assert_eq!(sdk.version, "unknown");
    assert!(sdk.executable.path().ends_with("flutter.bat"));
}
