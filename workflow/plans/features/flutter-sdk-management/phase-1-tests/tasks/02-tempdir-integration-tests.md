## Task: Tier 1 — Tempdir Integration Tests (Detection Chain)

**Objective**: Create end-to-end integration tests that exercise `find_flutter_sdk()` across all 11 detection strategies using tempdir-based filesystem fixtures, verifying priority ordering, fallthrough behavior, and correct `SdkSource` identification.

**Depends on**: 01-shared-test-infrastructure

### Scope

- `tests/sdk_detection/tier1_detection_chain.rs`: All Tier 1 detection chain integration tests

### Details

These tests call `find_flutter_sdk(project_path, explicit_path)` directly (library-level), not through the binary. Each test creates a realistic filesystem layout in a tempdir and verifies the correct SDK is resolved.

#### Test Categories

##### 1. Individual Strategy Verification

One test per strategy confirming it works in isolation:

```rust
#[test] fn test_strategy_explicit_config() { ... }        // Priority 1
#[test] #[serial] fn test_strategy_flutter_root_env() { ... }  // Priority 2
#[test] #[serial] fn test_strategy_fvm_modern() { ... }         // Priority 3
#[test] #[serial] fn test_strategy_fvm_legacy() { ... }         // Priority 4
#[test] #[serial] fn test_strategy_puro() { ... }               // Priority 5
#[test] #[serial] fn test_strategy_asdf() { ... }               // Priority 6
#[test] #[serial] fn test_strategy_mise() { ... }               // Priority 7
#[test] #[serial] fn test_strategy_proto() { ... }              // Priority 8
#[test] fn test_strategy_flutter_wrapper() { ... }              // Priority 9
#[test] #[serial] fn test_strategy_system_path() { ... }        // Priority 10
#[test] #[serial] fn test_strategy_system_path_lenient() { ... } // Priority 11
```

Each test:
1. Creates only the fixtures for that specific strategy
2. Sets relevant env vars (with `EnvGuard`) to point at tempdir paths
3. Clears other env vars to prevent interference (e.g., unset `FLUTTER_ROOT`, `FVM_CACHE_PATH`)
4. Calls `find_flutter_sdk(&project_dir, explicit_path)`
5. Asserts `sdk.source` matches the expected `SdkSource` variant
6. Asserts `sdk.root` points to the expected path
7. Asserts `sdk.version` matches the version string written to `VERSION`

##### 2. Priority Ordering Tests

Tests that set up multiple strategies simultaneously and verify the highest-priority one wins:

```rust
#[test] #[serial]
fn test_explicit_config_beats_flutter_root() {
    // Set up both explicit path AND FLUTTER_ROOT
    // Assert: SdkSource::ExplicitConfig is returned
}

#[test] #[serial]
fn test_flutter_root_beats_fvm() {
    // Set up both FLUTTER_ROOT AND .fvmrc
    // Assert: SdkSource::EnvironmentVariable is returned
}

#[test] #[serial]
fn test_fvm_modern_beats_fvm_legacy() {
    // Set up both .fvmrc AND .fvm/fvm_config.json
    // Assert: SdkSource::Fvm with modern config version
}

#[test] #[serial]
fn test_fvm_beats_puro() { ... }

#[test] #[serial]
fn test_puro_beats_asdf() { ... }

#[test] #[serial]
fn test_asdf_beats_mise() { ... }

#[test] #[serial]
fn test_mise_beats_proto() { ... }

#[test] #[serial]
fn test_proto_beats_flutter_wrapper() { ... }

#[test] #[serial]
fn test_flutter_wrapper_beats_system_path() { ... }

#[test] #[serial]
fn test_full_chain_explicit_wins_over_all() {
    // Set up ALL strategies simultaneously
    // Assert: SdkSource::ExplicitConfig wins
}
```

##### 3. Fallthrough Tests

Tests where higher-priority strategies fail (config exists but SDK path is invalid), verifying correct fallthrough to lower-priority strategies:

```rust
#[test] #[serial]
fn test_fvm_config_present_but_sdk_missing_falls_to_asdf() {
    // .fvmrc exists but ~/fvm/versions/<ver>/ doesn't contain valid SDK
    // .tool-versions exists with valid asdf SDK
    // Assert: SdkSource::Asdf is returned
}

#[test] #[serial]
fn test_invalid_flutter_root_falls_to_next_strategy() {
    // FLUTTER_ROOT set but path doesn't contain valid SDK
    // FVM layout exists and is valid
    // Assert: SdkSource::Fvm is returned
}

#[test] #[serial]
fn test_all_strategies_fail_returns_flutter_not_found() {
    // Empty project dir, no env vars, empty PATH
    // Assert: Err(Error::FlutterNotFound)
}
```

##### 4. Parent Directory Walk Tests (Monorepo)

```rust
#[test] #[serial]
fn test_fvmrc_in_parent_directory() {
    // workspace_root/.fvmrc
    // workspace_root/packages/my_app/  ← project_path
    // Assert: finds .fvmrc from parent, resolves SDK
}

#[test] #[serial]
fn test_fvmrc_in_grandparent_directory() {
    // root/.fvmrc
    // root/packages/domain/my_app/  ← project_path (3 levels deep)
    // Assert: finds .fvmrc from grandparent
}

#[test] #[serial]
fn test_closer_config_wins_over_parent() {
    // workspace_root/.fvmrc (version A)
    // workspace_root/packages/my_app/.fvmrc (version B)  ← project_path
    // Assert: version B's SDK is returned
}

#[test] #[serial]
fn test_tool_versions_in_parent_directory() {
    // Same pattern but with .tool-versions for asdf
}

#[test] #[serial]
fn test_mise_toml_in_parent_directory() {
    // Same pattern but with .mise.toml
}
```

##### 5. Version String & Channel Extraction

```rust
#[test] #[serial]
fn test_version_extracted_from_version_file() {
    // SDK with VERSION file containing "3.22.0"
    // Assert: sdk.version == "3.22.0"
}

#[test] #[serial]
fn test_channel_extracted_from_git_head() {
    // SDK with .git/HEAD containing "ref: refs/heads/stable"
    // Assert: sdk.channel == Some("stable")
}

#[test] #[serial]
fn test_beta_channel_detected() {
    // .git/HEAD → "ref: refs/heads/beta"
    // Assert: sdk.channel == Some("beta")
}

#[test] #[serial]
fn test_detached_head_channel_is_none_or_unknown() {
    // .git/HEAD → bare commit hash
}

#[test] #[serial]
fn test_no_git_dir_channel_is_none() {
    // SDK without .git/ directory
}
```

### Acceptance Criteria

1. Every detection strategy (1-11) has at least one passing test in isolation
2. Priority ordering verified for every adjacent pair in the chain
3. Fallthrough tested: invalid high-priority → correct fallthrough to lower priority
4. Parent directory walk tested for at least FVM, asdf, and mise
5. Version string and channel extraction verified
6. All tests pass on `cargo test`
7. Tests use `#[serial]` for any test that modifies env vars
8. Tests use `EnvGuard` (from Task 01) for clean env var management

### Testing

```bash
cargo test --test sdk_detection tier1_detection_chain -- --nocapture
```

All tests are standard `#[test]` functions — no async, no Docker, no external deps.

### Notes

- These tests call `fdemon_daemon::flutter_sdk::find_flutter_sdk()` directly — not through the binary. This gives us precise control and fast execution.
- The `#[serial]` attribute is required for all tests that set/unset env vars (`FLUTTER_ROOT`, `FVM_CACHE_PATH`, `ASDF_DATA_DIR`, `MISE_DATA_DIR`, `PROTO_HOME`, `PURO_ROOT`, `PATH`).
- System PATH tests need special care — save the original `PATH`, prepend the mock SDK's `bin/` dir, then restore.
- Some tests may need to unset env vars that are set in the actual development environment (e.g., if the developer has `FLUTTER_ROOT` set globally). The `EnvGuard` handles this.
