//! Tree panel rendering for the widget inspector.
//!
//! Contains the per-row tree view logic including viewport scrolling,
//! node styling, and the scroll indicator.

use fdemon_app::{MouseAction, MouseRect};
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
use crate::render::MouseCtx;
use crate::theme::palette;

impl WidgetInspector<'_> {
    /// Render the widget tree panel, optionally recording click regions.
    ///
    /// When `ctx` is `Some`, two regions are registered per visible row:
    ///
    /// 1. **Row region** — the full width of `tree_inner` at row `y`.
    ///    Action: `Emit(Message::DevToolsInspectorSelectRow { index: vis_index })`.
    /// 2. **Glyph region** — a 1-cell wide rect at the leading glyph position
    ///    (`▶` / `▼` / `●`).
    ///    Action: `Emit(Message::DevToolsInspectorToggleNode { index: vis_index })`.
    ///
    /// The glyph region is pushed *after* the row region so that the registry's
    /// last-pushed-wins-at-same-z invariant (Phase 3 unit test
    /// `test_last_pushed_wins_at_same_z`) makes the glyph region win on the
    /// glyph cell when both rects overlap.
    ///
    /// When `ctx` is `None`, this function is identical to the original
    /// `render_tree_panel` — no regions are registered.
    pub(super) fn render_tree_panel_inner(
        &self,
        area: Rect,
        buf: &mut Buffer,
        visible: &[(&DiagnosticsNode, usize)],
        selected: usize,
        mut ctx: Option<&mut MouseCtx<'_>>,
    ) {
        // Block border for tree area
        let tree_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(palette::BORDER_DIM))
            .title(Span::styled(
                " Widget Tree ",
                Style::default().fg(palette::ACCENT_DIM),
            ))
            .title_alignment(Alignment::Left);
        let tree_inner = tree_block.inner(area);
        tree_block.render(area, buf);

        if tree_inner.height == 0 || tree_inner.width == 0 {
            return;
        }

        let viewport_height = tree_inner.height as usize;
        let total = visible.len();
        let (start, end) = self.visible_viewport_range(viewport_height, total);

        for (offset, (node, depth)) in visible[start..end].iter().enumerate() {
            let y = tree_inner.y + offset as u16;
            if y >= tree_inner.bottom() {
                break;
            }

            let vis_index = start + offset;
            let is_selected = vis_index == selected;
            let is_user_code = node.is_user_code();

            // Build indent + expand icon + name
            let indent = "  ".repeat(*depth);
            let expand_icon = self.expand_icon(node);
            let name = node.display_name();
            let line = format!("{indent}{expand_icon} {name}");

            // Apply background across full row width for selected items
            if is_selected {
                let sel_bg = Style::default().bg(palette::SELECTED_ROW_BG);
                for x in tree_inner.x..tree_inner.right() {
                    if let Some(cell) = buf.cell_mut((x, y)) {
                        cell.set_style(sel_bg);
                    }
                }
            }

            let style = self.node_style(is_selected, is_user_code);

            // Truncate line to fit within available width
            let max_w = tree_inner.width as usize;
            let display_line = truncate_str(&line, max_w);
            buf.set_string(tree_inner.x, y, display_line, style);

            // Source location hint for selected user-code nodes
            if is_selected && is_user_code {
                if let Some(loc) = &node.creation_location {
                    let short = short_path(&loc.file);
                    let loc_text = format!(" ({}:{})", short, loc.line);
                    let used = display_line.len() as u16;
                    let remaining = tree_inner.width.saturating_sub(used);
                    if remaining > loc_text.len() as u16 {
                        buf.set_string(
                            tree_inner.x + used,
                            y,
                            &loc_text,
                            Style::default().fg(palette::TEXT_MUTED),
                        );
                    }
                }
            }

            // ── Phase 4: register click regions ──────────────────────────────
            //
            // Row region pushed first, then glyph region — so last-pushed-wins
            // at the glyph cell position.
            if let Some(c) = ctx.as_deref_mut() {
                use fdemon_app::message::Message;

                // Whole-row click region (left-click → select).
                let row_rect = MouseRect::new(tree_inner.x, y, tree_inner.width, 1);
                if !row_rect.is_empty() {
                    c.click(
                        row_rect,
                        MouseAction::emit(Message::DevToolsInspectorSelectRow { index: vis_index }),
                    );
                }

                // Glyph click region (left-click on ▶/▼/● → toggle).
                // Pushed AFTER the row region so the registry's last-pushed-wins-at-same-z
                // invariant makes the glyph rect win on overlap.
                //
                // depth-to-x conversion: checked_mul + checked_add guarantee we don't
                // silently saturate and place the glyph far off-screen. At depths above
                // ~32 767 (u16::MAX / 2) the tree is pathological; skip glyph registration.
                let Some(indent) = (*depth as u16).checked_mul(2) else {
                    continue; // depth overflows u16 — skip glyph for impossibly deep node
                };
                let Some(glyph_x) = tree_inner.x.checked_add(indent) else {
                    continue; // indent pushes glyph past u16 bounds — skip
                };
                if glyph_x >= tree_inner.right() {
                    continue; // glyph clipped past the right edge (normal viewport case)
                }
                let glyph_rect = MouseRect::new(glyph_x, y, 1, 1);
                c.click(
                    glyph_rect,
                    MouseAction::emit(Message::DevToolsInspectorToggleNode { index: vis_index }),
                );
            }
        }

        // Simple scroll indicator (right edge) if content overflows
        if total > viewport_height && viewport_height > 0 {
            let scroll_x = tree_inner.right().saturating_sub(1);
            // Top of scroll range indicator
            let thumb_y = tree_inner.y
                + ((selected * viewport_height).checked_div(total).unwrap_or(0) as u16)
                    .min(tree_inner.height.saturating_sub(1));
            if scroll_x < area.right() && thumb_y < tree_inner.bottom() {
                if let Some(cell) = buf.cell_mut((scroll_x, thumb_y)) {
                    cell.set_symbol("█").set_fg(palette::BORDER_DIM);
                }
            }
        }
    }

    // ── Node styling ──────────────────────────────────────────────────────────

    fn node_style(&self, is_selected: bool, is_user_code: bool) -> Style {
        let base = if is_user_code {
            Style::default().fg(palette::TEXT_PRIMARY) // User code: normal brightness
        } else {
            Style::default().fg(palette::TEXT_MUTED) // Framework code: dimmed
        };

        if is_selected {
            base.add_modifier(Modifier::BOLD)
        } else {
            base
        }
    }
}
