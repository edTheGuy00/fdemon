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

    /// Main content area (log view with integrated metadata bars)
    pub logs: Rect,
}

/// Layout mode based on terminal size
#[allow(dead_code)]
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
    #[allow(dead_code)]
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
#[allow(dead_code)]
pub fn create(area: Rect) -> ScreenAreas {
    create_with_sessions(area, 0)
}

/// Create the main screen layout with session count
///
/// # Arguments
/// * `area` - Total screen area
/// * `session_count` - Number of active sessions (determines header height for tabs)
pub fn create_with_sessions(area: Rect, session_count: usize) -> ScreenAreas {
    let _ = session_count; // Used by header widget to decide whether to show tabs

    // New layout: Header (3 rows) + Gap (1 row) + Logs (remaining)
    // The gap provides visual breathing room between header and log panel
    let constraints = vec![
        Constraint::Length(3), // Header (glass container)
        Constraint::Length(1), // Gap (breathing room, DEEPEST_BG shows through)
        Constraint::Min(3),    // Logs (glass container with top+bottom metadata bars)
    ];

    let chunks = Layout::vertical(constraints).split(area);

    ScreenAreas {
        header: chunks[0],
        logs: chunks[2], // Skip the gap at chunks[1]
    }
}

/// Whether to use compact mode for the integrated status footer
///
/// This controls the formatting of the status info displayed in the log view's
/// bottom metadata bar. Currently unused but available for future enhancement.
#[allow(dead_code)]
pub fn use_compact_footer(area: Rect) -> bool {
    area.width < MIN_FULL_STATUS_WIDTH
}

/// Check if compact header should be used
#[allow(dead_code)]
pub fn use_compact_header(area: Rect) -> bool {
    area.width < MIN_FULL_STATUS_WIDTH
}

/// Get header height (constant regardless of session count)
#[allow(dead_code)]
pub fn header_height(_session_count: usize) -> u16 {
    3 // Top border + content row + bottom border
}

/// Get timestamp format for log entries based on width
#[allow(dead_code)]
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
#[allow(dead_code)]
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

        // Header is always 3 rows (top border + content + bottom border)
        assert_eq!(layout.header.height, 3);
        // Log view gets remaining space after header (3) + gap (1)
        assert_eq!(layout.logs.height, 20); // 24 - 3 - 1 = 20
        assert_eq!(layout.logs.y, 4); // Starts after header (3) + gap (1)
    }

    #[test]
    fn test_create_layout_multiple_sessions() {
        let area = Rect::new(0, 0, 80, 24);
        let layout = create_with_sessions(area, 3);

        // Header is 3 rows regardless of session count
        assert_eq!(layout.header.height, 3);
        assert_eq!(layout.logs.height, 20);
    }

    #[test]
    fn test_create_layout_no_sessions() {
        let area = Rect::new(0, 0, 80, 24);
        let layout = create_with_sessions(area, 0);

        assert_eq!(layout.header.height, 3);
        assert_eq!(layout.logs.height, 20);
    }

    #[test]
    fn test_timestamp_format() {
        assert_eq!(timestamp_format(Rect::new(0, 0, 50, 24)), "%H:%M");
        assert_eq!(timestamp_format(Rect::new(0, 0, 70, 24)), "%H:%M:%S");
        assert_eq!(timestamp_format(Rect::new(0, 0, 90, 24)), "%H:%M:%S");
        assert_eq!(timestamp_format(Rect::new(0, 0, 130, 24)), "%H:%M:%S%.3f");
    }

    #[test]
    fn test_use_compact_footer() {
        assert!(use_compact_footer(Rect::new(0, 0, 40, 24)));
        assert!(use_compact_footer(Rect::new(0, 0, 59, 24)));
        assert!(!use_compact_footer(Rect::new(0, 0, 60, 24)));
        assert!(!use_compact_footer(Rect::new(0, 0, 100, 24)));
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
    fn test_layout_areas_with_gap() {
        let area = Rect::new(0, 0, 80, 24);

        // No sessions
        let layout = create_with_sessions(area, 0);
        // Header (3) + gap (1) + logs (20) = 24
        assert_eq!(layout.header.height + 1 + layout.logs.height, area.height);

        // Single session
        let layout = create_with_sessions(area, 1);
        assert_eq!(layout.header.height + 1 + layout.logs.height, area.height);

        // Multiple sessions
        let layout = create_with_sessions(area, 3);
        assert_eq!(layout.header.height + 1 + layout.logs.height, area.height);
    }
}
