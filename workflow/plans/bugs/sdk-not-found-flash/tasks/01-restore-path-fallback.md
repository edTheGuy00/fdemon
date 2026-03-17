## Task: Restore PATH-Based Fallback with SdkSource::PathInferred

**Objective**: Re-add a fallback after strategy 10 in `find_flutter_sdk` that creates a usable `FlutterSdk` when `flutter` is on PATH but the SDK root can't be fully resolved or the VERSION file is missing. Use a distinct `SdkSource::PathInferred` variant to make the limited resolution explicit.

**Depends on**: None

**Severity**: HIGH — causes visible error flash on every startup for affected installations

### Scope

- `crates/fdemon-daemon/src/flutter_sdk/types.rs`: Add `SdkSource::PathInferred` variant
- `crates/fdemon-daemon/src/flutter_sdk/locator.rs`: Add lenient PATH fallback (strategy 11)

### Details

#### Why the Bare Fallback Was Needed

The Phase 1 fixes Task 02 removed `try_system_path_bare()` because it created a `FlutterSdk` with a fake `root: PathBuf::from("flutter")` and `version: "unknown"`, using the same `SdkSource::SystemPath` variant as a properly resolved SDK. This was correctly flagged as misleading.

However, removing it entirely created a regression: on machines where the `flutter` binary is a wrapper script (Homebrew shim, snap, etc.) or the resolved SDK root has a missing/unreadable VERSION file, strategy 10 (`try_system_path`) fails via `try_resolve_sdk` returning `None`, and no further strategies exist.

#### The Fix: Strategy 11 — Lenient PATH Fallback

After strategy 10 fails, add a lenient fallback that:

1. Re-scans PATH for a `flutter` binary (reuse `try_system_path` scan)
2. If found, attempts `resolve_sdk_root_from_binary` to get the SDK root
3. If SDK root resolves, calls `validate_sdk_path` to verify the binary exists
4. Builds a `FlutterSdk` **without requiring** `read_version_file` — uses `version: "unknown".to_string()` and `channel: None` if the VERSION file is missing
5. Uses `SdkSource::PathInferred` to distinguish from a fully resolved `SdkSource::SystemPath`

```rust
// Strategy 11: Lenient PATH fallback — binary on PATH but VERSION file missing/unreadable
if let Some(sdk_root) = try_system_path() {
    match validate_sdk_path(&sdk_root) {
        Ok(executable) => {
            let version = read_version_file(&sdk_root).unwrap_or_else(|_| "unknown".to_string());
            let channel = detect_channel(&sdk_root).map(|c| c.to_string());
            let sdk = FlutterSdk {
                root: sdk_root,
                executable,
                source: SdkSource::PathInferred,
                version,
                channel,
            };
            info!(
                source = %sdk.source,
                version = %sdk.version,
                path = %sdk.root.display(),
                "Flutter SDK resolved (lenient — VERSION file may be missing)"
            );
            return Ok(sdk);
        }
        Err(e) => debug!("SDK detection: lenient PATH fallback — invalid: {e}"),
    }
}
```

**Why not just modify `try_resolve_sdk` to be lenient?** The strict behavior of `try_resolve_sdk` is correct for version-manager strategies (FVM, asdf, etc.) where a missing VERSION file indicates a corrupted or incomplete installation. The lenient behavior is only appropriate for the PATH fallback, where the user may have a working `flutter` binary without a standard SDK directory structure.

#### SdkSource::PathInferred Variant

```rust
// In types.rs, add to the SdkSource enum:
/// Flutter binary found on system PATH, but SDK could not be fully resolved.
/// The executable path is usable but version/channel may be unknown.
PathInferred,
```

The `Display` impl should return `"system PATH (inferred)"` to distinguish from `"system PATH"`.

### Acceptance Criteria

1. `SdkSource::PathInferred` variant exists in `types.rs` with appropriate `Display` impl
2. Strategy 11 (lenient PATH fallback) exists after strategy 10 in `find_flutter_sdk`
3. When `flutter` is on PATH and `validate_sdk_path` succeeds but `read_version_file` fails, `find_flutter_sdk` returns `Ok(sdk)` with `SdkSource::PathInferred` and `version: "unknown"`
4. When `flutter` is NOT on PATH at all, `find_flutter_sdk` still returns `Err(FlutterNotFound)`
5. All existing locator tests pass without modification
6. `cargo check --workspace && cargo test -p fdemon-daemon && cargo clippy -p fdemon-daemon -- -D warnings` passes

### Testing

```rust
#[test]
fn test_path_fallback_lenient_missing_version_file() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("my_app");
    fs::create_dir_all(&project).unwrap();

    // Create a valid SDK structure on PATH but WITHOUT a VERSION file
    let sdk_dir = tmp.path().join("flutter_sdk");
    let bin_dir = sdk_dir.join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    let flutter_bin = bin_dir.join("flutter");
    fs::write(&flutter_bin, "#!/bin/sh\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&flutter_bin, fs::Permissions::from_mode(0o755)).unwrap();
    }
    // No VERSION file created — this is the key scenario

    let original_path = std::env::var_os("PATH");
    std::env::set_var("PATH", &bin_dir);
    std::env::remove_var("FLUTTER_ROOT");
    let result = find_flutter_sdk(&project, None);
    match original_path {
        Some(v) => std::env::set_var("PATH", v),
        None => std::env::remove_var("PATH"),
    }

    let sdk = result.expect("Should succeed with lenient PATH fallback");
    assert!(matches!(sdk.source, SdkSource::PathInferred));
    assert_eq!(sdk.version, "unknown");
}

#[test]
fn test_all_strategies_fail_no_flutter_on_path() {
    // Existing test — verify it still returns Err(FlutterNotFound)
    // when flutter is truly not available anywhere
}
```

### Notes

- The lenient fallback deliberately re-scans PATH rather than caching the strategy 10 scan result, because `try_resolve_sdk` may have rejected candidates that the lenient resolver would accept.
- `resolve_sdk_root_from_binary` may fail for wrapper scripts — in that case, the lenient fallback also fails and `Err(FlutterNotFound)` is returned. This is acceptable; the wrapper script scenario is better handled by the user setting `flutter.sdk_path` in config.
- The `PathInferred` variant should be treated as a "best effort" detection in Phase 2 UI — possibly showing a warning that the SDK version couldn't be determined.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/flutter_sdk/types.rs` | Added `SdkSource::PathInferred` variant with `Display` returning `"system PATH (inferred)"`. Added `validate_sdk_path_lenient()` function that checks only for the flutter binary (no VERSION file requirement). Added `test_sdk_source_display_path_inferred` test. |
| `crates/fdemon-daemon/src/flutter_sdk/locator.rs` | Updated import to include `validate_sdk_path_lenient`. Updated module doc comment to describe 11 strategies. Added Strategy 11 (lenient PATH fallback) after Strategy 10, using `validate_sdk_path_lenient` and `SdkSource::PathInferred`. Added `test_path_fallback_lenient_missing_version_file` test. |

### Notable Decisions/Tradeoffs

1. **`validate_sdk_path_lenient` in `types.rs` not `locator.rs`**: The lenient validator lives next to `validate_sdk_path` in `types.rs` so it follows the same module ownership pattern. It duplicates the binary-check logic but intentionally omits the VERSION file check, making the distinction explicit.

2. **Re-scan PATH for Strategy 11**: As specified in the task notes, Strategy 11 re-calls `try_system_path()` rather than caching the result from Strategy 10. This is correct because `try_resolve_sdk` may have rejected a candidate that the lenient resolver would accept (e.g., missing VERSION). In practice `try_system_path()` is cheap (iterates PATH entries).

3. **Test is `#[cfg(not(target_os = "windows"))]`**: The test creates a Unix shell script as the flutter binary. The Windows equivalent would require `flutter.bat`, which would be a separate test. The core lenient-fallback logic is platform-agnostic.

### Testing Performed

- `cargo fmt --all` - Passed
- `cargo check -p fdemon-daemon` - Passed
- `cargo test -p fdemon-daemon` - Passed (682 tests, 0 failed, 3 ignored)
  - `flutter_sdk::locator::tests::test_path_fallback_lenient_missing_version_file` - Passed
  - `flutter_sdk::types::tests::test_sdk_source_display_path_inferred` - Passed
  - All 15 pre-existing locator tests - Passed without modification
- `cargo clippy -p fdemon-daemon -- -D warnings` - Passed (no warnings)

### Risks/Limitations

1. **Wrapper script SDK root resolution**: As noted in the task, if `resolve_sdk_root_from_binary` fails (e.g., for Homebrew shims that don't live in a standard `<root>/bin/` structure), Strategy 11 also fails and `Err(FlutterNotFound)` is returned. This is the intended behavior — those installations are better handled by `flutter.sdk_path` in config.
