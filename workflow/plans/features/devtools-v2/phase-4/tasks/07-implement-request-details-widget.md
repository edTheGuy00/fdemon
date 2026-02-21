## Task: Implement Request Details Widget

**Objective**: Create the `RequestDetails` widget that renders detailed information about a selected HTTP request. Includes a sub-tab bar for switching between General, Headers, Request Body, Response Body, and Timing views. This is the right/bottom panel of the Network Monitor.

**Depends on**: Task 01 (add-network-domain-types), Task 03 (add-network-state-and-messages)

### Scope

- `crates/fdemon-tui/src/widgets/devtools/network/request_details.rs`: **NEW** — `RequestDetails` widget with all sub-tab renderers

### Details

#### `RequestDetails` struct

```rust
//! # Network Request Details Widget
//!
//! Renders detailed information about a selected HTTP request, with sub-tab
//! switching between General, Headers, Request Body, Response Body, and Timing.

use fdemon_core::network::{
    HttpProfileEntry, HttpProfileEntryDetail, NetworkDetailTab,
    NetworkTiming, format_bytes, format_duration_ms,
};
use ratatui::prelude::*;
use ratatui::widgets::Widget;

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
    pub fn new(
        entry: &'a HttpProfileEntry,
        detail: Option<&'a HttpProfileEntryDetail>,
        active_tab: NetworkDetailTab,
        loading: bool,
    ) -> Self {
        Self { entry, detail, active_tab, loading }
    }
}
```

#### Main render

```rust
impl Widget for RequestDetails<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height < 3 { return; }

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
```

#### Sub-tab bar

```rust
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
            Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        let padded = format!(" {} ", label);
        buf.set_string(x, area.y, &padded, style);
        x += padded.len() as u16;
        if x >= area.right() { break; }
    }
}
```

#### General tab

Shows method, full URI, status, content-type, start/end time, duration, request/response sizes.

```rust
fn render_general(&self, area: Rect, buf: &mut Buffer) {
    let label_style = Style::default().fg(Color::DarkGray);
    let value_style = Style::default().fg(Color::White);
    let mut y = area.y;
    let x_label = area.x + 1;
    let x_value = area.x + 18; // column for values

    // Method + URI
    buf.set_string(x_label, y, "Method:", label_style);
    buf.set_string(x_value, y, &self.entry.method, method_style(&self.entry.method));
    y += 1;

    buf.set_string(x_label, y, "URI:", label_style);
    let uri_width = area.right().saturating_sub(x_value) as usize;
    buf.set_string(x_value, y, &truncate_str(&self.entry.uri, uri_width), value_style);
    y += 1;

    // Status
    buf.set_string(x_label, y, "Status:", label_style);
    let (status_text, status_style) = match self.entry.status_code {
        Some(code) => (code.to_string(), status_color(code)),
        None if self.entry.error.is_some() => ("Error".to_string(), Style::default().fg(Color::Red)),
        None => ("Pending".to_string(), Style::default().fg(Color::DarkGray)),
    };
    buf.set_string(x_value, y, &status_text, status_style);
    y += 1;

    // Content-Type
    if let Some(ct) = &self.entry.content_type {
        buf.set_string(x_label, y, "Content-Type:", label_style);
        buf.set_string(x_value, y, ct, value_style);
        y += 1;
    }

    // Duration
    buf.set_string(x_label, y, "Duration:", label_style);
    let dur_text = self.entry.duration_ms()
        .map(|ms| format_duration_ms(ms))
        .unwrap_or_else(|| "Pending...".to_string());
    buf.set_string(x_value, y, &dur_text, value_style);
    y += 1;

    // Sizes
    if let Some(len) = self.entry.request_content_length.filter(|&l| l >= 0) {
        buf.set_string(x_label, y, "Request Size:", label_style);
        buf.set_string(x_value, y, &format_bytes(len as u64), value_style);
        y += 1;
    }
    if let Some(len) = self.entry.response_content_length.filter(|&l| l >= 0) {
        buf.set_string(x_label, y, "Response Size:", label_style);
        buf.set_string(x_value, y, &format_bytes(len as u64), value_style);
        y += 1;
    }

    // Error
    if let Some(err) = &self.entry.error {
        y += 1; // blank line
        buf.set_string(x_label, y, "Error:", Style::default().fg(Color::Red));
        buf.set_string(x_value, y, err, Style::default().fg(Color::Red));
    }

    // Connection info (if detail available)
    if let Some(detail) = self.detail {
        if let Some(conn) = &detail.connection_info {
            y += 1;
            buf.set_string(x_label, y, "Remote:", label_style);
            let addr = format!("{}:{}",
                conn.remote_address.as_deref().unwrap_or("?"),
                conn.remote_port.unwrap_or(0),
            );
            buf.set_string(x_value, y, &addr, value_style);
        }
    }
}
```

#### Headers tab

```rust
fn render_headers(&self, area: Rect, buf: &mut Buffer) {
    let Some(detail) = self.detail else {
        buf.set_string(area.x + 1, area.y + 1, "Select a request to view headers",
            Style::default().fg(Color::DarkGray));
        return;
    };

    let header_style = Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD);
    let key_style = Style::default().fg(Color::Yellow);
    let value_style = Style::default().fg(Color::White);
    let mut y = area.y;

    // Request Headers
    buf.set_string(area.x + 1, y, "Request Headers", header_style);
    y += 1;
    for (key, values) in &detail.request_headers {
        if y >= area.bottom() { break; }
        let val_str = values.join(", ");
        buf.set_string(area.x + 2, y, &format!("{}: ", key), key_style);
        let val_x = area.x + 2 + key.len() as u16 + 2;
        let max_w = area.right().saturating_sub(val_x) as usize;
        buf.set_string(val_x, y, &truncate_str(&val_str, max_w), value_style);
        y += 1;
    }
    if detail.request_headers.is_empty() {
        buf.set_string(area.x + 2, y, "(none)", Style::default().fg(Color::DarkGray));
        y += 1;
    }

    y += 1; // separator

    // Response Headers
    if y < area.bottom() {
        buf.set_string(area.x + 1, y, "Response Headers", header_style);
        y += 1;
        for (key, values) in &detail.response_headers {
            if y >= area.bottom() { break; }
            let val_str = values.join(", ");
            buf.set_string(area.x + 2, y, &format!("{}: ", key), key_style);
            let val_x = area.x + 2 + key.len() as u16 + 2;
            let max_w = area.right().saturating_sub(val_x) as usize;
            buf.set_string(val_x, y, &truncate_str(&val_str, max_w), value_style);
            y += 1;
        }
        if detail.response_headers.is_empty() {
            buf.set_string(area.x + 2, y, "(none)", Style::default().fg(Color::DarkGray));
        }
    }
}
```

#### Request/Response body tabs

```rust
fn render_request_body(&self, area: Rect, buf: &mut Buffer) {
    self.render_body(area, buf, true);
}

fn render_response_body(&self, area: Rect, buf: &mut Buffer) {
    self.render_body(area, buf, false);
}

fn render_body(&self, area: Rect, buf: &mut Buffer, is_request: bool) {
    let Some(detail) = self.detail else {
        buf.set_string(area.x + 1, area.y + 1, "Loading...",
            Style::default().fg(Color::DarkGray));
        return;
    };

    let body = if is_request { &detail.request_body } else { &detail.response_body };
    if body.is_empty() {
        let label = if is_request { "No request body" } else { "No response body" };
        buf.set_string(area.x + 1, area.y + 1, label,
            Style::default().fg(Color::DarkGray));
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
                if y >= area.bottom() { break; }
                let max_w = area.width.saturating_sub(1) as usize;
                buf.set_string(area.x + 1, y, &truncate_str(line, max_w),
                    Style::default().fg(Color::White));
            }
        }
        None => {
            // Binary data — show size
            let msg = format!("Binary data ({}) — cannot display", format_bytes(body.len() as u64));
            buf.set_string(area.x + 1, area.y + 1, &msg,
                Style::default().fg(Color::DarkGray));
        }
    }
}
```

#### Timing tab

Renders a simple waterfall-style timing breakdown.

```rust
fn render_timing(&self, area: Rect, buf: &mut Buffer) {
    let Some(detail) = self.detail else {
        buf.set_string(area.x + 1, area.y + 1, "Loading...",
            Style::default().fg(Color::DarkGray));
        return;
    };

    let timing = detail.timing();
    let label_style = Style::default().fg(Color::DarkGray);
    let bar_width = area.width.saturating_sub(25) as usize; // reserve space for labels + values
    let total = timing.total_ms.max(1.0); // prevent division by zero
    let mut y = area.y;

    // Total duration header
    buf.set_string(area.x + 1, y, &format!("Total: {}", format_duration_ms(timing.total_ms)),
        Style::default().fg(Color::White).add_modifier(Modifier::BOLD));
    y += 2;

    // Timing phases as horizontal bars
    let phases: Vec<(&str, Option<f64>, Color)> = vec![
        ("Connect", timing.connection_ms, Color::Cyan),
        ("Wait", timing.waiting_ms, Color::Yellow),
        ("Receive", timing.receiving_ms, Color::Green),
    ];

    for (label, duration_opt, color) in &phases {
        if y >= area.bottom() { break; }
        let duration = duration_opt.unwrap_or(0.0);
        let bar_len = ((duration / total) * bar_width as f64) as usize;

        // Label
        buf.set_string(area.x + 1, y, &format!("{:>10}", label), label_style);

        // Bar
        let bar: String = "█".repeat(bar_len.max(if duration > 0.0 { 1 } else { 0 }));
        buf.set_string(area.x + 12, y, &bar, Style::default().fg(*color));

        // Duration value
        let val_x = area.x + 12 + bar_len as u16 + 1;
        if val_x < area.right() {
            buf.set_string(val_x, y, &format_duration_ms(duration),
                Style::default().fg(Color::Gray));
        }
        y += 1;
    }

    // Event timeline
    y += 1;
    if y < area.bottom() && !detail.events.is_empty() {
        buf.set_string(area.x + 1, y, "Events:", label_style);
        y += 1;
        for event in &detail.events {
            if y >= area.bottom() { break; }
            let offset_ms = (event.timestamp_us - self.entry.start_time_us) as f64 / 1000.0;
            let line = format!("  +{} {}", format_duration_ms(offset_ms), event.event);
            buf.set_string(area.x + 1, y, &truncate_str(&line, area.width as usize - 2),
                Style::default().fg(Color::Gray));
            y += 1;
        }
    }
}
```

### Acceptance Criteria

1. `RequestDetails` widget renders without panic for any terminal size
2. Sub-tab bar shows all 5 tabs with correct key hints
3. Active tab highlighted with `bg(Cyan) + fg(Black) + Bold`
4. General tab shows method, URI, status, content-type, duration, sizes
5. General tab shows error message when request failed
6. Headers tab shows request and response headers with key:value formatting
7. Request body tab shows UTF-8 body text or "No request body"
8. Response body tab shows UTF-8 body text or "No response body"
9. Binary body shows "Binary data (X KB) — cannot display"
10. Timing tab shows waterfall bars proportional to duration
11. Timing tab shows event timeline with relative timestamps
12. Loading state shows "Loading request details..." message
13. All new code has unit tests (15+ tests)

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use fdemon_core::network::*;

    fn make_entry() -> HttpProfileEntry { /* standard test entry */ }
    fn make_detail() -> HttpProfileEntryDetail { /* entry + headers + body + events */ }

    fn render_to_buf(widget: RequestDetails, w: u16, h: u16) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, w, h));
        widget.render(Rect::new(0, 0, w, h), &mut buf);
        buf
    }

    #[test]
    fn test_renders_without_panic() { /* 80x24, each tab */ }

    #[test]
    fn test_renders_tiny_terminal() { /* 20x3 */ }

    #[test]
    fn test_tab_bar_shows_all_tabs() { /* check text contains all labels */ }

    #[test]
    fn test_general_tab_shows_method_and_uri() { /* check text */ }

    #[test]
    fn test_general_tab_shows_error() { /* entry with error field */ }

    #[test]
    fn test_headers_tab_shows_headers() { /* detail with headers */ }

    #[test]
    fn test_headers_tab_no_detail_shows_message() { /* detail = None */ }

    #[test]
    fn test_body_tab_shows_text() { /* detail with UTF-8 body */ }

    #[test]
    fn test_body_tab_empty_body() { /* empty body vec */ }

    #[test]
    fn test_body_tab_binary_data() { /* non-UTF-8 bytes */ }

    #[test]
    fn test_timing_tab_shows_bars() { /* detail with events */ }

    #[test]
    fn test_loading_state() { /* loading=true */ }

    #[test]
    fn test_status_color_ranges() { /* 200=green, 404=yellow, 500=red */ }
}
```

### Notes

- **Sub-tab keys `q` and `s`**: Using `q` for Request body and `s` for Response body avoids conflict with `r` (which is used globally for refresh/reload). The mnemonic is: `q` = re**Q**uest, `s` = re**S**ponse.
- **Body display is basic**: No JSON pretty-printing or syntax highlighting in this initial implementation. Bodies are displayed as raw text. JSON formatting can be added as a follow-up enhancement.
- **Scroll support deferred**: The body and headers views don't support scrolling in this initial implementation. For long bodies/headers, content is simply truncated at the bottom of the area. Scroll support can be added later by tracking a per-tab scroll offset.
- **Timing breakdown is approximate**: The timing phases (Connect, Wait, Receive) are computed from event timestamps which may not always be present. When events are missing, the corresponding bar is not shown. The total duration from `entry.duration_ms()` is always accurate.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/widgets/devtools/network/request_details.rs` | NEW — `RequestDetails` widget with all 5 sub-tab renderers (General, Headers, RequestBody, ResponseBody, Timing), helper functions `method_style` and `status_color`, and 41 unit tests |
| `crates/fdemon-tui/src/widgets/devtools/network/mod.rs` | Added `mod request_details` declaration and `pub use request_details::RequestDetails` re-export |

### Notable Decisions/Tradeoffs

1. **Bounds checking throughout render methods**: Added `y >= area.bottom()` guards at every row-advance to prevent rendering outside the allocated area, which satisfies the "no panic for any terminal size" acceptance criterion.
2. **`truncate_str` reuse**: Used the existing `pub(super)` helper from `devtools/mod.rs` via `super::super::truncate_str`, staying consistent with the codebase pattern rather than duplicating logic.
3. **3xx mapped to Cyan in `status_color`**: The task spec only specified 2xx=green, 4xx=yellow, 5xx=red. I added 3xx=cyan as a reasonable choice for redirects, consistent with common HTTP tooling conventions.
4. **`network/mod.rs` already existed**: Task 06 (request_table) had already created this file, so I only needed to add `request_details` to the existing module declaration rather than creating it fresh.

### Testing Performed

- `cargo fmt --all` - Passed (auto-formatted)
- `cargo check -p fdemon-tui` - Passed (no errors)
- `cargo test -p fdemon-tui -- widgets::devtools::network::request_details` - Passed (41/41 tests)
- `cargo test -p fdemon-tui network` - Passed (69/69 network tests including pre-existing request_table tests)
- `cargo check --workspace` - Passed (no errors)

### Risks/Limitations

1. **Pre-existing failing test**: `test_allocation_table_none_profile` in the performance memory_chart module was already failing before this task — not introduced by our changes.
2. **Scroll not implemented**: As noted in the task, body and header content is truncated at the bottom of the area without scroll support. This is expected per the task spec.
3. **No JSON pretty-printing**: Bodies are shown as raw text. JSON formatting is deferred as a follow-up enhancement per the task spec.
