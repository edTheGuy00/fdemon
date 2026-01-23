//! NewSessionDialog navigation handlers
//!
//! Handles pane/tab switching and field navigation in the NewSessionDialog.

use crate::app::handler::{UpdateAction, UpdateResult};
use crate::app::message::Message;
use crate::app::state::{AppState, UiMode};

/// Handle pane switch (Tab key)
pub fn handle_switch_pane(state: &mut AppState) -> UpdateResult {
    state.new_session_dialog_state.toggle_pane_focus();
    UpdateResult::none()
}

/// Handle tab switch (Connected/Bootable tabs)
pub fn handle_switch_tab(
    state: &mut AppState,
    tab: crate::tui::widgets::TargetTab,
) -> UpdateResult {
    // Check if we need to trigger discovery BEFORE switch_tab modifies state
    let needs_bootable_discovery = tab == crate::tui::widgets::TargetTab::Bootable
        && state
            .new_session_dialog_state
            .target_selector
            .ios_simulators
            .is_empty()
        && state
            .new_session_dialog_state
            .target_selector
            .android_avds
            .is_empty()
        && !state
            .new_session_dialog_state
            .target_selector
            .bootable_loading;

    state.new_session_dialog_state.target_selector.set_tab(tab);

    // Trigger bootable device discovery if switching to Bootable tab and not loaded
    if needs_bootable_discovery {
        state
            .new_session_dialog_state
            .target_selector
            .bootable_loading = true;
        return UpdateResult::action(UpdateAction::DiscoverBootableDevices);
    }
    UpdateResult::none()
}

/// Handle tab toggle (Alt+Tab)
pub fn handle_toggle_tab(state: &mut AppState) -> UpdateResult {
    let new_tab = state
        .new_session_dialog_state
        .target_selector
        .active_tab
        .toggle();
    handle_switch_tab(state, new_tab)
}

/// Handle field navigation down (Tab in right pane)
pub fn handle_field_next(state: &mut AppState) -> UpdateResult {
    use crate::app::new_session_dialog::DialogPane;
    if state.new_session_dialog_state.focused_pane == DialogPane::LaunchContext {
        let current = state.new_session_dialog_state.launch_context.focused_field;
        state.new_session_dialog_state.launch_context.focused_field = current.next();
    }
    UpdateResult::none()
}

/// Handle field navigation up (Shift+Tab in right pane)
pub fn handle_field_prev(state: &mut AppState) -> UpdateResult {
    use crate::app::new_session_dialog::DialogPane;
    if state.new_session_dialog_state.focused_pane == DialogPane::LaunchContext {
        let current = state.new_session_dialog_state.launch_context.focused_field;
        state.new_session_dialog_state.launch_context.focused_field = current.prev();
    }
    UpdateResult::none()
}

/// Handle field activation (Enter on a field)
pub fn handle_field_activate(
    state: &mut AppState,
    update_fn: fn(&mut AppState, Message) -> UpdateResult,
) -> UpdateResult {
    use crate::app::new_session_dialog::{DialogPane, FuzzyModalType, LaunchContextField};

    if state.new_session_dialog_state.focused_pane != DialogPane::LaunchContext {
        return UpdateResult::none();
    }

    let current_field = state.new_session_dialog_state.launch_context.focused_field;
    match current_field {
        LaunchContextField::Config => {
            // Open config fuzzy modal
            return update_fn(
                state,
                Message::NewSessionDialogOpenFuzzyModal {
                    modal_type: FuzzyModalType::Config,
                },
            );
        }

        LaunchContextField::Mode => {
            // Mode uses left/right arrows, Enter moves to next field
            let next = current_field.next();
            state.new_session_dialog_state.launch_context.focused_field = next;
        }

        LaunchContextField::Flavor => {
            // Check if flavor is editable based on selected config
            if !state
                .new_session_dialog_state
                .launch_context
                .is_flavor_editable()
            {
                // VSCode configs are read-only, skip to next field
                let next = current_field.next();
                state.new_session_dialog_state.launch_context.focused_field = next;
                return UpdateResult::none();
            }

            // Open flavor fuzzy modal
            return update_fn(
                state,
                Message::NewSessionDialogOpenFuzzyModal {
                    modal_type: FuzzyModalType::Flavor,
                },
            );
        }

        LaunchContextField::DartDefines => {
            // Check if dart defines are editable
            if !state
                .new_session_dialog_state
                .launch_context
                .are_dart_defines_editable()
            {
                // VSCode configs are read-only, skip to next field
                let next = current_field.next();
                state.new_session_dialog_state.launch_context.focused_field = next;
                return UpdateResult::none();
            }

            // Open dart defines modal
            return update_fn(state, Message::NewSessionDialogOpenDartDefinesModal);
        }

        LaunchContextField::Launch => {
            // Trigger launch
            return update_fn(state, Message::NewSessionDialogLaunch);
        }
    }

    UpdateResult::none()
}

/// Opens the new session dialog and triggers device discovery.
///
/// Loads launch configurations from the project path and initializes
/// the dialog state. If no configurations are found, defaults are used.
///
/// Uses cached devices if available (< 30s old) for instant display,
/// then triggers background refresh to keep data fresh.
pub fn handle_open_new_session_dialog(state: &mut AppState) -> UpdateResult {
    // Load configs with error handling
    let configs = crate::config::load_all_configs(&state.project_path);

    // Log warning if no configs found (not an error, just informational)
    if configs.configs.is_empty() {
        tracing::info!("No launch configurations found, using defaults");
    }

    // Show the dialog
    state.show_new_session_dialog(configs);

    // Check cache first - this is the ONLY place where cache is checked and populated
    // to avoid duplicate logic. The handler manages both cache checking and background refresh.

    // Check connected devices cache
    let has_connected_cache = if let Some(cached_devices) = state.get_cached_devices() {
        tracing::debug!(
            "Using cached devices ({} devices, age: {:?})",
            cached_devices.len(),
            state.devices_last_updated.map(|t| t.elapsed())
        );

        // Populate dialog with cached devices immediately
        state
            .new_session_dialog_state
            .target_selector
            .set_connected_devices(cached_devices.clone());
        true
    } else {
        false
    };

    // Check bootable devices cache (independent of connected devices cache)
    if let Some((simulators, avds)) = state.get_cached_bootable_devices() {
        tracing::debug!(
            "Using cached bootable devices ({} simulators, {} AVDs, age: {:?})",
            simulators.len(),
            avds.len(),
            state.bootable_last_updated.map(|t| t.elapsed())
        );

        state
            .new_session_dialog_state
            .target_selector
            .set_bootable_devices(simulators, avds);
    }

    // If we have connected device cache, trigger background refresh
    if has_connected_cache {
        return UpdateResult::action(UpdateAction::RefreshDevicesBackground);
    }

    // Cache miss or expired - show loading and discover
    tracing::debug!("Device cache miss, triggering discovery");
    state.new_session_dialog_state.target_selector.loading = true;
    UpdateResult::action(UpdateAction::DiscoverDevices)
}

/// Closes the new session dialog and returns to the appropriate UI mode.
///
/// If sessions are running, returns to Normal mode. Otherwise, remains
/// in Normal mode (as startup state).
pub fn handle_close_new_session_dialog(state: &mut AppState) -> UpdateResult {
    state.hide_new_session_dialog();

    // Return to appropriate UI mode based on session state
    if state.session_manager.has_running_sessions() {
        state.ui_mode = UiMode::Normal;
    } else {
        // No sessions, stay in startup mode
        state.ui_mode = UiMode::Normal;
    }

    UpdateResult::none()
}

/// Handles the Escape key in the new session dialog.
///
/// Priority order:
/// 1. Close fuzzy modal if open
/// 2. Close dart defines modal if open (saves changes)
/// 3. Close dialog if sessions exist
/// 4. Quit if no sessions (in Startup mode, nowhere else to go)
pub fn handle_new_session_dialog_escape(state: &mut AppState) -> UpdateResult {
    // Priority 1: Close fuzzy modal
    if state.new_session_dialog_state.is_fuzzy_modal_open() {
        state.new_session_dialog_state.fuzzy_modal = None;
        return UpdateResult::none();
    }

    // Priority 2: Close dart defines modal (with save)
    if state.new_session_dialog_state.is_dart_defines_modal_open() {
        state
            .new_session_dialog_state
            .close_dart_defines_modal_with_changes();
        return UpdateResult::none();
    }

    // Priority 3: Close dialog (only if sessions exist)
    if state.session_manager.has_running_sessions() {
        return UpdateResult::message(Message::CloseNewSessionDialog);
    }

    // Priority 4: No sessions in Startup mode - quit immediately
    // There's nowhere else to go, so Escape should exit the app
    UpdateResult::message(Message::Quit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::new_session_dialog::TargetTab;
    use crate::app::state::{AppState, UiMode};
    use crate::config::LoadedConfigs;
    use crate::tui::test_utils::test_device_full;
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    fn test_app_state() -> AppState {
        let mut state = AppState::with_settings(
            PathBuf::from("/test/project"),
            crate::config::Settings::default(),
        );
        state.project_name = Some("TestProject".to_string());
        state
    }

    #[test]
    fn test_open_dialog_uses_cached_devices() {
        let mut state = test_app_state();

        // Pre-populate cache
        let devices = vec![
            test_device_full("1", "iPhone 15", "ios", false),
            test_device_full("2", "Pixel 7", "android", false),
        ];
        state.device_cache = Some(devices.clone());
        state.devices_last_updated = Some(Instant::now());

        let result = handle_open_new_session_dialog(&mut state);

        // Should have devices immediately
        assert_eq!(
            state
                .new_session_dialog_state
                .target_selector
                .connected_devices
                .len(),
            2
        );

        // Should NOT show loading
        assert!(!state.new_session_dialog_state.target_selector.loading);

        // Should trigger background refresh
        assert!(matches!(
            result.action,
            Some(UpdateAction::RefreshDevicesBackground)
        ));
    }

    #[test]
    fn test_open_dialog_cache_miss_shows_loading() {
        let mut state = test_app_state();
        // No cache set

        let result = handle_open_new_session_dialog(&mut state);

        // Should show loading
        assert!(state.new_session_dialog_state.target_selector.loading);

        // Should trigger foreground discovery
        assert!(matches!(result.action, Some(UpdateAction::DiscoverDevices)));
    }

    #[test]
    fn test_open_dialog_expired_cache_shows_loading() {
        let mut state = test_app_state();

        // Set cache with old timestamp (> 30s ago)
        state.device_cache = Some(vec![test_device_full("1", "iPhone", "ios", false)]);
        state.devices_last_updated = Some(Instant::now() - Duration::from_secs(60));

        let result = handle_open_new_session_dialog(&mut state);

        // Cache expired - should show loading
        assert!(state.new_session_dialog_state.target_selector.loading);
        assert!(matches!(result.action, Some(UpdateAction::DiscoverDevices)));
    }

    #[test]
    fn test_open_dialog_fresh_cache_instant_display() {
        let mut state = test_app_state();

        // Set cache with recent timestamp (< 30s ago)
        let devices = vec![
            test_device_full("1", "iPhone", "ios", false),
            test_device_full("2", "Pixel", "android", false),
        ];
        state.device_cache = Some(devices.clone());
        state.devices_last_updated = Some(Instant::now() - Duration::from_secs(5));

        handle_open_new_session_dialog(&mut state);

        // Devices should be immediately available
        assert_eq!(
            state
                .new_session_dialog_state
                .target_selector
                .connected_devices
                .len(),
            2
        );
        assert!(!state.new_session_dialog_state.target_selector.loading);
    }

    #[test]
    fn test_open_dialog_loads_configs() {
        let mut state = test_app_state();

        handle_open_new_session_dialog(&mut state);

        // Dialog should be shown
        assert_eq!(state.ui_mode, UiMode::NewSessionDialog);
        // Launch context should be initialized (uses index 0 if configs exist, or None if empty)
        // Since we have no configs in test, this should be None
        assert!(state
            .new_session_dialog_state
            .launch_context
            .selected_config_index
            .is_none());
    }

    #[test]
    fn test_close_dialog_with_running_sessions() {
        let mut state = test_app_state();
        state.ui_mode = UiMode::NewSessionDialog;

        // Create a running session
        let device = test_device_full("1", "iPhone", "ios", false);
        let session_id = state.session_manager.create_session(&device).unwrap();
        state
            .session_manager
            .get_mut(session_id)
            .unwrap()
            .session
            .mark_started("app-1".to_string());

        handle_close_new_session_dialog(&mut state);

        assert_eq!(state.ui_mode, UiMode::Normal);
    }

    #[test]
    fn test_close_dialog_without_sessions() {
        let mut state = test_app_state();
        state.ui_mode = UiMode::NewSessionDialog;
        // No running sessions

        handle_close_new_session_dialog(&mut state);

        assert_eq!(state.ui_mode, UiMode::Normal);
    }

    #[test]
    fn test_escape_closes_fuzzy_modal() {
        let mut state = test_app_state();
        state.ui_mode = UiMode::NewSessionDialog;
        state.show_new_session_dialog(LoadedConfigs::default());
        state
            .new_session_dialog_state
            .open_flavor_modal(vec!["dev".to_string()]);

        let result = handle_new_session_dialog_escape(&mut state);

        assert!(!state.new_session_dialog_state.is_fuzzy_modal_open());
        assert!(result.action.is_none());
    }

    #[test]
    fn test_escape_closes_dart_defines_modal() {
        let mut state = test_app_state();
        state.ui_mode = UiMode::NewSessionDialog;
        state.show_new_session_dialog(LoadedConfigs::default());
        state.new_session_dialog_state.open_dart_defines_modal();

        let result = handle_new_session_dialog_escape(&mut state);

        assert!(!state.new_session_dialog_state.is_dart_defines_modal_open());
        assert!(result.action.is_none());
    }

    #[test]
    fn test_escape_closes_dialog_with_sessions() {
        let mut state = test_app_state();
        state.ui_mode = UiMode::NewSessionDialog;
        state.show_new_session_dialog(LoadedConfigs::default());

        // Create a running session
        let device = test_device_full("1", "iPhone", "ios", false);
        let session_id = state.session_manager.create_session(&device).unwrap();
        state
            .session_manager
            .get_mut(session_id)
            .unwrap()
            .session
            .mark_started("app-1".to_string());

        let result = handle_new_session_dialog_escape(&mut state);

        assert!(matches!(
            result.message,
            Some(Message::CloseNewSessionDialog)
        ));
    }

    #[test]
    fn test_escape_quits_without_sessions() {
        let mut state = test_app_state();
        state.ui_mode = UiMode::NewSessionDialog;
        state.show_new_session_dialog(LoadedConfigs::default());
        // No running sessions

        let result = handle_new_session_dialog_escape(&mut state);

        assert!(matches!(result.message, Some(Message::Quit)));
    }

    #[test]
    fn test_switch_pane() {
        let mut state = test_app_state();
        state.show_new_session_dialog(LoadedConfigs::default());

        let initial_pane = state.new_session_dialog_state.focused_pane;
        handle_switch_pane(&mut state);

        assert_ne!(state.new_session_dialog_state.focused_pane, initial_pane);
    }

    #[test]
    fn test_switch_tab_to_bootable_triggers_discovery() {
        let mut state = test_app_state();
        state.show_new_session_dialog(LoadedConfigs::default());
        state
            .new_session_dialog_state
            .target_selector
            .set_tab(TargetTab::Connected);

        // Simulate that initial discovery has completed (no devices found)
        state
            .new_session_dialog_state
            .target_selector
            .bootable_loading = false;

        let result = handle_switch_tab(&mut state, TargetTab::Bootable);

        assert_eq!(
            state.new_session_dialog_state.target_selector.active_tab,
            TargetTab::Bootable
        );
        assert!(
            state
                .new_session_dialog_state
                .target_selector
                .bootable_loading
        );
        assert!(matches!(
            result.action,
            Some(UpdateAction::DiscoverBootableDevices)
        ));
    }

    #[test]
    fn test_switch_tab_to_bootable_already_loaded_no_discovery() {
        use crate::daemon::{IosSimulator, SimulatorState};

        let mut state = test_app_state();
        state.show_new_session_dialog(LoadedConfigs::default());

        // Pre-populate bootable devices
        state
            .new_session_dialog_state
            .target_selector
            .set_bootable_devices(
                vec![IosSimulator {
                    udid: "123".to_string(),
                    name: "iPhone 15".to_string(),
                    runtime: "iOS 17.2".to_string(),
                    state: SimulatorState::Shutdown,
                    device_type: "iPhone 15".to_string(),
                }],
                vec![],
            );

        let result = handle_switch_tab(&mut state, TargetTab::Bootable);

        // Should switch tab but NOT trigger discovery
        assert_eq!(
            state.new_session_dialog_state.target_selector.active_tab,
            TargetTab::Bootable
        );
        assert!(
            !state
                .new_session_dialog_state
                .target_selector
                .bootable_loading
        );
        assert!(result.action.is_none());
    }
}
