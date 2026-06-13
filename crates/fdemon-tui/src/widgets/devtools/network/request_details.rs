//! # Network Request Details Widget
//!
//! Renders detailed information about a selected HTTP request, with sub-tab
//! switching between General, Headers, Request Body, Response Body, and Timing.
//!
//! ## Layout
//!
//! All content is rendered in a **vertical stack** that fits the available
//! `area` — no horizontal overflow. Long URIs and header values wrap to the
//! next line. Body text is rendered with a scrollable viewport so short
//! terminals can still access the full content via keyboard (`Alt+j`/`Alt+k`)
//! or mouse (`Ctrl+wheel`).
//!
//! ## JSON pretty-print
//!
//! Request/response bodies that parse as valid JSON are pretty-printed and
//! lightly colorized (keys in yellow, string values in green, numbers in cyan).
//! Invalid JSON or binary data falls back to raw text or a size placeholder.

use fdemon_app::message::Message;
use fdemon_app::session::NetworkDetailTab;
use fdemon_app::{MouseAction, MouseRect};
use fdemon_core::network::{
    format_body_text, format_bytes, format_duration_ms, HttpProfileEntry, HttpProfileEntryDetail,
};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    prelude::StatefulWidget,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Widget},
};

use crate::widgets::MouseCtx;

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
    /// Current scroll offset for the body viewport (lines from top).
    scroll_offset: usize,
}

impl<'a> RequestDetails<'a> {
    /// Create a new `RequestDetails` widget.
    pub fn new(
        entry: &'a HttpProfileEntry,
        detail: Option<&'a HttpProfileEntryDetail>,
        active_tab: NetworkDetailTab,
        loading: bool,
        scroll_offset: usize,
    ) -> Self {
        Self {
            entry,
            detail,
            active_tab,
            loading,
            scroll_offset,
        }
    }
}

impl Widget for RequestDetails<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.render_impl(area, buf, None);
    }
}

impl RequestDetails<'_> {
    /// Render the details panel, optionally recording click regions.
    ///
    /// When `ctx` is `Some`, registers 5 click regions for the sub-tab bar
    /// (one per [`NetworkDetailTab`] variant). The body content area has no
    /// click regions in v1. Passing `None` is equivalent to `Widget::render`.
    pub fn render_with_regions(self, area: Rect, buf: &mut Buffer, ctx: Option<&mut MouseCtx<'_>>) {
        self.render_impl(area, buf, ctx);
    }

    fn render_impl(self, area: Rect, buf: &mut Buffer, ctx: Option<&mut MouseCtx<'_>>) {
        if area.height < 3 {
            return;
        }

        // Row 0: Sub-tab bar
        self.render_tab_bar(Rect { height: 1, ..area }, buf, ctx);

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

    /// Render the sub-tab bar, optionally recording click regions.
    ///
    /// When `ctx` is `Some`, registers one `MouseAction::Emit(NetworkSwitchDetailTab(tab))`
    /// region per tab label. Each region is 1 row tall and `padded.len()` columns wide.
    fn render_tab_bar(&self, area: Rect, buf: &mut Buffer, mut ctx: Option<&mut MouseCtx<'_>>) {
        let tabs = [
            (NetworkDetailTab::General, "[g] General"),
            (NetworkDetailTab::Headers, "[h] Headers"),
            (NetworkDetailTab::RequestBody, "[q] Request"),
            (NetworkDetailTab::ResponseBody, "[s] Response"),
            (NetworkDetailTab::Timing, "[t] Timing"),
        ];

        let mut x = area.x;
        for (tab, label) in &tabs {
            if x >= area.right() {
                break;
            }

            let style = if *tab == self.active_tab {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };
            let padded = format!(" {} ", label);
            let needed_width = padded.len() as u16;
            buf.set_string(x, area.y, &padded, style);

            // Register click region for this tab label.
            if let Some(c) = ctx.as_deref_mut() {
                let render_w = needed_width.min(area.right().saturating_sub(x));
                if render_w > 0 {
                    let rect = MouseRect::new(x, area.y, render_w, 1);
                    c.click(
                        rect,
                        MouseAction::emit(Message::NetworkSwitchDetailTab(*tab)),
                    );
                }
            }

            x += needed_width;
        }
    }

    // ── General tab ───────────────────────────────────────────────────────────

    /// Render the General tab as a vertical stack of label+value rows.
    ///
    /// Long values (URI, error) are truncated to fit the available width.
    /// The layout avoids horizontal overflow by using only the available `area`.
    fn render_general(&self, area: Rect, buf: &mut Buffer) {
        let label_style = Style::default().fg(Color::DarkGray);
        let value_style = Style::default().fg(Color::White);
        let mut y = area.y;

        // ── Method (label row) ────────────────────────────────────────────────
        if y >= area.bottom() {
            return;
        }
        buf.set_string(area.x + 1, y, "Method:", label_style);
        y += 1;

        if y >= area.bottom() {
            return;
        }
        buf.set_string(
            area.x + 3,
            y,
            &self.entry.method,
            Style::default().fg(super::http_method_color(&self.entry.method)),
        );
        y += 1;

        // ── URI ───────────────────────────────────────────────────────────────
        if y >= area.bottom() {
            return;
        }
        buf.set_string(area.x + 1, y, "URI:", label_style);
        y += 1;

        if y >= area.bottom() {
            return;
        }
        // Wrap URI across multiple rows so it doesn't overflow to the right.
        let uri_width = area.width.saturating_sub(3) as usize;
        if uri_width > 0 {
            for chunk in wrap_text(&self.entry.uri, uri_width) {
                if y >= area.bottom() {
                    break;
                }
                buf.set_string(area.x + 3, y, chunk, value_style);
                y += 1;
            }
        }

        // ── Status ────────────────────────────────────────────────────────────
        if y >= area.bottom() {
            return;
        }
        buf.set_string(area.x + 1, y, "Status:", label_style);
        y += 1;

        if y >= area.bottom() {
            return;
        }
        let (status_text, status_style) = match self.entry.status_code {
            Some(code) => (code.to_string(), status_color(code)),
            None if self.entry.error.is_some() => {
                ("Error".to_string(), Style::default().fg(Color::Red))
            }
            None => ("Pending".to_string(), Style::default().fg(Color::DarkGray)),
        };
        buf.set_string(area.x + 3, y, &status_text, status_style);
        y += 1;

        // ── Content-Type ──────────────────────────────────────────────────────
        if let Some(ct) = &self.entry.content_type {
            if y >= area.bottom() {
                return;
            }
            buf.set_string(area.x + 1, y, "Content-Type:", label_style);
            y += 1;
            if y >= area.bottom() {
                return;
            }
            let ct_width = area.width.saturating_sub(3) as usize;
            let display = if ct.len() > ct_width && ct_width > 0 {
                &ct[..ct_width]
            } else {
                ct.as_str()
            };
            buf.set_string(area.x + 3, y, display, value_style);
            y += 1;
        }

        // ── Duration ──────────────────────────────────────────────────────────
        if y >= area.bottom() {
            return;
        }
        buf.set_string(area.x + 1, y, "Duration:", label_style);
        y += 1;
        if y >= area.bottom() {
            return;
        }
        let dur_text = self
            .entry
            .duration_ms()
            .map(format_duration_ms)
            .unwrap_or_else(|| "Pending...".to_string());
        buf.set_string(area.x + 3, y, &dur_text, value_style);
        y += 1;

        // ── Request size ──────────────────────────────────────────────────────
        if let Some(len) = self.entry.request_content_length.filter(|&l| l >= 0) {
            if y >= area.bottom() {
                return;
            }
            buf.set_string(area.x + 1, y, "Request Size:", label_style);
            y += 1;
            if y >= area.bottom() {
                return;
            }
            buf.set_string(area.x + 3, y, format_bytes(len as u64), value_style);
            y += 1;
        }

        // ── Response size ─────────────────────────────────────────────────────
        if let Some(len) = self.entry.response_content_length.filter(|&l| l >= 0) {
            if y >= area.bottom() {
                return;
            }
            buf.set_string(area.x + 1, y, "Response Size:", label_style);
            y += 1;
            if y >= area.bottom() {
                return;
            }
            buf.set_string(area.x + 3, y, format_bytes(len as u64), value_style);
            y += 1;
        }

        // ── Error ─────────────────────────────────────────────────────────────
        if let Some(err) = &self.entry.error {
            if y >= area.bottom() {
                return;
            }
            buf.set_string(area.x + 1, y, "Error:", Style::default().fg(Color::Red));
            y += 1;
            let err_width = area.width.saturating_sub(3) as usize;
            for chunk in wrap_text(err, err_width) {
                if y >= area.bottom() {
                    break;
                }
                buf.set_string(
                    area.x + 3,
                    y,
                    chunk,
                    Style::default().fg(Color::Red),
                );
                y += 1;
            }
        }

        // ── Connection info ───────────────────────────────────────────────────
        if let Some(detail) = self.detail {
            if let Some(conn) = &detail.connection_info {
                if y >= area.bottom() {
                    return;
                }
                buf.set_string(area.x + 1, y, "Remote:", label_style);
                y += 1;
                if y < area.bottom() {
                    let addr = format!(
                        "{}:{}",
                        conn.remote_address.as_deref().unwrap_or("?"),
                        conn.remote_port.unwrap_or(0),
                    );
                    buf.set_string(area.x + 3, y, &addr, value_style);
                }
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
        let content_width = area.width.saturating_sub(2) as usize;

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
            // Render key on its own row to avoid overflow on narrow terminals.
            let key_line = format!("  {}:", key);
            buf.set_string(area.x + 1, y, &key_line, key_style);
            y += 1;
            // Wrap value.
            for chunk in wrap_text(&val_str, content_width) {
                if y >= area.bottom() {
                    break;
                }
                buf.set_string(area.x + 3, y, chunk, value_style);
                y += 1;
            }
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

        y += 1; // blank separator

        // Response Headers
        if y < area.bottom() {
            buf.set_string(area.x + 1, y, "Response Headers", header_style);
            y += 1;
            for (key, values) in &detail.response_headers {
                if y >= area.bottom() {
                    break;
                }
                let val_str = values.join(", ");
                let key_line = format!("  {}:", key);
                buf.set_string(area.x + 1, y, &key_line, key_style);
                y += 1;
                for chunk in wrap_text(&val_str, content_width) {
                    if y >= area.bottom() {
                        break;
                    }
                    buf.set_string(area.x + 3, y, chunk, value_style);
                    y += 1;
                }
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

        // Try to decode as UTF-8.
        let text_opt = if is_request {
            detail.request_body_text()
        } else {
            detail.response_body_text()
        };

        match text_opt {
            None => {
                // Binary data — show size info, no scroll needed.
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
            Some(raw_text) => {
                // Pretty-print if JSON, fall back to raw.
                let formatted = format_body_text(raw_text);
                // Guard against oversized placeholder strings.
                if formatted.is_empty() {
                    return;
                }

                // Reserve 1 column on the right for the scrollbar.
                let content_width = area.width.saturating_sub(2) as usize;
                if content_width == 0 {
                    return;
                }

                // Pre-wrap using textwrap so we can compute exact line count
                // without relying on ratatui's unstable line_count API.
                let wrapped_lines = wrap_text_owned(&formatted, content_width);
                let total_lines = wrapped_lines.len();
                let viewport_height = area.height as usize;

                // Clamp scroll offset to the valid range.
                let max_offset = total_lines.saturating_sub(viewport_height);
                let scroll = self.scroll_offset.min(max_offset);

                // Render visible slice — colorize if the formatted text is
                // pretty-printed JSON (detected by leading `{` or `[`).
                let is_json = formatted.trim_start().starts_with('{')
                    || formatted.trim_start().starts_with('[');

                for (i, line) in wrapped_lines
                    .iter()
                    .skip(scroll)
                    .take(viewport_height)
                    .enumerate()
                {
                    let y = area.y + i as u16;
                    if y >= area.bottom() {
                        break;
                    }
                    if is_json {
                        render_json_line(area.x + 1, y, line, content_width, buf);
                    } else {
                        buf.set_string(
                            area.x + 1,
                            y,
                            line.as_str(),
                            Style::default().fg(Color::White),
                        );
                    }
                }

                // Render scrollbar when content overflows.
                if total_lines > viewport_height {
                    let mut scrollbar_state = ScrollbarState::new(max_offset).position(scroll);
                    Scrollbar::new(ScrollbarOrientation::VerticalRight)
                        .render(area, buf, &mut scrollbar_state);
                }
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

            // Label (right-aligned in 10 chars)
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
            let event_width = area.width.saturating_sub(3) as usize;
            for event in &detail.events {
                if y >= area.bottom() {
                    break;
                }
                let offset_ms = (event.timestamp_us - self.entry.start_time_us) as f64 / 1000.0;
                let line = format!("  +{} {}", format_duration_ms(offset_ms), event.event);
                // Truncate long event lines to available width.
                let display = if event_width > 0 && line.len() > event_width {
                    &line[..event_width]
                } else {
                    &line
                };
                buf.set_string(
                    area.x + 1,
                    y,
                    display,
                    Style::default().fg(Color::Gray),
                );
                y += 1;
            }
        }
    }
}

// ── Text wrapping helpers ─────────────────────────────────────────────────────

/// Wrap `text` to `max_width` columns, returning a `Vec` of owned line strings.
///
/// Uses the `textwrap` crate so that line count is exact and matches what
/// we render. Empty `max_width` returns a single empty line vector.
fn wrap_text_owned(text: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 || text.is_empty() {
        return Vec::new();
    }
    textwrap::wrap(text, max_width)
        .into_iter()
        .map(|cow| cow.into_owned())
        .collect()
}

/// Wrap `text` to `max_width` columns using char-boundary splitting.
///
/// Used for short label/value strings in the General and Headers tabs.
/// For the body tab (which needs exact line counts), use [`wrap_text_owned`].
fn wrap_text(text: &str, max_width: usize) -> Vec<&str> {
    split_at_width(text, max_width)
}

/// Split `text` into `max_width`-char-boundary chunks (simple ASCII split).
///
/// Used for label/value pairs in the General and Headers tabs where content
/// is typically short ASCII text.
fn split_at_width(text: &str, max_width: usize) -> Vec<&str> {
    if max_width == 0 || text.is_empty() {
        return Vec::new();
    }
    let mut result = Vec::new();
    let mut remaining = text;
    while !remaining.is_empty() {
        // Find a char boundary at max_width.
        let split_at = if remaining.len() <= max_width {
            remaining.len()
        } else {
            // Walk backward to a char boundary.
            let mut idx = max_width;
            while idx > 0 && !remaining.is_char_boundary(idx) {
                idx -= 1;
            }
            idx.max(1) // ensure progress
        };
        result.push(&remaining[..split_at]);
        remaining = &remaining[split_at..];
    }
    result
}

// ── JSON colorizer ────────────────────────────────────────────────────────────

/// Render a single line of pretty-printed JSON with lightweight coloring.
///
/// - Object/array keys (before `:`) → Yellow
/// - String values (after `: "...") → Green
/// - Number / bool / null values → Cyan
/// - Structural characters (`{`, `}`, `[`, `]`, `,`) → DarkGray
/// - All other text → White
///
/// This is a best-effort colorizer operating on already-pretty-printed text,
/// not a full JSON parser. It handles the common patterns produced by
/// `serde_json::to_string_pretty`.
fn render_json_line(x: u16, y: u16, line: &str, max_width: usize, buf: &mut Buffer) {
    // Truncate the line to max_width to avoid overflow.
    let line = if line.len() > max_width && max_width > 0 {
        let mut idx = max_width;
        while idx > 0 && !line.is_char_boundary(idx) {
            idx -= 1;
        }
        &line[..idx]
    } else {
        line
    };

    // Build styled spans from the line content.
    let spans = colorize_json_line(line);
    let ratatui_line = Line::from(spans);
    let paragraph = Paragraph::new(vec![ratatui_line]);
    paragraph.render(Rect::new(x, y, line.len() as u16, 1), buf);
}

/// Convert a pretty-printed JSON line into a list of styled `Span`s.
fn colorize_json_line(line: &str) -> Vec<Span<'static>> {
    let structural_style = Style::default().fg(Color::DarkGray);
    let key_style = Style::default().fg(Color::Yellow);
    let default_style = Style::default().fg(Color::White);

    // Handle leading whitespace.
    let trimmed = line.trim_start();
    let indent = &line[..line.len() - trimmed.len()];

    let mut spans = Vec::new();
    if !indent.is_empty() {
        spans.push(Span::styled(indent.to_string(), default_style));
    }

    // Detect structural-only lines (closing braces/brackets with optional comma).
    let maybe_structural = trimmed.trim_end_matches(',').trim();
    if matches!(maybe_structural, "{" | "}" | "[" | "]") {
        spans.push(Span::styled(trimmed.to_string(), structural_style));
        return spans;
    }

    // Key: value pattern.  Keys in serde_json pretty output look like:
    //   "key": value
    //   "key": {
    //   "key": [
    if trimmed.starts_with('"') {
        // Find the closing quote for the key.
        if let Some(colon_pos) = find_key_colon(trimmed) {
            let key_part = &trimmed[..colon_pos + 1]; // includes `: `
            spans.push(Span::styled(key_part.to_string(), key_style));
            let rest = &trimmed[colon_pos + 1..];
            spans.extend(colorize_value(rest.trim_start()));
            return spans;
        }
    }

    // Value-only line (array element, or opening `{` / `[`).
    spans.extend(colorize_value(trimmed));
    spans
}

/// Find the `:` position that separates a JSON key from its value.
///
/// Returns the index of the character just after `": "` (or `":"`).
fn find_key_colon(s: &str) -> Option<usize> {
    // s starts with `"`.  Find the closing `"` then look for `:`.
    let mut i = 1usize;
    let bytes = s.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            i += 2; // skip escaped char
            continue;
        }
        if bytes[i] == b'"' {
            // After the closing quote, skip optional whitespace then expect `:`.
            let after = i + 1;
            let rest = &s[after..].trim_start();
            if rest.starts_with(':') {
                // Return position up to and including `: ` (with trailing space).
                let colon_in_rest = s[after..].find(':').unwrap_or(0);
                let after_colon = after + colon_in_rest + 1;
                // Skip one optional space after colon.
                let after_space = if s.as_bytes().get(after_colon) == Some(&b' ') {
                    after_colon + 1
                } else {
                    after_colon
                };
                return Some(after_space.saturating_sub(1));
            }
            return None;
        }
        i += 1;
    }
    None
}

/// Colorize a JSON value fragment (everything after `key: `).
fn colorize_value(value: &str) -> Vec<Span<'static>> {
    let structural_style = Style::default().fg(Color::DarkGray);
    let string_style = Style::default().fg(Color::Green);
    let number_style = Style::default().fg(Color::Cyan);
    let default_style = Style::default().fg(Color::White);

    if value.is_empty() {
        return Vec::new();
    }

    let trimmed = value.trim_end_matches(',');
    let trailing_comma = if value.ends_with(',') { "," } else { "" };

    let span_style = if trimmed.starts_with('"') {
        string_style
    } else if trimmed == "null" || trimmed == "true" || trimmed == "false" {
        number_style // booleans/null share number color
    } else if trimmed.starts_with('{')
        || trimmed.starts_with('}')
        || trimmed.starts_with('[')
        || trimmed.starts_with(']')
    {
        structural_style
    } else if trimmed
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_digit() || c == '-')
    {
        number_style
    } else {
        default_style
    };

    let mut spans = vec![Span::styled(trimmed.to_string(), span_style)];
    if !trailing_comma.is_empty() {
        spans.push(Span::styled(",".to_string(), structural_style));
    }
    spans
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
            let widget = RequestDetails::new(&entry, Some(&detail), tab, false, 0);
            let buf = render_to_buf(widget, 80, 24);
            // Verify we can read the buf text without panicking
            let _ = collect_buf_text(&buf, 80, 24);
        }
    }

    #[test]
    fn test_renders_tiny_terminal() {
        let entry = make_entry();
        let widget = RequestDetails::new(&entry, None, NetworkDetailTab::General, false, 0);
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
        let widget = RequestDetails::new(&entry, None, NetworkDetailTab::General, false, 0);
        // Height == 3 should render tab bar and at least one content row
        let buf = render_to_buf(widget, 80, 3);
        // Should not panic — that's the key requirement
        let _ = collect_buf_text(&buf, 80, 3);
    }

    // ── Tab bar tests ─────────────────────────────────────────────────────────

    #[test]
    fn test_tab_bar_shows_all_tabs() {
        let entry = make_entry();
        let widget = RequestDetails::new(&entry, None, NetworkDetailTab::General, false, 0);
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
        let widget = RequestDetails::new(&entry, None, NetworkDetailTab::General, false, 0);
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
        let widget = RequestDetails::new(&entry, None, NetworkDetailTab::General, false, 0);
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
        let widget = RequestDetails::new(&entry, None, NetworkDetailTab::General, false, 0);
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
        let widget = RequestDetails::new(&entry, None, NetworkDetailTab::General, false, 0);
        let buf = render_to_buf(widget, 80, 20);

        assert!(buf_contains(&buf, 80, 20, "200"), "Should show status code");
    }

    #[test]
    fn test_general_tab_shows_content_type() {
        let entry = make_entry();
        let widget = RequestDetails::new(&entry, None, NetworkDetailTab::General, false, 0);
        let buf = render_to_buf(widget, 80, 20);

        assert!(
            buf_contains(&buf, 80, 20, "application/json"),
            "Should show content-type"
        );
    }

    #[test]
    fn test_general_tab_shows_duration() {
        let entry = make_entry();
        let widget = RequestDetails::new(&entry, None, NetworkDetailTab::General, false, 0);
        let buf = render_to_buf(widget, 80, 20);

        // Duration should be 50ms (1_050_000 - 1_000_000 = 50_000 us = 50 ms)
        assert!(buf_contains(&buf, 80, 20, "50ms"), "Should show duration");
    }

    #[test]
    fn test_general_tab_shows_response_size() {
        let entry = make_entry();
        let widget = RequestDetails::new(&entry, None, NetworkDetailTab::General, false, 0);
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

        let widget = RequestDetails::new(&entry, None, NetworkDetailTab::General, false, 0);
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

        let widget = RequestDetails::new(&entry, None, NetworkDetailTab::General, false, 0);
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
        let widget = RequestDetails::new(&entry, Some(&detail), NetworkDetailTab::General, false, 0);
        let buf = render_to_buf(widget, 80, 30);

        assert!(
            buf_contains(&buf, 80, 30, "93.184.216.34"),
            "Should show remote address"
        );
        assert!(buf_contains(&buf, 80, 30, "443"), "Should show remote port");
    }

    // ── Headers tab tests ─────────────────────────────────────────────────────

    #[test]
    fn test_headers_tab_shows_request_headers() {
        let entry = make_entry();
        let detail = make_detail();
        let widget = RequestDetails::new(&entry, Some(&detail), NetworkDetailTab::Headers, false, 0);
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
        let widget = RequestDetails::new(&entry, Some(&detail), NetworkDetailTab::Headers, false, 0);
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
        let widget = RequestDetails::new(&entry, None, NetworkDetailTab::Headers, false, 0);
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

        let widget = RequestDetails::new(&entry, Some(&detail), NetworkDetailTab::Headers, false, 0);
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
            RequestDetails::new(&entry, Some(&detail), NetworkDetailTab::RequestBody, false, 0);
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
            RequestDetails::new(&entry, Some(&detail), NetworkDetailTab::ResponseBody, false, 0);
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
            RequestDetails::new(&entry, Some(&detail), NetworkDetailTab::ResponseBody, false, 0);
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
            RequestDetails::new(&entry, Some(&detail), NetworkDetailTab::ResponseBody, false, 0);
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
        let widget = RequestDetails::new(&entry, None, NetworkDetailTab::ResponseBody, false, 0);
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
            RequestDetails::new(&entry, Some(&detail), NetworkDetailTab::RequestBody, false, 0);
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
        let widget = RequestDetails::new(&entry, Some(&detail), NetworkDetailTab::Timing, false, 0);
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
        let widget = RequestDetails::new(&entry, Some(&detail), NetworkDetailTab::Timing, false, 0);
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
        let widget = RequestDetails::new(&entry, Some(&detail), NetworkDetailTab::Timing, false, 0);
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
        let widget = RequestDetails::new(&entry, None, NetworkDetailTab::Timing, false, 0);
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

        let widget = RequestDetails::new(&entry, Some(&detail), NetworkDetailTab::Timing, false, 0);
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
        let widget = RequestDetails::new(&entry, None, NetworkDetailTab::General, true, 0);
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
        let widget = RequestDetails::new(&entry, Some(&detail), NetworkDetailTab::General, true, 0);
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

        let widget = RequestDetails::new(&entry, None, NetworkDetailTab::General, false, 0);
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

        let widget = RequestDetails::new(&entry, None, NetworkDetailTab::General, false, 0);
        let buf = render_to_buf(widget, 80, 30);

        assert!(
            buf_contains(&buf, 80, 30, "512 B"),
            "Should show request size of 512 B"
        );
        assert!(
            buf_contains(&buf, 80, 30, "2.0 KB"),
            "Should show response size of 2.0 KB"
        );
    }

    #[test]
    fn test_multiline_response_body() {
        let entry = make_entry();
        let mut detail = make_detail();
        detail.response_body = b"line one\nline two\nline three".to_vec();

        let widget =
            RequestDetails::new(&entry, Some(&detail), NetworkDetailTab::ResponseBody, false, 0);
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

    // ── New T4 acceptance tests ───────────────────────────────────────────────

    #[test]
    fn test_body_wraps_on_narrow_terminal() {
        // A long body line on a narrow terminal (width=30) should wrap; the
        // content must not overflow to the right (no horizontal overflow).
        let entry = make_entry();
        let mut detail = make_detail();
        let long_line = "a".repeat(100);
        detail.response_body = long_line.as_bytes().to_vec();

        let widget =
            RequestDetails::new(&entry, Some(&detail), NetworkDetailTab::ResponseBody, false, 0);
        // Narrow width: 30
        let buf = render_to_buf(widget, 30, 20);

        // Content should appear — not be invisible (no panic, something rendered)
        let text = collect_buf_text(&buf, 30, 20);
        assert!(
            text.contains('a'),
            "Body content should be visible on narrow terminal, got: {text:?}"
        );

        // Verify wrapping: wrap_text_owned with width=27 (30 - 3 for scrollbar/indent)
        // should produce multiple lines for a 100-char string.
        let lines = wrap_text_owned(&long_line, 27);
        assert!(
            lines.len() > 1,
            "Long line should wrap into multiple lines for narrow terminal"
        );
    }

    #[test]
    fn test_scroll_offset_shows_later_lines() {
        // A body with 20 lines in a viewport of 5 rows. scroll_offset=10
        // should show lines 11-15 rather than lines 1-5.
        let entry = make_entry();
        let mut detail = make_detail();
        let body: String = (1..=20).map(|i| format!("LINE{i:02}\n")).collect();
        detail.response_body = body.into_bytes();

        // Without scroll: should see LINE01.
        let widget_no_scroll =
            RequestDetails::new(&entry, Some(&detail), NetworkDetailTab::ResponseBody, false, 0);
        let buf_no_scroll = render_to_buf(widget_no_scroll, 40, 6); // 1 tab + 5 body
        assert!(
            buf_contains(&buf_no_scroll, 40, 6, "LINE01"),
            "Without scroll should show first line"
        );

        // With scroll=10: should see LINE11, not LINE01.
        let widget_scroll =
            RequestDetails::new(&entry, Some(&detail), NetworkDetailTab::ResponseBody, false, 10);
        let buf_scroll = render_to_buf(widget_scroll, 40, 6);
        assert!(
            buf_contains(&buf_scroll, 40, 6, "LINE11"),
            "With scroll=10 should show line 11"
        );
        assert!(
            !buf_contains(&buf_scroll, 40, 6, "LINE01"),
            "With scroll=10 should NOT show line 1"
        );
    }

    #[test]
    fn test_scroll_offset_clamped_to_content_bounds() {
        // scroll_offset=9999 should not panic and should show the last lines.
        let entry = make_entry();
        let mut detail = make_detail();
        let body: String = (1..=5).map(|i| format!("LINE{i}\n")).collect();
        detail.response_body = body.into_bytes();

        let widget =
            RequestDetails::new(&entry, Some(&detail), NetworkDetailTab::ResponseBody, false, 9999);
        let buf = render_to_buf(widget, 40, 10);
        // Should not panic; at least LINE5 should be visible.
        let text = collect_buf_text(&buf, 40, 10);
        assert!(
            text.contains("LINE5") || text.contains("LINE4"),
            "Over-clamped scroll should show last lines, got: {text:?}"
        );
    }

    #[test]
    fn test_json_body_pretty_prints() {
        // A compact JSON body should be pretty-printed.
        let entry = make_entry();
        let mut detail = make_detail();
        detail.response_body = br#"{"name":"Alice","age":30}"#.to_vec();

        let widget =
            RequestDetails::new(&entry, Some(&detail), NetworkDetailTab::ResponseBody, false, 0);
        let buf = render_to_buf(widget, 80, 20);

        // After pretty-printing, "name" and "Alice" appear on separate (indented) lines.
        let text = collect_buf_text(&buf, 80, 20);
        assert!(
            text.contains("name"),
            "JSON key 'name' should be visible, got: {text:?}"
        );
        assert!(
            text.contains("Alice"),
            "JSON value 'Alice' should be visible, got: {text:?}"
        );
    }

    #[test]
    fn test_non_json_body_falls_back_to_raw() {
        // A non-JSON body should be displayed as-is (no panic, correct content).
        let entry = make_entry();
        let mut detail = make_detail();
        detail.response_body = b"plain text response, not JSON at all".to_vec();

        let widget =
            RequestDetails::new(&entry, Some(&detail), NetworkDetailTab::ResponseBody, false, 0);
        let buf = render_to_buf(widget, 80, 20);

        assert!(
            buf_contains(&buf, 80, 20, "plain text"),
            "Non-JSON body should render as raw text"
        );
    }

    #[test]
    fn test_invalid_json_falls_back_to_raw() {
        // Malformed JSON should fall back to raw text.
        let entry = make_entry();
        let mut detail = make_detail();
        detail.response_body = b"{invalid json}".to_vec();

        let widget =
            RequestDetails::new(&entry, Some(&detail), NetworkDetailTab::ResponseBody, false, 0);
        let buf = render_to_buf(widget, 80, 20);

        assert!(
            buf_contains(&buf, 80, 20, "invalid"),
            "Malformed JSON should fall back to raw text"
        );
    }

    // ── wrap_text_owned unit tests ────────────────────────────────────────────

    #[test]
    fn test_wrap_text_owned_short_text_single_line() {
        let lines = wrap_text_owned("hello", 20);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0], "hello");
    }

    #[test]
    fn test_wrap_text_owned_long_text_wraps() {
        let long = "a".repeat(100);
        let lines = wrap_text_owned(&long, 20);
        assert!(lines.len() > 1, "100-char string should wrap at width 20");
        // Each line should be at most 20 chars.
        for line in &lines {
            assert!(
                line.len() <= 20,
                "Wrapped line should be at most 20 chars, got len={}",
                line.len()
            );
        }
    }

    #[test]
    fn test_wrap_text_owned_empty_input() {
        let lines = wrap_text_owned("", 20);
        assert!(lines.is_empty(), "Empty input should produce no lines");
    }

    #[test]
    fn test_wrap_text_owned_zero_width() {
        let lines = wrap_text_owned("hello", 0);
        assert!(lines.is_empty(), "Zero width should produce no lines");
    }
}
