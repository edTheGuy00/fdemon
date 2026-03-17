## Task: Handler Module — Navigation and Actions

**Objective**: Implement the full handler module for the Flutter Version panel with navigation logic (open/close, pane switching, list scrolling) and action logic (version switching, removal), following the `handler/new_session/` decomposition pattern.

**Depends on**: 03-messages-and-update

### Scope

- `crates/fdemon-app/src/handler/flutter_version/mod.rs`: Replace stubs with real module structure
- `crates/fdemon-app/src/handler/flutter_version/navigation.rs`: **NEW** Open, close, escape, pane switch, list navigation
- `crates/fdemon-app/src/handler/flutter_version/actions.rs`: **NEW** Version switch, remove, scan result handling

### Details

#### 1. Module Structure

```
crates/fdemon-app/src/handler/flutter_version/
├── mod.rs          — Re-exports from navigation.rs and actions.rs
├── navigation.rs   — Panel lifecycle + list scrolling
└── actions.rs      — SDK operations + async result handling
```

Replace the Task 03 stubs in `mod.rs` with:

```rust
//! # Flutter Version Panel Handlers
//!
//! Handles all messages for the Flutter Version panel overlay.
//! Follows the handler/new_session/ decomposition pattern.

mod actions;
mod navigation;

pub use actions::*;
pub use navigation::*;
```

#### 2. Navigation Handlers (`navigation.rs`)

##### `handle_show(state) -> UpdateResult`

Opens the panel and triggers a cache scan:

```rust
pub fn handle_show(state: &mut AppState) -> UpdateResult {
    state.show_flutter_version();

    // Trigger async cache scan
    let active_sdk_root = state.resolved_sdk.as_ref().map(|sdk| sdk.root.clone());
    UpdateResult::action(UpdateAction::ScanInstalledSdks { active_sdk_root })
}
```

**Key behavior**: `show_flutter_version()` snapshots the current `resolved_sdk` into `SdkInfoState`. The cache scan runs asynchronously — the panel shows a loading spinner until `FlutterVersionScanCompleted` arrives.

##### `handle_hide(state) -> UpdateResult`

```rust
pub fn handle_hide(state: &mut AppState) -> UpdateResult {
    state.hide_flutter_version();
    UpdateResult::none()
}
```

##### `handle_escape(state) -> UpdateResult`

Priority-ordered escape (mirrors `handle_new_session_dialog_escape`):

```rust
pub fn handle_escape(state: &mut AppState) -> UpdateResult {
    // No sub-modals in Phase 2, so always close the panel
    state.hide_flutter_version();
    UpdateResult::none()
}
```

**Future-proofed**: When Phase 3 adds install/confirm modals, this function will check for open sub-modals first.

##### `handle_switch_pane(state) -> UpdateResult`

Toggle focus between left (SdkInfo) and right (VersionList) panes:

```rust
pub fn handle_switch_pane(state: &mut AppState) -> UpdateResult {
    let fv = &mut state.flutter_version_state;
    fv.focused_pane = match fv.focused_pane {
        FlutterVersionPane::SdkInfo => FlutterVersionPane::VersionList,
        FlutterVersionPane::VersionList => FlutterVersionPane::SdkInfo,
    };
    UpdateResult::none()
}
```

##### `handle_up(state) -> UpdateResult`

Navigate up in the version list (only when VersionList pane is focused):

```rust
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
```

##### `handle_down(state) -> UpdateResult`

```rust
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
```

##### `adjust_scroll(list) -> ()`

Uses the `Cell<usize>` render-hint pattern from `VersionListState.last_known_visible_height`:

```rust
/// Default visible height when no render-hint is available yet.
const DEFAULT_VISIBLE_HEIGHT: usize = 10;

fn adjust_scroll(list: &mut VersionListState) {
    let height = list.last_known_visible_height.get();
    let effective = if height > 0 { height } else { DEFAULT_VISIBLE_HEIGHT };

    // Scroll down if selected item is below visible window
    if list.selected_index >= list.scroll_offset + effective {
        list.scroll_offset = list.selected_index + 1 - effective;
    }
    // Scroll up if selected item is above visible window
    if list.selected_index < list.scroll_offset {
        list.scroll_offset = list.selected_index;
    }
}
```

#### 3. Action Handlers (`actions.rs`)

##### `handle_scan_completed(state, versions) -> UpdateResult`

```rust
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
```

##### `handle_scan_failed(state, reason) -> UpdateResult`

```rust
pub fn handle_scan_failed(state: &mut AppState, reason: String) -> UpdateResult {
    let fv = &mut state.flutter_version_state;
    fv.version_list.loading = false;
    fv.version_list.error = Some(reason);
    UpdateResult::none()
}
```

##### `handle_switch(state) -> UpdateResult`

Initiates a version switch when user presses Enter on a selected version:

```rust
pub fn handle_switch(state: &mut AppState) -> UpdateResult {
    let fv = &state.flutter_version_state;

    // Must be focused on VersionList pane
    if fv.focused_pane != FlutterVersionPane::VersionList {
        return UpdateResult::none();
    }

    // Get selected version
    let selected = match fv.version_list.installed_versions.get(fv.version_list.selected_index) {
        Some(sdk) => sdk,
        None => return UpdateResult::none(),
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
```

##### `handle_switch_completed(state, version) -> UpdateResult`

Called after the async action writes `.fvmrc` and re-resolves the SDK:

```rust
pub fn handle_switch_completed(state: &mut AppState, version: String) -> UpdateResult {
    state.flutter_version_state.status_message = Some(format!("Switched to {version}"));

    // Refresh the SDK info pane with the new resolved SDK
    state.flutter_version_state.sdk_info.resolved_sdk = state.resolved_sdk.clone();

    // Re-scan to update is_active markers
    let active_sdk_root = state.resolved_sdk.as_ref().map(|sdk| sdk.root.clone());
    UpdateResult::action(UpdateAction::ScanInstalledSdks { active_sdk_root })
}
```

**Important**: The actual `state.resolved_sdk` update happens via the existing `Message::SdkResolved` message, which the action dispatcher sends before `FlutterVersionSwitchCompleted`. The handler here just updates the panel's display state.

##### `handle_switch_failed(state, reason) -> UpdateResult`

```rust
pub fn handle_switch_failed(state: &mut AppState, reason: String) -> UpdateResult {
    state.flutter_version_state.status_message = Some(format!("Switch failed: {reason}"));
    UpdateResult::none()
}
```

##### `handle_remove(state) -> UpdateResult`

Initiates removal of the selected version:

```rust
pub fn handle_remove(state: &mut AppState) -> UpdateResult {
    let fv = &state.flutter_version_state;

    if fv.focused_pane != FlutterVersionPane::VersionList {
        return UpdateResult::none();
    }

    let selected = match fv.version_list.installed_versions.get(fv.version_list.selected_index) {
        Some(sdk) => sdk,
        None => return UpdateResult::none(),
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
```

##### `handle_remove_completed(state, version) -> UpdateResult`

```rust
pub fn handle_remove_completed(state: &mut AppState, version: String) -> UpdateResult {
    state.flutter_version_state.status_message = Some(format!("Removed {version}"));

    // Re-scan cache
    let active_sdk_root = state.resolved_sdk.as_ref().map(|sdk| sdk.root.clone());
    UpdateResult::action(UpdateAction::ScanInstalledSdks { active_sdk_root })
}
```

##### `handle_remove_failed(state, reason) -> UpdateResult`

```rust
pub fn handle_remove_failed(state: &mut AppState, reason: String) -> UpdateResult {
    state.flutter_version_state.status_message = Some(format!("Remove failed: {reason}"));
    UpdateResult::none()
}
```

### Acceptance Criteria

1. `handle_show` opens the panel and triggers `ScanInstalledSdks` action
2. `handle_hide` closes the panel and returns to `UiMode::Normal`
3. `handle_escape` closes the panel (no sub-modals in Phase 2)
4. `handle_switch_pane` toggles between `SdkInfo` and `VersionList`
5. `handle_up` / `handle_down` navigate the version list with bounds checking
6. Scroll adjustment uses the `Cell<usize>` render-hint, not a hardcoded height
7. `handle_scan_completed` populates the version list and clears loading state
8. `handle_switch` guards: must be in VersionList pane, must have a selection, must not be active
9. `handle_switch` returns `UpdateAction::SwitchFlutterVersion` with correct data
10. `handle_remove` guards: must not remove the active version
11. `handle_switch_completed` updates status message, refreshes SDK info, triggers re-scan
12. All handler functions follow the `UpdateResult` return pattern
13. `cargo check --workspace` compiles
14. `cargo test --workspace` passes
15. `cargo clippy --workspace -- -D warnings` passes

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn panel_state_with_versions() -> AppState {
        let mut state = test_app_state(); // from existing test helpers
        state.resolved_sdk = Some(fake_flutter_sdk());
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

    // ── Navigation tests ──

    #[test]
    fn test_show_sets_ui_mode_and_triggers_scan() {
        let mut state = test_app_state();
        let result = handle_show(&mut state);
        assert_eq!(state.ui_mode, UiMode::FlutterVersion);
        assert!(state.flutter_version_state.visible);
        assert!(matches!(result.action, Some(UpdateAction::ScanInstalledSdks { .. })));
    }

    #[test]
    fn test_hide_returns_to_normal() {
        let mut state = panel_state_with_versions();
        handle_hide(&mut state);
        assert_eq!(state.ui_mode, UiMode::Normal);
        assert!(!state.flutter_version_state.visible);
    }

    #[test]
    fn test_switch_pane_toggles() {
        let mut state = panel_state_with_versions();
        assert_eq!(state.flutter_version_state.focused_pane, FlutterVersionPane::SdkInfo);
        handle_switch_pane(&mut state);
        assert_eq!(state.flutter_version_state.focused_pane, FlutterVersionPane::VersionList);
        handle_switch_pane(&mut state);
        assert_eq!(state.flutter_version_state.focused_pane, FlutterVersionPane::SdkInfo);
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
    fn test_up_down_ignored_in_sdk_info_pane() {
        let mut state = panel_state_with_versions();
        state.flutter_version_state.focused_pane = FlutterVersionPane::SdkInfo;
        state.flutter_version_state.version_list.selected_index = 1;
        handle_up(&mut state);
        assert_eq!(state.flutter_version_state.version_list.selected_index, 1); // unchanged
    }

    // ── Action tests ──

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
        assert_eq!(state.flutter_version_state.version_list.installed_versions.len(), 1);
    }

    #[test]
    fn test_switch_active_version_shows_already_active() {
        let mut state = panel_state_with_versions();
        state.flutter_version_state.focused_pane = FlutterVersionPane::VersionList;
        state.flutter_version_state.version_list.selected_index = 0; // active version
        let result = handle_switch(&mut state);
        assert!(result.action.is_none());
        assert_eq!(state.flutter_version_state.status_message.as_deref(), Some("Already active"));
    }

    #[test]
    fn test_switch_non_active_returns_action() {
        let mut state = panel_state_with_versions();
        state.flutter_version_state.focused_pane = FlutterVersionPane::VersionList;
        state.flutter_version_state.version_list.selected_index = 1; // non-active
        let result = handle_switch(&mut state);
        assert!(matches!(result.action, Some(UpdateAction::SwitchFlutterVersion { .. })));
    }

    #[test]
    fn test_remove_active_version_blocked() {
        let mut state = panel_state_with_versions();
        state.flutter_version_state.focused_pane = FlutterVersionPane::VersionList;
        state.flutter_version_state.version_list.selected_index = 0; // active
        let result = handle_remove(&mut state);
        assert!(result.action.is_none());
        assert!(state.flutter_version_state.status_message.as_deref()
            .unwrap().contains("Cannot remove"));
    }

    #[test]
    fn test_remove_non_active_returns_action() {
        let mut state = panel_state_with_versions();
        state.flutter_version_state.focused_pane = FlutterVersionPane::VersionList;
        state.flutter_version_state.version_list.selected_index = 1; // non-active
        let result = handle_remove(&mut state);
        assert!(matches!(result.action, Some(UpdateAction::RemoveFlutterVersion { .. })));
    }

    #[test]
    fn test_switch_ignored_in_sdk_info_pane() {
        let mut state = panel_state_with_versions();
        state.flutter_version_state.focused_pane = FlutterVersionPane::SdkInfo;
        let result = handle_switch(&mut state);
        assert!(result.action.is_none());
    }
}
```

### Notes

- **`handle_switch` does NOT write `.fvmrc` directly.** It returns an `UpdateAction::SwitchFlutterVersion` which the action dispatcher (Task 07) executes asynchronously. This keeps handlers synchronous and side-effect-free, following TEA principles.
- **Version switch flow**: `handle_switch` → `UpdateAction::SwitchFlutterVersion` → action dispatcher writes `.fvmrc`, calls `find_flutter_sdk()`, sends `Message::SdkResolved` + `Message::FlutterVersionSwitchCompleted` → `handle_switch_completed` refreshes panel display.
- **`handle_remove` similarly delegates** to `UpdateAction::RemoveFlutterVersion` for async directory deletion.
- **Scroll adjustment** follows the `Cell<usize>` render-hint pattern from CODE_STANDARDS.md Principle 3. The `DEFAULT_VISIBLE_HEIGHT` constant is only used as fallback before the first render.
- **All handlers accept `&mut AppState`** (not `&mut FlutterVersionState`) to access `project_path`, `settings`, and `resolved_sdk` when building `UpdateAction` variants.
