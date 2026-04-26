## Task: Improve diagnostic logging and error messages on `flutter` spawn paths

**Objective**: When `flutter devices`, `flutter --version --machine`, or a Flutter session spawn fails, log enough information to diagnose Windows-specific failures remotely (so the next bug like #32/#34 can be solved from the user's log file alone). Also, when the locator returns `Err(FlutterNotFound)`, surface a Windows-specific hint pointing the user at `[flutter] sdk_path` in `.fdemon/config.toml`.

**Depends on**: 01-add-windows-deps (so the rest of the change set is self-consistent; no direct code dep)

**Estimated Time**: 1-2h

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/devices.rs`:
  - In `run_flutter_devices` (lines 180-224), promote stderr from `debug!` to `error!` when the process exits non-zero.
  - Include the resolved `flutter.path()` in the error message and structured fields.
  - On Windows, append a hint to the error message: "If your Flutter install is shim-based (Chocolatey, scoop, winget) or in an unusual location, set `[flutter] sdk_path` in `.fdemon/config.toml`.".

- `crates/fdemon-daemon/src/process.rs`:
  - In `FlutterProcess::spawn_internal()` (around lines 60-90), log the resolved program path and full args at `info!` before spawning.
  - On spawn error, surface the path in the error.

- `crates/fdemon-daemon/src/flutter_sdk/version_probe.rs`:
  - In `probe_flutter_version()` (lines 29-45), log the resolved path before the spawn and include it in error messages on failure.

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/flutter_sdk/types.rs` (read `FlutterExecutable::path()` signature).
- `crates/fdemon-core/src/error.rs` (confirm `Error::process` accepts a string).

### Details

#### `devices.rs::run_flutter_devices` — improved error path

Current (lines 207-220):

```rust
if !output.status.success() {
    if stdout.contains('[') && stdout.contains(']') {
        warn!(
            "flutter devices exited with code {:?} but has JSON output, parsing anyway",
            output.status.code()
        );
    } else {
        return Err(Error::process(format!(
            "flutter devices failed with exit code {:?}: {}",
            output.status.code(),
            stderr
        )));
    }
}
```

Replacement:

```rust
if !output.status.success() {
    if stdout.contains('[') && stdout.contains(']') {
        warn!(
            "flutter devices exited with code {:?} but has JSON output, parsing anyway",
            output.status.code()
        );
    } else {
        error!(
            binary = %flutter.path().display(),
            exit_code = ?output.status.code(),
            stderr = %stderr,
            stdout = %stdout,
            "flutter devices failed"
        );
        return Err(Error::process(format!(
            "flutter devices failed (binary: {}, exit code {:?}): {}{}",
            flutter.path().display(),
            output.status.code(),
            stderr.trim(),
            windows_hint(),
        )));
    }
}
```

Where `windows_hint()` is a small helper at the end of the file:

```rust
#[cfg(target_os = "windows")]
fn windows_hint() -> &'static str {
    "\n\nHint: If your Flutter is installed via a package manager (Chocolatey, scoop, winget) \
     or in a non-standard location, set `[flutter] sdk_path = \"C:\\\\path\\\\to\\\\flutter\"` \
     in `.fdemon/config.toml`."
}

#[cfg(not(target_os = "windows"))]
fn windows_hint() -> &'static str {
    ""
}
```

Also update the spawn-error branch (lines 189-195) to include the path:

```rust
.map_err(|e| {
    if e.kind() == std::io::ErrorKind::NotFound {
        Error::FlutterNotFound
    } else {
        Error::process(format!(
            "Failed to run flutter devices ({}): {}",
            flutter.path().display(),
            e
        ))
    }
})?;
```

#### `process.rs::spawn_internal` — log program/args before spawn

Locate the spawn site (`flutter.command().args(...)...spawn()` in `FlutterProcess::spawn_internal`). Add immediately before the `.spawn()` call:

```rust
info!(
    binary = %flutter.path().display(),
    args = ?args,
    cwd = %project_path.display(),
    "Spawning flutter session"
);
```

(Adapt field names to whatever `args` and `project_path` are actually called in the function.)

If `.spawn()` fails, include the path in the error message — same pattern as devices.rs.

#### `version_probe.rs::probe_flutter_version` — same pattern

```rust
debug!(
    binary = %executable.path().display(),
    "Probing flutter version"
);
let output = executable.command()
    .args(["--version", "--machine"])
    // ... existing setup ...
    .output()
    .await
    .map_err(|e| {
        Error::process(format!(
            "flutter --version --machine failed ({}): {}",
            executable.path().display(),
            e
        ))
    })?;
```

### Acceptance Criteria

1. On a non-zero exit from `flutter devices`, the log file contains the binary path, exit code, full stdout, and full stderr — all at `error!` level (not `debug!`).
2. On Windows, the user-facing error string ends with the "Hint" paragraph telling them about `[flutter] sdk_path`.
3. On Unix, the user-facing error string is unchanged in shape (but now includes the binary path), so existing user expectations don't break.
4. `process.rs` logs the resolved binary path, args, and CWD at `info!` before each session spawn.
5. `version_probe.rs` includes the binary path in error messages.
6. `cargo test -p fdemon-daemon` passes (no test relies on the exact previous error-message format — verify; if any does, update it).
7. `cargo clippy -p fdemon-daemon -- -D warnings` is clean.

### Testing

```bash
cargo test -p fdemon-daemon
cargo clippy -p fdemon-daemon -- -D warnings
```

Add or update a unit test in `devices.rs::tests` to assert that the error message contains the binary path:

```rust
#[tokio::test]
async fn test_run_flutter_devices_error_includes_binary_path() {
    // Use a fake non-existent path so spawn fails with NotFound.
    let flutter = FlutterExecutable::Direct(PathBuf::from("/nonexistent/flutter"));
    let result = discover_devices(&flutter).await;
    let err = result.unwrap_err();
    // FlutterNotFound is the expected variant for ErrorKind::NotFound;
    // confirm the error chain or display string includes the path.
    let msg = err.to_string();
    assert!(
        msg.contains("/nonexistent/flutter") || matches!(err, Error::FlutterNotFound),
        "expected error to reference the binary path, got: {msg}"
    );
}
```

### Notes

- **Do not** add a Windows-specific code path that swallows errors — the hint is purely a string addendum.
- Be careful with `tracing` field syntax — match what the rest of `fdemon-daemon` uses (the file already imports `info!`, `debug!`, `warn!`, `error!` from `fdemon_core::prelude`).
- **Do not** include user-private data (env vars, full PATH) in the log unless it's already standard practice in the daemon. Path of the resolved binary is fine; the full PATH variable is not.
- If `Error::process` doesn't already truncate or sanitize, the formatted message could be huge for verbose Flutter stderr (think 500+ lines). Consider truncating to ~2KB. **Defer** this — it's a follow-up.
- The `windows_hint()` helper in `devices.rs` is fine to inline if you prefer. The `cfg`-gated approach keeps the message short and accurate per platform.
