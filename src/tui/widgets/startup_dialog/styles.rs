//! Style constants for the startup dialog widget

use ratatui::style::Color;

// Section colors
pub const ACTIVE_BORDER: Color = Color::Cyan;
pub const INACTIVE_BORDER: Color = Color::DarkGray;

// Selection colors
pub const SELECTED_BG: Color = Color::Cyan;
pub const SELECTED_FG: Color = Color::Black;

// List colors
pub const DIVIDER_COLOR: Color = Color::DarkGray;
pub const EMULATOR_ANDROID: Color = Color::Green;
pub const EMULATOR_IOS: Color = Color::Blue;

// Text colors
pub const LABEL_COLOR: Color = Color::Gray;
pub const VALUE_COLOR: Color = Color::White;
pub const PLACEHOLDER_COLOR: Color = Color::DarkGray;
pub const ERROR_COLOR: Color = Color::Red;
pub const LOADING_COLOR: Color = Color::Yellow;
pub const DISABLED_COLOR: Color = Color::DarkGray;

// Task 10e: New config option color
pub const NEW_CONFIG_COLOR: Color = Color::Green;
