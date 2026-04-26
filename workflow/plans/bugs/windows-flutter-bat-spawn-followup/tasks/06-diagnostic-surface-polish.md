## Task: Diagnostic surface polish — gate hint, cache `try_system_path`, `InvalidInput` arm, ANSI strip, doc note

**Objective**: Address the medium-priority diagnostic-quality issues from `ACTION_ITEMS.md` (Minors 7, 8, 9 + security nits 14, 16) in a single coordinated task. These touch four files (`devices.rs`, `flutter_sdk/locator.rs`, `flutter_sdk/diagnostics.rs`, `process.rs`) but the changes are independent of each other — they're bundled into one task because they share a "polish the diagnostic surface" theme and avoid scattering tiny PRs.

**Depends on**: Task 01 (provides `flutter_sdk/diagnostics.rs`), Task 02 (already split args logging — this task adds the `InvalidInput` arm next to it), Task 04 (the cached `try_system_path()` change must respect the new Strategy 12) — Wave B

**Estimated Time**: 1.5-2h

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/flutter_sdk/diagnostics.rs`:
  - Add a new helper `is_path_resolution_error(stderr: &str) -> bool`.
  - Update `windows_hint()` callers to pass the stderr through this predicate.
- `crates/fdemon-daemon/src/devices.rs`:
  - Gate the `windows_hint()` append on `is_path_resolution_error(stderr)`.
  - Strip ANSI escape sequences from `stderr` before embedding in the user-facing error string and the `error!` log.
- `crates/fdemon-daemon/src/emulators.rs`:
  - Same hint-gating change as `devices.rs` (the Task-01 changes left `windows_hint()` unconditionally appended; this task gates it).
- `crates/fdemon-daemon/src/flutter_sdk/locator.rs`:
  - Cache the result of `try_system_path()` across strategies 10 and 11 (and the new Strategy 12 from Task 04).
  - Add a doc note to `try_system_path()` mentioning the explicit `[flutter] sdk_path` mitigation for security-sensitive environments.
- `crates/fdemon-daemon/src/process.rs`:
  - Add a third arm to `spawn_internal`'s spawn-error matcher (lines 80-88) for `ErrorKind::InvalidInput`, emitting a Windows-targeted message about dart-define escape failures.

**Files Read (Dependencies):**
- None beyond standard project context.

### Details

#### `diagnostics.rs` — add `is_path_resolution_error`

Append to the new module created by Task 01:

```rust
/// Returns `true` if the given stderr text indicates a Windows path-resolution
/// error — the kind that the `windows_hint()` advice can actually fix.
///
/// Matches phrases produced by `cmd.exe`, the NT loader, and `CreateProcessW`
/// when a binary or path cannot be resolved.
pub(crate) fn is_path_resolution_error(stderr: &str) -> bool {
    let lower = stderr.to_ascii_lowercase();
    lower.contains("cannot find the path")
        || lower.contains("system cannot find")
        || lower.contains("not recognized as an internal")
        || lower.contains("no such file or directory") // Unix counterpart, harmless on Windows
}
```

Add a unit test in the same file:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_path_resolution_error_matches_cmd_messages() {
        assert!(is_path_resolution_error("The system cannot find the path specified."));
        assert!(is_path_resolution_error("'flutter' is not recognized as an internal or external command"));
        assert!(!is_path_resolution_error("flutter doctor: please accept the Android licenses"));
        assert!(!is_path_resolution_error(""));
    }
}
```

#### `devices.rs` and `emulators.rs` — gate the hint

Current pattern (after Task 01 lands):

```rust
return Err(Error::process(format!(
    "flutter <verb> failed (binary: {}, exit code {:?}): {}{}",
    flutter.path().display(),
    output.status.code(),
    stderr.trim(),
    windows_hint(),
)));
```

Replacement:

```rust
let stderr_clean = strip_ansi(&stderr);  // see ANSI-strip section below
let hint = if is_path_resolution_error(&stderr_clean) {
    windows_hint()
} else {
    ""
};
return Err(Error::process(format!(
    "flutter <verb> failed (binary: {}, exit code {:?}): {}{}",
    flutter.path().display(),
    output.status.code(),
    stderr_clean.trim(),
    hint,
)));
```

Add `use crate::flutter_sdk::diagnostics::{is_path_resolution_error, windows_hint, strip_ansi};` to both files.

#### ANSI stripping helper

The `strip_ansi` helper goes in `diagnostics.rs`:

```rust
/// Strip ANSI escape sequences from a string. Useful when embedding a child
/// process's stderr into a user-facing error message — the TUI does not
/// interpret raw ANSI in error text, so leftover escapes appear as literal
/// noise.
pub(crate) fn strip_ansi(input: &str) -> String {
    // Minimal CSI-only stripper: handle ESC [ ... letter sequences.
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' && chars.peek() == Some(&'[') {
            chars.next(); // consume '['
            for inner in chars.by_ref() {
                if inner.is_ascii_alphabetic() {
                    break;
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}
```

Add a unit test:

```rust
#[test]
fn test_strip_ansi_removes_color_codes() {
    assert_eq!(strip_ansi("\x1b[31merror\x1b[0m: bad"), "error: bad");
    assert_eq!(strip_ansi("plain text"), "plain text");
    assert_eq!(strip_ansi(""), "");
}
```

(If a project already depends on `strip-ansi-escapes` or similar, consume it instead. As of this task no such dep exists, so the minimal in-house helper is preferred over adding another dep.)

#### `locator.rs` — cache `try_system_path()` result

Current (around lines 184-219, after Task 04 lands the Strategy 12 block):

```rust
// Strategy 10
if let Some(sdk_root) = try_system_path() { ... }

// Strategy 11
if let Some(sdk_root) = try_system_path() { ... }

// Strategy 12 (from Task 04)
if let Ok(binary_path) = which::which("flutter") { ... }
```

Replacement:

```rust
// Cache PATH-resolution result once for strategies 10, 11, and 12.
let path_resolution = try_system_path();

// Strategy 10
if let Some(ref sdk_root) = path_resolution {
    if let Some(sdk) = try_resolve_sdk(sdk_root.clone(), |_| SdkSource::SystemPath, "system PATH") {
        return Ok(sdk);
    }
} else {
    debug!("SDK detection: system PATH — flutter not found on PATH");
}

// Strategy 11
if let Some(ref sdk_root) = path_resolution {
    match validate_sdk_path_lenient(sdk_root) {
        ...
    }
}

// Strategy 12 (Task 04) — the binary-only fallback uses which::which directly.
// Note: try_system_path() walks up two parents to derive the SDK root; for
// Strategy 12 we want the binary itself, not the inferred root, so we cannot
// reuse `path_resolution` here. The cost of a second which::which call in this
// truly-last-resort branch is negligible.
if let Ok(binary_path) = which::which("flutter") { ... }
```

#### `try_system_path()` doc note

Update the existing doc comment on `try_system_path()` (line 295 area):

```rust
/// Strategy 10: Resolve `flutter` on the system PATH using `which::which`,
/// then walk up to the SDK root.
///
/// `which` respects `PATHEXT` on Windows (so it finds `flutter.bat`, `.cmd`,
/// or `.exe` according to the user's `PATHEXT` ordering) and follows symlinks
/// on Unix. It returns the absolute path to the binary; we then canonicalize
/// it (via `dunce::canonicalize` to avoid `\\?\` UNC prefixes on Windows) and
/// walk up two parents to find the SDK root (`<root>/bin/flutter`).
///
/// **Security note:** PATH-based binary resolution trusts every directory on
/// `PATH`. Users in security-sensitive environments (multi-tenant boxes,
/// shared developer machines) should pin an absolute SDK path via
/// `[flutter] sdk_path` in `.fdemon/config.toml` to bypass PATH lookup
/// entirely.
fn try_system_path() -> Option<PathBuf> {
    ...
}
```

#### `process.rs` — `InvalidInput` arm

Current spawn-error matcher (lines 80-88):

```rust
.map_err(|e| {
    if e.kind() == std::io::ErrorKind::NotFound {
        Error::FlutterNotFound
    } else {
        Error::ProcessSpawn {
            reason: format!("{} (binary: {})", e, flutter.path().display()),
        }
    }
})?;
```

Replacement:

```rust
.map_err(|e| match e.kind() {
    std::io::ErrorKind::NotFound => Error::FlutterNotFound,
    #[cfg(target_os = "windows")]
    std::io::ErrorKind::InvalidInput => Error::ProcessSpawn {
        reason: format!(
            "flutter spawn rejected an argument it could not safely escape (binary: {}). \
             This usually means a dart-define value contains characters cmd.exe cannot \
             pass safely (% ^ & | < > unmatched \"). Check launch.toml.",
            flutter.path().display()
        ),
    },
    _ => Error::ProcessSpawn {
        reason: format!("{} (binary: {})", e, flutter.path().display()),
    },
})?;
```

The cfg gate ensures the dedicated message only fires on Windows. On Unix, `InvalidInput` from `Command::spawn` is exceedingly rare and the catch-all is sufficient.

### Acceptance Criteria

1. `flutter_sdk/diagnostics.rs` exports `pub(crate) fn is_path_resolution_error(&str) -> bool` and `pub(crate) fn strip_ansi(&str) -> String`, each with a unit test.
2. `devices.rs` and `emulators.rs` no longer unconditionally append `windows_hint()`. The hint is appended only when `is_path_resolution_error(stderr)` returns `true`.
3. `devices.rs` and `emulators.rs` strip ANSI escape sequences from `stderr` before embedding it in error strings and `error!` logs.
4. `find_flutter_sdk` calls `try_system_path()` at most once per invocation; strategies 10 and 11 share the cached result. (Strategy 12 still calls `which::which("flutter")` directly because it needs the binary path, not the inferred SDK root.)
5. `try_system_path()`'s doc comment includes the security note about explicit `[flutter] sdk_path`.
6. `spawn_internal`'s spawn-error matcher includes a Windows-only `InvalidInput` arm with a clear message about dart-define escape failures.
7. All existing tests pass.
8. `cargo clippy -p fdemon-daemon` exits clean (no new warnings).

### Testing

```bash
cargo test -p fdemon-daemon flutter_sdk::diagnostics
cargo test -p fdemon-daemon flutter_sdk::locator
cargo test -p fdemon-daemon devices
cargo test -p fdemon-daemon emulators
cargo test -p fdemon-daemon process
cargo clippy -p fdemon-daemon
```

### Notes

- The hint-gating change keeps the hint *useful* — when the user actually has a path-resolution error, the hint fires; when the failure is unrelated (license errors, network proxy, adb crashed), the hint stays out of the way.
- The ANSI stripper is intentionally minimal (CSI-only). Flutter's CLI emits standard color sequences; exotic OSC or DCS sequences are not in scope. If the helper proves insufficient in practice, swap to the `strip-ansi-escapes` crate.
- Do NOT change the cfg gate on `windows_hint()` in `diagnostics.rs`. The hint should still return `""` on non-Windows; the gating predicate is orthogonal.
- Do NOT add a separate `linux_hint()` or `macos_hint()`. Those platforms have well-functioning native diagnostics; we don't need to layer on top.
- The `cfg(target_os = "windows")` arm on `InvalidInput` keeps the Unix flow simpler and avoids unneeded text on platforms where the error doesn't apply.
- `path_resolution.clone()` in the Strategy 10 block is deliberate — `try_resolve_sdk` consumes the path. The clone is one `PathBuf` per `find_flutter_sdk` call, negligible cost.
