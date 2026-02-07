//! Stack trace styling constants for log view rendering

use ratatui::style::{Modifier, Style};

use crate::theme::palette;

/// Frame number (#0, #1, etc.)
pub const FRAME_NUMBER: Style = Style::new().fg(palette::STACK_FRAME_NUMBER);

/// Function name for project frames
pub const FUNCTION_PROJECT: Style = Style::new().fg(palette::STACK_FUNCTION_PROJECT);

/// Function name for package frames
pub const FUNCTION_PACKAGE: Style = Style::new().fg(palette::STACK_FUNCTION_PACKAGE);

/// File path for project frames (clickable in Phase 3)
pub const FILE_PROJECT: Style = Style::new()
    .fg(palette::STACK_FILE_PROJECT)
    .add_modifier(Modifier::UNDERLINED);

/// File path for package frames
pub const FILE_PACKAGE: Style = Style::new().fg(palette::STACK_FILE_PACKAGE);

/// Line/column numbers for project frames
pub const LOCATION_PROJECT: Style = Style::new().fg(palette::STACK_LOCATION_PROJECT);

/// Line/column numbers for package frames
pub const LOCATION_PACKAGE: Style = Style::new().fg(palette::STACK_LOCATION_PACKAGE);

/// Async suspension marker
pub const ASYNC_GAP: Style = Style::new()
    .fg(palette::STACK_ASYNC_GAP)
    .add_modifier(Modifier::ITALIC);

/// Punctuation (parentheses, colons)
pub const PUNCTUATION: Style = Style::new().fg(palette::STACK_PUNCTUATION);

/// Indentation for stack frames
pub const INDENT: &str = "    ";
