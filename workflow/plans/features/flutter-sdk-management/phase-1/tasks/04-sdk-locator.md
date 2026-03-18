## Task: SDK Locator (Detection Chain)

**Objective**: Implement the top-level `find_flutter_sdk()` function that walks the 10-strategy detection chain, validates candidates, and returns the first valid `FlutterSdk`. This is the central orchestrator that combines types (task 01), version manager parsers (task 02), and channel detection (task 03).

**Depends on**: 02-version-manager-parsers, 03-channel-version-extraction

### Scope

- `crates/fdemon-daemon/src/flutter_sdk/locator.rs`: **NEW** — Detection chain orchestration
- `crates/fdemon-daemon/src/flutter_sdk/mod.rs`: Add `mod locator` and re-export `find_flutter_sdk`

### Details

#### Detection Chain

The locator walks strategies in strict priority order, returning the first valid SDK:

```
Priority  Strategy              Config/Source                SDK Path Resolution
────────  ──────────            ─────────────                ───────────────────
1.        Explicit config       sdk_path argument            User-specified path
2.        FLUTTER_ROOT          env var                      $FLUTTER_ROOT
3.        FVM (modern)          .fvmrc                       ~/fvm/versions/<ver>/
4.        FVM (legacy)          .fvm/fvm_config.json         resolve .fvm/flutter_sdk symlink
5.        Puro                  .puro.json                   ~/.puro/envs/<env>/flutter/
6.        asdf                  .tool-versions               ~/.asdf/installs/flutter/<ver>/
7.        mise                  .mise.toml                   ~/.local/share/mise/installs/flutter/<ver>/
8.        proto                 .prototools                  ~/.proto/tools/flutter/<ver>/
9.        flutter_wrapper       flutterw + .flutter/         .flutter/ (project-local)
10.       System PATH           which/where flutter          resolve symlinks to real path
```

#### Main Function

```rust
/// Resolve the Flutter SDK for a given project.
///
/// Walks the detection chain in priority order. Returns the first valid SDK found.
/// Each strategy is logged at `debug!` level. The final result is logged at `info!`.
///
/// # Arguments
/// * `project_path` — Root of the Flutter project (used for tree walk and relative paths)
/// * `explicit_path` — Optional user-configured SDK path from config.toml `[flutter] sdk_path`
///
/// # Errors
/// Returns `Error::FlutterNotFound` if no valid SDK is found after trying all strategies.
pub fn find_flutter_sdk(
    project_path: &Path,
    explicit_path: Option<&Path>,
) -> Result<FlutterSdk>
```

#### Implementation Pattern

Each strategy follows this pattern:

```rust
// Strategy N: <name>
debug!("SDK detection: trying <name>...");
match try_strategy_n(args) {
    Ok(Some(sdk_root)) => {
        debug!("SDK detection: <name> found candidate at {}", sdk_root.display());
        match validate_sdk_path(&sdk_root) {
            Ok(executable) => {
                let version = read_version_file(&sdk_root)?;
                let channel = detect_channel(&sdk_root).map(|c| c.to_string());
                let sdk = FlutterSdk {
                    root: sdk_root,
                    executable,
                    source: SdkSource::StrategyName { ... },
                    version,
                    channel,
                };
                info!("Flutter SDK resolved via {}: {} at {}", sdk.source, sdk.version, sdk.root.display());
                return Ok(sdk);
            }
            Err(e) => {
                debug!("SDK detection: <name> candidate invalid: {e}");
                // Continue to next strategy
            }
        }
    }
    Ok(None) => {
        debug!("SDK detection: <name> — no config file found");
    }
    Err(e) => {
        debug!("SDK detection: <name> — error: {e}");
    }
}
```

#### Strategy 1: Explicit Config

```rust
fn try_explicit_config(explicit_path: Option<&Path>) -> Option<PathBuf> {
    explicit_path.map(|p| p.to_path_buf())
}
```

Simple — if the user specified a path, use it. Validation will catch bad paths.

#### Strategy 2: FLUTTER_ROOT Environment Variable

```rust
fn try_flutter_root_env() -> Option<PathBuf> {
    std::env::var_os("FLUTTER_ROOT").map(PathBuf::from)
}
```

#### Strategy 10: System PATH

This is the most complex strategy — resolve `flutter` from PATH and follow symlinks to find the SDK root:

```rust
/// Find flutter on the system PATH and resolve to the SDK root.
///
/// On Unix: search PATH for `flutter`, resolve symlinks, then
/// walk up from the binary to find the SDK root (binary is at `<root>/bin/flutter`).
///
/// On Windows: search PATH for `flutter.bat` or `flutter.exe`.
fn try_system_path() -> Option<PathBuf>
```

Implementation:
1. Split `PATH` env var by `std::env::split_paths`
2. For each directory, check if `flutter` (Unix) or `flutter.bat`/`flutter.exe` (Windows) exists
3. If found, `fs::canonicalize()` to resolve symlinks
4. The SDK root is the parent's parent of the binary (e.g., `/usr/local/flutter/bin/flutter` → `/usr/local/flutter/`)
5. Return the SDK root

#### Strategies 3–9: Version Manager Delegation

Each delegates to the corresponding function in `version_managers.rs`:

```rust
// Strategy 3: FVM modern
if let Ok(Some(path)) = version_managers::detect_fvm_modern(project_path) { ... }

// Strategy 4: FVM legacy
if let Ok(Some(path)) = version_managers::detect_fvm_legacy(project_path) { ... }

// ... etc
```

### Acceptance Criteria

1. `find_flutter_sdk()` returns the first valid SDK from the priority chain
2. If `explicit_path` is provided and valid, it is always returned (highest priority)
3. `FLUTTER_ROOT` env var takes priority over all version managers
4. Version managers are tried in the documented priority order
5. System PATH is tried last
6. `Error::FlutterNotFound` is returned when all strategies fail
7. Each strategy logs at `debug!` level (strategy name, path tried, validation result)
8. Final resolution logs at `info!` level (source, version, path)
9. Invalid candidates (failed validation) are skipped, and the chain continues
10. PATH resolution correctly follows symlinks and resolves the SDK root

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    /// Create a mock valid Flutter SDK directory structure.
    fn create_mock_sdk(root: &Path, version: &str) {
        fs::create_dir_all(root.join("bin/cache/dart-sdk")).unwrap();
        fs::write(root.join("bin/flutter"), "#!/bin/sh\n").unwrap();
        fs::write(root.join("VERSION"), version).unwrap();
    }

    #[test]
    fn test_explicit_path_takes_priority() {
        let tmp = TempDir::new().unwrap();
        let sdk_root = tmp.path().join("my-flutter");
        create_mock_sdk(&sdk_root, "3.19.0");

        let result = find_flutter_sdk(tmp.path(), Some(&sdk_root)).unwrap();
        assert_eq!(result.source, SdkSource::ExplicitConfig);
        assert_eq!(result.version, "3.19.0");
    }

    #[test]
    fn test_explicit_path_invalid_falls_through() {
        let tmp = TempDir::new().unwrap();
        let bad_path = tmp.path().join("nonexistent");

        // No other strategies will find anything either
        let result = find_flutter_sdk(tmp.path(), Some(&bad_path));
        assert!(result.is_err());
    }

    #[test]
    fn test_fvm_modern_detection() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path().join("my_app");
        fs::create_dir_all(&project).unwrap();

        // Create .fvmrc
        fs::write(project.join(".fvmrc"), r#"{"flutter":"3.19.0"}"#).unwrap();

        // Create mock SDK in FVM cache
        let cache = tmp.path().join("fvm_cache/versions/3.19.0");
        create_mock_sdk(&cache, "3.19.0");

        std::env::set_var("FVM_CACHE_PATH", tmp.path().join("fvm_cache/versions"));
        let result = find_flutter_sdk(&project, None).unwrap();
        std::env::remove_var("FVM_CACHE_PATH");

        assert!(matches!(result.source, SdkSource::Fvm { .. }));
        assert_eq!(result.version, "3.19.0");
    }

    #[test]
    fn test_all_strategies_fail_returns_flutter_not_found() {
        let tmp = TempDir::new().unwrap();
        // Empty directory — no config files, no PATH flutter
        let result = find_flutter_sdk(tmp.path(), None);
        assert!(result.is_err());
    }

    #[test]
    fn test_system_path_resolves_sdk_root() {
        let tmp = TempDir::new().unwrap();
        let sdk_root = tmp.path().join("flutter-sdk");
        create_mock_sdk(&sdk_root, "3.22.0");

        // The try_system_path function needs PATH manipulation for testing
        // This is tested via helper function directly
        let binary = sdk_root.join("bin/flutter");
        let resolved = resolve_sdk_root_from_binary(&binary);
        assert_eq!(resolved, Some(sdk_root));
    }

    #[test]
    fn test_priority_order_fvm_before_asdf() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path().join("my_app");
        fs::create_dir_all(&project).unwrap();

        // Both FVM and asdf configs present
        fs::write(project.join(".fvmrc"), r#"{"flutter":"3.19.0"}"#).unwrap();
        fs::write(project.join(".tool-versions"), "flutter 3.16.0\n").unwrap();

        // Create mock SDKs for both
        let fvm_sdk = tmp.path().join("fvm_cache/versions/3.19.0");
        create_mock_sdk(&fvm_sdk, "3.19.0");

        std::env::set_var("FVM_CACHE_PATH", tmp.path().join("fvm_cache/versions"));
        let result = find_flutter_sdk(&project, None).unwrap();
        std::env::remove_var("FVM_CACHE_PATH");

        // FVM should win (priority 3 vs asdf priority 6)
        assert!(matches!(result.source, SdkSource::Fvm { .. }));
    }
}
```

#### Helper for PATH Testing

The system PATH strategy is hard to test in isolation without modifying `PATH`. Extract the resolution logic into a testable helper:

```rust
/// Given a path to a flutter binary, resolve the SDK root directory.
/// Expects the binary to be at `<root>/bin/flutter`.
fn resolve_sdk_root_from_binary(binary_path: &Path) -> Option<PathBuf> {
    // canonicalize → parent (bin/) → parent (root/)
    let canonical = fs::canonicalize(binary_path).ok()?;
    canonical.parent()?.parent().map(|p| p.to_path_buf())
}
```

### Notes

- **All detection is synchronous**: The locator only does filesystem operations (stat, read, canonicalize). No process spawning. This is critical because `Engine::new()` is synchronous.
- **Graceful degradation**: Each strategy's failure (parse error, missing file, invalid SDK) is logged and skipped. Only after ALL strategies fail does the locator return `FlutterNotFound`.
- **env var tests**: Functions that read env vars should be designed to accept the env value as a parameter for testability, with the public API reading from `std::env` directly. This avoids thread-safety issues with `set_var`.
- **PATH on Windows**: Must check both `flutter.bat` and `flutter.exe`. The `PATHEXT` env var determines which extensions are searched.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-daemon/src/flutter_sdk/locator.rs` | NEW — 10-strategy detection chain implementation with 14 unit tests |
| `crates/fdemon-daemon/src/flutter_sdk/mod.rs` | Added `mod locator`, re-exported `find_flutter_sdk`, updated module docs |

### Notable Decisions/Tradeoffs

1. **`#[serial]` on env-var tests**: All tests that call `std::env::set_var` / `remove_var` are marked `#[serial]` (using the `serial_test` crate already present in `fdemon-daemon`). This prevents flaky failures from parallel test threads racing on shared process-wide env state.

2. **macOS `/var` → `/private/var` symlink in PATH tests**: `fs::canonicalize()` inside `resolve_sdk_root_from_binary` follows all symlinks, so on macOS temp paths like `/var/folders/...` are resolved to `/private/var/folders/...`. Tests that compare the result of `resolve_sdk_root_from_binary` against a path built from `TempDir::path()` now canonicalize both sides.

3. **Puro env extraction from path**: The `detect_puro` function returns the full SDK path (`<root>/envs/<env>/flutter/`). To populate `SdkSource::Puro { env }`, the locator extracts the env name as the grandparent directory component of the returned path (i.e., `path.parent()?.file_name()`). This is consistent with how Puro paths are structured.

4. **FVM source version from VERSION file**: For FVM (both modern and legacy), the `SdkSource::Fvm { version }` field is populated from the `VERSION` file read by `read_version_file()` rather than the version string parsed from `.fvmrc`. These should be identical for a correctly installed FVM environment, and using the file ensures the displayed version matches the actual SDK content.

5. **Pre-existing workspace errors**: Tasks 01-03 modified signatures in `fdemon-daemon/src/devices.rs`, `emulators.rs`, and `process.rs`. This caused `fdemon-app` call sites (to be updated in task 06) to fail workspace-level `cargo check`. Task 04 scopes only to `fdemon-daemon`; `cargo check -p fdemon-daemon` is clean.

### Testing Performed

- `cargo check -p fdemon-daemon` — Passed
- `cargo test -p fdemon-daemon -- locator` — Passed (14/14 tests)
- `cargo test -p fdemon-daemon -- flutter_sdk` — Passed (99/99 tests)
- `cargo test -p fdemon-daemon` — Passed (679 passed, 3 ignored pre-existing)
- `cargo clippy -p fdemon-daemon -- -D warnings` — Passed (no warnings)
- `cargo fmt -p fdemon-daemon` — Passed

### Risks/Limitations

1. **Env var isolation**: Tests use `#[serial]` to avoid parallel races, but `FLUTTER_ROOT` set in the host shell could still interfere if another serial test sets it and a crash leaves it set. This is mitigated by calling `remove_var("FLUTTER_ROOT")` at the start of each test that needs a clean state.

2. **System PATH strategy not fully unit-tested in isolation**: The `try_system_path()` function is not directly unit-tested because it reads the real `PATH` env var. The resolution logic is covered via `resolve_sdk_root_from_binary` and `find_flutter_in_dir` helper tests. An integration test with full PATH manipulation would require `#[serial]` and real binary creation.
