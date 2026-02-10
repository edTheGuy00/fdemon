//! Styling helpers for settings panel rendering

use ratatui::style::{Modifier, Style};

use crate::theme::palette;
use fdemon_app::config::SettingValue;

/// Layout constants for setting rows
pub const INDICATOR_WIDTH: u16 = 3;
#[allow(dead_code)] // May be used in future override rendering
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
        SettingValue::Bool(true) => base.fg(palette::STATUS_GREEN),
        SettingValue::Bool(false) => base.fg(palette::STATUS_RED),
        SettingValue::Number(_) | SettingValue::Float(_) => base.fg(palette::ACCENT),
        SettingValue::String(s) if s.is_empty() => base.fg(palette::TEXT_MUTED),
        SettingValue::String(_) => base.fg(palette::TEXT_PRIMARY),
        SettingValue::Enum { .. } => base.fg(palette::STATUS_INDIGO),
        SettingValue::List(_) => base.fg(palette::STATUS_BLUE),
    }
}

/// Style for selection indicator
#[allow(dead_code)] // Used in some rendering paths
pub fn indicator_style(is_selected: bool) -> Style {
    if is_selected {
        Style::default().fg(palette::ACCENT)
    } else {
        Style::default()
    }
}

/// Style for override indicator
pub fn override_indicator_style(is_override: bool, is_selected: bool) -> Style {
    if is_override {
        Style::default().fg(palette::STATUS_YELLOW)
    } else if is_selected {
        Style::default().fg(palette::ACCENT)
    } else {
        Style::default()
    }
}

/// Style for labels
pub fn label_style(is_selected: bool) -> Style {
    if is_selected {
        Style::default()
            .fg(palette::TEXT_PRIMARY)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(palette::TEXT_SECONDARY)
    }
}

/// Style for editing mode
pub fn editing_style() -> Style {
    Style::default()
        .fg(palette::STATUS_YELLOW)
        .bg(palette::BORDER_DIM)
}

/// Style for section headers
pub fn section_header_style() -> Style {
    Style::default()
        .fg(palette::ACCENT_DIM)
        .add_modifier(Modifier::BOLD)
}

/// Style for config headers (launch configs)
pub fn config_header_style() -> Style {
    Style::default().fg(palette::ACCENT)
}

/// Style for VSCode config headers
pub fn vscode_header_style() -> Style {
    Style::default().fg(palette::STATUS_BLUE)
}

/// Style for descriptions (dimmed)
pub fn description_style() -> Style {
    Style::default()
        .fg(palette::TEXT_MUTED)
        .add_modifier(Modifier::ITALIC)
}

/// Style for read-only labels
pub fn readonly_label_style(is_selected: bool) -> Style {
    if is_selected {
        Style::default().fg(palette::TEXT_PRIMARY)
    } else {
        Style::default().fg(palette::TEXT_SECONDARY)
    }
}

/// Style for read-only values
pub fn readonly_value_style() -> Style {
    Style::default().fg(palette::TEXT_MUTED)
}

/// Style for read-only indicator
#[allow(dead_code)] // May be used in future VSCode rendering
pub fn readonly_indicator_style() -> Style {
    Style::default().fg(palette::TEXT_MUTED)
}

/// Style for info box borders (legacy)
#[allow(dead_code)] // Replaced by info_banner_border_style
pub fn info_border_style() -> Style {
    Style::default().fg(palette::STATUS_BLUE)
}

/// Style for "Add New" option
pub fn add_new_style(is_selected: bool) -> Style {
    if is_selected {
        Style::default()
            .fg(palette::STATUS_GREEN)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(palette::STATUS_GREEN)
    }
}

// ─────────────────────────────────────────────────────────
// Cyber-Glass Design Tokens (Phase 4)
// ─────────────────────────────────────────────────────────

/// Style for icon glyph in group headers (e.g., ⚡ before "BEHAVIOR")
#[allow(dead_code)] // Used in Phase 4 tasks 03-06
pub fn group_header_icon_style() -> Style {
    Style::default().fg(palette::ACCENT_DIM)
}

/// Style for selected row background
#[allow(dead_code)] // Used in Phase 4 tasks 03-06
pub fn selected_row_bg() -> Style {
    Style::default().bg(palette::SELECTED_ROW_BG)
}

/// Style for the accent bar on selected rows (▎ left border indicator)
#[allow(dead_code)] // Used in Phase 4 tasks 03-06
pub fn accent_bar_style() -> Style {
    Style::default().fg(palette::ACCENT)
}

/// Style for keyboard shortcut badges in footer (e.g., [Tab], [Esc])
#[allow(dead_code)] // Used in Phase 4 tasks 03-06
pub fn kbd_badge_style() -> Style {
    Style::default()
        .fg(palette::TEXT_SECONDARY)
        .bg(palette::POPUP_BG)
}

/// Style for description text after kbd badges (e.g., "Switch tabs")
#[allow(dead_code)] // Used in Phase 4 tasks 03-06
pub fn kbd_label_style() -> Style {
    Style::default().fg(palette::TEXT_MUTED)
}

/// Style for emphasized keyboard shortcuts (e.g., Ctrl+S)
#[allow(dead_code)] // Used in Phase 4 tasks 03-06
pub fn kbd_accent_style() -> Style {
    Style::default().fg(palette::ACCENT)
}

/// Style for info banner background (User tab)
#[allow(dead_code)] // Used in Phase 4 tasks 03-06
pub fn info_banner_bg() -> Style {
    Style::default().bg(palette::SELECTED_ROW_BG)
}

/// Style for info banner border
#[allow(dead_code)] // Used in Phase 4 tasks 03-06
pub fn info_banner_border_style() -> Style {
    Style::default().fg(palette::ACCENT_DIM)
}

/// Style for large icon in empty states (Launch tab)
#[allow(dead_code)] // Used in Phase 4 tasks 03-06
pub fn empty_state_icon_style() -> Style {
    Style::default().fg(palette::TEXT_MUTED)
}

/// Style for title text in empty states
#[allow(dead_code)] // Used in Phase 4 tasks 03-06
pub fn empty_state_title_style() -> Style {
    Style::default()
        .fg(palette::TEXT_PRIMARY)
        .add_modifier(Modifier::BOLD)
}

/// Style for subtitle text in empty states
#[allow(dead_code)] // Used in Phase 4 tasks 03-06
pub fn empty_state_subtitle_style() -> Style {
    Style::default()
        .fg(palette::TEXT_MUTED)
        .add_modifier(Modifier::ITALIC)
}

/// Style for inactive borders
pub fn border_inactive() -> Style {
    Style::default().fg(palette::BORDER_DIM)
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
