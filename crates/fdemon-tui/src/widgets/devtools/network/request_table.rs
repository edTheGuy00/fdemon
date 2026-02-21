//! # Network Request Table Widget
//!
//! Renders a scrollable table of HTTP requests with status, method, URI,
//! content type, duration, and response size columns. Supports selection
//! highlighting, pending request indicators, and filter highlighting.

use fdemon_core::network::{format_duration_ms, HttpProfileEntry};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::Widget;

// ── Column widths (characters) ────────────────────────────────────────────────

/// Status code column width in characters.
const COL_STATUS: u16 = 5;

/// HTTP method column width in characters.
const COL_METHOD: u16 = 7;

/// Duration column width in characters.
const COL_DURATION: u16 = 8;

/// Response size column width in characters.
const COL_SIZE: u16 = 8;

/// Content-type (short) column width in characters.
const COL_TYPE: u16 = 10;

// URI column gets the remaining space.

// ── RequestTable ──────────────────────────────────────────────────────────────

/// Scrollable table widget that renders a list of HTTP profile entries.
///
/// The widget is pure: it owns no state. The parent is responsible for
/// calling `session.network.filtered_entries()` and passing the result.
/// Scroll management and selection adjustments belong to the handler layer.
pub struct RequestTable<'a> {
    /// Pre-filtered entries to display.
    entries: &'a [&'a HttpProfileEntry],
    /// Index into `entries` that is currently selected (if any).
    selected_index: Option<usize>,
    /// First visible entry index (scroll state).
    scroll_offset: usize,
    /// Whether recording is active (affects indicator display).
    recording: bool,
    /// Current filter text (empty = no filter).
    filter: &'a str,
}

impl<'a> RequestTable<'a> {
    /// Create a new `RequestTable` widget.
    pub fn new(
        entries: &'a [&'a HttpProfileEntry],
        selected_index: Option<usize>,
        scroll_offset: usize,
        recording: bool,
        filter: &'a str,
    ) -> Self {
        Self {
            entries,
            selected_index,
            scroll_offset,
            recording,
            filter,
        }
    }
}

impl Widget for RequestTable<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Need at least 2 rows: header bar + column headers.
        if area.height < 2 {
            return;
        }

        // Row 0: Header bar (recording indicator + count + filter hint)
        self.render_header(area, buf);

        // Row 1: Column headers
        let header_area = Rect {
            y: area.y + 1,
            height: 1,
            ..area
        };
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

impl RequestTable<'_> {
    // ── Header bar ────────────────────────────────────────────────────────────

    /// Render the status header: recording indicator, request count, active filter.
    fn render_header(&self, area: Rect, buf: &mut Buffer) {
        let header_area = Rect { height: 1, ..area };

        // Recording indicator
        let recording_indicator = if self.recording {
            "● REC"
        } else {
            "○ PAUSED"
        };
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

        buf.set_string(
            header_area.x,
            header_area.y,
            recording_indicator,
            recording_style,
        );

        let after_indicator = header_area.x + recording_indicator.len() as u16;
        buf.set_string(
            after_indicator,
            header_area.y,
            &count_text,
            Style::default().fg(Color::Gray),
        );

        if !filter_text.is_empty() {
            buf.set_string(
                after_indicator + count_text.len() as u16,
                header_area.y,
                &filter_text,
                Style::default().fg(Color::Yellow),
            );
        }
    }

    // ── Column headers ────────────────────────────────────────────────────────

    /// Render the fixed column header row.
    fn render_column_headers(&self, area: Rect, buf: &mut Buffer) {
        let style = Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::BOLD);
        let mut x = area.x;

        buf.set_string(x, area.y, "Stat", style);
        x += COL_STATUS;
        buf.set_string(x, area.y, "Method", style);
        x += COL_METHOD;
        buf.set_string(x, area.y, "Duration", style);
        x += COL_DURATION;
        buf.set_string(x, area.y, "Size", style);
        x += COL_SIZE;
        buf.set_string(x, area.y, "Type", style);
        x += COL_TYPE;
        buf.set_string(x, area.y, "URI", style);
    }

    // ── Data rows ─────────────────────────────────────────────────────────────

    /// Render the visible window of request rows.
    fn render_rows(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }

        let visible_rows = area.height as usize;
        let start = self.scroll_offset;
        let end = (start + visible_rows).min(self.entries.len());

        for (row_idx, entry_idx) in (start..end).enumerate() {
            let entry = self.entries[entry_idx];
            let y = area.y + row_idx as u16;
            let is_selected = self.selected_index == Some(entry_idx);

            // Row background: DarkGray for selected, default otherwise.
            let row_style = if is_selected {
                Style::default().bg(Color::DarkGray)
            } else {
                Style::default()
            };

            // Clear entire row with the row background.
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
            let method_style = Style::default()
                .fg(super::http_method_color(&entry.method))
                .patch(row_style);
            buf.set_string(
                x,
                y,
                truncate(&entry.method, COL_METHOD as usize - 1),
                method_style,
            );
            x += COL_METHOD;

            // Duration
            let duration_text = entry
                .duration_ms()
                .map(format_duration_ms)
                .unwrap_or_else(|| "...".to_string());
            let duration_style = if entry.is_pending() {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default().fg(Color::White)
            };
            buf.set_string(
                x,
                y,
                truncate(&duration_text, COL_DURATION as usize - 1),
                duration_style.patch(row_style),
            );
            x += COL_DURATION;

            // Response size
            let size_text = entry.response_size_display().unwrap_or_default();
            buf.set_string(
                x,
                y,
                truncate(&size_text, COL_SIZE as usize - 1),
                Style::default().fg(Color::Gray).patch(row_style),
            );
            x += COL_SIZE;

            // Content type (short form)
            let type_text = entry
                .content_type
                .as_deref()
                .map(short_content_type)
                .unwrap_or_default();
            buf.set_string(
                x,
                y,
                truncate(&type_text, COL_TYPE as usize - 1),
                Style::default().fg(Color::DarkGray).patch(row_style),
            );
            x += COL_TYPE;

            // URI — gets remaining width
            let uri_width = area.right().saturating_sub(x) as usize;
            let uri_text = entry.short_uri();
            buf.set_string(
                x,
                y,
                truncate(uri_text, uri_width),
                Style::default().fg(Color::White).patch(row_style),
            );
        }
    }
}

// ── Style helpers ─────────────────────────────────────────────────────────────

/// Return (display text, style) for a request's status code.
///
/// - `2xx` → green
/// - `3xx` → cyan
/// - `4xx` → yellow
/// - `5xx` → red
/// - Error (no status) → `"ERR"` in red
/// - Pending (no status, no error) → `"..."` in dark gray
pub(super) fn status_display(entry: &HttpProfileEntry) -> (String, Style) {
    match entry.status_code {
        Some(code) if code < 300 => (code.to_string(), Style::default().fg(Color::Green)),
        Some(code) if code < 400 => (code.to_string(), Style::default().fg(Color::Cyan)),
        Some(code) if code < 500 => (code.to_string(), Style::default().fg(Color::Yellow)),
        Some(code) => (code.to_string(), Style::default().fg(Color::Red)),
        None if entry.error.is_some() => ("ERR".to_string(), Style::default().fg(Color::Red)),
        None => ("...".to_string(), Style::default().fg(Color::DarkGray)),
    }
}

/// Shorten a Content-Type header value to a one-word label.
///
/// Examples:
/// - `application/json` → `"json"`
/// - `text/html; charset=utf-8` → `"html"`
/// - `image/png` → `"image"`
pub(super) fn short_content_type(ct: &str) -> String {
    // More-specific checks must come before broader ones.
    // "javascript" and "css" are checked before "text" so that
    // "text/javascript" maps to "js" rather than "text".
    if ct.contains("json") {
        "json".to_string()
    } else if ct.contains("javascript") {
        "js".to_string()
    } else if ct.contains("css") {
        "css".to_string()
    } else if ct.contains("html") {
        "html".to_string()
    } else if ct.contains("xml") {
        "xml".to_string()
    } else if ct.contains("image") {
        "image".to_string()
    } else if ct.contains("text") {
        "text".to_string()
    } else {
        ct.split('/').next_back().unwrap_or(ct).to_string()
    }
}

/// Truncate `s` to at most `max` Unicode characters, appending `…` when truncated.
///
/// Delegates to [`super::super::truncate_str`] which uses `char_indices()` for
/// Unicode-safe slicing. This prevents panics on multi-byte characters such as
/// accented letters or CJK characters that may appear in URLs.
pub(super) fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        // Truncate to max-1 chars, leaving room for the "…" ellipsis.
        let truncated = super::super::truncate_str(s, max.saturating_sub(1));
        format!("{truncated}…")
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use fdemon_core::network::HttpProfileEntry;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use ratatui::style::Color;

    // ── Test helpers ──────────────────────────────────────────────────────────

    fn make_entry(id: &str, method: &str, status: Option<u16>) -> HttpProfileEntry {
        HttpProfileEntry {
            id: id.to_string(),
            method: method.to_string(),
            uri: format!("https://example.com/api/{}", id),
            status_code: status,
            content_type: Some("application/json".to_string()),
            start_time_us: 1_000_000,
            end_time_us: status.map(|_| 1_050_000),
            request_content_length: None,
            response_content_length: Some(1024),
            error: None,
        }
    }

    fn render_to_buf(
        entries: &[&HttpProfileEntry],
        selected: Option<usize>,
        w: u16,
        h: u16,
    ) -> Buffer {
        let widget = RequestTable::new(entries, selected, 0, true, "");
        let mut buf = Buffer::empty(Rect::new(0, 0, w, h));
        widget.render(Rect::new(0, 0, w, h), &mut buf);
        buf
    }

    fn buf_text(buf: &Buffer, w: u16, h: u16) -> String {
        let mut s = String::new();
        for y in 0..h {
            for x in 0..w {
                if let Some(c) = buf.cell((x, y)) {
                    s.push_str(c.symbol());
                }
            }
        }
        s
    }

    // ── Rendering / no-panic tests ─────────────────────────────────────────────

    #[test]
    fn test_renders_without_panic() {
        render_to_buf(&[], None, 80, 24);
    }

    #[test]
    fn test_renders_zero_height() {
        render_to_buf(&[], None, 80, 0);
    }

    #[test]
    fn test_renders_zero_width() {
        render_to_buf(&[], None, 0, 24);
    }

    #[test]
    fn test_renders_tiny_terminal() {
        render_to_buf(&[], None, 10, 3);
    }

    #[test]
    fn test_renders_single_height() {
        // height == 1 is below the 2-row minimum — must not panic.
        render_to_buf(&[], None, 80, 1);
    }

    // ── Header bar tests ──────────────────────────────────────────────────────

    #[test]
    fn test_shows_recording_indicator() {
        let buf = render_to_buf(&[], None, 80, 5);
        let text = buf_text(&buf, 80, 1);
        assert!(
            text.contains("REC"),
            "Expected 'REC' in header; got: {text:?}"
        );
    }

    #[test]
    fn test_shows_paused_indicator() {
        let widget = RequestTable::new(&[], None, 0, false, "");
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 5));
        widget.render(Rect::new(0, 0, 80, 5), &mut buf);
        let text = buf_text(&buf, 80, 1);
        assert!(
            text.contains("PAUSED"),
            "Expected 'PAUSED' in header; got: {text:?}"
        );
    }

    #[test]
    fn test_shows_request_count() {
        let e1 = make_entry("1", "GET", Some(200));
        let e2 = make_entry("2", "POST", Some(201));
        let entries: Vec<&HttpProfileEntry> = vec![&e1, &e2];
        let buf = render_to_buf(&entries, None, 80, 10);
        let text = buf_text(&buf, 80, 1);
        assert!(
            text.contains("2 requests"),
            "Expected '2 requests' in header; got: {text:?}"
        );
    }

    #[test]
    fn test_shows_filter_text() {
        let widget = RequestTable::new(&[], None, 0, true, "api");
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 5));
        widget.render(Rect::new(0, 0, 80, 5), &mut buf);
        let text = buf_text(&buf, 80, 1);
        assert!(
            text.contains("filter: api"),
            "Expected 'filter: api' in header; got: {text:?}"
        );
    }

    #[test]
    fn test_no_filter_text_when_empty() {
        let widget = RequestTable::new(&[], None, 0, true, "");
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 5));
        widget.render(Rect::new(0, 0, 80, 5), &mut buf);
        let text = buf_text(&buf, 80, 1);
        assert!(
            !text.contains("filter:"),
            "Should not show 'filter:' when filter is empty; got: {text:?}"
        );
    }

    // ── Column header tests ────────────────────────────────────────────────────

    #[test]
    fn test_column_headers_present() {
        let buf = render_to_buf(&[], None, 80, 5);
        let text = buf_text(&buf, 80, 5);
        assert!(
            text.contains("Method"),
            "Expected 'Method' column header; got: {text:?}"
        );
        assert!(
            text.contains("URI"),
            "Expected 'URI' column header; got: {text:?}"
        );
        assert!(
            text.contains("Duration"),
            "Expected 'Duration' column header; got: {text:?}"
        );
    }

    // ── Data row tests ────────────────────────────────────────────────────────

    #[test]
    fn test_renders_entry_row() {
        let e1 = make_entry("1", "GET", Some(200));
        let entries: Vec<&HttpProfileEntry> = vec![&e1];
        let buf = render_to_buf(&entries, None, 100, 10);
        let text = buf_text(&buf, 100, 10);
        assert!(text.contains("200"), "Expected '200' in row; got: {text:?}");
        assert!(text.contains("GET"), "Expected 'GET' in row; got: {text:?}");
        assert!(
            text.contains("/api/1"),
            "Expected '/api/1' in row; got: {text:?}"
        );
    }

    #[test]
    fn test_pending_request_shows_dots() {
        let e1 = make_entry("1", "GET", None);
        let entries: Vec<&HttpProfileEntry> = vec![&e1];
        let buf = render_to_buf(&entries, None, 100, 10);
        let text = buf_text(&buf, 100, 10);
        assert!(
            text.contains("..."),
            "Pending request should show '...' for status/duration; got: {text:?}"
        );
    }

    #[test]
    fn test_error_request_shows_err_in_status() {
        let mut e1 = make_entry("1", "GET", None);
        e1.error = Some("Connection refused".to_string());
        let entries: Vec<&HttpProfileEntry> = vec![&e1];
        let buf = render_to_buf(&entries, None, 100, 10);
        let text = buf_text(&buf, 100, 10);
        assert!(
            text.contains("ERR"),
            "Error request should show 'ERR' in status; got: {text:?}"
        );
    }

    #[test]
    fn test_scroll_offset_windows_into_entries() {
        let e0 = make_entry("0", "GET", Some(200));
        let e1 = make_entry("1", "POST", Some(201));
        let e2 = make_entry("2", "PUT", Some(204));
        let entries: Vec<&HttpProfileEntry> = vec![&e0, &e1, &e2];

        // Use scroll_offset=1: entries 1 and 2 should be visible, entry 0 should not.
        let widget = RequestTable::new(&entries, None, 1, true, "");
        let mut buf = Buffer::empty(Rect::new(0, 0, 100, 10));
        widget.render(Rect::new(0, 0, 100, 10), &mut buf);
        let text = buf_text(&buf, 100, 10);

        // The URI for entry 1 is /api/1
        assert!(
            text.contains("/api/1"),
            "Scrolled view should show entry 1; got: {text:?}"
        );
        // Entry 0's URI should NOT appear in the rows section.
        // The header says "3 requests" but row 0 (/api/0) is scrolled past.
        assert!(
            !text.contains("/api/0"),
            "Scrolled view should NOT show entry 0; got: {text:?}"
        );
    }

    // ── Status color unit tests ────────────────────────────────────────────────

    #[test]
    fn test_status_colors() {
        let (_, style_200) = status_display(&make_entry("1", "GET", Some(200)));
        assert_eq!(style_200.fg, Some(Color::Green), "2xx should be green");

        let (_, style_301) = status_display(&make_entry("1", "GET", Some(301)));
        assert_eq!(style_301.fg, Some(Color::Cyan), "3xx should be cyan");

        let (_, style_404) = status_display(&make_entry("1", "GET", Some(404)));
        assert_eq!(style_404.fg, Some(Color::Yellow), "4xx should be yellow");

        let (_, style_500) = status_display(&make_entry("1", "GET", Some(500)));
        assert_eq!(style_500.fg, Some(Color::Red), "5xx should be red");
    }

    #[test]
    fn test_status_display_pending() {
        let (text, style) = status_display(&make_entry("1", "GET", None));
        assert_eq!(text, "...", "Pending should show '...'");
        assert_eq!(style.fg, Some(Color::DarkGray));
    }

    #[test]
    fn test_status_display_error() {
        let mut entry = make_entry("1", "GET", None);
        entry.error = Some("timeout".to_string());
        let (text, style) = status_display(&entry);
        assert_eq!(text, "ERR", "Error should show 'ERR'");
        assert_eq!(style.fg, Some(Color::Red));
    }

    // ── Method color unit tests ────────────────────────────────────────────────

    #[test]
    fn test_method_colors() {
        assert_eq!(
            super::super::http_method_color("GET"),
            Color::Green,
            "GET should be green"
        );
        assert_eq!(
            super::super::http_method_color("POST"),
            Color::Blue,
            "POST should be blue"
        );
        assert_eq!(
            super::super::http_method_color("DELETE"),
            Color::Red,
            "DELETE should be red"
        );
        assert_eq!(
            super::super::http_method_color("PUT"),
            Color::Yellow,
            "PUT should be yellow"
        );
        assert_eq!(
            super::super::http_method_color("PATCH"),
            Color::Yellow,
            "PATCH should be yellow"
        );
        assert_eq!(
            super::super::http_method_color("HEAD"),
            Color::Cyan,
            "HEAD should be cyan"
        );
        assert_eq!(
            super::super::http_method_color("OPTIONS"),
            Color::Magenta,
            "OPTIONS should be magenta"
        );
    }

    // ── Content type unit tests ────────────────────────────────────────────────

    #[test]
    fn test_short_content_type() {
        assert_eq!(short_content_type("application/json"), "json");
        // html is checked before text
        assert_eq!(short_content_type("text/html; charset=utf-8"), "html");
        assert_eq!(short_content_type("image/png"), "image");
        // plain text (no html/xml/image match) → "text"
        assert_eq!(short_content_type("text/plain"), "text");
        // application/xml → "xml"
        assert_eq!(short_content_type("application/xml"), "xml");
    }

    #[test]
    fn test_short_content_type_javascript_without_text() {
        // application/javascript has no "text" prefix → matches "javascript" → "js"
        assert_eq!(short_content_type("application/javascript"), "js");
    }

    #[test]
    fn test_short_content_type_unknown_uses_subtype() {
        // Unknown types fall back to everything after '/'
        assert_eq!(
            short_content_type("application/octet-stream"),
            "octet-stream"
        );
    }

    #[test]
    fn test_short_content_type_css_without_text() {
        // application/css → matches "css" → "css"
        assert_eq!(short_content_type("application/css"), "css");
    }

    #[test]
    fn test_short_content_type_text_javascript_maps_to_js() {
        // "text/javascript" contains both "text" and "javascript". The more
        // specific "javascript" check must come first so the result is "js"
        // and not "text". This is the regression test for Issue 15.
        assert_eq!(
            short_content_type("text/javascript"),
            "js",
            "text/javascript should map to 'js', not 'text'"
        );
    }

    #[test]
    fn test_short_content_type_text_css_maps_to_css() {
        // "text/css" contains both "text" and "css". The more specific "css"
        // check must come first.
        assert_eq!(
            short_content_type("text/css"),
            "css",
            "text/css should map to 'css', not 'text'"
        );
    }

    // ── Truncate unit tests ───────────────────────────────────────────────────

    #[test]
    fn test_truncate_short_string_unchanged() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_exact_length_unchanged() {
        assert_eq!(truncate("hello", 5), "hello");
    }

    #[test]
    fn test_truncate_adds_ellipsis() {
        // "hello world" is 11 chars; max=5 → keep 4 chars + "…"
        assert_eq!(truncate("hello world", 5), "hell…");
    }

    #[test]
    fn test_truncate_max_zero_returns_ellipsis() {
        // max=0: saturating_sub(1) gives 0, truncate_str returns "",
        // which is shorter than "hello", so we append "…".
        let result = truncate("hello", 0);
        assert_eq!(result, "…", "max=0 on non-empty string should return \"…\"");
    }

    #[test]
    fn test_truncate_max_one_returns_single_ellipsis() {
        // max=1: truncate_str(s, 0) = "" → "" < "hello" → "…"
        let result = truncate("hello", 1);
        assert_eq!(result, "…");
    }

    #[test]
    fn test_truncate_empty_string_unchanged() {
        // Empty string is never longer than any max.
        assert_eq!(truncate("", 5), "");
        assert_eq!(truncate("", 0), "");
    }

    #[test]
    fn test_truncate_multibyte_ascii_accent_short_unchanged() {
        // "héllo" fits in max=10 — must not panic and must be returned as-is.
        assert_eq!(truncate("héllo", 10), "héllo");
    }

    #[test]
    fn test_truncate_multibyte_accent_truncated_safely() {
        // "héllo" = 5 Unicode chars (é is 2 bytes).
        // max=4 → keep 3 chars ("hél") + "…"
        let result = truncate("héllo", 4);
        assert_eq!(result, "hél…");
    }

    #[test]
    fn test_truncate_multibyte_boundary_mid_character() {
        // "héllo" — byte length is 6 (é = 2 bytes), char length is 5.
        // A byte-level slice at position 2 would land mid-character and panic.
        // The Unicode-safe implementation must not panic.
        let result = truncate("héllo", 3);
        // max=3 → keep 2 chars + "…"
        assert_eq!(result, "hé…");
    }

    #[test]
    fn test_truncate_cjk_characters_safely() {
        // "日本語テスト" = 6 CJK chars, each 3 bytes.
        // max=4 → keep 3 chars ("日本語") + "…"
        let result = truncate("日本語テスト", 4);
        assert_eq!(result, "日本語…");
    }

    #[test]
    fn test_truncate_cjk_short_unchanged() {
        // String fits within max — must be returned unchanged.
        assert_eq!(truncate("日本語", 10), "日本語");
    }

    #[test]
    fn test_truncate_url_with_unicode_path() {
        // Simulates a URL like "https://api.example.com/réservations"
        let url = "https://api.example.com/réservations";
        // Truncating to a width that would fall mid-byte on byte slicing must not panic.
        let result = truncate(url, 28);
        // 28 chars → keep 27 chars + "…"
        let expected_prefix: String = url.chars().take(27).collect();
        assert!(
            result.starts_with(&expected_prefix[..expected_prefix.len().min(20)]),
            "truncated URL should start with expected prefix; got: {result:?}"
        );
        assert!(
            result.ends_with('…'),
            "truncated URL should end with ellipsis"
        );
    }

    #[test]
    fn test_truncate_ellipsis_appended_only_when_truncated() {
        // Exactly at boundary: no ellipsis.
        let s = "hello";
        assert_eq!(truncate(s, 5), "hello");
        // One over: ellipsis appears.
        assert!(truncate(s, 4).ends_with('…'));
    }
}
