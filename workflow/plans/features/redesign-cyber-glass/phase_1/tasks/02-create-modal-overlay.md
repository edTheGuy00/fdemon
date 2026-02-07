## Task: Create Shared Modal Overlay Utilities

**Objective**: Create a reusable modal overlay widget module that consolidates the 6+ duplicated centering implementations and 2 identical dim overlay functions into shared utilities.

**Depends on**: None

### Scope

- `crates/fdemon-tui/src/widgets/modal_overlay.rs` — **NEW** Shared overlay utilities
- `crates/fdemon-tui/src/widgets/mod.rs` — Add `pub mod modal_overlay;`

### Details

#### Current Duplication

The codebase has **6+ centering implementations** using 3 different approaches:

| Location | Approach |
|----------|----------|
| `confirm_dialog.rs:27-31` | Manual arithmetic (fixed size) |
| `new_session_dialog/mod.rs:192-206` | Layout percentage (80%x70%) |
| `new_session_dialog/mod.rs:415-431` | Layout percentage (parameterized) |
| `new_session_dialog/fuzzy_modal.rs:50-65` | Manual (bottom-anchored) |
| `new_session_dialog/dart_defines_modal.rs:573-580` | Manual (near-fullscreen) |
| `selector.rs:246-253` | Layout with `Flex::Center` (cleanest) |
| `search_input.rs:103-109` | Manual arithmetic (fixed size) |
| `render/mod.rs:218-236` | Layout percentage |

There are also **2 identical dim overlay functions**:
- `fuzzy_modal.rs:225-238` — `render_dim_overlay()` (uses `saturating_add`)
- `dart_defines_modal.rs:696-704` — `render_dart_defines_dim_overlay()` (raw addition)

#### New Module (`widgets/modal_overlay.rs`)

Create shared utility functions:

```rust
//! Shared modal overlay utilities.
//!
//! Provides reusable functions for centering rects, dimming backgrounds,
//! and rendering shadows — common operations for modal dialogs.

use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::Style;
use ratatui::widgets::{Clear, Widget};

/// Center a fixed-size rect within an area.
///
/// If the requested size exceeds the area, clamps to the area dimensions.
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
pub fn dim_background(buf: &mut Buffer, area: Rect) {
    let dim_style = Style::default()
        .fg(ratatui::style::Color::DarkGray)
        .bg(ratatui::style::Color::Black);

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
pub fn render_shadow(buf: &mut Buffer, modal_rect: Rect) {
    let shadow_style = Style::default()
        .fg(ratatui::style::Color::Black)
        .bg(ratatui::style::Color::Black);

    // Right edge shadow (1 cell wide, full height)
    let right_x = modal_rect.x.saturating_add(modal_rect.width);
    for y in modal_rect.y.saturating_add(1)..modal_rect.y.saturating_add(modal_rect.height).saturating_add(1) {
        if let Some(cell) = buf.cell_mut((right_x, y)) {
            cell.set_char(' ');
            cell.set_style(shadow_style);
        }
    }

    // Bottom edge shadow (full width, 1 cell tall)
    let bottom_y = modal_rect.y.saturating_add(modal_rect.height);
    for x in modal_rect.x.saturating_add(1)..modal_rect.x.saturating_add(modal_rect.width).saturating_add(1) {
        if let Some(cell) = buf.cell_mut((x, bottom_y)) {
            cell.set_char(' ');
            cell.set_style(shadow_style);
        }
    }
}

/// Clear a rect and prepare it for modal content.
///
/// Renders the `Clear` widget to reset cells in the given area.
pub fn clear_area(buf: &mut Buffer, area: Rect) {
    Clear.render(area, buf);
}
```

#### Module Registration

Add to `crates/fdemon-tui/src/widgets/mod.rs`:
```rust
pub mod modal_overlay;
```

### Acceptance Criteria

1. `crates/fdemon-tui/src/widgets/modal_overlay.rs` exists with:
   - `centered_rect(width, height, area) -> Rect`
   - `centered_rect_percent(width_percent, height_percent, area) -> Rect`
   - `dim_background(buf, area)`
   - `render_shadow(buf, modal_rect)`
   - `clear_area(buf, area)`
2. Module registered in `widgets/mod.rs`
3. `cargo check -p fdemon-tui` passes
4. `cargo clippy -p fdemon-tui` passes with no warnings
5. **No existing code is modified** — this task is additive only. Migration of existing callers to use these shared utilities happens in Phase 3+ when modals are redesigned.

### Testing

```rust
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
    fn test_centered_rect_percent() {
        let area = Rect::new(0, 0, 100, 50);
        let result = centered_rect_percent(80, 70, area);
        assert!(result.width >= 78 && result.width <= 82); // ~80%
        assert!(result.height >= 33 && result.height <= 37); // ~70%
    }

    #[test]
    fn test_dim_background_covers_area() {
        let area = Rect::new(0, 0, 10, 5);
        let mut buf = Buffer::empty(area);
        dim_background(&mut buf, area);
        // All cells should now have DarkGray fg and Black bg
        for y in 0..5 {
            for x in 0..10 {
                let cell = &buf[(x, y)];
                assert_eq!(cell.fg, Color::DarkGray);
                assert_eq!(cell.bg, Color::Black);
            }
        }
    }

    #[test]
    fn test_render_shadow_offset() {
        let area = Rect::new(0, 0, 20, 10);
        let modal = Rect::new(5, 2, 10, 6);
        let mut buf = Buffer::empty(area);
        render_shadow(&mut buf, modal);
        // Cell at (15, 3) should be shadow (right edge)
        // Cell at (6, 8) should be shadow (bottom edge)
    }
}
```

### Notes

- This task is **additive only** — existing widget code keeps its own centering/dim implementations for now. The existing functions are not removed or redirected in Phase 1.
- The shared `dim_background` uses `saturating_add` for overflow safety (matching the better of the two existing implementations).
- `render_shadow` is new functionality — not currently used anywhere. It will be used in Phase 3 when modals are redesigned.
- The `clear_area` function is a thin wrapper around `Clear.render()` for consistency in the overlay API.
- Consider whether `dim_background` should use the theme palette's `SHADOW` color instead of hardcoded `Color::Black`. For Phase 1 keeping it simple is fine; Phase 2 can refine.
