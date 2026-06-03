//! # Toolchain Component Probes
//!
//! One `async fn check_*` per toolchain component, each returning a
//! [`ComponentCheck`]. All probes are read-only; they never install, download,
//! or modify system state.
//!
//! See also: [`android_sdk_root`] — resolves the Android SDK root from env vars
//! and OS-specific default locations.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;

use crate::flutter_sdk::find_flutter_sdk;

use super::types::{AndroidSdkRoot, ComponentCheck, ComponentKind, ComponentStatus, HostPlatform};

/// Timeout for lightweight `--version` style tool probes.
const PROBE_TIMEOUT: Duration = Duration::from_secs(10);

/// Timeout for the JDK probe, which may be slower on first run.
const JDK_PROBE_TIMEOUT: Duration = Duration::from_secs(15);

// ─── Flutter SDK ─────────────────────────────────────────────────────────────

/// Check for the Flutter SDK.
///
/// Runs the full 12-strategy SDK locator. On success, returns `Ok` with a
/// detail string containing the version and discovery source. On failure,
/// classifies as `Missing` or `Partial` depending on the error.
pub async fn check_flutter(
    project_path: &Path,
    explicit_path: Option<&Path>,
) -> (
    ComponentCheck,
    Option<crate::flutter_sdk::FlutterExecutable>,
) {
    match find_flutter_sdk(project_path, explicit_path) {
        Ok(sdk) => {
            let detail = format!("{} ({})", sdk.version, sdk.source);
            (
                ComponentCheck {
                    kind: ComponentKind::FlutterSdk,
                    status: ComponentStatus::Ok,
                    detail,
                },
                Some(sdk.executable),
            )
        }
        Err(e) => {
            use fdemon_core::error::Error;
            let (status, detail) = match &e {
                Error::FlutterNotFound => (
                    ComponentStatus::Missing,
                    "Flutter SDK not found. Ensure 'flutter' is in your PATH.".to_string(),
                ),
                Error::FlutterSdkInvalid { path, reason } => (
                    ComponentStatus::Partial,
                    format!("SDK at {} is invalid: {}", path.display(), reason),
                ),
                other => (ComponentStatus::Error, other.to_string()),
            };
            (
                ComponentCheck {
                    kind: ComponentKind::FlutterSdk,
                    status,
                    detail,
                },
                None,
            )
        }
    }
}

// ─── Git ─────────────────────────────────────────────────────────────────────

/// Check for `git` on PATH.
///
/// Runs `git --version` and parses the version string. Returns `Ok` when git
/// is found and responsive, `Missing` otherwise.
pub async fn check_git() -> ComponentCheck {
    let result = tokio::time::timeout(PROBE_TIMEOUT, async {
        Command::new("git")
            .arg("--version")
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
            let version = raw.trim().to_string();
            ComponentCheck {
                kind: ComponentKind::Git,
                status: ComponentStatus::Ok,
                detail: version,
            }
        }
        Ok(Ok(output)) => {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            ComponentCheck {
                kind: ComponentKind::Git,
                status: ComponentStatus::Error,
                detail: if stderr.is_empty() {
                    format!("git exited with status {}", output.status)
                } else {
                    stderr
                },
            }
        }
        Ok(Err(e)) if e.kind() == std::io::ErrorKind::NotFound => ComponentCheck {
            kind: ComponentKind::Git,
            status: ComponentStatus::Missing,
            detail: "git not found on PATH".to_string(),
        },
        Ok(Err(e)) => ComponentCheck {
            kind: ComponentKind::Git,
            status: ComponentStatus::Error,
            detail: format!("git probe failed: {e}"),
        },
        Err(_) => ComponentCheck {
            kind: ComponentKind::Git,
            status: ComponentStatus::Error,
            detail: "git --version timed out".to_string(),
        },
    }
}

// ─── JDK ─────────────────────────────────────────────────────────────────────

/// Check for a Java Development Kit.
///
/// Runs `java -version` (which writes to **stderr**) and parses the major version.
/// - Major version `>= 17` → `Ok`
/// - Present but `< 17` → `Partial` (detail names the version)
/// - Not found → `Missing`
pub async fn check_jdk() -> ComponentCheck {
    let result = tokio::time::timeout(JDK_PROBE_TIMEOUT, async {
        Command::new("java")
            .arg("-version")
            // `java -version` outputs to stderr, not stdout
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .stdin(Stdio::null())
            .output()
            .await
    })
    .await;

    match result {
        Ok(Ok(output)) => {
            // `java -version` exits with 0 whether or not stderr has output.
            let stderr_text = String::from_utf8_lossy(&output.stderr);
            parse_jdk_output(&stderr_text)
        }
        Ok(Err(e)) if e.kind() == std::io::ErrorKind::NotFound => ComponentCheck {
            kind: ComponentKind::Jdk,
            status: ComponentStatus::Missing,
            detail: "java not found on PATH".to_string(),
        },
        Ok(Err(e)) => ComponentCheck {
            kind: ComponentKind::Jdk,
            status: ComponentStatus::Error,
            detail: format!("java probe failed: {e}"),
        },
        Err(_) => ComponentCheck {
            kind: ComponentKind::Jdk,
            status: ComponentStatus::Error,
            detail: "java -version timed out".to_string(),
        },
    }
}

/// Parse the stderr output of `java -version` into a [`ComponentCheck`].
///
/// Handles both the modern `openjdk version "17.0.2" ...` format and the
/// older `java version "1.8.0_291"` format.
fn parse_jdk_output(text: &str) -> ComponentCheck {
    // Look for a version string like `"17.0.2"` or `"1.8.0_291"`
    // The first line typically has: openjdk version "X.Y.Z" ...
    let first_line = text.lines().next().unwrap_or("");

    if first_line.is_empty() {
        return ComponentCheck {
            kind: ComponentKind::Jdk,
            status: ComponentStatus::Missing,
            detail: "java not found on PATH".to_string(),
        };
    }

    // Extract the quoted version string
    let version_str = extract_quoted_version(first_line);

    match version_str {
        Some(v) => {
            let major = parse_java_major_version(&v);
            match major {
                Some(maj) if maj >= 17 => ComponentCheck {
                    kind: ComponentKind::Jdk,
                    status: ComponentStatus::Ok,
                    detail: format!("Java {v} (major {maj})"),
                },
                Some(maj) => ComponentCheck {
                    kind: ComponentKind::Jdk,
                    status: ComponentStatus::Partial,
                    detail: format!("Java {v} (major {maj}) — Android requires JDK 17 or newer"),
                },
                None => ComponentCheck {
                    kind: ComponentKind::Jdk,
                    status: ComponentStatus::Ok,
                    detail: format!("Java {v}"),
                },
            }
        }
        None => ComponentCheck {
            kind: ComponentKind::Jdk,
            status: ComponentStatus::Error,
            detail: format!("could not parse java version from: {first_line}"),
        },
    }
}

/// Extract the version string from inside double-quotes in a `java -version` line.
fn extract_quoted_version(line: &str) -> Option<String> {
    let start = line.find('"')? + 1;
    let end = line[start..].find('"')? + start;
    Some(line[start..end].to_string())
}

/// Parse the major Java version from a version string.
///
/// Handles both:
/// - Modern: `"17.0.2"` → 17
/// - Legacy: `"1.8.0_291"` → 8 (the second component when first is `1`)
fn parse_java_major_version(v: &str) -> Option<u32> {
    let mut parts = v.split('.');
    let first: u32 = parts.next()?.parse().ok()?;
    if first == 1 {
        // Legacy version format: 1.X.Y → major is X
        parts.next()?.parse().ok()
    } else {
        Some(first)
    }
}

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
fn sdkmanager_bin_name() -> &'static str {
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
            let first_line = raw.lines().next().unwrap_or("adb").trim().to_string();
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
fn count_subdirs(dir: &Path) -> Option<usize> {
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

// ─── Prerequisites ────────────────────────────────────────────────────────────

/// Check OS-level prerequisites for Flutter development.
///
/// The check is **lightweight and read-only** — it only verifies binary
/// presence via `which::which`, never generates install commands (Phase 4).
///
/// - **Linux**: checks for `cmake`, `ninja`, `pkg-config`, `clang`, `curl`,
///   `unzip`, `xz` (or `xz-utils`).
/// - **macOS**: checks `xcode-select -p` exit status.
/// - **Windows**: checks for `git` (a proxy for developer tools presence).
/// - **Other**: returns `Unknown`.
pub async fn check_prerequisites(platform: &HostPlatform) -> ComponentCheck {
    match platform {
        HostPlatform::Linux => check_linux_prerequisites().await,
        HostPlatform::MacOs => check_macos_prerequisites().await,
        HostPlatform::Windows => check_windows_prerequisites().await,
        HostPlatform::Unknown => ComponentCheck {
            kind: ComponentKind::Prerequisites,
            status: ComponentStatus::Unknown,
            detail: "Unknown platform — prerequisites check skipped".to_string(),
        },
    }
}

/// Required tools on Linux for Flutter development.
const LINUX_REQUIRED_TOOLS: &[&str] = &[
    "cmake",
    "ninja",
    "pkg-config",
    "clang",
    "curl",
    "unzip",
    "xz",
];

async fn check_linux_prerequisites() -> ComponentCheck {
    let missing: Vec<&str> = LINUX_REQUIRED_TOOLS
        .iter()
        .copied()
        .filter(|tool| {
            // Try both the bare name and common alternatives
            let found = which::which(tool).is_ok();
            if !found && *tool == "ninja" {
                // ninja may be called `ninja-build` on some distros
                return which::which("ninja-build").is_err();
            }
            if !found && *tool == "xz" {
                // xz may not be on PATH separately; also check `xz-utils`
                return which::which("xz-utils").is_err();
            }
            !found
        })
        .collect();

    if missing.is_empty() {
        ComponentCheck {
            kind: ComponentKind::Prerequisites,
            status: ComponentStatus::Ok,
            detail: "All required Linux tools present".to_string(),
        }
    } else {
        ComponentCheck {
            kind: ComponentKind::Prerequisites,
            status: ComponentStatus::Partial,
            detail: format!("Missing tools: {}", missing.join(", ")),
        }
    }
}

async fn check_macos_prerequisites() -> ComponentCheck {
    let result = tokio::time::timeout(PROBE_TIMEOUT, async {
        Command::new("xcode-select")
            .arg("-p")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .stdin(Stdio::null())
            .status()
            .await
    })
    .await;

    match result {
        Ok(Ok(status)) if status.success() => ComponentCheck {
            kind: ComponentKind::Prerequisites,
            status: ComponentStatus::Ok,
            detail: "Xcode Command Line Tools installed".to_string(),
        },
        Ok(Ok(_)) => ComponentCheck {
            kind: ComponentKind::Prerequisites,
            status: ComponentStatus::Missing,
            detail: "Xcode Command Line Tools not installed. Run: xcode-select --install"
                .to_string(),
        },
        Ok(Err(e)) if e.kind() == std::io::ErrorKind::NotFound => ComponentCheck {
            kind: ComponentKind::Prerequisites,
            status: ComponentStatus::Missing,
            detail: "xcode-select not found — install Xcode from the App Store".to_string(),
        },
        _ => ComponentCheck {
            kind: ComponentKind::Prerequisites,
            status: ComponentStatus::Unknown,
            detail: "Could not determine Xcode Command Line Tools status".to_string(),
        },
    }
}

async fn check_windows_prerequisites() -> ComponentCheck {
    // On Windows, use git presence as a proxy for developer tools
    match which::which("git") {
        Ok(_) => ComponentCheck {
            kind: ComponentKind::Prerequisites,
            status: ComponentStatus::Ok,
            detail: "Git found (Windows prerequisites appear satisfied)".to_string(),
        },
        Err(_) => ComponentCheck {
            kind: ComponentKind::Prerequisites,
            status: ComponentStatus::Partial,
            detail: "Git not found on PATH. Install Git for Windows.".to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    // ── android_sdk_root ──────────────────────────────────────────────────────

    #[test]
    fn test_android_sdk_root_from_env_android_home() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().to_str().unwrap().to_string();
        std::env::set_var("ANDROID_HOME", &path);
        // Unset SDK_ROOT to avoid interference
        std::env::remove_var("ANDROID_SDK_ROOT");

        let result = android_sdk_root();
        std::env::remove_var("ANDROID_HOME");

        assert!(result.is_some());
        assert_eq!(result.unwrap().0, tmp.path());
    }

    #[test]
    fn test_android_sdk_root_returns_none_for_nonexistent_path() {
        std::env::remove_var("ANDROID_HOME");
        std::env::remove_var("ANDROID_SDK_ROOT");
        // Default platform path is unlikely to exist in CI with a made-up name,
        // but we cannot set it to nothing easily — just verify no panic.
        let _ = android_sdk_root();
    }

    // ── JDK parsing ───────────────────────────────────────────────────────────

    #[test]
    fn test_parse_jdk_modern_version_17() {
        let text = r#"openjdk version "17.0.9" 2023-10-17
OpenJDK Runtime Environment (build 17.0.9+9)
"#;
        let check = parse_jdk_output(text);
        assert_eq!(check.status, ComponentStatus::Ok);
        assert!(check.detail.contains("17.0.9"));
    }

    #[test]
    fn test_parse_jdk_modern_version_21() {
        let text = r#"openjdk version "21.0.1" 2023-10-17
OpenJDK Runtime Environment (build 21.0.1+12)
"#;
        let check = parse_jdk_output(text);
        assert_eq!(check.status, ComponentStatus::Ok);
    }

    #[test]
    fn test_parse_jdk_legacy_version_8_yields_partial() {
        let text = r#"java version "1.8.0_291"
Java(TM) SE Runtime Environment (build 1.8.0_291-b10)
"#;
        let check = parse_jdk_output(text);
        assert_eq!(check.status, ComponentStatus::Partial);
        assert!(check.detail.contains("1.8.0_291"));
    }

    #[test]
    fn test_parse_jdk_version_11_yields_partial() {
        let text = r#"openjdk version "11.0.20" 2023-07-18
OpenJDK Runtime Environment (build 11.0.20+8)
"#;
        let check = parse_jdk_output(text);
        assert_eq!(check.status, ComponentStatus::Partial);
    }

    #[test]
    fn test_parse_jdk_empty_output_yields_missing() {
        let check = parse_jdk_output("");
        assert_eq!(check.status, ComponentStatus::Missing);
    }

    #[test]
    fn test_extract_quoted_version_basic() {
        assert_eq!(
            extract_quoted_version(r#"openjdk version "17.0.9" 2023"#),
            Some("17.0.9".to_string())
        );
    }

    #[test]
    fn test_extract_quoted_version_none_when_no_quotes() {
        assert_eq!(extract_quoted_version("no quotes here"), None);
    }

    #[test]
    fn test_parse_java_major_modern() {
        assert_eq!(parse_java_major_version("17.0.9"), Some(17));
        assert_eq!(parse_java_major_version("21.0.1"), Some(21));
    }

    #[test]
    fn test_parse_java_major_legacy() {
        assert_eq!(parse_java_major_version("1.8.0_291"), Some(8));
        assert_eq!(parse_java_major_version("1.11.0"), Some(11));
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

    // ── process-spawning checks (no panic guarantee) ───────────────────────

    #[tokio::test]
    async fn test_check_git_present_or_missing_never_panics() {
        let _ = check_git().await;
    }

    #[tokio::test]
    async fn test_check_jdk_present_or_missing_never_panics() {
        let _ = check_jdk().await;
    }

    #[tokio::test]
    async fn test_check_android_platform_tools_never_panics() {
        let _ = check_android_platform_tools(None).await;
    }

    #[tokio::test]
    async fn test_check_prerequisites_never_panics() {
        let platform = HostPlatform::detect();
        let _ = check_prerequisites(&platform).await;
    }
}
