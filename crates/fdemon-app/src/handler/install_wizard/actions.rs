//! # Install Wizard Panel Action Handlers
//!
//! Handles async result messages (preflight completed, step lifecycle) and
//! re-run for the Install Wizard panel.
//!
//! ## Step execution message chain
//!
//! `InstallWizardRunSelectedStep` → `RunWizardStep` action → executor sends
//! `WizardStepStarted` / `WizardStepLog` / `WizardDownloadProgress` /
//! `WizardStepCompleted|Failed`.
//!
//! On `WizardStepCompleted { kind: FlutterSdk, sdk_path: Some(p) }`:
//!   - action  → `PersistSettings`
//!   - message → `InstallWizardRerunPreflight`
//!   - `handle_rerun_preflight` fires `RunToolchainPreflight`
//!   - `handle_preflight_completed` fires `ScanInstalledSdks` (FVM cache refresh)

use crate::config::types::InstallMethod;
use crate::handler::{FlutterStepParams, UpdateAction, UpdateResult};
use crate::install_wizard::WizardStepKind;
use crate::message::Message;
use crate::state::AppState;
use fdemon_daemon::toolchain::ToolchainReport;

/// Handle `ToolchainPreflightCompleted` — populate the wizard with the report.
///
/// Calls `apply_report` to build the five UI steps from the report,
/// clears `loading`, and clears any status message.
///
/// Also fires `UpdateAction::ScanInstalledSdks` so the Flutter Version panel's
/// cache is refreshed after a managed SDK install completes and the preflight
/// re-runs (part of the `WizardStepCompleted(FlutterSdk)` message chain).
pub fn handle_preflight_completed(state: &mut AppState, report: ToolchainReport) -> UpdateResult {
    state.install_wizard_state.apply_report(report);
    state.install_wizard_state.status_message = None;

    // Refresh the FVM cache so the Flutter Version panel shows the newly
    // installed SDK.  `active_sdk_root` comes from the just-resolved SDK —
    // this is the same pattern used by `handle_switch_completed`.
    let active_sdk_root = state.resolved_sdk.as_ref().map(|sdk| sdk.root.clone());
    UpdateResult::action(UpdateAction::ScanInstalledSdks { active_sdk_root })
}

/// Handle `InstallWizardRerunPreflight` — re-run the preflight check.
///
/// Sets `loading = true` and dispatches `RunToolchainPreflight` so the
/// wizard shows a spinner until the updated report arrives.
///
/// Early-returns when a preflight is already in flight to prevent stacking
/// concurrent preflight tasks (each of which spawns `flutter doctor`).
pub fn handle_rerun_preflight(state: &mut AppState) -> UpdateResult {
    // Already running — ignore the re-run request (prevents stacking concurrent
    // preflight tasks, each of which spawns `flutter doctor`).
    if state.install_wizard_state.loading {
        return UpdateResult::none();
    }

    state.install_wizard_state.loading = true;
    state.install_wizard_state.status_message = None;

    let project_path = state.project_path.clone();
    let explicit_sdk_path = state.settings.flutter.sdk_path.clone();

    UpdateResult::action(UpdateAction::RunToolchainPreflight {
        project_path,
        explicit_sdk_path,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Step Execution Handlers (Phase 2, Task 09)
// ─────────────────────────────────────────────────────────────────────────────

/// Handle `InstallWizardRunSelectedStep` — build step params and dispatch the
/// appropriate `RunWizardStep` action for the selected step.
///
/// Guards:
/// - Returns `none()` when a step is already running (prevents concurrent runs).
/// - Returns `none()` with a `status_message` when the selected step is not
///   actionable (no selected step, PathConfig with no known Flutter bin dir,
///   or a step kind without an executor in this phase).
///
/// Side effect: calls `begin_step(kind)` before returning the action so the UI
/// flips to `Running` immediately without waiting for the `WizardStepStarted`
/// message round-trip.
pub fn handle_run_selected_step(state: &mut AppState) -> UpdateResult {
    // Guard: only one step at a time.
    if state.install_wizard_state.is_step_running() {
        return UpdateResult::none();
    }

    // Read the selected step kind.
    let kind = match state.install_wizard_state.selected_step() {
        Some(step) => step.kind,
        None => return UpdateResult::none(),
    };

    match kind {
        WizardStepKind::FlutterSdk => {
            // Build install parameters from settings.
            let method = map_install_method(state.settings.toolchain.install_method());
            let channel = state.settings.toolchain.channel.clone();
            let install_root = state.settings.toolchain.flutter_install_dir.clone();

            let params = FlutterStepParams {
                method,
                channel,
                install_root,
            };

            // Flip UI to Running immediately before the async round-trip.
            state.install_wizard_state.begin_step(kind);

            UpdateResult::action(UpdateAction::RunWizardStep {
                kind,
                install: Some(params),
                path_bin_dir: None,
            })
        }

        WizardStepKind::PathConfig => {
            // Prefer the sdk_path stashed by a just-completed FlutterSdk step,
            // then the settings-configured explicit path, then the resolved SDK root.
            let bin_dir: Option<std::path::PathBuf> = state
                .install_wizard_state
                .installed_sdk_path
                .as_ref()
                .map(|p| p.join("bin"))
                .or_else(|| {
                    state
                        .settings
                        .flutter
                        .sdk_path
                        .as_ref()
                        .map(|p| p.join("bin"))
                })
                .or_else(|| state.resolved_sdk.as_ref().map(|sdk| sdk.root.join("bin")));

            match bin_dir {
                Some(bin) => {
                    // Flip UI to Running immediately.
                    state.install_wizard_state.begin_step(kind);

                    UpdateResult::action(UpdateAction::RunWizardStep {
                        kind,
                        install: None,
                        path_bin_dir: Some(bin),
                    })
                }
                None => {
                    state.install_wizard_state.status_message =
                        Some("Install Flutter first".to_string());
                    UpdateResult::none()
                }
            }
        }

        WizardStepKind::Prerequisites | WizardStepKind::AndroidTools | WizardStepKind::Doctor => {
            state.install_wizard_state.status_message =
                Some("Available in a later phase".to_string());
            UpdateResult::none()
        }
    }
}

/// Handle `WizardStepStarted` — transition the execution state to `Running`.
///
/// Idempotent if `begin_step` was already called on dispatch (the step kind
/// and `Running` status will be the same).
pub fn handle_step_started(state: &mut AppState, kind: WizardStepKind) -> UpdateResult {
    state.install_wizard_state.begin_step(kind);
    UpdateResult::none()
}

/// Handle `WizardStepLog` — append a streamed log line to the detail buffer.
pub fn handle_step_log(state: &mut AppState, line: String) -> UpdateResult {
    state.install_wizard_state.push_step_log(line);
    UpdateResult::none()
}

/// Handle `WizardDownloadProgress` — update download progress counters.
pub fn handle_step_progress(
    state: &mut AppState,
    received: u64,
    total: Option<u64>,
) -> UpdateResult {
    state
        .install_wizard_state
        .set_step_progress(received, total);
    UpdateResult::none()
}

/// Handle `WizardStepCompleted` — record success and chain follow-up effects.
///
/// For `FlutterSdk` steps with a resolved `sdk_path`:
/// 1. Stashes `sdk_path` in `install_wizard_state.installed_sdk_path`.
/// 2. Updates `settings.flutter.sdk_path` so the new SDK is recognised.
/// 3. Returns `UpdateAction::PersistSettings` **and** a follow-up
///    `Message::InstallWizardRerunPreflight` to trigger the preflight→scan chain.
///
/// For all other steps: records `Succeeded` and returns no further effects.
pub fn handle_step_completed(
    state: &mut AppState,
    kind: WizardStepKind,
    summary: String,
    sdk_path: Option<std::path::PathBuf>,
) -> UpdateResult {
    use crate::install_wizard::StepExecStatus;

    state
        .install_wizard_state
        .finish_step(StepExecStatus::Succeeded, summary);

    if kind == WizardStepKind::FlutterSdk {
        if let Some(path) = sdk_path {
            // Stash for the subsequent PathConfig step.
            state.install_wizard_state.installed_sdk_path = Some(path.clone());

            // Update the settings sdk_path so the new SDK is recognised
            // on the next preflight run and SDK re-resolution.
            state.settings.flutter.sdk_path = Some(path);

            // Chain: persist settings → re-run preflight (→ ScanInstalledSdks).
            let project_path = state.project_path.clone();
            return UpdateResult::message_and_action(
                Message::InstallWizardRerunPreflight,
                UpdateAction::PersistSettings {
                    settings: Box::new(state.settings.clone()),
                    project_path,
                },
            );
        }
    }

    UpdateResult::none()
}

/// Handle `WizardStepFailed` — record failure so the step can be retried.
///
/// After this call `is_step_running()` returns `false`, and the next `Enter`
/// will dispatch a new `RunWizardStep` action for the same step.
pub fn handle_step_failed(state: &mut AppState, reason: String) -> UpdateResult {
    use crate::install_wizard::StepExecStatus;
    state
        .install_wizard_state
        .finish_step(StepExecStatus::Failed, reason);
    UpdateResult::none()
}

// ─────────────────────────────────────────────────────────────────────────────
// Internal helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Convert the config-layer `InstallMethod` to the daemon-layer equivalent.
///
/// Both enums have the same variants (`GitClone`, `Archive`) but live in
/// different crates (`fdemon-app/config` vs `fdemon-daemon/toolchain`) to
/// keep the config layer free of daemon dependencies at the `Settings`
/// struct level.
fn map_install_method(method: InstallMethod) -> fdemon_daemon::toolchain::InstallMethod {
    match method {
        InstallMethod::GitClone => fdemon_daemon::toolchain::InstallMethod::GitClone,
        InstallMethod::Archive => fdemon_daemon::toolchain::InstallMethod::Archive,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;
    use fdemon_daemon::toolchain::{
        ComponentCheck, ComponentKind, ComponentStatus, HostPlatform, HostShell, ToolchainReport,
    };

    fn make_report() -> ToolchainReport {
        ToolchainReport {
            platform: HostPlatform::Linux,
            shell: HostShell::Bash,
            components: vec![ComponentCheck {
                kind: ComponentKind::FlutterSdk,
                status: ComponentStatus::Ok,
                detail: String::new(),
            }],
            doctor: None,
        }
    }

    #[test]
    fn test_preflight_completed_populates_steps_clears_loading() {
        let mut state = AppState::new();
        state.show_install_wizard();
        assert!(state.install_wizard_state.loading);

        handle_preflight_completed(&mut state, make_report());

        assert!(!state.install_wizard_state.loading);
        assert_eq!(state.install_wizard_state.steps.len(), 5);
        assert!(state.install_wizard_state.report.is_some());
    }

    #[test]
    fn test_preflight_completed_clears_status_message() {
        let mut state = AppState::new();
        state.show_install_wizard();
        state.install_wizard_state.status_message = Some("old error".into());

        handle_preflight_completed(&mut state, make_report());

        assert!(state.install_wizard_state.status_message.is_none());
    }

    #[test]
    fn test_rerun_preflight_sets_loading_and_returns_action() {
        let mut state = AppState::new();
        state.show_install_wizard();
        state.install_wizard_state.apply_report(make_report());
        assert!(!state.install_wizard_state.loading);

        let result = handle_rerun_preflight(&mut state);

        assert!(state.install_wizard_state.loading);
        assert!(matches!(
            result.action,
            Some(UpdateAction::RunToolchainPreflight { .. })
        ));
    }

    #[test]
    fn test_rerun_preflight_noops_when_already_loading() {
        let mut state = AppState::new();
        state.show_install_wizard();
        // loading is already true after show_install_wizard()
        assert!(state.install_wizard_state.loading);

        let result = handle_rerun_preflight(&mut state);

        // Must stay loading, and must return no action
        assert!(state.install_wizard_state.loading);
        assert!(result.action.is_none());
    }

    #[test]
    fn test_rerun_preflight_spawns_when_idle() {
        let mut state = AppState::new();
        state.show_install_wizard();
        // Simulate preflight completed (loading = false)
        state.install_wizard_state.apply_report(make_report());
        assert!(!state.install_wizard_state.loading);

        let result = handle_rerun_preflight(&mut state);

        assert!(state.install_wizard_state.loading);
        assert!(matches!(
            result.action,
            Some(UpdateAction::RunToolchainPreflight { .. })
        ));
    }

    #[test]
    fn test_rerun_clears_status_message() {
        let mut state = AppState::new();
        state.show_install_wizard();
        // Apply a report to bring loading back to false (idle state), then
        // add a status_message to verify it is cleared on re-run.
        state.install_wizard_state.apply_report(make_report());
        assert!(!state.install_wizard_state.loading);
        state.install_wizard_state.status_message = Some("previous error".into());

        handle_rerun_preflight(&mut state);

        assert!(state.install_wizard_state.status_message.is_none());
    }

    #[test]
    fn test_rerun_carries_project_path() {
        let mut state = AppState::new();
        let result = handle_rerun_preflight(&mut state);
        if let Some(UpdateAction::RunToolchainPreflight { project_path, .. }) = result.action {
            assert_eq!(project_path, state.project_path);
        } else {
            panic!("expected RunToolchainPreflight action");
        }
    }

    #[test]
    fn test_preflight_completed_triggers_scan_installed_sdks() {
        let mut state = AppState::new();
        state.show_install_wizard();

        let result = handle_preflight_completed(&mut state, make_report());

        // Must return a ScanInstalledSdks action to refresh the FVM cache.
        assert!(
            matches!(result.action, Some(UpdateAction::ScanInstalledSdks { .. })),
            "preflight_completed must trigger ScanInstalledSdks; got {:?}",
            result.action
        );
    }

    // ── Step execution handler tests ──────────────────────────────────────────

    /// Helper: build a fresh state with the wizard open and a completed preflight.
    fn state_with_preflight() -> AppState {
        let mut state = AppState::new();
        state.show_install_wizard();
        state.install_wizard_state.apply_report(make_report());
        state
    }

    #[test]
    fn test_run_selected_flutter_step_dispatches_install_action() {
        let mut state = state_with_preflight();
        // Select the FlutterSdk step (index 3 in the 5-step list).
        state.install_wizard_state.selected_index = 3;
        assert_eq!(
            state.install_wizard_state.selected_step().map(|s| s.kind),
            Some(WizardStepKind::FlutterSdk),
            "precondition: selected step must be FlutterSdk"
        );

        let result = handle_run_selected_step(&mut state);

        assert!(
            matches!(
                result.action,
                Some(UpdateAction::RunWizardStep {
                    kind: WizardStepKind::FlutterSdk,
                    install: Some(_),
                    path_bin_dir: None,
                })
            ),
            "FlutterSdk step must dispatch RunWizardStep with install params; got {:?}",
            result.action
        );
        // UI must have already flipped to Running.
        assert!(
            state.install_wizard_state.is_step_running(),
            "begin_step must have been called before returning the action"
        );
    }

    #[test]
    fn test_run_selected_noop_while_running() {
        let mut state = state_with_preflight();
        // Select and start the FlutterSdk step.
        state.install_wizard_state.selected_index = 3;
        handle_run_selected_step(&mut state);
        assert!(state.install_wizard_state.is_step_running());

        // Second call must be a no-op.
        let result = handle_run_selected_step(&mut state);
        assert!(
            result.action.is_none(),
            "must not dispatch while step is running"
        );
        assert!(result.message.is_none());
    }

    #[test]
    fn test_pathconfig_without_known_sdk_sets_status_message() {
        let mut state = state_with_preflight();
        // Select PathConfig (index 2) with no SDK path set.
        state.install_wizard_state.selected_index = 2;
        assert_eq!(
            state.install_wizard_state.selected_step().map(|s| s.kind),
            Some(WizardStepKind::PathConfig),
            "precondition: selected step must be PathConfig"
        );
        // Ensure no SDK is resolved.
        state.settings.flutter.sdk_path = None;
        state.resolved_sdk = None;
        state.install_wizard_state.installed_sdk_path = None;

        let result = handle_run_selected_step(&mut state);

        assert!(
            result.action.is_none(),
            "must not dispatch without a known Flutter bin dir"
        );
        assert!(
            state.install_wizard_state.status_message.is_some(),
            "must set a helpful status_message"
        );
        assert!(
            state
                .install_wizard_state
                .status_message
                .as_deref()
                .unwrap()
                .contains("Flutter"),
            "status_message must mention Flutter"
        );
    }

    #[test]
    fn test_pathconfig_with_installed_sdk_path_dispatches_action() {
        let mut state = state_with_preflight();
        // Simulate a just-completed FlutterSdk step that stashed an sdk_path.
        state.install_wizard_state.installed_sdk_path =
            Some(std::path::PathBuf::from("/opt/flutter"));
        state.install_wizard_state.selected_index = 2; // PathConfig

        let result = handle_run_selected_step(&mut state);

        assert!(
            matches!(
                result.action,
                Some(UpdateAction::RunWizardStep {
                    kind: WizardStepKind::PathConfig,
                    install: None,
                    path_bin_dir: Some(_),
                })
            ),
            "PathConfig step with known SDK must dispatch RunWizardStep; got {:?}",
            result.action
        );
    }

    #[test]
    fn test_completed_flutter_persists_sdk_path_and_reruns_preflight() {
        let mut state = state_with_preflight();
        let sdk = std::path::PathBuf::from("/home/user/flutter");

        let result = handle_step_completed(
            &mut state,
            WizardStepKind::FlutterSdk,
            "Installed to /home/user/flutter".into(),
            Some(sdk.clone()),
        );

        // settings.flutter.sdk_path must be updated.
        assert_eq!(
            state.settings.flutter.sdk_path.as_ref(),
            Some(&sdk),
            "sdk_path must be written to settings"
        );

        // installed_sdk_path must be stashed.
        assert_eq!(
            state.install_wizard_state.installed_sdk_path.as_ref(),
            Some(&sdk),
            "sdk_path must be stashed for PathConfig step"
        );

        // Action must be PersistSettings.
        assert!(
            matches!(result.action, Some(UpdateAction::PersistSettings { .. })),
            "must return PersistSettings action; got {:?}",
            result.action
        );

        // Follow-up message must be InstallWizardRerunPreflight.
        assert!(
            matches!(result.message, Some(Message::InstallWizardRerunPreflight)),
            "must return InstallWizardRerunPreflight follow-up; got {:?}",
            result.message
        );
    }

    #[test]
    fn test_step_failed_records_reason_and_allows_retry() {
        let mut state = state_with_preflight();
        // Start a step first.
        state
            .install_wizard_state
            .begin_step(WizardStepKind::FlutterSdk);
        assert!(state.install_wizard_state.is_step_running());

        handle_step_failed(&mut state, "network timeout".into());

        // is_step_running must be false.
        assert!(!state.install_wizard_state.is_step_running());
        // Result summary must contain the reason.
        assert_eq!(
            state
                .install_wizard_state
                .execution
                .result_summary
                .as_deref(),
            Some("network timeout")
        );
        // A fresh run must now be dispatchable.
        state.install_wizard_state.selected_index = 3; // FlutterSdk
        let result = handle_run_selected_step(&mut state);
        assert!(
            result.action.is_some(),
            "retry must be possible after a failed step"
        );
    }

    #[test]
    fn test_step_log_appends_line() {
        let mut state = state_with_preflight();
        handle_step_log(&mut state, "Cloning...".into());
        handle_step_log(&mut state, "Done".into());
        assert_eq!(state.install_wizard_state.execution.log_tail.len(), 2);
        assert_eq!(
            state.install_wizard_state.execution.log_tail[0],
            "Cloning..."
        );
        assert_eq!(state.install_wizard_state.execution.log_tail[1], "Done");
    }

    #[test]
    fn test_step_progress_updates_counters() {
        let mut state = state_with_preflight();
        handle_step_progress(&mut state, 512, Some(1024));
        assert_eq!(state.install_wizard_state.execution.received, 512);
        assert_eq!(state.install_wizard_state.execution.total, Some(1024));
    }

    #[test]
    fn test_step_started_is_idempotent_with_begin_step() {
        let mut state = state_with_preflight();
        // begin_step called by handle_run_selected_step
        state
            .install_wizard_state
            .begin_step(WizardStepKind::FlutterSdk);
        // WizardStepStarted arrives from the executor
        handle_step_started(&mut state, WizardStepKind::FlutterSdk);
        // Must still be Running (not reset to Idle).
        assert!(state.install_wizard_state.is_step_running());
        assert_eq!(
            state.install_wizard_state.execution.kind,
            Some(WizardStepKind::FlutterSdk)
        );
    }

    #[test]
    fn test_completed_non_flutter_sdk_returns_none() {
        let mut state = state_with_preflight();
        state
            .install_wizard_state
            .begin_step(WizardStepKind::PathConfig);

        let result = handle_step_completed(
            &mut state,
            WizardStepKind::PathConfig,
            "PATH updated".into(),
            None,
        );

        // No chain for non-FlutterSdk steps.
        assert!(result.action.is_none());
        assert!(result.message.is_none());
    }
}
