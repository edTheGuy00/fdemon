//! Style helpers and threshold constants for the performance panel.
//!
//! Pure style/format helpers with no widget dependencies.

use ratatui::style::{Color, Style};

use crate::theme::palette;

// ── Style threshold constants ─────────────────────────────────────────────────

/// FPS at or above this value is considered healthy (green).
pub(super) const FPS_GREEN_THRESHOLD: f64 = 55.0;
/// FPS at or above this value (but below green) is degraded (yellow).
pub(super) const FPS_YELLOW_THRESHOLD: f64 = 30.0;
/// Jank frame percentage below this is acceptable (yellow, not red).
pub(super) const JANK_WARN_THRESHOLD: f64 = 0.05;

// ── Style helpers ─────────────────────────────────────────────────────────────

/// Choose a colour for the FPS value based on its magnitude.
pub(super) fn fps_style(fps: Option<f64>) -> Style {
    match fps {
        Some(v) if v >= FPS_GREEN_THRESHOLD => Style::default().fg(palette::STATUS_GREEN),
        Some(v) if v >= FPS_YELLOW_THRESHOLD => Style::default().fg(palette::STATUS_YELLOW),
        Some(_) => Style::default().fg(palette::STATUS_RED),
        None => Style::default().fg(Color::DarkGray), // stale / no data
    }
}

/// Choose a colour for the jank count.
pub(super) fn jank_style(jank_count: u32, total_frames: u64) -> Style {
    if total_frames == 0 || jank_count == 0 {
        return Style::default().fg(palette::STATUS_GREEN);
    }
    let pct = jank_count as f64 / total_frames as f64;
    if pct < JANK_WARN_THRESHOLD {
        Style::default().fg(palette::STATUS_YELLOW)
    } else {
        Style::default().fg(palette::STATUS_RED)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fps_color_green_high_fps() {
        let style = fps_style(Some(60.0));
        assert_eq!(style.fg, Some(palette::STATUS_GREEN));
    }

    #[test]
    fn test_fps_color_yellow_medium_fps() {
        let style = fps_style(Some(45.0));
        assert_eq!(style.fg, Some(palette::STATUS_YELLOW));
    }

    #[test]
    fn test_fps_color_red_low_fps() {
        let style = fps_style(Some(20.0));
        assert_eq!(style.fg, Some(palette::STATUS_RED));
    }

    #[test]
    fn test_fps_color_none() {
        let style = fps_style(None);
        assert_eq!(style.fg, Some(Color::DarkGray));
    }
}
