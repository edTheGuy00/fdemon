//! Styling helpers for settings panel rendering

use ratatui::style::{Color, Modifier, Style};

use crate::config::SettingValue;

/// Layout constants for setting rows
pub const INDICATOR_WIDTH: u16 = 3;
pub const INDICATOR_WIDTH_OVERRIDE: u16 = 4; // Extra space for override marker
pub const LABEL_WIDTH: u16 = 25;
pub const LABEL_WIDTH_SHORT: u16 = 24;
pub const LABEL_WIDTH_VSCODE: u16 = 20;
pub const VALUE_WIDTH: u16 = 15;
pub const VALUE_WIDTH_VSCODE: u16 = 20;

/// Get style for a setting value based on its type and selection state
pub fn value_style(value: &SettingValue, is_selected: bool) -> Style {
    let base = if is_selected {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    match value {
        SettingValue::Bool(true) => base.fg(Color::Green),
        SettingValue::Bool(false) => base.fg(Color::Red),
        SettingValue::Number(_) | SettingValue::Float(_) => base.fg(Color::Cyan),
        SettingValue::String(s) if s.is_empty() => base.fg(Color::DarkGray),
        SettingValue::String(_) => base.fg(Color::White),
        SettingValue::Enum { .. } => base.fg(Color::Magenta),
        SettingValue::List(_) => base.fg(Color::Blue),
    }
}

/// Style for selection indicator
pub fn indicator_style(is_selected: bool) -> Style {
    if is_selected {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    }
}

/// Style for override indicator
pub fn override_indicator_style(is_override: bool, is_selected: bool) -> Style {
    if is_override {
        Style::default().fg(Color::Yellow)
    } else if is_selected {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    }
}

/// Style for labels
pub fn label_style(is_selected: bool) -> Style {
    if is_selected {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    }
}

/// Style for editing mode
pub fn editing_style() -> Style {
    Style::default().fg(Color::Yellow).bg(Color::DarkGray)
}

/// Style for section headers
pub fn section_header_style() -> Style {
    Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD)
}

/// Style for config headers (launch configs)
pub fn config_header_style() -> Style {
    Style::default().fg(Color::Cyan)
}

/// Style for VSCode config headers
pub fn vscode_header_style() -> Style {
    Style::default().fg(Color::Blue)
}

/// Style for descriptions (dimmed)
pub fn description_style() -> Style {
    Style::default().fg(Color::DarkGray)
}

/// Style for read-only labels
pub fn readonly_label_style(is_selected: bool) -> Style {
    if is_selected {
        Style::default().fg(Color::White)
    } else {
        Style::default().fg(Color::Gray)
    }
}

/// Style for read-only values
pub fn readonly_value_style() -> Style {
    Style::default().fg(Color::DarkGray)
}

/// Style for read-only indicator
pub fn readonly_indicator_style() -> Style {
    Style::default().fg(Color::DarkGray)
}

/// Style for info box borders
pub fn info_border_style() -> Style {
    Style::default().fg(Color::Blue)
}

/// Style for "Add New" option
pub fn add_new_style(is_selected: bool) -> Style {
    if is_selected {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Green)
    }
}

/// Truncate string with ellipsis if too long
pub fn truncate_str(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else if max_len <= 1 {
        s.chars().take(max_len).collect()
    } else {
        let truncated: String = s.chars().take(max_len - 1).collect();
        format!("{}...", truncated)
    }
}
