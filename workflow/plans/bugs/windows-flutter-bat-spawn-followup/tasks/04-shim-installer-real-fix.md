## Task: Add binary-only fallback (Strategy 12) for shim-installer Flutter installations

**Objective**: Implement Option B from `ACTION_ITEMS.md` (Major #3): when `which::which("flutter")` resolves a binary but neither `validate_sdk_path` nor `validate_sdk_path_lenient` accepts the inferred SDK root, fall back to a "binary-only" `FlutterSdk` whose `executable` is the `which` result and whose `root`/`version` are best-effort placeholders. This makes scoop and winget Flutter installations work out of the box, matching what BUG.md originally claimed.

**Depends on**: nothing — Wave A

**Estimated Time**: 2.5-3h

### Scope

**Files Modified (Write):**
- `crates/fdemon-daemon/src/flutter_sdk/locator.rs`:
  - Add a new `Strategy 12: Binary-only fallback` block to `find_flutter_sdk` after the existing Strategy 11 (lines 184-219).
  - Add cross-platform unit tests for the new strategy at the bottom of the existing `#[cfg(test)] mod tests` block.

**Files Read (Dependencies):**
- `crates/fdemon-daemon/src/flutter_sdk/types.rs` (read-only — to confirm `FlutterSdk`, `FlutterExecutable`, `SdkSource` shapes).

### Details

#### Background — current state

After the Wave-1 fix, the locator's PATH-resolution flow is:

```
try_system_path()                                  // calls which::which("flutter")
  → resolve_sdk_root_from_binary()                 // dunce::canonicalize, walk-up-2
    → validate_sdk_path()         (Strategy 10)    // checks <root>/bin/flutter.bat AND VERSION
    → validate_sdk_path_lenient() (Strategy 11)    // checks <root>/bin/flutter.bat (no VERSION)
```

For real SDK installations and chocolatey shims, `<root>/bin/flutter.bat` exists and Strategy 10 or 11 succeeds. For scoop (shim at `<root>/shims/`) and winget (shim at `<root>/Links/`), the inferred root has no `bin/flutter.bat`, so both strategies return `None` and `find_flutter_sdk` produces `Err(Error::FlutterNotFound)`.

The fix: if both strategies reject the inferred root but `which::which("flutter")` succeeded, the binary itself is a working Flutter executable — we just don't have a usable SDK root. Construct a `FlutterSdk` with the binary as executable and "unknown" SDK metadata.

#### New strategy block (insert after line 219, before the `warn!("SDK detection: all strategies exhausted...")`)

```rust
// Strategy 12: Binary-only fallback for shim installers (scoop, winget, etc.)
// When `which::which("flutter")` resolved a binary but the inferred SDK root
// failed both strict and lenient validation, the binary itself is still a
// working executable — package-manager shims like scoop's `shims/` and
// winget's `Links/` simply don't follow the canonical `<root>/bin/flutter`
// layout. We construct a FlutterSdk with placeholder metadata so the engine
// can spawn flutter; SDK-root-dependent features (channel detection, version
// pinning) gracefully degrade.
if let Ok(binary_path) = which::which("flutter") {
    let canonical_binary = dunce::canonicalize(&binary_path).unwrap_or(binary_path);
    let executable = FlutterExecutable::from_binary_path(&canonical_binary);
    let root = canonical_binary
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| canonical_binary.clone());
    let sdk = FlutterSdk {
        root,
        executable,
        source: SdkSource::PathInferred,
        version: "unknown".to_string(),
        channel: None,
    };
    info!(
        source = %sdk.source,
        binary = %canonical_binary.display(),
        "Flutter SDK resolved (binary-only fallback — shim installer detected)"
    );
    return Ok(sdk);
}

warn!("SDK detection: all strategies exhausted, Flutter SDK not found");
Err(Error::FlutterNotFound)
```

#### Helper: `flutter_executable_from_binary_path` (private to `locator.rs`)

The construction `FlutterExecutable::from_binary_path(&canonical_binary)` does not exist yet. Add a private helper at module scope inside `locator.rs` (NOT in `types.rs`) so this task stays confined to a single file and avoids overlap with Task 07:

```rust
/// Construct the platform-appropriate `FlutterExecutable` variant from a
/// canonical binary path. Used by Strategy 12's binary-only fallback.
///
/// On Windows, returns `WindowsBatch(path)` if the path's extension is
/// `.bat` or `.cmd`, otherwise `Direct(path)`. On non-Windows, always
/// returns `Direct(path)`.
fn flutter_executable_from_binary_path(path: &Path) -> FlutterExecutable {
    #[cfg(target_os = "windows")]
    {
        let is_batch = path
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.eq_ignore_ascii_case("bat") || s.eq_ignore_ascii_case("cmd"))
            .unwrap_or(false);
        if is_batch {
            return FlutterExecutable::WindowsBatch(path.to_path_buf());
        }
    }
    FlutterExecutable::Direct(path.to_path_buf())
}
```

The helper is intentionally private — Strategy 12 is the only caller, and keeping it in `locator.rs` means this task does not touch `types.rs` (which Task 07 modifies for an unrelated doc-comment change).

In the Strategy 12 block above, replace `FlutterExecutable::from_binary_path(&canonical_binary)` with `flutter_executable_from_binary_path(&canonical_binary)`.

#### Cross-platform unit tests for Strategy 12

Add to the existing `#[cfg(test)] mod tests` block in `locator.rs`. These tests should NOT require a real Flutter binary — instead they construct fake shim layouts and assert behavior.

```rust
#[test]
fn test_strategy_12_binary_only_fallback_when_inferred_root_invalid() {
    // Simulate a shim layout: <temp>/scoop/shims/flutter (Unix-ish for cross-platform test)
    let temp = TempDir::new().unwrap();
    let shims = temp.path().join("scoop").join("shims");
    fs::create_dir_all(&shims).unwrap();
    // Create a fake flutter binary (executable on Unix; the test only inspects
    // path resolution, not spawn).
    let binary = shims.join("flutter");
    fs::write(&binary, b"#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&binary).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&binary, perms).unwrap();
    }

    // Prepend shims to PATH and clear FLUTTER_ROOT so strategies 1-9 fail.
    let _guard = path_prepend_guard(&shims);
    std::env::remove_var("FLUTTER_ROOT");

    // walk-up-2 from <temp>/scoop/shims/flutter is <temp> — which has no
    // bin/flutter, so strategies 10 and 11 both fail and Strategy 12 fires.
    let project = TempDir::new().unwrap();
    let sdk = find_flutter_sdk(project.path(), None).unwrap();
    assert_eq!(sdk.source, SdkSource::PathInferred);
    assert_eq!(sdk.version, "unknown");
    assert!(sdk.channel.is_none());
    // The executable must be the canonical binary path itself.
    assert!(sdk.executable.path().ends_with("flutter") || sdk.executable.path().ends_with("flutter.bat"));
}
```

(The `path_prepend_guard` helper already exists in the test module — verify and reuse.)

#### Windows-only tests (delegated to Task 05)

Task 05 will add scoop and winget shim layout tests under `windows_tests.rs`:
- `test_find_flutter_sdk_scoop_shim_resolves_via_strategy_12`
- `test_find_flutter_sdk_winget_shim_resolves_via_strategy_12`

This task ends with the cross-platform unit test above; Task 05 lands the Windows-specific cases.

### Acceptance Criteria

1. `find_flutter_sdk` includes a Strategy 12 block that fires when strategies 1-11 fail but `which::which("flutter")` succeeds.
2. Strategy 12 returns a `FlutterSdk` with `source = SdkSource::PathInferred`, `version = "unknown"`, `channel = None`, and an `executable` matching the canonical `which` result.
3. The new private helper `flutter_executable_from_binary_path(&Path) -> FlutterExecutable` exists in `locator.rs` and is used by Strategy 12.
4. The new cross-platform unit test `test_strategy_12_binary_only_fallback_when_inferred_root_invalid` passes on macOS and Linux.
5. Existing locator tests (`mod tests`) still pass.
6. `cargo clippy -p fdemon-daemon` exits clean (no new warnings).
7. The `info!` log message for Strategy 12 distinctly says "binary-only fallback — shim installer detected" so it is greppable in user-shared logs.

### Testing

```bash
cargo test -p fdemon-daemon flutter_sdk::locator::tests
cargo test -p fdemon-daemon flutter_sdk::types
cargo clippy -p fdemon-daemon
```

### Notes

- The `dunce::canonicalize(&binary_path).unwrap_or(binary_path)` pattern is intentional. On Windows, dunce strips `\\?\`. On Unix it's a passthrough. If canonicalization fails (rare — e.g. permission errors), fall through to the original path; the executable is the ground truth either way.
- Strategy 12 must NOT call `try_system_path()` or `resolve_sdk_root_from_binary()`. Those helpers fail in exactly the case we want to handle. Call `which::which("flutter")` directly inside Strategy 12.
- Do NOT delete or merge Strategy 11. It still has value: when the SDK lives in a non-canonical place but the canonical `<root>/bin/flutter.bat` happens to exist (e.g. a manually-installed SDK without a VERSION file), Strategy 11 captures it with the correct `<root>` for metadata extraction.
- Keep the order: 1-9 (explicit/env/version-managers) → 10 (strict) → 11 (lenient) → 12 (binary-only). Strategy 12 is genuinely the last resort — it accepts any working flutter binary on PATH.
- The placeholder `version = "unknown"` may break callers that assume version is parseable. Audit `fdemon-app` callers (search for `sdk.version`) — if any consumer assumes a numeric form, they need to handle the literal "unknown" string. (This is mentioned in BUG.md's risks.)
- Resist the temptation to also fix the double-`try_system_path()` call in strategies 10 and 11. That cleanup is in Task 06.
