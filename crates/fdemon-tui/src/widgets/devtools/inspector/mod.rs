//! Widget inspector panel for the DevTools TUI mode.
//!
//! Renders the Flutter widget tree as an expandable/collapsible tree view
//! with the selected widget's details shown in a side panel.

mod layout_panel;
mod tree_panel;

use fdemon_app::state::{DevToolsError, InspectorState, VmConnectionStatus};
use fdemon_core::widget_tree::DiagnosticsNode;
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};

// Re-export truncate_str from devtools/mod.rs so this module and sibling
// submodules can access it via `super::truncate_str`.
pub(super) use super::truncate_str;
use crate::theme::palette;

// ── Layout constants ──────────────────────────────────────────────────────────

/// Width threshold below which the layout panel is shown in vertical split instead of horizontal.
pub(super) const WIDE_TERMINAL_THRESHOLD: u16 = 100;

/// Percentage of horizontal space given to the tree panel when wide.
pub(super) const TREE_WIDTH_PCT: u16 = 50;

/// Percentage of horizontal space given to the layout panel when wide.
pub(super) const LAYOUT_WIDTH_PCT: u16 = 50;

/// Number of content lines in the disconnected-state panel.
const DISCONNECTED_CONTENT_LINES: u16 = 6;

/// Number of content lines in the error-state panel.
const ERROR_CONTENT_LINES: u16 = 5;

/// Minimum height required to render the full two-panel tree layout.
/// Below this threshold a compact single-line summary is shown instead.
const MIN_TREE_RENDER_HEIGHT: u16 = 4;

/// Minimum height for each split panel (tree + layout) in the vertical split.
/// If neither half would reach this height, show only the tree panel.
const MIN_SPLIT_PANEL_HEIGHT: u16 = 3;

// ── WidgetInspector ───────────────────────────────────────────────────────────

/// Widget inspector panel for the DevTools mode.
///
/// Renders the Flutter widget tree as an expandable/collapsible tree view
/// with the selected widget's details shown in a side panel. Handles loading,
/// error, and empty states when no tree data is available.
pub struct WidgetInspector<'a> {
    inspector_state: &'a InspectorState,
    /// Whether the VM Service WebSocket is currently connected.
    /// When `false`, the panel renders a dedicated "VM Service disconnected"
    /// state instead of the generic empty/error state.
    vm_connected: bool,
    /// Rich connection status for contextual disconnected messaging.
    connection_status: &'a VmConnectionStatus,
}

impl<'a> WidgetInspector<'a> {
    /// Create a new `WidgetInspector` widget.
    pub fn new(
        inspector_state: &'a InspectorState,
        vm_connected: bool,
        connection_status: &'a VmConnectionStatus,
    ) -> Self {
        Self {
            inspector_state,
            vm_connected,
            connection_status,
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
        // Clear background — set every cell to ' ' with the background style
        // so the log view underneath is fully occluded.
        let bg_style = Style::default().bg(palette::DEEPEST_BG);
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_style(bg_style).set_char(' ');
                }
            }
        }

        if !self.vm_connected {
            self.render_disconnected(area, buf);
        } else if self.inspector_state.loading {
            self.render_loading(area, buf);
        } else if let Some(ref error) = self.inspector_state.error {
            self.render_error_box(area, buf, error);
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

        // Guard: if the area is too small for a two-panel layout, show a compact
        // single-line summary instead of potentially garbled split output.
        if area.height < MIN_TREE_RENDER_HEIGHT {
            let node_count = visible.len();
            let msg = if node_count == 0 {
                "No widget tree".to_string()
            } else {
                format!("{} nodes", node_count)
            };
            let line = Line::from(Span::styled(msg, Style::default().fg(Color::DarkGray)));
            buf.set_line(area.x, area.y, &line, area.width);
            return;
        }

        // Wide terminals (>= 100 cols): horizontal split — tree left | layout right.
        // Narrow terminals (< 100 cols): vertical split — tree top | layout bottom.
        //
        // For the vertical (narrow) case, only show the layout panel when each half
        // would have at least MIN_SPLIT_PANEL_HEIGHT rows — otherwise show tree only.
        let (tree_area, layout_area) = if area.width >= WIDE_TERMINAL_THRESHOLD {
            let chunks = Layout::horizontal([
                Constraint::Percentage(TREE_WIDTH_PCT),
                Constraint::Percentage(LAYOUT_WIDTH_PCT),
            ])
            .split(area);
            (chunks[0], Some(chunks[1]))
        } else {
            // Each half gets ~50% of the height. If the resulting panels are too
            // short to be useful, skip the layout panel entirely.
            let half_height = area.height / 2;
            if half_height >= MIN_SPLIT_PANEL_HEIGHT {
                let chunks = Layout::vertical([
                    Constraint::Percentage(TREE_WIDTH_PCT),
                    Constraint::Percentage(LAYOUT_WIDTH_PCT),
                ])
                .split(area);
                (chunks[0], Some(chunks[1]))
            } else {
                // Not enough vertical space for two panels — tree panel only.
                (area, None)
            }
        };

        self.render_tree_panel(tree_area, buf, &visible, selected);

        if let Some(lay_area) = layout_area {
            self.render_layout_panel(lay_area, buf, &visible, selected);
        }
    }

    // ── Loading / Error / Empty / Disconnected states ─────────────────────────

    fn render_disconnected(&self, area: Rect, buf: &mut Buffer) {
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

        let status_line = match self.connection_status {
            VmConnectionStatus::Reconnecting {
                attempt,
                max_attempts,
            } => {
                format!("Reconnecting to VM Service... ({attempt}/{max_attempts})")
            }
            VmConnectionStatus::TimedOut => "Widget tree fetch timed out.".to_string(),
            _ => "VM Service disconnected.".to_string(),
        };

        let lines = vec![
            Line::from(Span::styled(
                status_line,
                Style::default().fg(palette::STATUS_RED),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Widget tree is unavailable while disconnected.",
                Style::default().fg(palette::TEXT_MUTED),
            )),
            Line::from(Span::styled(
                "Waiting for reconnection...",
                Style::default().fg(palette::TEXT_MUTED),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Press [r] to retry  |  Press [b] to open browser DevTools  |  Press [Esc] to return to logs",
                Style::default().fg(palette::TEXT_MUTED),
            )),
        ];

        let paragraph = Paragraph::new(lines).wrap(Wrap { trim: true });

        let y_offset = inner.height.saturating_sub(DISCONNECTED_CONTENT_LINES) / 2;
        let render_area = Rect {
            y: inner.y + y_offset,
            height: DISCONNECTED_CONTENT_LINES.min(inner.height),
            ..inner
        };
        paragraph.render(render_area, buf);
    }

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

    fn render_error_box(&self, area: Rect, buf: &mut Buffer, error: &DevToolsError) {
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

        let lines = vec![
            Line::from(Span::styled(
                format!("\u{26a0} {}", error.message),
                Style::default().fg(Color::Yellow),
            )),
            Line::from(""),
            Line::from(Span::styled(
                error.hint.as_str(),
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "[r] Retry   [b] Browser DevTools   [Esc] Return to logs",
                Style::default().fg(palette::TEXT_MUTED),
            )),
        ];

        let paragraph = Paragraph::new(lines).wrap(Wrap { trim: true });

        let y_offset = inner.height.saturating_sub(ERROR_CONTENT_LINES) / 2;
        let render_area = Rect {
            y: inner.y + y_offset,
            height: ERROR_CONTENT_LINES.min(inner.height),
            ..inner
        };
        paragraph.render(render_area, buf);
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
pub(super) fn short_path(file: &str) -> &str {
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

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
