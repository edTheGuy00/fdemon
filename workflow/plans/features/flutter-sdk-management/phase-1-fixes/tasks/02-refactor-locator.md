## Task: Refactor find_flutter_sdk — Extract Helper, Fix Error Propagation, Fix Bare PATH Fallback

**Objective**: Refactor the ~430-line `find_flutter_sdk` function to extract a shared `try_resolve_sdk` helper, fix `read_version_file` `?` propagation that aborts the entire chain, and remove the bare PATH fallback that creates a misleading `FlutterSdk`.

**Depends on**: None

**Addresses**: Review issues #2 (function length), #3 (? propagation), #4 (bare PATH fallback)

### Scope

- `crates/fdemon-daemon/src/flutter_sdk/locator.rs`: Major refactor of `find_flutter_sdk` + helper extraction
- `crates/fdemon-daemon/src/flutter_sdk/types.rs`: Possibly add `SdkSource::BarePathFallback` variant (if keeping fallback) or no change (if removing it)

### Details

#### Problem 1: Function Length (~430 lines, limit is 50)

The 10 strategy blocks share an identical 4-step validate/read-version/detect-channel/build pattern. Only two things vary:
1. **The candidate source** — how `sdk_root: PathBuf` is obtained
2. **The `SdkSource` variant** — how to construct it (some need `version.clone()`, Puro needs an `env` name extracted from the path)

#### Problem 2: `read_version_file` `?` Aborts the Chain

After `validate_sdk_path(&sdk_root)` succeeds, `read_version_file(&sdk_root)?` uses `?` at 10 call sites (lines 53, 88, 125, 167, 216, 258, 300, 342, 384, 424). If the VERSION file exists but is unreadable (permissions, race condition, encoding), the error propagates out of the entire `find_flutter_sdk` function — remaining strategies are never tried. This is inconsistent with `validate_sdk_path` failures which are logged and fall through.

#### Problem 3: Bare PATH Fallback Creates Misleading FlutterSdk

Lines 451-469 create `FlutterSdk { root: PathBuf::from("flutter"), version: "unknown" }` with `SdkSource::SystemPath` — indistinguishable from a properly resolved PATH SDK. The `root` is not a real directory, violating the type's contract.

#### The Refactored Design

**Step 1: Extract a `try_resolve_sdk` helper**

```rust
/// Validate a candidate SDK root and build a `FlutterSdk` if valid.
///
/// Returns `Ok(Some(sdk))` on success, `Ok(None)` if the candidate is
/// invalid or unreadable (falls through to next strategy), and never
/// returns `Err` — all errors are logged and swallowed.
fn try_resolve_sdk(
    sdk_root: PathBuf,
    make_source: impl FnOnce(&str) -> SdkSource,
    label: &str,
) -> Option<FlutterSdk> {
    match validate_sdk_path(&sdk_root) {
        Ok(executable) => {
            let version = match read_version_file(&sdk_root) {
                Ok(v) => v,
                Err(e) => {
                    debug!("SDK detection: {label} — VERSION file unreadable: {e}");
                    return None;  // Fall through to next strategy
                }
            };
            let channel = detect_channel(&sdk_root).map(|c| c.to_string());
            let source = make_source(&version);
            let sdk = FlutterSdk {
                root: sdk_root,
                executable,
                source,
                version,
                channel,
            };
            info!(
                source = %sdk.source,
                version = %sdk.version,
                path = %sdk.root.display(),
                "Flutter SDK resolved"
            );
            Some(sdk)
        }
        Err(e) => {
            debug!("SDK detection: {label} candidate invalid: {e}");
            None
        }
    }
}
```

This helper:
- Handles the `read_version_file` error by logging and returning `None` (fixes Problem 2)
- Takes a `make_source` closure that receives the version string, solving the `version.clone()` problem
- Reduces each strategy block from ~20 lines to 3-5 lines

**Three source-construction classes the closure handles:**

| Class | Example | Closure |
|-------|---------|---------|
| No version needed | ExplicitConfig, EnvironmentVariable, FlutterWrapper, SystemPath | `\|_\| SdkSource::ExplicitConfig` |
| Version needed | Fvm, Asdf, Mise, Proto | `\|v\| SdkSource::Fvm { version: v.to_string() }` |
| Extra data + no version | Puro | `\|_\| SdkSource::Puro { env }` (env captured by closure) |

**Step 2: Rewrite `find_flutter_sdk` using the helper**

Each strategy block becomes compact. Example for strategies 1-3:

```rust
pub fn find_flutter_sdk(project_path: &Path, explicit_path: Option<&Path>) -> Result<FlutterSdk> {
    // Strategy 1: Explicit config
    if let Some(sdk_root) = try_explicit_config(explicit_path) {
        if let Some(sdk) = try_resolve_sdk(sdk_root, |_| SdkSource::ExplicitConfig, "explicit config") {
            return Ok(sdk);
        }
    }

    // Strategy 2: FLUTTER_ROOT env var
    if let Some(sdk_root) = try_flutter_root_env() {
        if let Some(sdk) = try_resolve_sdk(sdk_root, |_| SdkSource::EnvironmentVariable, "FLUTTER_ROOT") {
            return Ok(sdk);
        }
    }

    // Strategy 3: FVM modern (.fvmrc)
    match version_managers::detect_fvm_modern(project_path) {
        Ok(Some(sdk_root)) => {
            if let Some(sdk) = try_resolve_sdk(sdk_root, |v| SdkSource::Fvm { version: v.to_string() }, "FVM modern") {
                return Ok(sdk);
            }
        }
        Ok(None) => debug!("SDK detection: FVM modern — no .fvmrc found"),
        Err(e) => debug!("SDK detection: FVM modern — error: {e}"),
    }

    // ... strategies 4-10 follow the same pattern ...

    // All strategies exhausted
    warn!("SDK detection: all strategies exhausted, Flutter SDK not found");
    Err(Error::FlutterNotFound)
}
```

**Step 3: Remove the bare PATH fallback (lines 451-469)**

The bare fallback (`try_system_path_bare`) creates a `FlutterSdk` with a fake `root` and `version: "unknown"`. Since `Engine::new()` already handles `Err(FlutterNotFound)` gracefully by logging a warning and continuing without an SDK, this fallback is unnecessary and misleading.

- Delete the `try_system_path_bare()` function (lines 547-580)
- Delete the bare PATH fallback block (lines 451-469)
- The function now ends with `Err(Error::FlutterNotFound)` after strategy 10 fails

**If the team decides to keep the fallback**, use a distinct `SdkSource::BarePathFallback` variant in `types.rs` to make the limited resolution explicit and allow downstream consumers to distinguish it.

### Acceptance Criteria

1. `find_flutter_sdk` function body is under 100 lines
2. A shared `try_resolve_sdk` helper handles validate/read-version/detect-channel/build
3. `read_version_file` failures within a strategy log a debug message and fall through to the next strategy (no `?` propagation)
4. The bare PATH fallback block and `try_system_path_bare` are removed
5. All 14 existing locator tests pass without modification (except `test_all_strategies_fail` which may need minor adjustment if it relied on the bare fallback)
6. Detection priority order is unchanged
7. `cargo check --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings` passes

### Testing

Existing tests should continue to pass since the refactor is behavior-preserving (except for the bare PATH fallback removal and the `?` propagation fix, which are intentional behavior changes).

The `?` propagation fix is testable:

```rust
#[test]
fn test_unreadable_version_file_falls_through_to_next_strategy() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("my_app");
    fs::create_dir_all(&project).unwrap();

    // Strategy 3: FVM modern — SDK exists but VERSION is unreadable
    fs::write(project.join(".fvmrc"), r#"{"flutter":"3.19.0"}"#).unwrap();
    let fvm_sdk = tmp.path().join("fvm_cache/versions/3.19.0");
    fs::create_dir_all(fvm_sdk.join("bin")).unwrap();
    fs::write(fvm_sdk.join("bin/flutter"), "#!/bin/sh\n").unwrap();
    // Create VERSION as a directory (unreadable as a file)
    fs::create_dir_all(fvm_sdk.join("VERSION")).unwrap();

    // Strategy 6: asdf — valid SDK
    fs::write(project.join(".tool-versions"), "flutter 3.16.0\n").unwrap();
    let asdf_sdk = tmp.path().join("asdf/installs/flutter/3.16.0");
    create_mock_sdk(&asdf_sdk, "3.16.0");

    std::env::remove_var("FLUTTER_ROOT");
    std::env::set_var("FVM_CACHE_PATH", tmp.path().join("fvm_cache/versions"));
    std::env::set_var("ASDF_DATA_DIR", tmp.path().join("asdf"));
    let result = find_flutter_sdk(&project, None).unwrap();
    std::env::remove_var("FVM_CACHE_PATH");
    std::env::remove_var("ASDF_DATA_DIR");

    // FVM had unreadable VERSION — should fall through to asdf
    assert!(matches!(result.source, SdkSource::Asdf { .. }));
}
```

### Notes

- The `try_resolve_sdk` helper signature uses `Option<FlutterSdk>` (not `Result`) because all errors are handled internally with fallthrough.
- Puro's env-name extraction (currently at locator.rs lines 211-215) moves to the closure setup before calling `try_resolve_sdk`.
- The `info!` log in `try_resolve_sdk` is the single place where "Flutter SDK resolved" is logged — the duplicate in `engine.rs` is removed in Task 03.
- The `resolve_sdk_root_from_binary` function can remain `pub(crate)` for now — it's used by `find_flutter_in_dir` internally.

---

## Completion Summary

**Status:** Not Started
