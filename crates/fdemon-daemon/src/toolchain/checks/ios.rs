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
//! ## Xcode probe (five-gate sequence)
//!
//! 1. `xcode-select -p` — the printed path must resolve to a full Xcode `.app`
//!    bundle (`Contents/Developer` path component). A non-zero exit code means
//!    no active developer directory at all (not CLT). If it points only at
//!    `/Library/Developer/CommandLineTools` (CLT-only), full Xcode is absent.
//! 2. `xcodebuild -version` — must succeed and return a parseable version
//!    string. The version detail is forwarded to subsequent gates.
//! 3. `xcodebuild -license check` — exit 0 = license accepted, non-zero = not
//!    accepted. Read-only; does **not** open a pager.
//! 4. `xcodebuild -checkFirstLaunchStatus` — exit 0 = first-launch components
//!    present, non-zero = run `-runFirstLaunch` to complete the setup.
//! 5. `xcrun simctl list devices booted` — exit 0 = simctl is reachable.
//!    Filtered to `booted` devices to avoid the known `simctl list` hang.
//!
//! `XcodeTools = Ok` is returned **only** when all five gates pass.  If any gate
//! fails, `Missing` is returned with a `detail` naming the specific gate and its
//! sudo remediation command.  All three usability gates run regardless of each
//! other's outcome (no short-circuit), mirroring Flutter's own validator.
//!
//! ## CocoaPods probe
//!
//! `pod --version` — `Ok` with version detail; `Missing` on absence or error.
//!
//! All spawned processes use `.kill_on_drop(true)` so that a hung
//! `xcodebuild`/`pod`/`xcrun` is killed on [`PROBE_TIMEOUT`] instead of being
//! orphaned.  All probes use `stdin(Stdio::null())` — they are read-only and
//! non-interactive.

use std::process::Stdio;

use tokio::process::Command;

use super::super::types::{ComponentCheck, ComponentKind, ComponentStatus, HostPlatform};
use super::{strip_and_truncate, PROBE_TIMEOUT};

// ─── GateResult ──────────────────────────────────────────────────────────────

/// Outcome of one read-only Xcode usability gate.
///
/// Used by [`probe_xcode_license`], [`probe_xcode_first_launch`], and
/// [`probe_simctl`].  Kept private to this module; only [`classify_xcode_gates`]
/// consumes it.
#[derive(Debug, Clone, PartialEq, Eq)]
enum GateResult {
    /// Gate command ran and exited 0.
    Pass,
    /// Gate command ran and exited non-zero.
    Fail,
    /// Gate timed out or could not be spawned.
    Unknown,
}

// ─── Public entry point ───────────────────────────────────────────────────────

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

// ─── Xcode probe (five-gate sequence) ────────────────────────────────────────

/// Probe full Xcode IDE installation.
///
/// Five-gate detection sequence:
/// 1. `xcode-select -p` — path must be under a `Xcode.app` bundle
///    (`Contents/Developer` in the path). A non-zero exit means no active
///    developer tools, not CLT. A CLT-only path yields `Missing`.
/// 2. `xcodebuild -version` — must succeed and return a version string.
/// 3. `xcodebuild -license check` — license must be accepted (exit 0).
/// 4. `xcodebuild -checkFirstLaunchStatus` — first-launch must be complete.
/// 5. `xcrun simctl list devices booted` — simctl must be reachable.
///
/// `Ok` is returned only when **all five** gates pass.  Any failing gate
/// returns `Missing` with a `detail` naming the gate and its remediation.
/// Gates 3–5 all run regardless of each other's outcome (no short-circuit).
async fn probe_xcode_tools() -> ComponentCheck {
    // Gate 1: check what xcode-select resolves to.
    let select_path = probe_xcode_select_path().await;

    match select_path {
        XcodeSelectResult::NotFound => {
            return ComponentCheck {
                kind: ComponentKind::XcodeTools,
                status: ComponentStatus::Missing,
                detail: "xcode-select not found — Xcode or CLT not installed".to_string(),
            };
        }
        XcodeSelectResult::NoActiveTools => {
            return ComponentCheck {
                kind: ComponentKind::XcodeTools,
                status: ComponentStatus::Missing,
                detail:
                    "xcode-select reports no active developer directory — Xcode or CLT not installed"
                        .to_string(),
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
            // Full Xcode path found — proceed to gate 2 (xcodebuild -version).
        }
    }

    // Gate 2: run xcodebuild -version to verify Xcode is reachable + get version.
    let version_detail = match probe_xcodebuild_version_detail().await {
        Ok(v) => v,
        Err(check) => return check,
    };

    // Gates 3–5: run all three usability gates concurrently; collect results.
    let (license, first_launch, simctl) = tokio::join!(
        probe_xcode_license(),
        probe_xcode_first_launch(),
        probe_simctl()
    );

    // Classify the combined gate outcomes into a final ComponentCheck.
    classify_xcode_gates(&version_detail, license, first_launch, simctl)
}

/// Result of parsing the `xcode-select -p` output.
enum XcodeSelectResult {
    /// `xcode-select` binary not found on PATH.
    NotFound,
    /// `xcode-select -p` ran but exited non-zero — no active developer tools.
    NoActiveTools,
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
/// - Success (exit 0) with a path under `Xcode.app/Contents/Developer` →
///   `FullXcode`.
/// - Success with any other path → `CltOnly(strip_and_truncate(path))`.
/// - Non-zero exit → `NoActiveTools` (not CLT — the system has no active
///   developer directory at all).
/// - Spawn error `NotFound` → `NotFound`.
/// - Timeout or other error → `Unknown`.
///
/// A pure-path classifier (`is_full_xcode_path`) is extracted for unit-testability.
async fn probe_xcode_select_path() -> XcodeSelectResult {
    let result = tokio::time::timeout(PROBE_TIMEOUT, async {
        Command::new("xcode-select")
            .arg("-p")
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .stdin(Stdio::null())
            .kill_on_drop(true)
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
                XcodeSelectResult::CltOnly(strip_and_truncate(&path))
            }
        }
        // Non-zero exit: xcode-select ran but no active developer directory.
        Ok(Ok(_)) => XcodeSelectResult::NoActiveTools,
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

/// Run `xcodebuild -version` and return the version detail string on success,
/// or an early-exit `ComponentCheck` on failure.
///
/// This is a helper called by [`probe_xcode_tools`] between gate 1 and the
/// three usability gates.  Separating it from [`probe_xcodebuild_version_detail`]
/// lets the caller handle the error path with an early `return`.
async fn probe_xcodebuild_version_detail() -> Result<String, ComponentCheck> {
    let result = tokio::time::timeout(PROBE_TIMEOUT, async {
        Command::new("xcodebuild")
            .arg("-version")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null())
            .kill_on_drop(true)
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
            Ok(detail)
        }
        Ok(Ok(output)) => {
            // Non-zero exit: license not accepted, broken Xcode, etc.
            let stderr_raw = String::from_utf8_lossy(&output.stderr);
            let stdout_raw = String::from_utf8_lossy(&output.stdout);
            let reason = if !stderr_raw.trim().is_empty() {
                strip_and_truncate(stderr_raw.lines().next().unwrap_or("").trim())
            } else if !stdout_raw.trim().is_empty() {
                strip_and_truncate(stdout_raw.lines().next().unwrap_or("").trim())
            } else {
                format!("xcodebuild -version exited with status {}", output.status)
            };
            Err(ComponentCheck {
                kind: ComponentKind::XcodeTools,
                status: ComponentStatus::Missing,
                detail: reason,
            })
        }
        Ok(Err(e)) if e.kind() == std::io::ErrorKind::NotFound => Err(ComponentCheck {
            kind: ComponentKind::XcodeTools,
            status: ComponentStatus::Missing,
            detail: "xcodebuild not found — full Xcode not installed".to_string(),
        }),
        Ok(Err(e)) => Err(ComponentCheck {
            kind: ComponentKind::XcodeTools,
            status: ComponentStatus::Error,
            detail: format!("xcodebuild probe failed: {e}"),
        }),
        Err(_) => Err(ComponentCheck {
            kind: ComponentKind::XcodeTools,
            status: ComponentStatus::Error,
            detail: "xcodebuild -version timed out".to_string(),
        }),
    }
}

// ─── Usability gates ──────────────────────────────────────────────────────────

/// Run `xcodebuild -license check` and report whether the Xcode license has
/// been accepted.
///
/// Exit 0 = accepted (`Pass`). Non-zero = not accepted (`Fail`).
/// This is the read-only form of the license check — it does **not** open an
/// interactive pager.
async fn probe_xcode_license() -> GateResult {
    let result = tokio::time::timeout(PROBE_TIMEOUT, async {
        Command::new("xcodebuild")
            .args(["-license", "check"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .stdin(Stdio::null())
            .kill_on_drop(true)
            .output()
            .await
    })
    .await;

    match result {
        Ok(Ok(output)) if output.status.success() => GateResult::Pass,
        Ok(Ok(_)) => GateResult::Fail,
        _ => GateResult::Unknown,
    }
}

/// Run `xcodebuild -checkFirstLaunchStatus` and report whether the Xcode
/// first-launch component installation is complete.
///
/// Exit 0 = components present (`Pass`). Non-zero = run `-runFirstLaunch`
/// (`Fail`). Both are read-only; no sudo required.
async fn probe_xcode_first_launch() -> GateResult {
    let result = tokio::time::timeout(PROBE_TIMEOUT, async {
        Command::new("xcodebuild")
            .arg("-checkFirstLaunchStatus")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .stdin(Stdio::null())
            .kill_on_drop(true)
            .output()
            .await
    })
    .await;

    match result {
        Ok(Ok(output)) if output.status.success() => GateResult::Pass,
        Ok(Ok(_)) => GateResult::Fail,
        _ => GateResult::Unknown,
    }
}

/// Run `xcrun simctl list devices booted` and report whether simctl is
/// reachable.
///
/// The `booted` filter avoids the known `simctl list` hang on some Xcode
/// versions while still exercising the simctl path.  Exit 0 = reachable
/// (`Pass`). Non-zero or timeout = `Unknown` (or `Fail` for definitive
/// non-zero exits).
async fn probe_simctl() -> GateResult {
    let result = tokio::time::timeout(PROBE_TIMEOUT, async {
        Command::new("xcrun")
            .args(["simctl", "list", "devices", "booted"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .stdin(Stdio::null())
            .kill_on_drop(true)
            .output()
            .await
    })
    .await;

    match result {
        Ok(Ok(output)) if output.status.success() => GateResult::Pass,
        Ok(Ok(_)) => GateResult::Fail,
        _ => GateResult::Unknown,
    }
}

// ─── Pure gate classifier ─────────────────────────────────────────────────────

/// Combine the `xcodebuild -version` detail with the three usability gate
/// outcomes into a final [`ComponentCheck`] for [`ComponentKind::XcodeTools`].
///
/// # Rules (applied in precedence order)
///
/// 1. All three gates `Pass` → `Ok`, `detail = version_detail`.
/// 2. `license` is `Fail` → `Missing`, detail names the license gate and the
///    sudo remediation.
/// 3. `first_launch` is `Fail` → `Missing`, detail names the first-launch gate.
/// 4. `simctl` is `Fail` → `Missing`, detail names the simctl gate.
/// 5. Any gate is `Unknown` (timeout / spawn error) → `Missing`, detail says
///    "could not verify <gate>".
///
/// Precedence: a `Fail` takes priority over `Unknown` for the same gate;
/// `license` takes priority over `first_launch` which takes priority over
/// `simctl` when multiple gates are non-passing.  All three gates were already
/// run concurrently before this call (no short-circuit in the probe step).
///
/// This is a **pure function** with no I/O, runnable in unit tests on any host.
fn classify_xcode_gates(
    version_detail: &str,
    license: GateResult,
    first_launch: GateResult,
    simctl: GateResult,
) -> ComponentCheck {
    // All three Pass → Ok.
    if license == GateResult::Pass && first_launch == GateResult::Pass && simctl == GateResult::Pass
    {
        return ComponentCheck {
            kind: ComponentKind::XcodeTools,
            status: ComponentStatus::Ok,
            detail: version_detail.to_string(),
        };
    }

    // License gate takes highest precedence.
    if license == GateResult::Fail {
        return ComponentCheck {
            kind: ComponentKind::XcodeTools,
            status: ComponentStatus::Missing,
            detail: format!(
                "{} — license not accepted; run: sudo xcodebuild -license accept",
                version_detail
            ),
        };
    }

    // First-launch gate.
    if first_launch == GateResult::Fail {
        return ComponentCheck {
            kind: ComponentKind::XcodeTools,
            status: ComponentStatus::Missing,
            detail: format!(
                "{} — first-launch incomplete; run: sudo xcodebuild -runFirstLaunch",
                version_detail
            ),
        };
    }

    // Simctl gate.
    if simctl == GateResult::Fail {
        return ComponentCheck {
            kind: ComponentKind::XcodeTools,
            status: ComponentStatus::Missing,
            detail: format!(
                "{} — simctl unreachable; run: sudo xcodebuild -runFirstLaunch",
                version_detail
            ),
        };
    }

    // At least one gate is Unknown (timed out / spawn error).  Report the first
    // unknown gate so the message is actionable.
    let gate_name = if license == GateResult::Unknown {
        "license check"
    } else if first_launch == GateResult::Unknown {
        "first-launch check"
    } else {
        "simctl check"
    };

    ComponentCheck {
        kind: ComponentKind::XcodeTools,
        status: ComponentStatus::Missing,
        detail: format!(
            "{} — could not verify {}; re-run preflight or check Xcode manually",
            version_detail, gate_name
        ),
    }
}

// ─── CocoaPods probe ──────────────────────────────────────────────────────────

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
            .kill_on_drop(true)
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

// ─── Tests ────────────────────────────────────────────────────────────────────

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

    // ── classify_xcode_gates — pure-function unit tests ───────────────────────

    /// All three gates Pass → Ok, detail equals the version string.
    #[test]
    fn test_classify_xcode_gates_all_pass_is_ok() {
        let version = "Xcode 15.2";
        let check = classify_xcode_gates(
            version,
            GateResult::Pass,
            GateResult::Pass,
            GateResult::Pass,
        );
        assert_eq!(
            check.status,
            ComponentStatus::Ok,
            "all-pass must yield Ok; got {:?}",
            check.status
        );
        assert_eq!(
            check.detail, version,
            "Ok detail must equal version_detail exactly"
        );
        assert_eq!(check.kind, ComponentKind::XcodeTools);
    }

    /// License Fail → Missing with license detail.
    #[test]
    fn test_classify_xcode_gates_license_fail_is_missing_with_license_detail() {
        let version = "Xcode 15.2";
        let check = classify_xcode_gates(
            version,
            GateResult::Fail,
            GateResult::Pass,
            GateResult::Pass,
        );
        assert_eq!(check.status, ComponentStatus::Missing);
        assert!(
            check.detail.contains("license not accepted"),
            "detail must mention license; got: {}",
            check.detail
        );
        assert!(
            check.detail.contains("sudo xcodebuild -license accept"),
            "detail must contain the remediation command; got: {}",
            check.detail
        );
        assert!(
            check.detail.contains(version),
            "detail must include version; got: {}",
            check.detail
        );
    }

    /// first_launch Fail → Missing with first-launch detail.
    #[test]
    fn test_classify_xcode_gates_first_launch_fail_is_missing_with_runfirstlaunch_detail() {
        let version = "Xcode 15.2";
        let check = classify_xcode_gates(
            version,
            GateResult::Pass,
            GateResult::Fail,
            GateResult::Pass,
        );
        assert_eq!(check.status, ComponentStatus::Missing);
        assert!(
            check.detail.contains("first-launch incomplete"),
            "detail must mention first-launch; got: {}",
            check.detail
        );
        assert!(
            check.detail.contains("sudo xcodebuild -runFirstLaunch"),
            "detail must contain the remediation command; got: {}",
            check.detail
        );
        assert!(
            check.detail.contains(version),
            "detail must include version; got: {}",
            check.detail
        );
    }

    /// simctl Fail → Missing with simctl detail.
    #[test]
    fn test_classify_xcode_gates_simctl_fail_is_missing_with_simctl_detail() {
        let version = "Xcode 15.2";
        let check = classify_xcode_gates(
            version,
            GateResult::Pass,
            GateResult::Pass,
            GateResult::Fail,
        );
        assert_eq!(check.status, ComponentStatus::Missing);
        assert!(
            check.detail.contains("simctl unreachable"),
            "detail must mention simctl; got: {}",
            check.detail
        );
        assert!(
            check.detail.contains(version),
            "detail must include version; got: {}",
            check.detail
        );
    }

    /// An Unknown gate must never yield Ok.
    #[test]
    fn test_classify_xcode_gates_unknown_gate_is_non_ok() {
        let version = "Xcode 15.2";

        // Unknown license gate.
        let check = classify_xcode_gates(
            version,
            GateResult::Unknown,
            GateResult::Pass,
            GateResult::Pass,
        );
        assert_ne!(
            check.status,
            ComponentStatus::Ok,
            "unknown license gate must not yield Ok; got {:?}",
            check.status
        );
        assert!(
            check.detail.contains("could not verify"),
            "Unknown-gate detail must say 'could not verify'; got: {}",
            check.detail
        );

        // Unknown first_launch gate.
        let check = classify_xcode_gates(
            version,
            GateResult::Pass,
            GateResult::Unknown,
            GateResult::Pass,
        );
        assert_ne!(check.status, ComponentStatus::Ok);
        assert!(check.detail.contains("could not verify"));

        // Unknown simctl gate.
        let check = classify_xcode_gates(
            version,
            GateResult::Pass,
            GateResult::Pass,
            GateResult::Unknown,
        );
        assert_ne!(check.status, ComponentStatus::Ok);
        assert!(check.detail.contains("could not verify"));
    }

    /// License Fail takes precedence over first_launch Fail.
    #[test]
    fn test_classify_xcode_gates_license_precedence_over_first_launch() {
        let version = "Xcode 15.2";
        let check = classify_xcode_gates(
            version,
            GateResult::Fail,
            GateResult::Fail,
            GateResult::Pass,
        );
        assert_eq!(check.status, ComponentStatus::Missing);
        assert!(
            check.detail.contains("license not accepted"),
            "license must take precedence; got: {}",
            check.detail
        );
    }

    /// first_launch Fail takes precedence over simctl Fail.
    #[test]
    fn test_classify_xcode_gates_first_launch_precedence_over_simctl() {
        let version = "Xcode 15.2";
        let check = classify_xcode_gates(
            version,
            GateResult::Pass,
            GateResult::Fail,
            GateResult::Fail,
        );
        assert_eq!(check.status, ComponentStatus::Missing);
        assert!(
            check.detail.contains("first-launch incomplete"),
            "first_launch must take precedence over simctl; got: {}",
            check.detail
        );
    }

    /// License Fail takes priority over Unknown for the same gate.
    #[test]
    fn test_classify_xcode_gates_fail_takes_priority_over_unknown_in_same_position() {
        let version = "Xcode 15.2";
        // License Fail + first_launch Unknown: license Fail wins.
        let check = classify_xcode_gates(
            version,
            GateResult::Fail,
            GateResult::Unknown,
            GateResult::Pass,
        );
        assert_eq!(check.status, ComponentStatus::Missing);
        assert!(
            check.detail.contains("license not accepted"),
            "license Fail must beat first_launch Unknown; got: {}",
            check.detail
        );
    }

    /// All Unknown → Missing (never Ok).
    #[test]
    fn test_classify_xcode_gates_all_unknown_is_non_ok() {
        let check = classify_xcode_gates(
            "Xcode 15.2",
            GateResult::Unknown,
            GateResult::Unknown,
            GateResult::Unknown,
        );
        assert_ne!(
            check.status,
            ComponentStatus::Ok,
            "all-Unknown must not yield Ok"
        );
    }
}
