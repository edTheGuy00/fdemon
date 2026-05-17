//! Render object tab — stub for Phase 1. Phase 2 will populate this from
//! `inspector_state.render_properties` (fetched via `getProperties` RPC).

use ratatui::{buffer::Buffer, layout::Rect};

use crate::theme::palette;

/// Render the Render-object tab content into `area`.
///
/// Phase 1 stub: displays a centered "Coming soon — Phase 2" message.
/// Phase 2 will replace this body with a rendered list of render-object
/// property nodes from `inspector_state.render_properties`.
pub(super) fn render(area: Rect, buf: &mut Buffer) {
    render_centered_text(area, buf, "Coming soon \u{2014} Phase 2");
}

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

    // Canonical copy lives in inspector::test_helpers (m13 fix).
    // Path: tests::super (render_object_tab) → ::super (details) → ::super (inspector) → test_helpers
    use super::super::super::test_helpers::collect_buf_text;

    #[test]
    fn render_object_stub_renders_coming_soon() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 60, 10));
        render(buf.area, &mut buf);
        let text = collect_buf_text(&buf, 60, 10);
        assert!(
            text.contains("Coming") && text.contains("soon"),
            "Expected 'Coming soon' in buffer, got: {text:?}"
        );
    }

    #[test]
    fn render_object_stub_zero_area_no_panic() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        render(buf.area, &mut buf);
        // Should not panic
    }

    #[test]
    fn render_object_stub_single_row_no_panic() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 1));
        render(buf.area, &mut buf);
        // Should not panic
    }
}
