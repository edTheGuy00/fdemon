//! `fdemon doctor` subcommand — read-only toolchain diagnostics.
//!
//! Runs [`fdemon_daemon::toolchain::run_preflight`] against the specified
//! project directory (or the current working directory), prints a structured
//! component report followed by the captured `flutter doctor -v` lines, and
//! exits with code 0 when all **gating** components are [`ComponentStatus::Ok`],
//! or 1 otherwise.
//!
//! ## Exit-code gating rules
//!
//! **Core components** (Flutter SDK, Git, JDK, Prerequisites) always gate the
//! exit code.
//!
//! **Android components** (cmdline-tools, platform-tools, platform, build-tools,
//! licenses) only gate the exit code when an Android SDK root was actually
//! resolved on this host.  If every Android component is `Unknown` or `Missing`
//! the Android SDK is absent and those components are printed but do **not**
//! contribute to a failing exit code — allowing pure Flutter web/desktop/iOS
//! projects and CI runners without an Android SDK to exit 0.
//!
//! This subcommand never starts the TUI or the Engine — it is a pure
//! diagnostic tool intended for CI pipelines and manual toolchain debugging.

use std::path::PathBuf;
use std::process::ExitCode;

use fdemon_daemon::toolchain::{run_preflight, ComponentKind, ComponentStatus};

/// Return `true` when a status counts as a failure for exit-code purposes.
#[inline]
fn is_failing(status: &ComponentStatus) -> bool {
    !matches!(status, ComponentStatus::Ok)
}

/// Return `true` when a [`ComponentKind`] is one of the five Android sub-checks.
#[inline]
fn is_android_component(kind: &ComponentKind) -> bool {
    matches!(
        kind,
        ComponentKind::AndroidCmdlineTools
            | ComponentKind::AndroidPlatformTools
            | ComponentKind::AndroidPlatform
            | ComponentKind::AndroidBuildTools
            | ComponentKind::AndroidLicenses
    )
}

/// Return `true` when the Android SDK appears to be present on this host.
///
/// The SDK is considered present when at least one Android component has a
/// status other than [`ComponentStatus::Unknown`] or [`ComponentStatus::Missing`]
/// — i.e. the SDK root was resolved and the probe could actually inspect it.
fn android_sdk_present(report: &fdemon_daemon::toolchain::ToolchainReport) -> bool {
    report
        .components
        .iter()
        .filter(|c| is_android_component(&c.kind))
        .any(|c| {
            !matches!(
                c.status,
                ComponentStatus::Unknown | ComponentStatus::Missing
            )
        })
}

/// Run the `fdemon doctor` diagnostics subcommand.
///
/// # Arguments
///
/// * `cwd` — The project directory to pass to the SDK locator. Typically the
///   current working directory or an explicitly-provided path.
/// * `explicit_sdk` — An optional Flutter SDK path taken from
///   `.fdemon/config.toml` `[flutter] sdk_path`, if any was configured for
///   the project.
///
/// # Returns
///
/// [`ExitCode::SUCCESS`] (0) when all gating components are `Ok`;
/// [`ExitCode`] 1 otherwise.  Android components only gate when an Android
/// SDK root is present on the host (see module-level doc).
pub async fn run_doctor(cwd: PathBuf, explicit_sdk: Option<PathBuf>) -> ExitCode {
    // Warn the user that preflight can take a while before blocking.
    eprintln!("Running toolchain checks…");

    // Headless doctor has no persisted wizard settings — rely on env/default
    // Android SDK resolution (no override).
    let outcome = run_preflight(&cwd, explicit_sdk.as_deref(), None).await;
    let report = outcome.report;

    // Determine whether Android components should gate the exit code.
    let android_gates = android_sdk_present(&report);

    let mut all_ok = true;
    for c in &report.components {
        // Android components only fail the exit code when the SDK is present.
        let gates = if is_android_component(&c.kind) {
            android_gates
        } else {
            true
        };
        if gates && is_failing(&c.status) {
            all_ok = false;
        }
        // Use .to_string() so the {:>4} right-align width specifier is
        // honoured — a String value respects f.width() / f.pad() padding
        // whereas a Display impl that calls write!() directly does not.
        // Column widths: "OK"=2, "!"=1, "MISS"=4, "ERR"=3, "?"=1 → pad to 4.
        println!("[{:>4}] {} — {}", c.status.to_string(), c.kind, c.detail);
    }

    if let Some(lines) = &report.doctor {
        println!("\nflutter doctor:");
        for l in lines {
            println!("  {}", l.text);
        }
    }

    if all_ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

#[cfg(test)]
mod tests {
    use fdemon_daemon::toolchain::{
        ComponentCheck, ComponentKind, ComponentStatus, HostPlatform, HostShell, ToolchainReport,
    };

    use super::{android_sdk_present, is_android_component, is_failing};

    // ── helper ────────────────────────────────────────────────────────────────

    fn make_check(kind: ComponentKind, status: ComponentStatus) -> ComponentCheck {
        ComponentCheck {
            kind,
            status,
            detail: String::new(),
        }
    }

    fn make_report(components: Vec<ComponentCheck>) -> ToolchainReport {
        ToolchainReport {
            platform: HostPlatform::Linux,
            shell: HostShell::Bash,
            components,
            doctor: None,
            linux_package_manager: None,
            winget_available: false,
        }
    }

    // ── is_failing ────────────────────────────────────────────────────────────

    #[test]
    fn is_failing_only_on_non_ok() {
        assert!(!is_failing(&ComponentStatus::Ok));
        assert!(is_failing(&ComponentStatus::Missing));
        assert!(is_failing(&ComponentStatus::Unknown));
        assert!(is_failing(&ComponentStatus::Error));
        assert!(is_failing(&ComponentStatus::Partial));
    }

    // ── is_android_component ─────────────────────────────────────────────────

    #[test]
    fn is_android_component_classifies_correctly() {
        assert!(is_android_component(&ComponentKind::AndroidCmdlineTools));
        assert!(is_android_component(&ComponentKind::AndroidPlatformTools));
        assert!(is_android_component(&ComponentKind::AndroidPlatform));
        assert!(is_android_component(&ComponentKind::AndroidBuildTools));
        assert!(is_android_component(&ComponentKind::AndroidLicenses));

        assert!(!is_android_component(&ComponentKind::FlutterSdk));
        assert!(!is_android_component(&ComponentKind::Git));
        assert!(!is_android_component(&ComponentKind::Jdk));
        assert!(!is_android_component(&ComponentKind::Prerequisites));
    }

    // ── android_sdk_present ───────────────────────────────────────────────────

    /// When all Android components are Unknown (no SDK root resolved) the SDK
    /// is absent and Android should not gate the exit code.
    #[test]
    fn android_sdk_absent_when_all_unknown() {
        let report = make_report(vec![
            make_check(ComponentKind::FlutterSdk, ComponentStatus::Ok),
            make_check(ComponentKind::Git, ComponentStatus::Ok),
            make_check(ComponentKind::Jdk, ComponentStatus::Ok),
            make_check(ComponentKind::AndroidCmdlineTools, ComponentStatus::Unknown),
            make_check(
                ComponentKind::AndroidPlatformTools,
                ComponentStatus::Unknown,
            ),
            make_check(ComponentKind::AndroidPlatform, ComponentStatus::Unknown),
            make_check(ComponentKind::AndroidBuildTools, ComponentStatus::Unknown),
            make_check(ComponentKind::AndroidLicenses, ComponentStatus::Unknown),
            make_check(ComponentKind::Prerequisites, ComponentStatus::Ok),
        ]);
        assert!(!android_sdk_present(&report));
    }

    /// When all Android components are Missing the SDK is also absent.
    #[test]
    fn android_sdk_absent_when_all_missing() {
        let report = make_report(vec![
            make_check(ComponentKind::AndroidCmdlineTools, ComponentStatus::Missing),
            make_check(
                ComponentKind::AndroidPlatformTools,
                ComponentStatus::Missing,
            ),
            make_check(ComponentKind::AndroidPlatform, ComponentStatus::Missing),
            make_check(ComponentKind::AndroidBuildTools, ComponentStatus::Missing),
            make_check(ComponentKind::AndroidLicenses, ComponentStatus::Missing),
        ]);
        assert!(!android_sdk_present(&report));
    }

    /// When at least one Android component has a non-Unknown/Missing status
    /// (e.g. Ok or Error) the SDK root was resolved → Android gates exit code.
    #[test]
    fn android_sdk_present_when_any_probed() {
        let report = make_report(vec![
            make_check(ComponentKind::AndroidCmdlineTools, ComponentStatus::Ok),
            make_check(
                ComponentKind::AndroidPlatformTools,
                ComponentStatus::Unknown,
            ),
            make_check(ComponentKind::AndroidPlatform, ComponentStatus::Unknown),
            make_check(ComponentKind::AndroidBuildTools, ComponentStatus::Unknown),
            make_check(ComponentKind::AndroidLicenses, ComponentStatus::Unknown),
        ]);
        assert!(android_sdk_present(&report));
    }

    #[test]
    fn android_sdk_present_when_any_error() {
        let report = make_report(vec![
            make_check(ComponentKind::AndroidCmdlineTools, ComponentStatus::Unknown),
            make_check(ComponentKind::AndroidPlatformTools, ComponentStatus::Error),
            make_check(ComponentKind::AndroidPlatform, ComponentStatus::Unknown),
            make_check(ComponentKind::AndroidBuildTools, ComponentStatus::Unknown),
            make_check(ComponentKind::AndroidLicenses, ComponentStatus::Unknown),
        ]);
        assert!(android_sdk_present(&report));
    }

    // ── exit-code aggregation integration ─────────────────────────────────────

    /// Core components OK, Android all Unknown/Missing → should succeed.
    ///
    /// This is the canonical "non-Android project on a CI runner without
    /// Android SDK" scenario.
    #[test]
    fn exit_succeeds_when_core_ok_android_absent() {
        let report = make_report(vec![
            make_check(ComponentKind::FlutterSdk, ComponentStatus::Ok),
            make_check(ComponentKind::Git, ComponentStatus::Ok),
            make_check(ComponentKind::Jdk, ComponentStatus::Ok),
            make_check(ComponentKind::AndroidCmdlineTools, ComponentStatus::Unknown),
            make_check(
                ComponentKind::AndroidPlatformTools,
                ComponentStatus::Unknown,
            ),
            make_check(ComponentKind::AndroidPlatform, ComponentStatus::Unknown),
            make_check(ComponentKind::AndroidBuildTools, ComponentStatus::Unknown),
            make_check(ComponentKind::AndroidLicenses, ComponentStatus::Unknown),
            make_check(ComponentKind::Prerequisites, ComponentStatus::Ok),
        ]);

        let android_gates = android_sdk_present(&report);
        assert!(!android_gates, "Android should not gate when SDK absent");

        let mut all_ok = true;
        for c in &report.components {
            let gates = if is_android_component(&c.kind) {
                android_gates
            } else {
                true
            };
            if gates && is_failing(&c.status) {
                all_ok = false;
            }
        }
        assert!(all_ok, "Should succeed when core OK and Android absent");
    }

    /// Missing Flutter SDK → must still fail even with Android absent.
    #[test]
    fn exit_fails_when_flutter_sdk_missing() {
        let report = make_report(vec![
            make_check(ComponentKind::FlutterSdk, ComponentStatus::Missing),
            make_check(ComponentKind::Git, ComponentStatus::Ok),
            make_check(ComponentKind::Jdk, ComponentStatus::Ok),
            make_check(ComponentKind::AndroidCmdlineTools, ComponentStatus::Unknown),
            make_check(
                ComponentKind::AndroidPlatformTools,
                ComponentStatus::Unknown,
            ),
            make_check(ComponentKind::AndroidPlatform, ComponentStatus::Unknown),
            make_check(ComponentKind::AndroidBuildTools, ComponentStatus::Unknown),
            make_check(ComponentKind::AndroidLicenses, ComponentStatus::Unknown),
            make_check(ComponentKind::Prerequisites, ComponentStatus::Ok),
        ]);

        let android_gates = android_sdk_present(&report);
        let mut all_ok = true;
        for c in &report.components {
            let gates = if is_android_component(&c.kind) {
                android_gates
            } else {
                true
            };
            if gates && is_failing(&c.status) {
                all_ok = false;
            }
        }
        assert!(!all_ok, "Should fail when Flutter SDK is missing");
    }

    /// Android SDK present and broken → must fail.
    #[test]
    fn exit_fails_when_android_present_and_broken() {
        let report = make_report(vec![
            make_check(ComponentKind::FlutterSdk, ComponentStatus::Ok),
            make_check(ComponentKind::Git, ComponentStatus::Ok),
            make_check(ComponentKind::Jdk, ComponentStatus::Ok),
            make_check(ComponentKind::AndroidCmdlineTools, ComponentStatus::Ok),
            make_check(ComponentKind::AndroidPlatformTools, ComponentStatus::Error),
            make_check(ComponentKind::AndroidPlatform, ComponentStatus::Unknown),
            make_check(ComponentKind::AndroidBuildTools, ComponentStatus::Unknown),
            make_check(ComponentKind::AndroidLicenses, ComponentStatus::Unknown),
            make_check(ComponentKind::Prerequisites, ComponentStatus::Ok),
        ]);

        let android_gates = android_sdk_present(&report);
        assert!(android_gates, "Android should gate when SDK present");

        let mut all_ok = true;
        for c in &report.components {
            let gates = if is_android_component(&c.kind) {
                android_gates
            } else {
                true
            };
            if gates && is_failing(&c.status) {
                all_ok = false;
            }
        }
        assert!(!all_ok, "Should fail when Android SDK present and broken");
    }

    // ── F20: status column width ───────────────────────────────────────────────

    /// F20: the status field must always be exactly 4 characters wide so the
    /// printed column is aligned regardless of status variant.  This verifies
    /// that `.to_string()` (a `String` value) correctly propagates the `{:>4}`
    /// width specifier used in `run_doctor`'s print loop.
    #[test]
    fn status_field_is_always_4_chars_wide() {
        let cases = [
            (ComponentStatus::Ok, "  OK"),
            (ComponentStatus::Missing, "MISS"),
            (ComponentStatus::Error, " ERR"),
            (ComponentStatus::Unknown, "   ?"),
            (ComponentStatus::Partial, "   !"),
        ];
        for (status, expected) in &cases {
            let field = format!("{:>4}", status.to_string());
            assert_eq!(
                field, *expected,
                "status {:?}: expected {:?} got {:?}",
                status, expected, field
            );
            assert_eq!(
                field.len(),
                4,
                "status {:?} field width should be 4, got {}",
                status,
                field.len()
            );
        }
    }
}
