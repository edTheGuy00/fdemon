//! # Windows Visual Studio C++ Probe
//!
//! Detect Visual Studio with the "Desktop development with C++" workload for
//! Windows-desktop Flutter development.
//!
//! **Windows-only**: returns an empty `Vec` on Linux/macOS (these components do
//! not exist on non-Windows hosts). For [`HostPlatform::Unknown`], one
//! `ComponentStatus::Unknown` check is emitted so the component slot remains
//! consistent if ever rendered on an unrecognised host.
//!
//! ## Detection strategy (two-gate vswhere query)
//!
//! 1. **Resolve `vswhere.exe`**: checked at its fixed, Microsoft-documented
//!    location (`%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe`)
//!    first, then via `which::which("vswhere")` as fallback.
//! 2. **Gate 1 — any VS instance**: `vswhere -products * -latest -format json -utf8`
//!    (`-products *` is required so Build Tools SKUs count).
//! 3. **Gate 2 — C++ workload**: same args plus
//!    `-requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64
//!    Microsoft.VisualStudio.Component.VC.CMake.Project`
//!    (AND semantics — both components required per Flutter Windows-setup docs).
//!
//! All spawned processes use `.kill_on_drop(true)` and `stdin(Stdio::null())`.
//! Timeouts are respected via [`PROBE_TIMEOUT`].

use std::process::Stdio;

use tokio::process::Command;

use super::super::types::{ComponentCheck, ComponentKind, ComponentStatus, HostPlatform};
use super::{strip_and_truncate, PROBE_TIMEOUT};

// ─── Prefix constant ─────────────────────────────────────────────────────────

/// Stable prefix used in the gate-1-hit / gate-2-miss detail string.
///
/// **Shared constant:** re-exported through `checks/mod.rs` → `toolchain/mod.rs`
/// → `fdemon-daemon/lib.rs` so `fdemon-app` can import it directly instead of
/// duplicating it. `windows_guided_commands` in
/// `fdemon-app/src/install_wizard/state.rs` branches on this exact prefix to
/// emit "modify the existing install" guidance — a test in this module asserts
/// the classifier's output starts with this prefix when gate 1 hits but gate 2
/// misses.
pub const VS_FOUND_PREFIX: &str = "Visual Studio found";

// ─── Public entry point ───────────────────────────────────────────────────────

/// Detect Visual Studio with the "Desktop development with C++" workload for
/// Windows-desktop Flutter development.
///
/// Windows-only: returns an empty `Vec` on Linux and macOS (the component
/// simply does not exist off-Windows). For [`HostPlatform::Unknown`], one
/// `ComponentStatus::Unknown` check is returned so the slot is consistent if
/// ever rendered.
///
/// One probe pass produces one [`ComponentCheck`]:
/// - [`ComponentKind::VisualStudioCpp`]
///
/// # Returns
///
/// - Empty `Vec` on Linux / macOS.
/// - One `Unknown`-status check on `HostPlatform::Unknown`.
/// - One `Ok` or `Missing` check on Windows, depending on probe results.
pub async fn check_windows(platform: &HostPlatform) -> Vec<ComponentCheck> {
    match platform {
        HostPlatform::Linux | HostPlatform::MacOs => {
            // These components don't exist outside Windows.
            Vec::new()
        }
        HostPlatform::Unknown => {
            // Unknown host — emit placeholder Unknown check for consistency.
            vec![ComponentCheck {
                kind: ComponentKind::VisualStudioCpp,
                status: ComponentStatus::Unknown,
                detail: "Unknown platform — Visual Studio check skipped".to_string(),
            }]
        }
        HostPlatform::Windows => {
            vec![probe_visual_studio_cpp().await]
        }
    }
}

// ─── Windows probe ────────────────────────────────────────────────────────────

/// Resolve the path to `vswhere.exe`.
///
/// Checks the fixed Microsoft-documented location first:
/// `%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe`
///
/// Falls back to `which::which("vswhere")` when the env var is unset or the
/// file is absent at the standard path.
fn resolve_vswhere() -> Option<std::path::PathBuf> {
    // Primary: the fixed, Microsoft-documented location.
    if let Ok(pf86) = std::env::var("ProgramFiles(x86)") {
        let fixed = std::path::Path::new(&pf86)
            .join("Microsoft Visual Studio")
            .join("Installer")
            .join("vswhere.exe");
        if fixed.exists() {
            return Some(fixed);
        }
    }

    // Fallback: PATH lookup (e.g., custom installs or CI environments).
    which::which("vswhere").ok()
}

/// Run `vswhere.exe` with the given extra arguments and return the stdout on
/// success, or `None` on timeout / spawn error / non-zero exit.
///
/// Base args: `-products * -latest -format json -utf8`
/// The `-products *` flag is required so that Visual Studio Build Tools SKUs
/// are included; without it, vswhere omits them.
async fn run_vswhere(vswhere_path: &std::path::Path, extra_args: &[&str]) -> Option<String> {
    let mut cmd = Command::new(vswhere_path);
    cmd.args(["-products", "*", "-latest", "-format", "json", "-utf8"])
        .args(extra_args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .stdin(Stdio::null())
        .kill_on_drop(true);

    let result = tokio::time::timeout(PROBE_TIMEOUT, cmd.output()).await;

    match result {
        Ok(Ok(output)) if output.status.success() => {
            Some(String::from_utf8_lossy(&output.stdout).into_owned())
        }
        // Non-zero exit or I/O error → treat as a miss.
        Ok(Ok(_)) | Ok(Err(_)) => None,
        // Timeout.
        Err(_) => None,
    }
}

/// Run the two-gate vswhere probe and return a single `VisualStudioCpp` check.
///
/// Called only on Windows; assumes the platform is `Windows`.
async fn probe_visual_studio_cpp() -> ComponentCheck {
    // Step 0: resolve vswhere.exe.
    let vswhere_path = match resolve_vswhere() {
        Some(p) => p,
        None => {
            return ComponentCheck {
                kind: ComponentKind::VisualStudioCpp,
                status: ComponentStatus::Missing,
                detail: "Visual Studio not found (vswhere.exe not present)".to_string(),
            }
        }
    };

    // Gate 1 (any VS instance) and Gate 2 (C++ workload) are independent —
    // run them concurrently, mirroring `check_ios` in checks/ios.rs.
    let (gate1_result, gate2_result) = tokio::join!(
        run_vswhere(&vswhere_path, &[]),
        run_vswhere(
            &vswhere_path,
            &[
                "-requires",
                "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
                "Microsoft.VisualStudio.Component.VC.CMake.Project",
            ],
        ),
    );
    let gate1_json = gate1_result.unwrap_or_default();
    let gate2_json = gate2_result.unwrap_or_default();

    classify_vswhere_gates(&gate1_json, &gate2_json)
}

// ─── Pure classifier ─────────────────────────────────────────────────────────

/// Minimal struct for deserialising one vswhere JSON array entry.
///
/// Only the fields we read are listed; additional vswhere fields are silently
/// ignored by serde's default behaviour.
#[derive(serde::Deserialize)]
struct VswhereEntry {
    #[serde(rename = "displayName", default)]
    display_name: String,
    #[serde(rename = "installationVersion", default)]
    installation_version: String,
}

/// Classify two vswhere JSON outputs into a single [`ComponentCheck`].
///
/// This is a **pure function** with no I/O; all decision logic lives here so
/// tests can run on any host (including Linux CI).
///
/// # Rules
///
/// - **Gate 2 hit** (non-empty valid JSON array) → `Ok`, detail
///   `"<displayName> <installationVersion>"`.
/// - **Gate 1 hit, gate 2 miss** → `Missing`, detail starts with
///   [`VS_FOUND_PREFIX`]:
///   `"Visual Studio found (<displayName>), but the 'Desktop development with C++' workload is missing"`.
/// - **Both miss** (empty array / parse failure in gate 1 too) → `Missing`,
///   detail `"Visual Studio not found"`.
///
/// `strip_and_truncate` is applied to any text interpolated from vswhere output.
pub(crate) fn classify_vswhere_gates(gate1_json: &str, gate2_json: &str) -> ComponentCheck {
    // Try to parse gate-2 JSON first (full workload present).
    if let Some(entry) = parse_first_entry(gate2_json) {
        let detail = strip_and_truncate(&format!(
            "{} {}",
            entry.display_name, entry.installation_version
        ));
        return ComponentCheck {
            kind: ComponentKind::VisualStudioCpp,
            status: ComponentStatus::Ok,
            detail,
        };
    }

    // Gate 2 miss — check gate 1 to distinguish "VS found, workload missing"
    // from "VS not installed at all".
    if let Some(entry) = parse_first_entry(gate1_json) {
        let display = strip_and_truncate(&entry.display_name);
        let detail = strip_and_truncate(&format!(
            "{VS_FOUND_PREFIX} ({display}), but the 'Desktop development with C++' workload is missing"
        ));
        return ComponentCheck {
            kind: ComponentKind::VisualStudioCpp,
            status: ComponentStatus::Missing,
            detail,
        };
    }

    // Both gates missed.
    ComponentCheck {
        kind: ComponentKind::VisualStudioCpp,
        status: ComponentStatus::Missing,
        detail: "Visual Studio not found".to_string(),
    }
}

/// Parse the first entry from a vswhere JSON array string.
///
/// Returns `None` when the input is empty, not valid JSON, not an array, or
/// when the array is empty.
fn parse_first_entry(json: &str) -> Option<VswhereEntry> {
    if json.trim().is_empty() {
        return None;
    }
    let entries: Vec<VswhereEntry> = serde_json::from_str(json).ok()?;
    entries.into_iter().next()
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Smoke test: never panics ──────────────────────────────────────────────

    #[tokio::test]
    async fn test_check_windows_never_panics() {
        let platform = HostPlatform::detect();
        let _ = check_windows(&platform).await;
    }

    // ── Non-Windows returns empty Vec ─────────────────────────────────────────

    #[tokio::test]
    async fn test_check_windows_non_windows_returns_empty_linux() {
        let result = check_windows(&HostPlatform::Linux).await;
        assert!(
            result.is_empty(),
            "check_windows on Linux must return empty Vec, got {} checks",
            result.len()
        );
    }

    #[tokio::test]
    async fn test_check_windows_non_windows_returns_empty_macos() {
        let result = check_windows(&HostPlatform::MacOs).await;
        assert!(
            result.is_empty(),
            "check_windows on macOS must return empty Vec, got {} checks",
            result.len()
        );
    }

    // ── Unknown platform returns one Unknown check ────────────────────────────

    #[tokio::test]
    async fn test_check_windows_unknown_platform_returns_unknown_check() {
        let result = check_windows(&HostPlatform::Unknown).await;
        assert_eq!(
            result.len(),
            1,
            "check_windows on Unknown must return exactly 1 check, got {}",
            result.len()
        );
        let check = &result[0];
        assert_eq!(
            check.kind,
            ComponentKind::VisualStudioCpp,
            "check kind must be VisualStudioCpp"
        );
        assert_eq!(
            check.status,
            ComponentStatus::Unknown,
            "check status must be Unknown; got {:?}",
            check.status
        );
    }

    // ── Windows presence test (Windows only) ──────────────────────────────────

    #[cfg(target_os = "windows")]
    #[tokio::test]
    async fn test_check_windows_windows_returns_one_component() {
        let result = check_windows(&HostPlatform::Windows).await;
        assert_eq!(
            result.len(),
            1,
            "check_windows on Windows must return exactly 1 check, got {}",
            result.len()
        );
        assert_eq!(
            result[0].kind,
            ComponentKind::VisualStudioCpp,
            "must contain VisualStudioCpp component; got: {:?}",
            result
        );
        assert!(
            !result[0].detail.is_empty(),
            "detail must be non-empty for VisualStudioCpp"
        );
    }

    // ── classify_vswhere_gates — pure-function fixture tests ──────────────────

    /// Realistic vswhere JSON for a full Visual Studio Community with the C++ workload.
    fn vs_community_json() -> &'static str {
        r#"[
            {
                "instanceId": "abc123",
                "displayName": "Visual Studio Community 2022",
                "installationVersion": "17.9.3",
                "installationPath": "C:\\Program Files\\Microsoft Visual Studio\\2022\\Community"
            }
        ]"#
    }

    /// Realistic vswhere JSON for VS Build Tools with the C++ workload.
    fn vs_build_tools_json() -> &'static str {
        r#"[
            {
                "instanceId": "def456",
                "displayName": "Visual Studio Build Tools 2022",
                "installationVersion": "17.9.4",
                "installationPath": "C:\\Program Files (x86)\\Microsoft Visual Studio\\2022\\BuildTools"
            }
        ]"#
    }

    /// VS Community present but the C++ workload is missing (gate 1 hit, gate 2 miss).
    fn vs_no_workload_json() -> &'static str {
        r#"[
            {
                "instanceId": "ghi789",
                "displayName": "Visual Studio Community 2022",
                "installationVersion": "17.9.3",
                "installationPath": "C:\\Program Files\\Microsoft Visual Studio\\2022\\Community"
            }
        ]"#
    }

    // Gate 2 hit (Community) → Ok with name + version detail.
    #[test]
    fn test_classify_gate2_hit_community_is_ok() {
        let check = classify_vswhere_gates(vs_community_json(), vs_community_json());
        assert_eq!(
            check.status,
            ComponentStatus::Ok,
            "gate-2 hit must yield Ok; got {:?}",
            check.status
        );
        assert!(
            check.detail.contains("Visual Studio Community 2022"),
            "detail must contain displayName; got: {}",
            check.detail
        );
        assert!(
            check.detail.contains("17.9.3"),
            "detail must contain version; got: {}",
            check.detail
        );
        assert_eq!(check.kind, ComponentKind::VisualStudioCpp);
    }

    // Gate 2 hit (Build Tools) → Ok with name + version detail.
    #[test]
    fn test_classify_gate2_hit_build_tools_is_ok() {
        let check = classify_vswhere_gates(vs_build_tools_json(), vs_build_tools_json());
        assert_eq!(check.status, ComponentStatus::Ok);
        assert!(check.detail.contains("Visual Studio Build Tools 2022"));
        assert!(check.detail.contains("17.9.4"));
    }

    // Gate 1 hit, gate 2 miss → Missing with VS_FOUND_PREFIX.
    #[test]
    fn test_classify_gate1_hit_gate2_miss_is_missing_with_found_prefix() {
        let check = classify_vswhere_gates(vs_no_workload_json(), "[]");
        assert_eq!(
            check.status,
            ComponentStatus::Missing,
            "gate-1-only must yield Missing; got {:?}",
            check.status
        );
        assert!(
            check.detail.starts_with(VS_FOUND_PREFIX),
            "detail must start with VS_FOUND_PREFIX '{VS_FOUND_PREFIX}'; got: {}",
            check.detail
        );
        assert!(
            check.detail.contains("Visual Studio Community 2022"),
            "detail must include displayName; got: {}",
            check.detail
        );
        assert!(
            check
                .detail
                .contains("'Desktop development with C++' workload is missing"),
            "detail must mention the missing workload; got: {}",
            check.detail
        );
    }

    // Both gates miss (empty array) → Missing "Visual Studio not found".
    #[test]
    fn test_classify_both_miss_empty_array_is_missing_not_found() {
        let check = classify_vswhere_gates("[]", "[]");
        assert_eq!(check.status, ComponentStatus::Missing);
        assert_eq!(check.detail, "Visual Studio not found");
    }

    // Both gates miss (malformed JSON) → Missing "Visual Studio not found".
    #[test]
    fn test_classify_both_miss_malformed_json_is_missing_not_found() {
        let check = classify_vswhere_gates("not valid json {{", "");
        assert_eq!(check.status, ComponentStatus::Missing);
        assert_eq!(check.detail, "Visual Studio not found");
    }

    // Both gates miss (empty string) → Missing "Visual Studio not found".
    #[test]
    fn test_classify_both_miss_empty_string_is_missing_not_found() {
        let check = classify_vswhere_gates("", "");
        assert_eq!(check.status, ComponentStatus::Missing);
        assert_eq!(check.detail, "Visual Studio not found");
    }

    // Over-long displayName is capped by strip_and_truncate.
    #[test]
    fn test_classify_over_long_display_name_is_truncated() {
        // Use the canonical constant from the checks module (checks/mod.rs).
        // `super` = windows module, `super::super` = checks module.
        use super::super::MAX_DETAIL_LEN;

        // Build a displayName longer than MAX_DETAIL_LEN.
        let long_name = "x".repeat(MAX_DETAIL_LEN + 100);
        let gate2_json =
            format!(r#"[{{"displayName": "{long_name}", "installationVersion": "17.0.0"}}]"#);
        let check = classify_vswhere_gates(&gate2_json, &gate2_json);
        assert_eq!(check.status, ComponentStatus::Ok);
        assert!(
            check.detail.len() <= MAX_DETAIL_LEN,
            "detail len {} exceeds MAX_DETAIL_LEN {}; detail: {}",
            check.detail.len(),
            MAX_DETAIL_LEN,
            check.detail
        );
    }

    // Prefix contract: gate-1-only classification detail starts with VS_FOUND_PREFIX.
    #[test]
    fn test_vs_found_prefix_contract() {
        // Gate 1 has VS, gate 2 has nothing → should have the prefix.
        let check = classify_vswhere_gates(vs_no_workload_json(), "[]");
        assert!(
            check.detail.starts_with(VS_FOUND_PREFIX),
            "classifier must start detail with the VS_FOUND_PREFIX constant '{VS_FOUND_PREFIX}'; \
             actual detail: {}",
            check.detail
        );
    }

    // Gate-2 hit with empty gate-1 is still Ok (gate-2 is always the winning gate).
    #[test]
    fn test_classify_gate2_hit_wins_regardless_of_gate1() {
        // Supplying gate2=community, gate1=[] means gate2 wins.
        let check = classify_vswhere_gates("[]", vs_community_json());
        assert_eq!(
            check.status,
            ComponentStatus::Ok,
            "gate-2 hit with empty gate-1 must still yield Ok"
        );
    }
}
