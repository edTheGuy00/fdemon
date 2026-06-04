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
use crate::handler::{AndroidStepParams, FlutterStepParams, UpdateAction, UpdateResult};
use crate::install_wizard::{is_jdk_actionable, WizardStepKind};
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
                android_sdk_root: None,
                android: None,
            })
        }

        WizardStepKind::AndroidTools => {
            // JDK gate: sdkmanager requires a JDK 17. Use the shared `is_jdk_actionable`
            // helper — the same predicate that populates the guided command in
            // `build_steps()` — so the gate message and the rendered command always agree:
            // when no Jdk entry is present the guided command IS shown and the executor
            // IS blocked.
            if is_jdk_actionable_from_state(state) {
                state.install_wizard_state.status_message = Some(
                    "Install JDK 17 first (see the command below), then press 'r' to re-check."
                        .into(),
                );
                return UpdateResult::none();
            }

            let ts = &state.settings.toolchain;
            let params = AndroidStepParams {
                sdk_root: ts.android_sdk_root.clone(),
                api_level: ts.android_api_level,
                cmdline_tools_build: ts.cmdline_tools_build.clone(),
                jdk_path: ts.jdk_path.clone(),
            };

            // Flip UI to Running immediately before the async round-trip.
            state.install_wizard_state.begin_step(kind);

            UpdateResult::action(UpdateAction::RunWizardStep {
                kind,
                install: None,
                path_bin_dir: None,
                android_sdk_root: None,
                android: Some(params),
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
                    // Include the Android SDK root so the executor can write ANDROID_HOME.
                    let android_sdk_root = state.settings.toolchain.android_sdk_root.clone();

                    // Ordering hint (m3): Android Tools should ideally be run before
                    // PathConfig so that ANDROID_HOME is also written. This is a soft
                    // hint — PathConfig still executes (it will write the Flutter PATH
                    // regardless). A user with ANDROID_HOME already set in their profile
                    // should not be blocked.
                    if android_sdk_root.is_none() {
                        state.install_wizard_state.status_message = Some(
                            "Tip: run Android Tools first so ANDROID_HOME is also configured."
                                .into(),
                        );
                    }

                    // Flip UI to Running immediately.
                    state.install_wizard_state.begin_step(kind);

                    UpdateResult::action(UpdateAction::RunWizardStep {
                        kind,
                        install: None,
                        path_bin_dir: Some(bin),
                        android_sdk_root,
                        android: None,
                    })
                }
                None => {
                    state.install_wizard_state.status_message =
                        Some("Install Flutter first".to_string());
                    UpdateResult::none()
                }
            }
        }

        WizardStepKind::Prerequisites => {
            // Prerequisites is non-executable: the wizard cannot auto-run
            // privileged package-manager or GUI commands. Instead, direct the
            // user to the guided command(s) shown in the detail pane.
            state.install_wizard_state.status_message =
                Some("Run the listed command(s), then press r to re-check.".to_string());
            UpdateResult::none()
        }

        WizardStepKind::Doctor => {
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

/// Handle `WizardStepPhase` — update the phase label shown in the progress widget.
///
/// Guards:
/// - No-op when no step is currently running (prevents stale updates from a
///   previous run arriving after the executor has finished).
/// - No-op when the running step's kind does not match `kind` (guards against
///   out-of-order messages from a superseded run).
///
/// Mirrors the guard logic used by `handle_step_log` and `handle_step_progress`.
pub fn handle_step_phase(
    state: &mut AppState,
    kind: WizardStepKind,
    label: String,
) -> UpdateResult {
    // Guard: only update when the reported kind matches the running step.
    let running_kind = state.install_wizard_state.execution.kind;
    if running_kind != Some(kind) {
        return UpdateResult::none();
    }

    state.install_wizard_state.set_step_phase(label);
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

    if kind == WizardStepKind::AndroidTools {
        // The executor passes the resolved Android SDK root via `sdk_path` so that
        // `settings.toolchain.android_sdk_root` can be updated and persisted.
        // Re-run preflight afterwards so the Android checks flip to Ok.
        if let Some(root) = sdk_path {
            state.settings.toolchain.android_sdk_root = Some(root);

            // Chain: persist settings → re-run preflight.
            let project_path = state.project_path.clone();
            return UpdateResult::message_and_action(
                Message::InstallWizardRerunPreflight,
                UpdateAction::PersistSettings {
                    settings: Box::new(state.settings.clone()),
                    project_path,
                },
            );
        }
        // Even without a resolved SDK root, re-run preflight so any partial
        // installs are reflected in the step list.
        return UpdateResult::message(Message::InstallWizardRerunPreflight);
    }

    if kind == WizardStepKind::PathConfig {
        // Clear the session stash once PathConfig has successfully consumed it.
        // The stash was set on a successful FlutterSdk completion and is used
        // to prefer the just-installed SDK root over the settings sdk_path when
        // resolving the bin dir for this step. Clearing it here prevents a stale
        // path from winning on a later PathConfig run (e.g. if the user changes
        // `settings.flutter.sdk_path` and re-runs PathConfig without re-installing).
        state.install_wizard_state.installed_sdk_path = None;
    }

    UpdateResult::none()
}

/// Handle `WizardStepFailed` — record failure so the step can be retried.
///
/// After this call `is_step_running()` returns `false`, and the next `Enter`
/// will dispatch a new `RunWizardStep` action for the same step.
///
/// When `reason` starts with the reserved prefix `"Cancelled:"` (written by
/// the executor when `Error::Cancelled` is observed), the step was stopped
/// by the user and the `status_message` reflects that; otherwise a "failed"
/// retry prompt is shown.
pub fn handle_step_failed(state: &mut AppState, reason: String) -> UpdateResult {
    use crate::install_wizard::StepExecStatus;

    // Always clear the task handle on any terminal path.
    let _ = state.install_wizard_state.install_task.take();

    if reason.starts_with("Cancelled:") {
        // User-initiated cancellation: keep a neutral message; the step is
        // reset to Idle so the next Enter retries cleanly.
        state
            .install_wizard_state
            .finish_step(StepExecStatus::Failed, reason);
        // Overwrite the summary with a neutral cancelled message (it was set
        // above by finish_step but we want a neutral display, not "Failed").
        state.install_wizard_state.status_message =
            Some("Cancelled. Press Enter to retry.".to_string());
    } else {
        state
            .install_wizard_state
            .finish_step(StepExecStatus::Failed, reason);
        state.install_wizard_state.status_message =
            Some("Failed \u{2014} press Enter to retry or r to re-check".to_string());
    }
    UpdateResult::none()
}

// ─────────────────────────────────────────────────────────────────────────────
// Phase 5, Task 03 — Cancel step handler
// ─────────────────────────────────────────────────────────────────────────────

/// Handle `WizardInstallTaskReady` — store the cancel token and join handle.
///
/// Called immediately after `RunWizardStep` spawns its background task, so
/// the TEA layer has access to the handle before any subsequent `Esc` press.
/// Idempotent: if no step is currently running, the handle is stored anyway
/// (it will be cleared when the terminal `WizardStepCompleted/Failed` arrives).
pub fn handle_install_task_ready(
    state: &mut AppState,
    cancel: std::sync::Arc<tokio_util::sync::CancellationToken>,
    handle: std::sync::Arc<std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>>,
) -> UpdateResult {
    use crate::install_wizard::InstallTaskHandle;
    use tokio_util::sync::CancellationToken;

    // Extract the JoinHandle from the Arc<Mutex<Option<>>>.
    let join = handle
        .lock()
        .ok()
        .and_then(|mut g| g.take())
        .unwrap_or_else(|| {
            // Handle not yet deposited — create a no-op handle as a fallback.
            tokio::spawn(std::future::ready(()))
        });

    // Unwrap the Arc to get the underlying token (or clone if Arc is shared).
    let cancel_token: CancellationToken = (*cancel).clone();

    state.install_wizard_state.install_task = Some(InstallTaskHandle {
        join,
        cancel: cancel_token,
    });
    UpdateResult::none()
}

/// Handle `InstallWizardCancelStep` — signal the running install to stop.
///
/// Cancels the token, optionally aborts the task as a backstop, resets
/// the step to idle, and sets a neutral "Cancelled" status message.
///
/// Idempotent — a second cancel with no running task is a no-op.
pub fn handle_cancel_step(state: &mut AppState) -> UpdateResult {
    if let Some(task) = state.install_wizard_state.install_task.take() {
        // Signal the install loop to stop at the next cancellation checkpoint.
        task.cancel.cancel();
        // Abort the task as a backstop in case the install loop doesn't check
        // the token frequently enough (e.g., during a blocking git-clone).
        task.join.abort();
    }
    state.install_wizard_state.reset_running_step_to_idle();
    state.install_wizard_state.status_message =
        Some("Cancelled. Press Enter to retry.".to_string());
    UpdateResult::none()
}

// ─────────────────────────────────────────────────────────────────────────────
// Phase 3, Task 07 — Copy-command handler
// ─────────────────────────────────────────────────────────────────────────────

/// Handle `InstallWizardCopyCommand` — copy the selected step's guided command
/// to the clipboard (`c` key).
///
/// Pushes a `WriteClipboard` action (intercepted by the runner in `process.rs`)
/// and sets a brief status message confirming the copy. When no guided command
/// is available for the selected step, sets a "no command" status message
/// instead.
///
/// Pure: no I/O, no async.
pub fn handle_copy_command(state: &mut AppState) -> UpdateResult {
    match state.install_wizard_state.selected_guided_command() {
        Some(cmd) => {
            let text = cmd.command.clone();
            state.install_wizard_state.status_message = Some(format!("Copied: {}", text));
            UpdateResult::action(UpdateAction::WriteClipboard { text })
        }
        None => {
            state.install_wizard_state.status_message =
                Some("No command to copy for this step.".into());
            UpdateResult::none()
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Internal helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Return `true` when JDK needs attention, pulling components from the current
/// preflight report stored on `state`.
///
/// Delegates to `is_jdk_actionable` (from `install_wizard::state`) so that the
/// gate here and the guided-command population in `build_steps()` agree exactly.
/// Returns `true` (actionable) when the report is absent — safe default.
fn is_jdk_actionable_from_state(state: &AppState) -> bool {
    match state.install_wizard_state.report.as_ref() {
        None => true, // No report yet → treat as actionable (safe default)
        Some(r) => is_jdk_actionable(&r.components),
    }
}

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
            linux_package_manager: Some(fdemon_daemon::toolchain::LinuxPackageManager::Unknown),
            winget_available: false,
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
                    ..
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
                    ..
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

    // ── handle_step_phase tests ───────────────────────────────────────────────

    #[test]
    fn test_step_phase_updates_phase_label_when_running() {
        let mut state = state_with_preflight();
        state
            .install_wizard_state
            .begin_step(WizardStepKind::FlutterSdk);
        assert!(state.install_wizard_state.is_step_running());

        let result = handle_step_phase(&mut state, WizardStepKind::FlutterSdk, "Cloning".into());

        assert!(result.action.is_none());
        assert!(result.message.is_none());
        assert_eq!(
            state.install_wizard_state.execution.phase_label.as_deref(),
            Some("Cloning"),
            "phase_label must be updated when the kind matches the running step"
        );
    }

    #[test]
    fn test_step_phase_ignored_when_no_step_running() {
        let mut state = state_with_preflight();
        // No step started — execution.kind is None.
        assert!(!state.install_wizard_state.is_step_running());

        handle_step_phase(&mut state, WizardStepKind::FlutterSdk, "Cloning".into());

        assert!(
            state.install_wizard_state.execution.phase_label.is_none(),
            "phase_label must not be set when no step is running"
        );
    }

    #[test]
    fn test_step_phase_ignored_on_kind_mismatch() {
        let mut state = state_with_preflight();
        // Start PathConfig step.
        state
            .install_wizard_state
            .begin_step(WizardStepKind::PathConfig);

        // A Phase event arrives for FlutterSdk — must be ignored.
        handle_step_phase(&mut state, WizardStepKind::FlutterSdk, "Cloning".into());

        assert!(
            state.install_wizard_state.execution.phase_label.is_none(),
            "phase_label must not be set when the kind does not match the running step"
        );
    }

    // ── installed_sdk_path clearing tests ────────────────────────────────────

    #[test]
    fn test_installed_sdk_path_cleared_after_pathconfig_success() {
        let mut state = state_with_preflight();
        // Simulate a stashed path from a previous FlutterSdk completion.
        state.install_wizard_state.installed_sdk_path =
            Some(std::path::PathBuf::from("/opt/flutter"));
        state
            .install_wizard_state
            .begin_step(WizardStepKind::PathConfig);

        handle_step_completed(
            &mut state,
            WizardStepKind::PathConfig,
            "PATH updated".into(),
            None,
        );

        assert!(
            state.install_wizard_state.installed_sdk_path.is_none(),
            "installed_sdk_path must be cleared after a successful PathConfig completion \
             to prevent a stale stash from winning on a later PathConfig run"
        );
    }

    #[test]
    fn test_installed_sdk_path_preserved_after_flutter_sdk_success() {
        let mut state = state_with_preflight();
        let sdk = std::path::PathBuf::from("/home/user/flutter");
        state
            .install_wizard_state
            .begin_step(WizardStepKind::FlutterSdk);

        handle_step_completed(
            &mut state,
            WizardStepKind::FlutterSdk,
            "Installed".into(),
            Some(sdk.clone()),
        );

        // Must be stashed (PathConfig reads it).
        assert_eq!(
            state.install_wizard_state.installed_sdk_path.as_ref(),
            Some(&sdk),
            "installed_sdk_path must be stashed after FlutterSdk completion"
        );
    }

    #[test]
    fn test_installed_sdk_path_not_cleared_by_failed_pathconfig() {
        let mut state = state_with_preflight();
        // Simulate a stashed path.
        state.install_wizard_state.installed_sdk_path =
            Some(std::path::PathBuf::from("/opt/flutter"));
        state
            .install_wizard_state
            .begin_step(WizardStepKind::PathConfig);

        // Failure — stash must survive so a retry can still use it.
        handle_step_failed(&mut state, "Permission denied".into());

        assert!(
            state.install_wizard_state.installed_sdk_path.is_some(),
            "installed_sdk_path must NOT be cleared on a failed PathConfig step"
        );
    }

    // ── Android Tools handler tests ───────────────────────────────────────────

    /// Build a report that includes a JDK component with the given status.
    fn make_report_with_jdk(jdk_status: ComponentStatus) -> ToolchainReport {
        ToolchainReport {
            platform: HostPlatform::Linux,
            shell: HostShell::Bash,
            components: vec![
                ComponentCheck {
                    kind: ComponentKind::FlutterSdk,
                    status: ComponentStatus::Ok,
                    detail: String::new(),
                },
                ComponentCheck {
                    kind: ComponentKind::Jdk,
                    status: jdk_status,
                    detail: String::new(),
                },
            ],
            doctor: None,
            linux_package_manager: Some(fdemon_daemon::toolchain::LinuxPackageManager::Unknown),
            winget_available: false,
        }
    }

    /// Build a fresh state with the wizard open and a JDK at the given status.
    fn wizard_state_with_jdk(jdk_status: ComponentStatus) -> AppState {
        let mut state = AppState::new();
        state.show_install_wizard();
        state
            .install_wizard_state
            .apply_report(make_report_with_jdk(jdk_status));
        state
    }

    /// Select the given step kind in the wizard step list.
    fn select_step(state: &mut AppState, kind: WizardStepKind) {
        let idx = state
            .install_wizard_state
            .steps
            .iter()
            .position(|s| s.kind == kind)
            .expect("step kind not found in wizard steps");
        state.install_wizard_state.selected_index = idx;
    }

    #[test]
    fn test_android_step_gated_when_jdk_missing() {
        let mut state = wizard_state_with_jdk(ComponentStatus::Missing);
        select_step(&mut state, WizardStepKind::AndroidTools);

        let r = handle_run_selected_step(&mut state);

        assert!(
            r.action.is_none(),
            "must not dispatch RunWizardStep when JDK is missing; got {:?}",
            r.action
        );
        let msg = state
            .install_wizard_state
            .status_message
            .as_deref()
            .unwrap_or("");
        assert!(
            msg.contains("JDK 17"),
            "status_message must mention JDK 17; got: {msg}"
        );
    }

    #[test]
    fn test_android_step_gated_when_jdk_partial() {
        let mut state = wizard_state_with_jdk(ComponentStatus::Partial);
        select_step(&mut state, WizardStepKind::AndroidTools);

        let r = handle_run_selected_step(&mut state);

        assert!(
            r.action.is_none(),
            "must not dispatch when JDK is Partial; got {:?}",
            r.action
        );
    }

    #[test]
    fn test_android_step_gated_when_no_report() {
        let mut state = AppState::new();
        state.show_install_wizard();
        // No report applied — loading is true, report is None.
        // Apply an empty report so steps exist but JDK entry is absent.
        state.install_wizard_state.apply_report(ToolchainReport {
            platform: HostPlatform::Linux,
            shell: HostShell::Bash,
            components: vec![],
            doctor: None,
            linux_package_manager: Some(fdemon_daemon::toolchain::LinuxPackageManager::Unknown),
            winget_available: false,
        });
        select_step(&mut state, WizardStepKind::AndroidTools);

        let r = handle_run_selected_step(&mut state);

        assert!(
            r.action.is_none(),
            "must not dispatch when no JDK entry in report"
        );
    }

    #[test]
    fn test_android_step_dispatches_when_jdk_ok() {
        let mut state = wizard_state_with_jdk(ComponentStatus::Ok);
        select_step(&mut state, WizardStepKind::AndroidTools);

        let r = handle_run_selected_step(&mut state);

        assert!(
            matches!(
                r.action,
                Some(UpdateAction::RunWizardStep {
                    kind: WizardStepKind::AndroidTools,
                    android: Some(_),
                    install: None,
                    ..
                })
            ),
            "must dispatch RunWizardStep(AndroidTools) when JDK is Ok; got {:?}",
            r.action
        );
        assert!(
            state.install_wizard_state.is_step_running(),
            "begin_step must have been called before returning the action"
        );
    }

    #[test]
    fn test_android_step_params_sourced_from_settings() {
        let mut state = wizard_state_with_jdk(ComponentStatus::Ok);
        state.settings.toolchain.android_sdk_root = Some(std::path::PathBuf::from("/opt/android"));
        state.settings.toolchain.android_api_level = 34;
        select_step(&mut state, WizardStepKind::AndroidTools);

        let r = handle_run_selected_step(&mut state);

        if let Some(UpdateAction::RunWizardStep {
            android: Some(params),
            ..
        }) = r.action
        {
            assert_eq!(
                params.sdk_root,
                Some(std::path::PathBuf::from("/opt/android")),
                "sdk_root must be sourced from settings"
            );
            assert_eq!(
                params.api_level, 34,
                "api_level must be sourced from settings"
            );
        } else {
            panic!("expected RunWizardStep with AndroidStepParams");
        }
    }

    #[test]
    fn test_completed_android_persists_sdk_root_and_reruns_preflight() {
        let mut state = wizard_state_with_jdk(ComponentStatus::Ok);
        state
            .install_wizard_state
            .begin_step(WizardStepKind::AndroidTools);
        let root = std::path::PathBuf::from("/home/user/.local/share/fdemon/android");

        let result = handle_step_completed(
            &mut state,
            WizardStepKind::AndroidTools,
            "Android SDK installed".into(),
            Some(root.clone()),
        );

        // settings.toolchain.android_sdk_root must be updated.
        assert_eq!(
            state.settings.toolchain.android_sdk_root.as_ref(),
            Some(&root),
            "android_sdk_root must be written to settings"
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
    fn test_completed_android_without_sdk_root_still_reruns_preflight() {
        let mut state = wizard_state_with_jdk(ComponentStatus::Ok);
        state
            .install_wizard_state
            .begin_step(WizardStepKind::AndroidTools);

        let result = handle_step_completed(
            &mut state,
            WizardStepKind::AndroidTools,
            "Partial install".into(),
            None,
        );

        // No PersistSettings when sdk_path is None.
        assert!(
            result.action.is_none(),
            "must not return PersistSettings when no sdk_root; got {:?}",
            result.action
        );
        // Must still re-run preflight.
        assert!(
            matches!(result.message, Some(Message::InstallWizardRerunPreflight)),
            "must still re-run preflight when sdk_root is absent; got {:?}",
            result.message
        );
    }

    #[test]
    fn test_pathconfig_dispatch_includes_android_sdk_root() {
        let mut state = state_with_preflight();
        // Set an Android SDK root in settings.
        state.settings.toolchain.android_sdk_root =
            Some(std::path::PathBuf::from("/opt/android-sdk"));
        // Give it a Flutter SDK path so PathConfig can resolve a bin dir.
        state.settings.flutter.sdk_path = Some(std::path::PathBuf::from("/opt/flutter"));
        state.install_wizard_state.selected_index = 2; // PathConfig

        let r = handle_run_selected_step(&mut state);

        if let Some(UpdateAction::RunWizardStep {
            kind: WizardStepKind::PathConfig,
            android_sdk_root,
            ..
        }) = r.action
        {
            assert_eq!(
                android_sdk_root,
                Some(std::path::PathBuf::from("/opt/android-sdk")),
                "PathConfig dispatch must include android_sdk_root from settings"
            );
        } else {
            panic!(
                "expected RunWizardStep(PathConfig) with android_sdk_root; got {:?}",
                r.action
            );
        }
    }

    // ── handle_copy_command tests ─────────────────────────────────────────────

    #[test]
    fn test_copy_command_pushes_write_clipboard() {
        // AndroidTools step has a JDK guided command when JDK is missing.
        let mut state = wizard_state_with_jdk(ComponentStatus::Missing);
        select_step(&mut state, WizardStepKind::AndroidTools);

        // Verify precondition: guided command must exist.
        assert!(
            state
                .install_wizard_state
                .selected_guided_command()
                .is_some(),
            "precondition: AndroidTools step must have a guided command when JDK is missing"
        );

        let result = handle_copy_command(&mut state);

        // Must return WriteClipboard action.
        assert!(
            matches!(result.action, Some(UpdateAction::WriteClipboard { .. })),
            "handle_copy_command must return WriteClipboard action; got {:?}",
            result.action
        );
        // Status message must confirm the copy.
        let msg = state
            .install_wizard_state
            .status_message
            .as_deref()
            .unwrap_or("");
        assert!(
            msg.starts_with("Copied:"),
            "status_message must confirm copy; got: {msg}"
        );
    }

    #[test]
    fn test_copy_command_sets_status_when_no_command() {
        // FlutterSdk step has no guided commands.
        let mut state = state_with_preflight();
        state.install_wizard_state.selected_index = 3; // FlutterSdk

        // Verify precondition: no guided command.
        assert!(
            state
                .install_wizard_state
                .selected_guided_command()
                .is_none(),
            "precondition: FlutterSdk step must have no guided commands"
        );

        let result = handle_copy_command(&mut state);

        // Must return no action.
        assert!(
            result.action.is_none(),
            "handle_copy_command must return no action when no command; got {:?}",
            result.action
        );
        // Status message must explain there's nothing to copy.
        let msg = state
            .install_wizard_state
            .status_message
            .as_deref()
            .unwrap_or("");
        assert!(
            msg.contains("No command"),
            "status_message must indicate no command available; got: {msg}"
        );
    }

    // ── m3: PathConfig ordering hint ─────────────────────────────────────────

    #[test]
    fn test_pathconfig_hints_when_android_sdk_root_absent() {
        // PathConfig should still execute when android_sdk_root is None,
        // but must set a non-blocking status_message hinting to run Android Tools first.
        let mut state = state_with_preflight();
        state.settings.toolchain.android_sdk_root = None;
        state.settings.flutter.sdk_path = Some(std::path::PathBuf::from("/opt/flutter"));
        state.install_wizard_state.selected_index = 2; // PathConfig

        let r = handle_run_selected_step(&mut state);

        // Step must still execute (action must be Some).
        assert!(
            matches!(
                r.action,
                Some(UpdateAction::RunWizardStep {
                    kind: WizardStepKind::PathConfig,
                    ..
                })
            ),
            "PathConfig must dispatch even when android_sdk_root is None; got {:?}",
            r.action
        );
        // A hint must be present.
        let msg = state
            .install_wizard_state
            .status_message
            .as_deref()
            .unwrap_or("");
        assert!(
            !msg.is_empty(),
            "status_message must be set when android_sdk_root is None"
        );
        assert!(
            msg.contains("Android"),
            "hint must mention Android Tools; got: {msg}"
        );
    }

    #[test]
    fn test_pathconfig_no_hint_when_android_sdk_root_present() {
        // When android_sdk_root is already set, no ordering hint should be emitted.
        let mut state = state_with_preflight();
        state.settings.toolchain.android_sdk_root =
            Some(std::path::PathBuf::from("/opt/android-sdk"));
        state.settings.flutter.sdk_path = Some(std::path::PathBuf::from("/opt/flutter"));
        state.install_wizard_state.selected_index = 2; // PathConfig

        handle_run_selected_step(&mut state);

        // No hint expected (status_message should be None).
        assert!(
            state.install_wizard_state.status_message.is_none(),
            "no status_message expected when android_sdk_root is present"
        );
    }

    // ── m2: no-JDK-entry gate/guided-command agreement ───────────────────────

    #[test]
    fn test_android_step_gated_and_guided_command_shown_when_no_jdk_entry() {
        // When the report has no Jdk component at all, the gate must block the
        // executor AND the guided command must be shown in the step (both derive
        // from `is_jdk_actionable`).
        let mut state = AppState::new();
        state.show_install_wizard();
        // Report with android tools but no Jdk entry.
        state.install_wizard_state.apply_report(ToolchainReport {
            platform: HostPlatform::Linux,
            shell: HostShell::Bash,
            components: vec![ComponentCheck {
                kind: ComponentKind::AndroidCmdlineTools,
                status: ComponentStatus::Missing,
                detail: String::new(),
            }],
            doctor: None,
            linux_package_manager: Some(fdemon_daemon::toolchain::LinuxPackageManager::Unknown),
            winget_available: false,
        });
        select_step(&mut state, WizardStepKind::AndroidTools);

        // Gate must block.
        let r = handle_run_selected_step(&mut state);
        assert!(
            r.action.is_none(),
            "must not dispatch when no Jdk entry in report (m2); got {:?}",
            r.action
        );

        // Guided command must be visible in the step (build_steps used same helper).
        let android_step = state
            .install_wizard_state
            .steps
            .iter()
            .find(|s| s.kind == WizardStepKind::AndroidTools)
            .expect("AndroidTools step must exist");
        assert_eq!(
            android_step.guided_commands.len(),
            1,
            "guided command must be shown when no Jdk entry (m2 fix)"
        );
    }

    #[test]
    fn test_copy_command_text_matches_guided_command() {
        // AndroidTools with missing JDK → guided command is JDK install cmd.
        let mut state = wizard_state_with_jdk(ComponentStatus::Missing);
        select_step(&mut state, WizardStepKind::AndroidTools);

        let expected_cmd = state
            .install_wizard_state
            .selected_guided_command()
            .map(|c| c.command.clone())
            .unwrap();

        let result = handle_copy_command(&mut state);

        if let Some(UpdateAction::WriteClipboard { text }) = result.action {
            assert_eq!(
                text, expected_cmd,
                "WriteClipboard text must match the guided command"
            );
        } else {
            panic!("expected WriteClipboard action");
        }
    }

    // ── Prerequisites vs Doctor status message ───────────────────────────────

    #[test]
    fn test_prerequisites_enter_returns_guided_message_not_later_phase() {
        // Prerequisites is non-executable; pressing Enter must set a "guided"
        // status message directing the user to run listed command(s), not the
        // old "Available in a later phase" stub.
        let mut state = state_with_preflight();
        select_step(&mut state, WizardStepKind::Prerequisites);

        let result = handle_run_selected_step(&mut state);

        // Must not dispatch RunWizardStep.
        assert!(
            result.action.is_none(),
            "Prerequisites Enter must not dispatch RunWizardStep; got {:?}",
            result.action
        );
        assert!(
            result.message.is_none(),
            "Prerequisites Enter must not dispatch any message; got {:?}",
            result.message
        );
        // Status message must be the new guided message.
        let msg = state
            .install_wizard_state
            .status_message
            .as_deref()
            .unwrap_or("");
        assert!(
            msg.contains("Run the listed command") || msg.contains("re-check"),
            "Prerequisites status_message must be the guided message; got: {msg}"
        );
        assert!(
            !msg.contains("later phase"),
            "Prerequisites must not show 'later phase' message anymore; got: {msg}"
        );
    }

    #[test]
    fn test_doctor_enter_still_returns_later_phase_message() {
        // Doctor step must still show "Available in a later phase" — unchanged.
        let mut state = state_with_preflight();
        select_step(&mut state, WizardStepKind::Doctor);

        let result = handle_run_selected_step(&mut state);

        assert!(
            result.action.is_none(),
            "Doctor Enter must not dispatch RunWizardStep"
        );
        let msg = state
            .install_wizard_state
            .status_message
            .as_deref()
            .unwrap_or("");
        assert!(
            msg.contains("later phase"),
            "Doctor must still show 'later phase' message; got: {msg}"
        );
    }

    // ── Task 03: cancel step + retry-failure affordance ──────────────────────

    /// `handle_cancel_step` must clear the handle slot and reset step to Idle.
    #[tokio::test]
    async fn cancel_step_clears_handle_and_resets_status() {
        let mut state = state_with_preflight();
        // Simulate a running step with a task handle.
        state
            .install_wizard_state
            .begin_step(WizardStepKind::FlutterSdk);
        assert!(state.install_wizard_state.is_step_running());

        // Populate install_task with a trivial no-op handle.
        let token = tokio_util::sync::CancellationToken::new();
        state.install_wizard_state.install_task = Some(crate::install_wizard::InstallTaskHandle {
            join: tokio::spawn(std::future::ready(())),
            cancel: token,
        });
        assert!(state.install_wizard_state.install_task.is_some());

        handle_cancel_step(&mut state);

        // After cancel: task handle must be gone.
        assert!(
            state.install_wizard_state.install_task.is_none(),
            "install_task must be None after cancel"
        );
        // Step must be Idle so the next Enter retries.
        assert!(
            !state.install_wizard_state.is_step_running(),
            "step must not be running after cancel"
        );
        // Status message must be set.
        let status = state
            .install_wizard_state
            .status_message
            .as_deref()
            .unwrap_or("");
        assert!(
            status.contains("Cancelled") || status.contains("retry"),
            "status_message must mention 'Cancelled' or 'retry'; got: {status}"
        );
    }

    /// Cancelling with no running task must be a no-op (idempotent).
    #[test]
    fn cancel_step_is_idempotent_when_no_task() {
        let mut state = state_with_preflight();
        assert!(!state.install_wizard_state.is_step_running());
        assert!(state.install_wizard_state.install_task.is_none());

        // Must not panic and must return UpdateResult::none().
        let result = handle_cancel_step(&mut state);

        assert!(result.action.is_none());
        assert!(result.message.is_none());
    }

    /// A genuine failure (non-Cancelled reason) must set the retry prompt.
    #[test]
    fn step_failed_sets_retry_prompt() {
        let mut state = state_with_preflight();
        state
            .install_wizard_state
            .begin_step(WizardStepKind::FlutterSdk);

        handle_step_failed(&mut state, "network timeout".to_string());

        let msg = state
            .install_wizard_state
            .status_message
            .as_deref()
            .unwrap_or("");
        assert!(
            msg.contains("press Enter to retry") || msg.contains("r to re-check"),
            "status_message must contain retry prompt; got: {msg}"
        );
        // Must not say "Cancelled".
        assert!(
            !msg.to_lowercase().contains("cancelled"),
            "genuine failure must not say Cancelled; got: {msg}"
        );
    }

    /// A Cancelled reason must set a neutral message, not the "Failed" prompt.
    #[test]
    fn step_failed_with_cancelled_prefix_sets_neutral_message() {
        let mut state = state_with_preflight();
        state
            .install_wizard_state
            .begin_step(WizardStepKind::FlutterSdk);

        handle_step_failed(
            &mut state,
            "Cancelled: Flutter install cancelled before start".to_string(),
        );

        let msg = state
            .install_wizard_state
            .status_message
            .as_deref()
            .unwrap_or("");
        assert!(
            msg.contains("Cancelled") || msg.contains("retry"),
            "cancelled failure must set a neutral or retry message; got: {msg}"
        );
        // Must not say "Failed —" (the genuine failure prompt).
        assert!(
            !msg.starts_with("Failed"),
            "cancelled path must not start with 'Failed'; got: {msg}"
        );
    }
}
