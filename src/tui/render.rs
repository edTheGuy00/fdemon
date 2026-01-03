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
    let areas = layout::create(area);

    // Header with optional session tabs
    let header = widgets::HeaderWithTabs::with_sessions(&state.session_manager);
    frame.render_widget(header, areas.header);

    // Log view (stateful for scroll tracking)
    let log_view = widgets::LogView::new(&state.logs);
    frame.render_stateful_widget(log_view, areas.logs, &mut state.log_view_state);

    // Status bar (use compact version for narrow terminals)
    if layout::use_compact_status(area) {
        frame.render_widget(widgets::StatusBarCompact::new(state), areas.status);
    } else {
        frame.render_widget(widgets::StatusBar::new(state), areas.status);
    }

    // Render modal overlays based on UI mode
    match state.ui_mode {
        UiMode::DeviceSelector | UiMode::Loading => {
            // Render device selector modal
            let selector = widgets::DeviceSelector::new(&state.device_selector);
            frame.render_widget(selector, area);
        }
        UiMode::ConfirmDialog => {
            // TODO: Render confirmation dialog
            // For now, the normal view is shown
        }
        UiMode::EmulatorSelector => {
            // TODO: Render emulator selector (Task 08)
            // For now, show device selector
            let selector = widgets::DeviceSelector::new(&state.device_selector);
            frame.render_widget(selector, area);
        }
        UiMode::Normal => {
            // No overlay
        }
    }
}
