//! # Flutter SDK Locator
//!
//! Top-level detection chain that resolves the Flutter SDK for a given project.
//! Walks 11 strategies in strict priority order, returning the first valid SDK found.
//!
//! ## Priority Order
//!
//! 1. Explicit config path (`[flutter] sdk_path` in config.toml)
//! 2. `FLUTTER_ROOT` environment variable
//! 3. FVM modern (`.fvmrc`)
//! 4. FVM legacy (`.fvm/fvm_config.json` + symlink)
//! 5. Puro (`.puro.json`)
//! 6. asdf (`.tool-versions`)
//! 7. mise (`.mise.toml`)
//! 8. proto (`.prototools`)
//! 9. flutter_wrapper (`flutterw` + `.flutter/`)
//! 10. System PATH (`which flutter` → resolve symlinks → SDK root)
//! 11. Lenient PATH fallback (binary on PATH but VERSION file missing/unreadable)

use std::{
    fs,
    path::{Path, PathBuf},
};

use fdemon_core::prelude::*;

use super::{
    channel::detect_channel,
    types::{
        read_version_file, validate_sdk_path, validate_sdk_path_lenient, FlutterSdk, SdkSource,
    },
    version_managers,
};

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
pub fn find_flutter_sdk(project_path: &Path, explicit_path: Option<&Path>) -> Result<FlutterSdk> {
    // Strategy 1: Explicit config
    if let Some(sdk_root) = try_explicit_config(explicit_path) {
        if let Some(sdk) =
            try_resolve_sdk(sdk_root, |_| SdkSource::ExplicitConfig, "explicit config")
        {
            return Ok(sdk);
        }
    } else {
        debug!("SDK detection: explicit config — no path provided");
    }

    // Strategy 2: FLUTTER_ROOT environment variable
    if let Some(sdk_root) = try_flutter_root_env() {
        if let Some(sdk) =
            try_resolve_sdk(sdk_root, |_| SdkSource::EnvironmentVariable, "FLUTTER_ROOT")
        {
            return Ok(sdk);
        }
    } else {
        debug!("SDK detection: FLUTTER_ROOT — env var not set");
    }

    // Strategy 3: FVM modern (.fvmrc)
    match version_managers::detect_fvm_modern(project_path) {
        Ok(Some(sdk_root)) => {
            if let Some(sdk) = try_resolve_sdk(
                sdk_root,
                |v| SdkSource::Fvm {
                    version: v.to_string(),
                },
                "FVM modern",
            ) {
                return Ok(sdk);
            }
        }
        Ok(None) => debug!("SDK detection: FVM modern — no .fvmrc found"),
        Err(e) => debug!("SDK detection: FVM modern — error: {e}"),
    }

    // Strategy 4: FVM legacy (.fvm/)
    match version_managers::detect_fvm_legacy(project_path) {
        Ok(Some(sdk_root)) => {
            if let Some(sdk) = try_resolve_sdk(
                sdk_root,
                |v| SdkSource::Fvm {
                    version: v.to_string(),
                },
                "FVM legacy",
            ) {
                return Ok(sdk);
            }
        }
        Ok(None) => debug!("SDK detection: FVM legacy — no .fvm/ found"),
        Err(e) => debug!("SDK detection: FVM legacy — error: {e}"),
    }

    // Strategy 5: Puro (.puro.json)
    match version_managers::detect_puro(project_path) {
        Ok(Some(sdk_root)) => {
            // Puro SDK path: <puro_root>/envs/<env>/flutter
            // Extract the env name from the path: grandparent component
            let env = sdk_root
                .parent() // flutter/
                .and_then(|p| p.file_name()) // <env>
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "default".to_string());
            if let Some(sdk) =
                try_resolve_sdk(sdk_root, |_| SdkSource::Puro { env: env.clone() }, "Puro")
            {
                return Ok(sdk);
            }
        }
        Ok(None) => debug!("SDK detection: Puro — no .puro.json found"),
        Err(e) => debug!("SDK detection: Puro — error: {e}"),
    }

    // Strategy 6: asdf (.tool-versions)
    match version_managers::detect_asdf(project_path) {
        Ok(Some(sdk_root)) => {
            if let Some(sdk) = try_resolve_sdk(
                sdk_root,
                |v| SdkSource::Asdf {
                    version: v.to_string(),
                },
                "asdf",
            ) {
                return Ok(sdk);
            }
        }
        Ok(None) => debug!("SDK detection: asdf — no .tool-versions found"),
        Err(e) => debug!("SDK detection: asdf — error: {e}"),
    }

    // Strategy 7: mise (.mise.toml)
    match version_managers::detect_mise(project_path) {
        Ok(Some(sdk_root)) => {
            if let Some(sdk) = try_resolve_sdk(
                sdk_root,
                |v| SdkSource::Mise {
                    version: v.to_string(),
                },
                "mise",
            ) {
                return Ok(sdk);
            }
        }
        Ok(None) => debug!("SDK detection: mise — no .mise.toml found"),
        Err(e) => debug!("SDK detection: mise — error: {e}"),
    }

    // Strategy 8: proto (.prototools)
    match version_managers::detect_proto(project_path) {
        Ok(Some(sdk_root)) => {
            if let Some(sdk) = try_resolve_sdk(
                sdk_root,
                |v| SdkSource::Proto {
                    version: v.to_string(),
                },
                "proto",
            ) {
                return Ok(sdk);
            }
        }
        Ok(None) => debug!("SDK detection: proto — no .prototools found"),
        Err(e) => debug!("SDK detection: proto — error: {e}"),
    }

    // Strategy 9: flutter_wrapper (flutterw + .flutter/)
    match version_managers::detect_flutter_wrapper(project_path) {
        Ok(Some(sdk_root)) => {
            if let Some(sdk) =
                try_resolve_sdk(sdk_root, |_| SdkSource::FlutterWrapper, "flutter_wrapper")
            {
                return Ok(sdk);
            }
        }
        Ok(None) => debug!("SDK detection: flutter_wrapper — flutterw or .flutter/ not found"),
        Err(e) => debug!("SDK detection: flutter_wrapper — error: {e}"),
    }

    // Strategy 10: System PATH
    if let Some(sdk_root) = try_system_path() {
        if let Some(sdk) = try_resolve_sdk(sdk_root, |_| SdkSource::SystemPath, "system PATH") {
            return Ok(sdk);
        }
    } else {
        debug!("SDK detection: system PATH — flutter not found on PATH");
    }

    // Strategy 11: Lenient PATH fallback — binary on PATH but VERSION file missing/unreadable.
    // Re-scans PATH using the same logic as strategy 10 but skips the VERSION file requirement.
    // Uses SdkSource::PathInferred to distinguish from a fully resolved SdkSource::SystemPath.
    if let Some(sdk_root) = try_system_path() {
        match validate_sdk_path_lenient(&sdk_root) {
            Ok(executable) => {
                let version =
                    read_version_file(&sdk_root).unwrap_or_else(|_| "unknown".to_string());
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

    warn!("SDK detection: all strategies exhausted, Flutter SDK not found");
    Err(Error::FlutterNotFound)
}

// ─────────────────────────────────────────────────────────────────────────────
// Core Helper
// ─────────────────────────────────────────────────────────────────────────────

/// Validate a candidate SDK root and build a [`FlutterSdk`] if valid.
///
/// Returns `Some(sdk)` on success, `None` if the candidate is invalid or the
/// VERSION file is unreadable — both cases are logged at `debug!` level and
/// fall through to the next strategy. Never returns an error.
///
/// # Arguments
/// * `sdk_root` — Candidate SDK directory to validate
/// * `make_source` — Closure that receives the version string and returns the [`SdkSource`]
/// * `label` — Human-readable strategy name used in log messages
fn try_resolve_sdk(
    sdk_root: PathBuf,
    make_source: impl FnOnce(&str) -> SdkSource,
    label: &str,
) -> Option<FlutterSdk> {
    debug!(
        path = %sdk_root.display(),
        "SDK detection: {label} found candidate"
    );
    match validate_sdk_path(&sdk_root) {
        Ok(executable) => {
            let version = match read_version_file(&sdk_root) {
                Ok(v) => v,
                Err(e) => {
                    debug!("SDK detection: {label} — VERSION file unreadable: {e}");
                    return None;
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

// ─────────────────────────────────────────────────────────────────────────────
// Strategy Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Strategy 1: Return the explicitly configured SDK path if provided.
fn try_explicit_config(explicit_path: Option<&Path>) -> Option<PathBuf> {
    explicit_path.map(|p| p.to_path_buf())
}

/// Strategy 2: Return `$FLUTTER_ROOT` as the SDK path if the env var is set.
fn try_flutter_root_env() -> Option<PathBuf> {
    std::env::var_os("FLUTTER_ROOT").map(PathBuf::from)
}

/// Strategy 10: Find `flutter` on the system PATH and resolve to the SDK root.
///
/// On Unix: searches PATH for `flutter`, resolves symlinks, then walks up
/// from the binary to find the SDK root (`<root>/bin/flutter` → `<root>/`).
///
/// On Windows: searches PATH for `flutter.bat` or `flutter.exe`.
fn try_system_path() -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;

    for dir in std::env::split_paths(&path_var) {
        if let Some(sdk_root) = find_flutter_in_dir(&dir) {
            return Some(sdk_root);
        }
    }

    None
}

/// Check if a directory contains the flutter binary and return the SDK root.
fn find_flutter_in_dir(dir: &Path) -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        // Try flutter.bat first, then flutter.exe
        for name in &["flutter.bat", "flutter.exe"] {
            let candidate = dir.join(name);
            if candidate.is_file() {
                if let Some(sdk_root) = resolve_sdk_root_from_binary(&candidate) {
                    return Some(sdk_root);
                }
            }
        }
        None
    }

    #[cfg(not(target_os = "windows"))]
    {
        let candidate = dir.join("flutter");
        if candidate.is_file() {
            return resolve_sdk_root_from_binary(&candidate);
        }
        None
    }
}

/// Given a path to a flutter binary, resolve the SDK root directory.
///
/// Expects the binary to be at `<root>/bin/flutter`.
/// Canonicalizes the path to follow symlinks, then walks up two levels.
pub(crate) fn resolve_sdk_root_from_binary(binary_path: &Path) -> Option<PathBuf> {
    // canonicalize → parent (bin/) → parent (root/)
    let canonical = fs::canonicalize(binary_path).ok()?;
    canonical.parent()?.parent().map(|p| p.to_path_buf())
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::fs;
    use tempfile::TempDir;

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

        // Bad explicit path should fall through to other strategies.
        // On a machine with flutter on PATH, the system PATH strategy may succeed.
        let result = find_flutter_sdk(tmp.path(), Some(&bad_path));
        match &result {
            Ok(sdk) => {
                // Fell through to another strategy — explicit path was skipped
                assert_ne!(sdk.source, SdkSource::ExplicitConfig);
            }
            Err(_) => {
                // No flutter found by any strategy — all strategies failed
            }
        }
    }

    #[test]
    #[serial]
    fn test_all_strategies_fail_returns_flutter_not_found() {
        let tmp = TempDir::new().unwrap();
        // Isolate PATH so no flutter binary can be found
        let original_path = std::env::var_os("PATH");
        std::env::set_var("PATH", tmp.path());
        std::env::remove_var("FLUTTER_ROOT");
        let result = find_flutter_sdk(tmp.path(), None);
        // Restore PATH to its original value
        match original_path {
            Some(v) => std::env::set_var("PATH", v),
            None => std::env::remove_var("PATH"),
        }
        assert!(result.is_err());
    }

    #[test]
    fn test_system_path_resolves_sdk_root() {
        let tmp = TempDir::new().unwrap();
        let sdk_root = tmp.path().join("flutter-sdk");
        create_mock_sdk(&sdk_root, "3.22.0");

        // Test the helper function directly — it resolves the SDK root from the binary path.
        // Canonicalize sdk_root as well since on macOS /var → /private/var is followed by
        // fs::canonicalize inside resolve_sdk_root_from_binary.
        let binary = sdk_root.join("bin/flutter");
        let resolved = resolve_sdk_root_from_binary(&binary);
        let expected = fs::canonicalize(&sdk_root).ok();
        assert_eq!(resolved, expected);
    }

    #[test]
    fn test_resolve_sdk_root_from_binary_not_found() {
        let tmp = TempDir::new().unwrap();
        let nonexistent = tmp.path().join("bin/flutter");
        // canonicalize will fail on a non-existent file
        let resolved = resolve_sdk_root_from_binary(&nonexistent);
        assert!(resolved.is_none());
    }

    #[test]
    fn test_try_explicit_config_some() {
        let path = PathBuf::from("/some/path");
        assert_eq!(try_explicit_config(Some(&path)), Some(path));
    }

    #[test]
    fn test_try_explicit_config_none() {
        assert_eq!(try_explicit_config(None), None);
    }

    #[test]
    #[serial]
    fn test_fvm_modern_detection() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path().join("my_app");
        fs::create_dir_all(&project).unwrap();

        // Create .fvmrc
        fs::write(project.join(".fvmrc"), r#"{"flutter":"3.19.0"}"#).unwrap();

        // Create mock SDK in FVM cache
        let cache = tmp.path().join("fvm_cache/versions/3.19.0");
        create_mock_sdk(&cache, "3.19.0");

        // Clear FLUTTER_ROOT so it doesn't interfere with priority ordering
        std::env::remove_var("FLUTTER_ROOT");
        std::env::set_var("FVM_CACHE_PATH", tmp.path().join("fvm_cache/versions"));
        let result = find_flutter_sdk(&project, None).unwrap();
        std::env::remove_var("FVM_CACHE_PATH");

        assert!(matches!(result.source, SdkSource::Fvm { .. }));
        assert_eq!(result.version, "3.19.0");
    }

    #[test]
    #[serial]
    fn test_priority_order_fvm_before_asdf() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path().join("my_app");
        fs::create_dir_all(&project).unwrap();

        // Both FVM and asdf configs present
        fs::write(project.join(".fvmrc"), r#"{"flutter":"3.19.0"}"#).unwrap();
        fs::write(project.join(".tool-versions"), "flutter 3.16.0\n").unwrap();

        // Create mock SDK for FVM only — if asdf wins it won't have a valid SDK
        let fvm_sdk = tmp.path().join("fvm_cache/versions/3.19.0");
        create_mock_sdk(&fvm_sdk, "3.19.0");

        // Clear FLUTTER_ROOT so it doesn't interfere with priority ordering
        std::env::remove_var("FLUTTER_ROOT");
        std::env::set_var("FVM_CACHE_PATH", tmp.path().join("fvm_cache/versions"));
        let result = find_flutter_sdk(&project, None).unwrap();
        std::env::remove_var("FVM_CACHE_PATH");

        // FVM should win (priority 3 vs asdf priority 6)
        assert!(matches!(result.source, SdkSource::Fvm { .. }));
    }

    #[test]
    #[serial]
    fn test_explicit_beats_flutter_root_env() {
        let tmp = TempDir::new().unwrap();

        let explicit_sdk = tmp.path().join("explicit-flutter");
        create_mock_sdk(&explicit_sdk, "3.22.0");

        let env_sdk = tmp.path().join("env-flutter");
        create_mock_sdk(&env_sdk, "3.19.0");

        std::env::set_var("FLUTTER_ROOT", &env_sdk);
        let result = find_flutter_sdk(tmp.path(), Some(&explicit_sdk)).unwrap();
        std::env::remove_var("FLUTTER_ROOT");

        assert_eq!(result.source, SdkSource::ExplicitConfig);
        assert_eq!(result.version, "3.22.0");
    }

    #[test]
    #[serial]
    fn test_flutter_root_env_beats_version_managers() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path().join("my_app");
        fs::create_dir_all(&project).unwrap();

        // Create a valid SDK for FLUTTER_ROOT
        let env_sdk = tmp.path().join("env-flutter");
        create_mock_sdk(&env_sdk, "3.22.0");

        // Also create FVM config — but FLUTTER_ROOT should win
        fs::write(project.join(".fvmrc"), r#"{"flutter":"3.19.0"}"#).unwrap();
        let fvm_sdk = tmp.path().join("fvm_cache/versions/3.19.0");
        create_mock_sdk(&fvm_sdk, "3.19.0");

        std::env::set_var("FLUTTER_ROOT", &env_sdk);
        std::env::set_var("FVM_CACHE_PATH", tmp.path().join("fvm_cache/versions"));
        let result = find_flutter_sdk(&project, None).unwrap();
        std::env::remove_var("FLUTTER_ROOT");
        std::env::remove_var("FVM_CACHE_PATH");

        assert_eq!(result.source, SdkSource::EnvironmentVariable);
        assert_eq!(result.version, "3.22.0");
    }

    #[test]
    #[serial]
    fn test_invalid_candidate_skipped_fallback_to_next() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path().join("my_app");
        fs::create_dir_all(&project).unwrap();

        // FVM modern points to a nonexistent path (invalid SDK)
        fs::write(project.join(".fvmrc"), r#"{"flutter":"3.19.0"}"#).unwrap();
        // Do NOT create the FVM SDK directory

        // But asdf points to a valid SDK
        fs::write(project.join(".tool-versions"), "flutter 3.16.0\n").unwrap();
        let asdf_sdk = tmp.path().join("asdf/installs/flutter/3.16.0");
        create_mock_sdk(&asdf_sdk, "3.16.0");

        // Clear FLUTTER_ROOT so it doesn't interfere
        std::env::remove_var("FLUTTER_ROOT");
        std::env::set_var("FVM_CACHE_PATH", tmp.path().join("fvm_versions"));
        std::env::set_var("ASDF_DATA_DIR", tmp.path().join("asdf"));
        let result = find_flutter_sdk(&project, None).unwrap();
        std::env::remove_var("FVM_CACHE_PATH");
        std::env::remove_var("ASDF_DATA_DIR");

        // FVM had invalid candidate, should fall through to asdf
        assert!(matches!(result.source, SdkSource::Asdf { .. }));
        assert_eq!(result.version, "3.16.0");
    }

    #[test]
    fn test_flutter_wrapper_detection() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path().join("my_app");
        fs::create_dir_all(&project).unwrap();

        // Create flutterw and .flutter/ directory
        fs::write(project.join("flutterw"), "#!/bin/sh\n").unwrap();
        let flutter_dir = project.join(".flutter");
        create_mock_sdk(&flutter_dir, "3.22.0");

        let result = find_flutter_sdk(&project, None).unwrap();
        assert_eq!(result.source, SdkSource::FlutterWrapper);
        assert_eq!(result.version, "3.22.0");
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn test_system_path_strategy_uses_find_flutter_in_dir() {
        let tmp = TempDir::new().unwrap();
        let sdk_root = tmp.path().join("flutter-sdk");
        create_mock_sdk(&sdk_root, "3.24.0");

        // find_flutter_in_dir looks for `flutter` binary in a dir.
        // Canonicalize sdk_root so macOS /var → /private/var symlink is resolved.
        let bin_dir = sdk_root.join("bin");
        let result = find_flutter_in_dir(&bin_dir);
        let expected = fs::canonicalize(&sdk_root).ok();
        // The binary exists; canonicalize should succeed and return the SDK root
        assert_eq!(result, expected);
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    #[serial]
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
    #[serial]
    fn test_unreadable_version_file_falls_through_to_next_strategy() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path().join("my_app");
        fs::create_dir_all(&project).unwrap();

        // Strategy 3: FVM modern — SDK structure is present but VERSION is a directory
        // (unreadable as a file), so read_version_file will fail.
        fs::write(project.join(".fvmrc"), r#"{"flutter":"3.19.0"}"#).unwrap();
        let fvm_sdk = tmp.path().join("fvm_cache/versions/3.19.0");
        fs::create_dir_all(fvm_sdk.join("bin/cache/dart-sdk")).unwrap();
        fs::write(fvm_sdk.join("bin/flutter"), "#!/bin/sh\n").unwrap();
        // Create VERSION as a directory so read_version_file fails
        fs::create_dir_all(fvm_sdk.join("VERSION")).unwrap();

        // Strategy 6: asdf — valid SDK that should be reached after FVM fails
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
        assert_eq!(result.version, "3.16.0");
    }
}
