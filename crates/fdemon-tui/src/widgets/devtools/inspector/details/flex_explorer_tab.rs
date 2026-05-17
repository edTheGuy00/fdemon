//! Flex explorer tab — stub for Phase 1. Phase 2 will populate this with an
//! interactive flex layout explorer for `Row`, `Column`, and `Flex` widgets,
//! mirroring the Flutter DevTools Flex Explorer panel.

use ratatui::{buffer::Buffer, layout::Rect};

use crate::theme::palette;

/// Render the Flex-explorer tab content into `area`.
///
/// Phase 1 stub: displays a centered "Coming soon — Phase 2" message.
/// Phase 2 will replace this body with the full flex layout explorer UI.
#[allow(dead_code)] // Called by details/mod.rs render_details_panel — wired by task 09
pub(super) fn render(area: Rect, buf: &mut Buffer) {
    render_centered_text(area, buf, "Coming soon \u{2014} Phase 2");
}

#[allow(dead_code)] // Called by render — wired by task 09
fn render_centered_text(area: Rect, buf: &mut Buffer, text: &str) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let y = area.y + area.height / 2;
    let text_len = text.chars().count() as u16;
    let x = area.x + area.width.saturating_sub(text_len) / 2;
    buf.set_string(
        x,
        y,
        text,
        ratatui::style::Style::default().fg(palette::TEXT_MUTED),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    fn collect_buf_text(buf: &Buffer, width: u16, height: u16) -> String {
        let mut full = String::new();
        for y in 0..height {
            for x in 0..width {
                if let Some(c) = buf.cell((x, y)) {
                    if let Some(ch) = c.symbol().chars().next() {
                        full.push(ch);
                    }
                }
            }
        }
        full
    }

    #[test]
    fn flex_explorer_stub_renders_coming_soon() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 60, 10));
        render(buf.area, &mut buf);
        let text = collect_buf_text(&buf, 60, 10);
        assert!(
            text.contains("Coming") && text.contains("soon"),
            "Expected 'Coming soon' in buffer, got: {text:?}"
        );
    }

    #[test]
    fn flex_explorer_stub_zero_area_no_panic() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        render(buf.area, &mut buf);
        // Should not panic
    }

    #[test]
    fn flex_explorer_stub_single_row_no_panic() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 1));
        render(buf.area, &mut buf);
        // Should not panic
    }
}
