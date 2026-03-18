//! # FVM Cache Scanner
//!
//! Scans the FVM cache directory (`~/fvm/versions/` or `$FVM_CACHE_PATH`) for
//! installed Flutter SDK versions. Used by the Flutter Version panel to display
//! the list of locally installed SDKs.

use std::path::{Path, PathBuf};

use super::channel::detect_channel;
use super::types::{read_version_file, validate_sdk_path_lenient};

/// Known FVM channel directory names.
///
/// Directories with these names are treated as Flutter release channels rather
/// than semantic version numbers.
const CHANNEL_NAMES: &[&str] = &["stable", "beta", "dev", "main", "master"];

/// Sort priority order for channel names (lower index = sorted first).
///
/// Derived from the Flutter release cadence: stable is the most stable,
/// beta is the next, dev/main/master are development channels.
const CHANNEL_SORT_ORDER: &[&str] = &["stable", "beta", "dev", "main", "master"];

/// A Flutter SDK version installed in the FVM cache.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledSdk {
    /// Version string (directory name, e.g., "3.19.0", "stable", "beta")
    pub version: String,
    /// Flutter channel if detectable (from git branch or known channel names)
    pub channel: Option<String>,
    /// Absolute path to this SDK installation
    pub path: PathBuf,
    /// Whether this version matches the currently active/resolved SDK
    pub is_active: bool,
}

/// Returns `true` if `name` is a known Flutter release channel directory name.
fn is_channel_name(name: &str) -> bool {
    CHANNEL_NAMES.contains(&name)
}

/// Resolves the FVM cache path.
///
/// Checks the `FVM_CACHE_PATH` environment variable first, then falls back to
/// `~/fvm/versions/`. Returns `None` if neither path exists as a directory.
pub fn resolve_fvm_cache_path() -> Option<PathBuf> {
    // 1. Check FVM_CACHE_PATH env var
    if let Ok(path) = std::env::var("FVM_CACHE_PATH") {
        let cache_path = PathBuf::from(path);
        if cache_path.is_dir() {
            return Some(cache_path);
        }
    }

    // 2. Fall back to ~/fvm/versions/
    let home = dirs::home_dir()?;
    let default_path = home.join("fvm").join("versions");
    if default_path.is_dir() {
        return Some(default_path);
    }

    None
}

/// Compares the canonical paths of two directories to determine if they refer
/// to the same filesystem location. Handles symlinks correctly.
///
/// Returns `false` if either path cannot be canonicalized (e.g., does not exist
/// or permission error).
fn paths_are_same(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(ca), Ok(cb)) => ca == cb,
        _ => false,
    }
}

/// Returns the sort key for a channel name: `Some(index)` for known channels
/// (lower index = sorted first), `None` for unknown names.
fn channel_sort_index(name: &str) -> Option<usize> {
    CHANNEL_SORT_ORDER.iter().position(|&ch| ch == name)
}

/// Scans a specific directory path for installed Flutter SDK versions.
///
/// Each subdirectory of `cache_path` is validated as a Flutter SDK by checking
/// that `bin/flutter` exists and that the `VERSION` file is readable. Invalid or
/// incomplete directories are silently skipped.
///
/// # Arguments
/// * `cache_path` - Path to the FVM versions directory to scan.
/// * `active_sdk_root` - Path of the currently resolved SDK, if any.
///   Used to mark the matching version as `is_active: true`.
///
/// # Returns
/// Sorted list of installed SDKs. Sort order: active first, then channels
/// (`stable > beta > dev > main > master`), then semantic versions descending,
/// then non-semver strings alphabetically.
pub fn scan_installed_versions_from_path(
    cache_path: &Path,
    active_sdk_root: Option<&Path>,
) -> Vec<InstalledSdk> {
    let entries = match std::fs::read_dir(cache_path) {
        Ok(e) => e,
        Err(err) => {
            tracing::debug!(
                "Failed to read FVM cache directory {}: {}",
                cache_path.display(),
                err
            );
            return Vec::new();
        }
    };

    let mut sdks: Vec<InstalledSdk> = Vec::new();

    for entry in entries.flatten() {
        let entry_path = entry.path();

        // Only process directories
        if !entry_path.is_dir() {
            continue;
        }

        let dir_name = match entry.file_name().into_string() {
            Ok(n) => n,
            Err(_) => {
                tracing::debug!(
                    "Skipping non-UTF-8 directory name in FVM cache: {}",
                    entry_path.display()
                );
                continue;
            }
        };

        // Validate that this directory contains a Flutter SDK (lenient: bin/flutter only)
        if validate_sdk_path_lenient(&entry_path).is_err() {
            tracing::debug!(
                "Skipping invalid SDK directory (no bin/flutter): {}",
                entry_path.display()
            );
            continue;
        }

        // Verify the VERSION file is readable — skip directories where it is absent.
        // The version string (directory name) is used for InstalledSdk::version, but
        // we require the VERSION file to confirm this is a real SDK installation.
        if read_version_file(&entry_path).is_err() {
            tracing::debug!(
                "Skipping SDK directory with no VERSION file: {}",
                entry_path.display()
            );
            continue;
        }

        // Determine channel:
        // - If the directory name IS a known channel, use it directly.
        // - Otherwise, try to detect the channel from git state.
        let channel = if is_channel_name(&dir_name) {
            Some(dir_name.clone())
        } else {
            detect_channel(&entry_path).map(|ch| ch.to_string())
        };

        // Determine if this SDK matches the active SDK root
        let is_active = active_sdk_root
            .map(|active| paths_are_same(&entry_path, active))
            .unwrap_or(false);

        sdks.push(InstalledSdk {
            version: dir_name,
            channel,
            path: entry_path,
            is_active,
        });
    }

    sort_installed_sdks(&mut sdks);
    sdks
}

/// Scans the FVM cache directory for installed Flutter SDK versions.
///
/// Checks the `FVM_CACHE_PATH` environment variable first, then falls back to
/// `~/fvm/versions/`. Each subdirectory is validated as a Flutter SDK (must
/// contain `bin/flutter` and a readable `VERSION` file).
///
/// # Arguments
/// * `active_sdk_root` - Path of the currently resolved SDK, if any.
///   Used to mark the matching version as `is_active: true`.
///
/// # Returns
/// Sorted list of installed SDKs. Sort order: active first, then channels
/// (`stable > beta > dev > main > master`), then semantic versions descending,
/// then non-semver strings alphabetically. Returns an empty `Vec` when no
/// FVM cache directory exists.
pub fn scan_installed_versions(active_sdk_root: Option<&Path>) -> Vec<InstalledSdk> {
    match resolve_fvm_cache_path() {
        Some(cache_path) => scan_installed_versions_from_path(&cache_path, active_sdk_root),
        None => {
            tracing::debug!("No FVM cache directory found; returning empty SDK list");
            Vec::new()
        }
    }
}

/// Sorts a slice of [`InstalledSdk`] in place.
///
/// Sort order:
/// 1. Active version first (`is_active = true`)
/// 2. Channel directories in priority order: `stable > beta > dev > main > master`
/// 3. Semantic versions descending (`3.22.0 > 3.19.0 > 3.16.0`)
/// 4. Non-semver strings alphabetically
fn sort_installed_sdks(sdks: &mut [InstalledSdk]) {
    sdks.sort_by(|a, b| {
        // Active version always comes first
        match (a.is_active, b.is_active) {
            (true, false) => return std::cmp::Ordering::Less,
            (false, true) => return std::cmp::Ordering::Greater,
            _ => {}
        }

        let a_channel_idx = channel_sort_index(&a.version);
        let b_channel_idx = channel_sort_index(&b.version);

        match (a_channel_idx, b_channel_idx) {
            // Both are channels — sort by channel priority index
            (Some(ai), Some(bi)) => ai.cmp(&bi),
            // Only a is a channel — a comes first
            (Some(_), None) => std::cmp::Ordering::Less,
            // Only b is a channel — b comes first
            (None, Some(_)) => std::cmp::Ordering::Greater,
            // Neither is a channel — compare as semver descending, then alphabetically
            (None, None) => compare_version_strings(&b.version, &a.version),
        }
    });
}

/// Compares two version strings semantically.
///
/// Parses `major.minor.patch` components and compares numerically. If either
/// string is not a valid semver, falls back to lexicographic ordering.
///
/// The comparison is `a` vs `b` (for ascending order). Callers that want
/// descending order should swap `a` and `b`.
fn compare_version_strings(a: &str, b: &str) -> std::cmp::Ordering {
    if let (Some(va), Some(vb)) = (parse_semver_components(a), parse_semver_components(b)) {
        // Compare major, minor, patch in order; pre-release versions sort after stable
        let cmp = va.0.cmp(&vb.0).then(va.1.cmp(&vb.1)).then(va.2.cmp(&vb.2));
        if cmp != std::cmp::Ordering::Equal {
            return cmp;
        }
        // If numeric parts are equal, compare pre-release strings:
        // no pre-release > has pre-release (stable > pre-release)
        match (&va.3, &vb.3) {
            (None, None) => std::cmp::Ordering::Equal,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (Some(_), None) => std::cmp::Ordering::Less,
            (Some(pa), Some(pb)) => pa.cmp(pb),
        }
    } else {
        // Fall back to alphabetic ordering
        a.cmp(b)
    }
}

/// Parses a version string into `(major, minor, patch, pre_release)` components.
///
/// Returns `None` if the string cannot be parsed as a three-component semver.
fn parse_semver_components(version: &str) -> Option<(u32, u32, u32, Option<String>)> {
    let (version_part, pre_release) = match version.split_once('-') {
        Some((v, pre)) => (v, Some(pre.to_string())),
        None => (version, None),
    };

    let parts: Vec<&str> = version_part.split('.').collect();
    if parts.len() < 3 {
        return None;
    }

    let major = parts[0].parse::<u32>().ok()?;
    let minor = parts[1].parse::<u32>().ok()?;
    let patch = parts[2].parse::<u32>().ok()?;

    Some((major, minor, patch, pre_release))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Helper: create a minimal valid SDK directory in the cache.
    ///
    /// Creates `<cache_dir>/<name>/bin/flutter` and `<cache_dir>/<name>/VERSION`.
    fn create_fake_sdk(cache_dir: &Path, name: &str, version: &str) -> PathBuf {
        let sdk_dir = cache_dir.join(name);
        fs::create_dir_all(sdk_dir.join("bin")).unwrap();
        fs::write(sdk_dir.join("bin/flutter"), "#!/bin/sh").unwrap();
        fs::write(sdk_dir.join("VERSION"), version).unwrap();
        sdk_dir
    }

    // ─────────────────────────────────────────────────────────────
    // scan_installed_versions_from_path basic tests
    // ─────────────────────────────────────────────────────────────

    #[test]
    fn test_empty_cache_returns_empty() {
        let tmp = TempDir::new().unwrap();
        let result = scan_installed_versions_from_path(tmp.path(), None);
        assert!(result.is_empty());
    }

    #[test]
    fn test_nonexistent_cache_returns_empty() {
        let result =
            scan_installed_versions_from_path(Path::new("/nonexistent/path/fvm/versions"), None);
        assert!(result.is_empty());
    }

    #[test]
    fn test_single_version() {
        let tmp = TempDir::new().unwrap();
        create_fake_sdk(tmp.path(), "3.19.0", "3.19.0");

        let result = scan_installed_versions_from_path(tmp.path(), None);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].version, "3.19.0");
        assert!(!result[0].is_active);
    }

    #[test]
    fn test_version_file_content_is_read() {
        let tmp = TempDir::new().unwrap();
        // VERSION file content differs from directory name (as is common for channel dirs)
        create_fake_sdk(tmp.path(), "stable", "3.19.0");

        let result = scan_installed_versions_from_path(tmp.path(), None);
        assert_eq!(result.len(), 1);
        // version field is the directory name
        assert_eq!(result[0].version, "stable");
    }

    #[test]
    fn test_multiple_versions() {
        let tmp = TempDir::new().unwrap();
        create_fake_sdk(tmp.path(), "3.19.0", "3.19.0");
        create_fake_sdk(tmp.path(), "3.22.0", "3.22.0");

        let result = scan_installed_versions_from_path(tmp.path(), None);
        assert_eq!(result.len(), 2);
    }

    // ─────────────────────────────────────────────────────────────
    // Active version detection
    // ─────────────────────────────────────────────────────────────

    #[test]
    fn test_active_version_detected() {
        let tmp = TempDir::new().unwrap();
        let sdk_path = create_fake_sdk(tmp.path(), "3.19.0", "3.19.0");

        let result = scan_installed_versions_from_path(tmp.path(), Some(&sdk_path));
        assert_eq!(result.len(), 1);
        assert!(result[0].is_active);
    }

    #[test]
    fn test_active_version_not_set_when_no_active_sdk() {
        let tmp = TempDir::new().unwrap();
        create_fake_sdk(tmp.path(), "3.19.0", "3.19.0");

        let result = scan_installed_versions_from_path(tmp.path(), None);
        assert_eq!(result.len(), 1);
        assert!(!result[0].is_active);
    }

    #[test]
    fn test_only_matching_sdk_is_active() {
        let tmp = TempDir::new().unwrap();
        let active = create_fake_sdk(tmp.path(), "3.19.0", "3.19.0");
        create_fake_sdk(tmp.path(), "3.22.0", "3.22.0");

        let result = scan_installed_versions_from_path(tmp.path(), Some(&active));
        let active_sdks: Vec<_> = result.iter().filter(|s| s.is_active).collect();
        assert_eq!(active_sdks.len(), 1);
        assert_eq!(active_sdks[0].version, "3.19.0");
    }

    // ─────────────────────────────────────────────────────────────
    // Channel detection
    // ─────────────────────────────────────────────────────────────

    #[test]
    fn test_channel_directories_recognized() {
        let tmp = TempDir::new().unwrap();
        create_fake_sdk(tmp.path(), "stable", "3.19.0");
        create_fake_sdk(tmp.path(), "beta", "3.22.0-beta");

        let result = scan_installed_versions_from_path(tmp.path(), None);
        assert_eq!(result.len(), 2);

        let stable = result.iter().find(|s| s.version == "stable").unwrap();
        let beta = result.iter().find(|s| s.version == "beta").unwrap();
        assert_eq!(stable.channel, Some("stable".to_string()));
        assert_eq!(beta.channel, Some("beta".to_string()));
    }

    #[test]
    fn test_all_known_channel_names_recognized() {
        let tmp = TempDir::new().unwrap();
        for channel in CHANNEL_NAMES {
            create_fake_sdk(tmp.path(), channel, "3.19.0");
        }

        let result = scan_installed_versions_from_path(tmp.path(), None);
        assert_eq!(result.len(), CHANNEL_NAMES.len());

        for sdk in &result {
            assert!(
                is_channel_name(&sdk.version),
                "Expected {} to be recognized as a channel",
                sdk.version
            );
            assert_eq!(sdk.channel, Some(sdk.version.clone()));
        }
    }

    #[test]
    fn test_semver_version_has_no_channel_without_git() {
        let tmp = TempDir::new().unwrap();
        // A version directory without a .git directory — channel should be None
        create_fake_sdk(tmp.path(), "3.19.0", "3.19.0");

        let result = scan_installed_versions_from_path(tmp.path(), None);
        assert_eq!(result.len(), 1);
        // No .git directory, so channel detection returns None
        assert_eq!(result[0].channel, None);
    }

    // ─────────────────────────────────────────────────────────────
    // Invalid/incomplete directory handling
    // ─────────────────────────────────────────────────────────────

    #[test]
    fn test_invalid_directories_skipped() {
        let tmp = TempDir::new().unwrap();
        create_fake_sdk(tmp.path(), "3.19.0", "3.19.0");
        // Create an invalid directory (no bin/flutter)
        fs::create_dir_all(tmp.path().join("corrupt")).unwrap();

        let result = scan_installed_versions_from_path(tmp.path(), None);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].version, "3.19.0");
    }

    #[test]
    fn test_directory_missing_version_file_skipped() {
        let tmp = TempDir::new().unwrap();
        create_fake_sdk(tmp.path(), "3.19.0", "3.19.0");
        // Create a directory with bin/flutter but no VERSION file
        let no_version = tmp.path().join("3.20.0");
        fs::create_dir_all(no_version.join("bin")).unwrap();
        fs::write(no_version.join("bin/flutter"), "#!/bin/sh").unwrap();
        // No VERSION file

        let result = scan_installed_versions_from_path(tmp.path(), None);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].version, "3.19.0");
    }

    #[test]
    fn test_empty_directory_skipped() {
        let tmp = TempDir::new().unwrap();
        create_fake_sdk(tmp.path(), "3.19.0", "3.19.0");
        // Empty directory (no bin/flutter, no VERSION)
        fs::create_dir_all(tmp.path().join("empty")).unwrap();

        let result = scan_installed_versions_from_path(tmp.path(), None);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_files_in_cache_dir_are_skipped() {
        let tmp = TempDir::new().unwrap();
        create_fake_sdk(tmp.path(), "3.19.0", "3.19.0");
        // A file (not a directory) in the cache dir — should be ignored
        fs::write(tmp.path().join("some_file.txt"), "not an sdk").unwrap();

        let result = scan_installed_versions_from_path(tmp.path(), None);
        assert_eq!(result.len(), 1);
    }

    // ─────────────────────────────────────────────────────────────
    // Sorting
    // ─────────────────────────────────────────────────────────────

    #[test]
    fn test_sort_order_active_first() {
        let tmp = TempDir::new().unwrap();
        create_fake_sdk(tmp.path(), "3.16.0", "3.16.0");
        let active = create_fake_sdk(tmp.path(), "3.19.0", "3.19.0");
        create_fake_sdk(tmp.path(), "3.22.0", "3.22.0");

        let result = scan_installed_versions_from_path(tmp.path(), Some(&active));
        assert!(result[0].is_active);
        assert_eq!(result[0].version, "3.19.0");
    }

    #[test]
    fn test_sort_order_channels_before_versions() {
        let tmp = TempDir::new().unwrap();
        create_fake_sdk(tmp.path(), "stable", "3.19.0");
        create_fake_sdk(tmp.path(), "3.22.0", "3.22.0");
        create_fake_sdk(tmp.path(), "beta", "3.22.0-beta");

        let result = scan_installed_versions_from_path(tmp.path(), None);
        // stable, beta, then 3.22.0
        assert_eq!(result[0].version, "stable");
        assert_eq!(result[1].version, "beta");
        assert_eq!(result[2].version, "3.22.0");
    }

    #[test]
    fn test_sort_order_channel_priority() {
        let tmp = TempDir::new().unwrap();
        // Create all channels in reverse priority order
        for channel in CHANNEL_NAMES.iter().rev() {
            create_fake_sdk(tmp.path(), channel, "3.19.0");
        }

        let result = scan_installed_versions_from_path(tmp.path(), None);
        assert_eq!(result.len(), CHANNEL_NAMES.len());
        // Should match CHANNEL_SORT_ORDER
        for (sdk, expected_channel) in result.iter().zip(CHANNEL_SORT_ORDER.iter()) {
            assert_eq!(&sdk.version, expected_channel);
        }
    }

    #[test]
    fn test_sort_order_versions_descending() {
        let tmp = TempDir::new().unwrap();
        create_fake_sdk(tmp.path(), "3.16.0", "3.16.0");
        create_fake_sdk(tmp.path(), "3.22.0", "3.22.0");
        create_fake_sdk(tmp.path(), "3.19.0", "3.19.0");

        let result = scan_installed_versions_from_path(tmp.path(), None);
        assert_eq!(result[0].version, "3.22.0");
        assert_eq!(result[1].version, "3.19.0");
        assert_eq!(result[2].version, "3.16.0");
    }

    #[test]
    fn test_sort_order_active_beats_channels() {
        let tmp = TempDir::new().unwrap();
        create_fake_sdk(tmp.path(), "stable", "3.19.0");
        let active = create_fake_sdk(tmp.path(), "3.22.0", "3.22.0");

        let result = scan_installed_versions_from_path(tmp.path(), Some(&active));
        assert!(result[0].is_active);
        assert_eq!(result[0].version, "3.22.0");
        assert_eq!(result[1].version, "stable");
    }

    // ─────────────────────────────────────────────────────────────
    // is_channel_name helper
    // ─────────────────────────────────────────────────────────────

    #[test]
    fn test_is_channel_name_stable() {
        assert!(is_channel_name("stable"));
    }

    #[test]
    fn test_is_channel_name_beta() {
        assert!(is_channel_name("beta"));
    }

    #[test]
    fn test_is_channel_name_dev() {
        assert!(is_channel_name("dev"));
    }

    #[test]
    fn test_is_channel_name_main() {
        assert!(is_channel_name("main"));
    }

    #[test]
    fn test_is_channel_name_master() {
        assert!(is_channel_name("master"));
    }

    #[test]
    fn test_is_channel_name_version_string_not_a_channel() {
        assert!(!is_channel_name("3.19.0"));
    }

    #[test]
    fn test_is_channel_name_empty_string_not_a_channel() {
        assert!(!is_channel_name(""));
    }

    // ─────────────────────────────────────────────────────────────
    // compare_version_strings helper
    // ─────────────────────────────────────────────────────────────

    #[test]
    fn test_compare_version_strings_ascending() {
        assert_eq!(
            compare_version_strings("3.16.0", "3.19.0"),
            std::cmp::Ordering::Less
        );
    }

    #[test]
    fn test_compare_version_strings_descending() {
        assert_eq!(
            compare_version_strings("3.22.0", "3.19.0"),
            std::cmp::Ordering::Greater
        );
    }

    #[test]
    fn test_compare_version_strings_equal() {
        assert_eq!(
            compare_version_strings("3.19.0", "3.19.0"),
            std::cmp::Ordering::Equal
        );
    }

    #[test]
    fn test_compare_version_strings_minor_version() {
        assert_eq!(
            compare_version_strings("3.19.0", "3.22.0"),
            std::cmp::Ordering::Less
        );
    }

    #[test]
    fn test_compare_version_strings_major_version() {
        assert_eq!(
            compare_version_strings("2.0.0", "3.0.0"),
            std::cmp::Ordering::Less
        );
    }

    #[test]
    fn test_compare_version_strings_stable_beats_prerelease() {
        // Stable release (no pre-release) should sort after pre-release when
        // numeric parts are equal (since compare_version_strings is ascending)
        assert_eq!(
            compare_version_strings("3.22.0", "3.22.0-beta.1"),
            std::cmp::Ordering::Greater
        );
    }

    #[test]
    fn test_compare_version_strings_non_semver_fallback() {
        // Non-semver strings fall back to alphabetical comparison
        assert_eq!(
            compare_version_strings("abc", "def"),
            std::cmp::Ordering::Less
        );
    }

    // ─────────────────────────────────────────────────────────────
    // parse_semver_components helper
    // ─────────────────────────────────────────────────────────────

    #[test]
    fn test_parse_semver_components_stable() {
        let result = parse_semver_components("3.19.0");
        assert_eq!(result, Some((3, 19, 0, None)));
    }

    #[test]
    fn test_parse_semver_components_prerelease() {
        let result = parse_semver_components("3.22.0-beta.1");
        assert_eq!(result, Some((3, 22, 0, Some("beta.1".to_string()))));
    }

    #[test]
    fn test_parse_semver_components_channel_name() {
        // Channel names are not valid semver
        assert!(parse_semver_components("stable").is_none());
    }

    #[test]
    fn test_parse_semver_components_empty() {
        assert!(parse_semver_components("").is_none());
    }

    #[test]
    fn test_parse_semver_components_partial() {
        assert!(parse_semver_components("3.19").is_none());
    }

    // ─────────────────────────────────────────────────────────────
    // scan_installed_versions (env-var path)
    // ─────────────────────────────────────────────────────────────

    #[test]
    fn test_scan_installed_versions_returns_empty_when_no_fvm() {
        // When FVM_CACHE_PATH is not set and ~/fvm/versions doesn't exist,
        // the function should return an empty Vec without panicking.
        // We test this indirectly via scan_installed_versions_from_path with a
        // nonexistent path.
        let result =
            scan_installed_versions_from_path(Path::new("/nonexistent/fvm/versions"), None);
        assert!(result.is_empty());
    }

    #[test]
    fn test_installed_sdk_fields() {
        let tmp = TempDir::new().unwrap();
        let sdk_path = create_fake_sdk(tmp.path(), "3.19.0", "3.19.0");

        let result = scan_installed_versions_from_path(tmp.path(), Some(&sdk_path));
        assert_eq!(result.len(), 1);

        let sdk = &result[0];
        assert_eq!(sdk.version, "3.19.0");
        assert_eq!(sdk.path, sdk_path);
        assert!(sdk.is_active);
    }

    #[test]
    fn test_sort_mixed_channels_and_versions() {
        let tmp = TempDir::new().unwrap();
        create_fake_sdk(tmp.path(), "3.16.0", "3.16.0");
        create_fake_sdk(tmp.path(), "main", "3.24.0");
        create_fake_sdk(tmp.path(), "stable", "3.19.0");
        create_fake_sdk(tmp.path(), "3.22.0", "3.22.0");
        create_fake_sdk(tmp.path(), "beta", "3.22.0-beta");
        create_fake_sdk(tmp.path(), "3.19.0", "3.19.0");

        let result = scan_installed_versions_from_path(tmp.path(), None);

        // Check channels come before versions
        let channel_count = result
            .iter()
            .filter(|s| is_channel_name(&s.version))
            .count();
        let version_count = result.len() - channel_count;

        // First `channel_count` items should all be channels
        for sdk in result.iter().take(channel_count) {
            assert!(
                is_channel_name(&sdk.version),
                "Expected {} to be a channel in sorted output",
                sdk.version
            );
        }

        // Remaining items should be semver versions in descending order
        let version_items: Vec<_> = result.iter().skip(channel_count).collect();
        assert_eq!(version_items.len(), version_count);
        for window in version_items.windows(2) {
            assert!(
                window[0].version >= window[1].version
                    || compare_version_strings(&window[0].version, &window[1].version)
                        != std::cmp::Ordering::Less,
                "Expected {} >= {} in sorted output",
                window[0].version,
                window[1].version
            );
        }
    }
}
