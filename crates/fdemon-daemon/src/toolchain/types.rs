//! # Toolchain Report Types
//!
//! Data types for the structured, read-only Flutter toolchain preflight
//! diagnostics. All types are `Debug + Clone` and owned by `ToolchainReport`.

use std::path::PathBuf;

/// The overall result of a toolchain preflight scan.
///
/// Contains platform metadata, per-component statuses, and the parsed output
/// of `flutter doctor -v` when Flutter is available.
///
/// The `linux_package_manager` and `winget_available` fields are pre-computed
/// by the async `run_preflight` task so that the TEA `update()` path (which
/// consumes this report via `build_steps`) remains a pure function of the
/// report — no synchronous `which::which` I/O inside `update()`.
#[derive(Debug, Clone)]
pub struct ToolchainReport {
    /// The operating system platform the diagnostic ran on.
    pub platform: HostPlatform,
    /// The shell detected for the current user.
    pub shell: HostShell,
    /// Status of each checked toolchain component, in user-facing order.
    pub components: Vec<ComponentCheck>,
    /// Parsed `flutter doctor -v` lines; `None` when Flutter is absent or
    /// capture timed out / failed.
    pub doctor: Option<Vec<DoctorLine>>,
    /// Detected Linux package manager, pre-computed during preflight.
    ///
    /// `Some(pm)` on Linux (always populated, even when `pm` is
    /// `LinuxPackageManager::Unknown`). `None` on non-Linux platforms where
    /// the probe does not apply.
    pub linux_package_manager: Option<crate::toolchain::checks::LinuxPackageManager>,
    /// Whether `winget` is available on PATH, pre-computed during preflight.
    ///
    /// `true` on Windows when `winget` is found; always `false` on non-Windows
    /// platforms.
    pub winget_available: bool,
}

/// Status of a single toolchain component.
#[derive(Debug, Clone)]
pub struct ComponentCheck {
    /// Which component was checked.
    pub kind: ComponentKind,
    /// Outcome of the check.
    pub status: ComponentStatus,
    /// Human-readable detail: version found, resolved path, or why it is missing.
    pub detail: String,
}

/// Outcome of a single component check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComponentStatus {
    /// Component is present and fully functional.
    Ok,
    /// Component is present but in a degraded or incomplete state (e.g., wrong
    /// version, or a partially-installed Android SDK).
    Partial,
    /// Component was not found on this system.
    Missing,
    /// A probe error occurred that prevented reliable classification.
    Error,
    /// The check was skipped because a prerequisite (e.g., Android SDK root)
    /// could not be determined.
    Unknown,
}

/// Identifies which toolchain component a [`ComponentCheck`] refers to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComponentKind {
    /// Flutter SDK (the primary Flutter CLI and SDK).
    FlutterSdk,
    /// Git version control system, required by the Flutter SDK.
    Git,
    /// Java Development Kit, required by Android tooling.
    Jdk,
    /// Android command-line tools (`sdkmanager`, `avdmanager`).
    AndroidCmdlineTools,
    /// Android platform tools (`adb`, `fastboot`).
    AndroidPlatformTools,
    /// Android platform SDK images (`platforms/android-XX`).
    AndroidPlatform,
    /// Android build tools.
    AndroidBuildTools,
    /// Android SDK licenses acceptance status.
    AndroidLicenses,
    /// OS-level prerequisites (cmake, ninja, clang, etc. on Linux; Xcode on macOS).
    Prerequisites,
}

impl std::fmt::Display for ComponentKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FlutterSdk => write!(f, "Flutter SDK"),
            Self::Git => write!(f, "Git"),
            Self::Jdk => write!(f, "JDK"),
            Self::AndroidCmdlineTools => write!(f, "Android Command-line Tools"),
            Self::AndroidPlatformTools => write!(f, "Android Platform Tools"),
            Self::AndroidPlatform => write!(f, "Android Platform SDK"),
            Self::AndroidBuildTools => write!(f, "Android Build Tools"),
            Self::AndroidLicenses => write!(f, "Android Licenses"),
            Self::Prerequisites => write!(f, "Prerequisites"),
        }
    }
}

/// The host operating system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostPlatform {
    Linux,
    MacOs,
    Windows,
    Unknown,
}

impl HostPlatform {
    /// Detect the current host platform at compile time.
    pub fn detect() -> Self {
        if cfg!(target_os = "linux") {
            Self::Linux
        } else if cfg!(target_os = "macos") {
            Self::MacOs
        } else if cfg!(target_os = "windows") {
            Self::Windows
        } else {
            Self::Unknown
        }
    }
}

impl std::fmt::Display for HostPlatform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Linux => write!(f, "Linux"),
            Self::MacOs => write!(f, "macOS"),
            Self::Windows => write!(f, "Windows"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

/// The user's current shell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostShell {
    Bash,
    Zsh,
    Fish,
    PowerShell,
    Cmd,
    Unknown,
}

impl HostShell {
    /// Detect the current shell from environment variables.
    ///
    /// On Unix, reads `$SHELL` and checks the basename. On Windows, returns
    /// `PowerShell` as the default.
    pub fn detect() -> Self {
        #[cfg(not(target_os = "windows"))]
        {
            if let Ok(shell_path) = std::env::var("SHELL") {
                let basename = std::path::Path::new(&shell_path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");
                return match basename {
                    "bash" => Self::Bash,
                    "zsh" => Self::Zsh,
                    "fish" => Self::Fish,
                    _ => Self::Unknown,
                };
            }
            Self::Unknown
        }

        #[cfg(target_os = "windows")]
        {
            // Check for common Windows shells via env
            if std::env::var("PSModulePath").is_ok() {
                Self::PowerShell
            } else if std::env::var("PROMPT").is_ok() {
                Self::Cmd
            } else {
                Self::PowerShell // Default assumption on Windows
            }
        }
    }
}

impl std::fmt::Display for HostShell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bash => write!(f, "bash"),
            Self::Zsh => write!(f, "zsh"),
            Self::Fish => write!(f, "fish"),
            Self::PowerShell => write!(f, "PowerShell"),
            Self::Cmd => write!(f, "cmd"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

/// A single line parsed from `flutter doctor -v` output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorLine {
    /// The leading marker character parsed from the line.
    pub marker: DoctorMarker,
    /// The line content, stripped of the marker prefix and ANSI codes.
    pub text: String,
    /// Leading whitespace depth (number of spaces before the marker or text),
    /// useful for rendering indented continuation lines.
    pub indent: usize,
}

/// The leading marker on a `flutter doctor` output line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DoctorMarker {
    /// `[✓]` — item is OK (also `[√]` ASCII fallback).
    Ok,
    /// `[!]` — item has a warning.
    Warning,
    /// `[✗]` — item has an error.
    Error,
    /// `[☠]` — item is dead / crashed.
    Dead,
    /// No marker; this is a continuation or detail line.
    None,
}

/// Resolved path to the Android SDK root, for use within the toolchain module.
#[derive(Debug, Clone)]
pub(super) struct AndroidSdkRoot(pub PathBuf);

// ── Phase 2 install types ─────────────────────────────────────────────────────

/// How a managed Flutter SDK is installed.
///
/// `GitClone` keeps the SDK self-updatable via `flutter upgrade`; `Archive`
/// is used on hosts where git is unavailable or a pre-built archive is preferred.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallMethod {
    /// `git clone -b <channel> --depth 1` — keeps `flutter upgrade` working.
    GitClone,
    /// Download + verify + extract the release archive (no git required).
    Archive,
}

/// Host CPU architecture, used to select the correct release archive.
///
/// Distinct from [`HostPlatform`] (which identifies the OS). The releases
/// manifest URL is constructed from the OS; the archive within the manifest is
/// selected by arch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostArch {
    /// 64-bit x86 (amd64).
    X64,
    /// 64-bit ARM (aarch64 / Apple Silicon).
    Arm64,
    /// Unrecognised or unsupported architecture.
    Unknown,
}

impl HostArch {
    /// Detect the current host CPU architecture at compile time.
    pub fn detect() -> Self {
        if cfg!(target_arch = "x86_64") {
            Self::X64
        } else if cfg!(target_arch = "aarch64") {
            Self::Arm64
        } else {
            Self::Unknown
        }
    }

    /// Return the architecture label as it appears in the Flutter release
    /// manifest's `dart_sdk_arch` field (e.g. `"x64"`, `"arm64"`).
    pub fn as_manifest_str(self) -> Option<&'static str> {
        match self {
            Self::X64 => Some("x64"),
            Self::Arm64 => Some("arm64"),
            Self::Unknown => None,
        }
    }
}

/// A single entry from the Flutter releases manifest (`releases_<os>.json`).
#[derive(Debug, Clone)]
pub struct FlutterRelease {
    /// Flutter version string, e.g. `"3.24.0"`.
    pub version: String,
    /// Release channel: `"stable"`, `"beta"`, or `"dev"`.
    pub channel: String,
    /// Relative archive path under [`FlutterReleaseManifest::base_url`].
    pub archive: String,
    /// Lowercase hex SHA-256 checksum of the archive file.
    pub sha256: String,
    /// Architecture label from the manifest (e.g. `"x64"`, `"arm64"`), or
    /// `None` when the manifest entry predates multi-arch fields.
    pub dart_sdk_arch: Option<String>,
}

/// The parsed Flutter releases manifest (`releases_<os>.json`).
///
/// Downloaded from the Flutter infrastructure and used to resolve the correct
/// archive URL and checksum for a given channel and host architecture.
#[derive(Debug, Clone)]
pub struct FlutterReleaseManifest {
    /// Base URL used to construct full archive download URLs.
    /// Typically `"https://storage.googleapis.com/flutter_infra_release/releases"`.
    pub base_url: String,
    /// The hash of the current stable release (from `current_release.stable`).
    pub current_stable_hash: Option<String>,
    /// All available releases, newest first.
    pub releases: Vec<FlutterRelease>,
}

impl FlutterReleaseManifest {
    /// Resolve the best stable release for the given host architecture.
    ///
    /// Selection order:
    /// 1. The first `stable` release whose `dart_sdk_arch` matches `arch`.
    /// 2. If no arch-specific match exists, fall back to the first `stable`
    ///    release regardless of arch (covers older manifests without per-arch
    ///    entries).
    ///
    /// Returns `None` only when the manifest contains no stable releases at all.
    pub fn resolve_stable(&self, arch: HostArch) -> Option<&FlutterRelease> {
        let arch_str = arch.as_manifest_str();

        // Pass 1: prefer an exact arch match.
        if let Some(label) = arch_str {
            if let Some(r) = self
                .releases
                .iter()
                .find(|r| r.channel == "stable" && r.dart_sdk_arch.as_deref() == Some(label))
            {
                return Some(r);
            }
        }

        // Pass 2: fall back to any stable release (no-arch or unknown arch).
        self.releases.iter().find(|r| r.channel == "stable")
    }
}

/// Resolved parameters for a managed Flutter SDK installation.
#[derive(Debug, Clone)]
pub struct FlutterInstallTarget {
    /// How the SDK should be installed.
    pub method: InstallMethod,
    /// Flutter channel to install (e.g. `"stable"`).
    pub channel: String,
    /// Parent directory that will contain the version subdirectory.
    pub install_root: PathBuf,
    /// Name of the directory to create inside `install_root`
    /// (e.g. `"stable"` or the resolved version string like `"3.24.0"`).
    pub version_dir_name: String,
}

/// Progress event emitted during an archive download.
#[derive(Debug, Clone, Copy)]
pub struct DownloadProgress {
    /// Number of bytes received so far.
    pub received: u64,
    /// Total expected bytes, or `None` when the server did not provide
    /// a `Content-Length` header.
    pub total: Option<u64>,
}

/// Final outcome of a managed Flutter SDK installation.
#[derive(Debug, Clone)]
pub struct FlutterInstallOutcome {
    /// Resolved SDK root directory (the directory containing `bin/flutter`).
    pub sdk_path: PathBuf,
    /// Best-effort version label (e.g. `"3.24.0"` or `"stable"`).
    pub version: String,
    /// The install method that was used.
    pub method: InstallMethod,
}

// ── Phase 3 Android install types ────────────────────────────────────────────

/// Default Android command-line tools build number used to construct the
/// download URL.
///
/// `cmdline-tools` has no stable build-less URL, so this is shipped as a known
/// default and is overridable via `[toolchain] cmdline_tools_build` in
/// `.fdemon/config.toml`.
///
/// Verify or update the current value at:
/// <https://developer.android.com/studio#command-tools>
pub const DEFAULT_CMDLINE_TOOLS_BUILD: &str = "11076708";

/// Resolved parameters for a managed Android SDK installation.
///
/// Carries everything the installer (Phase 3, task 02) needs to download the
/// Android command-line tools, run `sdkmanager`, and produce an
/// [`AndroidInstallOutcome`].
#[derive(Debug, Clone)]
pub struct AndroidInstallTarget {
    /// The resolved `ANDROID_HOME` target directory where the SDK will be
    /// installed.
    pub sdk_root: PathBuf,
    /// The Android API level to install (e.g. `36`).
    pub api_level: u32,
    /// The `cmdline-tools` build number used to construct the download URL.
    /// Resolved from config or defaults to [`DEFAULT_CMDLINE_TOOLS_BUILD`].
    pub cmdline_tools_build: String,
    /// Explicit JDK directory, if configured. When `None`, the installer uses
    /// whatever `java` is on `PATH`.
    pub jdk_path: Option<PathBuf>,
    /// The host operating system, used to select the correct cmdline-tools
    /// archive.
    pub platform: HostPlatform,
}

/// Final outcome of a managed Android SDK installation.
///
/// Returned by the installer (Phase 3, task 02) so the app layer can persist
/// `ANDROID_HOME` and update the wizard state.
#[derive(Debug, Clone)]
pub struct AndroidInstallOutcome {
    /// The SDK root directory that was populated (equivalent to `ANDROID_HOME`).
    pub sdk_root: PathBuf,
    /// The `sdkmanager` package identifiers that were successfully installed,
    /// e.g. `["platform-tools", "platforms;android-36", ...]`.
    pub packages_installed: Vec<String>,
}

/// Build the Android `cmdline-tools` download URL for the given host platform
/// and build number.
///
/// Returns `None` when `platform` is [`HostPlatform::Unknown`] (no matching
/// OS slug exists).
///
/// # Example
///
/// ```
/// use fdemon_daemon::toolchain::{cmdline_tools_url, HostPlatform};
/// let url = cmdline_tools_url(HostPlatform::Linux, "11076708").unwrap();
/// assert!(url.contains("commandlinetools-linux-11076708_latest.zip"));
/// ```
pub fn cmdline_tools_url(platform: HostPlatform, build: &str) -> Option<String> {
    let os = match platform {
        HostPlatform::Linux => "linux",
        HostPlatform::MacOs => "mac",
        HostPlatform::Windows => "win",
        HostPlatform::Unknown => return None,
    };
    Some(format!(
        "https://dl.google.com/android/repository/commandlinetools-{os}-{build}_latest.zip"
    ))
}

/// Return the list of `sdkmanager` package identifiers required for a working
/// Android development environment at the given API level.
///
/// The returned packages are:
/// - `"platform-tools"` — `adb`, `fastboot`, etc.
/// - `"platforms;android-<api>"` — SDK platform image.
/// - `"build-tools;<api>.0.0"` — build tools matching the platform.
/// - `"cmdline-tools;latest"` — self-update the command-line tools to `latest/`.
///
/// # Note on `build-tools` version
///
/// The `<api>.0.0` patch suffix is valid for all stable Android API releases.
/// If a future API level lacks a `.0.0` patch release, the mismatch surfaces as
/// a "package not found" error from `sdkmanager`, and the user can override
/// `android_api_level` in `.fdemon/config.toml`.
pub fn sdkmanager_packages(api_level: u32) -> Vec<String> {
    vec![
        "platform-tools".to_string(),
        format!("platforms;android-{api_level}"),
        format!("build-tools;{api_level}.0.0"),
        "cmdline-tools;latest".to_string(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_host_platform_detect_matches_cfg() {
        let detected = HostPlatform::detect();
        if cfg!(target_os = "linux") {
            assert_eq!(detected, HostPlatform::Linux);
        } else if cfg!(target_os = "macos") {
            assert_eq!(detected, HostPlatform::MacOs);
        } else if cfg!(target_os = "windows") {
            assert_eq!(detected, HostPlatform::Windows);
        } else {
            assert_eq!(detected, HostPlatform::Unknown);
        }
    }

    #[test]
    fn test_component_kind_display() {
        assert_eq!(ComponentKind::FlutterSdk.to_string(), "Flutter SDK");
        assert_eq!(ComponentKind::Git.to_string(), "Git");
        assert_eq!(ComponentKind::Jdk.to_string(), "JDK");
        assert_eq!(
            ComponentKind::AndroidCmdlineTools.to_string(),
            "Android Command-line Tools"
        );
        assert_eq!(
            ComponentKind::AndroidPlatformTools.to_string(),
            "Android Platform Tools"
        );
        assert_eq!(
            ComponentKind::AndroidPlatform.to_string(),
            "Android Platform SDK"
        );
        assert_eq!(
            ComponentKind::AndroidBuildTools.to_string(),
            "Android Build Tools"
        );
        assert_eq!(
            ComponentKind::AndroidLicenses.to_string(),
            "Android Licenses"
        );
        assert_eq!(ComponentKind::Prerequisites.to_string(), "Prerequisites");
    }

    #[test]
    fn test_host_platform_display() {
        assert_eq!(HostPlatform::Linux.to_string(), "Linux");
        assert_eq!(HostPlatform::MacOs.to_string(), "macOS");
        assert_eq!(HostPlatform::Windows.to_string(), "Windows");
        assert_eq!(HostPlatform::Unknown.to_string(), "Unknown");
    }

    #[test]
    fn test_host_shell_display() {
        assert_eq!(HostShell::Bash.to_string(), "bash");
        assert_eq!(HostShell::Zsh.to_string(), "zsh");
        assert_eq!(HostShell::Fish.to_string(), "fish");
        assert_eq!(HostShell::PowerShell.to_string(), "PowerShell");
        assert_eq!(HostShell::Cmd.to_string(), "cmd");
        assert_eq!(HostShell::Unknown.to_string(), "unknown");
    }

    #[test]
    fn test_host_shell_detect_does_not_panic() {
        // We cannot assert a specific value since the shell varies per environment,
        // but the function must not panic.
        let _ = HostShell::detect();
    }

    #[test]
    fn test_component_status_partial_ne_ok() {
        assert_ne!(ComponentStatus::Partial, ComponentStatus::Ok);
    }

    // ── Phase 2 install type tests ────────────────────────────────────────────

    #[test]
    fn test_host_arch_detect_matches_cfg() {
        let detected = HostArch::detect();
        if cfg!(target_arch = "x86_64") {
            assert_eq!(detected, HostArch::X64);
        } else if cfg!(target_arch = "aarch64") {
            assert_eq!(detected, HostArch::Arm64);
        } else {
            assert_eq!(detected, HostArch::Unknown);
        }
    }

    #[test]
    fn test_host_arch_as_manifest_str() {
        assert_eq!(HostArch::X64.as_manifest_str(), Some("x64"));
        assert_eq!(HostArch::Arm64.as_manifest_str(), Some("arm64"));
        assert_eq!(HostArch::Unknown.as_manifest_str(), None);
    }

    /// Helper: build a minimal `FlutterReleaseManifest` with two stable releases
    /// that differ only in `dart_sdk_arch`.
    fn make_manifest_with_two_arches() -> FlutterReleaseManifest {
        FlutterReleaseManifest {
            base_url: "https://example.com".to_string(),
            current_stable_hash: None,
            releases: vec![
                FlutterRelease {
                    version: "3.24.0".to_string(),
                    channel: "stable".to_string(),
                    archive: "stable/linux/flutter_linux_3.24.0-stable.tar.xz".to_string(),
                    sha256: "aaaa".to_string(),
                    dart_sdk_arch: Some("x64".to_string()),
                },
                FlutterRelease {
                    version: "3.24.0".to_string(),
                    channel: "stable".to_string(),
                    archive: "stable/linux/flutter_linux_arm64_3.24.0-stable.tar.xz".to_string(),
                    sha256: "bbbb".to_string(),
                    dart_sdk_arch: Some("arm64".to_string()),
                },
            ],
        }
    }

    #[test]
    fn test_resolve_stable_prefers_arch_match() {
        // manifest with two stable releases (x64, arm64) → resolve_stable(Arm64) picks arm64
        let manifest = make_manifest_with_two_arches();

        let x64 = manifest
            .resolve_stable(HostArch::X64)
            .expect("x64 must resolve");
        assert_eq!(x64.dart_sdk_arch.as_deref(), Some("x64"));
        assert_eq!(x64.sha256, "aaaa");

        let arm64 = manifest
            .resolve_stable(HostArch::Arm64)
            .expect("arm64 must resolve");
        assert_eq!(arm64.dart_sdk_arch.as_deref(), Some("arm64"));
        assert_eq!(arm64.sha256, "bbbb");
    }

    #[test]
    fn test_resolve_stable_falls_back_when_no_arch() {
        // Single stable release without `dart_sdk_arch` → still resolved for any arch.
        let manifest = FlutterReleaseManifest {
            base_url: "https://example.com".to_string(),
            current_stable_hash: None,
            releases: vec![FlutterRelease {
                version: "3.22.0".to_string(),
                channel: "stable".to_string(),
                archive: "stable/linux/flutter_linux_3.22.0-stable.tar.xz".to_string(),
                sha256: "cccc".to_string(),
                dart_sdk_arch: None, // no arch field — older manifest entry
            }],
        };

        // Both X64 and Arm64 should fall back to the single arch-less stable entry.
        let r_x64 = manifest
            .resolve_stable(HostArch::X64)
            .expect("must resolve even without arch field");
        assert_eq!(r_x64.sha256, "cccc");

        let r_arm64 = manifest
            .resolve_stable(HostArch::Arm64)
            .expect("must resolve even without arch field");
        assert_eq!(r_arm64.sha256, "cccc");
    }

    #[test]
    fn test_resolve_stable_returns_none_for_empty_manifest() {
        let manifest = FlutterReleaseManifest {
            base_url: "https://example.com".to_string(),
            current_stable_hash: None,
            releases: vec![],
        };
        assert!(manifest.resolve_stable(HostArch::X64).is_none());
    }

    #[test]
    fn test_resolve_stable_skips_non_stable_channels() {
        // manifest contains only a beta release → resolve_stable must return None
        let manifest = FlutterReleaseManifest {
            base_url: "https://example.com".to_string(),
            current_stable_hash: None,
            releases: vec![FlutterRelease {
                version: "3.25.0-0.1.pre".to_string(),
                channel: "beta".to_string(),
                archive: "beta/linux/flutter_linux_3.25.0-0.1.pre-beta.tar.xz".to_string(),
                sha256: "dddd".to_string(),
                dart_sdk_arch: Some("x64".to_string()),
            }],
        };
        assert!(manifest.resolve_stable(HostArch::X64).is_none());
    }

    #[test]
    fn test_install_method_is_copy() {
        // Ensure InstallMethod can be copied without move.
        let m = InstallMethod::GitClone;
        let _m2 = m; // copy
        assert_eq!(m, InstallMethod::GitClone);
    }

    #[test]
    fn test_download_progress_is_copy() {
        let p = DownloadProgress {
            received: 1024,
            total: Some(4096),
        };
        let _p2 = p; // copy
        assert_eq!(p.received, 1024);
    }

    // ── Phase 3 Android install type tests ────────────────────────────────────

    #[test]
    fn test_cmdline_tools_url_per_os() {
        assert!(cmdline_tools_url(HostPlatform::Linux, "123")
            .unwrap()
            .contains("commandlinetools-linux-123_latest.zip"));
        assert!(cmdline_tools_url(HostPlatform::MacOs, "123")
            .unwrap()
            .contains("-mac-"));
        assert!(cmdline_tools_url(HostPlatform::Windows, "123")
            .unwrap()
            .contains("-win-"));
        assert!(cmdline_tools_url(HostPlatform::Unknown, "123").is_none());
    }

    #[test]
    fn test_cmdline_tools_url_full_format() {
        let url = cmdline_tools_url(HostPlatform::Linux, "11076708").unwrap();
        assert_eq!(
            url,
            "https://dl.google.com/android/repository/commandlinetools-linux-11076708_latest.zip"
        );
        let url_mac = cmdline_tools_url(HostPlatform::MacOs, "11076708").unwrap();
        assert_eq!(
            url_mac,
            "https://dl.google.com/android/repository/commandlinetools-mac-11076708_latest.zip"
        );
        let url_win = cmdline_tools_url(HostPlatform::Windows, "11076708").unwrap();
        assert_eq!(
            url_win,
            "https://dl.google.com/android/repository/commandlinetools-win-11076708_latest.zip"
        );
    }

    #[test]
    fn test_sdkmanager_packages_api_36() {
        assert_eq!(
            sdkmanager_packages(36),
            vec![
                "platform-tools",
                "platforms;android-36",
                "build-tools;36.0.0",
                "cmdline-tools;latest"
            ]
        );
    }

    #[test]
    fn test_sdkmanager_packages_api_34() {
        let pkgs = sdkmanager_packages(34);
        assert_eq!(pkgs[0], "platform-tools");
        assert_eq!(pkgs[1], "platforms;android-34");
        assert_eq!(pkgs[2], "build-tools;34.0.0");
        assert_eq!(pkgs[3], "cmdline-tools;latest");
        assert_eq!(pkgs.len(), 4);
    }

    #[test]
    fn test_android_install_target_fields_accessible() {
        // Verify the struct fields are pub and the type compiles correctly.
        let target = AndroidInstallTarget {
            sdk_root: PathBuf::from("/home/user/Android/Sdk"),
            api_level: 36,
            cmdline_tools_build: DEFAULT_CMDLINE_TOOLS_BUILD.to_string(),
            jdk_path: None,
            platform: HostPlatform::Linux,
        };
        assert_eq!(target.api_level, 36);
        assert_eq!(target.cmdline_tools_build, DEFAULT_CMDLINE_TOOLS_BUILD);
        assert!(target.jdk_path.is_none());
        assert_eq!(target.platform, HostPlatform::Linux);
    }

    #[test]
    fn test_android_install_target_with_jdk_path() {
        let target = AndroidInstallTarget {
            sdk_root: PathBuf::from("/opt/android-sdk"),
            api_level: 35,
            cmdline_tools_build: "12345678".to_string(),
            jdk_path: Some(PathBuf::from("/usr/lib/jvm/java-21")),
            platform: HostPlatform::MacOs,
        };
        assert!(target.jdk_path.is_some());
        assert_eq!(
            target.jdk_path.unwrap(),
            PathBuf::from("/usr/lib/jvm/java-21")
        );
    }

    #[test]
    fn test_android_install_outcome_fields_accessible() {
        let outcome = AndroidInstallOutcome {
            sdk_root: PathBuf::from("/home/user/Android/Sdk"),
            packages_installed: sdkmanager_packages(36),
        };
        assert_eq!(outcome.packages_installed.len(), 4);
        assert!(outcome
            .packages_installed
            .contains(&"platform-tools".to_string()));
        assert!(outcome
            .packages_installed
            .contains(&"platforms;android-36".to_string()));
    }

    #[test]
    fn test_default_cmdline_tools_build_is_nonempty() {
        assert!(!DEFAULT_CMDLINE_TOOLS_BUILD.is_empty());
        // Must consist only of ASCII digits (valid build number).
        assert!(DEFAULT_CMDLINE_TOOLS_BUILD
            .chars()
            .all(|c| c.is_ascii_digit()));
    }
}
