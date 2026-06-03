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
}
