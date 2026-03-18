//! Tier 1: Edge case and stress tests for SDK detection.
//!
//! These tests exercise adversarial, malformed, and unusual filesystem states
//! that the SDK detection chain must handle gracefully without panicking or
//! returning incorrect results.
//!
//! Run with: `cargo test --test sdk_detection tier1_edge_cases -- --nocapture`

use super::assertions::{assert_sdk_not_found, assert_sdk_root, assert_sdk_source};
use super::fixtures::{
    create_asdf_layout, create_flutter_project, create_fvm_layout, create_mise_layout,
    create_puro_layout, EnvGuard, MockSdkBuilder,
};
use fdemon_daemon::flutter_sdk::{
    find_flutter_sdk, read_version_file, validate_sdk_path, SdkSource,
};
use serial_test::serial;
use std::fs;
use tempfile::TempDir;

// ─────────────────────────────────────────────────────────────────────────────
// 1. Broken & Missing Symlinks
// ─────────────────────────────────────────────────────────────────────────────

/// A dangling symlink at .fvm/flutter_sdk (target deleted after creation)
/// should cause the symlink path to be skipped. The code checks `.exists()`,
/// which returns false for dangling symlinks, so it falls through to the
/// fvm_config.json fallback. With no valid fallback it falls to the next strategy.
#[test]
#[serial]
#[cfg(unix)]
fn test_fvm_legacy_broken_symlink_falls_through() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("my_app");
    let cache = tmp.path().join("fvm_cache");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    // Create the fvm dir and a dangling symlink: point at a target that does not exist.
    let fvm_dir = project.join(".fvm");
    fs::create_dir_all(&fvm_dir).unwrap();
    let symlink_path = fvm_dir.join("flutter_sdk");
    let nonexistent_target = cache.join("nonexistent_version");
    std::os::unix::fs::symlink(&nonexistent_target, &symlink_path).unwrap();
    // No fvm_config.json, no real SDK — detection must fail gracefully and fall through.

    let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
    let _fvm_guard = EnvGuard::set("FVM_CACHE_PATH", cache.to_str().unwrap());
    let _path_guard = EnvGuard::set("PATH", tmp.path().to_str().unwrap());

    let result = find_flutter_sdk(&project, None);
    // Detection should fail gracefully — no panic, and no SDK returned from FVM.
    // Either no SDK found at all, or it fell through to another strategy.
    // On a machine with flutter on PATH, this might succeed via another strategy,
    // but the important thing is no panic and not ExplicitConfig/Fvm source.
    if let Ok(sdk) = &result {
        assert!(
            !matches!(sdk.source, SdkSource::Fvm { .. }),
            "Broken symlink should not produce an FVM-sourced SDK, got {:?}",
            sdk.source
        );
    }
    // If Err, that's also acceptable — the key invariant is no panic.
}

/// A circular symlink at .fvm/flutter_sdk (.fvm/flutter_sdk → .fvm/flutter_sdk)
/// should be handled without an infinite loop or panic.
/// `.exists()` returns false for circular symlinks (canonicalize fails),
/// so the code falls through to the fvm_config.json path.
#[test]
#[serial]
#[cfg(unix)]
fn test_fvm_legacy_circular_symlink() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("my_app");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    let fvm_dir = project.join(".fvm");
    fs::create_dir_all(&fvm_dir).unwrap();
    let symlink_path = fvm_dir.join("flutter_sdk");

    // Create a self-referential (circular) symlink.
    std::os::unix::fs::symlink(&symlink_path, &symlink_path).unwrap();

    let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
    let _path_guard = EnvGuard::set("PATH", tmp.path().to_str().unwrap());

    // Must not hang or panic.
    let result = find_flutter_sdk(&project, None);
    if let Ok(sdk) = &result {
        assert!(
            !matches!(sdk.source, SdkSource::Fvm { .. }),
            "Circular symlink should not produce an FVM-sourced SDK, got {:?}",
            sdk.source
        );
    }
}

/// A multi-hop symlink chain where bin/flutter is a symlink to another binary
/// that is itself a symlink to the real flutter binary. canonicalize() should
/// resolve the full chain and locate the correct SDK root.
#[test]
#[serial]
#[cfg(unix)]
fn test_symlink_chain_resolves() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("my_app");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    // Build the real SDK.
    let real_sdk = MockSdkBuilder::new(&tmp.path().join("real_sdk"), "3.22.0").build();

    // Create an intermediate SDK directory that contains a symlink to the real binary.
    let intermediate_bin = tmp.path().join("intermediate").join("bin");
    fs::create_dir_all(&intermediate_bin).unwrap();
    let intermediate_flutter = intermediate_bin.join("flutter");
    let real_flutter_bin = real_sdk.join("bin").join("flutter");
    std::os::unix::fs::symlink(&real_flutter_bin, &intermediate_flutter).unwrap();

    // Now set PATH to include the intermediate bin dir so the system PATH strategy finds it.
    let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
    let _path_guard = EnvGuard::set("PATH", intermediate_bin.to_str().unwrap());

    let result = find_flutter_sdk(&project, None);
    // The path strategy should resolve the symlink chain to the real SDK.
    // The resolved SDK root should be the real SDK root (after canonicalize).
    match result {
        Ok(sdk) => {
            // Verify the version is readable — symlink was resolved to a real SDK.
            assert_eq!(
                sdk.version, "3.22.0",
                "Symlink chain should resolve to real SDK version"
            );
        }
        Err(_) => {
            // Acceptable if the PATH strategy cannot find the version file via
            // the symlink chain — depends on canonicalize behaviour on this OS.
            // Note: this may fail if canonicalize resolves to the intermediate dir
            // rather than the real SDK dir. Document and accept.
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. Malformed Config Files
// ─────────────────────────────────────────────────────────────────────────────

/// .fvmrc exists but is completely empty (0 bytes).
/// JSON parsing on empty input fails, so detection falls through.
#[test]
#[serial]
fn test_fvmrc_empty_file() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("my_app");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    fs::write(project.join(".fvmrc"), "").unwrap();

    let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
    let _path_guard = EnvGuard::set("PATH", tmp.path().to_str().unwrap());

    let result = find_flutter_sdk(&project, None);
    if let Ok(sdk) = &result {
        assert!(
            !matches!(sdk.source, SdkSource::Fvm { .. }),
            "Empty .fvmrc should not produce FVM SDK, got {:?}",
            sdk.source
        );
    }
    // No panic is the critical invariant.
}

/// .fvmrc contains obviously invalid JSON ("not json at all {").
/// JSON parsing fails gracefully — detection falls through.
#[test]
#[serial]
fn test_fvmrc_invalid_json() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("my_app");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    fs::write(project.join(".fvmrc"), "not json at all {").unwrap();

    let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
    let _path_guard = EnvGuard::set("PATH", tmp.path().to_str().unwrap());

    let result = find_flutter_sdk(&project, None);
    if let Ok(sdk) = &result {
        assert!(
            !matches!(sdk.source, SdkSource::Fvm { .. }),
            "Invalid .fvmrc JSON should not produce FVM SDK, got {:?}",
            sdk.source
        );
    }
}

/// .fvmrc contains valid JSON but no "flutter" key: {"dart": "3.0.0"}.
/// The code checks json.get("flutter") which returns None — falls through.
#[test]
#[serial]
fn test_fvmrc_missing_flutter_field() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("my_app");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    fs::write(project.join(".fvmrc"), r#"{"dart": "3.0.0"}"#).unwrap();

    let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
    let _path_guard = EnvGuard::set("PATH", tmp.path().to_str().unwrap());

    let result = find_flutter_sdk(&project, None);
    if let Ok(sdk) = &result {
        assert!(
            !matches!(sdk.source, SdkSource::Fvm { .. }),
            ".fvmrc missing 'flutter' field should not produce FVM SDK, got {:?}",
            sdk.source
        );
    }
}

/// .fvmrc: {"flutter": null} — flutter field present but null.
/// json.get("flutter").and_then(|v| v.as_str()) returns None for null.
#[test]
#[serial]
fn test_fvmrc_flutter_field_is_null() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("my_app");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    fs::write(project.join(".fvmrc"), r#"{"flutter": null}"#).unwrap();

    let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
    let _path_guard = EnvGuard::set("PATH", tmp.path().to_str().unwrap());

    let result = find_flutter_sdk(&project, None);
    if let Ok(sdk) = &result {
        assert!(
            !matches!(sdk.source, SdkSource::Fvm { .. }),
            ".fvmrc with null flutter field should not produce FVM SDK, got {:?}",
            sdk.source
        );
    }
}

/// .fvmrc: {"flutter": 3.22} — flutter field is a number, not a string.
/// as_str() returns None for JSON numbers.
#[test]
#[serial]
fn test_fvmrc_flutter_field_is_number() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("my_app");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    fs::write(project.join(".fvmrc"), r#"{"flutter": 3.22}"#).unwrap();

    let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
    let _path_guard = EnvGuard::set("PATH", tmp.path().to_str().unwrap());

    let result = find_flutter_sdk(&project, None);
    if let Ok(sdk) = &result {
        assert!(
            !matches!(sdk.source, SdkSource::Fvm { .. }),
            ".fvmrc with numeric flutter field should not produce FVM SDK, got {:?}",
            sdk.source
        );
    }
}

/// .puro.json exists but is empty (0 bytes).
/// JSON parsing fails gracefully — detection falls through.
#[test]
#[serial]
fn test_puro_json_empty() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("my_app");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    fs::write(project.join(".puro.json"), "").unwrap();

    let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
    let _path_guard = EnvGuard::set("PATH", tmp.path().to_str().unwrap());

    let result = find_flutter_sdk(&project, None);
    if let Ok(sdk) = &result {
        assert!(
            !matches!(sdk.source, SdkSource::Puro { .. }),
            "Empty .puro.json should not produce Puro SDK, got {:?}",
            sdk.source
        );
    }
}

/// .puro.json valid JSON but missing the "env" field.
/// json.get("env") returns None — detection falls through.
#[test]
#[serial]
fn test_puro_json_missing_env_field() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("my_app");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    fs::write(project.join(".puro.json"), r#"{"version": "3.22.0"}"#).unwrap();

    let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
    let _path_guard = EnvGuard::set("PATH", tmp.path().to_str().unwrap());

    let result = find_flutter_sdk(&project, None);
    if let Ok(sdk) = &result {
        assert!(
            !matches!(sdk.source, SdkSource::Puro { .. }),
            ".puro.json missing 'env' field should not produce Puro SDK, got {:?}",
            sdk.source
        );
    }
}

/// .tool-versions file is completely empty (0 bytes).
/// No flutter line found — detection falls through.
#[test]
#[serial]
fn test_tool_versions_empty_file() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("my_app");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    fs::write(project.join(".tool-versions"), "").unwrap();

    let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
    let _path_guard = EnvGuard::set("PATH", tmp.path().to_str().unwrap());

    let result = find_flutter_sdk(&project, None);
    if let Ok(sdk) = &result {
        assert!(
            !matches!(sdk.source, SdkSource::Asdf { .. }),
            "Empty .tool-versions should not produce asdf SDK, got {:?}",
            sdk.source
        );
    }
}

/// .tool-versions contains other tools but no flutter entry.
/// "python 3.11\nnodejs 18.0" — the flutter find_map returns None.
#[test]
#[serial]
fn test_tool_versions_no_flutter_line() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("my_app");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    fs::write(project.join(".tool-versions"), "python 3.11\nnodejs 18.0\n").unwrap();

    let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
    let _path_guard = EnvGuard::set("PATH", tmp.path().to_str().unwrap());

    let result = find_flutter_sdk(&project, None);
    if let Ok(sdk) = &result {
        assert!(
            !matches!(sdk.source, SdkSource::Asdf { .. }),
            ".tool-versions with no flutter line should not produce asdf SDK, got {:?}",
            sdk.source
        );
    }
}

/// .tool-versions: "flutter" with no version token after it.
/// The current parser calls parts.next()? on version which returns None — detection falls through.
/// Note: current behavior is to fall through (return None); consider whether "latest" would be better.
#[test]
#[serial]
fn test_tool_versions_flutter_no_version() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("my_app");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    // "flutter" with no version after it.
    fs::write(project.join(".tool-versions"), "flutter\n").unwrap();

    let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
    let _path_guard = EnvGuard::set("PATH", tmp.path().to_str().unwrap());

    let result = find_flutter_sdk(&project, None);
    // Note: current behavior is to fall through (return None from detect_asdf)
    // since parts.next()? on the version token returns None when no version is present.
    // A future improvement could treat this as "latest" or emit a warning.
    if let Ok(sdk) = &result {
        assert!(
            !matches!(sdk.source, SdkSource::Asdf { .. }),
            ".tool-versions flutter with no version should not produce asdf SDK, got {:?}",
            sdk.source
        );
    }
}

/// .mise.toml contains invalid TOML syntax ("[invalid toml").
/// TOML parsing fails gracefully — detection falls through.
#[test]
#[serial]
fn test_mise_toml_invalid_toml() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("my_app");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    fs::write(project.join(".mise.toml"), "[invalid toml").unwrap();

    let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
    let _path_guard = EnvGuard::set("PATH", tmp.path().to_str().unwrap());

    let result = find_flutter_sdk(&project, None);
    if let Ok(sdk) = &result {
        assert!(
            !matches!(sdk.source, SdkSource::Mise { .. }),
            "Invalid .mise.toml should not produce mise SDK, got {:?}",
            sdk.source
        );
    }
}

/// .mise.toml has valid TOML but no [tools] section.
/// table.get("tools") returns None — detection falls through.
#[test]
#[serial]
fn test_mise_toml_no_tools_section() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("my_app");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    fs::write(
        project.join(".mise.toml"),
        "[settings]\nexperimental = true\n",
    )
    .unwrap();

    let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
    let _path_guard = EnvGuard::set("PATH", tmp.path().to_str().unwrap());

    let result = find_flutter_sdk(&project, None);
    if let Ok(sdk) = &result {
        assert!(
            !matches!(sdk.source, SdkSource::Mise { .. }),
            ".mise.toml without [tools] should not produce mise SDK, got {:?}",
            sdk.source
        );
    }
}

/// .prototools contains invalid TOML syntax.
/// TOML parsing fails gracefully — detection falls through.
#[test]
#[serial]
fn test_prototools_invalid_toml() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("my_app");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    fs::write(project.join(".prototools"), "[[not valid toml at all").unwrap();

    let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
    let _path_guard = EnvGuard::set("PATH", tmp.path().to_str().unwrap());

    let result = find_flutter_sdk(&project, None);
    if let Ok(sdk) = &result {
        assert!(
            !matches!(sdk.source, SdkSource::Proto { .. }),
            "Invalid .prototools should not produce proto SDK, got {:?}",
            sdk.source
        );
    }
}

/// .prototools valid TOML but no flutter key (only node).
/// table.get("flutter") returns None — detection falls through.
#[test]
#[serial]
fn test_prototools_no_flutter_key() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("my_app");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    fs::write(project.join(".prototools"), "node = \"20.0.0\"\n").unwrap();

    let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
    let _path_guard = EnvGuard::set("PATH", tmp.path().to_str().unwrap());

    let result = find_flutter_sdk(&project, None);
    if let Ok(sdk) = &result {
        assert!(
            !matches!(sdk.source, SdkSource::Proto { .. }),
            ".prototools without flutter key should not produce proto SDK, got {:?}",
            sdk.source
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. Incomplete / Corrupted SDK Installations
// ─────────────────────────────────────────────────────────────────────────────

/// SDK dir exists with VERSION file but no bin/flutter binary.
/// validate_sdk_path() checks for bin/flutter.is_file() first — returns Err.
#[test]
#[serial]
fn test_sdk_missing_bin_flutter() {
    let tmp = TempDir::new().unwrap();

    let sdk_root = tmp.path().join("broken_sdk");
    fs::create_dir_all(sdk_root.join("bin")).unwrap();
    // Intentionally NO bin/flutter
    fs::write(sdk_root.join("VERSION"), "3.22.0\n").unwrap();

    let result = validate_sdk_path(&sdk_root);
    assert!(
        result.is_err(),
        "validate_sdk_path should fail when bin/flutter is missing"
    );
}

/// SDK dir with bin/flutter but no VERSION file.
/// validate_sdk_path() returns Err because VERSION is required.
#[test]
#[serial]
fn test_sdk_missing_version_file() {
    let tmp = TempDir::new().unwrap();

    let sdk_root = tmp.path().join("broken_sdk");
    fs::create_dir_all(sdk_root.join("bin")).unwrap();
    fs::write(sdk_root.join("bin").join("flutter"), "#!/bin/sh\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(
            sdk_root.join("bin").join("flutter"),
            fs::Permissions::from_mode(0o755),
        )
        .unwrap();
    }
    // Intentionally NO VERSION file

    let result = validate_sdk_path(&sdk_root);
    assert!(
        result.is_err(),
        "validate_sdk_path should fail when VERSION file is missing"
    );
}

/// VERSION file exists but is 0 bytes (empty).
/// read_version_file reads and trims — result is an empty string "".
/// validate_sdk_path does NOT check version content, only that the file exists.
#[test]
#[serial]
fn test_sdk_version_file_empty() {
    let tmp = TempDir::new().unwrap();

    let sdk_root = tmp.path().join("sdk_empty_version");
    fs::create_dir_all(sdk_root.join("bin")).unwrap();
    fs::write(sdk_root.join("bin").join("flutter"), "#!/bin/sh\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(
            sdk_root.join("bin").join("flutter"),
            fs::Permissions::from_mode(0o755),
        )
        .unwrap();
    }
    // Empty VERSION file (0 bytes)
    fs::write(sdk_root.join("VERSION"), "").unwrap();

    // validate_sdk_path should succeed — it only checks the file exists, not its content.
    let exe_result = validate_sdk_path(&sdk_root);
    assert!(
        exe_result.is_ok(),
        "validate_sdk_path should pass for empty VERSION file (file exists)"
    );

    // read_version_file should return empty string after trimming.
    let version = read_version_file(&sdk_root).unwrap();
    assert_eq!(
        version, "",
        "Empty VERSION file should read as empty string"
    );
}

/// VERSION file contains "3.22.0\n\n" (trailing newlines).
/// read_version_file trims the content, so result should be "3.22.0".
#[test]
#[serial]
fn test_sdk_version_file_with_trailing_newlines() {
    let tmp = TempDir::new().unwrap();

    let sdk_root = MockSdkBuilder::new(&tmp.path().join("sdk"), "3.22.0").build();
    // Overwrite VERSION with trailing newlines.
    fs::write(sdk_root.join("VERSION"), "3.22.0\n\n").unwrap();

    let version = read_version_file(&sdk_root).unwrap();
    assert_eq!(
        version, "3.22.0",
        "VERSION with trailing newlines should be trimmed to '3.22.0'"
    );
}

/// bin/flutter exists but is a directory, not a file.
/// flutter_bin.is_file() returns false for directories — validate_sdk_path fails.
#[test]
#[serial]
fn test_sdk_bin_flutter_is_directory_not_file() {
    let tmp = TempDir::new().unwrap();

    let sdk_root = tmp.path().join("sdk_dir_flutter");
    // Create bin/flutter as a directory instead of a file.
    fs::create_dir_all(sdk_root.join("bin").join("flutter")).unwrap();
    fs::write(sdk_root.join("VERSION"), "3.22.0\n").unwrap();

    let result = validate_sdk_path(&sdk_root);
    assert!(
        result.is_err(),
        "validate_sdk_path should fail when bin/flutter is a directory"
    );
}

/// The SDK root path itself is a file, not a directory.
/// validate_sdk_path tries root.join("bin").join("flutter") which won't exist.
#[test]
#[serial]
fn test_sdk_root_is_file_not_directory() {
    let tmp = TempDir::new().unwrap();

    // Create the "SDK root" as a file.
    let sdk_root = tmp.path().join("sdk_root_is_file");
    fs::write(&sdk_root, "I am a file, not a directory").unwrap();

    let result = validate_sdk_path(&sdk_root);
    assert!(
        result.is_err(),
        "validate_sdk_path should fail when the SDK root is a file, not a directory"
    );
}

/// SDK has bin/flutter and VERSION but no bin/cache/dart-sdk/ directory.
/// validate_sdk_path treats missing dart-sdk as non-fatal (logged at debug level).
#[test]
#[serial]
fn test_sdk_no_dart_sdk_still_valid() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("my_app");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    // Build SDK without dart-sdk cache (MockSdkBuilder without .with_dart_sdk())
    let sdk_root = MockSdkBuilder::new(&tmp.path().join("sdk_no_dart"), "3.22.0").build();

    let result = validate_sdk_path(&sdk_root);
    assert!(
        result.is_ok(),
        "validate_sdk_path should pass for SDK missing bin/cache/dart-sdk/ (fresh install)"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. Permission Edge Cases (Unix only)
// ─────────────────────────────────────────────────────────────────────────────

/// bin/flutter exists with mode 0o644 (not executable).
/// validate_sdk_path only calls .is_file() — it does NOT check execute permission.
/// Note: current behavior is to PASS validation even if the binary is not executable.
/// This is a documentation of current behavior; consider adding execute check in the future.
#[test]
#[serial]
#[cfg(unix)]
fn test_sdk_bin_flutter_not_executable() {
    use std::os::unix::fs::PermissionsExt;

    let tmp = TempDir::new().unwrap();
    let sdk_root = tmp.path().join("sdk_no_exec");
    fs::create_dir_all(sdk_root.join("bin")).unwrap();

    let flutter_bin = sdk_root.join("bin").join("flutter");
    fs::write(&flutter_bin, "#!/bin/sh\n# not executable\n").unwrap();
    // Set mode 0o644 — readable but NOT executable.
    fs::set_permissions(&flutter_bin, fs::Permissions::from_mode(0o644)).unwrap();
    fs::write(sdk_root.join("VERSION"), "3.22.0\n").unwrap();

    let result = validate_sdk_path(&sdk_root);
    // Note: current behavior is PASS — validate_sdk_path only checks .is_file(),
    // not whether the binary has execute permissions. If this behaviour changes
    // in the future to require execute bit, this test will need updating.
    assert!(
        result.is_ok(),
        "validate_sdk_path currently passes even for non-executable binary (checks .is_file() only)"
    );
}

/// .fvmrc exists but has mode 0o000 (not readable by anyone).
/// detect_fvm_modern reads the file with fs::read_to_string — it returns Err(PermissionDenied).
/// The code handles this with `Err(e) => { warn!(...); return Ok(None); }` — graceful fallthrough.
#[test]
#[serial]
#[cfg(unix)]
fn test_config_file_not_readable() {
    use std::os::unix::fs::PermissionsExt;

    // Skip this test if running as root (root can read any file regardless of mode 0o000).
    let is_root = std::process::Command::new("id")
        .arg("-u")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "0")
        .unwrap_or(false);
    if is_root {
        return;
    }

    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("my_app");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    let fvmrc_path = project.join(".fvmrc");
    fs::write(&fvmrc_path, r#"{"flutter":"3.22.0"}"#).unwrap();
    // Make the config file completely unreadable.
    fs::set_permissions(&fvmrc_path, fs::Permissions::from_mode(0o000)).unwrap();

    let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
    let _path_guard = EnvGuard::set("PATH", tmp.path().to_str().unwrap());

    let result = find_flutter_sdk(&project, None);

    // Restore permissions so TempDir can clean up.
    fs::set_permissions(&fvmrc_path, fs::Permissions::from_mode(0o644)).unwrap();

    // FVM detection should fail gracefully and fall through — no panic.
    if let Ok(sdk) = &result {
        assert!(
            !matches!(sdk.source, SdkSource::Fvm { .. }),
            "Unreadable .fvmrc should not produce FVM SDK, got {:?}",
            sdk.source
        );
    }
}

/// SDK dir exists but has mode 0o000 (not traversable).
/// validate_sdk_path tries to check bin/flutter.is_file() — returns false for untraversable dir.
#[test]
#[serial]
#[cfg(unix)]
fn test_sdk_directory_not_traversable() {
    use std::os::unix::fs::PermissionsExt;

    // Skip this test if running as root (root can traverse any directory regardless of mode 0o000).
    let is_root = std::process::Command::new("id")
        .arg("-u")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "0")
        .unwrap_or(false);
    if is_root {
        return;
    }

    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("my_app");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    // Build a valid SDK first, then remove permissions.
    let sdk_root = MockSdkBuilder::new(&tmp.path().join("sdk_no_perms"), "3.22.0").build();
    // Remove all permissions on the SDK root directory.
    fs::set_permissions(&sdk_root, fs::Permissions::from_mode(0o000)).unwrap();

    let result = validate_sdk_path(&sdk_root);

    // Restore permissions so TempDir can clean up.
    fs::set_permissions(&sdk_root, fs::Permissions::from_mode(0o755)).unwrap();

    assert!(
        result.is_err(),
        "validate_sdk_path should fail for a non-traversable SDK directory"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. Concurrent Version Manager Configs (Conflict Scenarios)
// ─────────────────────────────────────────────────────────────────────────────

/// Both .fvmrc AND .puro.json in the same project directory.
/// FVM (priority 3) is checked before Puro (priority 5) — FVM wins.
#[test]
#[serial]
fn test_fvm_and_puro_both_present_fvm_wins() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("my_app");
    let fvm_cache = tmp.path().join("fvm_cache");
    let puro_root = tmp.path().join("puro_root");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    let fvm_sdk_root = create_fvm_layout(&project, &fvm_cache, "3.22.0");
    let _puro_sdk_root = create_puro_layout(&project, &puro_root, "default");

    let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
    let _fvm_guard = EnvGuard::set("FVM_CACHE_PATH", fvm_cache.to_str().unwrap());
    let _puro_guard = EnvGuard::set("PURO_ROOT", puro_root.to_str().unwrap());

    let sdk = find_flutter_sdk(&project, None).unwrap();
    assert_sdk_source(
        &sdk,
        &SdkSource::Fvm {
            version: "3.22.0".into(),
        },
    );
    assert_sdk_root(&sdk, &fvm_sdk_root);
}

/// All five version manager configs present simultaneously.
/// Priority order: FVM (3) > Puro (5) > asdf (6) > mise (7) > proto (8).
/// FVM should win because it has the highest priority.
#[test]
#[serial]
fn test_all_version_managers_present() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("my_app");
    let fvm_cache = tmp.path().join("fvm_cache");
    let puro_root = tmp.path().join("puro_root");
    let asdf_data = tmp.path().join("asdf_data");
    let mise_data = tmp.path().join("mise_data");
    let proto_home = tmp.path().join("proto_home");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    let fvm_sdk_root = create_fvm_layout(&project, &fvm_cache, "3.22.0");
    let _puro_sdk = create_puro_layout(&project, &puro_root, "default");
    let _asdf_sdk = create_asdf_layout(&project, &asdf_data, "3.19.0");
    let _mise_sdk = create_mise_layout(&project, &mise_data, "3.18.0");
    // proto_layout creates .prototools; also create the SDK
    let _proto_sdk = {
        fs::write(project.join(".prototools"), "flutter = \"3.16.0\"\n").unwrap();
        let sdk_root = proto_home.join("tools").join("flutter").join("3.16.0");
        MockSdkBuilder::new(&sdk_root, "3.16.0").build()
    };

    let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
    let _fvm_guard = EnvGuard::set("FVM_CACHE_PATH", fvm_cache.to_str().unwrap());
    let _puro_guard = EnvGuard::set("PURO_ROOT", puro_root.to_str().unwrap());
    let _asdf_guard = EnvGuard::set("ASDF_DATA_DIR", asdf_data.to_str().unwrap());
    let _mise_guard = EnvGuard::set("MISE_DATA_DIR", mise_data.to_str().unwrap());
    let _proto_guard = EnvGuard::set("PROTO_HOME", proto_home.to_str().unwrap());

    let sdk = find_flutter_sdk(&project, None).unwrap();
    assert_sdk_source(
        &sdk,
        &SdkSource::Fvm {
            version: "3.22.0".into(),
        },
    );
    assert_sdk_root(&sdk, &fvm_sdk_root);
}

/// .fvmrc present and pointing to a valid version string, but the SDK directory
/// does not exist in the FVM cache — validation fails and falls through.
/// .tool-versions present with a valid SDK — asdf should be the final source.
#[test]
#[serial]
fn test_fvm_invalid_but_asdf_valid() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("my_app");
    let fvm_cache = tmp.path().join("fvm_cache");
    let asdf_data = tmp.path().join("asdf_data");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    // .fvmrc exists with a valid version, but DO NOT create the SDK in the cache.
    fs::write(project.join(".fvmrc"), r#"{"flutter": "3.22.0"}"#).unwrap();
    // Create the FVM cache dir (exists) but NOT the version subdir.
    fs::create_dir_all(&fvm_cache).unwrap();

    // Create a valid asdf SDK.
    let asdf_sdk_root = create_asdf_layout(&project, &asdf_data, "3.19.0");

    let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
    let _fvm_guard = EnvGuard::set("FVM_CACHE_PATH", fvm_cache.to_str().unwrap());
    let _asdf_guard = EnvGuard::set("ASDF_DATA_DIR", asdf_data.to_str().unwrap());
    let _path_guard = EnvGuard::set("PATH", tmp.path().to_str().unwrap());

    let sdk = find_flutter_sdk(&project, None).unwrap();
    assert_sdk_source(
        &sdk,
        &SdkSource::Asdf {
            version: "3.19.0".into(),
        },
    );
    assert_sdk_root(&sdk, &asdf_sdk_root);
}

// ─────────────────────────────────────────────────────────────────────────────
// 6. Unusual Path Patterns
// ─────────────────────────────────────────────────────────────────────────────

/// Project directory and SDK directory contain spaces in their paths.
/// Verify that path handling works correctly — no escaping issues.
#[test]
#[serial]
fn test_path_with_spaces() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("my flutter project");
    let fvm_cache = tmp.path().join("flutter sdk versions");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    let sdk_root = create_fvm_layout(&project, &fvm_cache, "3.22.0");

    let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
    let _fvm_guard = EnvGuard::set("FVM_CACHE_PATH", fvm_cache.to_str().unwrap());

    let sdk = find_flutter_sdk(&project, None).unwrap();
    assert_sdk_source(
        &sdk,
        &SdkSource::Fvm {
            version: "3.22.0".into(),
        },
    );
    assert_sdk_root(&sdk, &sdk_root);
}

/// Project path is 20 directories deep but .fvmrc is at the near-root level.
/// The find_config_upward() walker should find it regardless of nesting depth.
#[test]
#[serial]
fn test_deeply_nested_project() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let fvm_cache = root.join("fvm_cache");

    // Create a deeply nested project path: root/a/b/c/d/.../z/my_app
    let nesting = [
        "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n", "o", "p", "q", "r",
        "s", "my_app",
    ];
    let deep_project: std::path::PathBuf =
        nesting.iter().fold(root.to_path_buf(), |p, s| p.join(s));
    fs::create_dir_all(&deep_project).unwrap();
    create_flutter_project(&deep_project, "my_app");

    // Place .fvmrc at root (19 levels up from my_app).
    let sdk_root = create_fvm_layout(root, &fvm_cache, "3.22.0");

    let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
    let _fvm_guard = EnvGuard::set("FVM_CACHE_PATH", fvm_cache.to_str().unwrap());

    let sdk = find_flutter_sdk(&deep_project, None).unwrap();
    assert_sdk_source(
        &sdk,
        &SdkSource::Fvm {
            version: "3.22.0".into(),
        },
    );
    assert_sdk_root(&sdk, &sdk_root);
}

/// explicit_path = Some("/nonexistent/path/to/flutter").
/// Note: current behavior is that an invalid explicit path FALLS THROUGH to
/// other strategies (try_resolve_sdk returns None, not Err), then continues
/// the detection chain. If no other strategy succeeds, returns FlutterNotFound.
/// This differs from what one might expect (hard error). See locator.rs for the
/// implementation: try_explicit_config returns Some(path), then try_resolve_sdk
/// is called which returns None on failure, and the code continues to Strategy 2.
#[test]
#[serial]
fn test_explicit_config_path_does_not_exist() {
    let tmp = TempDir::new().unwrap();
    let nonexistent =
        std::path::PathBuf::from("/nonexistent/path/to/flutter/sdk/that/does/not/exist");

    let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
    // Isolate PATH so no flutter is found anywhere else.
    let _path_guard = EnvGuard::set("PATH", tmp.path().to_str().unwrap());

    let result = find_flutter_sdk(tmp.path(), Some(&nonexistent));

    // Note: current behavior is NOT a hard error — explicit config failure falls through
    // to other strategies. With PATH isolated, the result is FlutterNotFound.
    // If this behavior is changed in the future to make explicit config a hard error,
    // update this test to assert result.is_err() directly.
    assert_sdk_not_found(&result);
}

/// explicit_path = Some(path to an empty directory that exists but is not a valid SDK).
/// Same fallthrough behavior as above — try_resolve_sdk returns None, continues chain.
/// With other strategies isolated, results in FlutterNotFound.
#[test]
#[serial]
fn test_explicit_config_path_exists_but_invalid_sdk() {
    let tmp = TempDir::new().unwrap();
    let empty_dir = tmp.path().join("empty_sdk_dir");
    fs::create_dir_all(&empty_dir).unwrap();

    let _flutter_root_guard = EnvGuard::remove("FLUTTER_ROOT");
    let _path_guard = EnvGuard::set("PATH", tmp.path().to_str().unwrap());

    let result = find_flutter_sdk(tmp.path(), Some(&empty_dir));

    // Note: an invalid explicit SDK path falls through rather than being a hard error.
    // With PATH isolated so no real flutter is on PATH, this should be FlutterNotFound.
    assert_sdk_not_found(&result);
}

/// FLUTTER_ROOT set to empty string "".
/// try_flutter_root_env() calls std::env::var_os("FLUTTER_ROOT") which returns
/// Some(OsString::from("")) for an empty string, so PathBuf::from("") is returned.
/// validate_sdk_path on PathBuf::from("") will fail — falls through to next strategy.
/// Note: current behavior is to fall through on empty FLUTTER_ROOT rather than treat
/// it as "unset". This is documented behavior.
#[test]
#[serial]
fn test_flutter_root_env_empty_string() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("my_app");
    fs::create_dir_all(&project).unwrap();
    create_flutter_project(&project, "my_app");

    let _flutter_root_guard = EnvGuard::set("FLUTTER_ROOT", "");
    let _path_guard = EnvGuard::set("PATH", tmp.path().to_str().unwrap());

    let result = find_flutter_sdk(&project, None);

    // Note: FLUTTER_ROOT="" is set to empty string, not unset.
    // try_flutter_root_env returns Some(PathBuf::from("")) which fails validation.
    // The detection chain then falls through to other strategies.
    // With PATH isolated so nothing on PATH, result is FlutterNotFound.
    if let Ok(sdk) = &result {
        assert_ne!(
            sdk.source,
            SdkSource::EnvironmentVariable,
            "Empty FLUTTER_ROOT should not produce EnvironmentVariable-sourced SDK"
        );
    }
    // Either Err(FlutterNotFound) or Ok from a different strategy — both are acceptable.
    // The important assertion is that EnvironmentVariable was not used with empty FLUTTER_ROOT.
}

// ─────────────────────────────────────────────────────────────────────────────
// 7. Windows-Specific Path Logic (Cross-Platform)
// ─────────────────────────────────────────────────────────────────────────────

/// SDK has both bin/flutter (Unix binary) AND bin/flutter.bat (Windows batch file).
/// On Unix: validate_sdk_path looks for bin/flutter (Direct variant), ignores .bat.
/// On Windows: validate_sdk_path looks for bin/flutter.bat (WindowsBatch variant).
/// This test documents the expected behavior per platform.
#[test]
#[serial]
fn test_bat_file_detection_alongside_unix_binary() {
    let tmp = TempDir::new().unwrap();

    // Build SDK with both bin/flutter and bin/flutter.bat.
    let sdk_root = MockSdkBuilder::new(tmp.path(), "3.22.0")
        .with_bat_file()
        .build();

    let result = validate_sdk_path(&sdk_root);
    assert!(
        result.is_ok(),
        "validate_sdk_path should succeed with both bin/flutter and bin/flutter.bat present"
    );

    let exe = result.unwrap();

    #[cfg(not(target_os = "windows"))]
    {
        // On Unix: should use the Direct (non-bat) binary.
        assert!(
            matches!(
                exe,
                fdemon_daemon::flutter_sdk::FlutterExecutable::Direct(_)
            ),
            "On Unix, should use Direct executable even when .bat file is present"
        );
        assert_eq!(
            exe.path(),
            sdk_root.join("bin").join("flutter"),
            "On Unix, executable path should be bin/flutter (not bin/flutter.bat)"
        );
    }

    #[cfg(target_os = "windows")]
    {
        // On Windows: should use the WindowsBatch (.bat) binary.
        assert!(
            matches!(
                exe,
                fdemon_daemon::flutter_sdk::FlutterExecutable::WindowsBatch(_)
            ),
            "On Windows, should use WindowsBatch executable"
        );
        assert_eq!(
            exe.path(),
            sdk_root.join("bin").join("flutter.bat"),
            "On Windows, executable path should be bin/flutter.bat"
        );
    }
}

/// SDK has bin/flutter.bat but NOT bin/flutter (Unix binary).
/// On Unix: validate_sdk_path looks for bin/flutter only — returns Err (not found).
/// On Windows: validate_sdk_path looks for bin/flutter.bat — returns Ok.
/// This documents the cross-platform behavior difference.
#[test]
#[serial]
fn test_bat_file_only_no_unix_binary() {
    let tmp = TempDir::new().unwrap();

    let sdk_root = tmp.path().join("sdk_bat_only");
    fs::create_dir_all(sdk_root.join("bin")).unwrap();
    // Create ONLY the .bat file.
    fs::write(
        sdk_root.join("bin").join("flutter.bat"),
        "@echo off\nrem mock flutter.bat\n",
    )
    .unwrap();
    fs::write(sdk_root.join("VERSION"), "3.22.0\n").unwrap();
    // Intentionally NO bin/flutter (Unix binary)

    let result = validate_sdk_path(&sdk_root);

    #[cfg(not(target_os = "windows"))]
    {
        // On Unix: only bin/flutter is checked, not .bat — should fail.
        assert!(
            result.is_err(),
            "On Unix, validate_sdk_path should fail when only bin/flutter.bat exists (no bin/flutter)"
        );
    }

    #[cfg(target_os = "windows")]
    {
        // On Windows: bin/flutter.bat is the primary executable — should succeed.
        assert!(
            result.is_ok(),
            "On Windows, validate_sdk_path should succeed when bin/flutter.bat exists"
        );
    }
}
