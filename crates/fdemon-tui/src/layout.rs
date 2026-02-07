//! Screen layout definitions for the TUI
//!
//! Provides responsive layout calculations for the main UI,
//! with dynamic header height based on session count.

use ratatui::layout::{Constraint, Layout, Rect};

/// Screen areas for the main layout
#[derive(Debug, Clone, Copy)]
pub struct ScreenAreas {
    /// Main header area (title + project name + keybindings)
    pub header: Rect,

    /// Main content area (log view with integrated metadata bars)
    pub logs: Rect,
}

/// Create the main screen layout with session count
///
/// # Arguments
/// * `area` - Total screen area
/// * `session_count` - Number of active sessions (determines header height for tabs)
pub fn create_with_sessions(area: Rect, session_count: usize) -> ScreenAreas {
    // Multi-session mode needs extra height for tabs:
    // - Single session: Length(3) = 1 inner row (title only)
    // - Multiple sessions: Length(5) = 3 inner rows (title + tabs + breathing room)
    let header_height = if session_count > 1 {
        5 // Top border + title row + tabs row + breathing row + bottom border
    } else {
        3 // Top border + title row + bottom border
    };

    // Layout: Header + Logs (remaining)
    // Both containers have their own borders, so no extra gap is needed
    let constraints = vec![
        Constraint::Length(header_height), // Header (glass container)
        Constraint::Min(3),                // Logs (glass container with top+bottom metadata bars)
    ];

    let chunks = Layout::vertical(constraints).split(area);

    ScreenAreas {
        header: chunks[0],
        logs: chunks[1],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_layout_single_session() {
        let area = Rect::new(0, 0, 80, 24);
        let layout = create_with_sessions(area, 1);

        // Header is always 3 rows (top border + content + bottom border)
        assert_eq!(layout.header.height, 3);
        // Log view gets remaining space after header (3)
        assert_eq!(layout.logs.height, 21); // 24 - 3 = 21
        assert_eq!(layout.logs.y, 3); // Starts after header (3)
    }

    #[test]
    fn test_create_layout_multiple_sessions() {
        let area = Rect::new(0, 0, 80, 24);
        let layout = create_with_sessions(area, 3);

        // Header is 5 rows for multiple sessions (includes tabs)
        assert_eq!(layout.header.height, 5);
        // Log view gets remaining space after header (5)
        assert_eq!(layout.logs.height, 19); // 24 - 5 = 19
        assert_eq!(layout.logs.y, 5); // Starts after header (5)
    }

    #[test]
    fn test_create_layout_no_sessions() {
        let area = Rect::new(0, 0, 80, 24);
        let layout = create_with_sessions(area, 0);

        assert_eq!(layout.header.height, 3);
        assert_eq!(layout.logs.height, 21);
    }

    #[test]
    fn test_layout_areas_contiguous() {
        let area = Rect::new(0, 0, 80, 24);

        // No sessions: header=3, logs=21
        let layout = create_with_sessions(area, 0);
        assert_eq!(layout.header.height + layout.logs.height, area.height);

        // Single session: header=3, logs=21
        let layout = create_with_sessions(area, 1);
        assert_eq!(layout.header.height + layout.logs.height, area.height);

        // Multiple sessions: header=5, logs=19
        let layout = create_with_sessions(area, 3);
        assert_eq!(layout.header.height + layout.logs.height, area.height);
    }

    #[test]
    fn test_create_with_sessions_returns_different_heights() {
        let area = Rect::new(0, 0, 80, 24);

        // Single session should use compact header (3 rows)
        let layout_single = create_with_sessions(area, 1);
        assert_eq!(layout_single.header.height, 3);

        // Multiple sessions should use expanded header (5 rows for tabs)
        let layout_multi = create_with_sessions(area, 2);
        assert_eq!(layout_multi.header.height, 5);

        // Verify inner height difference allows for tabs row
        // With 3 rows: 1 inner row (border takes 2)
        // With 5 rows: 3 inner rows (border takes 2)
        // The MainHeader widget needs inner.height >= 2 to show tabs
        assert!(layout_single.header.height < layout_multi.header.height);
    }
}
