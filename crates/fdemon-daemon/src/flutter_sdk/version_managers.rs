//! # Version Manager Config Parsers
//!
//! Detects the Flutter SDK path configured by each supported version manager
//! by parsing their config files. All detection is file-based — no CLI invocations.
//!
//! ## Supported Version Managers
//!
//! - **FVM modern** — `.fvmrc` (JSON)
//! - **FVM legacy** — `.fvm/fvm_config.json` + `.fvm/flutter_sdk` symlink
//! - **Puro** — `.puro.json`
//! - **asdf** — `.tool-versions`
//! - **mise** — `.mise.toml`
//! - **proto** — `.prototools`
//! - **flutter_wrapper** — `flutterw` + `.flutter/`
//!
//! Each function returns `Ok(None)` when the config file is not found,
//! and `Ok(None)` with a warning log when the config file is malformed.

use std::path::{Path, PathBuf};

use fdemon_core::prelude::*;

// ─────────────────────────────────────────────────────────────────────────────
// Shared Helper: Parent-Directory Tree Walk
// ─────────────────────────────────────────────────────────────────────────────

/// Walk from `start` upward to the filesystem root, looking for `filename`.
/// Returns the first matching path found, or `None`.
pub(crate) fn find_config_upward(start: &Path, filename: &str) -> Option<PathBuf> {
    let mut current = start.to_path_buf();
    loop {
        let candidate = current.join(filename);
        if candidate.exists() {
            return Some(candidate);
        }
        if !current.pop() {
            return None;
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Shared Helper: FVM Cache Path
// ─────────────────────────────────────────────────────────────────────────────

/// Resolves the FVM cache directory path.
///
/// Priority: `$FVM_CACHE_PATH` > `~/fvm/versions/`
fn resolve_fvm_cache() -> Option<PathBuf> {
    if let Ok(cache) = std::env::var("FVM_CACHE_PATH") {
        let path = PathBuf::from(cache);
        debug!(path = %path.display(), "Using FVM_CACHE_PATH env var");
        return Some(path);
    }
    let home = dirs::home_dir()?;
    Some(home.join("fvm").join("versions"))
}

// ─────────────────────────────────────────────────────────────────────────────
// FVM Modern (.fvmrc)
// ─────────────────────────────────────────────────────────────────────────────

/// Detect Flutter SDK via FVM modern config (`.fvmrc`).
///
/// Parses `.fvmrc` (JSON) for the `flutter` field (version string).
/// Resolves SDK path: `$FVM_CACHE_PATH/<version>/` or `~/fvm/versions/<version>/`.
pub fn detect_fvm_modern(project_path: &Path) -> Result<Option<PathBuf>> {
    let config_file = match find_config_upward(project_path, ".fvmrc") {
        Some(p) => p,
        None => {
            debug!(
                project = %project_path.display(),
                "FVM modern: .fvmrc not found"
            );
            return Ok(None);
        }
    };

    debug!(config = %config_file.display(), "FVM modern: found .fvmrc");

    let content = match std::fs::read_to_string(&config_file) {
        Ok(c) => c,
        Err(e) => {
            warn!(config = %config_file.display(), error = %e, "FVM modern: failed to read .fvmrc");
            return Ok(None);
        }
    };

    let json: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(e) => {
            warn!(config = %config_file.display(), error = %e, "FVM modern: .fvmrc is not valid JSON");
            return Ok(None);
        }
    };

    let version = match json.get("flutter").and_then(|v| v.as_str()) {
        Some(v) => v.to_string(),
        None => {
            warn!(config = %config_file.display(), "FVM modern: .fvmrc missing 'flutter' field");
            return Ok(None);
        }
    };

    debug!(version = %version, "FVM modern: resolved version");

    let cache = match resolve_fvm_cache() {
        Some(c) => c,
        None => {
            warn!("FVM modern: could not determine FVM cache path");
            return Ok(None);
        }
    };

    let sdk_path = cache.join(&version);
    debug!(sdk = %sdk_path.display(), "FVM modern: resolved SDK path");
    Ok(Some(sdk_path))
}

// ─────────────────────────────────────────────────────────────────────────────
// FVM Legacy (.fvm/fvm_config.json + symlink)
// ─────────────────────────────────────────────────────────────────────────────

/// Detect Flutter SDK via FVM legacy config (`.fvm/fvm_config.json` or `.fvm/flutter_sdk` symlink).
///
/// Strategy:
/// 1. Walk upward to find a `.fvm` directory.
/// 2. First attempt: resolve the `.fvm/flutter_sdk` symlink via `fs::canonicalize`.
/// 3. Fallback: parse `.fvm/fvm_config.json` for `flutterSdkVersion`, resolve via FVM cache.
pub fn detect_fvm_legacy(project_path: &Path) -> Result<Option<PathBuf>> {
    let fvm_dir = match find_config_upward(project_path, ".fvm") {
        Some(p) => p,
        None => {
            debug!(
                project = %project_path.display(),
                "FVM legacy: .fvm directory not found"
            );
            return Ok(None);
        }
    };

    debug!(fvm_dir = %fvm_dir.display(), "FVM legacy: found .fvm directory");

    // Try the symlink first
    let symlink = fvm_dir.join("flutter_sdk");
    if symlink.exists() {
        match std::fs::canonicalize(&symlink) {
            Ok(resolved) => {
                debug!(sdk = %resolved.display(), "FVM legacy: resolved flutter_sdk symlink");
                return Ok(Some(resolved));
            }
            Err(e) => {
                warn!(symlink = %symlink.display(), error = %e, "FVM legacy: failed to canonicalize flutter_sdk symlink");
            }
        }
    }

    // Fallback: parse fvm_config.json
    let config_file = fvm_dir.join("fvm_config.json");
    if !config_file.exists() {
        debug!(config = %config_file.display(), "FVM legacy: fvm_config.json not found");
        return Ok(None);
    }

    let content = match std::fs::read_to_string(&config_file) {
        Ok(c) => c,
        Err(e) => {
            warn!(config = %config_file.display(), error = %e, "FVM legacy: failed to read fvm_config.json");
            return Ok(None);
        }
    };

    let json: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(e) => {
            warn!(config = %config_file.display(), error = %e, "FVM legacy: fvm_config.json is not valid JSON");
            return Ok(None);
        }
    };

    let version = match json.get("flutterSdkVersion").and_then(|v| v.as_str()) {
        Some(v) => v.to_string(),
        None => {
            warn!(config = %config_file.display(), "FVM legacy: missing 'flutterSdkVersion' field");
            return Ok(None);
        }
    };

    debug!(version = %version, "FVM legacy: resolved version from config");

    let cache = match resolve_fvm_cache() {
        Some(c) => c,
        None => {
            warn!("FVM legacy: could not determine FVM cache path");
            return Ok(None);
        }
    };

    let sdk_path = cache.join(&version);
    debug!(sdk = %sdk_path.display(), "FVM legacy: resolved SDK path");
    Ok(Some(sdk_path))
}

// ─────────────────────────────────────────────────────────────────────────────
// Puro (.puro.json)
// ─────────────────────────────────────────────────────────────────────────────

/// Detect Flutter SDK via Puro config (`.puro.json`).
///
/// Parses `.puro.json` for the `env` field.
/// SDK at `$PURO_ROOT/envs/<env>/flutter/` or `~/.puro/envs/<env>/flutter/`.
pub fn detect_puro(project_path: &Path) -> Result<Option<PathBuf>> {
    let config_file = match find_config_upward(project_path, ".puro.json") {
        Some(p) => p,
        None => {
            debug!(
                project = %project_path.display(),
                "Puro: .puro.json not found"
            );
            return Ok(None);
        }
    };

    debug!(config = %config_file.display(), "Puro: found .puro.json");

    let content = match std::fs::read_to_string(&config_file) {
        Ok(c) => c,
        Err(e) => {
            warn!(config = %config_file.display(), error = %e, "Puro: failed to read .puro.json");
            return Ok(None);
        }
    };

    let json: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(e) => {
            warn!(config = %config_file.display(), error = %e, "Puro: .puro.json is not valid JSON");
            return Ok(None);
        }
    };

    let env_name = match json.get("env").and_then(|v| v.as_str()) {
        Some(e) => e.to_string(),
        None => {
            warn!(config = %config_file.display(), "Puro: .puro.json missing 'env' field");
            return Ok(None);
        }
    };

    debug!(env = %env_name, "Puro: resolved environment name");

    let puro_root = if let Ok(root) = std::env::var("PURO_ROOT") {
        debug!(puro_root = %root, "Puro: using PURO_ROOT env var");
        PathBuf::from(root)
    } else {
        match dirs::home_dir() {
            Some(home) => home.join(".puro"),
            None => {
                warn!("Puro: could not determine home directory");
                return Ok(None);
            }
        }
    };

    let sdk_path = puro_root.join("envs").join(&env_name).join("flutter");
    debug!(sdk = %sdk_path.display(), "Puro: resolved SDK path");
    Ok(Some(sdk_path))
}

// ─────────────────────────────────────────────────────────────────────────────
// asdf (.tool-versions)
// ─────────────────────────────────────────────────────────────────────────────

/// Detect Flutter SDK via asdf config (`.tool-versions`).
///
/// Parses `.tool-versions` (line format: `tool version`).
/// SDK at `~/.asdf/installs/flutter/<version>/`.
pub fn detect_asdf(project_path: &Path) -> Result<Option<PathBuf>> {
    let config_file = match find_config_upward(project_path, ".tool-versions") {
        Some(p) => p,
        None => {
            debug!(
                project = %project_path.display(),
                "asdf: .tool-versions not found"
            );
            return Ok(None);
        }
    };

    debug!(config = %config_file.display(), "asdf: found .tool-versions");

    let content = match std::fs::read_to_string(&config_file) {
        Ok(c) => c,
        Err(e) => {
            warn!(config = %config_file.display(), error = %e, "asdf: failed to read .tool-versions");
            return Ok(None);
        }
    };

    // Parse line-by-line: find the flutter entry.
    // Format: `flutter <version>` (optionally followed by more versions)
    // Skip comment lines (starting with #)
    let version = content
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .find_map(|line| {
            let mut parts = line.split_whitespace();
            let tool = parts.next()?;
            if tool == "flutter" {
                // Take the first version token; skip `ref:` prefixed ones
                let ver = parts.next()?;
                if ver.starts_with("ref:") {
                    // e.g. ref:stable — use the ref as-is minus the prefix
                    Some(ver.trim_start_matches("ref:").to_string())
                } else {
                    Some(ver.to_string())
                }
            } else {
                None
            }
        });

    let version = match version {
        Some(v) => v,
        None => {
            debug!(config = %config_file.display(), "asdf: no flutter entry in .tool-versions");
            return Ok(None);
        }
    };

    debug!(version = %version, "asdf: resolved version");

    let asdf_root = if let Ok(data_dir) = std::env::var("ASDF_DATA_DIR") {
        debug!(asdf_data_dir = %data_dir, "asdf: using ASDF_DATA_DIR env var");
        PathBuf::from(data_dir)
    } else {
        match dirs::home_dir() {
            Some(home) => home.join(".asdf"),
            None => {
                warn!("asdf: could not determine home directory");
                return Ok(None);
            }
        }
    };

    let sdk_path = asdf_root.join("installs").join("flutter").join(&version);
    debug!(sdk = %sdk_path.display(), "asdf: resolved SDK path");
    Ok(Some(sdk_path))
}

// ─────────────────────────────────────────────────────────────────────────────
// mise (.mise.toml)
// ─────────────────────────────────────────────────────────────────────────────

/// Detect Flutter SDK via mise config (`.mise.toml`).
///
/// Parses `.mise.toml` (TOML) `[tools]` section for `flutter` key.
/// SDK at `~/.local/share/mise/installs/flutter/<version>/`.
pub fn detect_mise(project_path: &Path) -> Result<Option<PathBuf>> {
    let config_file = match find_config_upward(project_path, ".mise.toml") {
        Some(p) => p,
        None => {
            debug!(
                project = %project_path.display(),
                "mise: .mise.toml not found"
            );
            return Ok(None);
        }
    };

    debug!(config = %config_file.display(), "mise: found .mise.toml");

    let content = match std::fs::read_to_string(&config_file) {
        Ok(c) => c,
        Err(e) => {
            warn!(config = %config_file.display(), error = %e, "mise: failed to read .mise.toml");
            return Ok(None);
        }
    };

    let table: toml::Value = match content.parse::<toml::Value>() {
        Ok(v) => v,
        Err(e) => {
            warn!(config = %config_file.display(), error = %e, "mise: .mise.toml is not valid TOML");
            return Ok(None);
        }
    };

    // Read [tools] section, flutter key. Value can be a string or array.
    let version = table
        .get("tools")
        .and_then(|t| t.get("flutter"))
        .and_then(|v| {
            if let Some(s) = v.as_str() {
                Some(s.to_string())
            } else if let Some(arr) = v.as_array() {
                // Take the first version in the array
                arr.first()
                    .and_then(|first| first.as_str())
                    .map(|s| s.to_string())
            } else {
                None
            }
        });

    let version = match version {
        Some(v) => v,
        None => {
            debug!(config = %config_file.display(), "mise: no flutter entry in [tools]");
            return Ok(None);
        }
    };

    debug!(version = %version, "mise: resolved version");

    let mise_root = if let Ok(data_dir) = std::env::var("MISE_DATA_DIR") {
        debug!(mise_data_dir = %data_dir, "mise: using MISE_DATA_DIR env var");
        PathBuf::from(data_dir)
    } else {
        match dirs::data_local_dir() {
            Some(local_data) => local_data.join("mise"),
            None => {
                warn!("mise: could not determine local data directory");
                return Ok(None);
            }
        }
    };

    let sdk_path = mise_root.join("installs").join("flutter").join(&version);
    debug!(sdk = %sdk_path.display(), "mise: resolved SDK path");
    Ok(Some(sdk_path))
}

// ─────────────────────────────────────────────────────────────────────────────
// proto (.prototools)
// ─────────────────────────────────────────────────────────────────────────────

/// Detect Flutter SDK via proto config (`.prototools`).
///
/// Parses `.prototools` (TOML) for `flutter` key.
/// SDK at `~/.proto/tools/flutter/<version>/`.
pub fn detect_proto(project_path: &Path) -> Result<Option<PathBuf>> {
    let config_file = match find_config_upward(project_path, ".prototools") {
        Some(p) => p,
        None => {
            debug!(
                project = %project_path.display(),
                "proto: .prototools not found"
            );
            return Ok(None);
        }
    };

    debug!(config = %config_file.display(), "proto: found .prototools");

    let content = match std::fs::read_to_string(&config_file) {
        Ok(c) => c,
        Err(e) => {
            warn!(config = %config_file.display(), error = %e, "proto: failed to read .prototools");
            return Ok(None);
        }
    };

    let table: toml::Value = match content.parse::<toml::Value>() {
        Ok(v) => v,
        Err(e) => {
            warn!(config = %config_file.display(), error = %e, "proto: .prototools is not valid TOML");
            return Ok(None);
        }
    };

    // Top-level `flutter` key — can be a string "3.19.0" or an inline table { version = "3.19.0" }
    let version = table.get("flutter").and_then(|v| {
        if let Some(s) = v.as_str() {
            Some(s.to_string())
        } else if let toml::Value::Table(tbl) = v {
            tbl.get("version")
                .and_then(|ver| ver.as_str())
                .map(|s| s.to_string())
        } else {
            None
        }
    });

    let version = match version {
        Some(v) => v,
        None => {
            debug!(config = %config_file.display(), "proto: no flutter key in .prototools");
            return Ok(None);
        }
    };

    debug!(version = %version, "proto: resolved version");

    let proto_root = if let Ok(home_env) = std::env::var("PROTO_HOME") {
        debug!(proto_home = %home_env, "proto: using PROTO_HOME env var");
        PathBuf::from(home_env)
    } else {
        match dirs::home_dir() {
            Some(home) => home.join(".proto"),
            None => {
                warn!("proto: could not determine home directory");
                return Ok(None);
            }
        }
    };

    let sdk_path = proto_root.join("tools").join("flutter").join(&version);
    debug!(sdk = %sdk_path.display(), "proto: resolved SDK path");
    Ok(Some(sdk_path))
}

// ─────────────────────────────────────────────────────────────────────────────
// flutter_wrapper (flutterw + .flutter/)
// ─────────────────────────────────────────────────────────────────────────────

/// Detect Flutter SDK via flutter_wrapper (`flutterw` script + `.flutter/` directory).
///
/// Checks for `flutterw` script at project root and `.flutter/` directory.
/// SDK at `<project_root>/.flutter/`.
///
/// Note: No parent-directory walk — flutter_wrapper is always at project root.
pub fn detect_flutter_wrapper(project_path: &Path) -> Result<Option<PathBuf>> {
    let flutterw = project_path.join("flutterw");
    let flutter_dir = project_path.join(".flutter");

    debug!(
        flutterw = %flutterw.display(),
        flutter_dir = %flutter_dir.display(),
        "flutter_wrapper: checking for flutterw and .flutter/"
    );

    if !flutterw.exists() {
        debug!("flutter_wrapper: flutterw script not found");
        return Ok(None);
    }

    if !flutter_dir.is_dir() {
        debug!("flutter_wrapper: .flutter/ directory not found");
        return Ok(None);
    }

    debug!(sdk = %flutter_dir.display(), "flutter_wrapper: resolved SDK path");
    Ok(Some(flutter_dir))
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

    // ── find_config_upward ────────────────────────────────────────────────────

    #[test]
    fn test_find_config_upward_at_start() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path();
        fs::write(dir.join(".fvmrc"), r#"{"flutter":"3.19.0"}"#).unwrap();

        let found = find_config_upward(dir, ".fvmrc");
        assert_eq!(found, Some(dir.join(".fvmrc")));
    }

    #[test]
    fn test_find_config_upward_in_parent() {
        let tmp = TempDir::new().unwrap();
        let parent = tmp.path();
        let child = parent.join("packages/my_app");
        fs::create_dir_all(&child).unwrap();
        fs::write(parent.join(".fvmrc"), r#"{"flutter":"3.19.0"}"#).unwrap();

        let found = find_config_upward(&child, ".fvmrc");
        assert_eq!(found, Some(parent.join(".fvmrc")));
    }

    #[test]
    fn test_find_config_upward_not_found() {
        let tmp = TempDir::new().unwrap();
        let found = find_config_upward(tmp.path(), ".nonexistent_config_file_abc123");
        assert!(found.is_none());
    }

    #[test]
    fn test_find_config_upward_deeply_nested() {
        let tmp = TempDir::new().unwrap();
        let parent = tmp.path();
        let deep = parent.join("a/b/c/d");
        fs::create_dir_all(&deep).unwrap();
        fs::write(parent.join(".prototools"), "flutter = \"3.19.0\"\n").unwrap();

        let found = find_config_upward(&deep, ".prototools");
        assert_eq!(found, Some(parent.join(".prototools")));
    }

    // ── detect_fvm_modern ─────────────────────────────────────────────────────

    #[test]
    fn test_detect_fvm_modern_not_found() {
        let tmp = TempDir::new().unwrap();
        let result = detect_fvm_modern(tmp.path()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    #[serial]
    fn test_detect_fvm_modern_valid() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path();
        fs::write(project.join(".fvmrc"), r#"{"flutter":"3.19.0"}"#).unwrap();

        // Create mock FVM cache directory
        let cache = project.join("fvm_cache/versions/3.19.0");
        fs::create_dir_all(&cache).unwrap();

        // Point FVM_CACHE_PATH to our mock cache
        std::env::set_var("FVM_CACHE_PATH", project.join("fvm_cache/versions"));
        let result = detect_fvm_modern(project).unwrap();
        std::env::remove_var("FVM_CACHE_PATH");

        assert_eq!(result, Some(project.join("fvm_cache/versions/3.19.0")));
    }

    #[test]
    #[serial]
    fn test_detect_fvm_modern_in_parent_dir() {
        let tmp = TempDir::new().unwrap();
        let parent = tmp.path();
        let child = parent.join("sub/project");
        fs::create_dir_all(&child).unwrap();
        fs::write(parent.join(".fvmrc"), r#"{"flutter":"3.22.0"}"#).unwrap();

        std::env::set_var("FVM_CACHE_PATH", parent.join("cache"));
        let result = detect_fvm_modern(&child).unwrap();
        std::env::remove_var("FVM_CACHE_PATH");

        assert_eq!(result, Some(parent.join("cache/3.22.0")));
    }

    #[test]
    fn test_detect_fvm_modern_invalid_json() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path();
        fs::write(project.join(".fvmrc"), "not json at all!!!").unwrap();

        let result = detect_fvm_modern(project).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_detect_fvm_modern_missing_flutter_field() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path();
        fs::write(project.join(".fvmrc"), r#"{"other":"value"}"#).unwrap();

        let result = detect_fvm_modern(project).unwrap();
        assert!(result.is_none());
    }

    // ── detect_fvm_legacy ─────────────────────────────────────────────────────

    #[test]
    fn test_detect_fvm_legacy_not_found() {
        let tmp = TempDir::new().unwrap();
        let result = detect_fvm_legacy(tmp.path()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    #[serial]
    fn test_detect_fvm_legacy_via_config_json() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path();
        let fvm_dir = project.join(".fvm");
        fs::create_dir_all(&fvm_dir).unwrap();
        fs::write(
            fvm_dir.join("fvm_config.json"),
            r#"{"flutterSdkVersion":"3.19.0"}"#,
        )
        .unwrap();

        std::env::set_var("FVM_CACHE_PATH", project.join("cache"));
        let result = detect_fvm_legacy(project).unwrap();
        std::env::remove_var("FVM_CACHE_PATH");

        assert_eq!(result, Some(project.join("cache/3.19.0")));
    }

    #[test]
    fn test_detect_fvm_legacy_invalid_json() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path();
        let fvm_dir = project.join(".fvm");
        fs::create_dir_all(&fvm_dir).unwrap();
        fs::write(fvm_dir.join("fvm_config.json"), "invalid json").unwrap();

        let result = detect_fvm_legacy(project).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_detect_fvm_legacy_missing_sdk_version_field() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path();
        let fvm_dir = project.join(".fvm");
        fs::create_dir_all(&fvm_dir).unwrap();
        fs::write(fvm_dir.join("fvm_config.json"), r#"{"other":"val"}"#).unwrap();

        let result = detect_fvm_legacy(project).unwrap();
        assert!(result.is_none());
    }

    // ── detect_puro ───────────────────────────────────────────────────────────

    #[test]
    fn test_detect_puro_not_found() {
        let tmp = TempDir::new().unwrap();
        let result = detect_puro(tmp.path()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    #[serial]
    fn test_detect_puro_valid() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path();
        fs::write(project.join(".puro.json"), r#"{"env":"stable"}"#).unwrap();

        std::env::set_var("PURO_ROOT", project.join("puro_root"));
        let result = detect_puro(project).unwrap();
        std::env::remove_var("PURO_ROOT");

        assert_eq!(result, Some(project.join("puro_root/envs/stable/flutter")));
    }

    #[test]
    #[serial]
    fn test_detect_puro_in_parent_dir() {
        let tmp = TempDir::new().unwrap();
        let parent = tmp.path();
        let child = parent.join("sub");
        fs::create_dir_all(&child).unwrap();
        fs::write(parent.join(".puro.json"), r#"{"env":"my-env"}"#).unwrap();

        std::env::set_var("PURO_ROOT", parent.join("puro"));
        let result = detect_puro(&child).unwrap();
        std::env::remove_var("PURO_ROOT");

        assert_eq!(result, Some(parent.join("puro/envs/my-env/flutter")));
    }

    #[test]
    fn test_detect_puro_invalid_json() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path();
        fs::write(project.join(".puro.json"), "not json").unwrap();

        let result = detect_puro(project).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_detect_puro_missing_env_field() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path();
        fs::write(project.join(".puro.json"), r#"{"other":"value"}"#).unwrap();

        let result = detect_puro(project).unwrap();
        assert!(result.is_none());
    }

    // ── detect_asdf ───────────────────────────────────────────────────────────

    #[test]
    fn test_detect_asdf_not_found() {
        let tmp = TempDir::new().unwrap();
        let result = detect_asdf(tmp.path()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_detect_asdf_parses_tool_versions() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path();
        fs::write(
            project.join(".tool-versions"),
            "flutter 3.19.0\nruby 3.2.0\n",
        )
        .unwrap();

        let result = detect_asdf(project).unwrap();
        assert!(result.is_some());
        let path = result.unwrap();
        assert!(path.to_string_lossy().contains("flutter"));
        assert!(path.to_string_lossy().contains("3.19.0"));
    }

    #[test]
    #[serial]
    fn test_detect_asdf_with_asdf_data_dir() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path();
        fs::write(project.join(".tool-versions"), "flutter 3.22.0\n").unwrap();

        std::env::set_var("ASDF_DATA_DIR", project.join("asdf_data"));
        let result = detect_asdf(project).unwrap();
        std::env::remove_var("ASDF_DATA_DIR");

        assert_eq!(
            result,
            Some(project.join("asdf_data/installs/flutter/3.22.0"))
        );
    }

    #[test]
    fn test_detect_asdf_no_flutter_entry() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path();
        fs::write(project.join(".tool-versions"), "ruby 3.2.0\nnode 20\n").unwrap();

        let result = detect_asdf(project).unwrap();
        assert!(result.is_none());
    }

    #[test]
    #[serial]
    fn test_detect_asdf_skips_comment_lines() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path();
        fs::write(
            project.join(".tool-versions"),
            "# flutter 1.0.0\nflutter 3.19.0\n",
        )
        .unwrap();

        std::env::set_var("ASDF_DATA_DIR", project.join("asdf"));
        let result = detect_asdf(project).unwrap();
        std::env::remove_var("ASDF_DATA_DIR");

        assert_eq!(result, Some(project.join("asdf/installs/flutter/3.19.0")));
    }

    #[test]
    #[serial]
    fn test_detect_asdf_in_parent_dir() {
        let tmp = TempDir::new().unwrap();
        let parent = tmp.path();
        let child = parent.join("nested/app");
        fs::create_dir_all(&child).unwrap();
        fs::write(parent.join(".tool-versions"), "flutter 3.16.0\n").unwrap();

        std::env::set_var("ASDF_DATA_DIR", parent.join("asdf"));
        let result = detect_asdf(&child).unwrap();
        std::env::remove_var("ASDF_DATA_DIR");

        assert_eq!(result, Some(parent.join("asdf/installs/flutter/3.16.0")));
    }

    // ── detect_mise ───────────────────────────────────────────────────────────

    #[test]
    fn test_detect_mise_not_found() {
        let tmp = TempDir::new().unwrap();
        let result = detect_mise(tmp.path()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_detect_mise_parses_toml() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path();
        fs::write(
            project.join(".mise.toml"),
            "[tools]\nflutter = \"3.19.0\"\n",
        )
        .unwrap();

        let result = detect_mise(project).unwrap();
        assert!(result.is_some());
        let path = result.unwrap();
        assert!(path.to_string_lossy().contains("flutter"));
        assert!(path.to_string_lossy().contains("3.19.0"));
    }

    #[test]
    #[serial]
    fn test_detect_mise_with_mise_data_dir() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path();
        fs::write(
            project.join(".mise.toml"),
            "[tools]\nflutter = \"3.22.0\"\n",
        )
        .unwrap();

        std::env::set_var("MISE_DATA_DIR", project.join("mise_data"));
        let result = detect_mise(project).unwrap();
        std::env::remove_var("MISE_DATA_DIR");

        assert_eq!(
            result,
            Some(project.join("mise_data/installs/flutter/3.22.0"))
        );
    }

    #[test]
    fn test_detect_mise_no_flutter_entry() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path();
        fs::write(project.join(".mise.toml"), "[tools]\nnode = \"20\"\n").unwrap();

        let result = detect_mise(project).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_detect_mise_invalid_toml() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path();
        fs::write(project.join(".mise.toml"), "not toml ]] [[ valid").unwrap();

        let result = detect_mise(project).unwrap();
        assert!(result.is_none());
    }

    #[test]
    #[serial]
    fn test_detect_mise_in_parent_dir() {
        let tmp = TempDir::new().unwrap();
        let parent = tmp.path();
        let child = parent.join("sub/app");
        fs::create_dir_all(&child).unwrap();
        fs::write(parent.join(".mise.toml"), "[tools]\nflutter = \"3.16.0\"\n").unwrap();

        std::env::set_var("MISE_DATA_DIR", parent.join("mise"));
        let result = detect_mise(&child).unwrap();
        std::env::remove_var("MISE_DATA_DIR");

        assert_eq!(result, Some(parent.join("mise/installs/flutter/3.16.0")));
    }

    // ── detect_proto ──────────────────────────────────────────────────────────

    #[test]
    fn test_detect_proto_not_found() {
        let tmp = TempDir::new().unwrap();
        let result = detect_proto(tmp.path()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    #[serial]
    fn test_detect_proto_valid_string() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path();
        fs::write(
            project.join(".prototools"),
            "flutter = \"3.19.0\"\nnode = \"20.0.0\"\n",
        )
        .unwrap();

        std::env::set_var("PROTO_HOME", project.join("proto_home"));
        let result = detect_proto(project).unwrap();
        std::env::remove_var("PROTO_HOME");

        assert_eq!(
            result,
            Some(project.join("proto_home/tools/flutter/3.19.0"))
        );
    }

    #[test]
    fn test_detect_proto_no_flutter_key() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path();
        fs::write(project.join(".prototools"), "node = \"20.0.0\"\n").unwrap();

        let result = detect_proto(project).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_detect_proto_invalid_toml() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path();
        fs::write(project.join(".prototools"), "[[not toml at all").unwrap();

        let result = detect_proto(project).unwrap();
        assert!(result.is_none());
    }

    #[test]
    #[serial]
    fn test_detect_proto_in_parent_dir() {
        let tmp = TempDir::new().unwrap();
        let parent = tmp.path();
        let child = parent.join("sub");
        fs::create_dir_all(&child).unwrap();
        fs::write(parent.join(".prototools"), "flutter = \"3.16.0\"\n").unwrap();

        std::env::set_var("PROTO_HOME", parent.join("proto"));
        let result = detect_proto(&child).unwrap();
        std::env::remove_var("PROTO_HOME");

        assert_eq!(result, Some(parent.join("proto/tools/flutter/3.16.0")));
    }

    // ── detect_flutter_wrapper ────────────────────────────────────────────────

    #[test]
    fn test_detect_flutter_wrapper_both_present() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path();
        fs::write(project.join("flutterw"), "#!/bin/sh").unwrap();
        fs::create_dir(project.join(".flutter")).unwrap();

        let result = detect_flutter_wrapper(project).unwrap();
        assert_eq!(result, Some(project.join(".flutter")));
    }

    #[test]
    fn test_detect_flutter_wrapper_missing_flutterw() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path();
        fs::create_dir(project.join(".flutter")).unwrap();
        // No flutterw script

        let result = detect_flutter_wrapper(project).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_detect_flutter_wrapper_missing_flutter_dir() {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path();
        fs::write(project.join("flutterw"), "#!/bin/sh").unwrap();
        // No .flutter/ directory

        let result = detect_flutter_wrapper(project).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_detect_flutter_wrapper_neither_present() {
        let tmp = TempDir::new().unwrap();
        let result = detect_flutter_wrapper(tmp.path()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_detect_flutter_wrapper_no_parent_walk() {
        // flutter_wrapper only checks at project root, not parent dirs
        let tmp = TempDir::new().unwrap();
        let parent = tmp.path();
        let child = parent.join("sub");
        fs::create_dir_all(&child).unwrap();
        // Put flutterw and .flutter/ in parent, not child
        fs::write(parent.join("flutterw"), "#!/bin/sh").unwrap();
        fs::create_dir(parent.join(".flutter")).unwrap();

        let result = detect_flutter_wrapper(&child).unwrap();
        assert!(result.is_none()); // Should NOT find parent's flutterw
    }
}
