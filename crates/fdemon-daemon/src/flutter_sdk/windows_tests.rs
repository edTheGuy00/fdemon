//! Windows-only integration tests for the Flutter SDK locator and executable.
//!
//! These tests run only when `cargo test` is invoked on a `windows` target.
//! On Unix CI / local runs they are compiled out entirely.

#![cfg(all(test, target_os = "windows"))]

use std::fs;
use std::path::Path;

use serial_test::serial;
use tempfile::TempDir;

use super::locator::{find_flutter_sdk, resolve_sdk_root_from_binary};
use super::types::{validate_sdk_path, FlutterExecutable};

/// Builds a fake Flutter SDK tree under `root` with a working `flutter.bat`
/// shim. The shim prints "FAKE_FLUTTER" and exits 0 — enough to verify the
/// invocation path without needing a real Flutter install.
fn create_fake_sdk(root: &Path, version: &str) {
    fs::create_dir_all(root.join(r"bin\cache\dart-sdk")).unwrap();
    fs::write(
        root.join(r"bin\flutter.bat"),
        "@echo off\r\necho FAKE_FLUTTER\r\nexit /b 0\r\n",
    )
    .unwrap();
    fs::write(root.join("VERSION"), version).unwrap();
}

/// Prepends `dir` to PATH for the duration of the guard's lifetime.
/// Restores the original PATH on drop.
struct PathPrepender {
    original: std::ffi::OsString,
}

impl PathPrepender {
    fn new(dir: &Path) -> Self {
        let original = std::env::var_os("PATH").unwrap_or_default();
        let mut new_path = std::ffi::OsString::from(dir);
        new_path.push(";");
        new_path.push(&original);
        std::env::set_var("PATH", new_path);
        Self { original }
    }
}

impl Drop for PathPrepender {
    fn drop(&mut self) {
        std::env::set_var("PATH", &self.original);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// FlutterExecutable / validate_sdk_path tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn validate_sdk_path_returns_windows_batch_variant() {
    let tmp = TempDir::new().unwrap();
    create_fake_sdk(tmp.path(), "3.99.0");

    let exe = validate_sdk_path(tmp.path()).unwrap();
    assert!(matches!(exe, FlutterExecutable::WindowsBatch(_)));
    assert!(exe.path().ends_with("flutter.bat"));
    assert!(exe.path().is_absolute());
}

#[test]
fn windows_batch_command_invokes_path_directly() {
    let tmp = TempDir::new().unwrap();
    create_fake_sdk(tmp.path(), "3.99.0");
    let exe = validate_sdk_path(tmp.path()).unwrap();

    let cmd = exe.command();
    // After the fix, command().get_program() must be the .bat path itself,
    // NOT "cmd". This is what proves we removed the cmd /c wrapper.
    assert_eq!(cmd.as_std().get_program(), exe.path().as_os_str());
}

#[test]
fn windows_batch_command_executes_successfully() {
    let tmp = TempDir::new().unwrap();
    create_fake_sdk(tmp.path(), "3.99.0");
    let exe = validate_sdk_path(tmp.path()).unwrap();

    // Verify Rust's stdlib correctly invokes the .bat via cmd internally.
    let rt = tokio::runtime::Runtime::new().unwrap();
    let output = rt.block_on(async { exe.command().output().await.unwrap() });
    assert!(
        output.status.success(),
        "exit code: {:?}",
        output.status.code()
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("FAKE_FLUTTER"), "stdout was: {stdout}");
}

#[test]
fn windows_batch_command_works_with_path_containing_spaces() {
    // This is the regression test for issues #32 / #34.
    let tmp = TempDir::new().unwrap();
    let root_with_spaces = tmp.path().join("Some Folder With Spaces");
    fs::create_dir_all(&root_with_spaces).unwrap();
    create_fake_sdk(&root_with_spaces, "3.99.0");
    let exe = validate_sdk_path(&root_with_spaces).unwrap();

    let rt = tokio::runtime::Runtime::new().unwrap();
    let output = rt.block_on(async { exe.command().arg("devices").output().await.unwrap() });
    assert!(
        output.status.success(),
        "spawn failed for path with spaces — this is the bug from issues #32/#34. \
         exit={:?} stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// which / dunce tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
#[serial]
fn which_resolves_flutter_bat_via_pathext() {
    let tmp = TempDir::new().unwrap();
    create_fake_sdk(tmp.path(), "3.99.0");
    let bin_dir = tmp.path().join("bin");

    let _path_guard = PathPrepender::new(&bin_dir);
    let resolved = which::which("flutter").expect("which should resolve via PATHEXT");
    assert!(resolved.ends_with("flutter.bat"));
    assert!(resolved.is_absolute());
}

#[test]
fn dunce_canonicalize_strips_unc_prefix() {
    let tmp = TempDir::new().unwrap();
    create_fake_sdk(tmp.path(), "3.99.0");
    let bat = tmp.path().join(r"bin\flutter.bat");

    let canonical = dunce::canonicalize(&bat).unwrap();
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
fn resolve_sdk_root_from_binary_returns_sdk_root() {
    let tmp = TempDir::new().unwrap();
    create_fake_sdk(tmp.path(), "3.99.0");

    let bat = tmp.path().join(r"bin\flutter.bat");
    let resolved = resolve_sdk_root_from_binary(&bat);
    let expected = dunce::canonicalize(tmp.path()).ok();
    assert_eq!(resolved, expected);
}

// ─────────────────────────────────────────────────────────────────────────────
// find_flutter_sdk end-to-end test
// ─────────────────────────────────────────────────────────────────────────────

#[test]
#[serial]
fn find_flutter_sdk_resolves_via_path() {
    let tmp = TempDir::new().unwrap();
    create_fake_sdk(tmp.path(), "3.99.0");
    let bin_dir = tmp.path().join("bin");

    let _path_guard = PathPrepender::new(&bin_dir);
    // No explicit config, no FLUTTER_ROOT, no version manager — should fall
    // through to strategy 10 (system PATH via which).
    std::env::remove_var("FLUTTER_ROOT");
    let project = TempDir::new().unwrap();
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
