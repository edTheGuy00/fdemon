//! Details panel rendering for the widget inspector.
//!
//! Shows the selected node's display name, properties list,
//! and creation location when a node is selected in the tree.

use fdemon_core::widget_tree::DiagnosticsNode;
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Widget},
};

use super::short_path;
use super::truncate_str;
use super::WidgetInspector;
use crate::theme::palette;

impl WidgetInspector<'_> {
    /// Render the details panel showing properties of the selected node.
    pub(super) fn render_details(
        &self,
        area: Rect,
        buf: &mut Buffer,
        visible: &[(&DiagnosticsNode, usize)],
        selected: usize,
    ) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(palette::BORDER_DIM))
            .title(Span::styled(
                " Details ",
                Style::default().fg(palette::ACCENT_DIM),
            ))
            .title_alignment(Alignment::Left);
        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height == 0 || inner.width == 0 {
            return;
        }

        let Some((node, _)) = visible.get(selected) else {
            return;
        };

        let mut y = inner.y;

        // Widget type / description
        let desc = node.display_name();
        let desc_trunc = truncate_str(desc, inner.width.saturating_sub(2) as usize);
        buf.set_string(
            inner.x + 1,
            y,
            desc_trunc,
            Style::default()
                .fg(palette::ACCENT)
                .add_modifier(Modifier::BOLD),
        );

        if y + 1 < inner.bottom() {
            y += 2; // one gap line after description
        } else {
            return;
        }

        // Properties section
        if !node.properties.is_empty() {
            if y < inner.bottom() {
                buf.set_string(
                    inner.x + 1,
                    y,
                    "Properties:",
                    Style::default().fg(palette::STATUS_YELLOW),
                );
                y += 1;
            }

            for prop in &node.properties {
                if y >= inner.bottom() {
                    break;
                }
                let name = prop.name.as_deref().unwrap_or("?");
                let value = &prop.description;
                let prop_line = format!("  {name}: {value}");
                let prop_trunc = truncate_str(&prop_line, inner.width.saturating_sub(2) as usize);
                buf.set_string(
                    inner.x + 1,
                    y,
                    prop_trunc,
                    Style::default().fg(palette::TEXT_PRIMARY),
                );
                y += 1;
            }
        }

        // Spacer before location
        if y < inner.bottom() {
            y += 1;
        } else {
            return;
        }

        // Creation location section
        if let Some(loc) = &node.creation_location {
            if y < inner.bottom() {
                buf.set_string(
                    inner.x + 1,
                    y,
                    "Location:",
                    Style::default().fg(palette::STATUS_YELLOW),
                );
                y += 1;
            }
            if y < inner.bottom() {
                let short = short_path(&loc.file);
                let path = format!("  {}:{}", short, loc.line);
                let path_trunc = truncate_str(&path, inner.width.saturating_sub(2) as usize);
                buf.set_string(
                    inner.x + 1,
                    y,
                    path_trunc,
                    Style::default().fg(palette::STATUS_BLUE),
                );
            }
        }
    }
}
