//! # iOS / macOS Xcode + CocoaPods Probe
//!
//! Detect full Xcode IDE installation and CocoaPods for Apple-platform Flutter
//! development (iOS and macOS targets).
//!
//! **macOS-only**: returns an empty `Vec` on Linux and Windows (these components
//! do not exist on non-Apple hosts).  For [`HostPlatform::Unknown`], two
//! `ComponentStatus::Unknown` checks are emitted so that the component slots
//! remain consistent if ever rendered on an unrecognised host.
//!
//! ## Xcode probe (two-step)
//!
//! 1. `xcode-select -p` — the printed path must resolve to a full Xcode `.app`
//!    bundle (`Contents/Developer` path component). If it points only at
//!    `/Library/Developer/CommandLineTools` (CLT-only), full Xcode is absent.
//! 2. `xcodebuild -version` — must succeed and return a parseable version
//!    string. Failure (e.g. license not yet accepted) yields `Missing` with the
//!    reason in `detail`.
//!
//! ## CocoaPods probe
//!
//! `pod --version` — `Ok` with version detail; `Missing` on absence or error.
//!
//! Both sub-probes respect [`PROBE_TIMEOUT`] via `tokio::time::timeout`.

use std::process::Stdio;

use tokio::process::Command;

use super::super::types::{ComponentCheck, ComponentKind, ComponentStatus, HostPlatform};
use super::{strip_and_truncate, PROBE_TIMEOUT};

/// Detect full Xcode + CocoaPods for Apple-platform Flutter development
/// (iOS/macOS).
///
/// macOS-only: returns an empty `Vec` on Linux and Windows (the components
/// simply do not exist off-macOS). For [`HostPlatform::Unknown`], two
/// `ComponentStatus::Unknown` checks are returned so the slots are consistent
/// if ever rendered.
///
/// One probe pass produces two [`ComponentCheck`]s:
/// - [`ComponentKind::XcodeTools`] — full Xcode IDE detection
/// - [`ComponentKind::CocoaPods`] — CocoaPods presence
///
/// # Returns
///
/// - Empty `Vec` on Linux / Windows.
/// - Two `Unknown`-status checks on `HostPlatform::Unknown`.
/// - Two `Ok` or `Missing` checks on macOS, depending on probe results.
pub async fn check_ios(platform: &HostPlatform) -> Vec<ComponentCheck> {
    match platform {
        HostPlatform::Linux | HostPlatform::Windows => {
            // These components don't exist outside Apple platforms.
            Vec::new()
        }
        HostPlatform::Unknown => {
            // Unknown host — emit placeholder Unknown checks for consistency.
            vec![
                ComponentCheck {
                    kind: ComponentKind::XcodeTools,
                    status: ComponentStatus::Unknown,
                    detail: "Unknown platform — Xcode check skipped".to_string(),
                },
                ComponentCheck {
                    kind: ComponentKind::CocoaPods,
                    status: ComponentStatus::Unknown,
                    detail: "Unknown platform — CocoaPods check skipped".to_string(),
                },
            ]
        }
        HostPlatform::MacOs => {
            // Run both sub-probes concurrently; each has its own PROBE_TIMEOUT.
            let (xcode_check, cocoapods_check) =
                tokio::join!(probe_xcode_tools(), probe_cocoapods());
            vec![xcode_check, cocoapods_check]
        }
    }
}

/// Probe full Xcode IDE installation.
///
/// Two-step detection:
/// 1. `xcode-select -p` — path must be under a `Xcode.app` bundle
///    (`Contents/Developer` in the path), not just the CLT path.
/// 2. `xcodebuild -version` — must succeed and return a version string.
///
/// Returns `Missing` for CLT-only setups or when `xcodebuild -version` fails
/// (e.g. license not yet accepted). The `detail` field carries the reason.
async fn probe_xcode_tools() -> ComponentCheck {
    // Step 1: check what xcode-select resolves to.
    let select_path = probe_xcode_select_path().await;

    match select_path {
        XcodeSelectResult::NotFound => {
            return ComponentCheck {
                kind: ComponentKind::XcodeTools,
                status: ComponentStatus::Missing,
                detail: "xcode-select not found — Xcode or CLT not installed".to_string(),
            };
        }
        XcodeSelectResult::CltOnly(path) => {
            return ComponentCheck {
                kind: ComponentKind::XcodeTools,
                status: ComponentStatus::Missing,
                detail: format!(
                    "Only Xcode Command Line Tools found ({}). Install full Xcode from the App Store.",
                    path
                ),
            };
        }
        XcodeSelectResult::Unknown => {
            return ComponentCheck {
                kind: ComponentKind::XcodeTools,
                status: ComponentStatus::Unknown,
                detail: "xcode-select -p timed out or failed".to_string(),
            };
        }
        XcodeSelectResult::FullXcode => {
            // Full Xcode path found — proceed to step 2 (xcodebuild -version).
        }
    }

    // Step 2: run xcodebuild -version to verify license accepted + get version.
    probe_xcodebuild_version().await
}

/// Result of parsing the `xcode-select -p` output.
enum XcodeSelectResult {
    /// `xcode-select` binary not found on PATH.
    NotFound,
    /// Points to CLT-only: `/Library/Developer/CommandLineTools` or similar.
    CltOnly(String),
    /// Points to a full `Xcode.app` bundle path (contains `Contents/Developer`).
    /// The path is consumed by the CLT check; we carry the unit form here since
    /// the caller proceeds to `xcodebuild -version` for the version detail.
    FullXcode,
    /// Timed out or unexpected error.
    Unknown,
}

/// Run `xcode-select -p` and classify the returned path.
///
/// A pure-path classifier (`is_full_xcode_path`) is extracted for unit-testability.
async fn probe_xcode_select_path() -> XcodeSelectResult {
    let result = tokio::time::timeout(PROBE_TIMEOUT, async {
        Command::new("xcode-select")
            .arg("-p")
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
            let path = raw.trim().to_string();
            if path.is_empty() {
                return XcodeSelectResult::Unknown;
            }
            if is_full_xcode_path(&path) {
                XcodeSelectResult::FullXcode
            } else {
                XcodeSelectResult::CltOnly(path)
            }
        }
        Ok(Ok(_)) => XcodeSelectResult::CltOnly(String::new()),
        Ok(Err(e)) if e.kind() == std::io::ErrorKind::NotFound => XcodeSelectResult::NotFound,
        _ => XcodeSelectResult::Unknown,
    }
}

/// Return `true` when `path` points inside a full `Xcode.app` bundle.
///
/// Full Xcode installs `xcode-select` to point at something like:
/// `/Applications/Xcode.app/Contents/Developer`
///
/// Versioned Xcode installs may use paths like:
/// `/Applications/Xcode_15.2.app/Contents/Developer`
///
/// CLT-only installs point at:
/// `/Library/Developer/CommandLineTools`
///
/// The discriminating marker is an `Xcode` (case-sensitive) component in a
/// `.app` bundle path combined with `Contents/Developer`. Specifically, we
/// require the path to contain `.app/Contents/Developer` and to have `Xcode`
/// somewhere in a bundle-name position (before `.app/`).
///
/// This is a pure function used by both the async probe and unit tests.
pub(super) fn is_full_xcode_path(path: &str) -> bool {
    // The path must contain ".app/Contents/Developer" (the canonical inner path
    // of any Xcode app bundle) AND the app bundle name must start with "Xcode"
    // (e.g. "Xcode.app", "Xcode_15.2.app", "Xcode-15.2.app").
    let marker = ".app/Contents/Developer";
    if let Some(app_pos) = path.find(marker) {
        // Find the start of the bundle name: scan backward from `.app` to `/`.
        let bundle_prefix = &path[..app_pos];
        let bundle_name_start = bundle_prefix.rfind('/').map_or(0, |i| i + 1);
        let bundle_name = &bundle_prefix[bundle_name_start..];
        bundle_name.starts_with("Xcode")
    } else {
        false
    }
}

/// Run `xcodebuild -version` and return a `ComponentCheck` for `XcodeTools`.
///
/// On success, returns `Ok` with the version string in `detail`.
/// On failure (license not accepted, etc.), returns `Missing` with the reason.
async fn probe_xcodebuild_version() -> ComponentCheck {
    let result = tokio::time::timeout(PROBE_TIMEOUT, async {
        Command::new("xcodebuild")
            .arg("-version")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null())
            .output()
            .await
    })
    .await;

    match result {
        Ok(Ok(output)) if output.status.success() => {
            let raw = String::from_utf8_lossy(&output.stdout);
            let version_line = raw.lines().next().unwrap_or("").trim();
            let detail = if version_line.is_empty() {
                "Xcode (version unknown)".to_string()
            } else {
                strip_and_truncate(version_line)
            };
            ComponentCheck {
                kind: ComponentKind::XcodeTools,
                status: ComponentStatus::Ok,
                detail,
            }
        }
        Ok(Ok(output)) => {
            // Non-zero exit: license not accepted or Xcode broken.
            let stderr_raw = String::from_utf8_lossy(&output.stderr);
            let stdout_raw = String::from_utf8_lossy(&output.stdout);
            // Prefer stderr for diagnostic messages (license prompts appear there).
            let reason = if !stderr_raw.trim().is_empty() {
                strip_and_truncate(stderr_raw.lines().next().unwrap_or("").trim())
            } else if !stdout_raw.trim().is_empty() {
                strip_and_truncate(stdout_raw.lines().next().unwrap_or("").trim())
            } else {
                format!("xcodebuild -version exited with status {}", output.status)
            };
            ComponentCheck {
                kind: ComponentKind::XcodeTools,
                status: ComponentStatus::Missing,
                detail: reason,
            }
        }
        Ok(Err(e)) if e.kind() == std::io::ErrorKind::NotFound => ComponentCheck {
            kind: ComponentKind::XcodeTools,
            status: ComponentStatus::Missing,
            detail: "xcodebuild not found — full Xcode not installed".to_string(),
        },
        Ok(Err(e)) => ComponentCheck {
            kind: ComponentKind::XcodeTools,
            status: ComponentStatus::Error,
            detail: format!("xcodebuild probe failed: {e}"),
        },
        Err(_) => ComponentCheck {
            kind: ComponentKind::XcodeTools,
            status: ComponentStatus::Error,
            detail: "xcodebuild -version timed out".to_string(),
        },
    }
}

/// Probe CocoaPods via `pod --version`.
///
/// Returns `Ok` with the version string on success; `Missing` when `pod` is not
/// found; `Error` on timeout or unexpected failure.
async fn probe_cocoapods() -> ComponentCheck {
    let result = tokio::time::timeout(PROBE_TIMEOUT, async {
        Command::new("pod")
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
            let version = strip_and_truncate(raw.trim());
            let detail = if version.is_empty() {
                "CocoaPods (version unknown)".to_string()
            } else {
                version
            };
            ComponentCheck {
                kind: ComponentKind::CocoaPods,
                status: ComponentStatus::Ok,
                detail,
            }
        }
        Ok(Ok(_)) => ComponentCheck {
            kind: ComponentKind::CocoaPods,
            status: ComponentStatus::Missing,
            detail: "pod --version failed — CocoaPods may not be installed correctly".to_string(),
        },
        Ok(Err(e)) if e.kind() == std::io::ErrorKind::NotFound => ComponentCheck {
            kind: ComponentKind::CocoaPods,
            status: ComponentStatus::Missing,
            detail: "CocoaPods not found (pod not on PATH). Install via: brew install cocoapods"
                .to_string(),
        },
        Ok(Err(e)) => ComponentCheck {
            kind: ComponentKind::CocoaPods,
            status: ComponentStatus::Error,
            detail: format!("pod probe failed: {e}"),
        },
        Err(_) => ComponentCheck {
            kind: ComponentKind::CocoaPods,
            status: ComponentStatus::Error,
            detail: "pod --version timed out".to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Smoke test: never panics ──────────────────────────────────────────────

    #[tokio::test]
    async fn test_check_ios_never_panics() {
        let platform = HostPlatform::detect();
        let _ = check_ios(&platform).await;
    }

    // ── Non-macOS returns empty Vec ───────────────────────────────────────────

    #[tokio::test]
    async fn test_check_ios_non_macos_returns_empty_linux() {
        let result = check_ios(&HostPlatform::Linux).await;
        assert!(
            result.is_empty(),
            "check_ios on Linux must return empty Vec, got {} checks",
            result.len()
        );
    }

    #[tokio::test]
    async fn test_check_ios_non_macos_returns_empty_windows() {
        let result = check_ios(&HostPlatform::Windows).await;
        assert!(
            result.is_empty(),
            "check_ios on Windows must return empty Vec, got {} checks",
            result.len()
        );
    }

    // ── Unknown platform returns two Unknown checks ───────────────────────────

    #[tokio::test]
    async fn test_check_ios_unknown_platform_returns_unknown_checks() {
        let result = check_ios(&HostPlatform::Unknown).await;
        assert_eq!(
            result.len(),
            2,
            "check_ios on Unknown must return exactly 2 checks, got {}",
            result.len()
        );
        assert!(
            result.iter().any(
                |c| c.kind == ComponentKind::XcodeTools && c.status == ComponentStatus::Unknown
            ),
            "expected XcodeTools Unknown check; got: {:?}",
            result
        );
        assert!(
            result.iter().any(|c| c.kind == ComponentKind::CocoaPods
                && c.status == ComponentStatus::Unknown),
            "expected CocoaPods Unknown check; got: {:?}",
            result
        );
    }

    // ── macOS presence test (macOS only) ──────────────────────────────────────

    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn test_check_ios_macos_returns_two_components() {
        let result = check_ios(&HostPlatform::MacOs).await;
        assert_eq!(
            result.len(),
            2,
            "check_ios on macOS must return exactly 2 checks, got {}",
            result.len()
        );
        assert!(
            result.iter().any(|c| c.kind == ComponentKind::XcodeTools),
            "must contain XcodeTools component; got: {:?}",
            result
        );
        assert!(
            result.iter().any(|c| c.kind == ComponentKind::CocoaPods),
            "must contain CocoaPods component; got: {:?}",
            result
        );
        // Each component must have a non-empty detail string.
        for check in &result {
            assert!(
                !check.detail.is_empty(),
                "detail must be non-empty for {:?}",
                check.kind
            );
        }
    }

    // ── Pure path-shape classifier ────────────────────────────────────────────

    /// Verify that `is_full_xcode_path` correctly discriminates between
    /// full-Xcode and CLT-only paths without requiring a real Xcode install.
    #[test]
    fn test_is_full_xcode_path_accepts_full_xcode_bundle() {
        assert!(
            is_full_xcode_path("/Applications/Xcode.app/Contents/Developer"),
            "canonical full-Xcode path must be accepted"
        );
        assert!(
            is_full_xcode_path("/Applications/Xcode_15.2.app/Contents/Developer"),
            "versioned Xcode app bundle must be accepted"
        );
        assert!(
            is_full_xcode_path("/Volumes/External/Applications/Xcode.app/Contents/Developer"),
            "non-standard mount path must be accepted"
        );
    }

    #[test]
    fn test_is_full_xcode_path_rejects_clt_only_path() {
        assert!(
            !is_full_xcode_path("/Library/Developer/CommandLineTools"),
            "CLT-only path must be rejected"
        );
        assert!(
            !is_full_xcode_path("/usr/local/Developer"),
            "arbitrary developer path without Xcode.app must be rejected"
        );
        assert!(!is_full_xcode_path(""), "empty path must be rejected");
    }

    #[test]
    fn test_is_full_xcode_path_requires_contents_developer() {
        // Has Xcode.app but no Contents/Developer — should be rejected.
        assert!(
            !is_full_xcode_path("/Applications/Xcode.app"),
            "path with Xcode.app but no Contents/Developer must be rejected"
        );
        // Has Contents/Developer but no Xcode.app — should be rejected.
        assert!(
            !is_full_xcode_path("/Some/Other.app/Contents/Developer"),
            "path with Contents/Developer but no Xcode.app must be rejected"
        );
    }
}
