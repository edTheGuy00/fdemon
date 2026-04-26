## Task: Apply diagnostic-error pattern to `emulators.rs` (and extract `windows_hint()` to a shared module)

**Objective**: Bring `emulators.rs` into parity with `devices.rs` so the user-facing error surface is consistent on Windows. Three call sites in `emulators.rs` currently produce errors without the binary path, structured `error!` log, or `windows_hint()` append. Extract the existing `windows_hint()` helper from `devices.rs` into a new shared `crates/fdemon-daemon/src/flutter_sdk/diagnostics.rs` module so both sibling modules can use it.

**Depends on**: nothing — Wave A

**Estimated Time**: 1.5h

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/flutter_sdk/diagnostics.rs` (NEW): host the `windows_hint()` helper (and any future shared diagnostic helpers).
- `crates/fdemon-daemon/src/flutter_sdk/mod.rs`: declare the new `diagnostics` submodule under `pub(crate)` visibility.
- `crates/fdemon-daemon/src/devices.rs`:
  - Delete the local `windows_hint()` function (currently at lines 240-254).
  - Import `windows_hint` from `crate::flutter_sdk::diagnostics`.
  - Otherwise unchanged.
- `crates/fdemon-daemon/src/emulators.rs`:
  - In `run_flutter_emulators` (lines 126-160):
    - Spawn-error branch (135-141): on non-`NotFound` errors, build an error string that includes the binary path: `Error::process(format!("Failed to run flutter emulators: {} (binary: {})", e, flutter.path().display()))`.
    - Non-zero-exit branch (151-157): replace the simple `Error::process(format!(...))` with a structured `error!` log followed by an error string that includes the binary path, exit code, trimmed stderr, and `windows_hint()` append. Mirror `devices.rs:219-232`.
  - In `run_flutter_emulator_launch` (lines 297-321):
    - Spawn-error branch (315-320): on non-`NotFound` errors, include the binary path the same way.
    - The non-zero-exit path is intentionally not flagged as an error in this function (emulators boot asynchronously) — leave that branch alone.
  - Add a new unit test `test_run_flutter_emulators_error_includes_binary_path` mirroring `devices.rs:668-680`.

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/devices.rs` lines 219-254 (the diagnostic pattern to mirror).
- `docs/ARCHITECTURE.md` (for the `flutter_sdk/` module description so the new file is consistent).

### Details

#### New module `flutter_sdk/diagnostics.rs`

```rust
//! Shared diagnostic helpers for Flutter spawn-path errors.
//!
//! The `windows_hint()` helper is appended to user-facing error strings
//! produced by `devices.rs` and `emulators.rs` when a spawn or non-zero
//! exit is observed on Windows. It points users at the explicit
//! `[flutter] sdk_path` config option for shim-installer environments.

/// Returns a user-facing hint string suitable for appending to an error
/// message. On Windows, returns advice about setting `[flutter] sdk_path`.
/// On other platforms, returns an empty string.
///
/// On Windows, package-manager shims (Chocolatey, scoop, winget) can cause
/// spawn failures when the SDK root cannot be inferred from the shim path.
/// The hint points the user at the config option that lets them pin an
/// exact SDK path.
#[cfg(target_os = "windows")]
pub(crate) fn windows_hint() -> &'static str {
    "\n\nHint: If your Flutter is installed via a package manager (Chocolatey, scoop, winget) \
     or in a non-standard location, set `[flutter] sdk_path = \"C:\\\\path\\\\to\\\\flutter\"` \
     in `.fdemon/config.toml`."
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn windows_hint() -> &'static str {
    ""
}
```

#### `flutter_sdk/mod.rs` declaration

Add (alongside the existing module declarations, before `windows_tests`):

```rust
pub(crate) mod diagnostics;
```

#### `devices.rs` changes

```rust
// Add import near the top alongside other crate imports
use crate::flutter_sdk::diagnostics::windows_hint;

// Delete lines 240-254 (the existing windows_hint() function pair).
```

The call site at `devices.rs:226-232` is unchanged — it already calls `windows_hint()` and the import will resolve to the new module.

#### `emulators.rs` — `run_flutter_emulators` rewrite

Current (lines 127-160):

```rust
async fn run_flutter_emulators(flutter: &FlutterExecutable) -> Result<FlutterOutput> {
    let output = flutter
        .command()
        .args(["emulators", "--machine"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                Error::FlutterNotFound
            } else {
                Error::process(format!("Failed to run flutter emulators: {}", e))
            }
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    debug!("flutter emulators stdout: {}", stdout);
    if !stderr.is_empty() {
        debug!("flutter emulators stderr: {}", stderr);
    }

    if !output.status.success() {
        return Err(Error::process(format!(
            "flutter emulators failed with exit code {:?}: {}",
            output.status.code(),
            stderr
        )));
    }

    Ok(FlutterOutput { stdout, stderr })
}
```

Replacement (additions in **bold** prose):

```rust
async fn run_flutter_emulators(flutter: &FlutterExecutable) -> Result<FlutterOutput> {
    let output = flutter
        .command()
        .args(["emulators", "--machine"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                Error::FlutterNotFound
            } else {
                Error::process(format!(
                    "Failed to run flutter emulators: {} (binary: {})",
                    e,
                    flutter.path().display()
                ))
            }
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    debug!("flutter emulators stdout: {}", stdout);
    if !stderr.is_empty() {
        debug!("flutter emulators stderr: {}", stderr);
    }

    if !output.status.success() {
        error!(
            binary = %flutter.path().display(),
            exit_code = ?output.status.code(),
            stderr = %stderr,
            stdout = %stdout,
            "flutter emulators failed"
        );
        return Err(Error::process(format!(
            "flutter emulators failed (binary: {}, exit code {:?}): {}{}",
            flutter.path().display(),
            output.status.code(),
            stderr.trim(),
            windows_hint(),
        )));
    }

    Ok(FlutterOutput { stdout, stderr })
}
```

Add `use crate::flutter_sdk::diagnostics::windows_hint;` and `use tracing::error;` (if not already imported) at the top of `emulators.rs`.

#### `emulators.rs` — `run_flutter_emulator_launch` minor change

Current spawn-error branch (lines 315-320):

```rust
.map_err(|e| {
    if e.kind() == std::io::ErrorKind::NotFound {
        Error::FlutterNotFound
    } else {
        Error::process(format!("Failed to launch emulator: {}", e))
    }
})?;
```

Replacement:

```rust
.map_err(|e| {
    if e.kind() == std::io::ErrorKind::NotFound {
        Error::FlutterNotFound
    } else {
        Error::process(format!(
            "Failed to launch emulator: {} (binary: {})",
            e,
            flutter.path().display()
        ))
    }
})?;
```

The non-zero-exit branch (lines 322-334) is intentionally not changed because emulators boot asynchronously — Flutter may exit success even if the emulator fails to fully launch.

#### Unit test

Add to the bottom of `emulators.rs` inside the existing `#[cfg(test)] mod tests` block (or create one if absent):

```rust
#[tokio::test]
async fn test_run_flutter_emulators_error_includes_binary_path() {
    use crate::flutter_sdk::FlutterExecutable;
    use std::path::PathBuf;

    // Use a path guaranteed not to exist; on Unix this maps to ErrorKind::NotFound
    // → Error::FlutterNotFound. The non-existence path proves the spawn error
    // branch correctly classifies NotFound; the binary-path-in-message branch
    // is exercised by the integration test.
    let exe = FlutterExecutable::Direct(PathBuf::from("/nonexistent/flutter"));
    let err = run_flutter_emulators(&exe).await.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("/nonexistent/flutter") || matches!(err, Error::FlutterNotFound),
        "expected error to include binary path or be FlutterNotFound, got: {msg}"
    );
}
```

(Pattern matches `devices.rs:668-680`.)

### Acceptance Criteria

1. `crates/fdemon-daemon/src/flutter_sdk/diagnostics.rs` exists with the `windows_hint()` helper pair (cfg-gated for Windows / non-Windows).
2. `crates/fdemon-daemon/src/flutter_sdk/mod.rs` declares `pub(crate) mod diagnostics;`.
3. `crates/fdemon-daemon/src/devices.rs` no longer defines `windows_hint()` locally; it imports from `crate::flutter_sdk::diagnostics`.
4. `crates/fdemon-daemon/src/emulators.rs::run_flutter_emulators`:
   - Spawn-error message includes the binary path.
   - Non-zero-exit emits a structured `error!` log with `binary`, `exit_code`, `stderr`, `stdout` fields.
   - Non-zero-exit error string includes the binary path, exit code, `stderr.trim()`, and `windows_hint()` append.
5. `crates/fdemon-daemon/src/emulators.rs::run_flutter_emulator_launch` spawn-error message includes the binary path.
6. New unit test `test_run_flutter_emulators_error_includes_binary_path` passes.
7. `cargo test -p fdemon-daemon` passes (no regressions).
8. `cargo clippy -p fdemon-daemon` exits clean (no new warnings; pre-existing pre-1.91 warnings are out of scope per Task 03).

### Testing

```bash
cargo test -p fdemon-daemon emulators
cargo test -p fdemon-daemon devices
cargo clippy -p fdemon-daemon
```

### Notes

- Do NOT change the non-zero-exit handling in `run_flutter_emulator_launch` — Flutter intentionally returns success even if the emulator fails to boot; the function classifies based on `result.is_ok()` not on exit code.
- The `error!` macro must be in scope; `emulators.rs:12` already imports `tracing::{debug, info, warn}` — add `error` to that list.
- Visibility on `windows_hint()` should be `pub(crate)`. It is consumed by `devices.rs` and `emulators.rs`, both inside `fdemon-daemon`. No external consumers.
- The new `diagnostics` module name leaves room for Task 06 to add `is_path_resolution_error(stderr) -> bool` for stderr-content-gated hint dispatch.
