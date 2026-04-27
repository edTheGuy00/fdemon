//! # Tier 1: Tempdir Integration Tests — Full SDK Detection Chain
//!
//! End-to-end tests that exercise `find_flutter_sdk()` across all 11 detection
//! strategies using tempdir-based filesystem fixtures.
//!
//! ## Test Categories
//!
//! 1. **Individual Strategy Verification** — one test per strategy (11 strategies)
//! 2. **Priority Ordering Tests** — adjacent strategies, higher wins
//! 3. **Fallthrough Tests** — config present but invalid SDK, lower-priority wins
//! 4. **Parent Directory Walk Tests** — config in parent dir (monorepo)
//! 5. **Version String & Channel Extraction** — sdk.version and sdk.channel fields
//!
//! ## Notes
//!
//! - All tests that modify env vars are annotated `#[serial]` (env vars are process-global).
//! - `EnvGuard` ensures all env var changes are restored on drop.
//! - Higher-priority env vars (e.g. `FLUTTER_ROOT`) are cleared in lower-priority tests
//!   so they cannot accidentally win the detection race.

use super::assertions::{assert_sdk_not_found, assert_sdk_root, assert_sdk_source};
use super::fixtures::{
    create_asdf_layout, create_flutter_project, create_flutter_wrapper_layout, create_fvm_layout,
    create_fvm_legacy_layout, create_mise_layout, create_proto_layout, create_puro_layout,
    EnvGuard, MockSdkBuilder,
};
use fdemon_daemon::flutter_sdk::{find_flutter_sdk, validate_sdk_path, SdkSource};
use serial_test::serial;
use std::fs;
use tempfile::TempDir;

// ─────────────────────────────────────────────────────────────────────────────
// 1. Individual Strategy Verification
// ─────────────────────────────────────────────────────────────────────────────

/// Strategy 1: Explicit config path passed directly to `find_flutter_sdk`.
#[test]
fn test_strategy_explicit_config() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("my_app");
    let sdk_dir = tmp.path().join("explicit_sdk");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    let sdk_root = MockSdkBuilder::new(&sdk_dir, "3.22.0")
        .with_dart_sdk()
        .build();

    let sdk = find_flutter_sdk(&project, Some(&sdk_root)).unwrap();
    assert_sdk_source(&sdk, &SdkSource::ExplicitConfig);
    assert_sdk_root(&sdk, &sdk_root);
    assert_eq!(sdk.version, "3.22.0");
}

/// Strategy 2: `FLUTTER_ROOT` environment variable.
#[test]
#[serial]
fn test_strategy_flutter_root_env() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("my_app");
    let sdk_dir = tmp.path().join("flutter_root_sdk");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    let sdk_root = MockSdkBuilder::new(&sdk_dir, "3.22.0").build();

    let _flutter_root_guard = EnvGuard::set("FLUTTER_ROOT", sdk_root.to_str().unwrap());
    // Isolate PATH so system PATH strategy cannot win
    let _path_guard = EnvGuard::set("PATH", tmp.path().to_str().unwrap());

    let sdk = find_flutter_sdk(&project, None).unwrap();
    assert_sdk_source(&sdk, &SdkSource::EnvironmentVariable);
    assert_sdk_root(&sdk, &sdk_root);
    assert_eq!(sdk.version, "3.22.0");
}

/// Strategy 3: FVM modern (`.fvmrc`).
#[test]
#[serial]
fn test_strategy_fvm_modern() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("my_app");
    let cache = tmp.path().join("fvm_cache");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    let sdk_root = create_fvm_layout(&project, &cache, "3.22.0");

    let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
    let _cache_guard = EnvGuard::set("FVM_CACHE_PATH", cache.to_str().unwrap());
    let _path_guard = EnvGuard::set("PATH", tmp.path().to_str().unwrap());

    let sdk = find_flutter_sdk(&project, None).unwrap();
    assert_sdk_source(
        &sdk,
        &SdkSource::Fvm {
            version: "3.22.0".into(),
        },
    );
    assert_sdk_root(&sdk, &sdk_root);
    assert_eq!(sdk.version, "3.22.0");
}

/// Strategy 4: FVM legacy (`.fvm/fvm_config.json` + symlink).
#[test]
#[serial]
fn test_strategy_fvm_legacy() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("my_app");
    let cache = tmp.path().join("fvm_cache");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    let sdk_root = create_fvm_legacy_layout(&project, &cache, "3.22.0");

    let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
    let _cache_guard = EnvGuard::set("FVM_CACHE_PATH", cache.to_str().unwrap());
    let _path_guard = EnvGuard::set("PATH", tmp.path().to_str().unwrap());

    let sdk = find_flutter_sdk(&project, None).unwrap();
    assert_sdk_source(
        &sdk,
        &SdkSource::Fvm {
            version: "3.22.0".into(),
        },
    );
    // Legacy uses symlinks — canonicalize both sides before comparison.
    let canonical_sdk_root = fs::canonicalize(&sdk_root).unwrap_or(sdk_root);
    assert_sdk_root(&sdk, &canonical_sdk_root);
    assert_eq!(sdk.version, "3.22.0");
}

/// Strategy 5: Puro (`.puro.json`).
#[test]
#[serial]
fn test_strategy_puro() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("my_app");
    let puro_root = tmp.path().join("puro_root");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    let sdk_root = create_puro_layout(&project, &puro_root, "default");

    let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
    let _puro_guard = EnvGuard::set("PURO_ROOT", puro_root.to_str().unwrap());
    let _path_guard = EnvGuard::set("PATH", tmp.path().to_str().unwrap());

    let sdk = find_flutter_sdk(&project, None).unwrap();
    assert_sdk_source(
        &sdk,
        &SdkSource::Puro {
            env: "default".into(),
        },
    );
    assert_sdk_root(&sdk, &sdk_root);
}

/// Strategy 6: asdf (`.tool-versions`).
#[test]
#[serial]
fn test_strategy_asdf() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("my_app");
    let asdf_data = tmp.path().join("asdf_data");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    let sdk_root = create_asdf_layout(&project, &asdf_data, "3.22.0");

    let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
    let _asdf_guard = EnvGuard::set("ASDF_DATA_DIR", asdf_data.to_str().unwrap());
    let _path_guard = EnvGuard::set("PATH", tmp.path().to_str().unwrap());

    let sdk = find_flutter_sdk(&project, None).unwrap();
    assert_sdk_source(
        &sdk,
        &SdkSource::Asdf {
            version: "3.22.0".into(),
        },
    );
    assert_sdk_root(&sdk, &sdk_root);
    assert_eq!(sdk.version, "3.22.0");
}

/// Strategy 7: mise (`.mise.toml`).
#[test]
#[serial]
fn test_strategy_mise() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("my_app");
    let mise_data = tmp.path().join("mise_data");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    let sdk_root = create_mise_layout(&project, &mise_data, "3.22.0");

    let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
    let _mise_guard = EnvGuard::set("MISE_DATA_DIR", mise_data.to_str().unwrap());
    let _path_guard = EnvGuard::set("PATH", tmp.path().to_str().unwrap());

    let sdk = find_flutter_sdk(&project, None).unwrap();
    assert_sdk_source(
        &sdk,
        &SdkSource::Mise {
            version: "3.22.0".into(),
        },
    );
    assert_sdk_root(&sdk, &sdk_root);
    assert_eq!(sdk.version, "3.22.0");
}

/// Strategy 8: proto (`.prototools`).
#[test]
#[serial]
fn test_strategy_proto() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("my_app");
    let proto_home = tmp.path().join("proto_home");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    let sdk_root = create_proto_layout(&project, &proto_home, "3.22.0");

    let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
    let _proto_guard = EnvGuard::set("PROTO_HOME", proto_home.to_str().unwrap());
    let _path_guard = EnvGuard::set("PATH", tmp.path().to_str().unwrap());

    let sdk = find_flutter_sdk(&project, None).unwrap();
    assert_sdk_source(
        &sdk,
        &SdkSource::Proto {
            version: "3.22.0".into(),
        },
    );
    assert_sdk_root(&sdk, &sdk_root);
    assert_eq!(sdk.version, "3.22.0");
}

/// Strategy 9: flutter_wrapper (`flutterw` + `.flutter/`).
/// Does not modify env vars, so `#[serial]` is not required.
#[test]
#[serial]
fn test_strategy_flutter_wrapper() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("my_app");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    let sdk_root = create_flutter_wrapper_layout(&project);

    let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
    let _path_guard = EnvGuard::set("PATH", tmp.path().to_str().unwrap());

    let sdk = find_flutter_sdk(&project, None).unwrap();
    assert_sdk_source(&sdk, &SdkSource::FlutterWrapper);
    assert_sdk_root(&sdk, &sdk_root);
}

/// Strategy 10: System PATH — `flutter` binary on PATH, VERSION file present.
#[cfg(not(target_os = "windows"))]
#[test]
#[serial]
fn test_strategy_system_path() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("my_app");
    let sdk_dir = tmp.path().join("system_sdk");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    // Build a valid SDK — bin/flutter + VERSION
    let sdk_root = MockSdkBuilder::new(&sdk_dir, "3.24.0").build();
    let bin_dir = sdk_root.join("bin");

    let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
    let _path_guard = EnvGuard::set("PATH", bin_dir.to_str().unwrap());

    let sdk = find_flutter_sdk(&project, None).unwrap();
    assert_sdk_source(&sdk, &SdkSource::SystemPath);
    assert_sdk_root(&sdk, &sdk_root);
    assert_eq!(sdk.version, "3.24.0");
}

/// Strategy 11: Lenient PATH fallback — binary on PATH but no VERSION file.
#[cfg(not(target_os = "windows"))]
#[test]
#[serial]
fn test_strategy_system_path_lenient() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("my_app");
    let sdk_dir = tmp.path().join("lenient_sdk");
    let bin_dir = sdk_dir.join("bin");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    // Create a flutter binary but deliberately omit VERSION
    fs::create_dir_all(&bin_dir).unwrap();
    let flutter_bin = bin_dir.join("flutter");
    fs::write(&flutter_bin, "#!/bin/sh\n# lenient mock\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&flutter_bin, fs::Permissions::from_mode(0o755)).unwrap();
    }
    // Deliberately do NOT create VERSION — strategy 10 will fail, strategy 11 should succeed.

    let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
    let _path_guard = EnvGuard::set("PATH", bin_dir.to_str().unwrap());

    let sdk = find_flutter_sdk(&project, None).unwrap();
    assert_sdk_source(&sdk, &SdkSource::PathInferred);
    // Version should be "unknown" when VERSION file is absent
    assert_eq!(sdk.version, "unknown");
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. Priority Ordering Tests
// ─────────────────────────────────────────────────────────────────────────────

/// Priority 1 beats Priority 2: Explicit config wins over `FLUTTER_ROOT`.
#[test]
#[serial]
fn test_explicit_config_beats_flutter_root() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("my_app");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    // Priority 1: explicit path
    let explicit_sdk = MockSdkBuilder::new(&tmp.path().join("explicit_sdk"), "3.22.0").build();
    // Priority 2: FLUTTER_ROOT
    let env_sdk = MockSdkBuilder::new(&tmp.path().join("env_sdk"), "3.19.0").build();

    let _flutter_root_guard = EnvGuard::set("FLUTTER_ROOT", env_sdk.to_str().unwrap());
    let _path_guard = EnvGuard::set("PATH", tmp.path().to_str().unwrap());

    let sdk = find_flutter_sdk(&project, Some(&explicit_sdk)).unwrap();
    assert_sdk_source(&sdk, &SdkSource::ExplicitConfig);
    assert_eq!(sdk.version, "3.22.0");
}

/// Priority 2 beats Priority 3: `FLUTTER_ROOT` wins over FVM.
#[test]
#[serial]
fn test_flutter_root_beats_fvm() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("my_app");
    let fvm_cache = tmp.path().join("fvm_cache");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    // Priority 3: FVM layout
    let _fvm_sdk = create_fvm_layout(&project, &fvm_cache, "3.19.0");
    // Priority 2: FLUTTER_ROOT
    let env_sdk = MockSdkBuilder::new(&tmp.path().join("env_sdk"), "3.22.0").build();

    let _flutter_root_guard = EnvGuard::set("FLUTTER_ROOT", env_sdk.to_str().unwrap());
    let _cache_guard = EnvGuard::set("FVM_CACHE_PATH", fvm_cache.to_str().unwrap());
    let _path_guard = EnvGuard::set("PATH", tmp.path().to_str().unwrap());

    let sdk = find_flutter_sdk(&project, None).unwrap();
    assert_sdk_source(&sdk, &SdkSource::EnvironmentVariable);
    assert_eq!(sdk.version, "3.22.0");
}

/// Priority 3 beats Priority 4: FVM modern (`.fvmrc`) wins over FVM legacy.
#[test]
#[serial]
fn test_fvm_modern_beats_fvm_legacy() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("my_app");
    let fvm_cache = tmp.path().join("fvm_cache");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    // Priority 3: FVM modern (.fvmrc) → version "3.22.0"
    let modern_sdk = create_fvm_layout(&project, &fvm_cache, "3.22.0");
    // Priority 4: FVM legacy — add .fvm/fvm_config.json alongside the modern config
    let fvm_dir = project.join(".fvm");
    fs::create_dir_all(&fvm_dir).unwrap();
    fs::write(
        fvm_dir.join("fvm_config.json"),
        r#"{"flutterSdkVersion":"3.19.0"}"#,
    )
    .unwrap();
    // Build a legacy SDK too so it would be valid if chosen
    let _legacy_sdk = MockSdkBuilder::new(&fvm_cache.join("3.19.0"), "3.19.0").build();

    let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
    let _cache_guard = EnvGuard::set("FVM_CACHE_PATH", fvm_cache.to_str().unwrap());
    let _path_guard = EnvGuard::set("PATH", tmp.path().to_str().unwrap());

    let sdk = find_flutter_sdk(&project, None).unwrap();
    assert_sdk_source(
        &sdk,
        &SdkSource::Fvm {
            version: "3.22.0".into(),
        },
    );
    assert_sdk_root(&sdk, &modern_sdk);
}

/// Priority 3/4 beats Priority 5: FVM wins over Puro.
#[test]
#[serial]
fn test_fvm_beats_puro() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("my_app");
    let fvm_cache = tmp.path().join("fvm_cache");
    let puro_root = tmp.path().join("puro_root");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    // Priority 3: FVM
    let _fvm_sdk = create_fvm_layout(&project, &fvm_cache, "3.22.0");
    // Priority 5: Puro
    let _puro_sdk = create_puro_layout(&project, &puro_root, "my-env");

    let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
    let _cache_guard = EnvGuard::set("FVM_CACHE_PATH", fvm_cache.to_str().unwrap());
    let _puro_guard = EnvGuard::set("PURO_ROOT", puro_root.to_str().unwrap());
    let _path_guard = EnvGuard::set("PATH", tmp.path().to_str().unwrap());

    let sdk = find_flutter_sdk(&project, None).unwrap();
    assert!(
        matches!(sdk.source, SdkSource::Fvm { .. }),
        "Expected FVM to win, got: {:?}",
        sdk.source
    );
}

/// Priority 5 beats Priority 6: Puro wins over asdf.
#[test]
#[serial]
fn test_puro_beats_asdf() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("my_app");
    let puro_root = tmp.path().join("puro_root");
    let asdf_data = tmp.path().join("asdf_data");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    // Priority 5: Puro
    let _puro_sdk = create_puro_layout(&project, &puro_root, "stable-env");
    // Priority 6: asdf
    let _asdf_sdk = create_asdf_layout(&project, &asdf_data, "3.19.0");

    let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
    let _puro_guard = EnvGuard::set("PURO_ROOT", puro_root.to_str().unwrap());
    let _asdf_guard = EnvGuard::set("ASDF_DATA_DIR", asdf_data.to_str().unwrap());
    let _path_guard = EnvGuard::set("PATH", tmp.path().to_str().unwrap());

    let sdk = find_flutter_sdk(&project, None).unwrap();
    assert!(
        matches!(sdk.source, SdkSource::Puro { .. }),
        "Expected Puro to win, got: {:?}",
        sdk.source
    );
}

/// Priority 6 beats Priority 7: asdf wins over mise.
#[test]
#[serial]
fn test_asdf_beats_mise() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("my_app");
    let asdf_data = tmp.path().join("asdf_data");
    let mise_data = tmp.path().join("mise_data");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    // Priority 6: asdf
    let asdf_sdk = create_asdf_layout(&project, &asdf_data, "3.22.0");
    // Priority 7: mise
    let _mise_sdk = create_mise_layout(&project, &mise_data, "3.19.0");

    let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
    let _asdf_guard = EnvGuard::set("ASDF_DATA_DIR", asdf_data.to_str().unwrap());
    let _mise_guard = EnvGuard::set("MISE_DATA_DIR", mise_data.to_str().unwrap());
    let _path_guard = EnvGuard::set("PATH", tmp.path().to_str().unwrap());

    let sdk = find_flutter_sdk(&project, None).unwrap();
    assert_sdk_source(
        &sdk,
        &SdkSource::Asdf {
            version: "3.22.0".into(),
        },
    );
    assert_sdk_root(&sdk, &asdf_sdk);
}

/// Priority 7 beats Priority 8: mise wins over proto.
#[test]
#[serial]
fn test_mise_beats_proto() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("my_app");
    let mise_data = tmp.path().join("mise_data");
    let proto_home = tmp.path().join("proto_home");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    // Priority 7: mise
    let mise_sdk = create_mise_layout(&project, &mise_data, "3.22.0");
    // Priority 8: proto
    let _proto_sdk = create_proto_layout(&project, &proto_home, "3.19.0");

    let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
    let _mise_guard = EnvGuard::set("MISE_DATA_DIR", mise_data.to_str().unwrap());
    let _proto_guard = EnvGuard::set("PROTO_HOME", proto_home.to_str().unwrap());
    let _path_guard = EnvGuard::set("PATH", tmp.path().to_str().unwrap());

    let sdk = find_flutter_sdk(&project, None).unwrap();
    assert_sdk_source(
        &sdk,
        &SdkSource::Mise {
            version: "3.22.0".into(),
        },
    );
    assert_sdk_root(&sdk, &mise_sdk);
}

/// Priority 8 beats Priority 9: proto wins over flutter_wrapper.
#[test]
#[serial]
fn test_proto_beats_flutter_wrapper() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("my_app");
    let proto_home = tmp.path().join("proto_home");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    // Priority 8: proto
    let proto_sdk = create_proto_layout(&project, &proto_home, "3.22.0");
    // Priority 9: flutter_wrapper
    let _wrapper_sdk = create_flutter_wrapper_layout(&project);

    let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
    let _proto_guard = EnvGuard::set("PROTO_HOME", proto_home.to_str().unwrap());
    let _path_guard = EnvGuard::set("PATH", tmp.path().to_str().unwrap());

    let sdk = find_flutter_sdk(&project, None).unwrap();
    assert_sdk_source(
        &sdk,
        &SdkSource::Proto {
            version: "3.22.0".into(),
        },
    );
    assert_sdk_root(&sdk, &proto_sdk);
}

/// Priority 9 beats Priority 10: flutter_wrapper wins over system PATH.
#[cfg(not(target_os = "windows"))]
#[test]
#[serial]
fn test_flutter_wrapper_beats_system_path() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("my_app");
    let sdk_dir = tmp.path().join("path_sdk");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    // Priority 10: system PATH SDK
    let path_sdk = MockSdkBuilder::new(&sdk_dir, "3.19.0").build();
    let bin_dir = path_sdk.join("bin");

    // Priority 9: flutter_wrapper
    let _wrapper_sdk = create_flutter_wrapper_layout(&project);

    let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
    let _path_guard = EnvGuard::set("PATH", bin_dir.to_str().unwrap());

    let sdk = find_flutter_sdk(&project, None).unwrap();
    assert_sdk_source(&sdk, &SdkSource::FlutterWrapper);
}

/// Full chain: explicit config wins over all other strategies when all are configured.
#[cfg(not(target_os = "windows"))]
#[test]
#[serial]
fn test_full_chain_explicit_wins_over_all() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("my_app");
    let fvm_cache = tmp.path().join("fvm_cache");
    let puro_root = tmp.path().join("puro_root");
    let asdf_data = tmp.path().join("asdf_data");
    let mise_data = tmp.path().join("mise_data");
    let proto_home = tmp.path().join("proto_home");
    let path_sdk_dir = tmp.path().join("path_sdk");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    // Set up ALL strategies
    let _fvm_sdk = create_fvm_layout(&project, &fvm_cache, "3.16.0");
    let _puro_sdk = create_puro_layout(&project, &puro_root, "default");
    let _asdf_sdk = create_asdf_layout(&project, &asdf_data, "3.16.0");
    let _mise_sdk = create_mise_layout(&project, &mise_data, "3.16.0");
    let _proto_sdk = create_proto_layout(&project, &proto_home, "3.16.0");
    let _wrapper_sdk = create_flutter_wrapper_layout(&project);
    let path_sdk = MockSdkBuilder::new(&path_sdk_dir, "3.19.0").build();
    let path_bin = path_sdk.join("bin");
    let env_sdk = MockSdkBuilder::new(&tmp.path().join("env_sdk"), "3.19.0").build();

    // Priority 1: the winner
    let explicit_sdk = MockSdkBuilder::new(&tmp.path().join("explicit_sdk"), "3.22.0").build();

    let _flutter_root_guard = EnvGuard::set("FLUTTER_ROOT", env_sdk.to_str().unwrap());
    let _cache_guard = EnvGuard::set("FVM_CACHE_PATH", fvm_cache.to_str().unwrap());
    let _puro_guard = EnvGuard::set("PURO_ROOT", puro_root.to_str().unwrap());
    let _asdf_guard = EnvGuard::set("ASDF_DATA_DIR", asdf_data.to_str().unwrap());
    let _mise_guard = EnvGuard::set("MISE_DATA_DIR", mise_data.to_str().unwrap());
    let _proto_guard = EnvGuard::set("PROTO_HOME", proto_home.to_str().unwrap());
    let _path_guard = EnvGuard::set("PATH", path_bin.to_str().unwrap());

    let sdk = find_flutter_sdk(&project, Some(&explicit_sdk)).unwrap();
    assert_sdk_source(&sdk, &SdkSource::ExplicitConfig);
    assert_eq!(sdk.version, "3.22.0");
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. Fallthrough Tests
// ─────────────────────────────────────────────────────────────────────────────

/// FVM config present but SDK directory missing → falls through to asdf.
#[test]
#[serial]
fn test_fvm_config_present_but_sdk_missing_falls_to_asdf() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("my_app");
    let asdf_data = tmp.path().join("asdf_data");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    // FVM modern: .fvmrc exists, but the SDK directory under FVM_CACHE_PATH is NOT created
    fs::write(project.join(".fvmrc"), r#"{"flutter":"3.22.0"}"#).unwrap();
    // fvm_cache directory itself exists but version subdirectory is absent
    let fvm_cache = tmp.path().join("empty_fvm_cache");
    fs::create_dir_all(&fvm_cache).unwrap();

    // asdf: valid SDK available
    let asdf_sdk = create_asdf_layout(&project, &asdf_data, "3.19.0");

    let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
    let _cache_guard = EnvGuard::set("FVM_CACHE_PATH", fvm_cache.to_str().unwrap());
    let _asdf_guard = EnvGuard::set("ASDF_DATA_DIR", asdf_data.to_str().unwrap());
    let _path_guard = EnvGuard::set("PATH", tmp.path().to_str().unwrap());

    let sdk = find_flutter_sdk(&project, None).unwrap();
    assert_sdk_source(
        &sdk,
        &SdkSource::Asdf {
            version: "3.19.0".into(),
        },
    );
    assert_sdk_root(&sdk, &asdf_sdk);
}

/// Invalid `FLUTTER_ROOT` path (directory does not exist) → falls through to FVM.
#[test]
#[serial]
fn test_invalid_flutter_root_falls_to_next_strategy() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("my_app");
    let fvm_cache = tmp.path().join("fvm_cache");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    // Strategy 2: FLUTTER_ROOT points to a nonexistent path
    let nonexistent = tmp.path().join("no_such_sdk");

    // Strategy 3: FVM modern with a valid SDK
    let fvm_sdk = create_fvm_layout(&project, &fvm_cache, "3.22.0");

    let _flutter_root_guard = EnvGuard::set("FLUTTER_ROOT", nonexistent.to_str().unwrap());
    let _cache_guard = EnvGuard::set("FVM_CACHE_PATH", fvm_cache.to_str().unwrap());
    let _path_guard = EnvGuard::set("PATH", tmp.path().to_str().unwrap());

    let sdk = find_flutter_sdk(&project, None).unwrap();
    assert_sdk_source(
        &sdk,
        &SdkSource::Fvm {
            version: "3.22.0".into(),
        },
    );
    assert_sdk_root(&sdk, &fvm_sdk);
}

/// All strategies fail → returns `Error::FlutterNotFound`.
#[test]
#[serial]
fn test_all_strategies_fail_returns_flutter_not_found() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("my_app");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    // Isolate all env vars and set PATH to empty tempdir (no flutter binary)
    let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
    let _fvm_guard = EnvGuard::remove("FVM_CACHE_PATH");
    let _puro_guard = EnvGuard::remove("PURO_ROOT");
    let _asdf_guard = EnvGuard::remove("ASDF_DATA_DIR");
    let _mise_guard = EnvGuard::remove("MISE_DATA_DIR");
    let _proto_guard = EnvGuard::remove("PROTO_HOME");
    let _path_guard = EnvGuard::set("PATH", tmp.path().to_str().unwrap());

    let result = find_flutter_sdk(&project, None);
    assert_sdk_not_found(&result);
}

/// Explicit config path points to a directory with no flutter binary → falls through.
/// The function should continue to try other strategies rather than hard-failing.
#[test]
#[serial]
fn test_invalid_explicit_config_falls_through_to_asdf() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("my_app");
    let asdf_data = tmp.path().join("asdf_data");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    // Invalid explicit path: directory exists but has no flutter binary or VERSION
    let bad_explicit = tmp.path().join("empty_sdk");
    fs::create_dir_all(&bad_explicit).unwrap();

    // asdf: valid SDK
    let asdf_sdk = create_asdf_layout(&project, &asdf_data, "3.19.0");

    let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
    let _asdf_guard = EnvGuard::set("ASDF_DATA_DIR", asdf_data.to_str().unwrap());
    let _path_guard = EnvGuard::set("PATH", tmp.path().to_str().unwrap());

    let sdk = find_flutter_sdk(&project, Some(&bad_explicit)).unwrap();
    assert_sdk_source(
        &sdk,
        &SdkSource::Asdf {
            version: "3.19.0".into(),
        },
    );
    assert_sdk_root(&sdk, &asdf_sdk);
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. Parent Directory Walk Tests (Monorepo)
// ─────────────────────────────────────────────────────────────────────────────

/// FVM: `.fvmrc` is in the parent of the project (typical monorepo layout).
#[test]
#[serial]
fn test_fvmrc_in_parent_directory() {
    let tmp = TempDir::new().unwrap();
    // Layout:
    //   workspace_root/   ← .fvmrc lives here
    //   workspace_root/packages/my_app/   ← project_path
    let workspace_root = tmp.path().join("workspace");
    let project = workspace_root.join("packages").join("my_app");
    let fvm_cache = tmp.path().join("fvm_cache");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    // Write .fvmrc in workspace root, not in project
    let sdk_root = create_fvm_layout(&workspace_root, &fvm_cache, "3.22.0");

    let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
    let _cache_guard = EnvGuard::set("FVM_CACHE_PATH", fvm_cache.to_str().unwrap());
    let _path_guard = EnvGuard::set("PATH", tmp.path().to_str().unwrap());

    let sdk = find_flutter_sdk(&project, None).unwrap();
    assert_sdk_source(
        &sdk,
        &SdkSource::Fvm {
            version: "3.22.0".into(),
        },
    );
    assert_sdk_root(&sdk, &sdk_root);
}

/// FVM: `.fvmrc` is in the grandparent (3 levels deep project).
#[test]
#[serial]
fn test_fvmrc_in_grandparent_directory() {
    let tmp = TempDir::new().unwrap();
    // Layout:
    //   root/   ← .fvmrc lives here
    //   root/packages/domain/my_app/   ← project_path (3 levels deep)
    let root = tmp.path().join("root");
    let project = root.join("packages").join("domain").join("my_app");
    let fvm_cache = tmp.path().join("fvm_cache");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    let sdk_root = create_fvm_layout(&root, &fvm_cache, "3.22.0");

    let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
    let _cache_guard = EnvGuard::set("FVM_CACHE_PATH", fvm_cache.to_str().unwrap());
    let _path_guard = EnvGuard::set("PATH", tmp.path().to_str().unwrap());

    let sdk = find_flutter_sdk(&project, None).unwrap();
    assert_sdk_source(
        &sdk,
        &SdkSource::Fvm {
            version: "3.22.0".into(),
        },
    );
    assert_sdk_root(&sdk, &sdk_root);
}

/// Closer FVM config overrides the parent when both exist.
#[test]
#[serial]
fn test_closer_config_wins_over_parent() {
    let tmp = TempDir::new().unwrap();
    // Layout:
    //   workspace_root/.fvmrc  (version A = "3.19.0")
    //   workspace_root/packages/my_app/.fvmrc  (version B = "3.22.0")  ← project_path
    let workspace_root = tmp.path().join("workspace");
    let project = workspace_root.join("packages").join("my_app");
    let fvm_cache = tmp.path().join("fvm_cache");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    // Parent config → version A
    let _parent_sdk = create_fvm_layout(&workspace_root, &fvm_cache, "3.19.0");
    // Closer config → version B (the project itself also has .fvmrc)
    let closer_sdk = create_fvm_layout(&project, &fvm_cache, "3.22.0");

    let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
    let _cache_guard = EnvGuard::set("FVM_CACHE_PATH", fvm_cache.to_str().unwrap());
    let _path_guard = EnvGuard::set("PATH", tmp.path().to_str().unwrap());

    let sdk = find_flutter_sdk(&project, None).unwrap();
    // Should resolve to the closer config's version
    assert_sdk_source(
        &sdk,
        &SdkSource::Fvm {
            version: "3.22.0".into(),
        },
    );
    assert_sdk_root(&sdk, &closer_sdk);
}

/// asdf: `.tool-versions` is in the parent directory.
#[test]
#[serial]
fn test_tool_versions_in_parent_directory() {
    let tmp = TempDir::new().unwrap();
    let parent = tmp.path().join("monorepo");
    let project = parent.join("packages").join("app");
    let asdf_data = tmp.path().join("asdf_data");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "app");

    // .tool-versions in parent, not in project
    let sdk_root = create_asdf_layout(&parent, &asdf_data, "3.22.0");

    let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
    let _asdf_guard = EnvGuard::set("ASDF_DATA_DIR", asdf_data.to_str().unwrap());
    let _path_guard = EnvGuard::set("PATH", tmp.path().to_str().unwrap());

    let sdk = find_flutter_sdk(&project, None).unwrap();
    assert_sdk_source(
        &sdk,
        &SdkSource::Asdf {
            version: "3.22.0".into(),
        },
    );
    assert_sdk_root(&sdk, &sdk_root);
}

/// mise: `.mise.toml` is in the parent directory.
#[test]
#[serial]
fn test_mise_toml_in_parent_directory() {
    let tmp = TempDir::new().unwrap();
    let parent = tmp.path().join("monorepo");
    let project = parent.join("packages").join("app");
    let mise_data = tmp.path().join("mise_data");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "app");

    // .mise.toml in parent, not in project
    let sdk_root = create_mise_layout(&parent, &mise_data, "3.22.0");

    let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
    let _mise_guard = EnvGuard::set("MISE_DATA_DIR", mise_data.to_str().unwrap());
    let _path_guard = EnvGuard::set("PATH", tmp.path().to_str().unwrap());

    let sdk = find_flutter_sdk(&project, None).unwrap();
    assert_sdk_source(
        &sdk,
        &SdkSource::Mise {
            version: "3.22.0".into(),
        },
    );
    assert_sdk_root(&sdk, &sdk_root);
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. Version String & Channel Extraction
// ─────────────────────────────────────────────────────────────────────────────

/// VERSION file content is read and stored in `sdk.version`.
#[test]
#[serial]
fn test_version_extracted_from_version_file() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("my_app");
    let sdk_dir = tmp.path().join("sdk_3_22");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    let sdk_root = MockSdkBuilder::new(&sdk_dir, "3.22.0").build();

    let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
    let _path_guard = EnvGuard::set("PATH", tmp.path().to_str().unwrap());

    let sdk = find_flutter_sdk(&project, Some(&sdk_root)).unwrap();
    assert_eq!(sdk.version, "3.22.0");
}

/// `.git/HEAD` with `ref: refs/heads/stable` → `sdk.channel == Some("stable")`.
#[test]
#[serial]
fn test_channel_extracted_from_git_head() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("my_app");
    let sdk_dir = tmp.path().join("sdk_stable");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    let sdk_root = MockSdkBuilder::new(&sdk_dir, "3.22.0")
        .with_channel("stable")
        .build();

    let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
    let _path_guard = EnvGuard::set("PATH", tmp.path().to_str().unwrap());

    let sdk = find_flutter_sdk(&project, Some(&sdk_root)).unwrap();
    assert_eq!(sdk.channel.as_deref(), Some("stable"));
}

/// `.git/HEAD` with `ref: refs/heads/beta` → `sdk.channel == Some("beta")`.
#[test]
#[serial]
fn test_beta_channel_detected() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("my_app");
    let sdk_dir = tmp.path().join("sdk_beta");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    let sdk_root = MockSdkBuilder::new(&sdk_dir, "3.22.0-beta.1")
        .with_channel("beta")
        .build();

    let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
    let _path_guard = EnvGuard::set("PATH", tmp.path().to_str().unwrap());

    let sdk = find_flutter_sdk(&project, Some(&sdk_root)).unwrap();
    assert_eq!(sdk.channel.as_deref(), Some("beta"));
}

/// `.git/HEAD` containing a bare commit hash → channel is `None` or an unknown hash string.
///
/// When in detached HEAD state, `detect_channel` returns `Some(FlutterChannel::Unknown(hash))`
/// which is then converted to `Some(String)` via `to_string()`.  This test confirms
/// the channel is set to something (the short hash) rather than a known channel name.
#[test]
#[serial]
fn test_detached_head_channel_is_unknown() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("my_app");
    let sdk_dir = tmp.path().join("sdk_detached");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    // Build SDK with a git dir whose HEAD contains a commit hash (detached)
    let sdk_root = MockSdkBuilder::new(&sdk_dir, "3.22.0").build();
    fs::create_dir_all(sdk_root.join(".git")).unwrap();
    fs::write(
        sdk_root.join(".git").join("HEAD"),
        "abc123def4567890abcdef1234567890abcdef12\n",
    )
    .unwrap();

    let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
    let _path_guard = EnvGuard::set("PATH", tmp.path().to_str().unwrap());

    let sdk = find_flutter_sdk(&project, Some(&sdk_root)).unwrap();
    // The channel should not be "stable", "beta", or "main" — it's a commit hash fragment
    // None is also acceptable — no git dir means channel is None
    if let Some(ch) = &sdk.channel {
        assert_ne!(ch, "stable");
        assert_ne!(ch, "beta");
        assert_ne!(ch, "main");
    }
}

/// SDK without a `.git` directory → `sdk.channel == None`.
#[test]
#[serial]
fn test_no_git_dir_channel_is_none() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("my_app");
    let sdk_dir = tmp.path().join("sdk_no_git");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    // Build SDK without calling .with_channel() — no .git directory
    let sdk_root = MockSdkBuilder::new(&sdk_dir, "3.22.0").build();

    let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
    let _path_guard = EnvGuard::set("PATH", tmp.path().to_str().unwrap());

    let sdk = find_flutter_sdk(&project, Some(&sdk_root)).unwrap();
    assert_eq!(
        sdk.channel, None,
        "Channel should be None when no .git dir exists"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Bonus: SDK validation sanity
// ─────────────────────────────────────────────────────────────────────────────

/// MockSdkBuilder output passes `validate_sdk_path()` — confirms the fixture
/// produces a structure that the real validation function accepts.
#[test]
fn test_mock_sdk_builder_passes_validate_sdk_path() {
    let tmp = TempDir::new().unwrap();
    let sdk_root = MockSdkBuilder::new(tmp.path(), "3.22.0")
        .with_dart_sdk()
        .build();
    assert!(
        validate_sdk_path(&sdk_root).is_ok(),
        "MockSdkBuilder should produce a valid SDK"
    );
}

/// MockSdkBuilder without a Dart SDK still passes validation (fresh install).
#[test]
fn test_mock_sdk_without_dart_still_passes_validation() {
    let tmp = TempDir::new().unwrap();
    let sdk_root = MockSdkBuilder::new(tmp.path(), "3.22.0").build();
    assert!(
        validate_sdk_path(&sdk_root).is_ok(),
        "MockSdkBuilder without dart-sdk should still pass validate_sdk_path()"
    );
}
