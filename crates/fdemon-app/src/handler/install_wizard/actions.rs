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
use crate::install_wizard::{is_jdk_actionable, InstallTaskHandle, WizardStepKind};
use crate::message::Message;
use crate::state::AppState;
use fdemon_daemon::toolchain::ToolchainReport;
use tokio_util::sync::CancellationToken;

/// Handle `ToolchainPreflightCompleted` — populate the wizard with the report.
///
/// Calls `apply_report` to build the five UI steps from the report,
/// clears `loading`, and clears any status message.
///
/// Also fires `UpdateAction::ScanInstalledSdks` so the Flutter Version panel's
/// cache is refreshed after a managed SDK install completes and the preflight
/// re-runs (part of the `WizardStepCompleted(FlutterSdk)` message chain).
///
/// **Handback (Phase 5, Task 04).** When the report shows the Flutter SDK is
/// live (`flutter_now_live()`) and the handback guard has not already fired
/// (`handback_done == false`), the wizard is auto-closed and a
/// `DiscoverDevices` action is returned so the launch dialog is populated.
/// The `handback_done` flag prevents a second discovery if the user also
/// manually closes the wizard.
pub fn handle_preflight_completed(state: &mut AppState, report: ToolchainReport) -> UpdateResult {
    state.install_wizard_state.apply_report(report);
    state.install_wizard_state.status_message = None;

    // Refresh the FVM cache so the Flutter Version panel shows the newly
    // installed SDK.  `active_sdk_root` comes from the just-resolved SDK —
    // this is the same pattern used by `handle_switch_completed`.
    let active_sdk_root = state.resolved_sdk.as_ref().map(|sdk| sdk.root.clone());
    let scan_action = UpdateAction::ScanInstalledSdks { active_sdk_root };

    // Handback: auto-close the wizard and dispatch device discovery when
    // Flutter is now live and the guard has not already fired.
    if state.install_wizard_state.flutter_now_live() && !state.install_wizard_state.handback_done {
        if let Some(discover) = close_wizard_and_dispatch_discovery(state) {
            return UpdateResult::actions_vec(vec![scan_action, discover]);
        }
    }

    UpdateResult::action(scan_action)
}

/// Shared handback helper: close the wizard, transition to the correct mode,
/// set the one-shot guard, and return a `DiscoverDevices` action when a live
/// SDK is available.
///
/// Always closes the wizard (sets `visible = false`). When a live Flutter
/// executable exists, transitions to `UiMode::Startup` and returns
/// `Some(DiscoverDevices)`; otherwise transitions to `UiMode::Normal` and
/// returns `None`.
///
/// This function is the **single source of truth** for the post-install
/// handback transition. Both the auto-close path (`handle_preflight_completed`)
/// and the manual-close path (`maybe_dispatch_discovery_on_close` in
/// `navigation.rs`) delegate here so the two paths cannot drift.
///
/// **Critical invariant:** when a live SDK is present, `ui_mode` is set to
/// `UiMode::Startup` (not `Normal`) so the subsequent `DevicesDiscovered`
/// message populates `new_session_dialog_state.target_selector` (the handler
/// guards on `UiMode::Startup | UiMode::NewSessionDialog`).
pub(super) fn close_wizard_and_dispatch_discovery(state: &mut AppState) -> Option<UpdateAction> {
    if let Some(flutter) = state.flutter_executable() {
        state.install_wizard_state.handback_done = true;
        state.hide_install_wizard();
        // Override the Normal mode set by hide_install_wizard() with Startup,
        // so the subsequent DevicesDiscovered message populates the selector.
        state.ui_mode = crate::state::UiMode::Startup;
        Some(UpdateAction::DiscoverDevices { flutter })
    } else {
        state.hide_install_wizard();
        None
    }
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
    let android_sdk_root = state.settings.toolchain.android_sdk_root.clone();

    UpdateResult::action(UpdateAction::RunToolchainPreflight {
        project_path,
        explicit_sdk_path,
        android_sdk_root,
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
            // begin_step clears any prior install_task and bumps run_seq (F8).
            state.install_wizard_state.begin_step(kind);

            // Mint the cancellation token synchronously (F3 fix) and store it
            // in state immediately so `Esc` can fire it even before the
            // `WizardInstallTaskReady` message arrives.
            let cancel_token = CancellationToken::new();
            let run_seq = state.install_wizard_state.run_seq;
            state.install_wizard_state.install_task = Some(InstallTaskHandle {
                cancel: cancel_token.clone(),
                join: None,
            });

            UpdateResult::action(UpdateAction::RunWizardStep {
                kind,
                run_seq,
                cancel_token,
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
                cmdline_tools_sha256: ts.cmdline_tools_sha256.clone(),
            };

            // Flip UI to Running immediately before the async round-trip.
            // begin_step clears any prior install_task and bumps run_seq (F8).
            state.install_wizard_state.begin_step(kind);

            // Mint the cancellation token synchronously (F3 fix).
            let cancel_token = CancellationToken::new();
            let run_seq = state.install_wizard_state.run_seq;
            state.install_wizard_state.install_task = Some(InstallTaskHandle {
                cancel: cancel_token.clone(),
                join: None,
            });

            UpdateResult::action(UpdateAction::RunWizardStep {
                kind,
                run_seq,
                cancel_token,
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
                    // Prefer the settings-stored root (set by a completed AndroidTools
                    // step), then fall back to the shared resolver ($ANDROID_HOME /
                    // $ANDROID_SDK_ROOT / platform default). Only show the ordering tip
                    // when no Android SDK exists anywhere — consistent with what the
                    // executor will actually do.
                    let android_sdk_root = state
                        .settings
                        .toolchain
                        .android_sdk_root
                        .clone()
                        .or_else(|| {
                            let p = fdemon_daemon::resolve_android_sdk_root_path(None);
                            if p.is_dir() {
                                Some(p)
                            } else {
                                None
                            }
                        });

                    // Ordering hint (m3): Android Tools should ideally be run before
                    // PathConfig so that ANDROID_HOME is also written. This is a soft
                    // hint — PathConfig still executes (it will write the Flutter PATH
                    // regardless). Only show the tip when no Android SDK is discoverable
                    // at all (settings None, env unset, default absent).
                    if android_sdk_root.is_none() {
                        state.install_wizard_state.status_message = Some(
                            "Tip: run Android Tools first so ANDROID_HOME is also configured."
                                .into(),
                        );
                    }

                    // Flip UI to Running immediately.
                    // begin_step clears any prior install_task and bumps run_seq (F8).
                    state.install_wizard_state.begin_step(kind);

                    // Mint the cancellation token synchronously (F3 fix).
                    let cancel_token = CancellationToken::new();
                    let run_seq = state.install_wizard_state.run_seq;
                    state.install_wizard_state.install_task = Some(InstallTaskHandle {
                        cancel: cancel_token.clone(),
                        join: None,
                    });

                    UpdateResult::action(UpdateAction::RunWizardStep {
                        kind,
                        run_seq,
                        cancel_token,
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

/// Handle `WizardStepStarted` — guard against stale cross-kind messages, then
/// reset the progress display fields.
///
/// ## Seq-guard (F-PR53-01)
///
/// `WizardStepStarted` now carries the `run_seq` assigned at dispatch.  Any
/// message whose `run_seq` does not equal `install_wizard_state.run_seq` is a
/// **no-op**: it means the message was emitted by a run that has already been
/// superseded (e.g. the user pressed Esc and then Enter before the first run's
/// async task sent its announce).  Discarding it prevents the cross-kind zombie:
///
/// ```text
/// Run A (AndroidTools, seq=1) → Esc → Run B (FlutterSdk, seq=2)
/// Delayed WizardStepStarted{AndroidTools, seq=1} arrives:
///   seq 1 ≠ state.run_seq 2  →  no-op  →  Run B survives intact
/// ```
///
/// ## Normal flow (same-seq, same-kind)
///
/// `handle_run_selected_step` always calls `begin_step(kind)` before
/// dispatching `RunWizardStep`, so by the time a current-seq Started arrives
/// the step is already `Running` for the same `kind`.  In that case this
/// handler calls `reset_progress_display()` — which clears only the visible
/// progress/log/summary fields — without touching `install_task` or `run_seq`.
///
/// ## Dropped fallback
///
/// The prior `begin_step(kind)` defensive fallback has been removed.
/// `handle_run_selected_step` is the single code path that calls `begin_step`,
/// so a current-seq Started is always already Running for its kind.  The
/// fallback path was reachable only by stale messages, which are now caught by
/// the seq-guard above.
pub fn handle_step_started(
    state: &mut AppState,
    _kind: WizardStepKind,
    run_seq: u64,
) -> UpdateResult {
    // Seq-guard: discard any Started from a superseded run.
    if run_seq != state.install_wizard_state.run_seq {
        return UpdateResult::none();
    }

    // Normal flow: the step is already Running for this kind (begin_step was
    // called synchronously by handle_run_selected_step before dispatch).
    // Reset only the progress display — do NOT clear install_task or run_seq.
    state.install_wizard_state.reset_progress_display();

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

        // Re-run preflight so the step list reflects the just-written PATH/env
        // (otherwise the PathConfig step stays visually stale until the user
        // manually re-checks).
        return UpdateResult::message(Message::InstallWizardRerunPreflight);
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
        // User-initiated cancellation: route through the Cancelled variant so
        // the TUI renders a neutral (non-red) result summary and suppresses
        // the run-failed badge.  The step is still retriable via Enter.
        state
            .install_wizard_state
            .finish_step(StepExecStatus::Cancelled, reason);
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

/// Handle `WizardInstallTaskReady` — upgrade the stored handle's `join` field.
///
/// The cancellation token has already been stored synchronously by
/// `handle_run_selected_step`; this message carries only the `JoinHandle`
/// (which is not available until after `tokio::spawn` returns).
///
/// **Validation (F4/F7):** The `kind` and `run_seq` fields must match the
/// current run before upgrading. If they don't match (e.g., the message
/// belongs to a previous run that was cancelled), the `JoinHandle` is
/// aborted and the message is silently discarded — the live `install_task`
/// is left untouched.
///
/// Also discards (with abort) when no step is currently running (the step
/// already completed before the ready arrived — fast-finish F7 path).
pub fn handle_install_task_ready(
    state: &mut AppState,
    kind: WizardStepKind,
    run_seq: u64,
    handle: std::sync::Arc<std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>>,
) -> UpdateResult {
    // Extract the JoinHandle from the Arc<Mutex<Option<>>>.
    let join = handle.lock().ok().and_then(|mut g| g.take());

    // Guard 1: no step is currently running — the step already completed or
    // was cancelled before this ready arrived. Abort the handle and discard.
    if !state.install_wizard_state.is_step_running() {
        if let Some(j) = join {
            j.abort();
        }
        return UpdateResult::none();
    }

    // Guard 2: kind/run_seq mismatch — this ready belongs to a superseded run.
    // Abort the handle and discard without touching the live install_task.
    let current_kind = state.install_wizard_state.execution.kind;
    let current_seq = state.install_wizard_state.run_seq;
    if current_kind != Some(kind) || current_seq != run_seq {
        if let Some(j) = join {
            j.abort();
        }
        return UpdateResult::none();
    }

    // Validated: upgrade the existing handle's `join` field.
    if let Some(task) = state.install_wizard_state.install_task.as_mut() {
        task.join = join;
    }
    UpdateResult::none()
}

/// Handle `InstallWizardCancelStep` — signal the running install to stop.
///
/// Cancels the token stored on `install_task`, aborts the join handle as a
/// backstop, resets the step to idle, and sets a neutral "Cancelled" status
/// message.
///
/// **Defensive (F3 fix):** only resets the step to Idle when a token was
/// actually present and fired, preventing a silent `Running → Idle` flip
/// from a future regression where `install_task` is `None` while a step is
/// still running (which would leave an orphaned download holding the lock).
///
/// Idempotent — a second cancel with no running task is a no-op.
pub fn handle_cancel_step(state: &mut AppState) -> UpdateResult {
    if let Some(task) = state.install_wizard_state.install_task.take() {
        // Signal the install loop to stop at the next cancellation checkpoint.
        task.cancel.cancel();
        // Abort the task as a backstop in case the install loop doesn't check
        // the token frequently enough (e.g., during a blocking git-clone).
        if let Some(j) = task.join {
            j.abort();
        }
        // Token was fired — safe to reset the step to Idle.
        state.install_wizard_state.reset_running_step_to_idle();
        state.install_wizard_state.status_message =
            Some("Cancelled. Press Enter to retry.".to_string());
    }
    // If install_task was None, we do NOT reset to Idle — this prevents a
    // silent flip that would hide an orphaned task holding the install lock.
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

    // ── F-PR53-12: execution cleared on apply_report / handle_preflight_completed ─

    /// F-PR53-12: after `handle_step_failed` → `handle_preflight_completed`,
    /// `execution` must be back to `Idle` so the detail pane renders the
    /// refreshed component list rather than the stale "Failed" view.
    #[test]
    fn test_preflight_completed_after_failed_step_clears_execution() {
        let mut state = AppState::new();
        state.show_install_wizard();
        state.install_wizard_state.apply_report(make_report());

        // Simulate a failed step run.
        state
            .install_wizard_state
            .begin_step(WizardStepKind::FlutterSdk);
        handle_step_failed(&mut state, "network timeout".into());
        assert_eq!(
            state.install_wizard_state.execution.status,
            crate::install_wizard::StepExecStatus::Failed,
            "precondition: execution must be Failed after handle_step_failed"
        );

        // User fixes the issue and re-checks — a new report arrives.
        handle_preflight_completed(&mut state, make_report());

        // execution must be back to default so the component list renders.
        assert_eq!(
            state.install_wizard_state.execution.status,
            crate::install_wizard::StepExecStatus::Idle,
            "handle_preflight_completed must reset execution to Idle"
        );
        assert_eq!(
            state.install_wizard_state.execution.kind, None,
            "handle_preflight_completed must clear execution.kind"
        );
    }

    /// F-PR53-12 (Cancelled variant): after a user-cancelled step, re-check
    /// via `handle_preflight_completed` must clear execution.
    #[test]
    fn test_preflight_completed_after_cancelled_step_clears_execution() {
        let mut state = AppState::new();
        state.show_install_wizard();
        state.install_wizard_state.apply_report(make_report());

        state
            .install_wizard_state
            .begin_step(WizardStepKind::AndroidTools);
        handle_step_failed(&mut state, "Cancelled: user pressed Esc".into());
        assert_eq!(
            state.install_wizard_state.execution.status,
            crate::install_wizard::StepExecStatus::Cancelled,
            "precondition: execution must be Cancelled"
        );

        handle_preflight_completed(&mut state, make_report());

        assert_eq!(
            state.install_wizard_state.execution.status,
            crate::install_wizard::StepExecStatus::Idle,
            "handle_preflight_completed must reset Cancelled execution to Idle"
        );
    }

    /// F-PR53-12 regression: handback auto-close still works when
    /// `apply_report` resets `execution`. The predicate `flutter_now_live()`
    /// reads `report.components`, not `execution`, so clearing execution must
    /// not break the auto-close path.
    #[test]
    fn test_preflight_completed_handback_still_fires_after_execution_reset() {
        let mut state = AppState::new();
        state.show_install_wizard();
        state.install_wizard_state.apply_report(make_report());

        // Simulate a successful step run that leaves execution = Succeeded.
        state
            .install_wizard_state
            .begin_step(WizardStepKind::FlutterSdk);
        state.install_wizard_state.finish_step(
            crate::install_wizard::StepExecStatus::Succeeded,
            "done".into(),
        );

        // Inject a live SDK so close_wizard_and_dispatch_discovery returns Some.
        inject_live_sdk(&mut state);
        assert!(!state.install_wizard_state.handback_done, "precondition");

        // A re-check report arrives with Flutter live.
        let result = handle_preflight_completed(&mut state, make_live_flutter_report());

        // Wizard must auto-close.
        assert!(
            !state.install_wizard_state.visible,
            "wizard must auto-close on live Flutter report"
        );
        // handback_done guard must be set.
        assert!(
            state.install_wizard_state.handback_done,
            "handback_done must be set after auto-close"
        );
        // DiscoverDevices action must be returned.
        let actions = result.actions();
        assert!(
            actions
                .iter()
                .any(|a| matches!(a, UpdateAction::DiscoverDevices { .. })),
            "DiscoverDevices action must be returned even when execution was reset; got {:?}",
            actions
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
        let current_seq = state.install_wizard_state.run_seq;
        // WizardStepStarted arrives from the executor with the same run_seq.
        handle_step_started(&mut state, WizardStepKind::FlutterSdk, current_seq);
        // Must still be Running (not reset to Idle).
        assert!(state.install_wizard_state.is_step_running());
        assert_eq!(
            state.install_wizard_state.execution.kind,
            Some(WizardStepKind::FlutterSdk)
        );
    }

    /// `handle_step_started` must NOT clobber the synchronously-stored
    /// `install_task` or bump `run_seq` when the step is already Running for
    /// the same kind (the normal flow after `handle_run_selected_step`).
    ///
    /// This is the regression guard for the F3-race defect: if `begin_step` is
    /// called again inside `handle_step_started`, `install_task` becomes `None`
    /// during the running window and `Esc` cannot fire the token.
    #[test]
    fn test_step_started_preserves_install_task_and_run_seq() {
        let mut state = state_with_preflight();
        state.install_wizard_state.selected_index = 3; // FlutterSdk

        // Drive handle_run_selected_step — this calls begin_step, mints the
        // token, and stores install_task synchronously.
        let result = handle_run_selected_step(&mut state);
        assert!(
            result.action.is_some(),
            "precondition: action must be dispatched"
        );

        // Capture the run_seq and verify install_task is Some.
        let run_seq_after_dispatch = state.install_wizard_state.run_seq;
        assert!(
            state.install_wizard_state.install_task.is_some(),
            "install_task must be Some immediately after handle_run_selected_step (precondition)"
        );

        // Now simulate WizardStepStarted arriving from the executor (same seq).
        handle_step_started(
            &mut state,
            WizardStepKind::FlutterSdk,
            run_seq_after_dispatch,
        );

        // install_task must STILL be Some — the token was not cleared.
        assert!(
            state.install_wizard_state.install_task.is_some(),
            "install_task must still be Some after handle_step_started \
             (clobbering would break Esc cancellation)"
        );
        // run_seq must NOT have been bumped again.
        assert_eq!(
            state.install_wizard_state.run_seq, run_seq_after_dispatch,
            "run_seq must NOT be incremented by handle_step_started \
             (a second bump would cause the legitimate WizardInstallTaskReady to be discarded)"
        );
        // Step must still be Running.
        assert!(
            state.install_wizard_state.is_step_running(),
            "step must still be Running after handle_step_started"
        );
    }

    /// A `WizardStepStarted` whose `run_seq` does not match the current
    /// `install_wizard_state.run_seq` must be a complete no-op: `install_task`,
    /// `run_seq`, and `execution.kind/status` are all unchanged.
    ///
    /// This is the core guard for the cross-kind zombie race (F-PR53-01):
    ///
    /// ```text
    /// Run A (AndroidTools, seq=N) starts
    /// Esc → handle_cancel_step takes install_task, resets to Idle
    /// Enter → Run B (FlutterSdk, seq=N+1) begin_step; install_task=Some{cancelB}
    /// Delayed WizardStepStarted{AndroidTools, seq=N} arrives:
    ///   seq N ≠ state.run_seq (N+1) → no-op → Run B survives intact
    /// ```
    #[test]
    fn test_stale_cross_kind_step_started_is_noop() {
        let mut state = state_with_preflight();
        state.install_wizard_state.selected_index = 3; // FlutterSdk

        // Simulate Run B: begin_step(FlutterSdk) with seq = N (e.g. 2).
        // We do this via handle_run_selected_step to get a real install_task.
        let result = handle_run_selected_step(&mut state);
        assert!(
            result.action.is_some(),
            "precondition: action must be dispatched"
        );
        let run_seq_b = state.install_wizard_state.run_seq; // This is N (e.g. 1 after first begin_step)

        // Stale run_seq simulating a delayed WizardStepStarted from Run A.
        // Use a seq that is not equal to the current one.
        let stale_seq = run_seq_b.wrapping_sub(1);

        // Feed the stale cross-kind Started (AndroidTools with Run A's seq).
        handle_step_started(&mut state, WizardStepKind::AndroidTools, stale_seq);

        // install_task must still be Some (Run B's cancelB token is intact).
        assert!(
            state.install_wizard_state.install_task.is_some(),
            "install_task must still be Some after stale Started (Run B's token must survive)"
        );
        // run_seq must NOT have been bumped.
        assert_eq!(
            state.install_wizard_state.run_seq, run_seq_b,
            "run_seq must NOT be bumped by a stale Started"
        );
        // execution.kind must still be FlutterSdk (Run B's kind).
        assert_eq!(
            state.install_wizard_state.execution.kind,
            Some(WizardStepKind::FlutterSdk),
            "execution.kind must remain FlutterSdk after stale AndroidTools Started"
        );
        // Step must still be Running.
        assert!(
            state.install_wizard_state.is_step_running(),
            "step must still be Running after stale Started"
        );
    }

    /// A `WizardStepStarted` with the current `run_seq` and the same kind as
    /// the running step routes through `reset_progress_display()` — preserving
    /// `install_task` and `run_seq` while clearing the visible progress fields.
    ///
    /// Regression guard for Phase-5 task 02: the same-kind, current-seq path
    /// must NOT call `begin_step` or otherwise drop the token.
    #[test]
    fn test_step_started_with_current_seq_same_kind_preserves_task() {
        let mut state = state_with_preflight();
        state.install_wizard_state.selected_index = 3; // FlutterSdk

        // Drive handle_run_selected_step to get a real install_task.
        let result = handle_run_selected_step(&mut state);
        assert!(
            result.action.is_some(),
            "precondition: action must be dispatched"
        );

        let run_seq = state.install_wizard_state.run_seq;
        assert!(
            state.install_wizard_state.install_task.is_some(),
            "precondition: install_task must be Some"
        );

        // Feed the Started with the correct (current) run_seq and same kind.
        handle_step_started(&mut state, WizardStepKind::FlutterSdk, run_seq);

        // install_task preserved.
        assert!(
            state.install_wizard_state.install_task.is_some(),
            "install_task must be preserved by same-seq same-kind Started"
        );
        // run_seq preserved.
        assert_eq!(
            state.install_wizard_state.run_seq, run_seq,
            "run_seq must be preserved by same-seq same-kind Started"
        );
        // Step still Running for FlutterSdk.
        assert!(state.install_wizard_state.is_step_running());
        assert_eq!(
            state.install_wizard_state.execution.kind,
            Some(WizardStepKind::FlutterSdk)
        );
    }

    #[test]
    fn test_completed_inert_step_returns_none() {
        // A step with no completion side-effect (Doctor) chains nothing.
        // (FlutterSdk/AndroidTools persist+rerun, PathConfig reruns — covered
        // by their own tests.)
        let mut state = state_with_preflight();
        state
            .install_wizard_state
            .begin_step(WizardStepKind::Doctor);

        let result = handle_step_completed(
            &mut state,
            WizardStepKind::Doctor,
            "Doctor done".into(),
            None,
        );

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
    fn test_completed_pathconfig_reruns_preflight() {
        // Bug fix: PathConfig completion used to return `none()`, leaving the
        // step list visually stale after PATH/env were written. It must now
        // re-run preflight.
        let mut state = wizard_state_with_jdk(ComponentStatus::Ok);
        state
            .install_wizard_state
            .begin_step(WizardStepKind::PathConfig);

        let result = handle_step_completed(
            &mut state,
            WizardStepKind::PathConfig,
            "PATH configured".into(),
            None,
        );

        assert!(
            matches!(result.message, Some(Message::InstallWizardRerunPreflight)),
            "PathConfig completion must re-run preflight; got {:?}",
            result.message
        );
        // The session stash is still cleared.
        assert!(state.install_wizard_state.installed_sdk_path.is_none());
    }

    #[test]
    fn test_rerun_preflight_forwards_android_sdk_root_override() {
        // The persisted Android SDK root must be threaded into the preflight
        // action so the re-check finds a just-installed SDK even when the
        // running process's $ANDROID_HOME is stale.
        let mut state = AppState::new();
        state.show_install_wizard();
        state.install_wizard_state.apply_report(make_report()); // loading = false
        let root = std::path::PathBuf::from("/home/user/Android/Sdk");
        state.settings.toolchain.android_sdk_root = Some(root.clone());

        let result = handle_rerun_preflight(&mut state);

        match result.action {
            Some(UpdateAction::RunToolchainPreflight {
                android_sdk_root, ..
            }) => {
                assert_eq!(
                    android_sdk_root,
                    Some(root),
                    "rerun preflight must forward settings.toolchain.android_sdk_root as the override"
                );
            }
            other => panic!("expected RunToolchainPreflight; got {other:?}"),
        }
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
    #[serial_test::serial]
    fn test_pathconfig_hints_when_android_sdk_root_absent() {
        // PathConfig should still execute when android_sdk_root is None and no
        // Android SDK is discoverable anywhere (env vars unset, default absent).
        // Must set a non-blocking status_message hinting to run Android Tools first.
        std::env::remove_var("ANDROID_HOME");
        std::env::remove_var("ANDROID_SDK_ROOT");

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

        // The dispatched android_sdk_root should be None when no SDK is anywhere.
        // (This may be Some if the platform default dir happens to exist on this machine,
        //  so we only verify the tip when android_sdk_root is None in the action.)
        if let Some(UpdateAction::RunWizardStep {
            android_sdk_root: dispatched_sdk_root,
            ..
        }) = &r.action
        {
            if dispatched_sdk_root.is_none() {
                // A hint must be present when no SDK was found.
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
        }
    }

    #[test]
    fn test_pathconfig_no_hint_when_android_sdk_root_present() {
        // When android_sdk_root is already set in settings, no ordering hint should be emitted.
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

    /// When settings android_sdk_root is None but $ANDROID_HOME points to a dir
    /// that exists, the dispatch must include the resolved root (no tip).
    #[test]
    #[serial_test::serial]
    fn test_pathconfig_no_hint_when_android_home_env_set_to_existing_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let android_home = tmp.path().join("android_sdk");
        std::fs::create_dir_all(&android_home).unwrap();

        std::env::set_var("ANDROID_HOME", android_home.as_os_str());
        std::env::remove_var("ANDROID_SDK_ROOT");

        let mut state = state_with_preflight();
        state.settings.toolchain.android_sdk_root = None; // not set in settings
        state.settings.flutter.sdk_path = Some(std::path::PathBuf::from("/opt/flutter"));
        state.install_wizard_state.selected_index = 2; // PathConfig

        let r = handle_run_selected_step(&mut state);

        std::env::remove_var("ANDROID_HOME");

        // The dispatched android_sdk_root must be the resolved dir (not None).
        if let Some(UpdateAction::RunWizardStep {
            android_sdk_root, ..
        }) = r.action
        {
            assert_eq!(
                android_sdk_root.as_deref(),
                Some(android_home.as_path()),
                "dispatch must include the resolved $ANDROID_HOME when it exists"
            );
        } else {
            panic!("expected RunWizardStep dispatch; got: {:?}", r.action);
        }

        // No ordering tip when SDK was discovered via env var.
        assert!(
            state.install_wizard_state.status_message.is_none(),
            "no tip expected when $ANDROID_HOME resolves to an existing dir; got: {:?}",
            state.install_wizard_state.status_message
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

        // Populate install_task with a trivial no-op handle (join is Some).
        let token = tokio_util::sync::CancellationToken::new();
        state.install_wizard_state.install_task = Some(crate::install_wizard::InstallTaskHandle {
            cancel: token,
            join: Some(tokio::spawn(std::future::ready(()))),
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

    // ── Phase 5, Task 04: launch-dialog handback ──────────────────────────────

    /// Helper: inject a minimal `FlutterSdk` so that `flutter_executable()` returns `Some`.
    fn inject_live_sdk(state: &mut AppState) {
        use fdemon_daemon::{FlutterExecutable, FlutterSdk, SdkSource};
        state.resolved_sdk = Some(FlutterSdk {
            root: std::path::PathBuf::from("/opt/flutter"),
            executable: FlutterExecutable::Direct(std::path::PathBuf::from(
                "/opt/flutter/bin/flutter",
            )),
            source: SdkSource::ExplicitConfig,
            version: "3.27.0".to_string(),
            channel: Some("stable".to_string()),
        });
    }

    /// Build a report where the Flutter SDK component is `Ok` (Flutter is live).
    fn make_live_flutter_report() -> ToolchainReport {
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

    /// Build a report where the Flutter SDK component is `Missing`.
    fn make_dead_flutter_report() -> ToolchainReport {
        ToolchainReport {
            platform: HostPlatform::Linux,
            shell: HostShell::Bash,
            components: vec![ComponentCheck {
                kind: ComponentKind::FlutterSdk,
                status: ComponentStatus::Missing,
                detail: String::new(),
            }],
            doctor: None,
            linux_package_manager: Some(fdemon_daemon::toolchain::LinuxPackageManager::Unknown),
            winget_available: false,
        }
    }

    /// When preflight re-runs after a successful Flutter install (`resolved_sdk` is `Some`
    /// and the report shows Flutter live), the wizard must auto-close, transition to
    /// `UiMode::Startup` (not `Normal`), and dispatch `DiscoverDevices` exactly once.
    #[test]
    fn preflight_completed_with_live_flutter_autocloses_and_discovers() {
        use crate::state::UiMode;

        let mut state = AppState::new();
        state.show_install_wizard();
        inject_live_sdk(&mut state);
        assert_eq!(state.ui_mode, UiMode::InstallWizard, "precondition");
        assert!(!state.install_wizard_state.handback_done, "precondition");

        let result = handle_preflight_completed(&mut state, make_live_flutter_report());

        // AC#1: mode must be Startup (not merely != InstallWizard, not Normal).
        assert_eq!(
            state.ui_mode,
            UiMode::Startup,
            "wizard auto-close must leave UiMode::Startup so DevicesDiscovered \
             populates the new-session dialog selector"
        );
        // handback_done must be set.
        assert!(
            state.install_wizard_state.handback_done,
            "handback_done must be true after auto-close"
        );
        // DiscoverDevices must be among the returned actions.
        let actions = result.actions();
        assert!(
            actions
                .iter()
                .any(|a| matches!(a, UpdateAction::DiscoverDevices { .. })),
            "DiscoverDevices action must be returned; got {:?}",
            actions
        );
    }

    /// AC#2: after the auto-close, feeding `Message::DevicesDiscovered` through
    /// `handler::update` must populate `target_selector.connected_devices`.
    #[test]
    fn devices_discovered_after_autoclose_populates_selector() {
        use crate::handler::update;
        use crate::message::Message;
        use crate::state::UiMode;
        use fdemon_daemon::Device;

        let mut state = AppState::new();
        state.show_install_wizard();
        inject_live_sdk(&mut state);

        // Trigger the auto-close handback.
        handle_preflight_completed(&mut state, make_live_flutter_report());

        // Postcondition from AC#1: mode must be Startup.
        assert_eq!(state.ui_mode, UiMode::Startup, "precondition for AC#2");

        // Simulate device discovery completing with one device.
        let fake_device = Device {
            id: "emulator-5554".to_string(),
            name: "Pixel 6 Emulator".to_string(),
            platform: "android-x86".to_string(),
            emulator: true,
            category: None,
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
            is_supported: true,
            capabilities: None,
        };
        update(
            &mut state,
            Message::DevicesDiscovered {
                devices: vec![fake_device],
            },
        );

        // AC#2: the selector must now hold the discovered device.
        assert!(
            !state
                .new_session_dialog_state
                .target_selector
                .connected_devices
                .is_empty(),
            "target_selector.connected_devices must be non-empty after DevicesDiscovered \
             in Startup mode; the dialog would otherwise show no devices"
        );
    }

    /// When the handback guard is already set (first preflight already fired),
    /// a second `handle_preflight_completed` call must NOT dispatch a second
    /// `DiscoverDevices` action.
    #[test]
    fn handback_does_not_fire_twice() {
        let mut state = AppState::new();
        state.show_install_wizard();
        inject_live_sdk(&mut state);
        // Pre-set the guard as if auto-close already fired once.
        state.install_wizard_state.handback_done = true;

        let result = handle_preflight_completed(&mut state, make_live_flutter_report());

        let actions = result.actions();
        assert!(
            !actions.iter().any(|a| matches!(a, UpdateAction::DiscoverDevices { .. })),
            "second preflight must not dispatch DiscoverDevices when handback_done is true; got {:?}",
            actions
        );
    }

    /// When the preflight report does NOT show Flutter live (still missing),
    /// `handle_preflight_completed` must NOT auto-close the wizard.
    #[test]
    fn preflight_completed_without_live_flutter_does_not_handback() {
        use crate::state::UiMode;

        let mut state = AppState::new();
        state.show_install_wizard();
        inject_live_sdk(&mut state); // resolved_sdk is Some, but report says Missing

        let result = handle_preflight_completed(&mut state, make_dead_flutter_report());

        // Wizard must remain open (install still in progress / failed).
        assert_eq!(
            state.ui_mode,
            UiMode::InstallWizard,
            "wizard must remain open when Flutter SDK is still missing in report"
        );
        assert!(
            !state.install_wizard_state.handback_done,
            "handback_done must NOT be set when report shows Flutter missing"
        );
        // No DiscoverDevices.
        let actions = result.actions();
        assert!(
            !actions
                .iter()
                .any(|a| matches!(a, UpdateAction::DiscoverDevices { .. })),
            "must not dispatch DiscoverDevices when Flutter is still missing; got {:?}",
            actions
        );
    }

    // ── Phase 5 Task 02: run_seq + synchronous token tests ───────────────────

    /// After `handle_run_selected_step`, `install_task` must be `Some` with
    /// a token that can be cancelled (F3: token stored before RunWizardStep
    /// dispatches, so Esc works in the window before WizardInstallTaskReady).
    #[test]
    fn run_selected_step_stores_token_synchronously() {
        let mut state = state_with_preflight();
        state.install_wizard_state.selected_index = 3; // FlutterSdk

        let result = handle_run_selected_step(&mut state);

        assert!(result.action.is_some(), "must return an action");
        // install_task must already be Some (synchronous store).
        let task = state
            .install_wizard_state
            .install_task
            .as_ref()
            .expect("install_task must be Some immediately after run_selected_step (F3)");
        // Token must not yet be cancelled.
        assert!(
            !task.cancel.is_cancelled(),
            "token must not be pre-cancelled"
        );
        // join is None until WizardInstallTaskReady upgrades it.
        assert!(
            task.join.is_none(),
            "join must be None before WizardInstallTaskReady upgrades it"
        );
    }

    /// Cancelling in the "running but no join yet" window must fire the
    /// synchronously-stored token and reset to Idle (F3).
    #[test]
    fn cancel_during_early_window_fires_token_and_resets_to_idle() {
        let mut state = state_with_preflight();
        state.install_wizard_state.selected_index = 3; // FlutterSdk
        handle_run_selected_step(&mut state);
        assert!(state.install_wizard_state.is_step_running(), "precondition");
        assert!(
            state
                .install_wizard_state
                .install_task
                .as_ref()
                .map(|t| t.join.is_none())
                .unwrap_or(false),
            "join must still be None in the early window"
        );

        // Cancel in the early window (before WizardInstallTaskReady).
        let token = state
            .install_wizard_state
            .install_task
            .as_ref()
            .map(|t| t.cancel.clone())
            .expect("token must exist");

        handle_cancel_step(&mut state);

        // Token must have been fired.
        assert!(
            token.is_cancelled(),
            "cancel token must be set after handle_cancel_step (F3)"
        );
        // Step must be reset to Idle.
        assert!(
            !state.install_wizard_state.is_step_running(),
            "step must be Idle after cancel (F3)"
        );
    }

    /// `handle_install_task_ready` with a matching kind and run_seq must
    /// upgrade the join handle (happy path).
    #[tokio::test]
    async fn install_task_ready_matching_seq_upgrades_join() {
        let mut state = state_with_preflight();
        state.install_wizard_state.selected_index = 3; // FlutterSdk
        handle_run_selected_step(&mut state);

        let current_seq = state.install_wizard_state.run_seq;
        let join_handle = tokio::spawn(std::future::ready(()));
        let handle_slot: std::sync::Arc<std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>> =
            std::sync::Arc::new(std::sync::Mutex::new(Some(join_handle)));

        handle_install_task_ready(
            &mut state,
            WizardStepKind::FlutterSdk,
            current_seq,
            handle_slot,
        );

        // join must have been upgraded.
        let task = state
            .install_wizard_state
            .install_task
            .as_ref()
            .expect("install_task must still be Some");
        assert!(
            task.join.is_some(),
            "join must be Some after a matching WizardInstallTaskReady"
        );
    }

    /// `handle_install_task_ready` with a non-matching kind must discard the
    /// join handle without touching `install_task` (F4).
    #[tokio::test]
    async fn install_task_ready_kind_mismatch_is_discarded() {
        let mut state = state_with_preflight();
        state.install_wizard_state.selected_index = 3; // FlutterSdk
        handle_run_selected_step(&mut state);

        let current_seq = state.install_wizard_state.run_seq;
        // Send a ready for AndroidTools — kind mismatch.
        let handle_slot: std::sync::Arc<std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>> =
            std::sync::Arc::new(std::sync::Mutex::new(Some(tokio::spawn(
                std::future::ready(()),
            ))));

        handle_install_task_ready(
            &mut state,
            WizardStepKind::AndroidTools, // wrong kind
            current_seq,
            handle_slot,
        );

        // install_task must still be present (join still None — not upgraded).
        let task = state
            .install_wizard_state
            .install_task
            .as_ref()
            .expect("install_task must survive a kind-mismatch discard (F4)");
        assert!(
            task.join.is_none(),
            "join must remain None after kind-mismatch discard"
        );
    }

    /// `handle_install_task_ready` with a non-matching run_seq must discard
    /// the join handle (cancel→retry same kind, F4).
    #[tokio::test]
    async fn install_task_ready_seq_mismatch_is_discarded() {
        let mut state = state_with_preflight();
        state.install_wizard_state.selected_index = 3; // FlutterSdk
        handle_run_selected_step(&mut state);

        let stale_seq = state.install_wizard_state.run_seq - 1; // seq before this run
        let handle_slot: std::sync::Arc<std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>> =
            std::sync::Arc::new(std::sync::Mutex::new(Some(tokio::spawn(
                std::future::ready(()),
            ))));

        handle_install_task_ready(
            &mut state,
            WizardStepKind::FlutterSdk, // kind matches but seq is stale
            stale_seq,
            handle_slot,
        );

        let task = state
            .install_wizard_state
            .install_task
            .as_ref()
            .expect("install_task must survive a seq-mismatch discard (F4)");
        assert!(
            task.join.is_none(),
            "join must remain None after seq-mismatch discard"
        );
    }

    /// A late `WizardInstallTaskReady` arriving AFTER a terminal `WizardStepFailed`
    /// must NOT re-install a handle — `install_task` stays None (F7).
    #[tokio::test]
    async fn install_task_ready_after_terminal_does_not_reinstall_handle() {
        let mut state = state_with_preflight();
        state.install_wizard_state.selected_index = 3; // FlutterSdk
        let result = handle_run_selected_step(&mut state);
        let run_seq = state.install_wizard_state.run_seq;
        let _action = result.action;

        // Simulate terminal: WizardStepFailed clears install_task.
        handle_step_failed(&mut state, "network timeout".to_string());
        assert!(
            state.install_wizard_state.install_task.is_none(),
            "precondition: install_task cleared by terminal"
        );
        assert!(
            !state.install_wizard_state.is_step_running(),
            "precondition: step no longer running"
        );

        // Late ready arrives after the terminal.
        let handle_slot: std::sync::Arc<std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>> =
            std::sync::Arc::new(std::sync::Mutex::new(Some(tokio::spawn(
                std::future::ready(()),
            ))));
        handle_install_task_ready(&mut state, WizardStepKind::FlutterSdk, run_seq, handle_slot);

        assert!(
            state.install_wizard_state.install_task.is_none(),
            "install_task must stay None — late ready after terminal must be discarded (F7)"
        );
    }

    /// cancel(A) → begin_step(K) again (run B) → late ready for A must be
    /// discarded; the live install_task is B's. Cancelling afterwards fires
    /// B's token (F4 full scenario).
    #[tokio::test]
    async fn cancel_retry_same_kind_late_ready_for_a_is_discarded() {
        let mut state = state_with_preflight();
        state.install_wizard_state.selected_index = 3; // FlutterSdk

        // Start run A.
        handle_run_selected_step(&mut state);
        let seq_a = state.install_wizard_state.run_seq;
        let token_a = state
            .install_wizard_state
            .install_task
            .as_ref()
            .unwrap()
            .cancel
            .clone();

        // Cancel run A.
        handle_cancel_step(&mut state);
        assert!(token_a.is_cancelled(), "A's token must be cancelled");

        // Start run B (same step kind).
        handle_run_selected_step(&mut state);
        let seq_b = state.install_wizard_state.run_seq;
        assert_ne!(seq_a, seq_b, "seq must have been bumped for run B");
        let token_b = state
            .install_wizard_state
            .install_task
            .as_ref()
            .unwrap()
            .cancel
            .clone();

        // Late ready for A arrives (kind matches, seq is stale).
        // It must be discarded — B's install_task must survive untouched.
        let handle_slot: std::sync::Arc<std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>> =
            std::sync::Arc::new(std::sync::Mutex::new(Some(tokio::spawn(
                std::future::ready(()),
            ))));
        handle_install_task_ready(
            &mut state,
            WizardStepKind::FlutterSdk,
            seq_a, // stale seq from run A
            handle_slot,
        );

        // B's install_task must still be present.
        assert!(
            state.install_wizard_state.install_task.is_some(),
            "install_task must be B's (not cleared by A's stale ready)"
        );
        // B's token must not be cancelled (A's cancel did not affect B).
        assert!(
            !token_b.is_cancelled(),
            "B's token must not be cancelled by A's stale ready"
        );
    }

    /// Partial toolchain (Flutter live, Android missing) must still hand back —
    /// handback is gated on Flutter only.
    #[test]
    fn partial_toolchain_still_handbacks_when_flutter_live() {
        use crate::state::UiMode;

        // Report: Flutter Ok, Android missing.
        let report = ToolchainReport {
            platform: HostPlatform::Linux,
            shell: HostShell::Bash,
            components: vec![
                ComponentCheck {
                    kind: ComponentKind::FlutterSdk,
                    status: ComponentStatus::Ok,
                    detail: String::new(),
                },
                ComponentCheck {
                    kind: ComponentKind::AndroidCmdlineTools,
                    status: ComponentStatus::Missing,
                    detail: String::new(),
                },
            ],
            doctor: None,
            linux_package_manager: Some(fdemon_daemon::toolchain::LinuxPackageManager::Unknown),
            winget_available: false,
        };

        let mut state = AppState::new();
        state.show_install_wizard();
        inject_live_sdk(&mut state);

        let result = handle_preflight_completed(&mut state, report);

        // Wizard must auto-close to Startup even though Android is missing.
        assert_eq!(
            state.ui_mode,
            UiMode::Startup,
            "wizard must handback to Startup even when Android tools are still missing"
        );
        let actions = result.actions();
        assert!(
            actions
                .iter()
                .any(|a| matches!(a, UpdateAction::DiscoverDevices { .. })),
            "DiscoverDevices must be dispatched even with partial toolchain; got {:?}",
            actions
        );
    }

    // ── Task 03 (F18): Cancelled variant routing + no double-prefix ─────────

    /// F18: `Error::Cancelled` Display already carries the "Cancelled: " prefix,
    /// so `format!("{e}")` must produce exactly one prefix — not two.
    #[test]
    fn cancelled_error_display_has_no_double_prefix() {
        let e = fdemon_core::Error::cancelled("Flutter install cancelled by user");
        let s = format!("{e}");
        // Must start with exactly one "Cancelled:" prefix.
        assert!(
            s.starts_with("Cancelled:"),
            "Display must start with 'Cancelled:'; got: {s}"
        );
        // Must NOT start with "Cancelled: Cancelled:" (double prefix).
        assert!(
            !s.starts_with("Cancelled: Cancelled:"),
            "Display must NOT have a double 'Cancelled:' prefix; got: {s}"
        );
        assert!(
            s.contains("Flutter install cancelled by user"),
            "Display must include the original message; got: {s}"
        );
    }

    /// F18: `handle_step_failed` with a `"Cancelled: …"` reason must leave the
    /// step in `StepExecStatus::Cancelled` (not `Failed`).
    #[test]
    fn step_failed_with_cancelled_reason_stores_cancelled_status() {
        let mut state = state_with_preflight();
        state
            .install_wizard_state
            .begin_step(WizardStepKind::FlutterSdk);

        handle_step_failed(
            &mut state,
            "Cancelled: Flutter install cancelled before start".to_string(),
        );

        assert_eq!(
            state.install_wizard_state.execution.status,
            crate::install_wizard::StepExecStatus::Cancelled,
            "a 'Cancelled:' reason must result in StepExecStatus::Cancelled, not Failed"
        );
    }

    /// F18: `handle_step_failed` with a genuine (non-Cancelled) reason must
    /// leave the step in `StepExecStatus::Failed`.
    #[test]
    fn step_failed_with_genuine_reason_stores_failed_status() {
        let mut state = state_with_preflight();
        state
            .install_wizard_state
            .begin_step(WizardStepKind::FlutterSdk);

        handle_step_failed(&mut state, "network timeout".to_string());

        assert_eq!(
            state.install_wizard_state.execution.status,
            crate::install_wizard::StepExecStatus::Failed,
            "a genuine (non-Cancelled) reason must result in StepExecStatus::Failed"
        );
    }

    /// F18: After a Cancelled terminal, `status_message` must be neutral —
    /// must NOT contain "Failed".
    #[test]
    fn step_failed_with_cancelled_reason_neutral_status_message() {
        let mut state = state_with_preflight();
        state
            .install_wizard_state
            .begin_step(WizardStepKind::AndroidTools);

        handle_step_failed(
            &mut state,
            "Cancelled: download cancelled by user".to_string(),
        );

        let msg = state
            .install_wizard_state
            .status_message
            .as_deref()
            .unwrap_or("");
        assert!(
            !msg.starts_with("Failed"),
            "status_message after cancel must not start with 'Failed'; got: {msg}"
        );
        assert!(
            msg.contains("Cancelled") || msg.contains("retry"),
            "status_message must mention 'Cancelled' or 'retry'; got: {msg}"
        );
    }
}
