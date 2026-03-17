## Task: Shared Test Infrastructure & Fixture Builders

**Objective**: Create reusable test utilities that build realistic version manager filesystem layouts in tempdirs, plus assertion helpers for verifying SDK detection results and parsing headless NDJSON output.

**Depends on**: None

### Scope

- `tests/sdk_detection.rs`: Integration test entry point — `mod sdk_detection;`
- `tests/sdk_detection/mod.rs`: Module root with shared imports and re-exports
- `tests/sdk_detection/fixtures.rs`: `MockSdkBuilder` and version manager layout creators
- `tests/sdk_detection/assertions.rs`: SDK result assertions and NDJSON event parsing

### Details

#### `MockSdkBuilder` — Reusable SDK Directory Creator

A builder that creates a valid Flutter SDK directory structure in a tempdir:

```rust
pub struct MockSdkBuilder {
    root: PathBuf,
    version: String,
    channel: Option<String>,
    include_dart_sdk: bool,
    create_bat_file: bool,
}

impl MockSdkBuilder {
    pub fn new(root: &Path, version: &str) -> Self { ... }
    pub fn with_channel(mut self, channel: &str) -> Self { ... }
    pub fn with_dart_sdk(mut self) -> Self { ... }
    pub fn with_bat_file(mut self) -> Self { ... }  // creates bin/flutter.bat
    pub fn build(self) -> PathBuf { ... }  // creates dirs + files, returns root
}
```

The `build()` method creates:
- `<root>/bin/flutter` (with `chmod 755` on Unix)
- `<root>/VERSION` file with version string
- `<root>/bin/cache/dart-sdk/` (if `include_dart_sdk`)
- `<root>/.git/HEAD` with `ref: refs/heads/<channel>` (if channel set)
- `<root>/bin/flutter.bat` (if `create_bat_file`)

#### Version Manager Layout Creators

One function per version manager, each creating the full expected directory structure:

```rust
/// Creates FVM v3 layout: .fvmrc + ~/fvm/versions/<version>/ with mock SDK
pub fn create_fvm_layout(
    project_dir: &Path,
    fvm_cache_dir: &Path,   // stands in for ~/fvm/versions/
    version: &str,
) -> PathBuf { ... }  // returns SDK root

/// Creates FVM v2 (legacy) layout: .fvm/fvm_config.json + .fvm/flutter_sdk symlink
pub fn create_fvm_legacy_layout(
    project_dir: &Path,
    fvm_cache_dir: &Path,
    version: &str,
) -> PathBuf { ... }

/// Creates Puro layout: .puro.json + ~/.puro/envs/<env>/flutter/
pub fn create_puro_layout(
    project_dir: &Path,
    puro_root: &Path,       // stands in for ~/.puro/
    env_name: &str,
) -> PathBuf { ... }

/// Creates asdf layout: .tool-versions + ~/.asdf/installs/flutter/<version>/
pub fn create_asdf_layout(
    project_dir: &Path,
    asdf_data_dir: &Path,   // stands in for ~/.asdf/
    version: &str,
) -> PathBuf { ... }

/// Creates mise layout: .mise.toml + ~/.local/share/mise/installs/flutter/<version>/
pub fn create_mise_layout(
    project_dir: &Path,
    mise_data_dir: &Path,
    version: &str,
) -> PathBuf { ... }

/// Creates proto layout: .prototools + ~/.proto/tools/flutter/<version>/
pub fn create_proto_layout(
    project_dir: &Path,
    proto_home: &Path,
    version: &str,
) -> PathBuf { ... }

/// Creates flutter_wrapper layout: flutterw + .flutter/ at project root
pub fn create_flutter_wrapper_layout(
    project_dir: &Path,
) -> PathBuf { ... }

/// Creates a minimal Flutter project (pubspec.yaml only)
pub fn create_flutter_project(project_dir: &Path, name: &str) { ... }
```

Each function:
1. Creates the version manager config file in `project_dir`
2. Creates the SDK cache directory with a mock SDK (via `MockSdkBuilder`)
3. Returns the SDK root path for assertion comparison

#### Env Var Guard Helper

A RAII guard for setting/restoring environment variables (cleaner than manual save/restore):

```rust
pub struct EnvGuard {
    key: String,
    original: Option<String>,
}

impl EnvGuard {
    pub fn set(key: &str, value: &str) -> Self { ... }
    pub fn remove(key: &str) -> Self { ... }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        // restore original value
    }
}
```

#### Assertion Helpers

```rust
/// Assert that a FlutterSdk was resolved from the expected source
pub fn assert_sdk_source(sdk: &FlutterSdk, expected_source: &SdkSource) { ... }

/// Assert that a FlutterSdk points to the expected root path
pub fn assert_sdk_root(sdk: &FlutterSdk, expected_root: &Path) { ... }

/// Assert that find_flutter_sdk returns FlutterNotFound error
pub fn assert_sdk_not_found(result: &Result<FlutterSdk>) { ... }

/// Parse NDJSON stdout from headless mode into structured events
pub fn parse_headless_events(stdout: &str) -> Vec<HeadlessEvent> { ... }

/// A parsed headless NDJSON event
pub struct HeadlessEvent {
    pub event: String,
    pub message: Option<String>,
    pub fatal: Option<bool>,
    pub extra: serde_json::Value,
}
```

### Acceptance Criteria

1. `MockSdkBuilder` creates a valid SDK that passes `validate_sdk_path()`
2. Each version manager layout creator produces a structure that the corresponding `detect_*` function recognizes
3. `EnvGuard` correctly restores env vars on drop (including removing vars that didn't exist before)
4. Assertion helpers provide clear error messages on failure
5. NDJSON parser handles multi-line headless output correctly
6. All fixture functions are documented with `///` doc comments
7. No production code modified — test infrastructure only

### Testing

Self-test the fixture builders by verifying them against the actual detection functions:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use fdemon_daemon::flutter_sdk::{find_flutter_sdk, validate_sdk_path};

    #[test]
    fn test_mock_sdk_passes_validation() {
        let tmp = TempDir::new().unwrap();
        let sdk_root = MockSdkBuilder::new(tmp.path(), "3.22.0")
            .with_dart_sdk()
            .build();
        assert!(validate_sdk_path(&sdk_root).is_ok());
    }

    #[test]
    #[serial]
    fn test_fvm_fixture_is_detected() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path().join("my_app");
        let cache = tmp.path().join("fvm_cache");
        fs::create_dir_all(&project).unwrap();
        create_flutter_project(&project, "my_app");
        let sdk_root = create_fvm_layout(&project, &cache, "3.22.0");

        let _guard = EnvGuard::set("FVM_CACHE_PATH", cache.to_str().unwrap());
        let sdk = find_flutter_sdk(&project, None).unwrap();
        assert_sdk_source(&sdk, &SdkSource::Fvm { version: "3.22.0".into() });
        assert_sdk_root(&sdk, &sdk_root);
    }
}
```

### Notes

- `MockSdkBuilder` replaces the `create_mock_sdk()` local helper in `locator.rs` tests — the integration tests need the same capability but accessible from `tests/`
- The `EnvGuard` pattern is more ergonomic than manual save/restore but **still requires `#[serial]`** since `std::env::set_var` is process-global
- Fixture creators should match the actual filesystem layouts documented in the Phase 1 plan exactly — reference `version_managers.rs` for expected paths
- `parse_headless_events()` is needed for Tier 2 Docker tests (Task 07) but defined here so it's available early

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `tests/sdk_detection.rs` | New — integration test entry point; declares `mod sdk_detection { pub mod assertions; pub mod fixtures; }` and contains all 21 self-tests inline (following the `e2e.rs` pattern) |
| `tests/sdk_detection/fixtures.rs` | New — `MockSdkBuilder`, `EnvGuard`, `create_flutter_project`, and seven version-manager layout creators (`create_fvm_layout`, `create_fvm_legacy_layout`, `create_puro_layout`, `create_asdf_layout`, `create_mise_layout`, `create_proto_layout`, `create_flutter_wrapper_layout`) |
| `tests/sdk_detection/assertions.rs` | New — `assert_sdk_source`, `assert_sdk_root`, `assert_sdk_not_found`, `HeadlessEvent` struct, and `parse_headless_events` |

### Notable Decisions/Tradeoffs

1. **Module layout matches `e2e.rs` pattern**: The task spec showed `tests/sdk_detection/mod.rs` as the module root, but Rust's module resolution rules mean `tests/sdk_detection.rs` with `mod sdk_detection;` would look for `tests/sdk_detection/sdk_detection.rs`. The existing `e2e.rs` uses an inline `mod e2e { ... }` block. We followed the same pattern — tests live inline in `tests/sdk_detection.rs` and the shared helper modules are `tests/sdk_detection/fixtures.rs` and `tests/sdk_detection/assertions.rs`. The `mod.rs` file was removed.

2. **`assert_sdk_root` uses path canonicalization**: Both the actual SDK root and the expected path are canonicalized before comparison. This handles macOS's `/var` → `/private/var` symlink and the FVM legacy symlink that `fs::canonicalize` resolves in `detect_fvm_legacy`. If canonicalization fails (non-existent path), raw paths are compared as a fallback.

3. **`parse_headless_events` silently skips non-JSON lines**: This matches the task spec and makes the parser robust against tracing output interleaved with NDJSON (common in CI environments where stderr is merged into stdout).

4. **`with_channel` and `with_bat_file` builder methods are unused in self-tests**: These are `pub` because downstream test tasks (Tasks 02–07) will need them. The compiler warnings in test binaries are informational only and do not affect the quality gate.

### Testing Performed

- `cargo fmt --all` — Passed
- `cargo check --workspace` — Passed
- `cargo test --test sdk_detection -- --nocapture` — Passed (21 tests, 0 failed)
- `cargo test --workspace` — Passed (all existing tests continue to pass; 21 new tests added)
- `cargo clippy --workspace -- -D warnings` — Passed

### Risks/Limitations

1. **Serial tests share process-global env vars**: All tests that call `EnvGuard::set/remove` are annotated with `#[serial]`. Tests that do not touch env vars run in parallel safely.

2. **FVM legacy test relies on symlink creation**: `create_fvm_legacy_layout` creates a Unix symlink via `std::os::unix::fs::symlink`. The test will fail on Windows unless the `cfg(windows)` path is exercised. This is acceptable for the current macOS/Linux CI environment.
