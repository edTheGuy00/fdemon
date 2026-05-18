//! Frame Analysis tab — populated in T05. T04 ships a single-line placeholder
//! so the dispatch in [`super::render`] compiles and the layout is testable.

use fdemon_app::session::PerformanceState;
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::Style,
    widgets::{Paragraph, Widget, Wrap},
};

use crate::theme::palette;

/// Render the Frame Analysis tab content area.
///
/// T05 replaces this with the real frame-number header + total/budget line +
/// proportional phase bar + hint list + no-data / no-selection fallbacks.
pub(super) fn render(area: Rect, buf: &mut Buffer, _performance: &PerformanceState) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    // T05 replaces this with the real frame-number header + total/budget line +
    // proportional phase bar + hint list + no-data / no-selection fallbacks.
    let placeholder = "Frame Analysis (Phase 2 stub — content lands in T05).";
    let p = Paragraph::new(placeholder)
        .style(Style::default().fg(palette::TEXT_MUTED))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });
    p.render(area, buf);
}
