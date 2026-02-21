//! # Network Monitor Widget
//!
//! Top-level widget for the Network tab in DevTools. Composes the request
//! table (left/top) and request details (right/bottom) into a responsive
//! split layout.
//!
//! Layout rules:
//! - **Wide** (>= [`WIDE_THRESHOLD`]): horizontal split — table left (55%), details right (45%)
//! - **Narrow** (< [`WIDE_THRESHOLD`]) **with selection**: vertical split — table top (50%), details bottom (50%)
//! - **No selection**: full-width table (both wide and narrow)

pub mod request_details;
pub mod request_table;

#[cfg(test)]
mod tests;

use fdemon_app::session::NetworkState;
use fdemon_app::state::VmConnectionStatus;
use fdemon_core::network::HttpProfileEntry;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Widget},
};

use crate::theme::palette;
use request_details::RequestDetails;
use request_table::RequestTable;

/// Terminal width threshold for horizontal vs vertical split.
const WIDE_THRESHOLD: u16 = 100;

/// Return a foreground [`Color`] for the given HTTP method string.
///
/// Provides a single authoritative color mapping used by both the request table
/// and the request details panel, ensuring visual consistency across the
/// Network Monitor.
pub(super) fn http_method_color(method: &str) -> Color {
    match method {
        "GET" => Color::Green,
        "POST" => Color::Blue,
        "PUT" | "PATCH" => Color::Yellow,
        "DELETE" => Color::Red,
        "HEAD" => Color::Cyan,
        "OPTIONS" => Color::Magenta,
        _ => Color::White,
    }
}

// ── NetworkMonitor ────────────────────────────────────────────────────────────

/// Top-level Network Monitor widget for the DevTools mode.
///
/// Composes the request table and request details panels into a responsive
/// layout. On wide terminals (>= [`WIDE_THRESHOLD`] columns) the table and
/// detail panel are shown side-by-side (horizontal split). On narrow terminals
/// with a selection, both panels are shown in a vertical split (table top,
/// details bottom). When nothing is selected the full area is used for the
/// table.
pub struct NetworkMonitor<'a> {
    network_state: &'a NetworkState,
    vm_connected: bool,
    connection_status: &'a VmConnectionStatus,
}

impl<'a> NetworkMonitor<'a> {
    /// Create a new `NetworkMonitor` widget.
    pub fn new(
        network_state: &'a NetworkState,
        vm_connected: bool,
        connection_status: &'a VmConnectionStatus,
    ) -> Self {
        Self {
            network_state,
            vm_connected,
            connection_status,
        }
    }
}

impl Widget for NetworkMonitor<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Fill background using an unstyled Block — more idiomatic than a
        // manual cell-by-cell loop and produces the same result.
        Block::new()
            .style(Style::default().bg(palette::DEEPEST_BG))
            .render(area, buf);

        // Gate on VM connection
        if !self.vm_connected {
            self.render_disconnected(area, buf);
            return;
        }

        // Check if extensions are unavailable
        if self.network_state.extensions_available == Some(false) {
            self.render_unavailable(area, buf);
            return;
        }

        // Reserve bottom row for parent footer
        let usable = Rect {
            height: area.height.saturating_sub(1),
            ..area
        };

        if usable.height < 3 {
            // Too small for any content
            return;
        }

        // Compute filtered entries once, used for both table and detail
        let filtered = self.network_state.filtered_entries();
        let has_selection = self.network_state.selected_index.is_some();

        if has_selection {
            if area.width >= WIDE_THRESHOLD {
                // Wide: horizontal split — table (55%) | details (45%)
                self.render_wide_layout(usable, buf, &filtered);
            } else {
                // Narrow: vertical split — table top (50%) | details bottom (50%)
                self.render_narrow_split(usable, buf, &filtered);
            }
        } else {
            // No selection: full-width table
            self.render_table_only(usable, buf, &filtered);
        }
    }
}

impl NetworkMonitor<'_> {
    // ── Layout variants ───────────────────────────────────────────────────────

    fn render_wide_layout(&self, area: Rect, buf: &mut Buffer, filtered: &[&HttpProfileEntry]) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(area);

        // Left: Request table
        let table = RequestTable::new(
            filtered,
            self.network_state.selected_index,
            self.network_state.scroll_offset,
            self.network_state.recording,
            &self.network_state.filter,
        );
        table.render(chunks[0], buf);

        // Right: Request details (with border)
        let detail_block = Block::default()
            .borders(Borders::LEFT)
            .border_style(Style::default().fg(palette::BORDER_DIM));
        let detail_inner = detail_block.inner(chunks[1]);
        detail_block.render(chunks[1], buf);

        if let Some(entry) = self.network_state.selected_entry() {
            let detail_widget = RequestDetails::new(
                entry,
                self.network_state.selected_detail.as_deref(),
                self.network_state.detail_tab,
                self.network_state.loading_detail,
            );
            detail_widget.render(detail_inner, buf);
        }
    }

    fn render_narrow_split(&self, area: Rect, buf: &mut Buffer, filtered: &[&HttpProfileEntry]) {
        let chunks =
            Layout::vertical([Constraint::Percentage(50), Constraint::Percentage(50)]).split(area);

        // Top: request table
        self.render_table_only(chunks[0], buf, filtered);

        // Bottom: request details
        if let Some(entry) = self.network_state.selected_entry() {
            let detail_widget = RequestDetails::new(
                entry,
                self.network_state.selected_detail.as_deref(),
                self.network_state.detail_tab,
                self.network_state.loading_detail,
            );
            detail_widget.render(chunks[1], buf);
        }
    }

    fn render_table_only(&self, area: Rect, buf: &mut Buffer, filtered: &[&HttpProfileEntry]) {
        let table = RequestTable::new(
            filtered,
            self.network_state.selected_index,
            self.network_state.scroll_offset,
            self.network_state.recording,
            &self.network_state.filter,
        );
        table.render(area, buf);
    }

    // ── Disconnected / unavailable states ─────────────────────────────────────

    fn render_disconnected(&self, area: Rect, buf: &mut Buffer) {
        let msg = match self.connection_status {
            VmConnectionStatus::Reconnecting {
                attempt,
                max_attempts,
            } => format!("Reconnecting to VM Service (attempt {attempt}/{max_attempts})..."),
            VmConnectionStatus::TimedOut => "VM Service connection timed out".to_string(),
            _ => "Waiting for VM Service connection...".to_string(),
        };

        let y = area.y + area.height / 2;
        let x = area.x + area.width.saturating_sub(msg.len() as u16) / 2;
        buf.set_string(x, y, &msg, Style::default().fg(Color::DarkGray));
    }

    fn render_unavailable(&self, area: Rect, buf: &mut Buffer) {
        let lines: &[&str] = &[
            "Network profiling is not available",
            "",
            "ext.dart.io.* extensions are not registered.",
            "This may be because the app is running in release mode.",
            "Network profiling requires debug or profile mode.",
        ];
        let start_y = area.y + area.height.saturating_sub(lines.len() as u16) / 2;
        for (i, line) in lines.iter().enumerate() {
            let y = start_y + i as u16;
            if y >= area.bottom() {
                break;
            }
            let x = area.x + area.width.saturating_sub(line.len() as u16) / 2;
            let style = if i == 0 {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            buf.set_string(x, y, line, style);
        }
    }
}
