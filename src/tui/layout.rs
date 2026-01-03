//! Screen layout definitions

use ratatui::layout::{Constraint, Layout, Rect};

/// Minimum terminal width for full status bar display
pub const MIN_FULL_STATUS_WIDTH: u16 = 60;

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
        Constraint::Length(2), // Status bar (1 for border + 1 for content)
    ])
    .split(area);

    ScreenAreas {
        header: chunks[0],
        logs: chunks[1],
        status: chunks[2],
    }
}

/// Check if we should use compact status bar
pub fn use_compact_status(area: Rect) -> bool {
    area.width < MIN_FULL_STATUS_WIDTH
}
