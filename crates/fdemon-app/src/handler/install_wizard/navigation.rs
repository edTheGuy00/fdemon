//! # Install Wizard Panel Navigation Handlers
//!
//! Handles panel lifecycle (open, close, escape) and step/detail navigation
//! for the Install Wizard panel.

use crate::handler::{UpdateAction, UpdateResult};
use crate::install_wizard::{WizardOrigin, WizardPane};
use crate::state::AppState;

/// Handle `ShowInstallWizard` — opens the Install Wizard panel.
///
/// Resets the wizard to a fresh loading state, transitions to
/// `UiMode::InstallWizard`, and triggers a toolchain preflight task.
/// The wizard shows `loading = true` until `ToolchainPreflightCompleted` arrives.
///
/// The `origin` is stored on `InstallWizardState` and gates the post-install
/// handback: only `Bootstrap` auto-advances to device discovery on close.
/// A `UserInvoked` open (the `I` key) is informational only — `Esc` returns
/// to `UiMode::Normal` without dispatching `DiscoverDevices`.
pub fn handle_show(state: &mut AppState, origin: WizardOrigin) -> UpdateResult {
    state.show_install_wizard(origin);

    let project_path = state.project_path.clone();
    let explicit_sdk_path = state.settings.flutter.sdk_path.clone();
    let android_sdk_root = state.settings.toolchain.android_sdk_root.clone();

    UpdateResult::action(UpdateAction::RunToolchainPreflight {
        project_path,
        explicit_sdk_path,
        android_sdk_root,
    })
}

/// Handle `HideInstallWizard` — closes the Install Wizard panel.
///
/// **Handback (Phase 5, Task 04).** When a live Flutter SDK exists at close
/// time and the handback guard has not already fired, also dispatches
/// `DiscoverDevices` and transitions to `UiMode::Startup` so the new-session
/// dialog is shown once devices arrive.  Delegates to
/// `close_wizard_and_dispatch_discovery` (single source of truth) which also
/// handles the wizard hide.
pub fn handle_hide(state: &mut AppState) -> UpdateResult {
    maybe_dispatch_discovery_on_close(state)
}

/// Handle `InstallWizardEscape` — priority-ordered escape from the panel.
///
/// When a step is running, `Esc` is handled by a higher-priority key arm
/// (dispatching `InstallWizardCancelStep`); by the time this function is
/// reached, no step is in flight.
///
/// **Handback (Phase 5, Task 04).** Same as `handle_hide`: when a live SDK
/// exists and the guard is unset, dispatch device discovery and route to
/// `UiMode::Startup`.  Delegates to `close_wizard_and_dispatch_discovery`
/// (single source of truth) which also handles the wizard hide.
pub fn handle_escape(state: &mut AppState) -> UpdateResult {
    maybe_dispatch_discovery_on_close(state)
}

/// Shared handback helper for `handle_hide` and `handle_escape`.
///
/// Delegates to `close_wizard_and_dispatch_discovery` (defined in `actions.rs`)
/// which is the single source of truth for the post-install handback transition.
/// Both auto-close (`handle_preflight_completed`) and manual-close paths route
/// through that function so they cannot drift.
///
/// The wizard is always closed (hidden) by this call.  When the one-shot guard
/// (`handback_done`) is already set — i.e. auto-close already dispatched
/// discovery — the wizard is hidden and `UpdateResult::none()` is returned so
/// a second `DiscoverDevices` is not emitted.
fn maybe_dispatch_discovery_on_close(state: &mut AppState) -> UpdateResult {
    if state.install_wizard_state.handback_done {
        // Guard already fired: hide the wizard (Normal mode) and return.
        state.hide_install_wizard();
        return UpdateResult::none();
    }
    match super::close_wizard_and_dispatch_discovery(state) {
        Some(action) => UpdateResult::action(action),
        None => UpdateResult::none(),
    }
}

/// Handle `InstallWizardSwitchPane` — toggle focus between step list and detail pane.
pub fn handle_switch_pane(state: &mut AppState) -> UpdateResult {
    let wiz = &mut state.install_wizard_state;
    wiz.focused_pane = match wiz.focused_pane {
        WizardPane::StepList => WizardPane::Detail,
        WizardPane::Detail => WizardPane::StepList,
    };
    UpdateResult::none()
}

/// Handle `InstallWizardUp` — navigate up in the step list or scroll detail up.
///
/// When `StepList` pane is focused: moves the selected step up (clamp at 0)
/// and resets `detail_scroll` to 0 so the detail view shows the top of the
/// newly selected step.
///
/// When `Detail` pane is focused: scrolls `detail_scroll` up by one line
/// (saturating at 0).
pub fn handle_up(state: &mut AppState) -> UpdateResult {
    let wiz = &mut state.install_wizard_state;
    match wiz.focused_pane {
        WizardPane::StepList => {
            if wiz.selected_index > 0 {
                wiz.selected_index -= 1;
                wiz.detail_scroll = 0;
                wiz.selected_command_index = 0;
            }
        }
        WizardPane::Detail => {
            wiz.detail_scroll = wiz.detail_scroll.saturating_sub(1);
        }
    }
    UpdateResult::none()
}

/// Handle `InstallWizardDown` — navigate down in the step list or scroll detail down.
///
/// When `StepList` pane is focused: moves the selected step down (clamp at
/// `steps.len() - 1`) and resets `detail_scroll` to 0 so the detail view
/// shows the top of the newly selected step.
///
/// When `Detail` pane is focused: scrolls `detail_scroll` down by one line.
/// The upper-bound clamp is applied at render time by `compute_corrected_scroll`
/// in `step_detail.rs`, which has access to the actual content length.
pub fn handle_down(state: &mut AppState) -> UpdateResult {
    let wiz = &mut state.install_wizard_state;
    match wiz.focused_pane {
        WizardPane::StepList => {
            let max = wiz.steps.len().saturating_sub(1);
            if wiz.selected_index < max {
                wiz.selected_index += 1;
                wiz.detail_scroll = 0;
                wiz.selected_command_index = 0;
            }
        }
        WizardPane::Detail => {
            // Advance by 1 unconditionally; the upper-bound clamp is applied at render
            // time via `compute_corrected_scroll` in `step_detail.rs`, which has access
            // to the actual content length.
            wiz.detail_scroll = wiz.detail_scroll.saturating_add(1);
        }
    }
    UpdateResult::none()
}

/// Handle `InstallWizardPrevCommand` (`[` key) — select the previous guided command.
///
/// Calls [`InstallWizardState::select_prev_command`], which is a no-op when
/// the currently selected step has 0 or 1 guided commands.
/// Works regardless of which pane is focused.
pub fn handle_prev_command(state: &mut AppState) -> UpdateResult {
    state.install_wizard_state.select_prev_command();
    UpdateResult::none()
}

/// Handle `InstallWizardNextCommand` (`]` key) — select the next guided command.
///
/// Calls [`InstallWizardState::select_next_command`], which is a no-op when
/// the currently selected step has 0 or 1 guided commands.
/// Works regardless of which pane is focused.
pub fn handle_next_command(state: &mut AppState) -> UpdateResult {
    state.install_wizard_state.select_next_command();
    UpdateResult::none()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{AppState, UiMode};
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

    fn state_with_wizard_open() -> AppState {
        let mut state = AppState::new();
        state.show_install_wizard(WizardOrigin::UserInvoked);
        state.install_wizard_state.apply_report(make_report());
        state
    }

    #[test]
    fn test_show_install_wizard_sets_mode_and_loading() {
        let mut state = AppState::new();
        let result = handle_show(&mut state, WizardOrigin::UserInvoked);
        assert_eq!(state.ui_mode, UiMode::InstallWizard);
        assert!(state.install_wizard_state.visible);
        assert!(state.install_wizard_state.loading);
        // Should return RunToolchainPreflight action
        assert!(matches!(
            result.action,
            Some(UpdateAction::RunToolchainPreflight { .. })
        ));
    }

    #[test]
    fn test_show_passes_project_path_to_action() {
        let mut state = AppState::new();
        let result = handle_show(&mut state, WizardOrigin::UserInvoked);
        if let Some(UpdateAction::RunToolchainPreflight { project_path, .. }) = result.action {
            // project_path comes from state.project_path (empty PathBuf in test state)
            assert_eq!(project_path, state.project_path);
        } else {
            panic!("expected RunToolchainPreflight action");
        }
    }

    #[test]
    fn test_hide_returns_to_normal() {
        let mut state = state_with_wizard_open();
        handle_hide(&mut state);
        assert_eq!(state.ui_mode, UiMode::Normal);
        assert!(!state.install_wizard_state.visible);
    }

    #[test]
    fn test_escape_returns_to_normal() {
        let mut state = state_with_wizard_open();
        handle_escape(&mut state);
        assert_eq!(state.ui_mode, UiMode::Normal);
        assert!(!state.install_wizard_state.visible);
    }

    #[test]
    fn test_switch_pane_toggles_focus() {
        let mut state = state_with_wizard_open();
        assert_eq!(
            state.install_wizard_state.focused_pane,
            WizardPane::StepList
        );
        handle_switch_pane(&mut state);
        assert_eq!(state.install_wizard_state.focused_pane, WizardPane::Detail);
        handle_switch_pane(&mut state);
        assert_eq!(
            state.install_wizard_state.focused_pane,
            WizardPane::StepList
        );
    }

    #[test]
    fn test_step_nav_clamps_selected_index() {
        let mut state = state_with_wizard_open();
        state.install_wizard_state.focused_pane = WizardPane::StepList;
        // Steps has 5 items (build_steps always returns 5)
        assert_eq!(state.install_wizard_state.steps.len(), 5);

        // Move to last item
        state.install_wizard_state.selected_index = 4;
        handle_down(&mut state);
        assert_eq!(
            state.install_wizard_state.selected_index, 4,
            "must not exceed last index"
        );

        // Move up from 0 — should stay at 0
        state.install_wizard_state.selected_index = 0;
        handle_up(&mut state);
        assert_eq!(
            state.install_wizard_state.selected_index, 0,
            "must not go below 0"
        );
    }

    #[test]
    fn test_step_nav_down_increments_index() {
        let mut state = state_with_wizard_open();
        state.install_wizard_state.focused_pane = WizardPane::StepList;
        state.install_wizard_state.selected_index = 0;
        handle_down(&mut state);
        assert_eq!(state.install_wizard_state.selected_index, 1);
    }

    #[test]
    fn test_step_nav_up_decrements_index() {
        let mut state = state_with_wizard_open();
        state.install_wizard_state.focused_pane = WizardPane::StepList;
        state.install_wizard_state.selected_index = 2;
        handle_up(&mut state);
        assert_eq!(state.install_wizard_state.selected_index, 1);
    }

    #[test]
    fn test_step_nav_resets_detail_scroll() {
        let mut state = state_with_wizard_open();
        state.install_wizard_state.focused_pane = WizardPane::StepList;
        state.install_wizard_state.detail_scroll = 5;
        state.install_wizard_state.selected_index = 1;
        handle_up(&mut state);
        assert_eq!(
            state.install_wizard_state.detail_scroll, 0,
            "detail_scroll must reset on step change"
        );
    }

    #[test]
    fn test_detail_scroll_up_decrements() {
        let mut state = state_with_wizard_open();
        state.install_wizard_state.focused_pane = WizardPane::Detail;
        state.install_wizard_state.detail_scroll = 3;
        handle_up(&mut state);
        assert_eq!(state.install_wizard_state.detail_scroll, 2);
    }

    #[test]
    fn test_detail_scroll_up_saturates_at_zero() {
        let mut state = state_with_wizard_open();
        state.install_wizard_state.focused_pane = WizardPane::Detail;
        state.install_wizard_state.detail_scroll = 0;
        handle_up(&mut state);
        assert_eq!(state.install_wizard_state.detail_scroll, 0);
    }

    #[test]
    fn test_detail_scroll_down_increments() {
        let mut state = state_with_wizard_open();
        state.install_wizard_state.focused_pane = WizardPane::Detail;
        state.install_wizard_state.detail_scroll = 0;
        handle_down(&mut state);
        assert_eq!(state.install_wizard_state.detail_scroll, 1);
    }

    // --- Step change resets selected_command_index ---

    #[test]
    fn test_step_nav_up_resets_selected_command_index() {
        let mut state = state_with_wizard_open();
        state.install_wizard_state.focused_pane = WizardPane::StepList;
        state.install_wizard_state.selected_index = 2;
        state.install_wizard_state.selected_command_index = 1;
        handle_up(&mut state);
        assert_eq!(
            state.install_wizard_state.selected_command_index, 0,
            "step up must reset selected_command_index"
        );
    }

    #[test]
    fn test_step_nav_down_resets_selected_command_index() {
        let mut state = state_with_wizard_open();
        state.install_wizard_state.focused_pane = WizardPane::StepList;
        state.install_wizard_state.selected_index = 0;
        state.install_wizard_state.selected_command_index = 1;
        handle_down(&mut state);
        assert_eq!(
            state.install_wizard_state.selected_command_index, 0,
            "step down must reset selected_command_index"
        );
    }

    #[test]
    fn test_detail_scroll_up_does_not_reset_command_index() {
        let mut state = state_with_wizard_open();
        state.install_wizard_state.focused_pane = WizardPane::Detail;
        state.install_wizard_state.detail_scroll = 5;
        state.install_wizard_state.selected_command_index = 1;
        handle_up(&mut state);
        assert_eq!(
            state.install_wizard_state.selected_command_index, 1,
            "detail scroll must not touch selected_command_index"
        );
    }

    // --- handle_prev_command / handle_next_command ---

    fn state_with_multi_command_prereqs() -> AppState {
        use fdemon_daemon::toolchain::{
            PREREQ_KEY_COCOAPODS, PREREQ_KEY_ROSETTA, PREREQ_KEY_XCODE_CLT,
        };
        let detail = format!(
            "missing: {}, {}, {}",
            PREREQ_KEY_XCODE_CLT, PREREQ_KEY_COCOAPODS, PREREQ_KEY_ROSETTA
        );
        let report = ToolchainReport {
            platform: HostPlatform::MacOs,
            shell: HostShell::Bash,
            components: vec![ComponentCheck {
                kind: ComponentKind::Prerequisites,
                status: ComponentStatus::Missing,
                detail,
            }],
            doctor: None,
            linux_package_manager: None,
            winget_available: false,
        };
        let mut state = AppState::new();
        state.show_install_wizard(WizardOrigin::UserInvoked);
        state.install_wizard_state.apply_report(report);
        // Select Prerequisites (index 0) which has 3 commands.
        state.install_wizard_state.selected_index = 0;
        state
    }

    #[test]
    fn test_handle_next_command_advances_index() {
        let mut state = state_with_multi_command_prereqs();
        assert_eq!(state.install_wizard_state.selected_command_index, 0);
        handle_next_command(&mut state);
        assert_eq!(state.install_wizard_state.selected_command_index, 1);
        handle_next_command(&mut state);
        assert_eq!(state.install_wizard_state.selected_command_index, 2);
    }

    #[test]
    fn test_handle_next_command_clamps_at_last() {
        let mut state = state_with_multi_command_prereqs();
        state.install_wizard_state.selected_command_index = 2;
        handle_next_command(&mut state);
        assert_eq!(
            state.install_wizard_state.selected_command_index, 2,
            "must clamp at last index"
        );
    }

    #[test]
    fn test_handle_prev_command_retreats_index() {
        let mut state = state_with_multi_command_prereqs();
        state.install_wizard_state.selected_command_index = 2;
        handle_prev_command(&mut state);
        assert_eq!(state.install_wizard_state.selected_command_index, 1);
        handle_prev_command(&mut state);
        assert_eq!(state.install_wizard_state.selected_command_index, 0);
    }

    #[test]
    fn test_handle_prev_command_saturates_at_zero() {
        let mut state = state_with_multi_command_prereqs();
        state.install_wizard_state.selected_command_index = 0;
        handle_prev_command(&mut state);
        assert_eq!(
            state.install_wizard_state.selected_command_index, 0,
            "must saturate at 0"
        );
    }

    #[test]
    fn test_handle_next_command_noop_for_single_command_step() {
        let mut state = state_with_wizard_open();
        // Single-component Linux report gives AndroidTools 1 guided command (JDK missing).
        let report = ToolchainReport {
            platform: HostPlatform::Linux,
            shell: HostShell::Bash,
            components: vec![ComponentCheck {
                kind: ComponentKind::Jdk,
                status: ComponentStatus::Missing,
                detail: String::new(),
            }],
            doctor: None,
            linux_package_manager: Some(fdemon_daemon::toolchain::LinuxPackageManager::Unknown),
            winget_available: false,
        };
        state.install_wizard_state.apply_report(report);
        state.install_wizard_state.selected_index = 1; // AndroidTools
        state.install_wizard_state.selected_command_index = 0;
        handle_next_command(&mut state);
        assert_eq!(
            state.install_wizard_state.selected_command_index, 0,
            "next must be no-op for single-command step"
        );
    }

    #[test]
    fn test_handlers_work_regardless_of_focused_pane() {
        // Verify that prev/next work even when Detail pane is focused.
        let mut state = state_with_multi_command_prereqs();
        state.install_wizard_state.focused_pane = WizardPane::Detail;
        handle_next_command(&mut state);
        assert_eq!(state.install_wizard_state.selected_command_index, 1);
        handle_prev_command(&mut state);
        assert_eq!(state.install_wizard_state.selected_command_index, 0);
    }

    // ── Phase 5, Task 04: handback tests ─────────────────────────────────────

    /// Helper: build an `AppState` with the wizard open and `resolved_sdk` set
    /// to a minimal `FlutterSdk` so that `flutter_executable()` returns `Some`.
    fn state_with_live_sdk() -> AppState {
        use fdemon_daemon::{FlutterExecutable, FlutterSdk, SdkSource};

        let mut state = AppState::new();
        state.show_install_wizard(WizardOrigin::Bootstrap);
        state.install_wizard_state.apply_report(make_report());
        // Inject a minimal resolved_sdk so flutter_executable() returns Some.
        state.resolved_sdk = Some(FlutterSdk {
            root: std::path::PathBuf::from("/opt/flutter"),
            executable: FlutterExecutable::Direct(std::path::PathBuf::from(
                "/opt/flutter/bin/flutter",
            )),
            source: SdkSource::ExplicitConfig,
            version: "3.27.0".to_string(),
            channel: Some("stable".to_string()),
        });
        state
    }

    #[test]
    fn manual_close_with_live_sdk_spawns_discovery() {
        // When Esc is pressed after a successful install (live SDK), handle_escape
        // must dispatch DiscoverDevices and transition to UiMode::Startup (not Normal)
        // so the subsequent DevicesDiscovered message populates the selector.
        let mut state = state_with_live_sdk();
        assert!(
            state.flutter_executable().is_some(),
            "precondition: SDK must be live"
        );

        let result = handle_escape(&mut state);

        // Must transition to Startup (not merely != InstallWizard).
        assert_eq!(
            state.ui_mode,
            UiMode::Startup,
            "Esc with live SDK must leave UiMode::Startup so DevicesDiscovered \
             populates the new-session dialog selector"
        );
        // Must dispatch DiscoverDevices.
        let actions = result.actions();
        assert!(
            actions
                .iter()
                .any(|a| matches!(a, crate::handler::UpdateAction::DiscoverDevices { .. })),
            "handle_escape with live SDK must dispatch DiscoverDevices; got {:?}",
            actions
        );
        // handback_done must be set.
        assert!(
            state.install_wizard_state.handback_done,
            "handback_done must be true after manual close with live SDK"
        );
    }

    #[test]
    fn manual_close_without_live_sdk_returns_none() {
        // When Esc is pressed with no live SDK, handle_escape must be a no-op
        // (no DiscoverDevices, bare Normal mode).
        let mut state = state_with_wizard_open();
        // No resolved_sdk — flutter_executable() is None.
        assert!(
            state.flutter_executable().is_none(),
            "precondition: SDK must be absent"
        );

        let result = handle_escape(&mut state);

        // No discovery action.
        let actions = result.actions();
        assert!(
            actions.is_empty(),
            "handle_escape without live SDK must return no actions; got {:?}",
            actions
        );
        assert_eq!(state.ui_mode, UiMode::Normal);
        assert!(!state.install_wizard_state.handback_done);
    }

    #[test]
    fn second_manual_close_does_not_spawn_discovery_again() {
        // After handback_done is set (e.g. auto-close already fired), a manual
        // Esc must be a no-op — no second DiscoverDevices.
        let mut state = state_with_live_sdk();
        state.install_wizard_state.handback_done = true; // guard already set

        let result = handle_escape(&mut state);

        let actions = result.actions();
        assert!(
            !actions
                .iter()
                .any(|a| matches!(a, crate::handler::UpdateAction::DiscoverDevices { .. })),
            "second Esc must not dispatch DiscoverDevices when handback_done is true; got {:?}",
            actions
        );
    }

    #[test]
    fn handle_hide_with_live_sdk_dispatches_discovery() {
        // handle_hide must behave identically to handle_escape for the handback.
        let mut state = state_with_live_sdk();

        let result = handle_hide(&mut state);

        let actions = result.actions();
        assert!(
            actions
                .iter()
                .any(|a| matches!(a, crate::handler::UpdateAction::DiscoverDevices { .. })),
            "handle_hide with live SDK must dispatch DiscoverDevices; got {:?}",
            actions
        );
        assert!(
            state.install_wizard_state.handback_done,
            "handback_done must be true after handle_hide with live SDK"
        );
    }

    // ── WizardOrigin gating tests ─────────────────────────────────────────────

    /// A `UserInvoked` Esc must return to `UiMode::Normal` without dispatching
    /// `DiscoverDevices` — even when a live SDK is present.
    ///
    /// Acceptance criterion 5: `close_wizard_and_dispatch_discovery` gates the
    /// handback on `is_bootstrap()`.  With `UserInvoked`, the function hides the
    /// wizard to `Normal` and returns `None`.
    #[test]
    fn user_invoked_escape_returns_to_normal() {
        let mut state = AppState::new();
        // Open with UserInvoked (the `I` key path).
        state.show_install_wizard(WizardOrigin::UserInvoked);
        state.install_wizard_state.apply_report(make_report());
        // Inject a live SDK so the only gate is the origin.
        state.resolved_sdk = Some(fdemon_daemon::FlutterSdk {
            root: std::path::PathBuf::from("/opt/flutter"),
            executable: fdemon_daemon::FlutterExecutable::Direct(std::path::PathBuf::from(
                "/opt/flutter/bin/flutter",
            )),
            source: fdemon_daemon::SdkSource::ExplicitConfig,
            version: "3.27.0".to_string(),
            channel: Some("stable".to_string()),
        });
        assert!(
            state.flutter_executable().is_some(),
            "precondition: SDK must be live"
        );
        assert!(!state.install_wizard_state.is_bootstrap());

        let result = handle_escape(&mut state);

        assert_eq!(
            state.ui_mode,
            UiMode::Normal,
            "UserInvoked Esc must return to Normal, not Startup"
        );
        assert!(
            !state.install_wizard_state.visible,
            "wizard must be hidden after UserInvoked Esc"
        );
        let actions = result.actions();
        assert!(
            !actions
                .iter()
                .any(|a| matches!(a, crate::handler::UpdateAction::DiscoverDevices { .. })),
            "UserInvoked Esc must not dispatch DiscoverDevices; got {:?}",
            actions
        );
    }
}
