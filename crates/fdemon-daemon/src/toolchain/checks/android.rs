//! # Android SDK Probes
//!
//! Read-only diagnostics for the Android SDK components: SDK root discovery,
//! command-line tools, platform tools (`adb`), platform images, build tools,
//! and SDK license acceptance.
//!
//! All functions are re-exported through [`super`] so callers in
//! `toolchain/mod.rs` see them as `checks::check_android_*`.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use tokio::process::Command;

use super::super::types::{AndroidSdkRoot, ComponentCheck, ComponentKind, ComponentStatus};
use super::PROBE_TIMEOUT;

// ─── Android SDK root resolver ────────────────────────────────────────────────

/// Resolve the Android SDK root path.
///
/// Checks in this order:
/// 1. `$ANDROID_HOME` environment variable
/// 2. `$ANDROID_SDK_ROOT` environment variable
/// 3. Platform-specific default location:
///    - Linux: `~/Android/Sdk`
///    - macOS: `~/Library/Android/sdk`
///    - Windows: `%LOCALAPPDATA%\Android\Sdk`
///
/// Returns the first path that **exists** on the filesystem, or `None` if none
/// do.
pub fn android_sdk_root() -> Option<AndroidSdkRoot> {
    // 1. ANDROID_HOME
    if let Ok(path) = std::env::var("ANDROID_HOME") {
        let p = PathBuf::from(&path);
        if p.is_dir() {
            tracing::debug!("Android SDK root from ANDROID_HOME: {}", p.display());
            return Some(AndroidSdkRoot(p));
        }
    }

    // 2. ANDROID_SDK_ROOT
    if let Ok(path) = std::env::var("ANDROID_SDK_ROOT") {
        let p = PathBuf::from(&path);
        if p.is_dir() {
            tracing::debug!("Android SDK root from ANDROID_SDK_ROOT: {}", p.display());
            return Some(AndroidSdkRoot(p));
        }
    }

    // 3. Platform-specific default
    if let Some(default_path) = platform_default_android_sdk() {
        if default_path.is_dir() {
            tracing::debug!(
                "Android SDK root from platform default: {}",
                default_path.display()
            );
            return Some(AndroidSdkRoot(default_path));
        }
    }

    None
}

/// Return the platform-specific default Android SDK installation path.
fn platform_default_android_sdk() -> Option<PathBuf> {
    #[cfg(target_os = "linux")]
    {
        dirs::home_dir().map(|h| h.join("Android").join("Sdk"))
    }

    #[cfg(target_os = "macos")]
    {
        dirs::home_dir().map(|h| h.join("Library").join("Android").join("sdk"))
    }

    #[cfg(target_os = "windows")]
    {
        dirs::data_local_dir().map(|d| d.join("Android").join("Sdk"))
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        None
    }
}

// ─── Android SDK components ───────────────────────────────────────────────────

/// Check for Android command-line tools (`sdkmanager`).
///
/// Looks for `<root>/cmdline-tools/latest/bin/sdkmanager(.bat)`.
/// - Found under `latest/` → `Ok`
/// - `cmdline-tools/` exists but not under `latest/` → `Partial`
/// - Not found → `Missing`
/// - No SDK root → `Unknown`
pub fn check_android_cmdline_tools(root: Option<&AndroidSdkRoot>) -> ComponentCheck {
    let root = match root {
        Some(r) => &r.0,
        None => {
            return ComponentCheck {
                kind: ComponentKind::AndroidCmdlineTools,
                status: ComponentStatus::Unknown,
                detail: "Android SDK root not found".to_string(),
            }
        }
    };

    let sdkmanager_bin = sdkmanager_bin_name();
    let latest_path = root
        .join("cmdline-tools")
        .join("latest")
        .join("bin")
        .join(sdkmanager_bin);

    if latest_path.is_file() {
        return ComponentCheck {
            kind: ComponentKind::AndroidCmdlineTools,
            status: ComponentStatus::Ok,
            detail: format!("sdkmanager found at {}", latest_path.display()),
        };
    }

    // Check if cmdline-tools exists but lacks the `latest/` subdirectory
    let cmdline_tools_dir = root.join("cmdline-tools");
    if cmdline_tools_dir.is_dir() {
        return ComponentCheck {
            kind: ComponentKind::AndroidCmdlineTools,
            status: ComponentStatus::Partial,
            detail: format!(
                "cmdline-tools found at {} but missing 'latest/' subdirectory. \
                 Re-install via SDK Manager to create the 'latest' alias.",
                cmdline_tools_dir.display()
            ),
        };
    }

    ComponentCheck {
        kind: ComponentKind::AndroidCmdlineTools,
        status: ComponentStatus::Missing,
        detail: format!(
            "cmdline-tools not found under {}. \
             Install 'Android SDK Command-line Tools' via SDK Manager.",
            root.display()
        ),
    }
}

/// Platform-appropriate name for the `sdkmanager` binary.
pub(super) fn sdkmanager_bin_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "sdkmanager.bat"
    } else {
        "sdkmanager"
    }
}

/// Check for Android platform tools (`adb`).
///
/// Prefers `<root>/platform-tools/adb`; falls back to `adb` on PATH.
pub async fn check_android_platform_tools(root: Option<&AndroidSdkRoot>) -> ComponentCheck {
    // Try the SDK-bundled adb first
    if let Some(r) = root {
        let adb_name = if cfg!(target_os = "windows") {
            "adb.exe"
        } else {
            "adb"
        };
        let sdk_adb = r.0.join("platform-tools").join(adb_name);
        if sdk_adb.is_file() {
            if let Some(version) = probe_adb_version(&sdk_adb).await {
                return ComponentCheck {
                    kind: ComponentKind::AndroidPlatformTools,
                    status: ComponentStatus::Ok,
                    detail: format!("{} ({})", sdk_adb.display(), version),
                };
            }
        }
    }

    // Fall back to PATH adb
    let result = tokio::time::timeout(PROBE_TIMEOUT, async {
        Command::new("adb")
            .arg("version")
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .stdin(Stdio::null())
            .output()
            .await
    })
    .await;

    match result {
        Ok(Ok(output)) if output.status.success() => {
            let raw = String::from_utf8_lossy(&output.stdout);
            let first_line = super::strip_and_truncate(raw.lines().next().unwrap_or("adb").trim());
            ComponentCheck {
                kind: ComponentKind::AndroidPlatformTools,
                status: ComponentStatus::Ok,
                detail: first_line,
            }
        }
        Ok(Err(e)) if e.kind() == std::io::ErrorKind::NotFound => ComponentCheck {
            kind: ComponentKind::AndroidPlatformTools,
            status: ComponentStatus::Missing,
            detail: "adb not found. Install Android Platform Tools via SDK Manager.".to_string(),
        },
        _ => ComponentCheck {
            kind: ComponentKind::AndroidPlatformTools,
            status: ComponentStatus::Missing,
            detail: "adb not found. Install Android Platform Tools via SDK Manager.".to_string(),
        },
    }
}

/// Probe the version string of an `adb` binary at the given path.
async fn probe_adb_version(adb_path: &Path) -> Option<String> {
    let result = tokio::time::timeout(PROBE_TIMEOUT, async {
        Command::new(adb_path)
            .arg("version")
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .stdin(Stdio::null())
            .output()
            .await
    })
    .await;

    match result {
        Ok(Ok(output)) if output.status.success() => {
            let raw = String::from_utf8_lossy(&output.stdout);
            Some(raw.lines().next().unwrap_or("").trim().to_string())
        }
        _ => None,
    }
}

/// Check whether at least one Android platform image is installed.
///
/// Scans `<root>/platforms/` for subdirectories (e.g., `android-34`).
pub fn check_android_platform(root: Option<&AndroidSdkRoot>) -> ComponentCheck {
    let root = match root {
        Some(r) => &r.0,
        None => {
            return ComponentCheck {
                kind: ComponentKind::AndroidPlatform,
                status: ComponentStatus::Unknown,
                detail: "Android SDK root not found".to_string(),
            }
        }
    };

    let platforms_dir = root.join("platforms");
    match count_subdirs(&platforms_dir) {
        Some(n) if n > 0 => ComponentCheck {
            kind: ComponentKind::AndroidPlatform,
            status: ComponentStatus::Ok,
            detail: format!(
                "{n} platform image(s) installed under {}",
                platforms_dir.display()
            ),
        },
        Some(_) => ComponentCheck {
            kind: ComponentKind::AndroidPlatform,
            status: ComponentStatus::Missing,
            detail: format!(
                "No platform images found in {}. \
                 Install at least one SDK platform via SDK Manager.",
                platforms_dir.display()
            ),
        },
        None => ComponentCheck {
            kind: ComponentKind::AndroidPlatform,
            status: ComponentStatus::Missing,
            detail: format!(
                "platforms/ directory not found under {}. \
                 Install Android SDK Platform via SDK Manager.",
                root.display()
            ),
        },
    }
}

/// Check whether at least one set of Android build tools is installed.
///
/// Scans `<root>/build-tools/` for version subdirectories (e.g., `34.0.0`).
pub fn check_android_build_tools(root: Option<&AndroidSdkRoot>) -> ComponentCheck {
    let root = match root {
        Some(r) => &r.0,
        None => {
            return ComponentCheck {
                kind: ComponentKind::AndroidBuildTools,
                status: ComponentStatus::Unknown,
                detail: "Android SDK root not found".to_string(),
            }
        }
    };

    let build_tools_dir = root.join("build-tools");
    match count_subdirs(&build_tools_dir) {
        Some(n) if n > 0 => ComponentCheck {
            kind: ComponentKind::AndroidBuildTools,
            status: ComponentStatus::Ok,
            detail: format!(
                "{n} build-tools version(s) installed under {}",
                build_tools_dir.display()
            ),
        },
        Some(_) => ComponentCheck {
            kind: ComponentKind::AndroidBuildTools,
            status: ComponentStatus::Missing,
            detail: format!(
                "No build-tools found in {}. \
                 Install Android SDK Build-Tools via SDK Manager.",
                build_tools_dir.display()
            ),
        },
        None => ComponentCheck {
            kind: ComponentKind::AndroidBuildTools,
            status: ComponentStatus::Missing,
            detail: format!(
                "build-tools/ directory not found under {}. \
                 Install Android SDK Build-Tools via SDK Manager.",
                root.display()
            ),
        },
    }
}

/// Check whether the Android SDK licenses have been accepted.
///
/// Checks for the presence of `<root>/licenses/android-sdk-license`.
pub fn check_android_licenses(root: Option<&AndroidSdkRoot>) -> ComponentCheck {
    let root = match root {
        Some(r) => &r.0,
        None => {
            return ComponentCheck {
                kind: ComponentKind::AndroidLicenses,
                status: ComponentStatus::Unknown,
                detail: "Android SDK root not found".to_string(),
            }
        }
    };

    let license_file = root.join("licenses").join("android-sdk-license");
    if license_file.is_file() {
        ComponentCheck {
            kind: ComponentKind::AndroidLicenses,
            status: ComponentStatus::Ok,
            detail: format!("android-sdk-license found at {}", license_file.display()),
        }
    } else {
        ComponentCheck {
            kind: ComponentKind::AndroidLicenses,
            status: ComponentStatus::Missing,
            detail: "Android SDK licenses not accepted. Run: flutter doctor --android-licenses"
                .to_string(),
        }
    }
}

/// Count the immediate subdirectories of `dir`. Returns `None` if the directory
/// does not exist or cannot be read.
pub(super) fn count_subdirs(dir: &Path) -> Option<usize> {
    if !dir.is_dir() {
        return None;
    }
    std::fs::read_dir(dir).ok().map(|entries| {
        entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .count()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    // ── android_sdk_root ──────────────────────────────────────────────────────

    /// Restore environment variable values after a test that mutates them.
    struct EnvGuard {
        key: &'static str,
        prior: Option<String>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let prior = std::env::var(key).ok();
            // SAFETY: single-threaded by #[serial_test::serial]; no other thread reads these vars.
            unsafe { std::env::set_var(key, value) };
            Self { key, prior }
        }

        fn remove(key: &'static str) -> Self {
            let prior = std::env::var(key).ok();
            // SAFETY: single-threaded by #[serial_test::serial]; no other thread reads these vars.
            unsafe { std::env::remove_var(key) };
            Self { key, prior }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.prior {
                // SAFETY: restoring a value that was present before the test began.
                Some(v) => unsafe { std::env::set_var(self.key, v) },
                // SAFETY: variable was absent before the test; restore that state.
                None => unsafe { std::env::remove_var(self.key) },
            }
        }
    }

    #[test]
    #[serial_test::serial]
    fn test_android_sdk_root_from_env_android_home() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().to_str().unwrap().to_string();
        let _home = EnvGuard::set("ANDROID_HOME", &path);
        let _sdk_root = EnvGuard::remove("ANDROID_SDK_ROOT");

        let result = android_sdk_root();

        assert!(result.is_some());
        assert_eq!(result.unwrap().0, tmp.path());
    }

    #[test]
    #[serial_test::serial]
    fn test_android_sdk_root_returns_none_for_nonexistent_path() {
        let _home = EnvGuard::remove("ANDROID_HOME");
        let _sdk_root = EnvGuard::remove("ANDROID_SDK_ROOT");
        // Default platform path is unlikely to exist in CI with a made-up name,
        // but we cannot set it to nothing easily — just verify no panic.
        let _ = android_sdk_root();
    }

    // ── Android SDK component checks ──────────────────────────────────────────

    #[test]
    fn test_check_android_cmdline_tools_ok_with_latest() {
        let tmp = TempDir::new().unwrap();
        let bin_dir = tmp.path().join("cmdline-tools").join("latest").join("bin");
        fs::create_dir_all(&bin_dir).unwrap();
        fs::write(bin_dir.join("sdkmanager"), "").unwrap();

        let root = AndroidSdkRoot(tmp.path().to_path_buf());
        let check = check_android_cmdline_tools(Some(&root));
        assert_eq!(check.status, ComponentStatus::Ok);
    }

    #[test]
    fn test_check_android_cmdline_tools_partial_without_latest() {
        let tmp = TempDir::new().unwrap();
        // cmdline-tools exists but no latest/ subdirectory
        fs::create_dir_all(tmp.path().join("cmdline-tools").join("6.0")).unwrap();

        let root = AndroidSdkRoot(tmp.path().to_path_buf());
        let check = check_android_cmdline_tools(Some(&root));
        assert_eq!(check.status, ComponentStatus::Partial);
    }

    #[test]
    fn test_check_android_cmdline_tools_missing() {
        let tmp = TempDir::new().unwrap();
        let root = AndroidSdkRoot(tmp.path().to_path_buf());
        let check = check_android_cmdline_tools(Some(&root));
        assert_eq!(check.status, ComponentStatus::Missing);
    }

    #[test]
    fn test_check_android_cmdline_tools_unknown_when_no_root() {
        let check = check_android_cmdline_tools(None);
        assert_eq!(check.status, ComponentStatus::Unknown);
    }

    #[test]
    fn test_check_android_platform_ok_with_platforms() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join("platforms").join("android-34")).unwrap();

        let root = AndroidSdkRoot(tmp.path().to_path_buf());
        let check = check_android_platform(Some(&root));
        assert_eq!(check.status, ComponentStatus::Ok);
    }

    #[test]
    fn test_check_android_platform_missing_when_empty() {
        let tmp = TempDir::new().unwrap();
        // platforms/ exists but empty
        fs::create_dir_all(tmp.path().join("platforms")).unwrap();

        let root = AndroidSdkRoot(tmp.path().to_path_buf());
        let check = check_android_platform(Some(&root));
        assert_eq!(check.status, ComponentStatus::Missing);
    }

    #[test]
    fn test_check_android_build_tools_ok_with_build_tools() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join("build-tools").join("34.0.0")).unwrap();

        let root = AndroidSdkRoot(tmp.path().to_path_buf());
        let check = check_android_build_tools(Some(&root));
        assert_eq!(check.status, ComponentStatus::Ok);
    }

    #[test]
    fn test_check_android_build_tools_missing() {
        let tmp = TempDir::new().unwrap();
        let root = AndroidSdkRoot(tmp.path().to_path_buf());
        let check = check_android_build_tools(Some(&root));
        assert_eq!(check.status, ComponentStatus::Missing);
    }

    #[test]
    fn test_check_android_licenses_ok() {
        let tmp = TempDir::new().unwrap();
        let licenses_dir = tmp.path().join("licenses");
        fs::create_dir_all(&licenses_dir).unwrap();
        fs::write(licenses_dir.join("android-sdk-license"), "hash").unwrap();

        let root = AndroidSdkRoot(tmp.path().to_path_buf());
        let check = check_android_licenses(Some(&root));
        assert_eq!(check.status, ComponentStatus::Ok);
    }

    #[test]
    fn test_check_android_licenses_missing() {
        let tmp = TempDir::new().unwrap();
        let root = AndroidSdkRoot(tmp.path().to_path_buf());
        let check = check_android_licenses(Some(&root));
        assert_eq!(check.status, ComponentStatus::Missing);
    }

    #[test]
    fn test_check_android_licenses_unknown_when_no_root() {
        let check = check_android_licenses(None);
        assert_eq!(check.status, ComponentStatus::Unknown);
    }

    // ── process-spawning check (no panic guarantee) ───────────────────────────

    #[tokio::test]
    async fn test_check_android_platform_tools_never_panics() {
        let _ = check_android_platform_tools(None).await;
    }
}
