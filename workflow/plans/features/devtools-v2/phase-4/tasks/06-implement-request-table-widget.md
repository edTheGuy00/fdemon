## Task: Implement Request Table Widget

**Objective**: Create the `RequestTable` widget that renders a scrollable, filterable table of HTTP requests with columns for status, method, URI, content type, duration, and size. This is the left/top panel of the Network Monitor.

**Depends on**: Task 01 (add-network-domain-types), Task 03 (add-network-state-and-messages)

### Scope

- `crates/fdemon-tui/src/widgets/devtools/network/request_table.rs`: **NEW** — `RequestTable` widget
- `crates/fdemon-tui/src/widgets/devtools/network/tests.rs`: **NEW** — Widget tests (linked from `mod.rs`)

### Details

#### `RequestTable` struct

```rust
//! # Network Request Table Widget
//!
//! Renders a scrollable table of HTTP requests with status, method, URI,
//! content type, duration, and response size columns. Supports selection
//! highlighting, pending request indicators, and filter highlighting.

use fdemon_core::network::{HttpProfileEntry, format_bytes, format_duration_ms};
use ratatui::prelude::*;
use ratatui::widgets::Widget;

/// Column widths (characters).
const COL_STATUS: u16 = 5;
const COL_METHOD: u16 = 7;
const COL_DURATION: u16 = 8;
const COL_SIZE: u16 = 8;
const COL_TYPE: u16 = 10;
// URI gets the remaining space

pub struct RequestTable<'a> {
    entries: &'a [&'a HttpProfileEntry],
    selected_index: Option<usize>,
    scroll_offset: usize,
    recording: bool,
    filter: &'a str,
}

impl<'a> RequestTable<'a> {
    pub fn new(
        entries: &'a [&'a HttpProfileEntry],
        selected_index: Option<usize>,
        scroll_offset: usize,
        recording: bool,
        filter: &'a str,
    ) -> Self {
        Self { entries, selected_index, scroll_offset, recording, filter }
    }
}
```

#### Rendering

```rust
impl Widget for RequestTable<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height < 2 { return; }

        // Row 0: Header bar (recording indicator + count + filter hint)
        self.render_header(area, buf);

        // Row 1: Column headers
        let header_area = Rect { y: area.y + 1, height: 1, ..area };
        self.render_column_headers(header_area, buf);

        // Rows 2+: Data rows
        let data_area = Rect {
            y: area.y + 2,
            height: area.height.saturating_sub(2),
            ..area
        };
        self.render_rows(data_area, buf);
    }
}
```

#### Header bar

```rust
fn render_header(&self, area: Rect, buf: &mut Buffer) {
    let header_area = Rect { height: 1, ..area };
    // Recording indicator: red dot when recording, gray when paused
    let recording_indicator = if self.recording { "● REC" } else { "○ PAUSED" };
    let recording_style = if self.recording {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let count_text = format!("  {} requests", self.entries.len());
    let filter_text = if self.filter.is_empty() {
        String::new()
    } else {
        format!("  filter: {}", self.filter)
    };

    buf.set_string(area.x, area.y, recording_indicator, recording_style);
    buf.set_string(
        area.x + recording_indicator.len() as u16,
        area.y,
        &count_text,
        Style::default().fg(Color::Gray),
    );
    if !filter_text.is_empty() {
        buf.set_string(
            area.x + recording_indicator.len() as u16 + count_text.len() as u16,
            area.y,
            &filter_text,
            Style::default().fg(Color::Yellow),
        );
    }
}
```

#### Column headers

```rust
fn render_column_headers(&self, area: Rect, buf: &mut Buffer) {
    let style = Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD);
    let mut x = area.x;

    buf.set_string(x, area.y, "Stat", style); x += COL_STATUS;
    buf.set_string(x, area.y, "Method", style); x += COL_METHOD;
    buf.set_string(x, area.y, "Duration", style); x += COL_DURATION;
    buf.set_string(x, area.y, "Size", style); x += COL_SIZE;
    buf.set_string(x, area.y, "Type", style); x += COL_TYPE;
    buf.set_string(x, area.y, "URI", style);
}
```

#### Data rows

```rust
fn render_rows(&self, area: Rect, buf: &mut Buffer) {
    let visible_rows = area.height as usize;
    let start = self.scroll_offset;
    let end = (start + visible_rows).min(self.entries.len());

    for (row_idx, entry_idx) in (start..end).enumerate() {
        let entry = self.entries[entry_idx];
        let y = area.y + row_idx as u16;
        let is_selected = self.selected_index == Some(entry_idx);

        // Background for selected row
        let row_style = if is_selected {
            Style::default().bg(Color::DarkGray)
        } else {
            Style::default()
        };

        // Clear row with background
        for x in area.x..area.right() {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_style(row_style).set_char(' ');
            }
        }

        let mut x = area.x;

        // Status code
        let (status_text, status_style) = status_display(entry);
        buf.set_string(x, y, &status_text, status_style.patch(row_style));
        x += COL_STATUS;

        // Method
        let method_style = method_color(&entry.method).patch(row_style);
        buf.set_string(x, y, &truncate(&entry.method, COL_METHOD as usize - 1), method_style);
        x += COL_METHOD;

        // Duration
        let duration_text = entry.duration_ms()
            .map(|ms| format_duration_ms(ms))
            .unwrap_or_else(|| "...".to_string());
        let duration_style = if entry.is_pending() {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default().fg(Color::White)
        };
        buf.set_string(x, y, &truncate(&duration_text, COL_DURATION as usize - 1), duration_style.patch(row_style));
        x += COL_DURATION;

        // Size
        let size_text = entry.response_size_display().unwrap_or_default();
        buf.set_string(x, y, &truncate(&size_text, COL_SIZE as usize - 1), Style::default().fg(Color::Gray).patch(row_style));
        x += COL_SIZE;

        // Content type (short)
        let type_text = entry.content_type.as_deref()
            .map(short_content_type)
            .unwrap_or_default();
        buf.set_string(x, y, &truncate(&type_text, COL_TYPE as usize - 1), Style::default().fg(Color::DarkGray).patch(row_style));
        x += COL_TYPE;

        // URI (remaining space)
        let uri_width = area.right().saturating_sub(x) as usize;
        let uri_text = entry.short_uri();
        buf.set_string(x, y, &truncate(uri_text, uri_width), Style::default().fg(Color::White).patch(row_style));
    }
}
```

#### Style helpers

```rust
fn status_display(entry: &HttpProfileEntry) -> (String, Style) {
    match entry.status_code {
        Some(code) if code < 300 => (code.to_string(), Style::default().fg(Color::Green)),
        Some(code) if code < 400 => (code.to_string(), Style::default().fg(Color::Cyan)),
        Some(code) if code < 500 => (code.to_string(), Style::default().fg(Color::Yellow)),
        Some(code) => (code.to_string(), Style::default().fg(Color::Red)),
        None if entry.error.is_some() => ("ERR".to_string(), Style::default().fg(Color::Red)),
        None => ("...".to_string(), Style::default().fg(Color::DarkGray)),
    }
}

fn method_color(method: &str) -> Style {
    match method {
        "GET" => Style::default().fg(Color::Green),
        "POST" => Style::default().fg(Color::Blue),
        "PUT" => Style::default().fg(Color::Yellow),
        "PATCH" => Style::default().fg(Color::Yellow),
        "DELETE" => Style::default().fg(Color::Red),
        "HEAD" => Style::default().fg(Color::Cyan),
        "OPTIONS" => Style::default().fg(Color::Magenta),
        _ => Style::default().fg(Color::White),
    }
}

fn short_content_type(ct: &str) -> String {
    if ct.contains("json") { "json".to_string() }
    else if ct.contains("html") { "html".to_string() }
    else if ct.contains("xml") { "xml".to_string() }
    else if ct.contains("image") { "image".to_string() }
    else if ct.contains("text") { "text".to_string() }
    else if ct.contains("javascript") { "js".to_string() }
    else if ct.contains("css") { "css".to_string() }
    else { ct.split('/').last().unwrap_or(ct).to_string() }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max { s.to_string() }
    else { format!("{}…", &s[..max.saturating_sub(1)]) }
}
```

### Acceptance Criteria

1. `RequestTable` widget renders without panic for any terminal size (including 0-height, 0-width)
2. Header bar shows recording indicator (red dot when recording, gray when paused)
3. Header bar shows request count
4. Header bar shows active filter text in yellow
5. Column headers render with correct labels
6. Each row shows status code colored by range (2xx green, 3xx cyan, 4xx yellow, 5xx red)
7. Method column colored by HTTP method
8. Pending requests show `...` for status and duration
9. Error requests show `ERR` in red for status
10. URI column truncates to fit available width, showing path only (via `short_uri()`)
11. Selected row has `bg(DarkGray)` highlight
12. Scroll offset correctly windows into the entry list
13. Content type shows short form (json, html, image, etc.)
14. All new code has unit tests (15+ tests)

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use fdemon_core::network::HttpProfileEntry;

    fn make_entry(id: &str, method: &str, status: Option<u16>) -> HttpProfileEntry {
        HttpProfileEntry {
            id: id.to_string(), method: method.to_string(),
            uri: format!("https://example.com/api/{}", id),
            status_code: status, content_type: Some("application/json".to_string()),
            start_time_us: 1_000_000, end_time_us: status.map(|_| 1_050_000),
            request_content_length: None, response_content_length: Some(1024),
            error: None,
        }
    }

    fn render_to_buf(entries: &[&HttpProfileEntry], selected: Option<usize>, w: u16, h: u16) -> Buffer {
        let widget = RequestTable::new(entries, selected, 0, true, "");
        let mut buf = Buffer::empty(Rect::new(0, 0, w, h));
        widget.render(Rect::new(0, 0, w, h), &mut buf);
        buf
    }

    fn buf_text(buf: &Buffer, w: u16, h: u16) -> String {
        let mut s = String::new();
        for y in 0..h { for x in 0..w {
            if let Some(c) = buf.cell((x, y)) {
                s.push_str(c.symbol());
            }
        }}
        s
    }

    #[test]
    fn test_renders_without_panic() { render_to_buf(&[], None, 80, 24); }

    #[test]
    fn test_renders_zero_height() { render_to_buf(&[], None, 80, 0); }

    #[test]
    fn test_renders_zero_width() { render_to_buf(&[], None, 0, 24); }

    #[test]
    fn test_renders_tiny_terminal() { render_to_buf(&[], None, 10, 3); }

    #[test]
    fn test_shows_recording_indicator() {
        let buf = render_to_buf(&[], None, 80, 5);
        let text = buf_text(&buf, 80, 1);
        assert!(text.contains("REC"));
    }

    #[test]
    fn test_shows_paused_indicator() {
        let widget = RequestTable::new(&[], None, 0, false, "");
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 5));
        widget.render(Rect::new(0, 0, 80, 5), &mut buf);
        let text = buf_text(&buf, 80, 1);
        assert!(text.contains("PAUSED"));
    }

    #[test]
    fn test_shows_request_count() {
        let e1 = make_entry("1", "GET", Some(200));
        let e2 = make_entry("2", "POST", Some(201));
        let entries: Vec<&HttpProfileEntry> = vec![&e1, &e2];
        let buf = render_to_buf(&entries, None, 80, 10);
        let text = buf_text(&buf, 80, 1);
        assert!(text.contains("2 requests"));
    }

    #[test]
    fn test_column_headers_present() {
        let buf = render_to_buf(&[], None, 80, 5);
        let text = buf_text(&buf, 80, 5);
        assert!(text.contains("Method"));
        assert!(text.contains("URI"));
    }

    #[test]
    fn test_renders_entry_row() {
        let e1 = make_entry("1", "GET", Some(200));
        let entries: Vec<&HttpProfileEntry> = vec![&e1];
        let buf = render_to_buf(&entries, None, 100, 10);
        let text = buf_text(&buf, 100, 10);
        assert!(text.contains("200"));
        assert!(text.contains("GET"));
        assert!(text.contains("/api/1"));
    }

    #[test]
    fn test_pending_request_shows_dots() {
        let e1 = make_entry("1", "GET", None);
        let entries: Vec<&HttpProfileEntry> = vec![&e1];
        let buf = render_to_buf(&entries, None, 100, 10);
        let text = buf_text(&buf, 100, 10);
        assert!(text.contains("..."));
    }

    #[test]
    fn test_status_colors() {
        let (_, style_200) = status_display(&make_entry("1", "GET", Some(200)));
        assert_eq!(style_200.fg, Some(Color::Green));
        let (_, style_404) = status_display(&make_entry("1", "GET", Some(404)));
        assert_eq!(style_404.fg, Some(Color::Yellow));
        let (_, style_500) = status_display(&make_entry("1", "GET", Some(500)));
        assert_eq!(style_500.fg, Some(Color::Red));
    }

    #[test]
    fn test_method_colors() {
        assert_eq!(method_color("GET").fg, Some(Color::Green));
        assert_eq!(method_color("POST").fg, Some(Color::Blue));
        assert_eq!(method_color("DELETE").fg, Some(Color::Red));
    }

    #[test]
    fn test_short_content_type() {
        assert_eq!(short_content_type("application/json"), "json");
        assert_eq!(short_content_type("text/html; charset=utf-8"), "html");
        assert_eq!(short_content_type("image/png"), "image");
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 5), "hell…");
    }
}
```

### Notes

- **No direct state access**: The widget borrows pre-filtered entries as `&[&HttpProfileEntry]`. The parent `NetworkMonitor` widget is responsible for calling `session.network.filtered_entries()` and passing the result. This keeps the widget pure.
- **Scroll management**: The scroll offset is passed in from `NetworkState.scroll_offset`. The widget doesn't modify scroll state — it just renders the visible window. Scroll adjustment happens in the handler layer.
- **Column width strategy**: Fixed widths for Status (5), Method (7), Duration (8), Size (8), Type (10). URI gets all remaining space. This works well for terminals 60+ chars wide. For very narrow terminals (< 40), the URI column may be tiny or hidden — acceptable degradation.
- **Style composition**: Use `.patch(row_style)` to layer column-specific foreground colors over the row background (selected vs unselected). This ensures the DarkGray selection background applies consistently.
