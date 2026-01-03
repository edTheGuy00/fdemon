//! Main render/view function (View in TEA pattern)

use super::{layout, widgets};
use crate::app::state::{AppState, UiMode};
use ratatui::Frame;

/// Render the complete UI (View function in TEA)
///
/// This is a pure rendering function - it should not modify state
/// except for widget state that tracks rendering info (scroll position).
pub fn view(frame: &mut Frame, state: &mut AppState) {
    let area = frame.area();
    let session_count = state.session_manager.len();
    let areas = layout::create_with_sessions(area, session_count);

    // Main header with project name
    let header = widgets::MainHeader::new(state.project_name.as_deref());
    frame.render_widget(header, areas.header);

    // Tab subheader (only if multiple sessions)
    if let Some(tabs_area) = areas.tabs {
        let tabs = widgets::SessionTabs::new(&state.session_manager);
        frame.render_widget(tabs, tabs_area);
    }

    // Log view - use selected session's logs or global logs as fallback
    if let Some(handle) = state.session_manager.selected_mut() {
        let log_view = widgets::LogView::new(&handle.session.logs);
        frame.render_stateful_widget(log_view, areas.logs, &mut handle.session.log_view_state);
    } else {
        // Fallback to global logs when no session active
        let log_view = widgets::LogView::new(&state.logs);
        frame.render_stateful_widget(log_view, areas.logs, &mut state.log_view_state);
    }

    // Status bar - use session data if available, otherwise use global state
    if layout::use_compact_status(area) {
        frame.render_widget(widgets::StatusBarCompact::new(state), areas.status);
    } else {
        frame.render_widget(widgets::StatusBar::new(state), areas.status);
    }

    // Render modal overlays based on UI mode
    match state.ui_mode {
        UiMode::DeviceSelector | UiMode::Loading => {
            // Render device selector modal with session awareness
            let has_sessions = state.session_manager.has_running_sessions();
            let selector =
                widgets::DeviceSelector::with_session_state(&state.device_selector, has_sessions);
            frame.render_widget(selector, area);
        }
        UiMode::ConfirmDialog => {
            // Render confirmation dialog
            if let Some(ref dialog_state) = state.confirm_dialog_state {
                let dialog = widgets::ConfirmDialog::new(dialog_state);
                frame.render_widget(dialog, area);
            }
        }
        UiMode::EmulatorSelector => {
            // Render emulator selector with session awareness
            let has_sessions = state.session_manager.has_running_sessions();
            let selector =
                widgets::DeviceSelector::with_session_state(&state.device_selector, has_sessions);
            frame.render_widget(selector, area);
        }
        UiMode::Normal => {
            // No overlay
        }
    }
}
