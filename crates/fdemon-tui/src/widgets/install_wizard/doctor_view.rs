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
    widgets::{Paragraph, Widget},
};

use fdemon_daemon::toolchain::{DoctorLine, DoctorMarker};

use crate::theme::palette;

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

    /// Render a single [`DoctorLine`] into the buffer at `y`.
    fn render_doctor_line(line: &DoctorLine, y: u16, area: Rect, buf: &mut Buffer) {
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

        let spans = vec![Span::styled(text, Style::default().fg(color))];
        let row_line = Line::from(spans);
        Paragraph::new(row_line).render(Rect::new(area.x, y, area.width, 1), buf);
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

        let visible_height = area.height as usize;
        for (i, line) in lines.iter().take(visible_height).enumerate() {
            let y = area.y + i as u16;
            Self::render_doctor_line(line, y, area, buf);
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
}
