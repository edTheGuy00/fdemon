//! Flex Explorer tab — ASCII flex diagram for `Row`, `Column`, and `Flex` widgets.
//!
//! Renders an annotated ASCII flex diagram that shows each child of the selected
//! flex container stacked in equal-size boxes labeled with measured sizes, flex
//! factors, and fit modes. Axis arrows and alignment labels are drawn on the
//! main-axis indicator strip and in the outer border title.
//!
//! Per parent PLAN §7.1, child boxes are **fixed-height equal-size stacks** — they
//! do NOT scale proportionally to flex factor or actual dimensions. Hierarchy is
//! communicated through labels.

use fdemon_app::state::InspectorState;
use fdemon_core::widget_tree::{Axis, CrossAxisAlignment, FlexChild, FlexFit, MainAxisAlignment};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::Style,
    widgets::{Block, Borders, Widget},
};

use crate::theme::palette;

// ── Constants ─────────────────────────────────────────────────────────────────

/// Minimum visible height inside the tab block (excluding outer borders) for
/// the visualization to fit. Below this, render a "Terminal too small" fallback.
const MIN_FLEX_VIZ_HEIGHT: u16 = 12;

/// Minimum visible width inside the tab block. Below this, fallback message.
const MIN_FLEX_VIZ_WIDTH: u16 = 40;

/// Height of one child box in cells. Constant — boxes do NOT scale with
/// flex factor or actual size (per parent PLAN §7.1).
const CHILD_BOX_HEIGHT: u16 = 4;

/// Width of the main-axis indicator strip (in cells) on the right (vertical)
/// or bottom (horizontal).
const MAIN_AXIS_STRIP_WIDTH: u16 = 3;

/// Minimum height (rows) required to render a horizontal flex visualization.
/// Composed of: 1 header row + 1 child row + 1 strip row + 1 footer row.
const MIN_HORIZONTAL_FLEX_HEIGHT: u16 = 4;

/// Up arrow char used in the vertical main-axis strip.
const MAIN_AXIS_ARROW_UP: char = '▲';

/// Down arrow char used in the vertical main-axis strip.
const MAIN_AXIS_ARROW_DOWN: char = '▼';

/// Left arrow char used in the horizontal main-axis strip.
const MAIN_AXIS_ARROW_LEFT: char = '◀';

/// Right arrow char used in the horizontal main-axis strip.
const MAIN_AXIS_ARROW_RIGHT: char = '▶';

// ── Public(super) entry point ────────────────────────────────────────────────

/// Render the Flex Explorer tab content into `area` using `inspector_state`.
///
/// Dispatches to the appropriate renderer based on layout data availability
/// and whether the selected widget is a flex container.
pub(super) fn render(area: Rect, buf: &mut Buffer, inspector_state: &InspectorState) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    // ── State dispatch ────────────────────────────────────────────────────────
    if inspector_state.layout_loading {
        render_muted_centered(area, buf, "Loading layout...");
        return;
    }

    if let Some(ref error) = inspector_state.layout_error {
        render_muted_centered(area, buf, &format!("Error: {}", error.message));
        return;
    }

    let Some(ref layout) = inspector_state.layout else {
        render_muted_centered(area, buf, "No layout data \u{2014} press Enter to fetch.");
        return;
    };

    // Non-flex widget: no direction and no children
    if layout.direction.is_none() && layout.children.is_empty() {
        render_muted_centered(
            area,
            buf,
            "This widget is not a Row, Column, or Flex container.",
        );
        return;
    }

    // Below minimum dimensions — fallback before allocating any layout
    if area.height < MIN_FLEX_VIZ_HEIGHT || area.width < MIN_FLEX_VIZ_WIDTH {
        render_muted_centered(area, buf, "Terminal too small for flex visualization.");
        return;
    }

    // Flex container: render the full visualization
    render_flex_viz(area, buf, layout);
}

// ── Flex visualization ────────────────────────────────────────────────────────

fn render_flex_viz(area: Rect, buf: &mut Buffer, layout: &fdemon_core::widget_tree::LayoutInfo) {
    let direction = layout.direction.unwrap_or(Axis::Vertical);
    let cross_align = layout
        .cross_axis_alignment
        .unwrap_or(CrossAxisAlignment::Center);
    let main_align = layout
        .main_axis_alignment
        .unwrap_or(MainAxisAlignment::Start);
    let widget_name = layout
        .description
        .as_deref()
        .unwrap_or(if direction == Axis::Vertical {
            "Column"
        } else {
            "Row"
        });
    let children = &layout.children;
    let total_flex = total_flex(children);

    // Outer bordered block with combined main-axis + cross-axis label in title
    let title = flex_axis_title(direction, main_align, cross_align);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(palette::BORDER_DIM))
        .title(ratatui::text::Span::styled(
            title,
            Style::default().fg(palette::ACCENT_DIM),
        ));

    let inner = block.inner(area);
    block.render(area, buf);

    if inner.height == 0 || inner.width == 0 {
        return;
    }

    // Minimum size guard inside inner
    if inner.height < MIN_FLEX_VIZ_HEIGHT || inner.width < MIN_FLEX_VIZ_WIDTH {
        render_muted_centered(inner, buf, "Terminal too small for flex visualization.");
        return;
    }

    match direction {
        Axis::Vertical => {
            render_vertical_flex(inner, buf, widget_name, children, total_flex, layout)
        }
        Axis::Horizontal => render_horizontal_flex(
            inner,
            buf,
            widget_name,
            children,
            total_flex,
            main_align,
            layout,
        ),
    }
}

// ── Vertical (Column) rendering ───────────────────────────────────────────────

fn render_vertical_flex(
    area: Rect,
    buf: &mut Buffer,
    widget_name: &str,
    children: &[FlexChild],
    total_flex: u32,
    layout: &fdemon_core::widget_tree::LayoutInfo,
) {
    // Reserve the right side for the main-axis indicator strip
    if area.width <= MAIN_AXIS_STRIP_WIDTH {
        render_muted_centered(area, buf, "Terminal too small for flex visualization.");
        return;
    }
    let content_width = area.width - MAIN_AXIS_STRIP_WIDTH;
    let strip_x = area.x + content_width;

    // ── Layout sections ───────────────────────────────────────────────────────
    // header (1 row) + child boxes (N * CHILD_BOX_HEIGHT rows) + footer (1 row)
    let n_children = children.len().max(1) as u16;
    let children_height = n_children * CHILD_BOX_HEIGHT;

    // Split content area vertically: header / children / footer
    let chunks = Layout::vertical([
        Constraint::Length(1), // header
        Constraint::Length(children_height.min(area.height.saturating_sub(2))), // child boxes
        Constraint::Length(1), // footer
        Constraint::Min(0),    // absorber
    ])
    .split(Rect {
        x: area.x,
        y: area.y,
        width: content_width,
        height: area.height,
    });

    let header_area = chunks[0];
    let children_area = chunks[1];
    let footer_area = chunks[2];

    // ── Header row ────────────────────────────────────────────────────────────
    render_header_row(header_area, buf, widget_name, total_flex, content_width);

    // ── Child boxes ───────────────────────────────────────────────────────────
    render_child_boxes_vertical(children_area, buf, children, content_width);

    // ── Main-axis strip (right side) ──────────────────────────────────────────
    let strip_area = Rect {
        x: strip_x,
        y: area.y,
        width: MAIN_AXIS_STRIP_WIDTH,
        height: area.height,
    };
    render_main_axis_strip_vertical(strip_area, buf);

    // ── Footer row ────────────────────────────────────────────────────────────
    render_footer_row(footer_area, buf, layout, content_width);
}

fn render_header_row(area: Rect, buf: &mut Buffer, widget_name: &str, total_flex: u32, width: u16) {
    if area.height == 0 {
        return;
    }
    let flex_label = format!("Total Flex: {total_flex}");
    let name_max = width.saturating_sub(flex_label.len() as u16 + 2) as usize;
    let name_trunc: String = widget_name.chars().take(name_max).collect();

    // Widget name on the left
    buf.set_string(
        area.x,
        area.y,
        &name_trunc,
        Style::default()
            .fg(palette::ACCENT)
            .add_modifier(ratatui::style::Modifier::BOLD),
    );

    // Total flex on the right
    let right_x = area.x + width.saturating_sub(flex_label.len() as u16);
    if right_x > area.x + name_trunc.len() as u16 {
        buf.set_string(
            right_x,
            area.y,
            &flex_label,
            Style::default().fg(palette::TEXT_SECONDARY),
        );
    }
}

fn render_child_boxes_vertical(area: Rect, buf: &mut Buffer, children: &[FlexChild], width: u16) {
    if area.height == 0 || width == 0 {
        return;
    }

    let mut y = area.y;

    if children.is_empty() {
        // Placeholder when there are no children yet
        if y < area.bottom() {
            buf.set_string(
                area.x,
                y,
                "(no children)",
                Style::default().fg(palette::TEXT_MUTED),
            );
        }
        return;
    }

    for (i, child) in children.iter().enumerate() {
        if y >= area.bottom() {
            break;
        }

        let is_last = i == children.len() - 1;
        let box_height = CHILD_BOX_HEIGHT.min(area.bottom().saturating_sub(y));

        draw_child_box(
            Rect {
                x: area.x,
                y,
                width,
                height: box_height,
            },
            buf,
            child,
            i + 1,
            is_last,
        );

        y += box_height;
    }
}

/// Draw a single child box.
///
/// Structure (for non-last child):
/// ```text
/// ┌──────────────────────────────────────────────────────────────┐
/// │  w=180  h=341                                                │
/// │     [Container] flex=0 fit=loose                             │
/// ├──────────────────────────────────────────────────────────────┤
/// ```
///
/// The last child uses `└─` for the bottom border instead of `├─`.
fn draw_child_box(area: Rect, buf: &mut Buffer, child: &FlexChild, index: usize, is_last: bool) {
    if area.height == 0 || area.width < 4 {
        return;
    }

    let style_border = Style::default().fg(palette::BORDER_DIM);
    let style_label = Style::default().fg(palette::TEXT_PRIMARY);
    let style_name = Style::default().fg(palette::STATUS_INDIGO);

    let inner_width = area.width.saturating_sub(2) as usize;

    // Row 0: top border ┌──...──┐ or ├──...──┤ (continuation from previous)
    if area.y < area.bottom() {
        let top_left = if index == 1 { '\u{250C}' } else { '\u{251C}' }; // ┌ or ├
        let top_right = if index == 1 { '\u{2510}' } else { '\u{2524}' }; // ┐ or ┤
        draw_box_line(
            buf,
            area.x,
            area.y,
            area.width,
            top_left,
            '\u{2500}',
            top_right,
            style_border,
        );
    }

    // Row 1: size label │  w=W  h=H  │
    if area.y + 1 < area.bottom() {
        let size_str = if let Some(ref s) = child.size {
            format!("  w={:.0}  h={:.0}", s.width, s.height)
        } else {
            "  (unmeasured)".to_string()
        };
        let padded = pad_to_width(&size_str, inner_width);
        let row = format!("\u{2502}{padded}\u{2502}"); // │...│
        buf.set_string(area.x, area.y + 1, &row, style_label);
    }

    // Row 2: child info │     [Name] flex=N fit=T │
    if area.y + 2 < area.bottom() {
        let flex_n = child.flex_factor.unwrap_or(0);
        let fit_label = fit_short_label(child.flex_fit.unwrap_or(FlexFit::Loose));
        let info_str = format!("     [{}] flex={} fit={}", child.name, flex_n, fit_label);
        let padded = pad_to_width(&info_str, inner_width);
        let row = format!("\u{2502}{padded}\u{2502}"); // │...│
        buf.set_string(area.x, area.y + 2, &row, style_name);
    }

    // Row 3: bottom border └──...──┘ or ├──...──┤
    if area.y + 3 < area.bottom() {
        let (bot_left, bot_right) = if is_last {
            ('\u{2514}', '\u{2518}') // └ ┘
        } else {
            ('\u{251C}', '\u{2524}') // ├ ┤
        };
        draw_box_line(
            buf,
            area.x,
            area.y + 3,
            area.width,
            bot_left,
            '\u{2500}',
            bot_right,
            style_border,
        );
    }
}

/// Fill a line with `left + (width-2 × mid) + right` characters.
#[allow(clippy::too_many_arguments)]
fn draw_box_line(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    width: u16,
    left: char,
    mid: char,
    right: char,
    style: Style,
) {
    if width == 0 {
        return;
    }
    buf.set_string(x, y, left.to_string(), style);
    if width > 2 {
        let fill: String = std::iter::repeat_n(mid, (width - 2) as usize).collect();
        buf.set_string(x + 1, y, &fill, style);
    }
    if width > 1 {
        buf.set_string(x + width - 1, y, right.to_string(), style);
    }
}

/// Pad or truncate `s` to exactly `width` chars (ASCII).
fn pad_to_width(s: &str, width: usize) -> String {
    let char_count = s.chars().count();
    if char_count >= width {
        s.chars().take(width).collect()
    } else {
        let mut out = s.to_string();
        out.extend(std::iter::repeat_n(' ', width - char_count));
        out
    }
}

/// Render the vertical main-axis strip on the right side of the content area.
///
/// Shows only `▲` at the top and `▼` at the bottom. Textual labels (previously
/// rendered as vertical letter stacks) have been moved to the block title via
/// [`flex_axis_title`].
fn render_main_axis_strip_vertical(area: Rect, buf: &mut Buffer) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let style = Style::default().fg(palette::TEXT_MUTED);

    // Arrow at the top
    if area.y < area.bottom() {
        buf.set_string(area.x, area.y, MAIN_AXIS_ARROW_UP.to_string(), style);
    }

    // Arrow at the bottom
    if area.height >= 2 && area.y + area.height - 1 < area.bottom() {
        buf.set_string(
            area.x,
            area.y + area.height - 1,
            MAIN_AXIS_ARROW_DOWN.to_string(),
            style,
        );
    }
}

fn render_footer_row(
    area: Rect,
    buf: &mut Buffer,
    layout: &fdemon_core::widget_tree::LayoutInfo,
    width: u16,
) {
    if area.height == 0 {
        return;
    }

    let constraints_str = layout.constraints.as_ref().map(|c| {
        use crate::widgets::devtools::inspector::layout_panel::format_constraint_value;
        format!(
            "constraints: 0 \u{2264} w \u{2264} {}  0 \u{2264} h \u{2264} {}",
            format_constraint_value(c.max_width),
            format_constraint_value(c.max_height),
        )
    });

    let size_str = layout.size.as_ref().map(|s| {
        format!("size: {:.0}\u{00D7}{:.0}", s.width, s.height) // ×
    });

    let footer = match (constraints_str, size_str) {
        (Some(c), Some(s)) => format!("{c}   {s}"),
        (Some(c), None) => c,
        (None, Some(s)) => s,
        (None, None) => return,
    };

    let trunc: String = footer.chars().take(width as usize).collect();
    buf.set_string(
        area.x,
        area.y,
        &trunc,
        Style::default().fg(palette::TEXT_MUTED),
    );
}

// ── Horizontal (Row) rendering ────────────────────────────────────────────────

fn render_horizontal_flex(
    area: Rect,
    buf: &mut Buffer,
    widget_name: &str,
    children: &[FlexChild],
    total_flex: u32,
    main_align: MainAxisAlignment,
    layout: &fdemon_core::widget_tree::LayoutInfo,
) {
    // Reserve the bottom row for the main-axis indicator strip
    if area.height < MIN_HORIZONTAL_FLEX_HEIGHT {
        render_muted_centered(area, buf, "Terminal too small for flex visualization.");
        return;
    }
    let strip_height: u16 = 1; // single bottom row for horizontal strip
    let content_height = area.height.saturating_sub(strip_height + 2); // header + content + footer
    let strip_y = area.y + area.height - strip_height;

    // Header row
    let header_area = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: 1,
    };
    render_header_row(header_area, buf, widget_name, total_flex, area.width);

    // Child boxes arranged horizontally
    let children_area = Rect {
        x: area.x,
        y: area.y + 1,
        width: area.width,
        height: content_height.min(CHILD_BOX_HEIGHT),
    };
    render_child_boxes_horizontal(children_area, buf, children);

    // Main-axis strip (bottom row)
    let strip_area = Rect {
        x: area.x,
        y: strip_y,
        width: area.width,
        height: strip_height,
    };
    render_main_axis_strip_horizontal(strip_area, buf, main_align);

    // Footer row above the strip
    if strip_y > area.y + 2 {
        let footer_area = Rect {
            x: area.x,
            y: strip_y - 1,
            width: area.width,
            height: 1,
        };
        render_footer_row(footer_area, buf, layout, area.width);
    }
}

fn render_child_boxes_horizontal(area: Rect, buf: &mut Buffer, children: &[FlexChild]) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    if children.is_empty() {
        buf.set_string(
            area.x,
            area.y,
            "(no children)",
            Style::default().fg(palette::TEXT_MUTED),
        );
        return;
    }

    // Each box gets equal width
    let n = children.len() as u16;
    let box_width = (area.width / n).max(10);

    let mut x = area.x;
    for (i, child) in children.iter().enumerate() {
        if x >= area.x + area.width {
            break;
        }
        let is_last = i == children.len() - 1;
        let actual_w = if is_last {
            (area.x + area.width).saturating_sub(x)
        } else {
            box_width
        };

        draw_child_box_horizontal(
            Rect {
                x,
                y: area.y,
                width: actual_w,
                height: area.height,
            },
            buf,
            child,
            i + 1,
            is_last,
        );
        x += actual_w;
    }
}

/// Draw a horizontal child box (column-oriented within the row layout).
fn draw_child_box_horizontal(
    area: Rect,
    buf: &mut Buffer,
    child: &FlexChild,
    index: usize,
    is_last: bool,
) {
    if area.height == 0 || area.width < 3 {
        return;
    }

    let style_border = Style::default().fg(palette::BORDER_DIM);
    let style_label = Style::default().fg(palette::TEXT_PRIMARY);
    let style_name = Style::default().fg(palette::STATUS_INDIGO);

    let inner_width = area.width.saturating_sub(1) as usize; // no right border for non-last (shared)
    let right_char = '\u{2502}'; // │ always (same for last and non-last in horizontal layout)

    // Top border: ┌──...──┐ or continuation
    if area.y < area.bottom() {
        let top_left = if index == 1 { '\u{250C}' } else { '\u{252C}' }; // ┌ or ┬
        let top_right = if is_last { '\u{2510}' } else { '\u{2500}' }; // ┐ or ─ (border continues)
        draw_box_line(
            buf,
            area.x,
            area.y,
            area.width,
            top_left,
            '\u{2500}',
            top_right,
            style_border,
        );
    }

    // Size row
    if area.y + 1 < area.bottom() {
        let size_str = if let Some(ref s) = child.size {
            format!(" w={:.0}", s.width)
        } else {
            " (?)".to_string()
        };
        let padded = pad_to_width(&size_str, inner_width);
        let row = format!("{padded}{}", right_char);
        buf.set_string(area.x, area.y + 1, &row, style_label);
    }

    // Child info row
    if area.y + 2 < area.bottom() {
        let flex_n = child.flex_factor.unwrap_or(0);
        let fit_label = fit_short_label(child.flex_fit.unwrap_or(FlexFit::Loose));
        let name: String = child
            .name
            .chars()
            .take(inner_width.saturating_sub(2))
            .collect();
        let info_str = format!(" {name} f{flex_n}{fit_label}");
        let padded = pad_to_width(&info_str, inner_width);
        let row = format!("{padded}{}", right_char);
        buf.set_string(area.x, area.y + 2, &row, style_name);
    }

    // Bottom border
    if area.y + 3 < area.bottom() {
        let bot_left = if index == 1 { '\u{2514}' } else { '\u{2534}' }; // └ or ┴
        let bot_right = if is_last { '\u{2518}' } else { '\u{2500}' }; // ┘ or ─
        draw_box_line(
            buf,
            area.x,
            area.y + 3,
            area.width,
            bot_left,
            '\u{2500}',
            bot_right,
            style_border,
        );
    }
}

/// Render the horizontal main-axis strip on the bottom row.
fn render_main_axis_strip_horizontal(area: Rect, buf: &mut Buffer, main_align: MainAxisAlignment) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let align_label = main_axis_value(main_align);
    let text = format!(
        "{} Main Axis ({}) {}",
        MAIN_AXIS_ARROW_LEFT, align_label, MAIN_AXIS_ARROW_RIGHT
    );
    let trunc: String = text.chars().take(area.width as usize).collect();
    buf.set_string(
        area.x,
        area.y,
        &trunc,
        Style::default().fg(palette::TEXT_MUTED),
    );
}

// ── Shared helpers ────────────────────────────────────────────────────────────

/// Render a muted, roughly-centered single-line message.
fn render_muted_centered(area: Rect, buf: &mut Buffer, text: &str) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let y = area.y + area.height / 2;
    if y >= area.bottom() {
        return;
    }
    let text_len = text.chars().count() as u16;
    let x = area.x + area.width.saturating_sub(text_len) / 2;
    buf.set_string(x, y, text, Style::default().fg(palette::TEXT_MUTED));
}

/// Return `"tight"` or `"loose"` label for a [`FlexFit`].
fn fit_short_label(fit: FlexFit) -> &'static str {
    match fit {
        FlexFit::Tight => "tight",
        FlexFit::Loose => "loose",
    }
}

/// Build the combined main-axis and cross-axis label for the outer border title.
///
/// Produces a string like `" Main ↕ start  │  Cross Axis: stretch "` for a
/// vertical flex, or `" Main ↔ center  │  Cross Axis: center "` for horizontal.
/// The leading and trailing spaces preserve the title padding style.
fn flex_axis_title(
    direction: Axis,
    main_align: MainAxisAlignment,
    cross_align: CrossAxisAlignment,
) -> String {
    let arrow = match direction {
        Axis::Vertical => "↕",
        Axis::Horizontal => "↔",
    };
    format!(
        " Main {} {}  │  Cross Axis: {} ",
        arrow,
        main_axis_value(main_align),
        cross_axis_value(cross_align),
    )
}

fn cross_axis_value(a: CrossAxisAlignment) -> &'static str {
    match a {
        CrossAxisAlignment::Start => "start",
        CrossAxisAlignment::End => "end",
        CrossAxisAlignment::Center => "center",
        CrossAxisAlignment::Stretch => "stretch",
        CrossAxisAlignment::Baseline => "baseline",
    }
}

fn main_axis_value(a: MainAxisAlignment) -> &'static str {
    match a {
        MainAxisAlignment::Start => "start",
        MainAxisAlignment::End => "end",
        MainAxisAlignment::Center => "center",
        MainAxisAlignment::SpaceBetween => "spaceBetween",
        MainAxisAlignment::SpaceAround => "spaceAround",
        MainAxisAlignment::SpaceEvenly => "spaceEvenly",
    }
}

/// Sum the flex factors of all children.
fn total_flex(children: &[FlexChild]) -> u32 {
    children.iter().map(|c| c.flex_factor.unwrap_or(0)).sum()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use fdemon_app::state::{DetailsTab, InspectorState};
    use fdemon_core::widget_tree::{
        Axis, BoxConstraints, CrossAxisAlignment, FlexChild, FlexFit, LayoutInfo,
        MainAxisAlignment, MainAxisSize, WidgetSize,
    };
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    use super::{render, MAIN_AXIS_STRIP_WIDTH};

    // Path: tests → super (flex_explorer_tab) → super (details) → super (inspector) → test_helpers
    use super::super::super::test_helpers::collect_buf_text;

    // ── Test helper ───────────────────────────────────────────────────────────

    /// Render the flex explorer tab for the given inspector state into a buffer
    /// of the specified `(width, height)` and return the buffer.
    fn render_flex_explorer_tab(state: &InspectorState, (width, height): (u16, u16)) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        render(buf.area, &mut buf, state);
        buf
    }

    fn buffer_to_string(buf: &Buffer) -> String {
        collect_buf_text(buf, buf.area.width, buf.area.height)
    }

    // ── State tests ───────────────────────────────────────────────────────────

    #[test]
    fn flex_explorer_loading_state() {
        let state = InspectorState {
            details_open: true,
            details_tab: DetailsTab::FlexExplorer,
            layout_loading: true,
            ..Default::default()
        };
        let buf = render_flex_explorer_tab(&state, (60, 12));
        assert!(
            buffer_to_string(&buf).contains("Loading"),
            "Loading state should contain 'Loading'"
        );
    }

    #[test]
    fn flex_explorer_no_layout_data() {
        let state = InspectorState {
            details_open: true,
            details_tab: DetailsTab::FlexExplorer,
            // layout is None and layout_loading is false
            ..Default::default()
        };
        let buf = render_flex_explorer_tab(&state, (60, 12));
        let text = buffer_to_string(&buf);
        assert!(
            text.contains("No layout data") || text.contains("press Enter"),
            "No-layout state should show fetch hint, got: {text:?}"
        );
    }

    #[test]
    fn flex_explorer_non_flex_widget_shows_explanation() {
        let state = InspectorState {
            details_open: true,
            details_tab: DetailsTab::FlexExplorer,
            layout: Some(LayoutInfo {
                description: Some("Container".into()),
                // direction == None; children == empty.
                ..Default::default()
            }),
            ..Default::default()
        };
        let buf = render_flex_explorer_tab(&state, (60, 12));
        assert!(
            buffer_to_string(&buf).contains("not a Row, Column, or Flex"),
            "Non-flex widget should show explanation"
        );
    }

    #[test]
    fn flex_explorer_renders_column_with_two_children() {
        let state = InspectorState {
            details_open: true,
            details_tab: DetailsTab::FlexExplorer,
            layout: Some(LayoutInfo {
                description: Some("Column".into()),
                direction: Some(Axis::Vertical),
                main_axis_alignment: Some(MainAxisAlignment::Start),
                cross_axis_alignment: Some(CrossAxisAlignment::Stretch),
                main_axis_size: Some(MainAxisSize::Max),
                children: vec![
                    FlexChild {
                        name: "Container".into(),
                        size: Some(WidgetSize {
                            width: 180.0,
                            height: 341.0,
                        }),
                        flex_factor: None,
                        ..Default::default()
                    },
                    FlexChild {
                        name: "Expanded".into(),
                        size: Some(WidgetSize {
                            width: 180.0,
                            height: 189.0,
                        }),
                        flex_factor: Some(1),
                        flex_fit: Some(FlexFit::Tight),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }),
            ..Default::default()
        };
        let buf = render_flex_explorer_tab(&state, (80, 24));
        let s = buffer_to_string(&buf);
        assert!(s.contains("Column"), "Should contain 'Column', got: {s:?}");
        assert!(
            s.contains("Cross Axis") && s.contains("stretch"),
            "Should contain cross-axis label, got: {s:?}"
        );
        assert!(
            s.contains("Container"),
            "Should contain child name 'Container', got: {s:?}"
        );
        assert!(
            s.contains("Expanded"),
            "Should contain child name 'Expanded', got: {s:?}"
        );
        assert!(s.contains("flex=1"), "Should contain 'flex=1', got: {s:?}");
        assert!(
            s.contains("fit=tight"),
            "Should contain 'fit=tight', got: {s:?}"
        );
        assert!(
            s.contains("Total Flex: 1"),
            "Should contain 'Total Flex: 1', got: {s:?}"
        );
    }

    #[test]
    fn flex_explorer_too_small_fallback() {
        let state = InspectorState {
            details_open: true,
            details_tab: DetailsTab::FlexExplorer,
            layout: Some(LayoutInfo {
                description: Some("Column".into()),
                direction: Some(Axis::Vertical),
                children: vec![FlexChild {
                    name: "A".into(),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        // Below MIN_FLEX_VIZ_HEIGHT (12).
        let buf = render_flex_explorer_tab(&state, (60, 5));
        assert!(
            buffer_to_string(&buf).contains("too small"),
            "Below minimum height should show 'too small' message"
        );
    }

    #[test]
    fn flex_explorer_renders_row_with_children() {
        let state = InspectorState {
            details_open: true,
            details_tab: DetailsTab::FlexExplorer,
            layout: Some(LayoutInfo {
                description: Some("Row".into()),
                direction: Some(Axis::Horizontal),
                main_axis_alignment: Some(MainAxisAlignment::Center),
                cross_axis_alignment: Some(CrossAxisAlignment::Center),
                children: vec![
                    FlexChild {
                        name: "Text".into(),
                        size: Some(WidgetSize {
                            width: 80.0,
                            height: 20.0,
                        }),
                        flex_factor: None,
                        ..Default::default()
                    },
                    FlexChild {
                        name: "Icon".into(),
                        size: Some(WidgetSize {
                            width: 24.0,
                            height: 24.0,
                        }),
                        flex_factor: None,
                        flex_fit: Some(FlexFit::Loose),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }),
            ..Default::default()
        };
        let buf = render_flex_explorer_tab(&state, (80, 24));
        let s = buffer_to_string(&buf);
        assert!(s.contains("Row"), "Should contain 'Row', got: {s:?}");
        assert!(
            s.contains("Cross Axis") && s.contains("center"),
            "Should contain cross-axis label, got: {s:?}"
        );
        // Row variant may truncate child names — just check at least one is visible
        assert!(
            s.contains("Text") || s.contains("Icon"),
            "Should contain at least one child name, got: {s:?}"
        );
    }

    #[test]
    fn flex_explorer_constraints_in_footer() {
        let state = InspectorState {
            details_tab: DetailsTab::FlexExplorer,
            layout: Some(LayoutInfo {
                description: Some("Column".into()),
                direction: Some(Axis::Vertical),
                cross_axis_alignment: Some(CrossAxisAlignment::Start),
                constraints: Some(BoxConstraints {
                    min_width: 0.0,
                    max_width: 392.0,
                    min_height: 0.0,
                    max_height: 872.0,
                }),
                size: Some(WidgetSize {
                    width: 180.0,
                    height: 872.0,
                }),
                children: vec![FlexChild {
                    name: "Child".into(),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let buf = render_flex_explorer_tab(&state, (80, 24));
        let s = buffer_to_string(&buf);
        // Footer should contain constraint or size info
        assert!(
            s.contains("constraints") || s.contains("size"),
            "Footer should contain constraint/size info, got: {s:?}"
        );
    }

    #[test]
    fn flex_explorer_zero_area_no_panic() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 0, 0));
        let state = InspectorState::default();
        render(buf.area, &mut buf, &state);
        // Should not panic
    }

    #[test]
    fn flex_explorer_single_row_no_panic() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 1));
        let state = InspectorState::default();
        render(buf.area, &mut buf, &state);
        // Should not panic
    }

    #[test]
    fn flex_explorer_total_flex_zero_when_no_flex_children() {
        let state = InspectorState {
            layout: Some(LayoutInfo {
                description: Some("Column".into()),
                direction: Some(Axis::Vertical),
                cross_axis_alignment: Some(CrossAxisAlignment::Stretch),
                children: vec![
                    FlexChild {
                        name: "A".into(),
                        flex_factor: None,
                        ..Default::default()
                    },
                    FlexChild {
                        name: "B".into(),
                        flex_factor: None,
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }),
            ..Default::default()
        };
        let buf = render_flex_explorer_tab(&state, (80, 24));
        let s = buffer_to_string(&buf);
        assert!(
            s.contains("Total Flex: 0"),
            "All fixed children → Total Flex: 0, got: {s:?}"
        );
    }

    // ── New tests (C3, C1, C1 strip) ─────────────────────────────────────────

    #[test]
    fn render_centers_too_small_message_in_panel_not_buffer() {
        // Buffer is much larger than the tab pane; pane is smaller than min dims
        let mut buf = Buffer::empty(Rect::new(0, 0, 100, 50));
        let area = Rect::new(20, 10, 8, 4); // below MIN_FLEX_VIZ_WIDTH / HEIGHT
        let state = InspectorState {
            layout: Some(LayoutInfo {
                description: Some("Column".into()),
                direction: Some(Axis::Vertical),
                children: vec![FlexChild {
                    name: "A".into(),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        render(area, &mut buf, &state);

        // The "Terminal too small" message must land within `area`, not buffer centre
        let mut found_in_area = false;
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                let ch = buf
                    .cell((x, y))
                    .map(|c| c.symbol().to_string())
                    .unwrap_or_default();
                if ch.contains('T') || ch.contains('e') {
                    found_in_area = true;
                }
            }
        }
        assert!(
            found_in_area,
            "message must render inside `area`, not the full buffer"
        );

        // And NOT outside area in the buffer centre (where buf.area centre would be ~50,25)
        let buf_centre_ch = buf
            .cell((50, 25))
            .map(|c| c.symbol().to_string())
            .unwrap_or_default();
        assert!(
            buf_centre_ch.trim().is_empty(),
            "no message should render at the full-buffer centre"
        );
    }

    #[test]
    fn vertical_flex_title_contains_main_and_cross_axis_labels() {
        let state = InspectorState {
            layout: Some(LayoutInfo {
                description: Some("Column".into()),
                direction: Some(Axis::Vertical),
                main_axis_alignment: Some(MainAxisAlignment::SpaceBetween),
                cross_axis_alignment: Some(CrossAxisAlignment::Stretch),
                main_axis_size: Some(MainAxisSize::Max),
                children: vec![FlexChild {
                    name: "Child".into(),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 20));
        let area = Rect::new(0, 0, 80, 20);
        render(area, &mut buf, &state);

        let text: String = (0..buf.area.width)
            .filter_map(|x| buf.cell((x, 0)))
            .map(|c| c.symbol().to_string())
            .collect();
        assert!(
            text.contains("Main") && text.contains("spaceBetween"),
            "title must include main-axis label: got `{text}`"
        );
        assert!(
            text.contains("Cross") && text.contains("stretch"),
            "title must include cross-axis label: got `{text}`"
        );
    }

    #[test]
    fn vertical_main_axis_strip_no_longer_renders_letter_stacks() {
        let state = InspectorState {
            layout: Some(LayoutInfo {
                description: Some("Column".into()),
                direction: Some(Axis::Vertical),
                main_axis_alignment: Some(MainAxisAlignment::Start),
                cross_axis_alignment: Some(CrossAxisAlignment::Center),
                children: vec![FlexChild {
                    name: "Child".into(),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 20));
        let area = Rect::new(0, 0, 80, 20);
        render(area, &mut buf, &state);

        // Strip is at right edge (inside the outer block border). The block border
        // itself is 1 cell, so inner area is (1,1) to (78,18). The strip occupies
        // the rightmost MAIN_AXIS_STRIP_WIDTH columns of the inner area.
        // inner_right = 79 (exclusive). strip_x_start = 79 - 3 = 76.
        // But we need to account for the outer block: inner x goes from 1 to 78,
        // and strip_x within inner = inner_width - MAIN_AXIS_STRIP_WIDTH.
        // Let's just check the rightmost non-border columns in rows 1..19.
        let strip_x_start = area
            .right()
            .saturating_sub(1)
            .saturating_sub(MAIN_AXIS_STRIP_WIDTH);
        let mut letters_in_strip = 0u32;
        for y in (area.y + 2)..(area.bottom().saturating_sub(1)) {
            for x in strip_x_start..(area.right().saturating_sub(1)) {
                let s = buf
                    .cell((x, y))
                    .map(|c| c.symbol().to_string())
                    .unwrap_or_default();
                if s.chars().any(|c| c.is_ascii_alphabetic()) {
                    letters_in_strip += 1;
                }
            }
        }
        assert_eq!(
            letters_in_strip, 0,
            "no letters should appear in the side strip"
        );
    }
}
