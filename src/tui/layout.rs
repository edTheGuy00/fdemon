//! Screen layout definitions for the TUI
//!
//! Provides responsive layout calculations for the main UI,
//! with dynamic header height based on session count.

use ratatui::layout::{Constraint, Layout, Rect};

/// Minimum terminal width for full status bar display
pub const MIN_FULL_STATUS_WIDTH: u16 = 60;

/// Screen areas for the main layout
#[derive(Debug, Clone, Copy)]
pub struct ScreenAreas {
    /// Main header area (title + project name + keybindings)
    pub header: Rect,

    /// Tab subheader area (only when multiple sessions)
    pub tabs: Option<Rect>,

    /// Main content area (log view)
    pub logs: Rect,

    /// Status bar area
    pub status: Rect,
}

/// Layout mode based on terminal size
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutMode {
    /// Very narrow terminal (< 60 cols)
    Compact,
    /// Standard terminal (60-80 cols)
    Standard,
    /// Comfortable width (80-120 cols)
    Comfortable,
    /// Wide terminal (> 120 cols)
    Wide,
}

impl LayoutMode {
    /// Determine layout mode from terminal width
    pub fn from_width(width: u16) -> Self {
        match width {
            0..=59 => LayoutMode::Compact,
            60..=79 => LayoutMode::Standard,
            80..=119 => LayoutMode::Comfortable,
            _ => LayoutMode::Wide,
        }
    }
}

/// Create the main screen layout
///
/// # Arguments
/// * `area` - Total screen area
/// * `session_count` - Number of active sessions (determines if tabs are shown)
pub fn create(area: Rect) -> ScreenAreas {
    create_with_sessions(area, 0)
}

/// Create the main screen layout with session count
///
/// # Arguments
/// * `area` - Total screen area
/// * `session_count` - Number of active sessions (determines header height for tabs)
pub fn create_with_sessions(area: Rect, session_count: usize) -> ScreenAreas {
    // Header: 3 rows (top border + 1 content row + bottom border)
    // Tabs render in the single content row when sessions exist
    let header_height = 3;
    let status_height = 2; // 1 for border + 1 for content
    let _ = session_count; // Used by header widget to decide whether to show tabs

    let constraints = vec![
        Constraint::Length(header_height),
        Constraint::Min(3), // Content area
        Constraint::Length(status_height),
    ];

    let chunks = Layout::vertical(constraints).split(area);

    ScreenAreas {
        header: chunks[0],
        tabs: None, // Tabs are now rendered inside the header
        logs: chunks[1],
        status: chunks[2],
    }
}

/// Check if we should use compact status bar
pub fn use_compact_status(area: Rect) -> bool {
    area.width < MIN_FULL_STATUS_WIDTH
}

/// Check if compact header should be used
pub fn use_compact_header(area: Rect) -> bool {
    area.width < MIN_FULL_STATUS_WIDTH
}

/// Get header height (constant regardless of session count)
pub fn header_height(_session_count: usize) -> u16 {
    3 // Top border + content row + bottom border
}

/// Get timestamp format for log entries based on width
pub fn timestamp_format(area: Rect) -> &'static str {
    let mode = LayoutMode::from_width(area.width);

    match mode {
        LayoutMode::Compact => "%H:%M",        // 12:34
        LayoutMode::Standard => "%H:%M:%S",    // 12:34:56
        LayoutMode::Comfortable => "%H:%M:%S", // 12:34:56
        LayoutMode::Wide => "%H:%M:%S%.3f",    // 12:34:56.789
    }
}

/// Get maximum tab count that fits in the header
pub fn max_visible_tabs(area: Rect) -> usize {
    let mode = LayoutMode::from_width(area.width);

    // Each tab is approximately 15-20 chars
    let tab_width = match mode {
        LayoutMode::Compact => 10,
        LayoutMode::Standard => 14,
        LayoutMode::Comfortable => 16,
        LayoutMode::Wide => 20,
    };

    // Most of the width is available for tabs in subheader
    (area.width / tab_width).max(1) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout_mode_from_width() {
        assert_eq!(LayoutMode::from_width(40), LayoutMode::Compact);
        assert_eq!(LayoutMode::from_width(59), LayoutMode::Compact);
        assert_eq!(LayoutMode::from_width(60), LayoutMode::Standard);
        assert_eq!(LayoutMode::from_width(79), LayoutMode::Standard);
        assert_eq!(LayoutMode::from_width(80), LayoutMode::Comfortable);
        assert_eq!(LayoutMode::from_width(119), LayoutMode::Comfortable);
        assert_eq!(LayoutMode::from_width(120), LayoutMode::Wide);
        assert_eq!(LayoutMode::from_width(200), LayoutMode::Wide);
    }

    #[test]
    fn test_create_layout_single_session() {
        let area = Rect::new(0, 0, 80, 24);
        let layout = create_with_sessions(area, 1);

        // Tabs are rendered inside the header, not as a separate area
        assert!(layout.tabs.is_none());
        // Header is always 3 rows (top border + content + bottom border)
        assert_eq!(layout.header.height, 3);
        assert!(layout.logs.height > 0);
        assert!(layout.status.height > 0);
    }

    #[test]
    fn test_create_layout_multiple_sessions() {
        let area = Rect::new(0, 0, 80, 24);
        let layout = create_with_sessions(area, 3);

        // Tabs are rendered inside header
        assert!(layout.tabs.is_none());
        assert_eq!(layout.header.height, 3);
    }

    #[test]
    fn test_create_layout_no_sessions() {
        let area = Rect::new(0, 0, 80, 24);
        let layout = create_with_sessions(area, 0);

        assert!(layout.tabs.is_none());
        assert_eq!(layout.header.height, 3);
    }

    #[test]
    fn test_timestamp_format() {
        assert_eq!(timestamp_format(Rect::new(0, 0, 50, 24)), "%H:%M");
        assert_eq!(timestamp_format(Rect::new(0, 0, 70, 24)), "%H:%M:%S");
        assert_eq!(timestamp_format(Rect::new(0, 0, 90, 24)), "%H:%M:%S");
        assert_eq!(timestamp_format(Rect::new(0, 0, 130, 24)), "%H:%M:%S%.3f");
    }

    #[test]
    fn test_use_compact_status() {
        assert!(use_compact_status(Rect::new(0, 0, 40, 24)));
        assert!(use_compact_status(Rect::new(0, 0, 59, 24)));
        assert!(!use_compact_status(Rect::new(0, 0, 60, 24)));
        assert!(!use_compact_status(Rect::new(0, 0, 100, 24)));
    }

    #[test]
    fn test_use_compact_header() {
        assert!(use_compact_header(Rect::new(0, 0, 40, 24)));
        assert!(!use_compact_header(Rect::new(0, 0, 80, 24)));
    }

    #[test]
    fn test_header_height() {
        // Header is always 3 rows regardless of session count
        assert_eq!(header_height(0), 3);
        assert_eq!(header_height(1), 3);
        assert_eq!(header_height(2), 3);
        assert_eq!(header_height(5), 3);
    }

    #[test]
    fn test_max_visible_tabs() {
        // Wide terminal should fit more tabs
        assert!(max_visible_tabs(Rect::new(0, 0, 120, 24)) >= 6);
        // Narrow terminal should fit fewer
        assert!(max_visible_tabs(Rect::new(0, 0, 40, 24)) >= 1);
    }

    #[test]
    fn test_layout_areas_sum_to_total() {
        let area = Rect::new(0, 0, 80, 24);

        // No sessions
        let layout = create_with_sessions(area, 0);
        let total = layout.header.height + layout.logs.height + layout.status.height;
        assert_eq!(total, area.height);

        // Single session (tabs are inside header now)
        let layout = create_with_sessions(area, 1);
        let total = layout.header.height + layout.logs.height + layout.status.height;
        assert_eq!(total, area.height);

        // Multiple sessions (tabs are inside header now)
        let layout = create_with_sessions(area, 3);
        let total = layout.header.height + layout.logs.height + layout.status.height;
        assert_eq!(total, area.height);
    }
}
