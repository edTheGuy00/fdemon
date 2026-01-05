//! Stack trace styling constants for log view rendering

use ratatui::style::{Color, Modifier, Style};

/// Frame number (#0, #1, etc.)
pub const FRAME_NUMBER: Style = Style::new().fg(Color::DarkGray);

/// Function name for project frames
pub const FUNCTION_PROJECT: Style = Style::new().fg(Color::White);

/// Function name for package frames
pub const FUNCTION_PACKAGE: Style = Style::new().fg(Color::DarkGray);

/// File path for project frames (clickable in Phase 3)
pub const FILE_PROJECT: Style = Style::new()
    .fg(Color::Blue)
    .add_modifier(Modifier::UNDERLINED);

/// File path for package frames
pub const FILE_PACKAGE: Style = Style::new().fg(Color::DarkGray);

/// Line/column numbers for project frames
pub const LOCATION_PROJECT: Style = Style::new().fg(Color::Cyan);

/// Line/column numbers for package frames
pub const LOCATION_PACKAGE: Style = Style::new().fg(Color::DarkGray);

/// Async suspension marker
pub const ASYNC_GAP: Style = Style::new()
    .fg(Color::DarkGray)
    .add_modifier(Modifier::ITALIC);

/// Punctuation (parentheses, colons)
pub const PUNCTUATION: Style = Style::new().fg(Color::DarkGray);

/// Indentation for stack frames
pub const INDENT: &str = "    ";
