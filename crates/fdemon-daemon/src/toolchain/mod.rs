//! # Toolchain Preflight Subsystem
//!
//! Provides a single entry point [`run_preflight`] that runs a **read-only**
//! structured diagnosis of the Flutter toolchain and returns a
//! [`ToolchainReport`].
//!
//! ## Design Principles
//!
//! - **Read-only**: no installs, downloads, or file mutations.
//! - **Never fails**: `run_preflight` returns `ToolchainReport`, never `Err`.
//!   Failures are encoded as `ComponentStatus::Error` or `ComponentStatus::Missing`.
//! - **Concurrent**: independent checks run with `tokio::join!`.
//! - **Reuses existing SDK detection**: calls the same `find_flutter_sdk` used
//!   by the Flutter process spawner.
//!
//! ## Public API
//!
//! - [`run_preflight`] — top-level orchestrator
//! - [`ToolchainReport`], [`ComponentCheck`], [`ComponentStatus`], [`ComponentKind`]
//! - [`HostPlatform`], [`HostShell`]
//! - [`DoctorLine`], [`DoctorMarker`]

mod android_install;
mod checks;
mod doctor;
pub mod download;
pub mod flutter_install;
pub mod jdk;
pub mod path_config;
pub mod process_stream;
mod types;

pub use android_install::{
    install_android_tools, relocate_cmdline_tools, resolve_cmdline_tools_url,
};
pub use checks::resolve_android_sdk_root_path;
pub use download::{download_to_file, extract_archive, extract_tar_xz, extract_zip, verify_sha256};
pub use flutter_install::{
    archive_download_url, fetch_release_manifest, install_flutter, resolve_install_dir,
    InstallEvent,
};
pub use jdk::{configure_flutter_jdk_dir, resolve_jdk_home};
pub use path_config::{add_android_env, add_to_path, rc_file_for_shell, PathConfigOutcome};
pub use process_stream::{run_streaming, run_streaming_with_input};
pub use types::{
    cmdline_tools_url, sdkmanager_packages, AndroidInstallOutcome, AndroidInstallTarget,
    ComponentCheck, ComponentKind, ComponentStatus, DoctorLine, DoctorMarker, DownloadProgress,
    FlutterInstallOutcome, FlutterInstallTarget, FlutterRelease, FlutterReleaseManifest, HostArch,
    HostPlatform, HostShell, InstallMethod, ToolchainReport, DEFAULT_CMDLINE_TOOLS_BUILD,
};

use std::path::Path;

/// Run a full read-only toolchain preflight diagnostic.
///
/// Detects the host platform and shell, probes each toolchain component
/// concurrently, and (when Flutter is found) captures the output of
/// `flutter doctor -v`.
///
/// # Arguments
///
/// * `project_path` — Root of the Flutter project (forwarded to the SDK locator
///   for version-manager config discovery).
/// * `explicit_sdk_path` — Optional user-configured SDK path from
///   `.fdemon/config.toml` `[flutter] sdk_path`. Pass `None` to rely on
///   automatic detection.
///
/// # Returns
///
/// A [`ToolchainReport`] that is always populated — this function **never
/// panics** and never returns `Err`. All probe failures are encoded as
/// [`ComponentStatus::Error`] or [`ComponentStatus::Missing`] inside the
/// returned report.
pub async fn run_preflight(
    project_path: &Path,
    explicit_sdk_path: Option<&Path>,
) -> ToolchainReport {
    let platform = HostPlatform::detect();
    let shell = HostShell::detect();

    tracing::debug!(
        "Toolchain preflight starting (platform={}, shell={})",
        platform,
        shell
    );

    // Step 1: Flutter SDK check — sequential first because other checks may
    //         branch on whether we have a usable executable.
    let (flutter_check, maybe_exe) = checks::check_flutter(project_path, explicit_sdk_path).await;

    // Step 2: If Flutter was found, capture `flutter doctor -v` concurrently
    //         with the remaining component probes.
    let android_root = checks::android_sdk_root();
    let android_root_ref = android_root.as_ref();

    // Capture synchronous Android filesystem checks before entering async block
    // (they take immutable refs that cannot cross await points easily).
    let cmdline_check = checks::check_android_cmdline_tools(android_root_ref);
    let platform_check = checks::check_android_platform(android_root_ref);
    let build_tools_check = checks::check_android_build_tools(android_root_ref);
    let licenses_check = checks::check_android_licenses(android_root_ref);

    // Run async checks concurrently
    let (git_check, jdk_check, platform_tools_check, prereq_check, doctor_output) = tokio::join!(
        checks::check_git(),
        checks::check_jdk(),
        checks::check_android_platform_tools(android_root_ref),
        checks::check_prerequisites(&platform),
        capture_doctor_if_available(&maybe_exe),
    );

    // Assemble components in user-facing order:
    // Flutter → Git → JDK → Android (cmdline, platform-tools, platform, build-tools, licenses) → Prerequisites
    let components = vec![
        flutter_check,
        git_check,
        jdk_check,
        cmdline_check,
        platform_tools_check,
        platform_check,
        build_tools_check,
        licenses_check,
        prereq_check,
    ];

    let report = ToolchainReport {
        platform,
        shell,
        components,
        doctor: doctor_output,
    };

    tracing::debug!(
        "Toolchain preflight complete ({} components checked, doctor={})",
        report.components.len(),
        report.doctor.as_ref().map_or(0, |d| d.len()),
    );

    report
}

/// Run `flutter doctor -v` if an executable is available and parse the output.
///
/// Returns `None` when no Flutter executable was found or when capture fails.
async fn capture_doctor_if_available(
    exe: &Option<crate::flutter_sdk::FlutterExecutable>,
) -> Option<Vec<types::DoctorLine>> {
    let exe = exe.as_ref()?;
    let raw = doctor::capture_flutter_doctor(exe).await?;
    let lines = doctor::parse_doctor_output(&raw);
    if lines.is_empty() {
        None
    } else {
        Some(lines)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_run_preflight_returns_report_without_panicking() {
        // Use a temp directory as the project path so the locator does not
        // accidentally pick up the actual repo's Flutter configuration.
        let tmp = tempfile::TempDir::new().unwrap();
        let report = run_preflight(tmp.path(), None).await;

        // Must always have 9 components in the defined order
        assert_eq!(report.components.len(), 9);
        assert_eq!(report.components[0].kind, ComponentKind::FlutterSdk);
        assert_eq!(report.components[1].kind, ComponentKind::Git);
        assert_eq!(report.components[2].kind, ComponentKind::Jdk);
        assert_eq!(
            report.components[3].kind,
            ComponentKind::AndroidCmdlineTools
        );
        assert_eq!(
            report.components[4].kind,
            ComponentKind::AndroidPlatformTools
        );
        assert_eq!(report.components[5].kind, ComponentKind::AndroidPlatform);
        assert_eq!(report.components[6].kind, ComponentKind::AndroidBuildTools);
        assert_eq!(report.components[7].kind, ComponentKind::AndroidLicenses);
        assert_eq!(report.components[8].kind, ComponentKind::Prerequisites);
    }

    #[tokio::test]
    async fn test_run_preflight_nonexistent_sdk_path_does_not_panic() {
        let tmp = tempfile::TempDir::new().unwrap();
        let fake_sdk = PathBuf::from("/nonexistent/flutter/sdk");
        let report = run_preflight(tmp.path(), Some(&fake_sdk)).await;

        // With a non-existent explicit SDK path, Flutter check should be Partial or Missing
        let flutter = &report.components[0];
        assert_eq!(flutter.kind, ComponentKind::FlutterSdk);
        assert_ne!(flutter.status, ComponentStatus::Ok);
        // Doctor must be None when Flutter is missing
        assert!(report.doctor.is_none());
    }

    #[test]
    fn test_toolchain_report_has_expected_fields() {
        // Ensure the type compiles and fields are accessible
        let report = ToolchainReport {
            platform: HostPlatform::Linux,
            shell: HostShell::Bash,
            components: vec![],
            doctor: None,
        };
        assert_eq!(report.platform, HostPlatform::Linux);
        assert_eq!(report.shell, HostShell::Bash);
        assert!(report.components.is_empty());
        assert!(report.doctor.is_none());
    }
}
