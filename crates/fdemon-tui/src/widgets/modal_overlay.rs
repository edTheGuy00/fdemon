//! Shared modal overlay utilities.
//!
//! Provides reusable functions for centering rects, dimming backgrounds,
//! and rendering shadows — common operations for modal dialogs.

use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::widgets::{Clear, Widget};

use crate::theme::palette;

/// Center a fixed-size rect within an area.
///
/// If the requested size exceeds the area, clamps to the area dimensions.
///
/// # Arguments
/// * `width` - Desired width of the centered rect
/// * `height` - Desired height of the centered rect
/// * `area` - Area within which to center the rect
///
/// # Returns
/// A `Rect` centered within the area, clamped if necessary
///
/// # Examples
/// ```
/// use ratatui::layout::Rect;
/// use fdemon_tui::widgets::modal_overlay::centered_rect;
///
/// let area = Rect::new(0, 0, 80, 24);
/// let modal = centered_rect(40, 10, area);
/// assert_eq!(modal, Rect::new(20, 7, 40, 10));
/// ```
pub fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    Rect::new(x, y, w, h)
}

/// Center a percentage-based rect within an area.
///
/// `width_percent` and `height_percent` should be 0-100.
///
/// # Arguments
/// * `width_percent` - Width as percentage of area (0-100)
/// * `height_percent` - Height as percentage of area (0-100)
/// * `area` - Area within which to center the rect
///
/// # Returns
/// A `Rect` centered within the area, sized by percentage
///
/// # Examples
/// ```
/// use ratatui::layout::Rect;
/// use fdemon_tui::widgets::modal_overlay::centered_rect_percent;
///
/// let area = Rect::new(0, 0, 100, 50);
/// let modal = centered_rect_percent(80, 70, area);
/// // Result will be approximately 80% width and 70% height, centered
/// ```
pub fn centered_rect_percent(width_percent: u16, height_percent: u16, area: Rect) -> Rect {
    let popup_layout = Layout::vertical([
        Constraint::Percentage((100 - height_percent) / 2),
        Constraint::Percentage(height_percent),
        Constraint::Percentage((100 - height_percent) / 2),
    ])
    .split(area);

    Layout::horizontal([
        Constraint::Percentage((100 - width_percent) / 2),
        Constraint::Percentage(width_percent),
        Constraint::Percentage((100 - width_percent) / 2),
    ])
    .split(popup_layout[1])[1]
}

/// Dim all cells in the given area by overriding their styles.
///
/// Simulates a semi-transparent dark overlay (like CSS `bg-black/40 backdrop-blur`).
/// Iterates all cells and applies a dim fg + dark bg.
///
/// # Arguments
/// * `buf` - Buffer to modify
/// * `area` - Area to dim
///
/// # Examples
/// ```
/// use ratatui::buffer::Buffer;
/// use ratatui::layout::Rect;
/// use fdemon_tui::widgets::modal_overlay::dim_background;
///
/// let area = Rect::new(0, 0, 10, 5);
/// let mut buf = Buffer::empty(area);
/// dim_background(&mut buf, area);
/// // All cells now have dimmed styling
/// ```
pub fn dim_background(buf: &mut Buffer, area: Rect) {
    let dim_style = Style::default()
        .fg(palette::TEXT_MUTED)
        .bg(palette::DEEPEST_BG);

    let y_end = area.y.saturating_add(area.height);
    let x_end = area.x.saturating_add(area.width);
    for y in area.y..y_end {
        for x in area.x..x_end {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_style(dim_style);
            }
        }
    }
}

/// Render a 1-cell shadow offset to the right and bottom of a modal rect.
///
/// Creates the illusion of elevation by drawing darker cells along the
/// right edge and bottom edge, offset by 1 cell.
///
/// # Arguments
/// * `buf` - Buffer to modify
/// * `modal_rect` - The modal rect to add shadow to
///
/// # Examples
/// ```
/// use ratatui::buffer::Buffer;
/// use ratatui::layout::Rect;
/// use fdemon_tui::widgets::modal_overlay::render_shadow;
///
/// let area = Rect::new(0, 0, 20, 10);
/// let modal = Rect::new(5, 2, 10, 6);
/// let mut buf = Buffer::empty(area);
/// render_shadow(&mut buf, modal);
/// // Shadow cells rendered at right and bottom edges
/// ```
pub fn render_shadow(buf: &mut Buffer, modal_rect: Rect) {
    let shadow_style = Style::default().fg(palette::SHADOW).bg(palette::SHADOW);

    // Right edge shadow (1 cell wide, full height)
    let right_x = modal_rect.x.saturating_add(modal_rect.width);
    for y in modal_rect.y.saturating_add(1)
        ..modal_rect
            .y
            .saturating_add(modal_rect.height)
            .saturating_add(1)
    {
        if let Some(cell) = buf.cell_mut((right_x, y)) {
            cell.set_char(' ');
            cell.set_style(shadow_style);
        }
    }

    // Bottom edge shadow (full width, 1 cell tall)
    let bottom_y = modal_rect.y.saturating_add(modal_rect.height);
    for x in modal_rect.x.saturating_add(1)
        ..modal_rect
            .x
            .saturating_add(modal_rect.width)
            .saturating_add(1)
    {
        if let Some(cell) = buf.cell_mut((x, bottom_y)) {
            cell.set_char(' ');
            cell.set_style(shadow_style);
        }
    }
}

/// Clear a rect and prepare it for modal content.
///
/// Renders the `Clear` widget to reset cells in the given area.
///
/// # Arguments
/// * `buf` - Buffer to modify
/// * `area` - Area to clear
///
/// # Examples
/// ```
/// use ratatui::buffer::Buffer;
/// use ratatui::layout::Rect;
/// use fdemon_tui::widgets::modal_overlay::clear_area;
///
/// let area = Rect::new(0, 0, 10, 5);
/// let mut buf = Buffer::empty(area);
/// clear_area(&mut buf, area);
/// // Area is now cleared and ready for content
/// ```
pub fn clear_area(buf: &mut Buffer, area: Rect) {
    Clear.render(area, buf);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_centered_rect_within_area() {
        let area = Rect::new(0, 0, 80, 24);
        let result = centered_rect(40, 10, area);
        assert_eq!(result, Rect::new(20, 7, 40, 10));
    }

    #[test]
    fn test_centered_rect_clamps_to_area() {
        let area = Rect::new(0, 0, 30, 10);
        let result = centered_rect(40, 10, area);
        assert_eq!(result.width, 30);
        assert_eq!(result.height, 10);
    }

    #[test]
    fn test_centered_rect_with_offset_area() {
        let area = Rect::new(10, 5, 80, 24);
        let result = centered_rect(40, 10, area);
        assert_eq!(result, Rect::new(30, 12, 40, 10));
    }

    #[test]
    fn test_centered_rect_percent() {
        let area = Rect::new(0, 0, 100, 50);
        let result = centered_rect_percent(80, 70, area);
        assert!(result.width >= 78 && result.width <= 82); // ~80%
        assert!(result.height >= 33 && result.height <= 37); // ~70%
    }

    #[test]
    fn test_centered_rect_percent_50_50() {
        let area = Rect::new(0, 0, 100, 100);
        let result = centered_rect_percent(50, 50, area);
        assert!(result.width >= 48 && result.width <= 52); // ~50%
        assert!(result.height >= 48 && result.height <= 52); // ~50%
    }

    #[test]
    fn test_dim_background_covers_area() {
        let area = Rect::new(0, 0, 10, 5);
        let mut buf = Buffer::empty(area);
        dim_background(&mut buf, area);
        // All cells should now have TEXT_MUTED fg and DEEPEST_BG bg
        for y in 0..5 {
            for x in 0..10 {
                let cell = &buf[(x, y)];
                assert_eq!(cell.fg, palette::TEXT_MUTED);
                assert_eq!(cell.bg, palette::DEEPEST_BG);
            }
        }
    }

    #[test]
    fn test_dim_background_offset_area() {
        let area = Rect::new(5, 3, 10, 5);
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 10));
        dim_background(&mut buf, area);
        // Only cells within the offset area should be dimmed
        for y in 3..8 {
            for x in 5..15 {
                let cell = &buf[(x, y)];
                assert_eq!(cell.fg, palette::TEXT_MUTED);
                assert_eq!(cell.bg, palette::DEEPEST_BG);
            }
        }
    }

    #[test]
    fn test_render_shadow_offset() {
        let area = Rect::new(0, 0, 20, 10);
        let modal = Rect::new(5, 2, 10, 6);
        let mut buf = Buffer::empty(area);
        render_shadow(&mut buf, modal);

        // Cell at (15, 3) should be shadow (right edge, offset by 1)
        let right_shadow = &buf[(15, 3)];
        assert_eq!(right_shadow.fg, palette::SHADOW);
        assert_eq!(right_shadow.bg, palette::SHADOW);
        assert_eq!(right_shadow.symbol(), " ");

        // Cell at (6, 8) should be shadow (bottom edge, offset by 1)
        let bottom_shadow = &buf[(6, 8)];
        assert_eq!(bottom_shadow.fg, palette::SHADOW);
        assert_eq!(bottom_shadow.bg, palette::SHADOW);
        assert_eq!(bottom_shadow.symbol(), " ");
    }

    #[test]
    fn test_render_shadow_no_overflow() {
        let area = Rect::new(0, 0, 10, 10);
        let modal = Rect::new(8, 8, 2, 2); // Near edge
        let mut buf = Buffer::empty(area);
        // Should not panic with out-of-bounds access
        render_shadow(&mut buf, modal);
    }

    #[test]
    fn test_clear_area() {
        let area = Rect::new(0, 0, 10, 5);
        let mut buf = Buffer::empty(area);

        // Fill buffer with content first
        for y in 0..5 {
            for x in 0..10 {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_char('X');
                }
            }
        }

        // Clear a portion of it
        let clear_rect = Rect::new(2, 2, 5, 2);
        clear_area(&mut buf, clear_rect);

        // Cleared cells should be reset
        for y in 2..4 {
            for x in 2..7 {
                let cell = &buf[(x, y)];
                assert_eq!(cell.symbol(), " ");
            }
        }
    }
}
