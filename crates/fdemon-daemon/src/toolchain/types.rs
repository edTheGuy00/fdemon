//! # Toolchain Report Types
//!
//! Data types for the structured, read-only Flutter toolchain preflight
//! diagnostics. All types are `Debug + Clone` and owned by `ToolchainReport`.

use std::path::PathBuf;

/// The overall result of a toolchain preflight scan.
///
/// Contains platform metadata, per-component statuses, and the parsed output
/// of `flutter doctor -v` when Flutter is available.
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
}
