//! Semantic style builders for the Cyber-Glass theme.

use fdemon_core::types::AppPhase;
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders};

use super::palette;

// --- Text styles ---
// Kept for future widget styling in Phase 2+
#[allow(dead_code)]
pub fn text_primary() -> Style {
    Style::default().fg(palette::TEXT_PRIMARY)
}

pub fn text_secondary() -> Style {
    Style::default().fg(palette::TEXT_SECONDARY)
}

pub fn text_muted() -> Style {
    Style::default().fg(palette::TEXT_MUTED)
}

// Kept for future high-emphasis text in Phase 2+
#[allow(dead_code)]
pub fn text_bright() -> Style {
    Style::default().fg(palette::TEXT_BRIGHT)
}

// --- Border styles ---
pub fn border_inactive() -> Style {
    Style::default().fg(palette::BORDER_DIM)
}

pub fn border_active() -> Style {
    Style::default().fg(palette::BORDER_ACTIVE)
}

// --- Accent styles ---
pub fn accent() -> Style {
    Style::default().fg(palette::ACCENT)
}

// Kept for future widget styling in Phase 2+
#[allow(dead_code)]
pub fn accent_bold() -> Style {
    Style::default()
        .fg(palette::ACCENT)
        .add_modifier(Modifier::BOLD)
}

// --- Status styles ---
// Kept for future widget styling in Phase 2+
#[allow(dead_code)]
pub fn status_green() -> Style {
    Style::default().fg(palette::STATUS_GREEN)
}

pub fn status_red() -> Style {
    Style::default().fg(palette::STATUS_RED)
}

// Kept for future widget styling in Phase 2+
#[allow(dead_code)]
pub fn status_yellow() -> Style {
    Style::default().fg(palette::STATUS_YELLOW)
}

// Kept for future widget styling in Phase 2+
#[allow(dead_code)]
pub fn status_blue() -> Style {
    Style::default().fg(palette::STATUS_BLUE)
}

// --- Keybinding hint style ---
// Kept for future help panel in Phase 2+
#[allow(dead_code)]
pub fn keybinding() -> Style {
    Style::default().fg(palette::STATUS_YELLOW)
}

// --- Selection styles ---
// Kept for future widget styling in Phase 2+
#[allow(dead_code)]
pub fn selected_highlight() -> Style {
    Style::default()
        .fg(palette::TEXT_BRIGHT)
        .bg(palette::ACCENT)
        .add_modifier(Modifier::BOLD)
}

/// "Black on Cyan" - used for focused+selected items across widgets
pub fn focused_selected() -> Style {
    Style::default()
        .fg(palette::CONTRAST_FG)
        .bg(palette::ACCENT)
        .add_modifier(Modifier::BOLD)
}

// --- Block builders ---
pub fn glass_block(focused: bool) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(if focused {
            border_active()
        } else {
            border_inactive()
        })
}

// Kept for future modal dialogs in Phase 2+
#[allow(dead_code)]
pub fn modal_block(title: &str) -> Block<'_> {
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_inactive())
        .style(Style::default().bg(palette::POPUP_BG))
}

// --- Phase indicator mapping ---

/// Phase indicator for session tabs and status displays.
///
/// Returns `(icon_char, label, Style)` for the given AppPhase.
/// The label is the human-readable status text (e.g., "Running", "Stopped").
pub fn phase_indicator(phase: &AppPhase) -> (&'static str, &'static str, Style) {
    match phase {
        AppPhase::Running => (
            "●",
            "Running",
            Style::default()
                .fg(palette::STATUS_GREEN)
                .add_modifier(Modifier::BOLD),
        ),
        AppPhase::Reloading => (
            "↻",
            "Reloading",
            Style::default()
                .fg(palette::STATUS_YELLOW)
                .add_modifier(Modifier::BOLD),
        ),
        AppPhase::Initializing => ("○", "Starting", Style::default().fg(palette::TEXT_MUTED)),
        AppPhase::Stopped => ("○", "Stopped", Style::default().fg(palette::TEXT_MUTED)),
        AppPhase::Quitting => ("✗", "Stopping", Style::default().fg(palette::STATUS_RED)),
    }
}

/// Phase indicator for "busy" override (running but currently reloading).
///
/// When a session is Running but has pending operations, show the reload indicator.
pub fn phase_indicator_busy() -> (&'static str, &'static str, Style) {
    (
        "↻",
        "Reloading",
        Style::default()
            .fg(palette::STATUS_YELLOW)
            .add_modifier(Modifier::BOLD),
    )
}

/// "Not connected" indicator for when no sessions exist.
// Kept for future multi-session UI in Phase 2+
#[allow(dead_code)]
pub fn phase_indicator_disconnected() -> (&'static str, &'static str, Style) {
    (
        "○",
        "Not Connected",
        Style::default().fg(palette::TEXT_MUTED),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_style_builders_return_styles() {
        let s = text_primary();
        assert_eq!(s.fg, Some(palette::TEXT_PRIMARY));
    }

    #[test]
    fn test_text_styles_have_correct_colors() {
        assert_eq!(text_primary().fg, Some(palette::TEXT_PRIMARY));
        assert_eq!(text_secondary().fg, Some(palette::TEXT_SECONDARY));
        assert_eq!(text_muted().fg, Some(palette::TEXT_MUTED));
        assert_eq!(text_bright().fg, Some(palette::TEXT_BRIGHT));
    }

    #[test]
    fn test_border_styles_have_correct_colors() {
        assert_eq!(border_inactive().fg, Some(palette::BORDER_DIM));
        assert_eq!(border_active().fg, Some(palette::BORDER_ACTIVE));
    }

    #[test]
    fn test_accent_bold_has_modifier() {
        let style = accent_bold();
        assert_eq!(style.fg, Some(palette::ACCENT));
        assert!(style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn test_glass_block_focused_vs_unfocused() {
        // Verify both focused and unfocused blocks can be created
        let _focused = glass_block(true);
        let _unfocused = glass_block(false);
        // Block doesn't expose getters, but we can verify construction succeeds
    }

    #[test]
    fn test_modal_block_has_popup_background() {
        // Verify modal block can be created with a title
        let _block = modal_block("Test Modal");
        // Block doesn't expose getters, but we can verify construction succeeds
    }

    #[test]
    fn test_selected_highlight_has_background() {
        let style = selected_highlight();
        assert_eq!(style.bg, Some(palette::ACCENT));
        assert!(style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn test_focused_selected_uses_black_on_cyan() {
        let style = focused_selected();
        assert_eq!(style.fg, Some(palette::CONTRAST_FG));
        assert_eq!(style.bg, Some(palette::ACCENT));
    }

    #[test]
    fn test_status_styles_have_correct_colors() {
        assert_eq!(status_green().fg, Some(palette::STATUS_GREEN));
        assert_eq!(status_red().fg, Some(palette::STATUS_RED));
        assert_eq!(status_yellow().fg, Some(palette::STATUS_YELLOW));
        assert_eq!(status_blue().fg, Some(palette::STATUS_BLUE));
    }

    #[test]
    fn test_phase_indicator_running() {
        let (icon, label, style) = phase_indicator(&AppPhase::Running);
        assert_eq!(icon, "●");
        assert_eq!(label, "Running");
        assert_eq!(style.fg, Some(palette::STATUS_GREEN));
        assert!(style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn test_phase_indicator_reloading() {
        let (icon, label, style) = phase_indicator(&AppPhase::Reloading);
        assert_eq!(icon, "↻");
        assert_eq!(label, "Reloading");
        assert_eq!(style.fg, Some(palette::STATUS_YELLOW));
        assert!(style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn test_phase_indicator_initializing() {
        let (icon, label, style) = phase_indicator(&AppPhase::Initializing);
        assert_eq!(icon, "○");
        assert_eq!(label, "Starting");
        assert_eq!(style.fg, Some(palette::TEXT_MUTED));
    }

    #[test]
    fn test_phase_indicator_stopped() {
        let (icon, label, style) = phase_indicator(&AppPhase::Stopped);
        assert_eq!(icon, "○");
        assert_eq!(label, "Stopped");
        assert_eq!(style.fg, Some(palette::TEXT_MUTED));
    }

    #[test]
    fn test_phase_indicator_quitting() {
        let (icon, label, style) = phase_indicator(&AppPhase::Quitting);
        assert_eq!(icon, "✗");
        assert_eq!(label, "Stopping");
        assert_eq!(style.fg, Some(palette::STATUS_RED));
    }

    #[test]
    fn test_phase_indicator_all_phases_covered() {
        // Ensure every AppPhase variant returns valid data
        for phase in [
            AppPhase::Running,
            AppPhase::Reloading,
            AppPhase::Initializing,
            AppPhase::Stopped,
            AppPhase::Quitting,
        ] {
            let (icon, label, _style) = phase_indicator(&phase);
            assert!(!icon.is_empty());
            assert!(!label.is_empty());
        }
    }

    #[test]
    fn test_phase_indicator_busy() {
        let (icon, label, style) = phase_indicator_busy();
        assert_eq!(icon, "↻");
        assert_eq!(label, "Reloading");
        assert_eq!(style.fg, Some(palette::STATUS_YELLOW));
        assert!(style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn test_phase_indicator_disconnected() {
        let (icon, label, style) = phase_indicator_disconnected();
        assert_eq!(icon, "○");
        assert_eq!(label, "Not Connected");
        assert_eq!(style.fg, Some(palette::TEXT_MUTED));
    }
}
