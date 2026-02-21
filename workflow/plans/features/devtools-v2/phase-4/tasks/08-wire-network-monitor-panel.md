## Task: Wire Network Monitor Panel and Tab

**Objective**: Create the top-level `NetworkMonitor` widget that composes the request table and request details into a responsive layout, and integrate it into the DevTools tab bar. This is the final TUI integration that makes the Network tab visible and functional.

**Depends on**: Task 04 (network-handlers-and-keybindings), Task 06 (request-table-widget), Task 07 (request-details-widget)

### Scope

- `crates/fdemon-tui/src/widgets/devtools/network/mod.rs`: **NEW** — `NetworkMonitor` top-level widget
- `crates/fdemon-tui/src/widgets/devtools/mod.rs`: Add `pub mod network;`, tab bar entry, render dispatch, footer hints

### Details

#### Create `network/mod.rs`

Follow the `inspector/mod.rs` and `performance/mod.rs` patterns exactly:

```rust
//! # Network Monitor Widget
//!
//! Top-level widget for the Network tab in DevTools. Composes the request
//! table (left/top) and request details (right/bottom) into a responsive
//! split layout.

pub mod request_table;
pub mod request_details;

#[cfg(test)]
mod tests;

use fdemon_core::network::NetworkDetailTab;
use crate::session::network::NetworkState;
use crate::state::VmConnectionStatus;
use ratatui::prelude::*;
use ratatui::widgets::{Block, BorderType, Borders, Widget};
use request_table::RequestTable;
use request_details::RequestDetails;

/// Terminal width threshold for horizontal vs vertical split.
const WIDE_THRESHOLD: u16 = 100;

pub struct NetworkMonitor<'a> {
    network_state: &'a NetworkState,
    vm_connected: bool,
    connection_status: &'a VmConnectionStatus,
}

impl<'a> NetworkMonitor<'a> {
    pub fn new(
        network_state: &'a NetworkState,
        vm_connected: bool,
        connection_status: &'a VmConnectionStatus,
    ) -> Self {
        Self { network_state, vm_connected, connection_status }
    }
}
```

#### Widget implementation

```rust
impl Widget for NetworkMonitor<'_> {
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

        // Check if we have a selected entry to show details
        let has_selection = self.network_state.selected_index.is_some();
        let filtered = self.network_state.filtered_entries();

        if area.width >= WIDE_THRESHOLD && has_selection {
            // Wide: horizontal split — table (55%) | details (45%)
            self.render_wide_layout(usable, buf, &filtered);
        } else if has_selection && area.width < WIDE_THRESHOLD {
            // Narrow with selection: show details only (full width)
            // User pressed Enter to view details, Esc to go back
            self.render_narrow_detail(usable, buf, &filtered);
        } else {
            // No selection or narrow without selection: full-width table
            self.render_table_only(usable, buf, &filtered);
        }
    }
}
```

#### Layout variants

```rust
impl NetworkMonitor<'_> {
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

    fn render_narrow_detail(&self, area: Rect, buf: &mut Buffer, filtered: &[&HttpProfileEntry]) {
        // Full-width detail view
        if let Some(entry) = self.network_state.selected_entry() {
            let detail_widget = RequestDetails::new(
                entry,
                self.network_state.selected_detail.as_deref(),
                self.network_state.detail_tab,
                self.network_state.loading_detail,
            );
            detail_widget.render(area, buf);
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

    fn render_disconnected(&self, area: Rect, buf: &mut Buffer) {
        let msg = match self.connection_status {
            VmConnectionStatus::Reconnecting { attempt } =>
                format!("Reconnecting to VM Service (attempt {})...", attempt),
            VmConnectionStatus::TimedOut =>
                "VM Service connection timed out".to_string(),
            _ => "Waiting for VM Service connection...".to_string(),
        };
        let y = area.y + area.height / 2;
        let x = area.x + area.width.saturating_sub(msg.len() as u16) / 2;
        buf.set_string(x, y, &msg, Style::default().fg(Color::DarkGray));
    }

    fn render_unavailable(&self, area: Rect, buf: &mut Buffer) {
        let lines = [
            "Network profiling is not available",
            "",
            "ext.dart.io.* extensions are not registered.",
            "This may be because the app is running in release mode.",
            "Network profiling requires debug or profile mode.",
        ];
        let start_y = area.y + area.height.saturating_sub(lines.len() as u16) / 2;
        for (i, line) in lines.iter().enumerate() {
            let y = start_y + i as u16;
            if y >= area.bottom() { break; }
            let x = area.x + area.width.saturating_sub(line.len() as u16) / 2;
            let style = if i == 0 {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            buf.set_string(x, y, line, style);
        }
    }
}
```

#### Integrate into `devtools/mod.rs`

1. **Add module declaration** at the top:
```rust
pub mod network;
```

2. **Add tab to the tab bar array** (in `render_tab_bar()`):
```rust
let tabs = [
    (DevToolsPanel::Inspector, "[i] Inspector"),
    (DevToolsPanel::Performance, "[p] Performance"),
    (DevToolsPanel::Network, "[n] Network"),        // NEW
];
```

3. **Add render dispatch** (in the main `render()` match):
```rust
DevToolsPanel::Network => {
    let session = state.session_manager.active_session();
    let network_state = session.map(|s| &s.network);
    let vm_connected = session.map_or(false, |s| s.vm_connected);

    if let Some(network_state) = network_state {
        let widget = network::NetworkMonitor::new(
            network_state,
            vm_connected,
            &state.devtools_view_state.connection_status,
        );
        widget.render(content_area, buf);
    }
}
```

4. **Add footer hints** (in `render_footer()`):
```rust
DevToolsPanel::Network => {
    let session = state.session_manager.active_session();
    let has_selection = session.map_or(false, |s| s.network.selected_index.is_some());
    if has_selection {
        "[Esc] Deselect  [g/h/q/s/t] Detail tabs  [Space] Toggle rec  [b] Browser"
    } else {
        "[Esc] Logs  [↑↓] Navigate  [Enter] Detail  [Space] Toggle rec  [b] Browser"
    }
}
```

### Acceptance Criteria

1. `NetworkMonitor` widget renders without panic for any terminal size
2. Disconnected state shows centered message with reconnection attempt count
3. Extensions unavailable state shows informative message about release mode
4. Wide terminals (>= 100 cols) show horizontal split: table 55% | details 45%
5. Narrow terminals with selection show full-width detail view
6. Narrow terminals without selection show full-width table
7. Bottom row reserved for parent footer (height - 1)
8. Tab bar in `devtools/mod.rs` shows `[n] Network` tab
9. `'n'` key switches to Network panel
10. Footer hints update based on selection state
11. Render dispatch correctly passes session network state to widget
12. Empty state (no requests, VM connected) shows recording indicator and empty table
13. All new code has unit tests (20+ tests)

### Testing

Create `crates/fdemon-tui/src/widgets/devtools/network/tests.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use fdemon_core::network::*;
    use crate::session::network::NetworkState;
    use crate::state::VmConnectionStatus;

    fn make_network_state() -> NetworkState {
        NetworkState::default()
    }

    fn make_network_state_with_entries(n: usize) -> NetworkState {
        let mut state = NetworkState::default();
        for i in 0..n {
            state.merge_entries(vec![HttpProfileEntry {
                id: format!("req_{}", i),
                method: if i % 2 == 0 { "GET" } else { "POST" }.to_string(),
                uri: format!("https://api.example.com/resource/{}", i),
                status_code: Some(200),
                content_type: Some("application/json".to_string()),
                start_time_us: 1_000_000 + (i as i64 * 50_000),
                end_time_us: Some(1_050_000 + (i as i64 * 50_000)),
                request_content_length: None,
                response_content_length: Some(1024),
                error: None,
            }]);
        }
        state
    }

    fn render_monitor(state: &NetworkState, vm_connected: bool, w: u16, h: u16) -> Buffer {
        let conn_status = VmConnectionStatus::Connected;
        let widget = NetworkMonitor::new(state, vm_connected, &conn_status);
        let mut buf = Buffer::empty(Rect::new(0, 0, w, h));
        widget.render(Rect::new(0, 0, w, h), &mut buf);
        buf
    }

    fn buf_contains(buf: &Buffer, w: u16, h: u16, text: &str) -> bool {
        let mut full = String::new();
        for y in 0..h { for x in 0..w {
            if let Some(c) = buf.cell((x, y)) { full.push_str(c.symbol()); }
        }}
        full.contains(text)
    }

    #[test]
    fn test_renders_without_panic() { render_monitor(&make_network_state(), true, 80, 24); }

    #[test]
    fn test_disconnected_state() {
        let buf = render_monitor(&make_network_state(), false, 80, 24);
        assert!(buf_contains(&buf, 80, 24, "Waiting for VM Service"));
    }

    #[test]
    fn test_extensions_unavailable() {
        let mut state = make_network_state();
        state.extensions_available = Some(false);
        let buf = render_monitor(&state, true, 80, 24);
        assert!(buf_contains(&buf, 80, 24, "not available"));
    }

    #[test]
    fn test_empty_state_shows_recording() {
        let buf = render_monitor(&make_network_state(), true, 80, 24);
        assert!(buf_contains(&buf, 80, 24, "REC"));
    }

    #[test]
    fn test_with_entries_shows_table() {
        let state = make_network_state_with_entries(5);
        let buf = render_monitor(&state, true, 100, 24);
        assert!(buf_contains(&buf, 100, 24, "GET"));
        assert!(buf_contains(&buf, 100, 24, "5 requests"));
    }

    #[test]
    fn test_wide_terminal_with_selection_shows_split() {
        let mut state = make_network_state_with_entries(5);
        state.selected_index = Some(0);
        let buf = render_monitor(&state, true, 120, 24);
        // Should show both table and detail
        assert!(buf_contains(&buf, 120, 24, "GET"));
        assert!(buf_contains(&buf, 120, 24, "General")); // detail tab
    }

    #[test]
    fn test_narrow_terminal_no_selection_shows_table() {
        let state = make_network_state_with_entries(5);
        let buf = render_monitor(&state, true, 60, 24);
        assert!(buf_contains(&buf, 60, 24, "GET"));
    }

    #[test]
    fn test_tiny_terminal_no_panic() { render_monitor(&make_network_state(), true, 10, 3); }

    #[test]
    fn test_zero_height_no_panic() { render_monitor(&make_network_state(), true, 80, 0); }

    #[test]
    fn test_reconnecting_shows_attempt() {
        let state = make_network_state();
        let conn_status = VmConnectionStatus::Reconnecting { attempt: 3 };
        let widget = NetworkMonitor::new(&state, false, &conn_status);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
        widget.render(Rect::new(0, 0, 80, 24), &mut buf);
        assert!(buf_contains(&buf, 80, 24, "attempt 3"));
    }

    #[test]
    fn test_footer_does_not_overlap_content() {
        let state = make_network_state_with_entries(20);
        let buf = render_monitor(&state, true, 80, 10);
        // Last row should be blank (reserved for parent footer)
        let last_row_y = 9u16;
        let mut last_row_blank = true;
        for x in 0..80 {
            if let Some(c) = buf.cell((x, last_row_y)) {
                if c.symbol() != " " { last_row_blank = false; break; }
            }
        }
        assert!(last_row_blank, "Last row should be reserved for footer");
    }
}
```

### Notes

- **Footer row convention**: The parent `DevToolsView` renders a footer hint in the last row of the content area. `NetworkMonitor` must reserve this row by using `area.height.saturating_sub(1)` — matching the pattern used by `PerformancePanel`.
- **Responsive threshold**: Using 100 columns as the wide/narrow breakpoint. Below 100, the detail panel would be too cramped for headers/body display. This matches the `WIDE_THRESHOLD` used in the Inspector panel.
- **Narrow mode interaction**: In narrow terminals, selecting a request and pressing Enter shows the detail view full-width. Pressing Esc returns to the table. This is handled by the handler logic (selection state), not the widget.
- **`filtered_entries()` called once**: The parent widget calls `filtered_entries()` once and passes the slice to both the table and the selection logic. This avoids computing the filter twice per render.
- **`selected_detail.as_deref()`**: The `Box<HttpProfileEntryDetail>` is deref'd to `&HttpProfileEntryDetail` for the details widget. This avoids cloning.
