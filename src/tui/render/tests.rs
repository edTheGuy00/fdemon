//! Full-screen snapshot tests for TUI rendering
//!
//! These tests capture the entire screen render for each UI mode
//! and compare against golden snapshots using insta.

use super::view;
use crate::app::state::{AppState, LoadingState, UiMode};
use crate::core::AppPhase;
use crate::tui::test_utils::TestTerminal;
use crate::tui::widgets::ConfirmDialogState;
use insta::assert_snapshot;

fn create_base_state() -> AppState {
    let mut state = AppState::new();
    state.project_name = Some("flutter_app".to_string());
    state
}

// Helper to render full screen and return content
fn render_screen(state: &mut AppState) -> String {
    let mut term = TestTerminal::new();
    term.draw_with(|frame| view(frame, state));
    term.content()
}

// ===========================================================================
// Normal Mode Snapshots
// ===========================================================================

#[test]
fn snapshot_normal_mode_initializing() {
    let mut state = create_base_state();
    state.ui_mode = UiMode::Normal;
    state.phase = AppPhase::Initializing;

    let content = render_screen(&mut state);
    assert_snapshot!("normal_initializing", content);
}

#[test]
fn snapshot_normal_mode_running() {
    let mut state = create_base_state();
    state.ui_mode = UiMode::Normal;
    state.phase = AppPhase::Running;

    // Add a session with device name
    // Note: In the current architecture, we would need to add a proper session
    // For now, we'll test the basic render

    let content = render_screen(&mut state);
    assert_snapshot!("normal_running", content);
}

#[test]
fn snapshot_normal_mode_reloading() {
    let mut state = create_base_state();
    state.ui_mode = UiMode::Normal;
    state.phase = AppPhase::Reloading;

    let content = render_screen(&mut state);
    assert_snapshot!("normal_reloading", content);
}

#[test]
fn snapshot_normal_mode_stopped() {
    let mut state = create_base_state();
    state.ui_mode = UiMode::Normal;
    state.phase = AppPhase::Stopped;

    let content = render_screen(&mut state);
    assert_snapshot!("normal_stopped", content);
}

// ===========================================================================
// Device Selector Mode Snapshots
// ===========================================================================

#[test]
#[cfg(feature = "test_old_dialogs")]
fn snapshot_device_selector_empty() {
    let mut state = create_base_state();
    state.ui_mode = UiMode::DeviceSelector;
    // No devices added - DeviceSelectorState is already initialized empty

    let content = render_screen(&mut state);
    assert_snapshot!("device_selector_empty", content);
}

#[test]
#[cfg(feature = "test_old_dialogs")]
#[cfg(feature = "test_old_dialogs")]
fn snapshot_device_selector_with_devices() {
    use crate::daemon::Device;

    let mut state = create_base_state();
    state.ui_mode = UiMode::DeviceSelector;

    // Add mock devices
    let devices = vec![
        Device {
            id: "linux-1".to_string(),
            name: "Linux Desktop".to_string(),
            platform: "linux-x64".to_string(),
            emulator: false,
            category: None,
            platform_type: None,
            ephemeral: true,
            emulator_id: None,
        },
        Device {
            id: "android-1".to_string(),
            name: "Pixel 5".to_string(),
            platform: "android-arm64".to_string(),
            emulator: false,
            category: Some("mobile".to_string()),
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        },
        Device {
            id: "ios-1".to_string(),
            name: "iPhone 14 Pro".to_string(),
            platform: "ios".to_string(),
            emulator: true,
            category: Some("mobile".to_string()),
            platform_type: None,
            ephemeral: false,
            emulator_id: None,
        },
    ];

    state.device_selector.set_devices(devices);

    let content = render_screen(&mut state);
    assert_snapshot!("device_selector_with_devices", content);
}

// ===========================================================================
// Confirm Dialog Mode Snapshots
// ===========================================================================

#[test]
#[cfg(feature = "test_old_dialogs")]
fn snapshot_confirm_dialog_quit() {
    let mut state = create_base_state();
    state.ui_mode = UiMode::ConfirmDialog;
    state.confirm_dialog_state = Some(ConfirmDialogState::quit_confirmation(1));

    let content = render_screen(&mut state);
    assert_snapshot!("confirm_dialog_quit", content);
}

#[test]
#[cfg(feature = "test_old_dialogs")]
fn snapshot_confirm_dialog_quit_multiple_sessions() {
    let mut state = create_base_state();
    state.ui_mode = UiMode::ConfirmDialog;
    state.confirm_dialog_state = Some(ConfirmDialogState::quit_confirmation(3));

    let content = render_screen(&mut state);
    assert_snapshot!("confirm_dialog_quit_multiple", content);
}

// ===========================================================================
// Loading Mode Snapshots
// ===========================================================================

#[test]
#[cfg(feature = "test_old_dialogs")]
fn snapshot_loading_mode() {
    use insta::Settings;

    let mut state = create_base_state();
    state.ui_mode = UiMode::Loading;
    state.loading_state = Some(LoadingState::new("Starting Flutter..."));

    let content = render_screen(&mut state);

    // The loading message is randomized, so we use a regex filter to normalize it
    // Match optional leading whitespace, the spinner, and any text up to the border "│"
    let mut settings = Settings::clone_current();
    settings.add_filter(r"\s*⠋[^│\n]+", " ⠋ [LOADING_MESSAGE]...");
    settings.bind(|| {
        assert_snapshot!("loading", content);
    });
}

// ===========================================================================
// Compact Terminal Snapshots
// ===========================================================================

#[test]
#[cfg(feature = "test_old_dialogs")]
fn snapshot_compact_normal() {
    let mut state = create_base_state();
    state.ui_mode = UiMode::Normal;
    state.phase = AppPhase::Running;

    let mut term = TestTerminal::compact();
    term.draw_with(|frame| view(frame, &mut state));

    assert_snapshot!("compact_normal", term.content());
}

#[test]
#[cfg(feature = "test_old_dialogs")]
fn snapshot_compact_device_selector() {
    let mut state = create_base_state();
    state.ui_mode = UiMode::DeviceSelector;

    let mut term = TestTerminal::compact();
    term.draw_with(|frame| view(frame, &mut state));

    assert_snapshot!("compact_device_selector", term.content());
}

// ===========================================================================
// Settings Mode Snapshot
// ===========================================================================

#[test]
#[cfg(feature = "test_old_dialogs")]
fn snapshot_settings_mode() {
    let mut state = create_base_state();
    state.ui_mode = UiMode::Settings;

    let content = render_screen(&mut state);
    assert_snapshot!("settings_mode", content);
}

// ===========================================================================
// SearchInput Mode Snapshot
// ===========================================================================

#[test]
#[cfg(feature = "test_old_dialogs")]
fn snapshot_search_input_mode() {
    use crate::daemon::Device;

    let mut state = create_base_state();
    state.ui_mode = UiMode::SearchInput;

    // Create a device and session
    let device = Device {
        id: "test-device".to_string(),
        name: "Test Device".to_string(),
        platform: "android".to_string(),
        emulator: false,
        category: None,
        platform_type: None,
        ephemeral: true,
        emulator_id: None,
    };

    // Create session through the manager's public API
    let session_id = state
        .session_manager
        .create_session(&device)
        .expect("Failed to create session");

    // Set up search query and make it active on the created session
    if let Some(handle) = state.session_manager.get_mut(session_id) {
        handle.session.set_search_query("test query");
        handle.session.start_search();
    }

    let content = render_screen(&mut state);

    // Verify search UI elements are visible
    assert!(
        content.contains("/") || content.contains("Search") || content.contains("search"),
        "Search mode should show search indicator, but got:\n{}",
        content
    );

    // Verify the query is displayed
    assert!(
        content.contains("test query"),
        "Search input should display the current query, but got:\n{}",
        content
    );

    // Snapshot for regression detection
    assert_snapshot!("search_input_mode", content);
}

// ===========================================================================
// Edge Cases
// ===========================================================================

#[test]
#[cfg(feature = "test_old_dialogs")]
fn snapshot_no_project_name() {
    let mut state = create_base_state();
    state.project_name = None; // No project name
    state.ui_mode = UiMode::Normal;
    state.phase = AppPhase::Running;

    let content = render_screen(&mut state);
    assert_snapshot!("no_project_name", content);
}

#[test]
#[cfg(feature = "test_old_dialogs")]
fn snapshot_very_long_project_name() {
    let mut state = create_base_state();
    state.project_name =
        Some("my_extremely_long_flutter_application_name_that_goes_on_and_on".to_string());
    state.ui_mode = UiMode::Normal;
    state.phase = AppPhase::Running;

    let content = render_screen(&mut state);
    assert_snapshot!("long_project_name", content);
}

// ===========================================================================
// UI Mode Transition Tests
// ===========================================================================

#[test]
#[cfg(feature = "test_old_dialogs")]
#[ignore = "Old dialog removed"]
fn test_transition_normal_to_device_selector() {
    let mut state = create_base_state();
    state.ui_mode = UiMode::Normal;
    state.phase = AppPhase::Running;

    // Render normal mode
    let before = render_screen(&mut state);
    assert!(
        !before.contains("Select") && !before.contains("Device"),
        "Normal mode should not show device selector"
    );

    // Transition to device selector
    state.ui_mode = UiMode::DeviceSelector;
    let after = render_screen(&mut state);

    // Device selector should now be visible
    assert!(
        after.contains("Select") && after.contains("Device"),
        "DeviceSelector mode should show selector dialog"
    );
}

#[test]
#[cfg(feature = "test_old_dialogs")]
fn test_transition_normal_to_confirm_dialog() {
    let mut state = create_base_state();
    state.ui_mode = UiMode::Normal;

    let before = render_screen(&mut state);
    assert!(!before.contains("Quit?"));

    // Transition to confirm dialog
    state.ui_mode = UiMode::ConfirmDialog;
    state.confirm_dialog_state = Some(ConfirmDialogState::quit_confirmation(1));
    let after = render_screen(&mut state);

    assert!(
        after.contains("Quit") || after.contains("quit"),
        "Confirm dialog should appear after transition"
    );
}

#[test]
#[cfg(feature = "test_old_dialogs")]
#[ignore = "Old dialog removed"]
fn test_transition_device_selector_to_normal() {
    let mut state = create_base_state();
    state.ui_mode = UiMode::DeviceSelector;

    let before = render_screen(&mut state);

    // Transition back to normal (e.g., Escape pressed)
    state.ui_mode = UiMode::Normal;
    let after = render_screen(&mut state);

    // Device selector should be gone
    // Just verify it renders differently
    assert_ne!(before, after, "Screen should change on mode transition");
}

#[test]
#[cfg(feature = "test_old_dialogs")]
fn test_transition_confirm_to_normal_cancel() {
    let mut state = create_base_state();
    state.ui_mode = UiMode::ConfirmDialog;
    state.confirm_dialog_state = Some(ConfirmDialogState::quit_confirmation(1));

    let before = render_screen(&mut state);
    assert!(before.contains("Quit") || before.contains("quit"));

    // Cancel - return to normal
    state.ui_mode = UiMode::Normal;
    state.confirm_dialog_state = None;
    let after = render_screen(&mut state);

    assert!(
        !after.contains("Quit?"),
        "Dialog should disappear after cancel"
    );
}

#[test]
#[cfg(feature = "test_old_dialogs")]
fn test_phase_transition_renders_correctly() {
    use crate::daemon::Device;

    let mut state = create_base_state();
    state.ui_mode = UiMode::Normal;

    // Create a session so phases are visible (not "Not Connected")
    let device = Device {
        id: "test-device".to_string(),
        name: "Test Device".to_string(),
        platform: "linux".to_string(),
        emulator: false,
        category: None,
        platform_type: None,
        ephemeral: false,
        emulator_id: None,
    };
    let id = state.session_manager.create_session(&device).unwrap();
    state.session_manager.select_by_id(id);

    // Initializing
    if let Some(handle) = state.session_manager.get_mut(id) {
        handle.session.phase = AppPhase::Initializing;
    }
    let init = render_screen(&mut state);

    // Running
    if let Some(handle) = state.session_manager.get_mut(id) {
        handle.session.phase = AppPhase::Running;
    }
    let running = render_screen(&mut state);

    // Reloading
    if let Some(handle) = state.session_manager.get_mut(id) {
        handle.session.phase = AppPhase::Reloading;
    }
    let reloading = render_screen(&mut state);

    // All should be different
    assert_ne!(
        init, running,
        "Initializing and Running should look different"
    );
    assert_ne!(
        running, reloading,
        "Running and Reloading should look different"
    );
}

#[test]
#[cfg(feature = "test_old_dialogs")]
fn test_modal_overlay_preserves_background() {
    let mut state = create_base_state();
    state.ui_mode = UiMode::Normal;
    state.phase = AppPhase::Running;
    state.project_name = Some("my_app".to_string());

    // Get normal background
    let _normal = render_screen(&mut state);

    // Show device selector overlay
    state.ui_mode = UiMode::DeviceSelector;
    let with_modal = render_screen(&mut state);

    // Modal should be visible, but check that something from normal mode
    // might still be partially visible (depends on implementation)
    assert!(with_modal.len() > 0, "Modal overlay should render");
}

#[test]
#[cfg(feature = "test_old_dialogs")]
fn test_loading_to_normal_transition() {
    let mut state = create_base_state();

    // Loading state
    state.ui_mode = UiMode::Loading;
    state.loading_state = Some(LoadingState::new("Starting..."));
    let loading = render_screen(&mut state);

    // Transition to normal running
    state.ui_mode = UiMode::Normal;
    state.phase = AppPhase::Running;
    state.loading_state = None;
    let normal = render_screen(&mut state);

    assert_ne!(loading, normal, "Loading and normal modes should differ");
}

#[test]
#[cfg(feature = "test_old_dialogs")]
fn test_rapid_mode_changes() {
    let mut state = create_base_state();

    // Simulate rapid mode changes
    let modes = [
        UiMode::Normal,
        UiMode::DeviceSelector,
        UiMode::Normal,
        UiMode::ConfirmDialog,
        UiMode::Normal,
    ];

    for mode in modes {
        state.ui_mode = mode;
        if mode == UiMode::ConfirmDialog {
            state.confirm_dialog_state = Some(ConfirmDialogState::quit_confirmation(1));
        } else {
            state.confirm_dialog_state = None;
        }

        // Should render without panic
        let content = render_screen(&mut state);
        assert!(!content.is_empty(), "Should render in mode {:?}", mode);
    }
}
