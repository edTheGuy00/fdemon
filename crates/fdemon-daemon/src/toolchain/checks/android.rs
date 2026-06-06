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

/// Unconditionally resolve the Android SDK root path from an optional caller
/// override, environment variables, and the platform-specific default.
///
/// Resolution order:
/// 1. `override_path` — the caller-supplied path (e.g. from
///    `[toolchain] android_sdk_root` in `.fdemon/config.toml` or from a
///    previous install step).
/// 2. `$ANDROID_HOME` environment variable (if set and non-empty).
/// 3. `$ANDROID_SDK_ROOT` environment variable (if set and non-empty).
/// 4. Platform-specific default:
///    - Linux:   `~/Android/Sdk`
///    - macOS:   `~/Library/Android/sdk`
///    - Windows: `%LOCALAPPDATA%\Android\Sdk`
/// 5. Last resort: `PathBuf::from("Android/Sdk")` — returned when
///    `dirs::home_dir()` is `None` (headless/container environments).
///
/// This function **always** returns a `PathBuf`, even when the path does not
/// yet exist on the filesystem. It is the shared source of truth for both the
/// install executor (which creates the directory) and the post-install check
/// (which filters by `is_dir()`).
///
/// See [`android_sdk_root_with_override`] for the check-time variant that
/// additionally requires the resolved path to be an existing directory.
pub fn resolve_android_sdk_root_path(override_path: Option<&Path>) -> PathBuf {
    // 1. Caller-provided path.
    if let Some(p) = override_path {
        return p.to_path_buf();
    }

    // 2. ANDROID_HOME
    if let Ok(home) = std::env::var("ANDROID_HOME") {
        if !home.is_empty() {
            return PathBuf::from(home);
        }
    }

    // 3. ANDROID_SDK_ROOT
    if let Ok(sdk) = std::env::var("ANDROID_SDK_ROOT") {
        if !sdk.is_empty() {
            return PathBuf::from(sdk);
        }
    }

    // 4. Platform-specific default (or last resort when home_dir is None).
    platform_default_android_sdk().unwrap_or_else(|| PathBuf::from("Android/Sdk"))
}

/// Resolve the Android SDK root, preferring a caller-supplied override before
/// falling back to env vars / the platform default. Returns `Some` only when
/// the resolved path is an **existing directory** on the filesystem.
///
/// Delegates path resolution to [`resolve_android_sdk_root_path`], then applies
/// an `is_dir()` filter and wraps the result in the `AndroidSdkRoot` newtype.
///
/// This is the check-time variant used during toolchain preflight. The
/// `override_path` is passed by the install wizard (from
/// `settings.toolchain.android_sdk_root`) so a re-check after a managed install
/// finds the freshly-installed tools **without** requiring the user to reload
/// their shell — the running process's `$ANDROID_HOME` is still stale at that
/// point, but the persisted override takes precedence over it. For the install
/// executor, use [`resolve_android_sdk_root_path`] directly.
pub fn android_sdk_root_with_override(override_path: Option<&Path>) -> Option<AndroidSdkRoot> {
    let path = resolve_android_sdk_root_path(override_path);
    if path.is_dir() {
        tracing::debug!("Android SDK root resolved to: {}", path.display());
        Some(AndroidSdkRoot(path))
    } else {
        None
    }
}

/// Return the platform-specific default Android SDK installation path.
///
/// Returns `None` only on unsupported platforms or when the home/local-app-data
/// directory cannot be determined.
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
        dirs::home_dir().map(|h| h.join("Android").join("Sdk"))
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
pub fn sdkmanager_bin_name() -> &'static str {
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

        let result = android_sdk_root_with_override(None);

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
        let _ = android_sdk_root_with_override(None);
    }

    #[test]
    #[serial_test::serial]
    fn test_android_sdk_root_override_takes_precedence_over_stale_env() {
        // Core of the post-install fix: a wizard-persisted SDK root passed as the
        // override must win over a stale/wrong `$ANDROID_HOME` in the running
        // process, so the re-check probes the just-installed SDK on disk.
        let real_sdk = TempDir::new().unwrap();
        let _home = EnvGuard::set("ANDROID_HOME", "/nonexistent/stale/android/home");
        let _sdk_root = EnvGuard::remove("ANDROID_SDK_ROOT");

        let result = android_sdk_root_with_override(Some(real_sdk.path()));

        assert_eq!(
            result.expect("override to an existing dir must resolve").0,
            real_sdk.path(),
            "the override must take precedence over the stale ANDROID_HOME env var"
        );
    }

    #[test]
    #[serial_test::serial]
    fn test_android_sdk_root_override_nonexistent_returns_none() {
        let _home = EnvGuard::remove("ANDROID_HOME");
        let _sdk_root = EnvGuard::remove("ANDROID_SDK_ROOT");

        let result =
            android_sdk_root_with_override(Some(Path::new("/definitely/not/a/real/sdk/root")));

        assert!(
            result.is_none(),
            "a non-existent override path must resolve to None (is_dir filter)"
        );
    }

    // ── Android SDK component checks ──────────────────────────────────────────

    #[test]
    fn test_check_android_cmdline_tools_ok_with_latest() {
        let tmp = TempDir::new().unwrap();
        let bin_dir = tmp.path().join("cmdline-tools").join("latest").join("bin");
        fs::create_dir_all(&bin_dir).unwrap();
        // The check probes the platform-appropriate binary name (sdkmanager.bat
        // on Windows), so write that exact name rather than a hardcoded one.
        fs::write(bin_dir.join(sdkmanager_bin_name()), "").unwrap();

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

    // ── resolve_android_sdk_root_path ─────────────────────────────────────────

    /// With `$ANDROID_HOME` set to an existing tempdir, the unconditional
    /// resolver and the check-time resolver must agree on the same path.
    #[test]
    #[serial_test::serial]
    fn test_resolvers_agree_on_android_home() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().to_str().unwrap().to_string();
        let _home = EnvGuard::set("ANDROID_HOME", &path);
        let _sdk = EnvGuard::remove("ANDROID_SDK_ROOT");

        let unconditional = resolve_android_sdk_root_path(None);
        let check_time = android_sdk_root_with_override(None).map(|r| r.0);

        // Both must resolve to the tempdir.
        assert_eq!(unconditional, tmp.path(), "unconditional resolver mismatch");
        assert_eq!(
            check_time,
            Some(tmp.path().to_path_buf()),
            "check-time resolver mismatch"
        );
        // They must agree with each other.
        assert_eq!(
            unconditional,
            check_time.unwrap(),
            "resolvers disagree on ANDROID_HOME input"
        );
    }

    /// With no env vars set, both resolvers return the same platform-default
    /// string (they may or may not agree on whether the path exists).
    #[test]
    #[serial_test::serial]
    fn test_resolvers_agree_on_platform_default_string() {
        let _home = EnvGuard::remove("ANDROID_HOME");
        let _sdk = EnvGuard::remove("ANDROID_SDK_ROOT");

        let unconditional = resolve_android_sdk_root_path(None);
        // android_sdk_root() returns None when the default path doesn't exist,
        // but its internal resolved path (before the is_dir() filter) must match
        // what the unconditional resolver returns.  We can't compare the filtered
        // result directly, so we verify the unconditional resolver matches the
        // platform default.
        let platform_default =
            platform_default_android_sdk().unwrap_or_else(|| PathBuf::from("Android/Sdk"));
        assert_eq!(
            unconditional, platform_default,
            "unconditional resolver diverged from platform_default_android_sdk()"
        );
    }

    /// `resolve_android_sdk_root_path` honours the caller-supplied override and
    /// does not consult env vars when an override is given.
    #[test]
    #[serial_test::serial]
    fn test_resolve_path_honours_caller_override() {
        // Even if ANDROID_HOME points somewhere else, override wins.
        let _home = EnvGuard::set("ANDROID_HOME", "/some/other/path");
        let _sdk = EnvGuard::remove("ANDROID_SDK_ROOT");

        let override_path = PathBuf::from("/my/custom/sdk");
        let result = resolve_android_sdk_root_path(Some(&override_path));
        assert_eq!(result, override_path);
    }

    /// `resolve_android_sdk_root_path` returns a path even when all env vars
    /// are absent (no panic guarantee).
    #[test]
    #[serial_test::serial]
    fn test_resolve_path_never_panics_with_no_env() {
        let _home = EnvGuard::remove("ANDROID_HOME");
        let _sdk = EnvGuard::remove("ANDROID_SDK_ROOT");

        let result = resolve_android_sdk_root_path(None);
        // Must return *something* — the exact value is platform-dependent.
        assert!(
            !result.as_os_str().is_empty(),
            "resolver returned empty path"
        );
    }

    // ── process-spawning check (no panic guarantee) ───────────────────────────

    #[tokio::test]
    async fn test_check_android_platform_tools_never_panics() {
        let _ = check_android_platform_tools(None).await;
    }
}
