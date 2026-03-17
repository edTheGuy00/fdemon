## Task: FVM Cache Scanner

**Objective**: Add a cache scanner to `fdemon-daemon` that discovers installed Flutter SDK versions from the FVM cache directory (`~/fvm/versions/`), returning structured data for the Flutter Version panel.

**Depends on**: None

### Scope

- `crates/fdemon-daemon/src/flutter_sdk/cache_scanner.rs`: **NEW** Cache scanning logic
- `crates/fdemon-daemon/src/flutter_sdk/mod.rs`: Declare and re-export `cache_scanner` module
- `crates/fdemon-daemon/src/lib.rs`: Re-export `InstalledSdk` and `scan_installed_versions`

### Details

#### 1. `InstalledSdk` Type

Define in `cache_scanner.rs` (close to where it's produced):

```rust
use std::path::PathBuf;

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
```

#### 2. `scan_installed_versions` Function

```rust
/// Scans the FVM cache directory for installed Flutter SDK versions.
///
/// Checks `FVM_CACHE_PATH` env var first, then falls back to `~/fvm/versions/`.
/// Each subdirectory is validated as a Flutter SDK (must contain `bin/flutter`
/// and a `VERSION` file).
///
/// # Arguments
/// * `active_sdk_root` - Path of the currently resolved SDK, if any.
///   Used to mark the matching version as `is_active: true`.
///
/// # Returns
/// Sorted list of installed SDKs. Sorted by: active first, then channels
/// (stable, beta, main), then semantic versions descending.
pub fn scan_installed_versions(active_sdk_root: Option<&Path>) -> Vec<InstalledSdk> {
    // 1. Determine cache path: $FVM_CACHE_PATH or ~/fvm/versions/
    // 2. Read directory entries
    // 3. For each entry that is a directory:
    //    a. Validate SDK (bin/flutter exists, VERSION file readable)
    //    b. Read version from VERSION file
    //    c. Detect channel (check_channel_name or detect_channel from channel.rs)
    //    d. Compare canonical path with active_sdk_root for is_active
    // 4. Sort and return
}
```

#### 3. Cache Path Resolution

```rust
/// Resolves the FVM cache path.
/// Checks `FVM_CACHE_PATH` env var first, then falls back to `~/fvm/versions/`.
fn resolve_fvm_cache_path() -> Option<PathBuf> {
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
```

#### 4. Channel Detection from Directory Name

Channel-named directories (`stable`, `beta`, `dev`, `main`, `master`) are recognized as channels rather than version numbers. For version-numbered directories (e.g., `3.19.0`), attempt to detect the channel from the SDK's git state using the existing `detect_channel()` from `channel.rs`.

```rust
/// Known FVM channel directory names.
const CHANNEL_NAMES: &[&str] = &["stable", "beta", "dev", "main", "master"];

fn is_channel_name(name: &str) -> bool {
    CHANNEL_NAMES.contains(&name)
}
```

#### 5. Sorting Order

```
1. Active version first (is_active = true)
2. Channel directories: stable > beta > dev > main > master
3. Semver versions descending (3.22.0 > 3.19.0 > 3.16.0)
4. Non-semver strings alphabetically
```

#### 6. SDK Validation

Reuse the existing `validate_sdk_path()` from `flutter_sdk/types.rs` for strict validation, or `validate_sdk_path_lenient()` for directories that may not have a complete Dart SDK cache. For cache scanning, use **lenient validation** — a cached SDK might not have run `flutter precache` yet.

```rust
use super::types::validate_sdk_path_lenient;
```

#### 7. Re-exports

In `flutter_sdk/mod.rs`, add:
```rust
pub mod cache_scanner;
pub use cache_scanner::{InstalledSdk, scan_installed_versions};
```

In `lib.rs`, add `InstalledSdk` and `scan_installed_versions` to the public re-exports.

### Acceptance Criteria

1. `scan_installed_versions()` returns an empty `Vec` when no FVM cache exists
2. `scan_installed_versions()` discovers all valid SDK directories in the cache
3. Invalid/empty directories in the cache are silently skipped
4. `FVM_CACHE_PATH` env var is respected when set
5. `is_active` is `true` for the SDK matching `active_sdk_root` (via canonical path comparison)
6. Results are sorted: active first, then channels, then versions descending
7. Channel names ("stable", "beta", etc.) are correctly identified
8. VERSION file is read for each installed SDK
9. `InstalledSdk` is re-exported from `fdemon_daemon::flutter_sdk`
10. `cargo check --workspace` compiles
11. `cargo test --workspace` passes
12. `cargo clippy --workspace -- -D warnings` passes

### Testing

Use `tempdir` to create fake FVM cache structures. Each test creates a temp directory with the expected layout and calls `scan_installed_versions()`.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    /// Helper: create a minimal valid SDK directory in the cache
    fn create_fake_sdk(cache_dir: &Path, name: &str, version: &str) -> PathBuf {
        let sdk_dir = cache_dir.join(name);
        fs::create_dir_all(sdk_dir.join("bin")).unwrap();
        fs::write(sdk_dir.join("bin/flutter"), "#!/bin/sh").unwrap();
        fs::write(sdk_dir.join("VERSION"), version).unwrap();
        sdk_dir
    }

    #[test]
    fn test_empty_cache_returns_empty() {
        let tmp = TempDir::new().unwrap();
        // No versions directory
        let result = scan_installed_versions_from_path(tmp.path(), None);
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
    fn test_active_version_detected() {
        let tmp = TempDir::new().unwrap();
        let sdk_path = create_fake_sdk(tmp.path(), "3.19.0", "3.19.0");

        let result = scan_installed_versions_from_path(tmp.path(), Some(&sdk_path));
        assert_eq!(result.len(), 1);
        assert!(result[0].is_active);
    }

    #[test]
    fn test_channel_directories_recognized() {
        let tmp = TempDir::new().unwrap();
        create_fake_sdk(tmp.path(), "stable", "3.19.0");
        create_fake_sdk(tmp.path(), "beta", "3.22.0-beta");

        let result = scan_installed_versions_from_path(tmp.path(), None);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].channel, Some("stable".to_string()));
        assert_eq!(result[1].channel, Some("beta".to_string()));
    }

    #[test]
    fn test_invalid_directories_skipped() {
        let tmp = TempDir::new().unwrap();
        create_fake_sdk(tmp.path(), "3.19.0", "3.19.0");
        // Create an invalid directory (no bin/flutter)
        fs::create_dir_all(tmp.path().join("corrupt")).unwrap();

        let result = scan_installed_versions_from_path(tmp.path(), None);
        assert_eq!(result.len(), 1);
    }

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
    fn test_fvm_cache_path_env_var() {
        // Test that FVM_CACHE_PATH is respected
        // (use temp_env or similar to set env var in test)
    }
}
```

**Implementation note**: Expose a `scan_installed_versions_from_path(cache_path, active_sdk_root)` function for testability, with `scan_installed_versions(active_sdk_root)` as the public wrapper that resolves the cache path.

### Notes

- **This function will be called asynchronously** via `tokio::task::spawn_blocking()` from the action dispatcher, since it does filesystem I/O. The function itself is synchronous (no async).
- **`dirs` crate is already a workspace dependency** in `fdemon-daemon` (added in Phase 1 for home directory resolution).
- **Lenient validation** (`validate_sdk_path_lenient`) should be used because FVM cache entries may be channel checkouts without `bin/cache/dart-sdk/` until `flutter precache` is run.
- **Canonical path comparison** for `is_active`: use `fs::canonicalize()` on both the candidate and `active_sdk_root`, comparing the canonical paths. This handles symlinks correctly (FVM often uses symlinks).
- **Sorting**: Use a custom `Ord` implementation or a comparator closure. The `semver` crate is not needed — simple string comparison of version components is sufficient, or parse manually with `split('.')`.
