//! Main render/view function (View in TEA pattern)

use super::{layout, widgets};
use crate::app::state::AppState;
use ratatui::Frame;

/// Render the complete UI (View function in TEA)
///
/// This is a pure rendering function - it should not modify state
/// except for widget state that tracks rendering info (scroll position).
pub fn view(frame: &mut Frame, state: &mut AppState) {
    let area = frame.area();
    let areas = layout::create(area);

    // Header
    frame.render_widget(widgets::Header::new(), areas.header);

    // Log view (stateful for scroll tracking)
    let log_view = widgets::LogView::new(&state.logs);
    frame.render_stateful_widget(log_view, areas.logs, &mut state.log_view_state);

    // Status bar (use compact version for narrow terminals)
    if layout::use_compact_status(area) {
        frame.render_widget(widgets::StatusBarCompact::new(state), areas.status);
    } else {
        frame.render_widget(widgets::StatusBar::new(state), areas.status);
    }
}
