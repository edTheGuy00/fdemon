//! # Flutter Version Panel Action Handlers
//!
//! Handles SDK operations (version switch, removal) and async result messages
//! (scan completed/failed, switch completed/failed, remove completed/failed)
//! for the Flutter Version panel.

use crate::flutter_version::{FlutterVersionPane, InstalledSdk};
use crate::handler::{UpdateAction, UpdateResult};
use crate::state::AppState;
use fdemon_daemon::FlutterVersionInfo;

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

    // Refresh dart_version from the new SDK's dart-sdk/version file
    state.flutter_version_state.sdk_info.dart_version = state
        .resolved_sdk
        .as_ref()
        .and_then(|sdk| crate::flutter_version::read_dart_version(&sdk.root));

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
/// Implements a double-press `d` confirmation pattern (similar to Vim's `dd`):
/// - First press: Set `pending_delete = Some(selected_index)`, show confirmation prompt.
/// - Second press (same index): Execute removal, clear `pending_delete`.
/// - Active version guard: Show error message, clear `pending_delete`.
///
/// Guards:
/// - Must be focused on the `VersionList` pane.
/// - Must not remove the currently active version.
///
/// Returns `UpdateAction::RemoveFlutterVersion` on confirmed second press;
/// otherwise sets a status message and returns no action.
pub fn handle_remove(state: &mut AppState) -> UpdateResult {
    let fv = &state.flutter_version_state;

    if fv.focused_pane != FlutterVersionPane::VersionList {
        return UpdateResult::none();
    }

    let selected_index = fv.version_list.selected_index;

    let Some(selected) = fv.version_list.installed_versions.get(selected_index) else {
        return UpdateResult::none();
    };

    // Don't allow removing the active version — clear any pending state
    if selected.is_active {
        state.flutter_version_state.status_message =
            Some("Cannot remove the active SDK version".into());
        state.flutter_version_state.pending_delete = None;
        return UpdateResult::none();
    }

    // Double-press confirmation pattern
    if state.flutter_version_state.pending_delete == Some(selected_index) {
        // Second press — confirmed, proceed with removal
        state.flutter_version_state.pending_delete = None;
        let fv = &state.flutter_version_state;
        let selected = &fv.version_list.installed_versions[selected_index];
        let version = selected.version.clone();
        let path = selected.path.clone();
        let active_sdk_root = state.resolved_sdk.as_ref().map(|sdk| sdk.root.clone());
        UpdateResult::action(UpdateAction::RemoveFlutterVersion {
            version,
            path,
            active_sdk_root,
        })
    } else {
        // First press — set pending and show confirmation prompt
        let version = state.flutter_version_state.version_list.installed_versions[selected_index]
            .version
            .clone();
        state.flutter_version_state.pending_delete = Some(selected_index);
        state.flutter_version_state.status_message =
            Some(format!("Press d again to remove {version}"));
        UpdateResult::none()
    }
}

/// Handle `FlutterVersionInstall` — Phase 3 stub.
///
/// Install functionality is not yet available; shows an informational message.
pub fn handle_install(state: &mut AppState) -> UpdateResult {
    state.flutter_version_state.status_message = Some("Install not yet available".into());
    UpdateResult::none()
}

/// Handle `FlutterVersionUpdate` — Phase 3 stub.
///
/// Update functionality is not yet available; shows an informational message.
pub fn handle_update(state: &mut AppState) -> UpdateResult {
    state.flutter_version_state.status_message = Some("Update not yet available".into());
    UpdateResult::none()
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

/// Handle `FlutterVersionProbeRequested` — dispatch the version probe if not yet done.
///
/// Returns `UpdateAction::ProbeFlutterVersion` only when `probe_completed` is
/// `false` (i.e., the probe has not yet run for the current panel open).
/// Re-opening the panel after a successful probe is a no-op here because
/// `probe_completed` persists across closes until the state is reset.
pub fn handle_probe_requested(state: &mut AppState) -> UpdateResult {
    if state.flutter_version_state.sdk_info.probe_completed {
        // Probe already finished — cached data is current, do nothing.
        return UpdateResult::none();
    }
    let executable = state
        .resolved_sdk
        .as_ref()
        .map(|sdk| sdk.executable.clone());
    UpdateResult::action(UpdateAction::ProbeFlutterVersion { executable })
}

/// Handle `FlutterVersionProbeCompleted` — populate extended SDK metadata.
///
/// On success, fills in `framework_revision`, `engine_revision`,
/// `devtools_version`, and (if previously unknown) `version`, `channel`, and
/// `dart_version` from the probe result.  On failure, logs at debug level and
/// sets `probe_completed = true` so the TUI shows em-dashes rather than
/// indefinite loading indicators.
pub fn handle_version_probe_completed(
    state: &mut AppState,
    result: std::result::Result<FlutterVersionInfo, String>,
) -> UpdateResult {
    match result {
        Ok(info) => {
            let sdk_info = &mut state.flutter_version_state.sdk_info;

            // Populate extended probe fields
            sdk_info.framework_revision = info.framework_revision;
            sdk_info.engine_revision = info.engine_revision.map(|r| {
                // Truncate engine hash to 10 chars for display (matches Flutter CLI short hash)
                if r.len() > 10 {
                    r[..10].to_string()
                } else {
                    r
                }
            });
            sdk_info.devtools_version = info.devtools_version;

            // If the resolved SDK's version was "unknown", update from probe
            if let Some(ref mut sdk) = sdk_info.resolved_sdk {
                if sdk.version == "unknown" {
                    if let Some(ref ver) = info.framework_version {
                        sdk.version = ver.clone();
                    }
                }
                // Populate channel if it was missing
                if sdk.channel.is_none() {
                    sdk.channel = info.channel.clone();
                }
            }

            // Update dart_version if it was missing
            if sdk_info.dart_version.is_none() {
                sdk_info.dart_version = info.dart_sdk_version;
            }

            // Propagate enriched metadata to top-level resolved_sdk so that
            // future panel opens start with correct version/channel.
            if let Some(ref mut top_sdk) = state.resolved_sdk {
                if top_sdk.version == "unknown" {
                    if let Some(ref ver) = info.framework_version {
                        top_sdk.version = ver.clone();
                    }
                }
                if top_sdk.channel.is_none() {
                    if let Some(ch) = info.channel {
                        top_sdk.channel = Some(ch);
                    }
                }
            }

            state.flutter_version_state.sdk_info.probe_completed = true;
        }
        Err(reason) => {
            tracing::debug!("Flutter version probe failed: {reason}");
            // Non-fatal — file-based data remains displayed; TUI switches from
            // "..." loading indicator to "—" em-dash for unavailable fields.
            state.flutter_version_state.sdk_info.probe_completed = true;
        }
    }
    UpdateResult::none()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flutter_version::InstalledSdk;
    use crate::state::AppState;
    use fdemon_daemon::test_utils::fake_flutter_sdk;
    use std::path::PathBuf;

    // ── probe handler helpers ─────────────────────────────────────────────────

    fn make_app_state_with_unknown_version() -> AppState {
        let mut state = AppState::new();
        let mut sdk = fake_flutter_sdk();
        sdk.version = "unknown".to_string();
        sdk.channel = None;
        state.resolved_sdk = Some(sdk.clone());
        state.show_flutter_version();
        // sdk_info snapshot also has "unknown"
        state.flutter_version_state.sdk_info.resolved_sdk = Some(sdk);
        state.flutter_version_state.sdk_info.dart_version = None;
        state
    }

    fn make_app_state_with_known_version(version: &str) -> AppState {
        let mut state = AppState::new();
        let mut sdk = fake_flutter_sdk();
        sdk.version = version.to_string();
        state.resolved_sdk = Some(sdk.clone());
        state.show_flutter_version();
        state.flutter_version_state.sdk_info.resolved_sdk = Some(sdk);
        state
    }

    // ── probe handler tests ───────────────────────────────────────────────────

    #[test]
    fn test_handle_version_probe_completed_success() {
        let mut state = make_app_state_with_unknown_version();

        let info = FlutterVersionInfo {
            framework_version: Some("3.38.6".into()),
            channel: Some("stable".into()),
            framework_revision: Some("8b87286849".into()),
            engine_revision: Some("6f3039bf7c3cb5306513c75092822d4d94716003".into()),
            dart_sdk_version: Some("3.10.7".into()),
            devtools_version: Some("2.51.1".into()),
            ..Default::default()
        };

        let result = handle_version_probe_completed(&mut state, Ok(info));
        assert!(result.action.is_none());

        // Version should be updated from "unknown" to "3.38.6"
        let sdk = state
            .flutter_version_state
            .sdk_info
            .resolved_sdk
            .as_ref()
            .unwrap();
        assert_eq!(sdk.version, "3.38.6");

        // Extended fields populated; engine revision truncated to 10 chars
        assert_eq!(
            state
                .flutter_version_state
                .sdk_info
                .framework_revision
                .as_deref(),
            Some("8b87286849")
        );
        assert_eq!(
            state
                .flutter_version_state
                .sdk_info
                .engine_revision
                .as_deref(),
            Some("6f3039bf7c")
        );
        assert_eq!(
            state
                .flutter_version_state
                .sdk_info
                .devtools_version
                .as_deref(),
            Some("2.51.1")
        );
        assert!(state.flutter_version_state.sdk_info.probe_completed);
    }

    #[test]
    fn test_handle_version_probe_completed_failure() {
        let mut state = make_app_state_with_unknown_version();

        let result = handle_version_probe_completed(&mut state, Err("timeout".into()));
        assert!(result.action.is_none());
        assert!(state.flutter_version_state.sdk_info.probe_completed);

        // Original "unknown" version should remain
        let sdk = state
            .flutter_version_state
            .sdk_info
            .resolved_sdk
            .as_ref()
            .unwrap();
        assert_eq!(sdk.version, "unknown");
    }

    #[test]
    fn test_handle_version_probe_does_not_overwrite_known_version() {
        let mut state = make_app_state_with_known_version("3.19.0");

        let info = FlutterVersionInfo {
            framework_version: Some("3.19.0".into()),
            ..Default::default()
        };

        handle_version_probe_completed(&mut state, Ok(info));

        // Version should remain "3.19.0" (was already known)
        let sdk = state
            .flutter_version_state
            .sdk_info
            .resolved_sdk
            .as_ref()
            .unwrap();
        assert_eq!(sdk.version, "3.19.0");
    }

    #[test]
    fn test_probe_completed_sets_probe_flag() {
        let mut state = make_app_state_with_unknown_version();
        assert!(!state.flutter_version_state.sdk_info.probe_completed);

        handle_version_probe_completed(&mut state, Err("any error".into()));
        assert!(state.flutter_version_state.sdk_info.probe_completed);
    }

    #[test]
    fn test_probe_requested_returns_probe_action_when_not_completed() {
        let mut state = make_app_state_with_known_version("3.19.0");
        state.flutter_version_state.sdk_info.probe_completed = false;

        let result = handle_probe_requested(&mut state);
        assert!(matches!(
            result.action,
            Some(UpdateAction::ProbeFlutterVersion { .. })
        ));
    }

    #[test]
    fn test_probe_requested_skips_when_already_completed() {
        let mut state = make_app_state_with_known_version("3.19.0");
        state.flutter_version_state.sdk_info.probe_completed = true;

        let result = handle_probe_requested(&mut state);
        assert!(result.action.is_none());
    }

    #[test]
    fn test_probe_requested_with_no_sdk_passes_none_executable() {
        let mut state = AppState::new();
        state.resolved_sdk = None;
        state.show_flutter_version();
        state.flutter_version_state.sdk_info.probe_completed = false;

        let result = handle_probe_requested(&mut state);
        // Should still return the action, but with None executable
        match result.action {
            Some(UpdateAction::ProbeFlutterVersion { executable }) => {
                assert!(executable.is_none());
            }
            _ => panic!("expected ProbeFlutterVersion action"),
        }
    }

    #[test]
    fn test_probe_updates_dart_version_when_missing() {
        let mut state = make_app_state_with_unknown_version();
        state.flutter_version_state.sdk_info.dart_version = None;

        let info = FlutterVersionInfo {
            dart_sdk_version: Some("3.10.7".into()),
            ..Default::default()
        };

        handle_version_probe_completed(&mut state, Ok(info));
        assert_eq!(
            state.flutter_version_state.sdk_info.dart_version.as_deref(),
            Some("3.10.7")
        );
    }

    #[test]
    fn test_probe_does_not_overwrite_existing_dart_version() {
        let mut state = make_app_state_with_unknown_version();
        state.flutter_version_state.sdk_info.dart_version = Some("3.3.0".into());

        let info = FlutterVersionInfo {
            dart_sdk_version: Some("3.10.7".into()),
            ..Default::default()
        };

        handle_version_probe_completed(&mut state, Ok(info));
        // Existing dart_version should be preserved
        assert_eq!(
            state.flutter_version_state.sdk_info.dart_version.as_deref(),
            Some("3.3.0")
        );
    }

    #[test]
    fn test_probe_truncates_long_engine_revision() {
        let mut state = make_app_state_with_known_version("3.19.0");

        let info = FlutterVersionInfo {
            engine_revision: Some("6f3039bf7c3cb5306513c75092822d4d94716003".into()), // 40 chars
            ..Default::default()
        };

        handle_version_probe_completed(&mut state, Ok(info));
        let engine = state
            .flutter_version_state
            .sdk_info
            .engine_revision
            .as_deref();
        assert_eq!(engine, Some("6f3039bf7c")); // truncated to 10 chars
    }

    #[test]
    fn test_probe_short_engine_revision_not_truncated() {
        let mut state = make_app_state_with_known_version("3.19.0");

        let info = FlutterVersionInfo {
            engine_revision: Some("abc123".into()), // 6 chars, shorter than 10
            ..Default::default()
        };

        handle_version_probe_completed(&mut state, Ok(info));
        assert_eq!(
            state
                .flutter_version_state
                .sdk_info
                .engine_revision
                .as_deref(),
            Some("abc123")
        );
    }

    #[test]
    fn test_probe_updates_top_level_resolved_sdk() {
        let mut state = make_app_state_with_unknown_version();

        let info = FlutterVersionInfo {
            framework_version: Some("3.38.6".into()),
            channel: Some("stable".into()),
            ..Default::default()
        };

        handle_version_probe_completed(&mut state, Ok(info));

        // Top-level resolved_sdk should also be updated
        let top_sdk = state.resolved_sdk.as_ref().unwrap();
        assert_eq!(top_sdk.version, "3.38.6");
        assert_eq!(top_sdk.channel.as_deref(), Some("stable"));
    }

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
    fn test_switch_completed_updates_dart_version() {
        let mut state = panel_state_with_versions();
        // Simulate a dart_version from the original SDK
        state.flutter_version_state.sdk_info.dart_version = Some("3.3.0".to_string());

        // After switch, dart_version should be refreshed (will be None in test
        // since the fake SDK path doesn't have a real dart-sdk/version file)
        let result = handle_switch_completed(&mut state, "3.22.0".to_string());

        // The key assertion is that the code path runs without error
        assert!(result.action.is_some()); // ScanInstalledSdks returned
                                          // resolved_sdk was copied to sdk_info
        assert!(state.flutter_version_state.sdk_info.resolved_sdk.is_some());
        // dart_version was refreshed (fake SDK has no version file, so it's None)
        assert!(state.flutter_version_state.sdk_info.dart_version.is_none());
    }

    #[test]
    fn test_switch_completed_reads_new_dart_version() {
        let dir = tempfile::tempdir().unwrap();
        let dart_version_dir = dir.path().join("bin/cache/dart-sdk");
        std::fs::create_dir_all(&dart_version_dir).unwrap();
        std::fs::write(dart_version_dir.join("version"), "3.4.0\n").unwrap();

        let mut state = panel_state_with_versions();
        let mut sdk = fdemon_daemon::test_utils::fake_flutter_sdk();
        sdk.root = dir.path().to_path_buf();
        state.resolved_sdk = Some(sdk);
        state.flutter_version_state.sdk_info.dart_version = Some("3.3.0".to_string());

        handle_switch_completed(&mut state, "3.22.0".to_string());

        assert_eq!(
            state.flutter_version_state.sdk_info.dart_version.as_deref(),
            Some("3.4.0")
        );
    }

    #[test]
    fn test_switch_completed_none_sdk_clears_dart_version() {
        let mut state = panel_state_with_versions();
        // Set resolved_sdk to None (edge case)
        state.resolved_sdk = None;
        state.flutter_version_state.sdk_info.dart_version = Some("3.3.0".to_string());

        handle_switch_completed(&mut state, "3.22.0".to_string());

        // When resolved_sdk is None, dart_version should be None
        assert!(state.flutter_version_state.sdk_info.dart_version.is_none());
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
    fn test_remove_active_version_clears_pending() {
        let mut state = panel_state_with_versions();
        state.flutter_version_state.focused_pane = FlutterVersionPane::VersionList;
        state.flutter_version_state.version_list.selected_index = 0; // active
        state.flutter_version_state.pending_delete = Some(0);
        handle_remove(&mut state);
        assert!(state.flutter_version_state.pending_delete.is_none());
    }

    #[test]
    fn test_remove_first_press_sets_pending() {
        let mut state = panel_state_with_versions();
        state.flutter_version_state.focused_pane = FlutterVersionPane::VersionList;
        state.flutter_version_state.version_list.selected_index = 1; // non-active
        let result = handle_remove(&mut state);
        assert!(result.action.is_none());
        assert_eq!(state.flutter_version_state.pending_delete, Some(1));
        assert!(state
            .flutter_version_state
            .status_message
            .as_deref()
            .unwrap()
            .contains("again"));
    }

    #[test]
    fn test_remove_first_press_shows_version_in_message() {
        let mut state = panel_state_with_versions();
        state.flutter_version_state.focused_pane = FlutterVersionPane::VersionList;
        state.flutter_version_state.version_list.selected_index = 1; // 3.16.0
        handle_remove(&mut state);
        assert!(state
            .flutter_version_state
            .status_message
            .as_deref()
            .unwrap()
            .contains("3.16.0"));
    }

    #[test]
    fn test_remove_second_press_returns_action() {
        let mut state = panel_state_with_versions();
        state.flutter_version_state.focused_pane = FlutterVersionPane::VersionList;
        state.flutter_version_state.version_list.selected_index = 1; // non-active

        // First press
        handle_remove(&mut state);
        assert_eq!(state.flutter_version_state.pending_delete, Some(1));

        // Second press — should trigger removal
        let result = handle_remove(&mut state);
        assert!(matches!(
            result.action,
            Some(UpdateAction::RemoveFlutterVersion { .. })
        ));
        assert!(state.flutter_version_state.pending_delete.is_none());
    }

    #[test]
    fn test_remove_second_press_carries_correct_version() {
        let mut state = panel_state_with_versions();
        state.flutter_version_state.focused_pane = FlutterVersionPane::VersionList;
        state.flutter_version_state.version_list.selected_index = 2; // 3.22.0-beta

        // First press
        handle_remove(&mut state);
        // Second press
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
