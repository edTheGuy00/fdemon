## Task: Tier 1 — Edge Case & Stress Tests

**Objective**: Test adversarial, malformed, and unusual filesystem states that the SDK detection chain must handle gracefully without panicking or returning incorrect results.

**Depends on**: 01-shared-test-infrastructure

### Scope

- `tests/sdk_detection/tier1_edge_cases.rs`: All Tier 1 edge case and stress tests

### Details

These tests exercise error paths, boundary conditions, and real-world messiness that users encounter. Each test uses tempdir fixtures and calls `find_flutter_sdk()` directly.

#### Test Categories

##### 1. Broken & Missing Symlinks

```rust
#[test] #[serial]
fn test_fvm_legacy_broken_symlink_falls_through() {
    // .fvm/flutter_sdk → dangling symlink (target deleted)
    // Assert: FVM legacy detection fails, falls to next strategy
}

#[test] #[serial]
fn test_fvm_legacy_circular_symlink() {
    // .fvm/flutter_sdk → .fvm/flutter_sdk (circular)
    // Assert: detection fails gracefully, no infinite loop
}

#[test] #[serial]
fn test_symlink_chain_resolves() {
    // bin/flutter → ../../other/flutter → ../real/bin/flutter
    // Assert: canonicalize resolves the chain to real SDK root
}
```

##### 2. Malformed Config Files

```rust
#[test] #[serial]
fn test_fvmrc_empty_file() {
    // .fvmrc exists but is empty (0 bytes)
    // Assert: FVM detection fails, falls through
}

#[test] #[serial]
fn test_fvmrc_invalid_json() {
    // .fvmrc contains "not json at all {"
    // Assert: FVM detection fails, falls through
}

#[test] #[serial]
fn test_fvmrc_missing_flutter_field() {
    // .fvmrc contains valid JSON but no "flutter" key: {"dart": "3.0.0"}
    // Assert: FVM detection fails, falls through
}

#[test] #[serial]
fn test_fvmrc_flutter_field_is_null() {
    // .fvmrc: {"flutter": null}
    // Assert: FVM detection fails, falls through
}

#[test] #[serial]
fn test_fvmrc_flutter_field_is_number() {
    // .fvmrc: {"flutter": 3.22}
    // Assert: FVM detection fails, falls through
}

#[test] #[serial]
fn test_puro_json_empty() { ... }

#[test] #[serial]
fn test_puro_json_missing_env_field() { ... }

#[test] #[serial]
fn test_tool_versions_empty_file() { ... }

#[test] #[serial]
fn test_tool_versions_no_flutter_line() {
    // .tool-versions: "python 3.11\nnodejs 18.0"
    // Assert: asdf detection fails, falls through
}

#[test] #[serial]
fn test_tool_versions_flutter_no_version() {
    // .tool-versions: "flutter"  (no version after tool name)
    // Assert: asdf detection fails or uses "latest"
}

#[test] #[serial]
fn test_mise_toml_invalid_toml() {
    // .mise.toml: "[invalid toml"
    // Assert: mise detection fails, falls through
}

#[test] #[serial]
fn test_mise_toml_no_tools_section() {
    // .mise.toml: "[settings]\nexperimental = true"
    // Assert: mise detection fails, falls through
}

#[test] #[serial]
fn test_prototools_invalid_toml() { ... }

#[test] #[serial]
fn test_prototools_no_flutter_key() { ... }
```

##### 3. Incomplete / Corrupted SDK Installations

```rust
#[test] #[serial]
fn test_sdk_missing_bin_flutter() {
    // SDK dir exists with VERSION file but no bin/flutter
    // Assert: validation fails, falls through
}

#[test] #[serial]
fn test_sdk_missing_version_file() {
    // SDK dir exists with bin/flutter but no VERSION
    // Assert: strict validation fails (may fall to lenient PATH)
}

#[test] #[serial]
fn test_sdk_version_file_empty() {
    // VERSION file exists but is 0 bytes
    // Assert: version is empty string or detection fails
}

#[test] #[serial]
fn test_sdk_version_file_with_trailing_newlines() {
    // VERSION: "3.22.0\n\n"
    // Assert: version is trimmed to "3.22.0"
}

#[test] #[serial]
fn test_sdk_bin_flutter_is_directory_not_file() {
    // bin/flutter exists but is a directory, not a file
    // Assert: validation fails
}

#[test] #[serial]
fn test_sdk_root_is_file_not_directory() {
    // The "SDK root" path points to a regular file
    // Assert: validation fails
}

#[test] #[serial]
fn test_sdk_no_dart_sdk_still_valid() {
    // SDK has bin/flutter and VERSION but no bin/cache/dart-sdk/
    // Assert: validation passes (dart-sdk absence is just a warning)
}
```

##### 4. Permission Edge Cases (Unix only)

```rust
#[test]
#[serial]
#[cfg(unix)]
fn test_sdk_bin_flutter_not_executable() {
    // bin/flutter exists but has mode 0o644 (not executable)
    // Assert: document current behavior — does validation pass or fail?
}

#[test]
#[serial]
#[cfg(unix)]
fn test_config_file_not_readable() {
    // .fvmrc exists but has mode 0o000
    // Assert: FVM detection fails gracefully, falls through
}

#[test]
#[serial]
#[cfg(unix)]
fn test_sdk_directory_not_traversable() {
    // SDK dir exists but has mode 0o000
    // Assert: validation fails gracefully
}
```

##### 5. Concurrent Version Manager Configs (Conflict Scenarios)

```rust
#[test] #[serial]
fn test_fvm_and_puro_both_present_fvm_wins() {
    // .fvmrc AND .puro.json in same project dir
    // Assert: FVM's SDK is returned (higher priority)
}

#[test] #[serial]
fn test_all_version_managers_present() {
    // .fvmrc, .puro.json, .tool-versions, .mise.toml, .prototools all present
    // Each pointing to different SDK versions
    // Assert: FVM wins (priority 3)
}

#[test] #[serial]
fn test_fvm_invalid_but_asdf_valid() {
    // .fvmrc present but points to non-existent version
    // .tool-versions present and valid
    // Assert: asdf SDK is returned (FVM failed validation)
}
```

##### 6. Unusual Path Patterns

```rust
#[test] #[serial]
fn test_path_with_spaces() {
    // Project dir: "/tmp/my flutter project/"
    // SDK dir: "/tmp/flutter sdk/versions/3.22.0/"
    // Assert: detection works with spaces in paths
}

#[test] #[serial]
fn test_deeply_nested_project() {
    // project_path is 20 directories deep
    // .fvmrc at root (19 levels up)
    // Assert: parent walk finds it
}

#[test] #[serial]
fn test_explicit_config_path_does_not_exist() {
    // explicit_path = Some("/nonexistent/path/to/flutter")
    // Assert: returns error, does NOT fall through to other strategies
    //         (explicit config is a hard error, not a soft fallthrough)
}

#[test] #[serial]
fn test_explicit_config_path_exists_but_invalid_sdk() {
    // explicit_path = Some("/tmp/empty_dir/")
    // Assert: returns error (invalid SDK at explicit path)
}

#[test] #[serial]
fn test_flutter_root_env_empty_string() {
    // FLUTTER_ROOT = ""
    // Assert: treated as unset, falls through
}
```

##### 7. Windows-Specific Path Logic (Cross-Platform)

```rust
#[test]
fn test_bat_file_detection_alongside_unix_binary() {
    // SDK has both bin/flutter AND bin/flutter.bat
    // Assert: on cfg(unix), Direct variant; documents expected behavior
}

#[test]
fn test_bat_file_only_no_unix_binary() {
    // SDK has bin/flutter.bat but NOT bin/flutter
    // Assert: documents expected behavior per platform
}
```

### Acceptance Criteria

1. All broken symlink scenarios handled without panic
2. All malformed config files cause graceful fallthrough (no panics, no unwrap failures)
3. Incomplete SDK installations detected and skipped
4. Permission errors handled gracefully on Unix
5. Conflict resolution follows priority order exactly
6. Paths with spaces work correctly
7. Explicit config path errors are hard failures (not fallthrough)
8. All tests pass on `cargo test`

### Testing

```bash
cargo test --test sdk_detection tier1_edge_cases -- --nocapture
```

### Notes

- These tests are the "chaos engineering" of SDK detection — they represent real-world scenarios users hit
- Permission tests (`#[cfg(unix)]`) won't run on Windows — that's fine since Windows permission model is fundamentally different
- The "explicit config path" behavior is a design decision worth documenting: should an invalid explicit path fall through to auto-detection, or should it be a hard error? Current implementation makes it a hard error (reasonable — if user explicitly configured a path, they want that path)
- Some tests may reveal gaps in the current implementation — that's the point. Document findings and file follow-up issues if needed.
