## Task: Add Windows-only unit tests for the locator and `FlutterExecutable`

**Objective**: Cover the Windows-specific behavior (`.bat` resolution via `which`, `dunce` UNC stripping, `WindowsBatch::command()` invoking the path directly) with unit tests gated by `#[cfg(target_os = "windows")]` so they execute only on the new Windows CI runner. Without these tests, the rest of this fix has no automated proof on Windows.

**Depends on**: 02-simplify-flutter-executable, 03-locator-which-dunce

**Estimated Time**: 2-3h

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/flutter_sdk/windows_tests.rs` (NEW): a `#[cfg(target_os = "windows")]`-gated test module covering everything below.
- `crates/fdemon-daemon/src/flutter_sdk/mod.rs`: add `#[cfg(target_os = "windows")] #[cfg(test)] mod windows_tests;` declaration.

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/flutter_sdk/types.rs` — for `FlutterExecutable`, `validate_sdk_path*`.
- `crates/fdemon-daemon/src/flutter_sdk/locator.rs` — for `find_flutter_sdk`, `resolve_sdk_root_from_binary`, `try_system_path`.

### Details

#### Module declaration in `mod.rs`

Add (placement: with the other `mod` declarations near the top of `mod.rs`):

```rust
#[cfg(all(test, target_os = "windows"))]
mod windows_tests;
```

#### New file `windows_tests.rs`

Structure: a stand-alone test module with helpers that build a fake Flutter SDK tree (a `flutter.bat` shim that just `@echo`s a known string) under a `tempfile::TempDir`, then verifies the locator and executable behave correctly.

```rust
//! Windows-only integration tests for the Flutter SDK locator and executable.
//!
//! These tests run only when `cargo test` is invoked on a `windows` target.
//! On Unix CI / local runs they are compiled out entirely.

#![cfg(all(test, target_os = "windows"))]

use std::fs;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

use super::types::{validate_sdk_path, FlutterExecutable};
use super::locator::{find_flutter_sdk, resolve_sdk_root_from_binary};

/// Builds a fake Flutter SDK tree under `root` with a working `flutter.bat`
/// shim. The shim prints "FAKE_FLUTTER" and exits 0 — enough to verify the
/// invocation path without needing a real Flutter install.
fn create_fake_sdk(root: &Path, version: &str) {
    fs::create_dir_all(root.join(r"bin\cache\dart-sdk")).unwrap();
    fs::write(
        root.join(r"bin\flutter.bat"),
        "@echo off\r\necho FAKE_FLUTTER\r\nexit /b 0\r\n",
    ).unwrap();
    fs::write(root.join("VERSION"), version).unwrap();
}

/// Helper: prepend `dir` to PATH for the duration of `f`. Restores PATH on drop.
struct PathPrepender { original: std::ffi::OsString }
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
    use tokio::runtime::Runtime;
    let tmp = TempDir::new().unwrap();
    create_fake_sdk(tmp.path(), "3.99.0");
    let exe = validate_sdk_path(tmp.path()).unwrap();

    // Verify Rust's stdlib correctly invokes the .bat via cmd internally.
    let rt = Runtime::new().unwrap();
    let output = rt.block_on(async { exe.command().output().await.unwrap() });
    assert!(output.status.success(), "exit code: {:?}", output.status.code());
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
    let output = rt.block_on(async {
        exe.command().arg("devices").output().await.unwrap()
    });
    assert!(
        output.status.success(),
        "spawn failed for path with spaces — this is the bug from issues #32/#34. \
         exit={:?} stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
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

#[test]
fn find_flutter_sdk_resolves_via_path() {
    let tmp = TempDir::new().unwrap();
    create_fake_sdk(tmp.path(), "3.99.0");
    let bin_dir = tmp.path().join("bin");

    let _path_guard = PathPrepender::new(&bin_dir);
    // No explicit config, no FLUTTER_ROOT, no version manager — should fall
    // through to strategy 10 (system PATH via which).
    std::env::remove_var("FLUTTER_ROOT");
    let project = TempDir::new().unwrap();
    let sdk = find_flutter_sdk(project.path(), None)
        .expect("locator should find the fake SDK on PATH");
    assert!(sdk.executable.path().ends_with("flutter.bat"));
    assert!(sdk.executable.path().is_absolute());
    // The executable path must NOT have a UNC prefix.
    let p = sdk.executable.path().to_string_lossy();
    assert!(!p.starts_with(r"\\?\"), "UNC prefix leaked into executable path: {p}");
}
```

#### Test isolation considerations

- The `PathPrepender` mutates `std::env::set_var("PATH", ...)`, which is **process-global**. Use `serial_test::serial` (already a workspace dev-dependency per existing locator tests) to serialize tests that touch PATH/env vars.
- Equivalently, gate them with `#[serial]` from `serial_test` — copy the pattern from the existing `mod tests` in `locator.rs`.

Add at the top of the file:
```rust
use serial_test::serial;
```
And tag PATH-mutating tests with `#[serial]`.

### Acceptance Criteria

1. New file `crates/fdemon-daemon/src/flutter_sdk/windows_tests.rs` exists, gated by `#[cfg(all(test, target_os = "windows"))]`.
2. `crates/fdemon-daemon/src/flutter_sdk/mod.rs` declares the module under the same cfg gate.
3. On a Linux/macOS dev machine, `cargo test -p fdemon-daemon` does **not** compile or execute these tests (they vanish under the cfg gate).
4. On `windows-latest` CI, all eight tests above pass.
5. The `windows_batch_command_works_with_path_containing_spaces` test specifically reproduces the failure described in issues #32/#34 *with the wrapper still present*, and passes after the wrapper is removed (by task 02).
6. `cargo clippy --target x86_64-pc-windows-msvc -p fdemon-daemon -- -D warnings` (run on CI) is clean.

### Testing

Locally (Unix) — confirms the cfg gate works:
```bash
cargo check -p fdemon-daemon                 # No new warnings
cargo test -p fdemon-daemon                  # No new tests run
```

Cross-compile sanity check (no Windows runtime needed):
```bash
rustup target add x86_64-pc-windows-msvc
cargo check --target x86_64-pc-windows-msvc -p fdemon-daemon
```

(MSVC linker may not be installed locally; if `cargo check` hits a linker error, that's expected — `check` may still surface compile errors. Alternatively use `--target x86_64-pc-windows-gnu` if installed.)

On CI: covered by task 06's workflow.

### Notes

- The fake `flutter.bat` is intentionally minimal — printing `FAKE_FLUTTER` to stdout. That's enough to assert `Command::output().status.success()` without depending on a real Flutter install.
- Do **not** spawn the real Flutter SDK in CI; we don't want to install it on the runner.
- The dunce test is a spec test — it would catch a regression if we ever switched back to `fs::canonicalize`.
- If `tokio::runtime::Runtime::new()` is overkill for the spawn assertion, use `#[tokio::test]` annotations instead. Match the surrounding test style in the daemon crate.
- **Do not** add or modify tests in `types.rs` or `locator.rs` from this task — task 02 and task 03 own those files. This task only writes to `windows_tests.rs` and adds one line to `mod.rs`.
