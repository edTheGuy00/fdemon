//! Screen layout definitions

use ratatui::layout::{Constraint, Layout, Rect};

/// Screen areas for the main layout
pub struct ScreenAreas {
    pub header: Rect,
    pub logs: Rect,
    pub status: Rect,
}

/// Create the main screen layout
pub fn create(area: Rect) -> ScreenAreas {
    let chunks = Layout::vertical([
        Constraint::Length(3), // Header
        Constraint::Min(5),    // Logs
        Constraint::Length(1), // Status bar
    ])
    .split(area);

    ScreenAreas {
        header: chunks[0],
        logs: chunks[1],
        status: chunks[2],
    }
}
