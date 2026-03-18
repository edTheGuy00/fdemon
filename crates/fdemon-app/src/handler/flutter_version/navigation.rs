//! # Flutter Version Panel Navigation Handlers
//!
//! Handles panel lifecycle (open, close, escape) and list scrolling
//! for the Flutter Version panel.

use crate::flutter_version::{FlutterVersionPane, VersionListState};
use crate::handler::{UpdateAction, UpdateResult};
use crate::state::AppState;

/// Default visible height when no render-hint is available yet.
///
/// Used as a fallback before the first render frame has written back
/// the actual height via `Cell<usize>`. Follows CODE_STANDARDS.md Principle 3.
const DEFAULT_VISIBLE_HEIGHT: usize = 10;

/// Handle `ShowFlutterVersion` — opens the Flutter Version panel.
///
/// Snapshots the current `resolved_sdk` into `SdkInfoState` and transitions
/// to `UiMode::FlutterVersion`. Triggers an async cache scan so the version
/// list is populated once `FlutterVersionScanCompleted` arrives.
pub fn handle_show(state: &mut AppState) -> UpdateResult {
    state.show_flutter_version();

    // Trigger async cache scan
    let active_sdk_root = state.resolved_sdk.as_ref().map(|sdk| sdk.root.clone());
    UpdateResult::action(UpdateAction::ScanInstalledSdks { active_sdk_root })
}

/// Handle `HideFlutterVersion` — closes the Flutter Version panel.
pub fn handle_hide(state: &mut AppState) -> UpdateResult {
    state.hide_flutter_version();
    UpdateResult::none()
}

/// Handle `FlutterVersionEscape` — priority-ordered escape from the panel.
///
/// In Phase 2 there are no sub-modals, so this always closes the panel.
/// When Phase 3 adds install/confirm modals, this function will check for
/// open sub-modals first.
pub fn handle_escape(state: &mut AppState) -> UpdateResult {
    state.hide_flutter_version();
    UpdateResult::none()
}

/// Handle `FlutterVersionSwitchPane` — toggle focus between SDK info and version list.
pub fn handle_switch_pane(state: &mut AppState) -> UpdateResult {
    let fv = &mut state.flutter_version_state;
    fv.focused_pane = match fv.focused_pane {
        FlutterVersionPane::SdkInfo => FlutterVersionPane::VersionList,
        FlutterVersionPane::VersionList => FlutterVersionPane::SdkInfo,
    };
    UpdateResult::none()
}

/// Handle `FlutterVersionUp` — navigate up in the version list.
///
/// No-op when the `SdkInfo` pane is focused.
pub fn handle_up(state: &mut AppState) -> UpdateResult {
    let fv = &mut state.flutter_version_state;
    if fv.focused_pane != FlutterVersionPane::VersionList {
        return UpdateResult::none();
    }
    if fv.version_list.selected_index > 0 {
        fv.version_list.selected_index -= 1;
        adjust_scroll(&mut fv.version_list);
    }
    UpdateResult::none()
}

/// Handle `FlutterVersionDown` — navigate down in the version list.
///
/// No-op when the `SdkInfo` pane is focused.
pub fn handle_down(state: &mut AppState) -> UpdateResult {
    let fv = &mut state.flutter_version_state;
    if fv.focused_pane != FlutterVersionPane::VersionList {
        return UpdateResult::none();
    }
    let max = fv.version_list.installed_versions.len().saturating_sub(1);
    if fv.version_list.selected_index < max {
        fv.version_list.selected_index += 1;
        adjust_scroll(&mut fv.version_list);
    }
    UpdateResult::none()
}

/// Adjust `scroll_offset` so that `selected_index` is visible in the viewport.
///
/// Uses the `Cell<usize>` render-hint from `last_known_visible_height` with
/// `DEFAULT_VISIBLE_HEIGHT` as a fallback before the first render.
/// Follows CODE_STANDARDS.md Principle 3 (Cell render-hint pattern).
fn adjust_scroll(list: &mut VersionListState) {
    let height = list.last_known_visible_height.get();
    let effective = if height > 0 {
        height
    } else {
        DEFAULT_VISIBLE_HEIGHT
    };

    // Scroll down if selected item is below visible window
    if list.selected_index >= list.scroll_offset + effective {
        list.scroll_offset = list.selected_index + 1 - effective;
    }
    // Scroll up if selected item is above visible window
    if list.selected_index < list.scroll_offset {
        list.scroll_offset = list.selected_index;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flutter_version::InstalledSdk;
    use crate::state::{AppState, UiMode};
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
    fn test_show_sets_ui_mode_and_triggers_scan() {
        let mut state = test_app_state();
        let result = handle_show(&mut state);
        assert_eq!(state.ui_mode, UiMode::FlutterVersion);
        assert!(state.flutter_version_state.visible);
        assert!(matches!(
            result.action,
            Some(UpdateAction::ScanInstalledSdks { .. })
        ));
    }

    #[test]
    fn test_hide_returns_to_normal() {
        let mut state = panel_state_with_versions();
        handle_hide(&mut state);
        assert_eq!(state.ui_mode, UiMode::Normal);
        assert!(!state.flutter_version_state.visible);
    }

    #[test]
    fn test_escape_closes_panel() {
        let mut state = panel_state_with_versions();
        handle_escape(&mut state);
        assert_eq!(state.ui_mode, UiMode::Normal);
        assert!(!state.flutter_version_state.visible);
    }

    #[test]
    fn test_switch_pane_toggles() {
        let mut state = panel_state_with_versions();
        assert_eq!(
            state.flutter_version_state.focused_pane,
            FlutterVersionPane::SdkInfo
        );
        handle_switch_pane(&mut state);
        assert_eq!(
            state.flutter_version_state.focused_pane,
            FlutterVersionPane::VersionList
        );
        handle_switch_pane(&mut state);
        assert_eq!(
            state.flutter_version_state.focused_pane,
            FlutterVersionPane::SdkInfo
        );
    }

    #[test]
    fn test_up_decrements_selected_index() {
        let mut state = panel_state_with_versions();
        state.flutter_version_state.focused_pane = FlutterVersionPane::VersionList;
        state.flutter_version_state.version_list.selected_index = 2;
        handle_up(&mut state);
        assert_eq!(state.flutter_version_state.version_list.selected_index, 1);
    }

    #[test]
    fn test_up_at_zero_stays_at_zero() {
        let mut state = panel_state_with_versions();
        state.flutter_version_state.focused_pane = FlutterVersionPane::VersionList;
        state.flutter_version_state.version_list.selected_index = 0;
        handle_up(&mut state);
        assert_eq!(state.flutter_version_state.version_list.selected_index, 0);
    }

    #[test]
    fn test_down_increments_selected_index() {
        let mut state = panel_state_with_versions();
        state.flutter_version_state.focused_pane = FlutterVersionPane::VersionList;
        state.flutter_version_state.version_list.selected_index = 0;
        handle_down(&mut state);
        assert_eq!(state.flutter_version_state.version_list.selected_index, 1);
    }

    #[test]
    fn test_down_at_max_stays_at_max() {
        let mut state = panel_state_with_versions();
        state.flutter_version_state.focused_pane = FlutterVersionPane::VersionList;
        state.flutter_version_state.version_list.selected_index = 2; // last item
        handle_down(&mut state);
        assert_eq!(state.flutter_version_state.version_list.selected_index, 2);
    }

    #[test]
    fn test_up_ignored_in_sdk_info_pane() {
        let mut state = panel_state_with_versions();
        state.flutter_version_state.focused_pane = FlutterVersionPane::SdkInfo;
        state.flutter_version_state.version_list.selected_index = 1;
        handle_up(&mut state);
        assert_eq!(state.flutter_version_state.version_list.selected_index, 1);
    }

    #[test]
    fn test_down_ignored_in_sdk_info_pane() {
        let mut state = panel_state_with_versions();
        state.flutter_version_state.focused_pane = FlutterVersionPane::SdkInfo;
        state.flutter_version_state.version_list.selected_index = 0;
        handle_down(&mut state);
        assert_eq!(state.flutter_version_state.version_list.selected_index, 0);
    }

    #[test]
    fn test_adjust_scroll_uses_render_hint_when_set() {
        let mut list = VersionListState::default();
        // EXCEPTION: TEA render-hint write-back via Cell — see docs/CODE_STANDARDS.md
        list.last_known_visible_height.set(3);
        list.installed_versions = vec![
            InstalledSdk {
                version: "a".into(),
                channel: None,
                path: PathBuf::from("/a"),
                is_active: false,
            },
            InstalledSdk {
                version: "b".into(),
                channel: None,
                path: PathBuf::from("/b"),
                is_active: false,
            },
            InstalledSdk {
                version: "c".into(),
                channel: None,
                path: PathBuf::from("/c"),
                is_active: false,
            },
            InstalledSdk {
                version: "d".into(),
                channel: None,
                path: PathBuf::from("/d"),
                is_active: false,
            },
        ];
        // Select item 3, window size 3 — scroll should move to show it
        list.selected_index = 3;
        adjust_scroll(&mut list);
        assert_eq!(list.scroll_offset, 1); // 3 + 1 - 3 = 1
    }

    #[test]
    fn test_adjust_scroll_falls_back_to_default_height_when_not_rendered() {
        let mut list = VersionListState::default();
        // last_known_visible_height is 0 (not yet rendered)
        assert_eq!(list.last_known_visible_height.get(), 0);
        // With DEFAULT_VISIBLE_HEIGHT = 10, selecting index 0 should not scroll
        list.selected_index = 0;
        adjust_scroll(&mut list);
        assert_eq!(list.scroll_offset, 0);
    }

    #[test]
    fn test_show_with_no_resolved_sdk_still_triggers_scan() {
        let mut state = AppState::new();
        // No resolved_sdk
        let result = handle_show(&mut state);
        assert!(state.flutter_version_state.visible);
        assert!(matches!(
            result.action,
            Some(UpdateAction::ScanInstalledSdks {
                active_sdk_root: None
            })
        ));
    }
}
