//! Timeline Events tab — Phase 2 stub. Populated in Phase 3.

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::Style,
    widgets::{Paragraph, Widget, Wrap},
};

use crate::theme::palette;

const STUB_MESSAGE: &str = "Coming soon — Phase 3 streams UI/Raster thread timeline events.";

/// Render the Timeline Events tab content area (Phase 2 stub).
pub(super) fn render(area: Rect, buf: &mut Buffer) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let p = Paragraph::new(STUB_MESSAGE)
        .style(Style::default().fg(palette::TEXT_MUTED))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });
    let y_offset = area.height.saturating_sub(2) / 2;
    let centered = Rect {
        y: area.y + y_offset,
        height: area.height.saturating_sub(y_offset),
        ..area
    };
    p.render(centered, buf);
}
