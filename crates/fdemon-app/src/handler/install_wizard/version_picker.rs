//! # Install Wizard — Version Picker Handlers
//!
//! Handles the message lifecycle for the Flutter version picker overlay
//! (open / close / navigate / tab / refetch / manifest-applied / confirm).
//!
//! The picker is a sub-overlay of the Install Wizard. It lets the user choose a
//! specific Flutter release (or a git-only `master`/`main` ref) before the
//! managed `FlutterSdk` install runs. The actual install dispatch is delegated
//! to [`super::actions::dispatch_flutter_install`] so token minting / `begin_step`
//! / `run_seq` stay single-sourced.
//!
//! ## Fetch lifecycle
//!
//! `open()` reports whether a manifest fetch is needed. When it is, the handler
//! transitions the picker to `Loading` (`begin_fetch`) and returns
//! `UpdateAction::FetchFlutterReleaseManifest`. The executor (Task 04) then emits
//! `FlutterManifestFetched` or `FlutterManifestFetchFailed`.

use fdemon_daemon::toolchain::{FlutterReleaseManifest, HostArch};

use super::actions::dispatch_flutter_install;
use crate::handler::{UpdateAction, UpdateResult};
use crate::install_wizard::{PickerFetch, WizardStepKind};
use crate::state::AppState;

/// Open the version picker, guarded for the `FlutterSdk` step.
///
/// Shared by the `v` key (`InstallWizardOpenVersionPicker`) and the
/// `Enter`-on-FlutterSdk gate in [`super::actions::handle_run_selected_step`].
///
/// Guards:
/// - Refuses (no-op + status message) while a step is running.
/// - No-ops unless the currently selected step is `FlutterSdk` — the picker only
///   makes sense for the Flutter install step.
///
/// When opened and a manifest fetch is needed (`NotFetched` / `Failed`), the
/// picker transitions to `Loading` and the fetch action is returned.
pub fn open_flutter_version_picker(state: &mut AppState) -> UpdateResult {
    // Refuse while a step is running — opening a picker mid-install is confusing
    // and the install token must not be disturbed.
    if state.install_wizard_state.is_step_running() {
        state.install_wizard_state.status_message =
            Some("Cannot pick a version while a step is running.".to_string());
        return UpdateResult::none();
    }

    // Only meaningful on the FlutterSdk step.
    let on_flutter_sdk = state
        .install_wizard_state
        .selected_step()
        .map(|s| s.kind == WizardStepKind::FlutterSdk)
        .unwrap_or(false);
    if !on_flutter_sdk {
        return UpdateResult::none();
    }

    let needs_fetch = state.install_wizard_state.version_picker.open();
    if needs_fetch {
        state.install_wizard_state.version_picker.begin_fetch();
        return UpdateResult::action(UpdateAction::FetchFlutterReleaseManifest);
    }
    UpdateResult::none()
}

/// Handle `InstallWizardOpenVersionPicker` — the `v` key.
///
/// Delegates to [`open_flutter_version_picker`]; no-ops off the FlutterSdk step.
pub fn handle_open_picker(state: &mut AppState) -> UpdateResult {
    open_flutter_version_picker(state)
}

/// Handle `InstallWizardVersionPickerClose` — Esc closes the overlay.
///
/// Keeps the loaded manifest and last selection cached so re-opening is cheap.
pub fn handle_close_picker(state: &mut AppState) -> UpdateResult {
    state.install_wizard_state.version_picker.close();
    UpdateResult::none()
}

/// Handle `InstallWizardVersionPickerUp` — move the cursor up one row.
pub fn handle_up(state: &mut AppState) -> UpdateResult {
    state.install_wizard_state.version_picker.move_up();
    UpdateResult::none()
}

/// Handle `InstallWizardVersionPickerDown` — move the cursor down one row.
pub fn handle_down(state: &mut AppState) -> UpdateResult {
    state.install_wizard_state.version_picker.move_down();
    UpdateResult::none()
}

/// Handle `InstallWizardVersionPickerNextTab` — cycle to the next channel tab.
pub fn handle_next_tab(state: &mut AppState) -> UpdateResult {
    state.install_wizard_state.version_picker.next_tab();
    UpdateResult::none()
}

/// Handle `InstallWizardVersionPickerRefetch` — `r` re-fetches the manifest.
///
/// No-op unless the picker is visible. Transitions to `Loading` and returns the
/// fetch action so a `Failed` state can be retried.
pub fn handle_refetch(state: &mut AppState) -> UpdateResult {
    if !state.install_wizard_state.version_picker.visible {
        return UpdateResult::none();
    }
    state.install_wizard_state.version_picker.begin_fetch();
    UpdateResult::action(UpdateAction::FetchFlutterReleaseManifest)
}

/// Handle `FlutterManifestFetched` — group the releases and populate the picker.
///
/// Stale-safe: applying with the picker already closed is harmless — the rows
/// are cached for the next open. No sequence guard is needed because the fetch
/// is idempotent and read-only.
pub fn handle_manifest_fetched(
    state: &mut AppState,
    manifest: FlutterReleaseManifest,
) -> UpdateResult {
    state
        .install_wizard_state
        .version_picker
        .apply_manifest(&manifest, HostArch::detect());
    UpdateResult::none()
}

/// Handle `FlutterManifestFetchFailed` — record the error (→ `Failed` state).
pub fn handle_manifest_fetch_failed(state: &mut AppState, error: String) -> UpdateResult {
    state
        .install_wizard_state
        .version_picker
        .apply_fetch_error(error);
    UpdateResult::none()
}

/// Handle `InstallWizardVersionPickerConfirm` — Enter confirms the selection.
///
/// - `PickerFetch::Failed` → offline escape hatch: close the picker and dispatch
///   an un-pinned default-channel install (the same run path as a normal
///   `FlutterSdk` Enter, but with `version_tag: None`).
/// - Otherwise `confirm()` returns the selected row; the pinned install is
///   dispatched through the shared [`dispatch_flutter_install`] helper. An empty
///   tab yields `None` → no-op (the picker stays visible).
pub fn handle_confirm(state: &mut AppState) -> UpdateResult {
    // Offline escape hatch: confirm in the Failed state falls back to a
    // default-channel install so the user is never stranded by a manifest
    // download failure.
    if state.install_wizard_state.version_picker.fetch == PickerFetch::Failed {
        state.install_wizard_state.version_picker.close();
        return dispatch_flutter_install(state, None);
    }

    // Confirm the selection; an empty tab is a no-op (picker stays visible).
    match state.install_wizard_state.version_picker.confirm() {
        Some(row) => dispatch_flutter_install(state, Some(row)),
        None => UpdateResult::none(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::FlutterStepParams;
    use crate::install_wizard::{PickerRow, WizardOrigin};
    use crate::message::Message;
    use fdemon_daemon::toolchain::{
        ComponentCheck, ComponentKind, ComponentStatus, FlutterRelease, HostPlatform, HostShell,
        ToolchainReport,
    };

    fn make_report() -> ToolchainReport {
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

    /// Build a state with the wizard open, a preflight applied, and the
    /// FlutterSdk step selected.
    fn state_on_flutter_sdk() -> AppState {
        let mut state = AppState::new();
        state.show_install_wizard(WizardOrigin::UserInvoked);
        state.install_wizard_state.apply_report(make_report());
        let idx = state
            .install_wizard_state
            .steps
            .iter()
            .position(|s| s.kind == WizardStepKind::FlutterSdk)
            .expect("FlutterSdk step must exist");
        state.install_wizard_state.selected_index = idx;
        state
    }

    fn make_manifest() -> FlutterReleaseManifest {
        FlutterReleaseManifest {
            base_url: "https://example.com".to_string(),
            current_stable_hash: None,
            releases: vec![
                FlutterRelease {
                    version: "3.24.0".to_string(),
                    channel: "stable".to_string(),
                    archive: "stable/3.24.0.tar.xz".to_string(),
                    sha256: "abc".to_string(),
                    dart_sdk_arch: None,
                    release_date: Some("2024-08-21".to_string()),
                },
                FlutterRelease {
                    version: "3.25.0".to_string(),
                    channel: "beta".to_string(),
                    archive: "beta/3.25.0.tar.xz".to_string(),
                    sha256: "def".to_string(),
                    dart_sdk_arch: None,
                    release_date: Some("2024-09-01".to_string()),
                },
            ],
        }
    }

    // ── open / fetch ──────────────────────────────────────────────────────────

    #[test]
    fn test_open_picker_on_flutter_sdk_fetches_once() {
        let mut state = state_on_flutter_sdk();
        let result = open_flutter_version_picker(&mut state);
        assert!(state.install_wizard_state.version_picker.visible);
        assert_eq!(
            state.install_wizard_state.version_picker.fetch,
            PickerFetch::Loading
        );
        assert!(
            matches!(
                result.action,
                Some(UpdateAction::FetchFlutterReleaseManifest)
            ),
            "first open must dispatch the manifest fetch; got {:?}",
            result.action
        );
    }

    #[test]
    fn test_second_open_after_loaded_fetches_nothing() {
        let mut state = state_on_flutter_sdk();
        open_flutter_version_picker(&mut state);
        handle_manifest_fetched(&mut state, make_manifest());
        assert_eq!(
            state.install_wizard_state.version_picker.fetch,
            PickerFetch::Loaded
        );
        // Close, then re-open: no new fetch.
        handle_close_picker(&mut state);
        let result = open_flutter_version_picker(&mut state);
        assert!(
            result.action.is_none(),
            "re-open after Loaded must not fetch; got {:?}",
            result.action
        );
    }

    #[test]
    fn test_open_refused_while_step_running() {
        let mut state = state_on_flutter_sdk();
        state
            .install_wizard_state
            .begin_step(WizardStepKind::FlutterSdk);
        let result = open_flutter_version_picker(&mut state);
        assert!(!state.install_wizard_state.version_picker.visible);
        assert!(result.action.is_none());
        assert!(state.install_wizard_state.status_message.is_some());
    }

    #[test]
    fn test_open_noops_off_flutter_sdk_step() {
        let mut state = state_on_flutter_sdk();
        // Select a non-FlutterSdk step (Prerequisites is always present).
        let idx = state
            .install_wizard_state
            .steps
            .iter()
            .position(|s| s.kind == WizardStepKind::Prerequisites)
            .expect("Prerequisites step must exist");
        state.install_wizard_state.selected_index = idx;

        let result = handle_open_picker(&mut state);
        assert!(!state.install_wizard_state.version_picker.visible);
        assert!(result.action.is_none());
    }

    // ── manifest applied / failed ───────────────────────────────────────────────

    #[test]
    fn test_manifest_fetched_populates_rows() {
        let mut state = state_on_flutter_sdk();
        open_flutter_version_picker(&mut state);
        handle_manifest_fetched(&mut state, make_manifest());
        assert_eq!(
            state.install_wizard_state.version_picker.fetch,
            PickerFetch::Loaded
        );
        assert_eq!(state.install_wizard_state.version_picker.stable.len(), 1);
        assert_eq!(state.install_wizard_state.version_picker.beta.len(), 1);
    }

    #[test]
    fn test_manifest_fetch_failed_sets_failed_state() {
        let mut state = state_on_flutter_sdk();
        open_flutter_version_picker(&mut state);
        handle_manifest_fetch_failed(&mut state, "network down".to_string());
        assert_eq!(
            state.install_wizard_state.version_picker.fetch,
            PickerFetch::Failed
        );
        assert_eq!(
            state.install_wizard_state.version_picker.error.as_deref(),
            Some("network down")
        );
    }

    #[test]
    fn test_refetch_from_failed_redispatches_fetch() {
        let mut state = state_on_flutter_sdk();
        open_flutter_version_picker(&mut state);
        handle_manifest_fetch_failed(&mut state, "network down".to_string());
        let result = handle_refetch(&mut state);
        assert_eq!(
            state.install_wizard_state.version_picker.fetch,
            PickerFetch::Loading
        );
        assert!(matches!(
            result.action,
            Some(UpdateAction::FetchFlutterReleaseManifest)
        ));
    }

    #[test]
    fn test_refetch_noop_when_picker_closed() {
        let mut state = state_on_flutter_sdk();
        let result = handle_refetch(&mut state);
        assert!(result.action.is_none());
    }

    // ── confirm ─────────────────────────────────────────────────────────────────

    #[test]
    fn test_confirm_stable_row_dispatches_pinned_install() {
        let mut state = state_on_flutter_sdk();
        open_flutter_version_picker(&mut state);
        handle_manifest_fetched(&mut state, make_manifest());
        state.install_wizard_state.version_picker.selected_index = 0;

        let result = handle_confirm(&mut state);

        match result.action {
            Some(UpdateAction::RunWizardStep {
                kind: WizardStepKind::FlutterSdk,
                install:
                    Some(FlutterStepParams {
                        version_tag,
                        channel,
                        ..
                    }),
                ..
            }) => {
                assert_eq!(version_tag.as_deref(), Some("3.24.0"));
                assert_eq!(channel, "stable");
            }
            other => panic!("expected pinned FlutterSdk RunWizardStep; got {other:?}"),
        }
        assert!(state.install_wizard_state.is_step_running());
    }

    #[test]
    fn test_confirm_git_only_row_forces_git_clone() {
        let mut state = state_on_flutter_sdk();
        open_flutter_version_picker(&mut state);
        handle_manifest_fetched(&mut state, make_manifest());
        // Switch to the Master tab (Stable → Beta → Master).
        handle_next_tab(&mut state);
        handle_next_tab(&mut state);
        state.install_wizard_state.version_picker.selected_index = 0; // "master"

        let result = handle_confirm(&mut state);

        match result.action {
            Some(UpdateAction::RunWizardStep {
                install: Some(FlutterStepParams { method, .. }),
                ..
            }) => {
                assert_eq!(method, fdemon_daemon::toolchain::InstallMethod::GitClone);
            }
            other => panic!("expected git-clone install; got {other:?}"),
        }
    }

    #[test]
    fn test_confirm_bumps_run_seq_once() {
        let mut state = state_on_flutter_sdk();
        open_flutter_version_picker(&mut state);
        handle_manifest_fetched(&mut state, make_manifest());
        state.install_wizard_state.version_picker.selected_index = 0;
        let seq_before = state.install_wizard_state.run_seq;

        handle_confirm(&mut state);

        assert_eq!(
            state.install_wizard_state.run_seq,
            seq_before + 1,
            "confirm must bump run_seq exactly once"
        );
    }

    #[test]
    fn test_confirm_failed_dispatches_unpinned_default_install() {
        let mut state = state_on_flutter_sdk();
        open_flutter_version_picker(&mut state);
        handle_manifest_fetch_failed(&mut state, "network down".to_string());

        let result = handle_confirm(&mut state);

        // Picker must be closed.
        assert!(!state.install_wizard_state.version_picker.visible);
        match result.action {
            Some(UpdateAction::RunWizardStep {
                kind: WizardStepKind::FlutterSdk,
                install: Some(FlutterStepParams { version_tag, .. }),
                ..
            }) => {
                assert!(
                    version_tag.is_none(),
                    "offline fallback must be un-pinned (version_tag: None)"
                );
            }
            other => panic!("expected un-pinned FlutterSdk install; got {other:?}"),
        }
    }

    #[test]
    fn test_confirm_empty_tab_is_noop() {
        let mut state = state_on_flutter_sdk();
        open_flutter_version_picker(&mut state);
        // Loaded state but no rows in the active tab.
        state
            .install_wizard_state
            .version_picker
            .apply_manifest(&empty_manifest(), HostArch::detect());
        // Stable tab is empty.
        let result = handle_confirm(&mut state);
        assert!(result.action.is_none());
        assert!(
            state.install_wizard_state.version_picker.visible,
            "picker stays visible on empty confirm"
        );
    }

    fn empty_manifest() -> FlutterReleaseManifest {
        FlutterReleaseManifest {
            base_url: "https://example.com".to_string(),
            current_stable_hash: None,
            releases: vec![],
        }
    }

    // ── Enter-on-FlutterSdk gate (two-step flow) ───────────────────────────────

    #[test]
    fn test_enter_on_flutter_sdk_with_no_choice_opens_picker() {
        let mut state = state_on_flutter_sdk();
        let result = super::super::actions::handle_run_selected_step(&mut state);
        assert!(
            state.install_wizard_state.version_picker.visible,
            "Enter on FlutterSdk with no choice must open the picker"
        );
        assert!(
            matches!(
                result.action,
                Some(UpdateAction::FetchFlutterReleaseManifest)
            ),
            "must dispatch the manifest fetch on first open; got {:?}",
            result.action
        );
        // No install was dispatched yet.
        assert!(!state.install_wizard_state.is_step_running());
    }

    #[test]
    fn test_enter_on_flutter_sdk_with_existing_choice_dispatches_install() {
        let mut state = state_on_flutter_sdk();
        // Simulate a prior confirmed selection.
        state.install_wizard_state.version_picker.selected_release = Some(PickerRow {
            version: "3.24.0".to_string(),
            channel: "stable".to_string(),
            release_date: None,
            arch: None,
            git_only: false,
        });

        let result = super::super::actions::handle_run_selected_step(&mut state);
        match result.action {
            Some(UpdateAction::RunWizardStep {
                install: Some(FlutterStepParams { version_tag, .. }),
                ..
            }) => assert_eq!(version_tag.as_deref(), Some("3.24.0")),
            other => panic!("expected pinned install with existing choice; got {other:?}"),
        }
    }

    #[test]
    fn test_close_keeps_manifest_for_reopen() {
        let mut state = state_on_flutter_sdk();
        open_flutter_version_picker(&mut state);
        handle_manifest_fetched(&mut state, make_manifest());
        handle_close_picker(&mut state);
        assert!(!state.install_wizard_state.version_picker.visible);
        assert_eq!(
            state.install_wizard_state.version_picker.fetch,
            PickerFetch::Loaded,
            "close must keep the loaded manifest"
        );
    }

    /// Wizard hide must reset the picker (manifest dropped, selection cleared).
    #[test]
    fn test_wizard_hide_resets_picker() {
        let mut state = state_on_flutter_sdk();
        open_flutter_version_picker(&mut state);
        handle_manifest_fetched(&mut state, make_manifest());
        state.install_wizard_state.version_picker.selected_release = Some(PickerRow {
            version: "3.24.0".to_string(),
            channel: "stable".to_string(),
            release_date: None,
            arch: None,
            git_only: false,
        });

        state.hide_install_wizard();

        assert_eq!(
            state.install_wizard_state.version_picker.fetch,
            PickerFetch::NotFetched,
            "hide must drop the manifest"
        );
        assert!(
            state
                .install_wizard_state
                .version_picker
                .selected_release
                .is_none(),
            "hide must clear the confirmed selection"
        );
        assert!(state.install_wizard_state.version_picker.stable.is_empty());
    }

    // Silence unused-import warnings on platforms where Message is otherwise unused.
    #[allow(dead_code)]
    fn _assert_message_variant_exists() {
        let _ = Message::InstallWizardOpenVersionPicker;
    }
}
