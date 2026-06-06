//! # Doctor View
//!
//! Renders the parsed output of `flutter doctor -v` inside the Install Wizard
//! detail pane.
//!
//! Each [`DoctorLine`] is rendered as a single terminal row, indented by
//! `line.indent` spaces and prefixed with a marker glyph for lines that have
//! a non-`None` marker.  Coloring follows the [`DoctorMarker`] semantics:
//! - `Ok`      → green
//! - `Warning` → yellow
//! - `Error` / `Dead` → red
//! - `None`    → default text color

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Paragraph, Widget, Wrap},
};

use fdemon_app::install_wizard::{DoctorLine, DoctorMarker};

use crate::theme::palette;

/// Compute the number of terminal rows needed to render `text` in a pane of `width` columns.
///
/// Fallback for `Paragraph::line_count` (which is behind an unstable gate).
/// Each character is measured by its display width; wide (CJK/emoji) characters count as 2.
/// Returns at least 1.
fn wrapped_height(text: &str, width: u16) -> u16 {
    if width == 0 {
        return 1;
    }
    let display_w: u16 = text
        .chars()
        .filter(|c| !c.is_control())
        .map(|c| {
            let cp = c as u32;
            if matches!(cp,
                0x1100..=0x115F | 0x2E80..=0x303E | 0x3041..=0x33FF | 0x3400..=0x4DBF
                | 0x4E00..=0xA4CF | 0xA960..=0xA97F | 0xAC00..=0xD7FF | 0xF900..=0xFAFF
                | 0xFE10..=0xFE19 | 0xFE30..=0xFE4F | 0xFF01..=0xFF60 | 0xFFE0..=0xFFE6
                | 0x1F004..=0x1F9FF | 0x20000..=0x2FFFF
            ) {
                2u16
            } else {
                1u16
            }
        })
        .sum();
    (display_w.max(1) as u32)
        .div_ceil(width as u32)
        .try_into()
        .unwrap_or(u16::MAX)
        .max(1)
}

/// Glyph prefix for `DoctorMarker::Ok` lines (`[✓]`).
const MARKER_OK: &str = "[✓] ";
/// Glyph prefix for `DoctorMarker::Warning` lines (`[!]`).
const MARKER_WARNING: &str = "[!] ";
/// Glyph prefix for `DoctorMarker::Error` lines (`[✗]`).
const MARKER_ERROR: &str = "[✗] ";
/// Glyph prefix for `DoctorMarker::Dead` lines (`[☠]`).
const MARKER_DEAD: &str = "[☠] ";

/// Widget that renders a slice of [`DoctorLine`] items.
///
/// Accepts an `Option<&[DoctorLine]>` — when `None`, renders a placeholder
/// explaining that Flutter is not installed.
pub struct DoctorView<'a> {
    lines: Option<&'a [DoctorLine]>,
}

impl<'a> DoctorView<'a> {
    /// Create a new doctor view.
    ///
    /// Pass `None` when no doctor output is available (Flutter not installed).
    pub fn new(lines: Option<&'a [DoctorLine]>) -> Self {
        Self { lines }
    }

    /// Render a single [`DoctorLine`] into the buffer at `y`, wrapping if needed.
    ///
    /// Returns the number of terminal rows consumed (≥ 1), clamped to `remaining`.
    fn render_doctor_line(
        line: &DoctorLine,
        y: u16,
        area: Rect,
        remaining: u16,
        buf: &mut Buffer,
    ) -> u16 {
        if remaining == 0 {
            return 0;
        }

        let color = match line.marker {
            DoctorMarker::Ok => palette::STATUS_GREEN,
            DoctorMarker::Warning => palette::STATUS_YELLOW,
            DoctorMarker::Error | DoctorMarker::Dead => palette::STATUS_RED,
            DoctorMarker::None => palette::TEXT_PRIMARY,
        };

        let prefix = match line.marker {
            DoctorMarker::Ok => MARKER_OK,
            DoctorMarker::Warning => MARKER_WARNING,
            DoctorMarker::Error => MARKER_ERROR,
            DoctorMarker::Dead => MARKER_DEAD,
            DoctorMarker::None => "",
        };

        let indent = " ".repeat(line.indent);
        let text = format!("{indent}{prefix}{}", line.text);

        let h = wrapped_height(&text, area.width).min(remaining);
        let spans = vec![Span::styled(text, Style::default().fg(color))];
        let row_line = Line::from(spans);
        Paragraph::new(row_line)
            .wrap(Wrap { trim: false })
            .render(Rect::new(area.x, y, area.width, h), buf);
        h
    }
}

impl Widget for DoctorView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }

        let Some(lines) = self.lines else {
            // Flutter not installed — render a placeholder message.
            let msg = Line::from(Span::styled(
                "  flutter doctor unavailable (Flutter not installed)",
                Style::default().fg(palette::TEXT_MUTED),
            ));
            Paragraph::new(msg).render(Rect::new(area.x, area.y, area.width, 1), buf);
            return;
        };

        if lines.is_empty() {
            let msg = Line::from(Span::styled(
                "  No flutter doctor output available.",
                Style::default().fg(palette::TEXT_MUTED),
            ));
            Paragraph::new(msg).render(Rect::new(area.x, area.y, area.width, 1), buf);
            return;
        }

        // Render lines with wrapping, advancing y by each line's rendered height.
        // This replaces the old fixed i→y mapping so long doctor lines wrap instead
        // of being clipped at the right edge of the pane.
        let mut y = area.y;
        for line in lines.iter() {
            if y >= area.y + area.height {
                break;
            }
            let remaining = area.y + area.height - y;
            let h = Self::render_doctor_line(line, y, area, remaining, buf);
            y += h;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{buffer::Buffer, layout::Rect};

    fn make_area() -> Rect {
        Rect::new(0, 0, 60, 20)
    }

    fn make_doctor_lines() -> Vec<DoctorLine> {
        vec![
            DoctorLine {
                marker: DoctorMarker::Ok,
                text: "Flutter (Channel stable, 3.19.0)".to_string(),
                indent: 0,
            },
            DoctorLine {
                marker: DoctorMarker::Warning,
                text: "Android toolchain - develop for Android devices".to_string(),
                indent: 0,
            },
            DoctorLine {
                marker: DoctorMarker::Error,
                text: "Chrome - develop for the web".to_string(),
                indent: 0,
            },
            DoctorLine {
                marker: DoctorMarker::None,
                text: "  • Flutter version 3.19.0 on channel stable".to_string(),
                indent: 2,
            },
        ]
    }

    #[test]
    fn test_doctor_view_renders_markers() {
        let lines = make_doctor_lines();
        let view = DoctorView::new(Some(&lines));
        let area = make_area();
        let mut buf = Buffer::empty(area);
        view.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();

        // Check marker glyphs are present
        assert!(content.contains("✓"), "Ok marker should be rendered");
        assert!(content.contains("!"), "Warning marker should be rendered");
        assert!(content.contains("✗"), "Error marker should be rendered");
    }

    #[test]
    fn test_doctor_view_renders_line_text() {
        let lines = make_doctor_lines();
        let view = DoctorView::new(Some(&lines));
        let area = make_area();
        let mut buf = Buffer::empty(area);
        view.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();

        assert!(
            content.contains("Flutter"),
            "Flutter line text should be rendered"
        );
        assert!(
            content.contains("Android"),
            "Android line text should be rendered"
        );
    }

    #[test]
    fn test_doctor_view_none_shows_unavailable() {
        let view = DoctorView::new(None);
        let area = make_area();
        let mut buf = Buffer::empty(area);
        view.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();

        assert!(
            content.contains("unavailable"),
            "None input should show unavailable message"
        );
        assert!(
            content.contains("Flutter not installed"),
            "should mention Flutter not installed"
        );
    }

    #[test]
    fn test_doctor_view_empty_lines_shows_no_output() {
        let lines: Vec<DoctorLine> = vec![];
        let view = DoctorView::new(Some(&lines));
        let area = make_area();
        let mut buf = Buffer::empty(area);
        view.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();

        assert!(
            content.contains("No flutter doctor output"),
            "empty lines should show no-output message"
        );
    }

    #[test]
    fn test_doctor_view_no_panic_zero_height() {
        let lines = make_doctor_lines();
        let view = DoctorView::new(Some(&lines));
        let area = Rect::new(0, 0, 60, 0);
        let mut buf = Buffer::empty(Rect::new(0, 0, 60, 1));
        view.render(area, &mut buf); // must not panic
    }

    #[test]
    fn test_doctor_view_dead_marker_rendered() {
        let lines = vec![DoctorLine {
            marker: DoctorMarker::Dead,
            text: "VS Code".to_string(),
            indent: 0,
        }];
        let view = DoctorView::new(Some(&lines));
        let area = make_area();
        let mut buf = Buffer::empty(area);
        view.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(
            content.contains("☠"),
            "Dead marker should render skull glyph"
        );
    }

    #[test]
    fn test_doctor_view_indented_line() {
        let lines = vec![DoctorLine {
            marker: DoctorMarker::None,
            text: "some detail".to_string(),
            indent: 4,
        }];
        let view = DoctorView::new(Some(&lines));
        let area = make_area();
        let mut buf = Buffer::empty(area);
        view.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();
        assert!(
            content.contains("some detail"),
            "indented line text should appear"
        );
    }

    /// NEW (Phase 6): A long doctor line wraps onto the next row so the tail is visible.
    #[test]
    fn test_doctor_view_long_line_wraps() {
        // A line too long for a 40-column pane — tail token "en_US.UTF-8" would be clipped
        // without wrapping.
        let lines = vec![DoctorLine {
            marker: DoctorMarker::Ok,
            text: "Flutter (Channel stable, 3.19.0, on Linux 6.x x86_64, locale en_US.UTF-8)"
                .to_string(),
            indent: 0,
        }];
        let view = DoctorView::new(Some(&lines));
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);
        view.render(area, &mut buf);
        let content: String = buf.content().iter().map(|c| c.symbol()).collect();

        assert!(
            content.contains("en_US.UTF-8"),
            "tail of long doctor line must be visible after wrapping: '{content}'"
        );
    }

    /// NEW (Phase 6): `wrapped_height` helper unit test — basic ASCII.
    #[test]
    fn test_wrapped_height_basic() {
        assert_eq!(wrapped_height("hello", 80), 1, "short string → 1 row");
        assert_eq!(
            wrapped_height(&"a".repeat(80), 80),
            1,
            "exact width → 1 row"
        );
        assert_eq!(wrapped_height(&"a".repeat(81), 80), 2, "width+1 → 2 rows");
        assert_eq!(wrapped_height("x", 0), 1, "zero width → 1 (no panic)");
    }
}
