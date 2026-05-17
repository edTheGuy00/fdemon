//! Tree panel rendering for the widget inspector.
//!
//! Contains the per-row tree view logic including viewport scrolling,
//! node styling, and the scroll indicator.
//!
//! ## Rendering pipeline
//!
//! For each [`InspectorRow`] the renderer:
//!
//! 1. Fills the row background if selected.
//! 2. Draws vertical `│` guidelines at ancestor columns that still have siblings below.
//! 3. Draws a branch tick (`├─` for non-last child, `└─` for last child) at the
//!    depth-minus-one column.
//! 4. Draws a per-widget-type icon glyph at the row's own depth column.
//! 5. Writes the node name (and optional source-location hint for selected user-code rows).
//!
//! [`InspectorRow`]: fdemon_core::widget_tree::InspectorRow

use fdemon_app::{MouseAction, MouseRect};
use fdemon_core::widget_tree::{DiagnosticsNode, InspectorRow, RowGroup};
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

// ── Layout constants ─────────────────────────────────────────────────────────

/// Horizontal cells per depth level in the inspector tree.
///
/// Chosen as 2 cells: 1 for the guideline/branch-tick column, 1 for spacing
/// before the icon glyph at the next depth.
const TREE_INDENT_COLS: u16 = 2;

/// Compute the X offset (relative to `tree_inner.x`) of the glyph column for a
/// given depth level.
///
/// Each depth level is `TREE_INDENT_COLS` cells wide; the glyph for depth `d`
/// starts at column `d * TREE_INDENT_COLS`.
fn glyph_col(depth: usize) -> u16 {
    (depth as u16).saturating_mul(TREE_INDENT_COLS)
}

// ── Type-icon glyph table ────────────────────────────────────────────────────

/// Map a widget's runtime type to a single-cell Unicode glyph.
///
/// The table is a linear scan over `(&str, char)` pairs. The first prefix
/// match wins, so longer / more specific names must appear before shorter
/// ones.  All glyphs are exactly 1 cell wide (no East-Asian-wide characters,
/// no combining sequences).
fn glyph_for_widget(node: &DiagnosticsNode) -> char {
    let Some(widget_type) = node.widget_runtime_type() else {
        return fallback_glyph(node);
    };

    // Table: (prefix_or_exact_match, glyph)
    // Ordered from most-specific to least-specific.
    const GLYPH_TABLE: &[(&str, char)] = &[
        // Flex containers
        ("Row", '\u{25A6}'),    // ▦ grid-like
        ("Column", '\u{25A6}'), // ▦ grid-like
        ("Flex", '\u{25A6}'),   // ▦ grid-like
        // Box / spacing
        ("Container", '\u{25A3}'), // ▣ filled box
        ("Padding", '\u{25A3}'),   // ▣ filled box
        ("SizedBox", '\u{25A3}'),  // ▣ filled box
        // Stack / layering
        ("Stack", '\u{25A4}'), // ▤ layered
        // App shells
        ("MaterialApp", '\u{25A5}'),  // ▥ app
        ("CupertinoApp", '\u{25A5}'), // ▥ app
        ("Scaffold", '\u{25CB}'),     // ◯ shell frame
        // Text
        ("RichText", 'T'),
        ("Text", 'T'),
        // Media
        ("Image", '\u{25A8}'), // ▨ media
        ("Icon", '\u{25A8}'),  // ▨ media
        // Alignment / positioning
        ("Center", '+'),
        ("Align", '+'),
        ("Positioned", '+'),
        // Scrollable lists / grids
        ("ListView", '\u{2261}'),              // ≡ list
        ("GridView", '\u{2261}'),              // ≡ list
        ("SingleChildScrollView", '\u{2261}'), // ≡ list
        // Builders
        ("StreamBuilder", 'B'),
        ("ValueListenableBuilder", 'B'),
        ("Builder", 'B'),
        // Bloc providers (must come after "Builder" to avoid false match)
        ("MultiBlocProvider", '\u{25AA}'), // ▪ provider
        ("BlocProvider", '\u{25AA}'),      // ▪ provider
    ];

    for (prefix, glyph) in GLYPH_TABLE {
        if widget_type == *prefix || widget_type.starts_with(prefix) {
            return *glyph;
        }
    }

    fallback_glyph(node)
}

/// Fallback glyph: the first uppercase ASCII letter of the description, or `?`.
fn fallback_glyph(node: &DiagnosticsNode) -> char {
    node.description
        .chars()
        .find(|c| c.is_ascii_uppercase())
        .unwrap_or('?')
}

// ── WidgetInspector impl ─────────────────────────────────────────────────────

impl WidgetInspector<'_> {
    /// Render the widget tree panel, optionally recording click regions.
    ///
    /// When `ctx` is `Some`, two regions are registered per visible row:
    ///
    /// 1. **Row region** — the full width of `tree_inner` at row `y`.
    ///    Action: `Emit(Message::DevToolsInspectorSelectRow { index: vis_index })`.
    /// 2. **Glyph region** — a 1-cell wide rect at the leading glyph position.
    ///    Action: `Emit(Message::DevToolsInspectorToggleNode { index: vis_index })`.
    ///
    /// The glyph region is pushed *after* the row region so that the registry's
    /// last-pushed-wins-at-same-z invariant makes the glyph rect win on the
    /// glyph cell when both rects overlap.
    ///
    /// When `ctx` is `None`, this function is identical to the original
    /// `render_tree_panel` — no regions are registered.
    ///
    /// ## Signature note
    ///
    /// The `rows` slice is pre-built by `render_impl` (via `inspector_rows()`)
    /// so that `inspector_rows()` is called exactly once per render frame and
    /// the slice is threaded through both the tree renderer and the details
    /// renderer without a redundant rebuild.
    pub(super) fn render_tree_panel_inner(
        &self,
        area: Rect,
        buf: &mut Buffer,
        rows: &[InspectorRow<'_>],
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

        // `rows` is pre-built by the caller (render_impl via inspector_rows()) —
        // one call per frame, threaded through both the tree and details renderers.
        // INVARIANT: inspector_rows() is called exactly once per render frame.
        let total = rows.len();
        let viewport_height = tree_inner.height as usize;
        let (start, end) = self.visible_viewport_range(viewport_height, total);

        for (offset, row) in rows[start..end].iter().enumerate() {
            let y = tree_inner.y + offset as u16;
            if y >= tree_inner.bottom() {
                break;
            }

            let vis_index = start + offset;
            let is_selected = vis_index == selected;
            let node = row.node;
            let is_user_code = node.is_user_code();

            // ── 1. Background ─────────────────────────────────────────────────
            if is_selected {
                let sel_bg = Style::default().bg(palette::SELECTED_ROW_BG);
                for x in tree_inner.x..tree_inner.right() {
                    if let Some(cell) = buf.cell_mut((x, y)) {
                        cell.set_style(sel_bg);
                    }
                }
            }

            // ── 2. Vertical guidelines ────────────────────────────────────────
            //
            // For each ancestor depth `d` where a guideline should be drawn
            // (because that ancestor still has siblings below), render `│`.
            for d in 0..row.depth {
                let col = glyph_col(d);
                let x = match tree_inner.x.checked_add(col) {
                    Some(x) if x < tree_inner.right() => x,
                    _ => break,
                };
                let glyph_char = if row.ticks.contains(&d) { '│' } else { ' ' };
                if let Some(cell) = buf.cell_mut((x, y)) {
                    // Only overwrite if the cell is currently blank (background).
                    // This avoids trampling the branch tick that is drawn next.
                    let style = Style::default().fg(palette::TREE_GUIDELINE);
                    cell.set_char(glyph_char).set_style(style);
                }
            }

            // ── 3. Branch tick ────────────────────────────────────────────────
            //
            // Drawn at the column of the node's parent depth (depth-1),
            // occupying the two cells [glyph_col(depth-1), glyph_col(depth)-1].
            //
            // `branch_x` uses `Option<u16>` instead of a `0` sentinel so that a
            // tree whose `tree_inner.x == 0` and `branch_col == 0` still draws the
            // tick correctly at column 0 (C3 fix).
            if row.depth > 0 {
                let branch_col = glyph_col(row.depth.saturating_sub(1));
                let branch_x: Option<u16> = tree_inner
                    .x
                    .checked_add(branch_col)
                    .filter(|&x| x < tree_inner.right());

                let tick_style = Style::default().fg(palette::TREE_BRANCH_TICK);
                let (ch1, ch2) = if row.line_to_parent {
                    ('\u{251C}', '\u{2500}') // ├─
                } else {
                    ('\u{2514}', '\u{2500}') // └─
                };

                if let Some(bx) = branch_x {
                    if let Some(cell) = buf.cell_mut((bx, y)) {
                        cell.set_char(ch1).set_style(tick_style);
                    }
                    let bx2 = bx + 1;
                    if bx2 < tree_inner.right() {
                        if let Some(cell) = buf.cell_mut((bx2, y)) {
                            cell.set_char(ch2).set_style(tick_style);
                        }
                    }
                }
            }

            // ── 4. Icon glyph ─────────────────────────────────────────────────
            let icon_col = glyph_col(row.depth);
            let icon_x = match tree_inner.x.checked_add(icon_col) {
                Some(x) if x < tree_inner.right() => x,
                _ => {
                    // Icon beyond visible area — skip to next row but still
                    // register the row click region if we have a ctx.
                    if let Some(c) = ctx.as_deref_mut() {
                        use fdemon_app::message::Message;
                        let row_rect = MouseRect::new(tree_inner.x, y, tree_inner.width, 1);
                        if !row_rect.is_empty() {
                            c.click(
                                row_rect,
                                MouseAction::emit(Message::DevToolsInspectorSelectRow {
                                    index: vis_index,
                                }),
                            );
                        }
                    }
                    continue;
                }
            };

            let (icon_char, icon_style) = self.icon_and_style(row, node, is_selected, is_user_code);
            if let Some(cell) = buf.cell_mut((icon_x, y)) {
                cell.set_char(icon_char).set_style(icon_style);
            }

            // ── 5. Name text ──────────────────────────────────────────────────
            //
            // Name starts at icon_col + 2 (icon glyph + 1 space separator).
            // For group-leader-collapsed rows, show the badge text instead.
            let name_start_col = icon_col.saturating_add(2);
            let name_x = match tree_inner.x.checked_add(name_start_col) {
                Some(x) if x < tree_inner.right() => x,
                _ => tree_inner.right(), // no space — skip text
            };

            if name_x < tree_inner.right() {
                let max_name_w = (tree_inner.right() - name_x) as usize;
                let text_style = self.text_style(row, is_selected, is_user_code);

                let (name_text, name_style) = match &row.group {
                    RowGroup::LeaderCollapsed { hidden_count } => {
                        let badge = format!("{} +{} more", node.display_name(), hidden_count);
                        (badge, Style::default().fg(palette::TREE_GROUP_LEADER_TEXT))
                    }
                    _ => {
                        let raw = node.display_name();
                        (raw.to_string(), text_style)
                    }
                };

                let display = truncate_str(&name_text, max_name_w);
                buf.set_string(name_x, y, display, name_style);

                // Source-location hint for selected user-code rows (normal rows only).
                if is_selected
                    && is_user_code
                    && !matches!(row.group, RowGroup::LeaderCollapsed { .. })
                {
                    if let Some(loc) = &node.creation_location {
                        let short = short_path(&loc.file);
                        let loc_text = format!(" ({}:{})", short, loc.line);
                        let used = display.len() as u16;
                        let remaining = tree_inner
                            .right()
                            .saturating_sub(name_x)
                            .saturating_sub(used);
                        if remaining > loc_text.len() as u16 {
                            buf.set_string(
                                name_x + used,
                                y,
                                &loc_text,
                                Style::default().fg(palette::TEXT_MUTED),
                            );
                        }
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

                // Glyph click region (left-click on icon → toggle).
                // Uses new glyph_col() math instead of the old `depth * 2`.
                // Pushed AFTER the row region so the registry's last-pushed-wins-at-same-z
                // invariant makes the glyph rect win on overlap.
                //
                // Saturating arithmetic: at depths above ~32 767 (u16::MAX / 2)
                // the tree is pathological; skip glyph registration.
                let glyph_offset = glyph_col(row.depth);
                let Some(glyph_x) = tree_inner.x.checked_add(glyph_offset) else {
                    continue; // indent overflows u16 — skip glyph for impossibly deep node
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

    // ── Icon + style helpers ──────────────────────────────────────────────────

    /// Returns the icon character and its style for a given row.
    ///
    /// - Regular nodes: `▶` / `▼` / `●` (same as legacy `expand_icon()`).
    /// - `LeaderExpanded`: type icon with a `▼` prefix (indicates collapsible chain).
    /// - `LeaderCollapsed`: `+` glyph to signal expansion is possible.
    /// - `Member`: type icon with dim chain-member style.
    fn icon_and_style(
        &self,
        row: &InspectorRow<'_>,
        node: &DiagnosticsNode,
        is_selected: bool,
        is_user_code: bool,
    ) -> (char, Style) {
        let base_icon = glyph_for_widget(node);

        match &row.group {
            RowGroup::LeaderCollapsed { .. } => {
                // "+" expander — chain is collapsed.
                let style = if is_selected {
                    Style::default()
                        .fg(palette::TREE_GROUP_LEADER_TEXT)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(palette::TREE_GROUP_LEADER_TEXT)
                };
                ('+', style)
            }
            RowGroup::LeaderExpanded => {
                // Type icon styled as an expanded chain leader.
                let style = if is_selected {
                    Style::default()
                        .fg(palette::TREE_GROUP_LEADER_TEXT)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(palette::TREE_GROUP_LEADER_TEXT)
                };
                (base_icon, style)
            }
            RowGroup::Member => {
                // Chain member — dimmed.
                let style = if is_selected {
                    Style::default()
                        .fg(palette::TREE_CHAIN_MEMBER_TEXT)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(palette::TREE_CHAIN_MEMBER_TEXT)
                };
                (base_icon, style)
            }
            RowGroup::None => {
                // Standard expand/collapse icon using the legacy helper.
                let icon_char = match self.expand_icon(node) {
                    "▶" => '▶',
                    "▼" => '▼',
                    _ => '●',
                };
                let style = self.node_style(is_selected, is_user_code);
                (icon_char, style)
            }
        }
    }

    /// Returns the text style for the name portion of a row.
    fn text_style(&self, row: &InspectorRow<'_>, is_selected: bool, is_user_code: bool) -> Style {
        match &row.group {
            RowGroup::Member => {
                if is_selected {
                    Style::default()
                        .fg(palette::TREE_CHAIN_MEMBER_TEXT)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(palette::TREE_CHAIN_MEMBER_TEXT)
                }
            }
            RowGroup::LeaderExpanded | RowGroup::LeaderCollapsed { .. } => {
                if is_selected {
                    Style::default()
                        .fg(palette::TREE_GROUP_LEADER_TEXT)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(palette::TREE_GROUP_LEADER_TEXT)
                }
            }
            RowGroup::None => self.node_style(is_selected, is_user_code),
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
