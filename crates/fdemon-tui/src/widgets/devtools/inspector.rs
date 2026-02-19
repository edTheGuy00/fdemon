//! Widget inspector panel for the DevTools TUI mode.
//!
//! Renders the Flutter widget tree as an expandable/collapsible tree view
//! with the selected widget's details shown in a side panel.

use fdemon_app::state::InspectorState;
use fdemon_core::widget_tree::DiagnosticsNode;
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};

use crate::theme::{icons::IconSet, palette};

// ── Layout constants ──────────────────────────────────────────────────────────

/// Width threshold below which the details panel is hidden.
const WIDE_TERMINAL_THRESHOLD: u16 = 80;

/// Percentage of horizontal space given to the tree panel when wide.
const TREE_WIDTH_PCT: u16 = 60;

/// Percentage of horizontal space given to the details panel when wide.
const DETAILS_WIDTH_PCT: u16 = 40;

// ── WidgetInspector ───────────────────────────────────────────────────────────

/// Widget inspector panel for the DevTools mode.
///
/// Renders the Flutter widget tree as an expandable/collapsible tree view
/// with the selected widget's details shown in a side panel. Handles loading,
/// error, and empty states when no tree data is available.
pub struct WidgetInspector<'a> {
    inspector_state: &'a InspectorState,
    #[allow(dead_code)]
    icons: IconSet,
}

impl<'a> WidgetInspector<'a> {
    /// Create a new `WidgetInspector` widget.
    pub fn new(inspector_state: &'a InspectorState, icons: IconSet) -> Self {
        Self {
            inspector_state,
            icons,
        }
    }

    // ── Public helpers (used in tests) ────────────────────────────────────────

    /// Return the expand/collapse icon for a node.
    ///
    /// - `"▶"` for collapsed nodes with children
    /// - `"▼"` for expanded nodes with children
    /// - `"●"` for leaf nodes (no children or no `value_id`)
    pub fn expand_icon(&self, node: &DiagnosticsNode) -> &'static str {
        if node.children.is_empty() {
            "●" // Leaf node
        } else if let Some(value_id) = &node.value_id {
            if self.inspector_state.is_expanded(value_id) {
                "▼" // Expanded
            } else {
                "▶" // Collapsed
            }
        } else {
            "●" // No ID — treat as leaf
        }
    }

    /// Compute the viewport start/end indices that keep the selected node
    /// visible near the centre of the viewport.
    pub fn visible_viewport_range(
        &self,
        viewport_height: usize,
        total_items: usize,
    ) -> (usize, usize) {
        let selected = self.inspector_state.selected_index;
        let half = viewport_height / 2;
        let start = if selected > half {
            (selected - half).min(total_items.saturating_sub(viewport_height))
        } else {
            0
        };
        let end = (start + viewport_height).min(total_items);
        (start, end)
    }
}

impl Widget for WidgetInspector<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Clear background
        let bg_style = Style::default().bg(palette::DEEPEST_BG);
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_style(bg_style).set_char(' ');
                }
            }
        }

        if self.inspector_state.loading {
            self.render_loading(area, buf);
        } else if let Some(ref error) = self.inspector_state.error {
            self.render_error(area, buf, error);
        } else if self.inspector_state.root.is_some() {
            self.render_tree(area, buf);
        } else {
            self.render_empty(area, buf);
        }
    }
}

impl WidgetInspector<'_> {
    // ── Tree rendering ────────────────────────────────────────────────────────

    fn render_tree(&self, area: Rect, buf: &mut Buffer) {
        let visible = self.inspector_state.visible_nodes();
        let selected = self.inspector_state.selected_index;

        // Horizontal split: tree (60%) | details (40%) — only for wide terminals.
        let (tree_area, details_area) = if area.width >= WIDE_TERMINAL_THRESHOLD {
            let chunks = Layout::horizontal([
                Constraint::Percentage(TREE_WIDTH_PCT),
                Constraint::Percentage(DETAILS_WIDTH_PCT),
            ])
            .split(area);
            (chunks[0], Some(chunks[1]))
        } else {
            (area, None)
        };

        self.render_tree_panel(tree_area, buf, &visible, selected);

        if let Some(det_area) = details_area {
            self.render_details(det_area, buf, &visible, selected);
        }
    }

    fn render_tree_panel(
        &self,
        area: Rect,
        buf: &mut Buffer,
        visible: &[(&DiagnosticsNode, usize)],
        selected: usize,
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
            buf.set_string(tree_inner.x, y, &display_line, style);

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
        }

        // Simple scroll indicator (right edge) if content overflows
        if total > viewport_height && viewport_height > 0 {
            let scroll_x = tree_inner.right().saturating_sub(1);
            // Top of scroll range indicator
            let thumb_y = if total > 0 {
                tree_inner.y
                    + ((selected * viewport_height / total) as u16)
                        .min(tree_inner.height.saturating_sub(1))
            } else {
                tree_inner.y
            };
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

    // ── Details panel ─────────────────────────────────────────────────────────

    fn render_details(
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
            &desc_trunc,
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
                    &prop_trunc,
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
                    &path_trunc,
                    Style::default().fg(palette::STATUS_BLUE),
                );
            }
        }
    }

    // ── Loading / Error / Empty states ────────────────────────────────────────

    fn render_loading(&self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(palette::BORDER_DIM))
            .title(" Widget Inspector ")
            .title_alignment(Alignment::Left);
        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height == 0 {
            return;
        }

        let text = Paragraph::new("Loading widget tree...")
            .style(Style::default().fg(palette::TEXT_MUTED))
            .alignment(Alignment::Center);
        let y_offset = inner.height / 2;
        text.render(
            Rect {
                y: inner.y + y_offset,
                height: 1,
                ..inner
            },
            buf,
        );
    }

    fn render_error(&self, area: Rect, buf: &mut Buffer, error: &str) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(palette::BORDER_DIM))
            .title(" Widget Inspector ")
            .title_alignment(Alignment::Left);
        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height == 0 {
            return;
        }

        let text = Paragraph::new(format!("Error: {error}"))
            .style(Style::default().fg(palette::STATUS_RED))
            .wrap(Wrap { trim: true });
        text.render(inner, buf);
    }

    fn render_empty(&self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(palette::BORDER_DIM))
            .title(" Widget Inspector ")
            .title_alignment(Alignment::Left);
        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height == 0 {
            return;
        }

        let text = Paragraph::new("Press 'r' to load widget tree")
            .style(Style::default().fg(palette::TEXT_MUTED))
            .alignment(Alignment::Center);
        let y_offset = inner.height / 2;
        text.render(
            Rect {
                y: inner.y + y_offset,
                height: 1,
                ..inner
            },
            buf,
        );
    }
}

// ── Helper functions ──────────────────────────────────────────────────────────

/// Compute a short display path from a file URI.
///
/// Strips the `file://` prefix (and any leading host/authority), then returns
/// the last two path components (parent dir + filename) to keep it concise
/// enough to fit in the tree panel.
///
/// Examples:
/// - `"file:///app/lib/main.dart"` → `"lib/main.dart"`
/// - `"/app/lib/main.dart"` → `"lib/main.dart"`
/// - `"main.dart"` → `"main.dart"`
fn short_path(file: &str) -> &str {
    // Strip `file://` scheme prefix if present.
    let without_scheme = if let Some(rest) = file.strip_prefix("file://") {
        rest
    } else {
        file
    };

    // Find the last two '/' separators to get "parent/file.dart".
    // Walk backwards: first slash gives filename, second gives parent dir.
    let bytes = without_scheme.as_bytes();
    let mut slash_count = 0u8;
    let mut split_pos = 0usize; // default: return the whole (scheme-stripped) string

    for (i, &b) in bytes.iter().enumerate().rev() {
        if b == b'/' {
            slash_count += 1;
            if slash_count == 2 {
                split_pos = i + 1; // start after the second-to-last slash
                break;
            }
        }
    }

    // If fewer than two slashes, return the full scheme-stripped path.
    &without_scheme[split_pos..]
}

/// Truncate a string to at most `max_chars` characters (by char count, not bytes).
fn truncate_str(s: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_chars {
        s.to_string()
    } else {
        chars[..max_chars].iter().collect()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use fdemon_app::state::InspectorState;
    use fdemon_core::widget_tree::{CreationLocation, DiagnosticsNode};

    fn make_test_tree() -> DiagnosticsNode {
        DiagnosticsNode {
            description: "MyApp".to_string(),
            value_id: Some("widget-1".to_string()),
            children: vec![DiagnosticsNode {
                description: "MaterialApp".to_string(),
                value_id: Some("widget-2".to_string()),
                children: vec![DiagnosticsNode {
                    description: "Scaffold".to_string(),
                    value_id: Some("widget-3".to_string()),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    #[test]
    fn test_inspector_renders_tree_without_panic() {
        let mut state = InspectorState::new();
        state.root = Some(make_test_tree());
        state.expanded.insert("widget-1".to_string());

        let widget = WidgetInspector::new(&state, IconSet::default());
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
        widget.render(Rect::new(0, 0, 80, 24), &mut buf);
    }

    #[test]
    fn test_inspector_renders_loading_state() {
        let mut state = InspectorState::new();
        state.loading = true;

        let widget = WidgetInspector::new(&state, IconSet::default());
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
        widget.render(Rect::new(0, 0, 80, 24), &mut buf);
    }

    #[test]
    fn test_inspector_renders_error_state() {
        let mut state = InspectorState::new();
        state.error = Some("Connection failed".to_string());

        let widget = WidgetInspector::new(&state, IconSet::default());
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
        widget.render(Rect::new(0, 0, 80, 24), &mut buf);
    }

    #[test]
    fn test_inspector_renders_empty_state() {
        let state = InspectorState::new();
        let widget = WidgetInspector::new(&state, IconSet::default());
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
        widget.render(Rect::new(0, 0, 80, 24), &mut buf);
    }

    #[test]
    fn test_inspector_narrow_terminal_no_details() {
        let mut state = InspectorState::new();
        state.root = Some(make_test_tree());
        state.expanded.insert("widget-1".to_string());

        let widget = WidgetInspector::new(&state, IconSet::default());
        let mut buf = Buffer::empty(Rect::new(0, 0, 60, 24)); // < 80 cols
        widget.render(Rect::new(0, 0, 60, 24), &mut buf);
        // Should render without panic, no details panel
    }

    #[test]
    fn test_expand_icon_leaf_node() {
        let state = InspectorState::new();
        let widget = WidgetInspector::new(&state, IconSet::default());
        let leaf = DiagnosticsNode {
            description: "Text".to_string(),
            children: vec![],
            ..Default::default()
        };
        assert_eq!(widget.expand_icon(&leaf), "●");
    }

    #[test]
    fn test_expand_icon_collapsed() {
        let state = InspectorState::new();
        let widget = WidgetInspector::new(&state, IconSet::default());
        let node = DiagnosticsNode {
            description: "Column".to_string(),
            value_id: Some("w1".to_string()),
            children: vec![DiagnosticsNode::default()],
            ..Default::default()
        };
        assert_eq!(widget.expand_icon(&node), "▶");
    }

    #[test]
    fn test_expand_icon_expanded() {
        let mut state = InspectorState::new();
        state.expanded.insert("w1".to_string());
        let widget = WidgetInspector::new(&state, IconSet::default());
        let node = DiagnosticsNode {
            description: "Column".to_string(),
            value_id: Some("w1".to_string()),
            children: vec![DiagnosticsNode::default()],
            ..Default::default()
        };
        assert_eq!(widget.expand_icon(&node), "▼");
    }

    #[test]
    fn test_viewport_scrolling_keeps_selected_visible() {
        let state = InspectorState {
            selected_index: 50,
            ..Default::default()
        };
        let widget = WidgetInspector::new(&state, IconSet::default());
        let (start, end) = widget.visible_viewport_range(20, 100);
        assert!(start <= 50, "start ({start}) should be <= 50");
        assert!(end > 50, "end ({end}) should be > 50");
    }

    #[test]
    fn test_viewport_scrolling_at_start() {
        let state = InspectorState {
            selected_index: 0,
            ..Default::default()
        };
        let widget = WidgetInspector::new(&state, IconSet::default());
        let (start, end) = widget.visible_viewport_range(20, 100);
        assert_eq!(start, 0);
        assert_eq!(end, 20);
    }

    #[test]
    fn test_viewport_scrolling_near_end() {
        let state = InspectorState {
            selected_index: 99,
            ..Default::default()
        };
        let widget = WidgetInspector::new(&state, IconSet::default());
        let (start, end) = widget.visible_viewport_range(20, 100);
        assert_eq!(end, 100);
        assert!(start <= 99);
    }

    #[test]
    fn test_viewport_empty_total() {
        let state = InspectorState::default();
        let widget = WidgetInspector::new(&state, IconSet::default());
        let (start, end) = widget.visible_viewport_range(20, 0);
        assert_eq!(start, 0);
        assert_eq!(end, 0);
    }

    #[test]
    fn test_short_path_strips_file_scheme() {
        assert_eq!(short_path("file:///app/lib/main.dart"), "lib/main.dart");
    }

    #[test]
    fn test_short_path_no_scheme() {
        assert_eq!(short_path("/app/lib/main.dart"), "lib/main.dart");
    }

    #[test]
    fn test_short_path_bare_filename() {
        assert_eq!(short_path("main.dart"), "main.dart");
    }

    #[test]
    fn test_short_path_deep_path() {
        assert_eq!(
            short_path("file:///home/user/project/lib/src/widgets/button.dart"),
            "widgets/button.dart"
        );
    }

    #[test]
    fn test_truncate_str_short() {
        assert_eq!(truncate_str("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_str_exact() {
        assert_eq!(truncate_str("hello", 5), "hello");
    }

    #[test]
    fn test_truncate_str_too_long() {
        assert_eq!(truncate_str("hello world", 5), "hello");
    }

    #[test]
    fn test_truncate_str_zero_max() {
        assert_eq!(truncate_str("hello", 0), "");
    }

    #[test]
    fn test_inspector_selected_node_highlighted() {
        // Place first node selected and render — should not panic.
        let mut state = InspectorState::new();
        state.root = Some(make_test_tree());
        state.expanded.insert("widget-1".to_string());
        state.selected_index = 0;

        let widget = WidgetInspector::new(&state, IconSet::default());
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
        widget.render(Rect::new(0, 0, 80, 24), &mut buf);
    }

    #[test]
    fn test_inspector_user_code_shown_differently() {
        // User code node
        let mut state = InspectorState::new();
        let mut root = DiagnosticsNode {
            description: "MyWidget".to_string(),
            value_id: Some("user-widget".to_string()),
            created_by_local_project: true,
            creation_location: Some(CreationLocation {
                file: "file:///app/lib/main.dart".to_string(),
                line: 42,
                column: 8,
                name: Some("MyWidget".to_string()),
            }),
            ..Default::default()
        };
        // Framework child
        let framework_child = DiagnosticsNode {
            description: "Container".to_string(),
            value_id: Some("fw-widget".to_string()),
            created_by_local_project: false,
            ..Default::default()
        };
        root.children.push(framework_child);
        state.root = Some(root);
        state.expanded.insert("user-widget".to_string());
        state.selected_index = 0;

        let widget = WidgetInspector::new(&state, IconSet::default());
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
        widget.render(Rect::new(0, 0, 80, 24), &mut buf);
    }

    #[test]
    fn test_inspector_with_properties() {
        let mut state = InspectorState::new();
        let mut root = DiagnosticsNode {
            description: "Text".to_string(),
            value_id: Some("text-1".to_string()),
            ..Default::default()
        };
        root.properties.push(DiagnosticsNode {
            description: "Hello World".to_string(),
            name: Some("data".to_string()),
            ..Default::default()
        });
        state.root = Some(root);
        state.selected_index = 0;

        let widget = WidgetInspector::new(&state, IconSet::default());
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
        widget.render(Rect::new(0, 0, 80, 24), &mut buf);
    }

    #[test]
    fn test_inspector_zero_area_no_panic() {
        let state = InspectorState::default();
        let widget = WidgetInspector::new(&state, IconSet::default());
        let mut buf = Buffer::empty(Rect::new(0, 0, 10, 1));
        widget.render(Rect::new(0, 0, 10, 1), &mut buf);
    }

    #[test]
    fn test_inspector_loading_state_contains_message() {
        let mut state = InspectorState::new();
        state.loading = true;

        let widget = WidgetInspector::new(&state, IconSet::default());
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
        widget.render(Rect::new(0, 0, 80, 24), &mut buf);

        // Collect all text from buffer
        let mut full = String::new();
        for y in 0..24u16 {
            for x in 0..80u16 {
                if let Some(c) = buf.cell((x, y)) {
                    if let Some(ch) = c.symbol().chars().next() {
                        full.push(ch);
                    }
                }
            }
        }
        assert!(
            full.contains("Loading"),
            "Expected 'Loading' in buffer, got: {full:?}"
        );
    }

    #[test]
    fn test_inspector_empty_state_contains_prompt() {
        let state = InspectorState::new();

        let widget = WidgetInspector::new(&state, IconSet::default());
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
        widget.render(Rect::new(0, 0, 80, 24), &mut buf);

        let mut full = String::new();
        for y in 0..24u16 {
            for x in 0..80u16 {
                if let Some(c) = buf.cell((x, y)) {
                    if let Some(ch) = c.symbol().chars().next() {
                        full.push(ch);
                    }
                }
            }
        }
        assert!(
            full.contains("Press"),
            "Expected 'Press' in buffer, got: {full:?}"
        );
    }

    #[test]
    fn test_inspector_error_state_contains_error() {
        let mut state = InspectorState::new();
        state.error = Some("VM not connected".to_string());

        let widget = WidgetInspector::new(&state, IconSet::default());
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
        widget.render(Rect::new(0, 0, 80, 24), &mut buf);

        let mut full = String::new();
        for y in 0..24u16 {
            for x in 0..80u16 {
                if let Some(c) = buf.cell((x, y)) {
                    if let Some(ch) = c.symbol().chars().next() {
                        full.push(ch);
                    }
                }
            }
        }
        assert!(
            full.contains("Error") || full.contains("VM"),
            "Expected error message in buffer, got: {full:?}"
        );
    }
}
