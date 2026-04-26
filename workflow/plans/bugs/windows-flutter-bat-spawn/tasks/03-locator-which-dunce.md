## Task: Use `which::which` for system-PATH discovery and `dunce::canonicalize` for symlink resolution

**Objective**: Replace the hand-rolled PATH walker (`try_system_path` + `find_flutter_in_dir`) with the `which` crate, which respects `PATHEXT` on Windows so it correctly finds `flutter.bat` / `flutter.cmd` / `flutter.exe`. Replace `fs::canonicalize` (in `resolve_sdk_root_from_binary`) with `dunce::canonicalize` so the resolved path doesn't carry a `\\?\` UNC prefix that would later break `cmd.exe`. Together these fix shim-style installs (Chocolatey, scoop, winget) and the symlink-via-UNC failure mode (hypothesis 3 in BUG.md).

**Depends on**: 01-add-windows-deps

**Estimated Time**: 2-3h

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/flutter_sdk/locator.rs`:
  - Replace `try_system_path` (lines 304-314) with a `which::which("flutter")`-based implementation.
  - Replace `find_flutter_in_dir` (lines 317-340) — keep it as a fallback for environments where `which` returns `Err` but PATH walking might still find something, OR delete it if `which` is sufficient. Recommendation: delete; `which` covers PATHEXT, custom user extensions, symlinks, the lot.
  - Replace `fs::canonicalize` with `dunce::canonicalize` in `resolve_sdk_root_from_binary` (lines 346-350).
  - Add an `info!` log when `which` resolves a path, naming the path and the source ("which crate" vs the previous "PATH walk").

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/flutter_sdk/types.rs` (for `validate_sdk_path` / `validate_sdk_path_lenient` signatures — unchanged but referenced).
- `crates/fdemon-daemon/Cargo.toml` (confirm `which` and `dunce` are now declared).

### Details

#### Replace `try_system_path`

Current code (lines 304-314):

```rust
fn try_system_path() -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;

    for dir in std::env::split_paths(&path_var) {
        if let Some(sdk_root) = find_flutter_in_dir(&dir) {
            return Some(sdk_root);
        }
    }

    None
}
```

Replacement:

```rust
/// Strategy 10: Resolve `flutter` on the system PATH using `which::which`,
/// then walk up to the SDK root.
///
/// `which` respects `PATHEXT` on Windows (so it finds `flutter.bat`, `.cmd`,
/// or `.exe` according to the user's `PATHEXT` ordering) and follows symlinks
/// on Unix. It returns the absolute path to the binary; we then canonicalize
/// it (via `dunce::canonicalize` to avoid `\\?\` UNC prefixes on Windows) and
/// walk up two parents to find the SDK root (`<root>/bin/flutter`).
fn try_system_path() -> Option<PathBuf> {
    match which::which("flutter") {
        Ok(binary_path) => {
            debug!(path = %binary_path.display(), "SDK detection: which resolved flutter");
            resolve_sdk_root_from_binary(&binary_path)
        }
        Err(e) => {
            debug!("SDK detection: which::which(\"flutter\") failed: {e}");
            None
        }
    }
}
```

Then **delete** `find_flutter_in_dir` (lines 317-340) — `which` subsumes its functionality and handles edge cases (PATHEXT, App Execution Aliases on Windows, symlinks) that the hand-rolled walker did not.

#### Replace `fs::canonicalize` with `dunce::canonicalize`

Current code (lines 346-350):

```rust
pub(crate) fn resolve_sdk_root_from_binary(binary_path: &Path) -> Option<PathBuf> {
    let canonical = fs::canonicalize(binary_path).ok()?;
    canonical.parent()?.parent().map(|p| p.to_path_buf())
}
```

Replacement:

```rust
/// Given a path to a flutter binary, resolve the SDK root directory.
///
/// Expects the binary to be at `<root>/bin/flutter` (or `flutter.bat` on Windows).
/// Canonicalizes the path to follow symlinks, then walks up two levels.
///
/// Uses [`dunce::canonicalize`] instead of [`std::fs::canonicalize`] to avoid
/// `\\?\` UNC-prefixed paths on Windows. UNC prefixes are valid Win32 paths but
/// are not understood by `cmd.exe` — leaving them in place would break any
/// downstream invocation that flows through cmd. `dunce::canonicalize` returns
/// the same value as `fs::canonicalize` on Unix, so this is a transparent
/// upgrade for non-Windows targets.
pub(crate) fn resolve_sdk_root_from_binary(binary_path: &Path) -> Option<PathBuf> {
    let canonical = dunce::canonicalize(binary_path).ok()?;
    canonical.parent()?.parent().map(|p| p.to_path_buf())
}
```

Remove the `use std::fs;` import if it's no longer needed elsewhere in the file (check `cargo check` after the change — there are still `fs::create_dir_all`/`fs::write` calls in the test module which are gated by `#[cfg(test)]`, so likely the top-level import stays).

#### Confirm strategy 11 still works

Strategy 11 (lenient PATH fallback) at lines 196-222 calls `try_system_path()` again and applies `validate_sdk_path_lenient` to the returned root. With the new `try_system_path` it will continue to work:
- `which::which("flutter")` returns the absolute path to `flutter.bat` (Windows) or `flutter` (Unix).
- `resolve_sdk_root_from_binary` walks up two parents.
- For shim-style installs (e.g. Chocolatey: `C:\ProgramData\chocolatey\bin\flutter.bat`), walking up two parents gives `C:\ProgramData\chocolatey` — still not a real SDK. Strategy 10's `validate_sdk_path` will reject it (no `bin/cache/dart-sdk`). Strategy 11's `validate_sdk_path_lenient` *would* also reject it (no `bin/flutter.bat` directly inside the candidate root).

  **This is acceptable for this PR.** Shim users will see `Err(FlutterNotFound)` with a clearer hint to set `[flutter] sdk_path` in `.fdemon/config.toml` (added in task 04). A complete fix for shim-installs requires resolving the shim itself (e.g. via `where flutter` on Windows or by running `flutter --version` to detect the real install path), and is out of scope.

#### Logging additions

Add at the call site of strategy 10 in `find_flutter_sdk`:

```rust
// Strategy 10: System PATH (uses which crate for PATHEXT-aware lookup)
if let Some(sdk_root) = try_system_path() {
    if let Some(sdk) = try_resolve_sdk(sdk_root, |_| SdkSource::SystemPath, "system PATH") {
        return Ok(sdk);
    }
}
```

(no functional change beyond what's already there; the existing `try_resolve_sdk` already logs at `info!` on success).

### Acceptance Criteria

1. `try_system_path` uses `which::which("flutter")` and no longer iterates PATH manually.
2. `find_flutter_in_dir` is deleted (no remaining references).
3. `resolve_sdk_root_from_binary` uses `dunce::canonicalize`.
4. On Unix, behavior is unchanged: `find_flutter_sdk` still locates the SDK via PATH.
5. On Windows (verified by task 05's tests on CI), `find_flutter_sdk` correctly resolves to a path ending in `flutter.bat`, with no `\\?\` prefix, even when the binary is a symlink to a non-system drive.
6. The existing locator tests in `mod tests` (lines 356+) still pass on Unix. Some of them shell out to the real PATH; those should continue to behave identically.
7. `cargo clippy -p fdemon-daemon -- -D warnings` is clean.

### Testing

Existing Unix tests:
```bash
cargo test -p fdemon-daemon flutter_sdk::locator
cargo clippy -p fdemon-daemon -- -D warnings
```

Windows-specific tests live in `windows_tests.rs` (task 05) and run on CI only.

### Notes

- `which::which` returns `Result<PathBuf, which::Error>` — match on it; do not unwrap.
- On Windows, `which` may return paths with mixed casing (`C:\Program Files\flutter\BIN\flutter.BAT`) depending on filesystem state. That's fine — `Command::new` is case-insensitive on Windows.
- `which` does NOT search `cwd` by default on Unix (matching shell behavior); on Windows it does NOT search `cwd` either if you use `which::which` (vs. `which::which_in`). This matches our intent — we want PATH-based lookup, not "is there a flutter in the current Flutter project dir".
- Consider adding a tracing `instrument` macro on `try_system_path` and `resolve_sdk_root_from_binary` if other functions in the file already use it. Match the existing style.
- **Do not** modify `validate_sdk_path` or `validate_sdk_path_lenient` in this task — those changes (if any) belong in task 02. Keep this task tightly scoped to `locator.rs`.
