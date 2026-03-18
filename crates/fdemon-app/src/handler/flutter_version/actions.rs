//! # Flutter Version Panel Action Handlers
//!
//! Handles SDK operations (version switch, removal) and async result messages
//! (scan completed/failed, switch completed/failed, remove completed/failed)
//! for the Flutter Version panel.

use crate::flutter_version::{FlutterVersionPane, InstalledSdk};
use crate::handler::{UpdateAction, UpdateResult};
use crate::state::AppState;

/// Handle `FlutterVersionScanCompleted` — populate the version list after a cache scan.
///
/// Replaces the installed versions list, clears the loading state, and resets
/// selection to the top.
pub fn handle_scan_completed(state: &mut AppState, versions: Vec<InstalledSdk>) -> UpdateResult {
    let fv = &mut state.flutter_version_state;
    fv.version_list.installed_versions = versions;
    fv.version_list.loading = false;
    fv.version_list.error = None;
    // Reset selection to top
    fv.version_list.selected_index = 0;
    fv.version_list.scroll_offset = 0;
    UpdateResult::none()
}

/// Handle `FlutterVersionScanFailed` — report a cache scan error.
pub fn handle_scan_failed(state: &mut AppState, reason: String) -> UpdateResult {
    let fv = &mut state.flutter_version_state;
    fv.version_list.loading = false;
    fv.version_list.error = Some(reason);
    UpdateResult::none()
}

/// Handle `FlutterVersionSwitch` — initiate a version switch for the selected entry.
///
/// Guards:
/// - Must be focused on the `VersionList` pane.
/// - Must have a selected version.
/// - Must not select the already-active version.
///
/// Returns `UpdateAction::SwitchFlutterVersion` on success; otherwise sets a
/// status message and returns no action.
pub fn handle_switch(state: &mut AppState) -> UpdateResult {
    let fv = &state.flutter_version_state;

    // Must be focused on VersionList pane
    if fv.focused_pane != FlutterVersionPane::VersionList {
        return UpdateResult::none();
    }

    // Get selected version
    let Some(selected) = fv
        .version_list
        .installed_versions
        .get(fv.version_list.selected_index)
    else {
        return UpdateResult::none();
    };

    // Don't switch to the already-active version
    if selected.is_active {
        state.flutter_version_state.status_message = Some("Already active".into());
        return UpdateResult::none();
    }

    let version = selected.version.clone();
    let sdk_path = selected.path.clone();
    let project_path = state.project_path.clone();
    let explicit_sdk_path = state.settings.flutter.sdk_path.clone();

    UpdateResult::action(UpdateAction::SwitchFlutterVersion {
        version,
        sdk_path,
        project_path,
        explicit_sdk_path,
    })
}

/// Handle `FlutterVersionSwitchCompleted` — update display state after a successful switch.
///
/// Updates the status message, refreshes the SDK info pane with the newly
/// resolved SDK, and triggers a re-scan to update `is_active` markers.
///
/// Note: The actual `state.resolved_sdk` update happens via `Message::SdkResolved`,
/// which the action dispatcher sends before `FlutterVersionSwitchCompleted`.
pub fn handle_switch_completed(state: &mut AppState, version: String) -> UpdateResult {
    state.flutter_version_state.status_message = Some(format!("Switched to {version}"));

    // Refresh the SDK info pane with the new resolved SDK
    state.flutter_version_state.sdk_info.resolved_sdk = state.resolved_sdk.clone();

    // Re-scan to update is_active markers
    let active_sdk_root = state.resolved_sdk.as_ref().map(|sdk| sdk.root.clone());
    UpdateResult::action(UpdateAction::ScanInstalledSdks { active_sdk_root })
}

/// Handle `FlutterVersionSwitchFailed` — report a version switch error.
pub fn handle_switch_failed(state: &mut AppState, reason: String) -> UpdateResult {
    state.flutter_version_state.status_message = Some(format!("Switch failed: {reason}"));
    UpdateResult::none()
}

/// Handle `FlutterVersionRemove` — initiate removal of the selected version.
///
/// Guards:
/// - Must be focused on the `VersionList` pane.
/// - Must not remove the currently active version.
///
/// Returns `UpdateAction::RemoveFlutterVersion` on success; otherwise sets a
/// status message and returns no action.
pub fn handle_remove(state: &mut AppState) -> UpdateResult {
    let fv = &state.flutter_version_state;

    if fv.focused_pane != FlutterVersionPane::VersionList {
        return UpdateResult::none();
    }

    let Some(selected) = fv
        .version_list
        .installed_versions
        .get(fv.version_list.selected_index)
    else {
        return UpdateResult::none();
    };

    // Don't allow removing the active version
    if selected.is_active {
        state.flutter_version_state.status_message =
            Some("Cannot remove the active version".into());
        return UpdateResult::none();
    }

    let version = selected.version.clone();
    let path = selected.path.clone();
    let active_sdk_root = state.resolved_sdk.as_ref().map(|sdk| sdk.root.clone());

    UpdateResult::action(UpdateAction::RemoveFlutterVersion {
        version,
        path,
        active_sdk_root,
    })
}

/// Handle `FlutterVersionRemoveCompleted` — update display state after a successful removal.
///
/// Sets a status message and triggers a re-scan of the FVM cache.
pub fn handle_remove_completed(state: &mut AppState, version: String) -> UpdateResult {
    state.flutter_version_state.status_message = Some(format!("Removed {version}"));

    // Re-scan cache
    let active_sdk_root = state.resolved_sdk.as_ref().map(|sdk| sdk.root.clone());
    UpdateResult::action(UpdateAction::ScanInstalledSdks { active_sdk_root })
}

/// Handle `FlutterVersionRemoveFailed` — report a version removal error.
pub fn handle_remove_failed(state: &mut AppState, reason: String) -> UpdateResult {
    state.flutter_version_state.status_message = Some(format!("Remove failed: {reason}"));
    UpdateResult::none()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flutter_version::InstalledSdk;
    use crate::state::AppState;
    use fdemon_daemon::test_utils::fake_flutter_sdk;
    use std::path::PathBuf;

    fn test_app_state() -> AppState {
        let mut state = AppState::new();
        state.resolved_sdk = Some(fake_flutter_sdk());
        state
    }

    fn panel_state_with_versions() -> AppState {
        let mut state = test_app_state();
        state.show_flutter_version();
        state.flutter_version_state.version_list.installed_versions = vec![
            InstalledSdk {
                version: "3.19.0".into(),
                channel: Some("stable".into()),
                path: PathBuf::from("/home/user/fvm/versions/3.19.0"),
                is_active: true,
            },
            InstalledSdk {
                version: "3.16.0".into(),
                channel: None,
                path: PathBuf::from("/home/user/fvm/versions/3.16.0"),
                is_active: false,
            },
            InstalledSdk {
                version: "3.22.0-beta".into(),
                channel: Some("beta".into()),
                path: PathBuf::from("/home/user/fvm/versions/3.22.0-beta"),
                is_active: false,
            },
        ];
        state.flutter_version_state.version_list.loading = false;
        state
    }

    #[test]
    fn test_scan_completed_populates_list() {
        let mut state = panel_state_with_versions();
        state.flutter_version_state.version_list.loading = true;
        let versions = vec![InstalledSdk {
            version: "3.19.0".into(),
            channel: None,
            path: PathBuf::from("/test"),
            is_active: false,
        }];
        handle_scan_completed(&mut state, versions);
        assert!(!state.flutter_version_state.version_list.loading);
        assert_eq!(
            state
                .flutter_version_state
                .version_list
                .installed_versions
                .len(),
            1
        );
    }

    #[test]
    fn test_scan_completed_clears_error() {
        let mut state = panel_state_with_versions();
        state.flutter_version_state.version_list.error = Some("previous error".into());
        handle_scan_completed(&mut state, vec![]);
        assert!(state.flutter_version_state.version_list.error.is_none());
    }

    #[test]
    fn test_scan_completed_resets_selection() {
        let mut state = panel_state_with_versions();
        state.flutter_version_state.version_list.selected_index = 2;
        state.flutter_version_state.version_list.scroll_offset = 1;
        handle_scan_completed(&mut state, vec![]);
        assert_eq!(state.flutter_version_state.version_list.selected_index, 0);
        assert_eq!(state.flutter_version_state.version_list.scroll_offset, 0);
    }

    #[test]
    fn test_scan_failed_sets_error() {
        let mut state = panel_state_with_versions();
        state.flutter_version_state.version_list.loading = true;
        handle_scan_failed(&mut state, "fvm not found".into());
        assert!(!state.flutter_version_state.version_list.loading);
        assert_eq!(
            state.flutter_version_state.version_list.error.as_deref(),
            Some("fvm not found")
        );
    }

    #[test]
    fn test_switch_active_version_shows_already_active() {
        let mut state = panel_state_with_versions();
        state.flutter_version_state.focused_pane = FlutterVersionPane::VersionList;
        state.flutter_version_state.version_list.selected_index = 0; // active version
        let result = handle_switch(&mut state);
        assert!(result.action.is_none());
        assert_eq!(
            state.flutter_version_state.status_message.as_deref(),
            Some("Already active")
        );
    }

    #[test]
    fn test_switch_non_active_returns_action() {
        let mut state = panel_state_with_versions();
        state.flutter_version_state.focused_pane = FlutterVersionPane::VersionList;
        state.flutter_version_state.version_list.selected_index = 1; // non-active
        let result = handle_switch(&mut state);
        assert!(matches!(
            result.action,
            Some(UpdateAction::SwitchFlutterVersion { .. })
        ));
    }

    #[test]
    fn test_switch_non_active_carries_correct_version() {
        let mut state = panel_state_with_versions();
        state.flutter_version_state.focused_pane = FlutterVersionPane::VersionList;
        state.flutter_version_state.version_list.selected_index = 1; // 3.16.0
        let result = handle_switch(&mut state);
        if let Some(UpdateAction::SwitchFlutterVersion { version, .. }) = result.action {
            assert_eq!(version, "3.16.0");
        } else {
            panic!("expected SwitchFlutterVersion action");
        }
    }

    #[test]
    fn test_switch_ignored_in_sdk_info_pane() {
        let mut state = panel_state_with_versions();
        state.flutter_version_state.focused_pane = FlutterVersionPane::SdkInfo;
        let result = handle_switch(&mut state);
        assert!(result.action.is_none());
    }

    #[test]
    fn test_switch_empty_list_returns_none() {
        let mut state = test_app_state();
        state.show_flutter_version();
        state.flutter_version_state.focused_pane = FlutterVersionPane::VersionList;
        let result = handle_switch(&mut state);
        assert!(result.action.is_none());
    }

    #[test]
    fn test_switch_completed_sets_status_and_triggers_scan() {
        let mut state = panel_state_with_versions();
        let result = handle_switch_completed(&mut state, "3.19.0".into());
        assert_eq!(
            state.flutter_version_state.status_message.as_deref(),
            Some("Switched to 3.19.0")
        );
        assert!(matches!(
            result.action,
            Some(UpdateAction::ScanInstalledSdks { .. })
        ));
    }

    #[test]
    fn test_switch_completed_refreshes_sdk_info() {
        let mut state = panel_state_with_versions();
        // resolved_sdk is set via fake_flutter_sdk in test_app_state
        handle_switch_completed(&mut state, "3.19.0".into());
        // sdk_info.resolved_sdk should now mirror resolved_sdk
        assert!(state.flutter_version_state.sdk_info.resolved_sdk.is_some());
    }

    #[test]
    fn test_switch_failed_sets_status() {
        let mut state = panel_state_with_versions();
        handle_switch_failed(&mut state, "permission denied".into());
        assert_eq!(
            state.flutter_version_state.status_message.as_deref(),
            Some("Switch failed: permission denied")
        );
    }

    #[test]
    fn test_remove_active_version_blocked() {
        let mut state = panel_state_with_versions();
        state.flutter_version_state.focused_pane = FlutterVersionPane::VersionList;
        state.flutter_version_state.version_list.selected_index = 0; // active
        let result = handle_remove(&mut state);
        assert!(result.action.is_none());
        assert!(state
            .flutter_version_state
            .status_message
            .as_deref()
            .unwrap()
            .contains("Cannot remove"));
    }

    #[test]
    fn test_remove_non_active_returns_action() {
        let mut state = panel_state_with_versions();
        state.flutter_version_state.focused_pane = FlutterVersionPane::VersionList;
        state.flutter_version_state.version_list.selected_index = 1; // non-active
        let result = handle_remove(&mut state);
        assert!(matches!(
            result.action,
            Some(UpdateAction::RemoveFlutterVersion { .. })
        ));
    }

    #[test]
    fn test_remove_non_active_carries_correct_version() {
        let mut state = panel_state_with_versions();
        state.flutter_version_state.focused_pane = FlutterVersionPane::VersionList;
        state.flutter_version_state.version_list.selected_index = 2; // 3.22.0-beta
        let result = handle_remove(&mut state);
        if let Some(UpdateAction::RemoveFlutterVersion { version, .. }) = result.action {
            assert_eq!(version, "3.22.0-beta");
        } else {
            panic!("expected RemoveFlutterVersion action");
        }
    }

    #[test]
    fn test_remove_ignored_in_sdk_info_pane() {
        let mut state = panel_state_with_versions();
        state.flutter_version_state.focused_pane = FlutterVersionPane::SdkInfo;
        let result = handle_remove(&mut state);
        assert!(result.action.is_none());
    }

    #[test]
    fn test_remove_empty_list_returns_none() {
        let mut state = test_app_state();
        state.show_flutter_version();
        state.flutter_version_state.focused_pane = FlutterVersionPane::VersionList;
        let result = handle_remove(&mut state);
        assert!(result.action.is_none());
    }

    #[test]
    fn test_remove_completed_sets_status_and_triggers_scan() {
        let mut state = panel_state_with_versions();
        let result = handle_remove_completed(&mut state, "3.16.0".into());
        assert_eq!(
            state.flutter_version_state.status_message.as_deref(),
            Some("Removed 3.16.0")
        );
        assert!(matches!(
            result.action,
            Some(UpdateAction::ScanInstalledSdks { .. })
        ));
    }

    #[test]
    fn test_remove_failed_sets_status() {
        let mut state = panel_state_with_versions();
        handle_remove_failed(&mut state, "directory not found".into());
        assert_eq!(
            state.flutter_version_state.status_message.as_deref(),
            Some("Remove failed: directory not found")
        );
    }
}
