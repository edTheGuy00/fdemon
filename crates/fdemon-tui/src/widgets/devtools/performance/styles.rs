//! Style helpers and threshold constants for the performance panel.
//!
//! Pure style/format helpers with no widget dependencies.

use ratatui::style::{Color, Style};

use crate::theme::palette;

// ── Style threshold constants ─────────────────────────────────────────────────

/// Cap for sparkline bar heights (2x the 16.67ms frame budget at 60fps).
pub(super) const SPARKLINE_MAX_MS: u64 = 33;
/// FPS at or above this value is considered healthy (green).
pub(super) const FPS_GREEN_THRESHOLD: f64 = 55.0;
/// FPS at or above this value (but below green) is degraded (yellow).
pub(super) const FPS_YELLOW_THRESHOLD: f64 = 30.0;
/// Memory utilization below this is healthy (green).
pub(super) const MEM_GREEN_THRESHOLD: f64 = 0.6;
/// Memory utilization below this (but above green) is elevated (yellow).
pub(super) const MEM_YELLOW_THRESHOLD: f64 = 0.8;
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

/// Choose a gauge colour based on heap utilisation (0.0–1.0).
pub(super) fn gauge_style_for_utilization(util: f64) -> Style {
    if util < MEM_GREEN_THRESHOLD {
        Style::default().fg(palette::STATUS_GREEN)
    } else if util < MEM_YELLOW_THRESHOLD {
        Style::default().fg(palette::STATUS_YELLOW)
    } else {
        Style::default().fg(palette::STATUS_RED)
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

/// Format a large number with comma separators (e.g. 1234567 → "1,234,567").
pub(super) fn format_number(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::with_capacity(s.len() + s.len() / 3);
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    result.chars().rev().collect()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fps_color_green_high_fps() {
        // FPS >= 55 should use STATUS_GREEN
        let style = fps_style(Some(60.0));
        assert_eq!(style.fg, Some(palette::STATUS_GREEN));
    }

    #[test]
    fn test_fps_color_yellow_medium_fps() {
        // FPS 30-54.9 should use STATUS_YELLOW
        let style = fps_style(Some(45.0));
        assert_eq!(style.fg, Some(palette::STATUS_YELLOW));
    }

    #[test]
    fn test_fps_color_red_low_fps() {
        // FPS < 30 should use STATUS_RED
        let style = fps_style(Some(20.0));
        assert_eq!(style.fg, Some(palette::STATUS_RED));
    }

    #[test]
    fn test_fps_color_none() {
        // None fps → DarkGray
        let style = fps_style(None);
        assert_eq!(style.fg, Some(Color::DarkGray));
    }

    #[test]
    fn test_memory_gauge_color_low_utilization() {
        // < 60% should be STATUS_GREEN
        let style = gauge_style_for_utilization(0.4);
        assert_eq!(style.fg, Some(palette::STATUS_GREEN));
    }

    #[test]
    fn test_memory_gauge_color_medium_utilization() {
        // 60%-79% should be STATUS_YELLOW
        let style = gauge_style_for_utilization(0.7);
        assert_eq!(style.fg, Some(palette::STATUS_YELLOW));
    }

    #[test]
    fn test_memory_gauge_color_high_utilization() {
        // >= 80% should be STATUS_RED
        let style = gauge_style_for_utilization(0.85);
        assert_eq!(style.fg, Some(palette::STATUS_RED));
    }

    #[test]
    fn test_format_number_small() {
        assert_eq!(format_number(0), "0");
        assert_eq!(format_number(999), "999");
    }

    #[test]
    fn test_format_number_thousands() {
        assert_eq!(format_number(1000), "1,000");
        assert_eq!(format_number(1234), "1,234");
    }

    #[test]
    fn test_format_number_millions() {
        assert_eq!(format_number(1_234_567), "1,234,567");
    }
}
