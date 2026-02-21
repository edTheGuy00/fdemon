//! # Network Request Details Widget
//!
//! Renders detailed information about a selected HTTP request, with sub-tab
//! switching between General, Headers, Request Body, Response Body, and Timing.

use fdemon_app::session::NetworkDetailTab;
use fdemon_core::network::{
    format_bytes, format_duration_ms, HttpProfileEntry, HttpProfileEntryDetail,
};

/// Width of the label column in the General tab layout (characters).
const LABEL_COL_WIDTH: u16 = 18;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::Widget,
};

use super::super::truncate_str;

// ── RequestDetails ────────────────────────────────────────────────────────────

/// Widget that renders detailed information about a selected HTTP request.
///
/// Displays a sub-tab bar at the top for switching between General, Headers,
/// Request Body, Response Body, and Timing views. The right/bottom panel of
/// the Network Monitor.
pub struct RequestDetails<'a> {
    /// The selected entry summary (always available when this widget is shown).
    entry: &'a HttpProfileEntry,
    /// Full detail (may be None while loading).
    detail: Option<&'a HttpProfileEntryDetail>,
    /// Active sub-tab.
    active_tab: NetworkDetailTab,
    /// Whether detail is currently loading.
    loading: bool,
}

impl<'a> RequestDetails<'a> {
    /// Create a new `RequestDetails` widget.
    pub fn new(
        entry: &'a HttpProfileEntry,
        detail: Option<&'a HttpProfileEntryDetail>,
        active_tab: NetworkDetailTab,
        loading: bool,
    ) -> Self {
        Self {
            entry,
            detail,
            active_tab,
            loading,
        }
    }
}

impl Widget for RequestDetails<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height < 3 {
            return;
        }

        // Row 0: Sub-tab bar
        self.render_tab_bar(Rect { height: 1, ..area }, buf);

        // Remaining: Tab content
        let content_area = Rect {
            y: area.y + 1,
            height: area.height.saturating_sub(1),
            ..area
        };

        if self.loading {
            let msg = "Loading request details...";
            let x = content_area.x + 1;
            let y = content_area.y + 1;
            buf.set_string(x, y, msg, Style::default().fg(Color::DarkGray));
            return;
        }

        match self.active_tab {
            NetworkDetailTab::General => self.render_general(content_area, buf),
            NetworkDetailTab::Headers => self.render_headers(content_area, buf),
            NetworkDetailTab::RequestBody => self.render_request_body(content_area, buf),
            NetworkDetailTab::ResponseBody => self.render_response_body(content_area, buf),
            NetworkDetailTab::Timing => self.render_timing(content_area, buf),
        }
    }
}

impl RequestDetails<'_> {
    // ── Sub-tab bar ───────────────────────────────────────────────────────────

    fn render_tab_bar(&self, area: Rect, buf: &mut Buffer) {
        let tabs = [
            (NetworkDetailTab::General, "[g] General"),
            (NetworkDetailTab::Headers, "[h] Headers"),
            (NetworkDetailTab::RequestBody, "[q] Request"),
            (NetworkDetailTab::ResponseBody, "[s] Response"),
            (NetworkDetailTab::Timing, "[t] Timing"),
        ];

        let mut x = area.x;
        for (tab, label) in &tabs {
            let style = if *tab == self.active_tab {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };
            let padded = format!(" {} ", label);
            buf.set_string(x, area.y, &padded, style);
            x += padded.len() as u16;
            if x >= area.right() {
                break;
            }
        }
    }

    // ── General tab ───────────────────────────────────────────────────────────

    fn render_general(&self, area: Rect, buf: &mut Buffer) {
        let label_style = Style::default().fg(Color::DarkGray);
        let value_style = Style::default().fg(Color::White);
        let mut y = area.y;
        let x_label = area.x + 1;
        let x_value = area.x + LABEL_COL_WIDTH;

        if y >= area.bottom() {
            return;
        }

        // Method + URI
        buf.set_string(x_label, y, "Method:", label_style);
        buf.set_string(
            x_value,
            y,
            &self.entry.method,
            Style::default().fg(super::http_method_color(&self.entry.method)),
        );
        y += 1;

        if y >= area.bottom() {
            return;
        }

        buf.set_string(x_label, y, "URI:", label_style);
        let uri_width = area.right().saturating_sub(x_value) as usize;
        buf.set_string(
            x_value,
            y,
            truncate_str(&self.entry.uri, uri_width),
            value_style,
        );
        y += 1;

        if y >= area.bottom() {
            return;
        }

        // Status
        buf.set_string(x_label, y, "Status:", label_style);
        let (status_text, status_style) = match self.entry.status_code {
            Some(code) => (code.to_string(), status_color(code)),
            None if self.entry.error.is_some() => {
                ("Error".to_string(), Style::default().fg(Color::Red))
            }
            None => ("Pending".to_string(), Style::default().fg(Color::DarkGray)),
        };
        buf.set_string(x_value, y, &status_text, status_style);
        y += 1;

        // Content-Type
        if let Some(ct) = &self.entry.content_type {
            if y >= area.bottom() {
                return;
            }
            buf.set_string(x_label, y, "Content-Type:", label_style);
            buf.set_string(x_value, y, ct, value_style);
            y += 1;
        }

        if y >= area.bottom() {
            return;
        }

        // Duration
        buf.set_string(x_label, y, "Duration:", label_style);
        let dur_text = self
            .entry
            .duration_ms()
            .map(format_duration_ms)
            .unwrap_or_else(|| "Pending...".to_string());
        buf.set_string(x_value, y, &dur_text, value_style);
        y += 1;

        // Request size
        if let Some(len) = self.entry.request_content_length.filter(|&l| l >= 0) {
            if y >= area.bottom() {
                return;
            }
            buf.set_string(x_label, y, "Request Size:", label_style);
            buf.set_string(x_value, y, format_bytes(len as u64), value_style);
            y += 1;
        }

        // Response size
        if let Some(len) = self.entry.response_content_length.filter(|&l| l >= 0) {
            if y >= area.bottom() {
                return;
            }
            buf.set_string(x_label, y, "Response Size:", label_style);
            buf.set_string(x_value, y, format_bytes(len as u64), value_style);
            y += 1;
        }

        // Error
        if let Some(err) = &self.entry.error {
            y += 1; // blank line
            if y >= area.bottom() {
                return;
            }
            buf.set_string(x_label, y, "Error:", Style::default().fg(Color::Red));
            let err_x = x_value;
            let max_w = area.right().saturating_sub(err_x) as usize;
            buf.set_string(
                err_x,
                y,
                truncate_str(err, max_w),
                Style::default().fg(Color::Red),
            );
            y += 1;
        }

        // Connection info (if detail available)
        if let Some(detail) = self.detail {
            if let Some(conn) = &detail.connection_info {
                if y >= area.bottom() {
                    return;
                }
                buf.set_string(x_label, y, "Remote:", label_style);
                let addr = format!(
                    "{}:{}",
                    conn.remote_address.as_deref().unwrap_or("?"),
                    conn.remote_port.unwrap_or(0),
                );
                buf.set_string(x_value, y, &addr, value_style);
            }
        }
    }

    // ── Headers tab ───────────────────────────────────────────────────────────

    fn render_headers(&self, area: Rect, buf: &mut Buffer) {
        let Some(detail) = self.detail else {
            buf.set_string(
                area.x + 1,
                area.y + 1,
                "Select a request to view headers",
                Style::default().fg(Color::DarkGray),
            );
            return;
        };

        let header_style = Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD);
        let key_style = Style::default().fg(Color::Yellow);
        let value_style = Style::default().fg(Color::White);
        let mut y = area.y;

        if y >= area.bottom() {
            return;
        }

        // Request Headers
        buf.set_string(area.x + 1, y, "Request Headers", header_style);
        y += 1;
        for (key, values) in &detail.request_headers {
            if y >= area.bottom() {
                break;
            }
            let val_str = values.join(", ");
            buf.set_string(area.x + 2, y, format!("{}: ", key), key_style);
            let val_x = area.x + 2 + key.len() as u16 + 2;
            let max_w = area.right().saturating_sub(val_x) as usize;
            buf.set_string(val_x, y, truncate_str(&val_str, max_w), value_style);
            y += 1;
        }
        if detail.request_headers.is_empty() && y < area.bottom() {
            buf.set_string(
                area.x + 2,
                y,
                "(none)",
                Style::default().fg(Color::DarkGray),
            );
            y += 1;
        }

        y += 1; // separator

        // Response Headers
        if y < area.bottom() {
            buf.set_string(area.x + 1, y, "Response Headers", header_style);
            y += 1;
            for (key, values) in &detail.response_headers {
                if y >= area.bottom() {
                    break;
                }
                let val_str = values.join(", ");
                buf.set_string(area.x + 2, y, format!("{}: ", key), key_style);
                let val_x = area.x + 2 + key.len() as u16 + 2;
                let max_w = area.right().saturating_sub(val_x) as usize;
                buf.set_string(val_x, y, truncate_str(&val_str, max_w), value_style);
                y += 1;
            }
            if detail.response_headers.is_empty() && y < area.bottom() {
                buf.set_string(
                    area.x + 2,
                    y,
                    "(none)",
                    Style::default().fg(Color::DarkGray),
                );
            }
        }
    }

    // ── Request/Response body tabs ────────────────────────────────────────────

    fn render_request_body(&self, area: Rect, buf: &mut Buffer) {
        self.render_body(area, buf, true);
    }

    fn render_response_body(&self, area: Rect, buf: &mut Buffer) {
        self.render_body(area, buf, false);
    }

    fn render_body(&self, area: Rect, buf: &mut Buffer, is_request: bool) {
        let Some(detail) = self.detail else {
            buf.set_string(
                area.x + 1,
                area.y + 1,
                "Loading...",
                Style::default().fg(Color::DarkGray),
            );
            return;
        };

        let body = if is_request {
            &detail.request_body
        } else {
            &detail.response_body
        };

        if body.is_empty() {
            let label = if is_request {
                "No request body"
            } else {
                "No response body"
            };
            buf.set_string(
                area.x + 1,
                area.y + 1,
                label,
                Style::default().fg(Color::DarkGray),
            );
            return;
        }

        // Try to decode as UTF-8 and display
        let text = if is_request {
            detail.request_body_text()
        } else {
            detail.response_body_text()
        };

        match text {
            Some(text) => {
                // Render text line by line
                for (i, line) in text.lines().enumerate() {
                    let y = area.y + i as u16;
                    if y >= area.bottom() {
                        break;
                    }
                    let max_w = area.width.saturating_sub(1) as usize;
                    buf.set_string(
                        area.x + 1,
                        y,
                        truncate_str(line, max_w),
                        Style::default().fg(Color::White),
                    );
                }
            }
            None => {
                // Binary data — show size
                let msg = format!(
                    "Binary data ({}) — cannot display",
                    format_bytes(body.len() as u64)
                );
                buf.set_string(
                    area.x + 1,
                    area.y + 1,
                    &msg,
                    Style::default().fg(Color::DarkGray),
                );
            }
        }
    }

    // ── Timing tab ────────────────────────────────────────────────────────────

    fn render_timing(&self, area: Rect, buf: &mut Buffer) {
        let Some(detail) = self.detail else {
            buf.set_string(
                area.x + 1,
                area.y + 1,
                "Loading...",
                Style::default().fg(Color::DarkGray),
            );
            return;
        };

        let timing = detail.timing();
        let label_style = Style::default().fg(Color::DarkGray);
        // Reserve space for labels + values
        let bar_width = area.width.saturating_sub(25) as usize;
        let total = timing.total_ms.max(1.0); // prevent division by zero
        let mut y = area.y;

        if y >= area.bottom() {
            return;
        }

        // Total duration header
        buf.set_string(
            area.x + 1,
            y,
            format!("Total: {}", format_duration_ms(timing.total_ms)),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        );
        y += 2;

        // Timing phases as horizontal bars
        let phases: Vec<(&str, Option<f64>, Color)> = vec![
            ("Connect", timing.connection_ms, Color::Cyan),
            ("Wait", timing.waiting_ms, Color::Yellow),
            ("Receive", timing.receiving_ms, Color::Green),
        ];

        for (label, duration_opt, color) in &phases {
            if y >= area.bottom() {
                break;
            }
            let duration = duration_opt.unwrap_or(0.0);
            let bar_len = ((duration / total) * bar_width as f64) as usize;

            // Label
            buf.set_string(area.x + 1, y, format!("{:>10}", label), label_style);

            // Bar
            let min_bar = if duration > 0.0 { 1 } else { 0 };
            let bar: String = "\u{2588}".repeat(bar_len.max(min_bar));
            buf.set_string(area.x + 12, y, &bar, Style::default().fg(*color));

            // Duration value
            let val_x = area.x + 12 + bar_len as u16 + 1;
            if val_x < area.right() {
                buf.set_string(
                    val_x,
                    y,
                    format_duration_ms(duration),
                    Style::default().fg(Color::Gray),
                );
            }
            y += 1;
        }

        // Event timeline
        y += 1;
        if y < area.bottom() && !detail.events.is_empty() {
            buf.set_string(area.x + 1, y, "Events:", label_style);
            y += 1;
            for event in &detail.events {
                if y >= area.bottom() {
                    break;
                }
                let offset_ms = (event.timestamp_us - self.entry.start_time_us) as f64 / 1000.0;
                let line = format!("  +{} {}", format_duration_ms(offset_ms), event.event);
                buf.set_string(
                    area.x + 1,
                    y,
                    truncate_str(&line, area.width.saturating_sub(2) as usize),
                    Style::default().fg(Color::Gray),
                );
                y += 1;
            }
        }
    }
}

// ── Style helpers ─────────────────────────────────────────────────────────────

/// Choose a style for the HTTP status code.
fn status_color(code: u16) -> Style {
    if code < 300 {
        Style::default().fg(Color::Green)
    } else if code < 400 {
        Style::default().fg(Color::Cyan)
    } else if code < 500 {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Red)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use fdemon_core::network::{
        ConnectionInfo, HttpProfileEntry, HttpProfileEntryDetail, HttpProfileEvent,
    };
    use ratatui::{buffer::Buffer, layout::Rect};

    // ── Test helpers ──────────────────────────────────────────────────────────

    fn make_entry() -> HttpProfileEntry {
        HttpProfileEntry {
            id: "req_1".to_string(),
            method: "GET".to_string(),
            uri: "https://api.example.com/users".to_string(),
            status_code: Some(200),
            content_type: Some("application/json".to_string()),
            start_time_us: 1_000_000,
            end_time_us: Some(1_050_000),
            request_content_length: None,
            response_content_length: Some(1024),
            error: None,
        }
    }

    fn make_detail() -> HttpProfileEntryDetail {
        HttpProfileEntryDetail {
            entry: make_entry(),
            request_headers: vec![
                (
                    "Content-Type".to_string(),
                    vec!["application/json".to_string()],
                ),
                (
                    "Authorization".to_string(),
                    vec!["Bearer token123".to_string()],
                ),
            ],
            response_headers: vec![
                (
                    "Content-Type".to_string(),
                    vec!["application/json".to_string()],
                ),
                ("X-Request-Id".to_string(), vec!["abc-123".to_string()]),
            ],
            request_body: b"".to_vec(),
            response_body: b"{\"users\":[]}".to_vec(),
            events: vec![
                HttpProfileEvent {
                    event: "connection established".to_string(),
                    timestamp_us: 1_010_000,
                },
                HttpProfileEvent {
                    event: "response started".to_string(),
                    timestamp_us: 1_040_000,
                },
            ],
            connection_info: Some(ConnectionInfo {
                local_port: Some(54321),
                remote_address: Some("93.184.216.34".to_string()),
                remote_port: Some(443),
            }),
        }
    }

    fn render_to_buf(widget: RequestDetails<'_>, w: u16, h: u16) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, w, h));
        widget.render(Rect::new(0, 0, w, h), &mut buf);
        buf
    }

    fn collect_buf_text(buf: &Buffer, width: u16, height: u16) -> String {
        let mut full = String::new();
        for y in 0..height {
            for x in 0..width {
                if let Some(c) = buf.cell((x, y)) {
                    if let Some(ch) = c.symbol().chars().next() {
                        full.push(ch);
                    }
                }
            }
        }
        full
    }

    fn buf_contains(buf: &Buffer, w: u16, h: u16, text: &str) -> bool {
        collect_buf_text(buf, w, h).contains(text)
    }

    // ── Basic render tests ────────────────────────────────────────────────────

    #[test]
    fn test_renders_without_panic() {
        let entry = make_entry();
        let detail = make_detail();

        for tab in [
            NetworkDetailTab::General,
            NetworkDetailTab::Headers,
            NetworkDetailTab::RequestBody,
            NetworkDetailTab::ResponseBody,
            NetworkDetailTab::Timing,
        ] {
            let widget = RequestDetails::new(&entry, Some(&detail), tab, false);
            let buf = render_to_buf(widget, 80, 24);
            // Verify we can read the buf text without panicking
            let _ = collect_buf_text(&buf, 80, 24);
        }
    }

    #[test]
    fn test_renders_tiny_terminal() {
        let entry = make_entry();
        let widget = RequestDetails::new(&entry, None, NetworkDetailTab::General, false);
        // Height < 3 should be a no-op
        let buf = render_to_buf(widget, 20, 2);
        let text = collect_buf_text(&buf, 20, 2);
        // Nothing should be rendered (early return)
        assert!(
            text.chars().all(|c| c == ' '),
            "Tiny terminal should render nothing, got: {text:?}"
        );
    }

    #[test]
    fn test_renders_minimum_height() {
        let entry = make_entry();
        let widget = RequestDetails::new(&entry, None, NetworkDetailTab::General, false);
        // Height == 3 should render tab bar and at least one content row
        let buf = render_to_buf(widget, 80, 3);
        // Should not panic — that's the key requirement
        let _ = collect_buf_text(&buf, 80, 3);
    }

    // ── Tab bar tests ─────────────────────────────────────────────────────────

    #[test]
    fn test_tab_bar_shows_all_tabs() {
        let entry = make_entry();
        let widget = RequestDetails::new(&entry, None, NetworkDetailTab::General, false);
        let buf = render_to_buf(widget, 80, 10);
        let text = collect_buf_text(&buf, 80, 10);

        assert!(
            text.contains("General"),
            "Should show General tab, got: {text:?}"
        );
        assert!(
            text.contains("Headers"),
            "Should show Headers tab, got: {text:?}"
        );
        assert!(
            text.contains("Request"),
            "Should show Request tab, got: {text:?}"
        );
        assert!(
            text.contains("Response"),
            "Should show Response tab, got: {text:?}"
        );
        assert!(
            text.contains("Timing"),
            "Should show Timing tab, got: {text:?}"
        );
    }

    #[test]
    fn test_tab_bar_shows_key_hints() {
        let entry = make_entry();
        let widget = RequestDetails::new(&entry, None, NetworkDetailTab::General, false);
        let buf = render_to_buf(widget, 80, 5);
        let text = collect_buf_text(&buf, 80, 5);

        // Check for key hints in tab bar
        assert!(text.contains("[g]"), "Should show [g] key hint");
        assert!(text.contains("[h]"), "Should show [h] key hint");
        assert!(text.contains("[q]"), "Should show [q] key hint");
        assert!(text.contains("[s]"), "Should show [s] key hint");
        assert!(text.contains("[t]"), "Should show [t] key hint");
    }

    #[test]
    fn test_active_tab_highlighted() {
        // Test that the active tab cell has Cyan background
        let entry = make_entry();
        let widget = RequestDetails::new(&entry, None, NetworkDetailTab::General, false);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 5));
        widget.render(Rect::new(0, 0, 80, 5), &mut buf);

        // The active "General" tab should have Cyan bg and Black fg
        // Find a cell that's part of "General" text on row 0
        let row0_cells: Vec<_> = (0..80)
            .filter_map(|x| buf.cell((x, 0)))
            .filter(|c| c.symbol().contains('G') || c.symbol().contains('e'))
            .collect();

        // At least some cell in the General tab area should have Cyan bg
        let has_cyan = row0_cells.iter().any(|c| c.style().bg == Some(Color::Cyan));
        assert!(has_cyan, "Active tab should have Cyan background");
    }

    // ── General tab tests ─────────────────────────────────────────────────────

    #[test]
    fn test_general_tab_shows_method_and_uri() {
        let entry = make_entry();
        let widget = RequestDetails::new(&entry, None, NetworkDetailTab::General, false);
        let buf = render_to_buf(widget, 80, 20);

        assert!(buf_contains(&buf, 80, 20, "GET"), "Should show method");
        assert!(
            buf_contains(&buf, 80, 20, "api.example.com"),
            "Should show URI"
        );
    }

    #[test]
    fn test_general_tab_shows_status() {
        let entry = make_entry();
        let widget = RequestDetails::new(&entry, None, NetworkDetailTab::General, false);
        let buf = render_to_buf(widget, 80, 20);

        assert!(buf_contains(&buf, 80, 20, "200"), "Should show status code");
    }

    #[test]
    fn test_general_tab_shows_content_type() {
        let entry = make_entry();
        let widget = RequestDetails::new(&entry, None, NetworkDetailTab::General, false);
        let buf = render_to_buf(widget, 80, 20);

        assert!(
            buf_contains(&buf, 80, 20, "application/json"),
            "Should show content-type"
        );
    }

    #[test]
    fn test_general_tab_shows_duration() {
        let entry = make_entry();
        let widget = RequestDetails::new(&entry, None, NetworkDetailTab::General, false);
        let buf = render_to_buf(widget, 80, 20);

        // Duration should be 50ms (1_050_000 - 1_000_000 = 50_000 us = 50 ms)
        assert!(buf_contains(&buf, 80, 20, "50ms"), "Should show duration");
    }

    #[test]
    fn test_general_tab_shows_response_size() {
        let entry = make_entry();
        let widget = RequestDetails::new(&entry, None, NetworkDetailTab::General, false);
        let buf = render_to_buf(widget, 80, 20);

        // 1024 bytes = 1.0 KB
        assert!(
            buf_contains(&buf, 80, 20, "1.0 KB"),
            "Should show response size"
        );
    }

    #[test]
    fn test_general_tab_shows_error() {
        let mut entry = make_entry();
        entry.status_code = None;
        entry.end_time_us = None;
        entry.error = Some("Connection refused".to_string());

        let widget = RequestDetails::new(&entry, None, NetworkDetailTab::General, false);
        let buf = render_to_buf(widget, 80, 20);

        assert!(
            buf_contains(&buf, 80, 20, "Error"),
            "Should show Error status"
        );
        assert!(
            buf_contains(&buf, 80, 20, "Connection refused"),
            "Should show error message"
        );
    }

    #[test]
    fn test_general_tab_pending_request() {
        let mut entry = make_entry();
        entry.status_code = None;
        entry.end_time_us = None;

        let widget = RequestDetails::new(&entry, None, NetworkDetailTab::General, false);
        let buf = render_to_buf(widget, 80, 20);

        assert!(
            buf_contains(&buf, 80, 20, "Pending"),
            "Should show Pending status"
        );
    }

    #[test]
    fn test_general_tab_shows_connection_info() {
        let entry = make_entry();
        let detail = make_detail();
        let widget = RequestDetails::new(&entry, Some(&detail), NetworkDetailTab::General, false);
        let buf = render_to_buf(widget, 80, 24);

        assert!(
            buf_contains(&buf, 80, 24, "93.184.216.34"),
            "Should show remote address"
        );
        assert!(buf_contains(&buf, 80, 24, "443"), "Should show remote port");
    }

    // ── Headers tab tests ─────────────────────────────────────────────────────

    #[test]
    fn test_headers_tab_shows_request_headers() {
        let entry = make_entry();
        let detail = make_detail();
        let widget = RequestDetails::new(&entry, Some(&detail), NetworkDetailTab::Headers, false);
        let buf = render_to_buf(widget, 80, 24);

        assert!(
            buf_contains(&buf, 80, 24, "Request Headers"),
            "Should show Request Headers section"
        );
        assert!(
            buf_contains(&buf, 80, 24, "Authorization"),
            "Should show Authorization header"
        );
    }

    #[test]
    fn test_headers_tab_shows_response_headers() {
        let entry = make_entry();
        let detail = make_detail();
        let widget = RequestDetails::new(&entry, Some(&detail), NetworkDetailTab::Headers, false);
        let buf = render_to_buf(widget, 80, 24);

        assert!(
            buf_contains(&buf, 80, 24, "Response Headers"),
            "Should show Response Headers section"
        );
        assert!(
            buf_contains(&buf, 80, 24, "X-Request-Id"),
            "Should show X-Request-Id header"
        );
    }

    #[test]
    fn test_headers_tab_no_detail_shows_message() {
        let entry = make_entry();
        let widget = RequestDetails::new(&entry, None, NetworkDetailTab::Headers, false);
        let buf = render_to_buf(widget, 80, 20);

        assert!(
            buf_contains(&buf, 80, 20, "Select a request to view headers"),
            "Should show placeholder when no detail"
        );
    }

    #[test]
    fn test_headers_tab_empty_headers_shows_none() {
        let entry = make_entry();
        let mut detail = make_detail();
        detail.request_headers.clear();
        detail.response_headers.clear();

        let widget = RequestDetails::new(&entry, Some(&detail), NetworkDetailTab::Headers, false);
        let buf = render_to_buf(widget, 80, 24);

        // Should show "(none)" for empty headers
        assert!(
            buf_contains(&buf, 80, 24, "(none)"),
            "Should show (none) for empty headers"
        );
    }

    // ── Body tab tests ────────────────────────────────────────────────────────

    #[test]
    fn test_request_body_tab_empty_body_shows_message() {
        let entry = make_entry();
        let detail = make_detail(); // request_body is empty in make_detail
        let widget =
            RequestDetails::new(&entry, Some(&detail), NetworkDetailTab::RequestBody, false);
        let buf = render_to_buf(widget, 80, 20);

        assert!(
            buf_contains(&buf, 80, 20, "No request body"),
            "Should show 'No request body' for empty request body"
        );
    }

    #[test]
    fn test_response_body_tab_shows_text() {
        let entry = make_entry();
        let detail = make_detail(); // response_body = b"{\"users\":[]}"
        let widget =
            RequestDetails::new(&entry, Some(&detail), NetworkDetailTab::ResponseBody, false);
        let buf = render_to_buf(widget, 80, 20);

        assert!(
            buf_contains(&buf, 80, 20, "users"),
            "Should show response body text"
        );
    }

    #[test]
    fn test_response_body_tab_empty_shows_message() {
        let entry = make_entry();
        let mut detail = make_detail();
        detail.response_body.clear();

        let widget =
            RequestDetails::new(&entry, Some(&detail), NetworkDetailTab::ResponseBody, false);
        let buf = render_to_buf(widget, 80, 20);

        assert!(
            buf_contains(&buf, 80, 20, "No response body"),
            "Should show 'No response body' for empty response body"
        );
    }

    #[test]
    fn test_body_tab_binary_data_shows_message() {
        let entry = make_entry();
        let mut detail = make_detail();
        // Non-UTF-8 bytes (binary data)
        detail.response_body = vec![0xFF, 0xFE, 0x00, 0x01, 0xD8, 0x00];

        let widget =
            RequestDetails::new(&entry, Some(&detail), NetworkDetailTab::ResponseBody, false);
        let buf = render_to_buf(widget, 80, 20);

        assert!(
            buf_contains(&buf, 80, 20, "Binary data"),
            "Should show 'Binary data' for non-UTF-8 response body"
        );
        assert!(
            buf_contains(&buf, 80, 20, "cannot display"),
            "Should show 'cannot display' for binary data"
        );
    }

    #[test]
    fn test_body_tab_no_detail_shows_loading() {
        let entry = make_entry();
        let widget = RequestDetails::new(&entry, None, NetworkDetailTab::ResponseBody, false);
        let buf = render_to_buf(widget, 80, 20);

        assert!(
            buf_contains(&buf, 80, 20, "Loading..."),
            "Should show 'Loading...' when no detail available"
        );
    }

    #[test]
    fn test_request_body_shows_text_when_present() {
        let entry = make_entry();
        let mut detail = make_detail();
        detail.request_body = b"{\"name\":\"Alice\"}".to_vec();

        let widget =
            RequestDetails::new(&entry, Some(&detail), NetworkDetailTab::RequestBody, false);
        let buf = render_to_buf(widget, 80, 20);

        assert!(
            buf_contains(&buf, 80, 20, "Alice"),
            "Should show request body text content"
        );
    }

    // ── Timing tab tests ──────────────────────────────────────────────────────

    #[test]
    fn test_timing_tab_shows_total_duration() {
        let entry = make_entry();
        let detail = make_detail();
        let widget = RequestDetails::new(&entry, Some(&detail), NetworkDetailTab::Timing, false);
        let buf = render_to_buf(widget, 80, 24);

        assert!(
            buf_contains(&buf, 80, 24, "Total:"),
            "Should show 'Total:' label"
        );
        assert!(
            buf_contains(&buf, 80, 24, "50ms"),
            "Should show 50ms total duration"
        );
    }

    #[test]
    fn test_timing_tab_shows_phase_labels() {
        let entry = make_entry();
        let detail = make_detail();
        let widget = RequestDetails::new(&entry, Some(&detail), NetworkDetailTab::Timing, false);
        let buf = render_to_buf(widget, 80, 24);

        assert!(
            buf_contains(&buf, 80, 24, "Connect"),
            "Should show Connect phase"
        );
        assert!(buf_contains(&buf, 80, 24, "Wait"), "Should show Wait phase");
        assert!(
            buf_contains(&buf, 80, 24, "Receive"),
            "Should show Receive phase"
        );
    }

    #[test]
    fn test_timing_tab_shows_events() {
        let entry = make_entry();
        let detail = make_detail();
        let widget = RequestDetails::new(&entry, Some(&detail), NetworkDetailTab::Timing, false);
        let buf = render_to_buf(widget, 80, 24);

        assert!(
            buf_contains(&buf, 80, 24, "Events:"),
            "Should show Events section"
        );
        assert!(
            buf_contains(&buf, 80, 24, "connection established"),
            "Should show connection established event"
        );
    }

    #[test]
    fn test_timing_tab_no_detail_shows_loading() {
        let entry = make_entry();
        let widget = RequestDetails::new(&entry, None, NetworkDetailTab::Timing, false);
        let buf = render_to_buf(widget, 80, 20);

        assert!(
            buf_contains(&buf, 80, 20, "Loading..."),
            "Should show 'Loading...' when no detail"
        );
    }

    #[test]
    fn test_timing_tab_empty_events_no_events_section() {
        let entry = make_entry();
        let mut detail = make_detail();
        detail.events.clear();

        let widget = RequestDetails::new(&entry, Some(&detail), NetworkDetailTab::Timing, false);
        let buf = render_to_buf(widget, 80, 24);

        // Should not crash; "Events:" section should be absent
        assert!(
            !buf_contains(&buf, 80, 24, "Events:"),
            "Should not show Events section when events list is empty"
        );
    }

    // ── Loading state test ────────────────────────────────────────────────────

    #[test]
    fn test_loading_state_shows_message() {
        let entry = make_entry();
        let widget = RequestDetails::new(&entry, None, NetworkDetailTab::General, true);
        let buf = render_to_buf(widget, 80, 20);

        assert!(
            buf_contains(&buf, 80, 20, "Loading request details..."),
            "Should show loading message"
        );
    }

    #[test]
    fn test_loading_state_suppresses_content() {
        let entry = make_entry();
        let detail = make_detail();
        // Even with detail present, loading=true should show loading message
        let widget = RequestDetails::new(&entry, Some(&detail), NetworkDetailTab::General, true);
        let buf = render_to_buf(widget, 80, 20);

        assert!(
            buf_contains(&buf, 80, 20, "Loading request details..."),
            "Loading state should show loading message even when detail is available"
        );
    }

    // ── Status color tests ────────────────────────────────────────────────────

    #[test]
    fn test_status_color_2xx_green() {
        let style = status_color(200);
        assert_eq!(style.fg, Some(Color::Green), "2xx should be green");

        let style = status_color(201);
        assert_eq!(style.fg, Some(Color::Green), "201 should be green");
    }

    #[test]
    fn test_status_color_3xx_cyan() {
        let style = status_color(301);
        assert_eq!(style.fg, Some(Color::Cyan), "3xx should be cyan");

        let style = status_color(302);
        assert_eq!(style.fg, Some(Color::Cyan), "302 should be cyan");
    }

    #[test]
    fn test_status_color_4xx_yellow() {
        let style = status_color(404);
        assert_eq!(style.fg, Some(Color::Yellow), "404 should be yellow");

        let style = status_color(400);
        assert_eq!(style.fg, Some(Color::Yellow), "400 should be yellow");
    }

    #[test]
    fn test_status_color_5xx_red() {
        let style = status_color(500);
        assert_eq!(style.fg, Some(Color::Red), "500 should be red");

        let style = status_color(503);
        assert_eq!(style.fg, Some(Color::Red), "503 should be red");
    }

    // ── Method color tests (delegated to shared http_method_color) ───────────

    #[test]
    fn test_method_color_get_green() {
        assert_eq!(
            super::super::http_method_color("GET"),
            Color::Green,
            "GET should be green"
        );
    }

    #[test]
    fn test_method_color_post_blue() {
        assert_eq!(
            super::super::http_method_color("POST"),
            Color::Blue,
            "POST should be blue (consistent with request table)"
        );
    }

    #[test]
    fn test_method_color_delete_red() {
        assert_eq!(
            super::super::http_method_color("DELETE"),
            Color::Red,
            "DELETE should be red"
        );
    }

    #[test]
    fn test_general_tab_no_content_type_skips_row() {
        let mut entry = make_entry();
        entry.content_type = None;

        let widget = RequestDetails::new(&entry, None, NetworkDetailTab::General, false);
        let buf = render_to_buf(widget, 80, 20);

        // Should still show method, URI, status, duration without crashing
        assert!(
            buf_contains(&buf, 80, 20, "GET"),
            "Should still show method"
        );
        assert!(
            buf_contains(&buf, 80, 20, "200"),
            "Should still show status"
        );
    }

    #[test]
    fn test_general_tab_with_request_and_response_sizes() {
        let mut entry = make_entry();
        entry.request_content_length = Some(512);
        entry.response_content_length = Some(2048);

        let widget = RequestDetails::new(&entry, None, NetworkDetailTab::General, false);
        let buf = render_to_buf(widget, 80, 24);

        assert!(
            buf_contains(&buf, 80, 24, "512 B"),
            "Should show request size of 512 B"
        );
        assert!(
            buf_contains(&buf, 80, 24, "2.0 KB"),
            "Should show response size of 2.0 KB"
        );
    }

    #[test]
    fn test_multiline_response_body() {
        let entry = make_entry();
        let mut detail = make_detail();
        detail.response_body = b"line one\nline two\nline three".to_vec();

        let widget =
            RequestDetails::new(&entry, Some(&detail), NetworkDetailTab::ResponseBody, false);
        let buf = render_to_buf(widget, 80, 20);

        assert!(
            buf_contains(&buf, 80, 20, "line one"),
            "Should show first line"
        );
        assert!(
            buf_contains(&buf, 80, 20, "line two"),
            "Should show second line"
        );
    }
}
