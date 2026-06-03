//! # Install Wizard Panel Action Handlers
//!
//! Handles async result messages (preflight completed) and re-run
//! for the Install Wizard panel.

use crate::handler::{UpdateAction, UpdateResult};
use crate::state::AppState;
use fdemon_daemon::toolchain::ToolchainReport;

/// Handle `ToolchainPreflightCompleted` — populate the wizard with the report.
///
/// Calls `apply_report` to build the five UI steps from the report,
/// clears `loading`, and clears any status message.
pub fn handle_preflight_completed(state: &mut AppState, report: ToolchainReport) -> UpdateResult {
    state.install_wizard_state.apply_report(report);
    state.install_wizard_state.status_message = None;
    UpdateResult::none()
}

/// Handle `InstallWizardRerunPreflight` — re-run the preflight check.
///
/// Sets `loading = true` and dispatches `RunToolchainPreflight` so the
/// wizard shows a spinner until the updated report arrives.
pub fn handle_rerun_preflight(state: &mut AppState) -> UpdateResult {
    state.install_wizard_state.loading = true;
    state.install_wizard_state.status_message = None;

    let project_path = state.project_path.clone();
    let explicit_sdk_path = state.settings.flutter.sdk_path.clone();

    UpdateResult::action(UpdateAction::RunToolchainPreflight {
        project_path,
        explicit_sdk_path,
    })
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
    fn test_rerun_clears_status_message() {
        let mut state = AppState::new();
        state.show_install_wizard();
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
}
