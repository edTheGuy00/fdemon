//! # Network Monitor Domain Types
//!
//! Domain data types for representing HTTP request/response entries, socket
//! profiling data, timing breakdowns, and UI state for the Network Monitor tab.
//!
//! These types are the shared vocabulary between:
//! - `fdemon-daemon` (parsing VM Service HTTP profile responses)
//! - `fdemon-app` (state management and aggregation)
//! - `fdemon-tui` (rendering the network monitor UI)
//!
//! ## Protocol Assumptions
//!
//! - **Protocol v4.0+**: All IDs are `String`, all timestamps are `i64` (microseconds
//!   since Unix epoch). Targets Dart 3.0+ / Flutter 3.10+.
//! - **Bodies are `Vec<u8>`**: The VM Service transmits bodies as JSON int arrays.
//! - **Headers as `Vec<(String, Vec<String>)>`**: Preserves insertion order and
//!   supports duplicate header names.
//! - **`NetworkTiming` is computed**: Computed on-demand from the event list, not
//!   stored as separate fields, to avoid redundancy.

// ── HttpProfileEntry ──────────────────────────────────────────────────────────

/// Summary of a single HTTP request from the VM Service HTTP profile.
///
/// Returned by `ext.dart.io.getHttpProfile`. Does NOT include request/response
/// bodies — those require a separate `getHttpProfileRequest` call.
#[derive(Debug, Clone)]
pub struct HttpProfileEntry {
    /// Unique request identifier (String in protocol v4.0+).
    pub id: String,
    /// HTTP method: GET, POST, PUT, DELETE, PATCH, HEAD, OPTIONS, etc.
    pub method: String,
    /// Full request URI.
    pub uri: String,
    /// HTTP status code. `None` while the request is still in-flight.
    pub status_code: Option<u16>,
    /// Content-Type from response headers, if available.
    pub content_type: Option<String>,
    /// Request start time (microseconds since Unix epoch).
    pub start_time_us: i64,
    /// Request end time (microseconds since Unix epoch). `None` if in-flight.
    pub end_time_us: Option<i64>,
    /// Request body content length in bytes. -1 or `None` if unknown.
    pub request_content_length: Option<i64>,
    /// Response body content length in bytes. -1 or `None` if unknown.
    pub response_content_length: Option<i64>,
    /// Error message if the request failed.
    pub error: Option<String>,
}

impl HttpProfileEntry {
    /// Whether this request is still in-flight (no end time or status).
    pub fn is_pending(&self) -> bool {
        self.end_time_us.is_none()
    }

    /// Duration in milliseconds. `None` if still pending.
    pub fn duration_ms(&self) -> Option<f64> {
        self.end_time_us
            .map(|end| (end - self.start_time_us) as f64 / 1000.0)
    }

    /// Whether the request resulted in an error (non-2xx or explicit error).
    pub fn is_error(&self) -> bool {
        self.error.is_some() || self.status_code.is_some_and(|s| s >= 400)
    }

    /// Human-readable response size. Returns `None` if unknown or negative.
    pub fn response_size_display(&self) -> Option<String> {
        self.response_content_length
            .filter(|&len| len >= 0)
            .map(|len| format_bytes(len as u64))
    }

    /// Short path from the URI (strips scheme + host for display).
    pub fn short_uri(&self) -> &str {
        // Try to find the path portion after the authority
        if let Some(rest) = self
            .uri
            .strip_prefix("https://")
            .or_else(|| self.uri.strip_prefix("http://"))
        {
            if let Some(slash_pos) = rest.find('/') {
                return &rest[slash_pos..];
            }
        }
        &self.uri
    }
}

// ── HttpProfileEntryDetail ────────────────────────────────────────────────────

/// Full detail for a single HTTP request, including headers and bodies.
///
/// Returned by `ext.dart.io.getHttpProfileRequest`. Bodies are raw bytes
/// (transmitted as JSON int[] arrays from the VM Service).
#[derive(Debug, Clone)]
pub struct HttpProfileEntryDetail {
    /// The base entry summary.
    pub entry: HttpProfileEntry,
    /// Request headers: header name → list of values.
    pub request_headers: Vec<(String, Vec<String>)>,
    /// Response headers: header name → list of values.
    pub response_headers: Vec<(String, Vec<String>)>,
    /// Raw request body bytes. Empty if no body or not yet available.
    pub request_body: Vec<u8>,
    /// Raw response body bytes. Empty if no body or not yet available.
    pub response_body: Vec<u8>,
    /// Timeline events for this request (connection, send, receive, etc.).
    pub events: Vec<HttpProfileEvent>,
    /// Connection info (remote address, ports).
    pub connection_info: Option<ConnectionInfo>,
}

impl HttpProfileEntryDetail {
    /// Request body as a UTF-8 string slice, or None if empty or not valid UTF-8.
    ///
    /// Returns a borrowed `&str` into the existing byte buffer to avoid an
    /// allocation. Call `.to_string()` at the call site if an owned value is needed.
    pub fn request_body_text(&self) -> Option<&str> {
        if self.request_body.is_empty() {
            return None;
        }
        std::str::from_utf8(&self.request_body).ok()
    }

    /// Response body as a UTF-8 string slice, or None if empty or not valid UTF-8.
    ///
    /// Returns a borrowed `&str` into the existing byte buffer to avoid an
    /// allocation. Call `.to_string()` at the call site if an owned value is needed.
    pub fn response_body_text(&self) -> Option<&str> {
        if self.response_body.is_empty() {
            return None;
        }
        std::str::from_utf8(&self.response_body).ok()
    }

    /// Compute timing breakdown from events.
    pub fn timing(&self) -> NetworkTiming {
        NetworkTiming::from_events(&self.events, &self.entry)
    }
}

// ── HttpProfileEvent ──────────────────────────────────────────────────────────

/// A timeline event within an HTTP request lifecycle.
#[derive(Debug, Clone)]
pub struct HttpProfileEvent {
    /// Event description (e.g., "connection established", "request sent").
    pub event: String,
    /// Event timestamp (microseconds since Unix epoch).
    pub timestamp_us: i64,
}

// ── ConnectionInfo ────────────────────────────────────────────────────────────

/// Connection info for an HTTP request.
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    /// Local port used for this connection.
    pub local_port: Option<u16>,
    /// Remote IP address.
    pub remote_address: Option<String>,
    /// Remote port.
    pub remote_port: Option<u16>,
}

// ── NetworkTiming ─────────────────────────────────────────────────────────────

/// Timing breakdown for a network request, computed from timeline events.
#[derive(Debug, Clone, Default)]
pub struct NetworkTiming {
    /// Total duration in milliseconds.
    pub total_ms: f64,
    /// Time from first event to "connection established" event.
    pub connection_ms: Option<f64>,
    /// Time spent waiting for the first response byte.
    pub waiting_ms: Option<f64>,
    /// Time receiving the response body.
    pub receiving_ms: Option<f64>,
}

impl NetworkTiming {
    /// Compute timing from the event list and entry timestamps.
    pub fn from_events(events: &[HttpProfileEvent], entry: &HttpProfileEntry) -> Self {
        let total_ms = entry.duration_ms().unwrap_or(0.0);

        // Find known event timestamps for breakdown
        let connection_ts = events
            .iter()
            .find(|e| e.event.contains("connection"))
            .map(|e| e.timestamp_us);
        let response_start_ts = events
            .iter()
            .find(|e| e.event.contains("response"))
            .map(|e| e.timestamp_us);

        let connection_ms = connection_ts.map(|ts| (ts - entry.start_time_us) as f64 / 1000.0);
        let waiting_ms = response_start_ts.map(|rs| {
            let base = connection_ts.unwrap_or(entry.start_time_us);
            (rs - base) as f64 / 1000.0
        });
        let receiving_ms = entry
            .end_time_us
            .and_then(|end| response_start_ts.map(|rs| (end - rs) as f64 / 1000.0));

        Self {
            total_ms,
            connection_ms,
            waiting_ms,
            receiving_ms,
        }
    }
}

// ── SocketEntry ───────────────────────────────────────────────────────────────

/// A socket statistics entry from the VM Service socket profile.
#[derive(Debug, Clone)]
pub struct SocketEntry {
    /// Unique socket identifier.
    pub id: String,
    /// Remote address (IP).
    pub address: String,
    /// Remote port.
    pub port: u16,
    /// Socket type: "tcp" or "udp".
    pub socket_type: String,
    /// Socket open time (microseconds since Unix epoch).
    pub start_time_us: i64,
    /// Socket close time. `None` if still open.
    pub end_time_us: Option<i64>,
    /// Total bytes read through this socket.
    pub read_bytes: u64,
    /// Total bytes written through this socket.
    pub write_bytes: u64,
}

// ── Body formatting ───────────────────────────────────────────────────────────

/// Maximum body size to attempt pretty-printing (4 MiB).
///
/// Bodies larger than this are returned as a trimmed placeholder to avoid
/// attempting to allocate a potentially very large pretty-printed string.
const MAX_PRETTY_PRINT_BYTES: usize = 4 * 1024 * 1024;

/// Format a body string for display.
///
/// - If `body` is empty, returns an empty string.
/// - If `body.len()` exceeds [`MAX_PRETTY_PRINT_BYTES`], returns a placeholder
///   string indicating the body is too large to display.
/// - If `body` parses as valid JSON, returns the pretty-printed JSON string.
/// - Otherwise returns the raw text unchanged.
///
/// This function never panics and never allocates more than `O(body.len())`
/// memory (the pretty-printed JSON may be slightly larger in the worst case but
/// is bounded by the `MAX_PRETTY_PRINT_BYTES` guard).
pub fn format_body_text(body: &str) -> String {
    if body.is_empty() {
        return String::new();
    }

    if body.len() > MAX_PRETTY_PRINT_BYTES {
        return format!(
            "(body too large to display: {})",
            format_bytes(body.len() as u64)
        );
    }

    // Attempt JSON pretty-print. Parse cost is bounded by `MAX_PRETTY_PRINT_BYTES`;
    // only `to_string_pretty` allocates and it is bounded by the guard above.
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(body) {
        // `to_string_pretty` should not fail for a valid Value, but handle the
        // error gracefully rather than unwrapping.
        if let Ok(pretty) = serde_json::to_string_pretty(&value) {
            return pretty;
        }
    }

    // Not valid JSON or pretty-print failed — return the raw text.
    body.to_string()
}

// ── Helper functions ──────────────────────────────────────────────────────────

/// Format a byte count as a human-readable string (B, KB, MB).
pub fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

/// Format a duration in milliseconds for display.
pub fn format_duration_ms(ms: f64) -> String {
    if ms < 1.0 {
        format!("{:.0}us", ms * 1000.0)
    } else if ms < 1000.0 {
        format!("{:.0}ms", ms)
    } else {
        format!("{:.2}s", ms / 1000.0)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(status: Option<u16>, start: i64, end: Option<i64>) -> HttpProfileEntry {
        HttpProfileEntry {
            id: "req_1".to_string(),
            method: "GET".to_string(),
            uri: "https://api.example.com/data?q=1".to_string(),
            status_code: status,
            content_type: Some("application/json".to_string()),
            start_time_us: start,
            end_time_us: end,
            request_content_length: None,
            response_content_length: Some(1024),
            error: None,
        }
    }

    #[test]
    fn test_is_pending_when_no_end_time() {
        let entry = make_entry(None, 1_000_000, None);
        assert!(entry.is_pending());
    }

    #[test]
    fn test_is_not_pending_when_completed() {
        let entry = make_entry(Some(200), 1_000_000, Some(1_050_000));
        assert!(!entry.is_pending());
    }

    #[test]
    fn test_duration_ms_completed() {
        let entry = make_entry(Some(200), 1_000_000, Some(1_050_000));
        let dur = entry.duration_ms().unwrap();
        assert!((dur - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_duration_ms_pending_returns_none() {
        let entry = make_entry(None, 1_000_000, None);
        assert!(entry.duration_ms().is_none());
    }

    #[test]
    fn test_is_error_4xx() {
        let mut entry = make_entry(Some(404), 0, Some(1000));
        assert!(entry.is_error());
        entry.status_code = Some(200);
        assert!(!entry.is_error());
    }

    #[test]
    fn test_is_error_explicit_error() {
        let mut entry = make_entry(None, 0, None);
        entry.error = Some("Connection refused".to_string());
        assert!(entry.is_error());
    }

    #[test]
    fn test_short_uri_strips_authority() {
        let entry = make_entry(Some(200), 0, Some(1000));
        assert_eq!(entry.short_uri(), "/data?q=1");
    }

    #[test]
    fn test_short_uri_no_scheme_returns_full() {
        let mut entry = make_entry(Some(200), 0, Some(1000));
        entry.uri = "/local/path".to_string();
        assert_eq!(entry.short_uri(), "/local/path");
    }

    #[test]
    fn test_response_size_display() {
        let entry = make_entry(Some(200), 0, Some(1000));
        assert_eq!(entry.response_size_display(), Some("1.0 KB".to_string()));
    }

    #[test]
    fn test_response_size_display_none_when_negative() {
        let mut entry = make_entry(Some(200), 0, Some(1000));
        entry.response_content_length = Some(-1);
        assert!(entry.response_size_display().is_none());
    }

    #[test]
    fn test_detail_body_text_valid_utf8() {
        let detail = HttpProfileEntryDetail {
            entry: make_entry(Some(200), 0, Some(1000)),
            request_headers: vec![],
            response_headers: vec![],
            request_body: b"hello".to_vec(),
            response_body: b"{\"ok\":true}".to_vec(),
            events: vec![],
            connection_info: None,
        };
        assert_eq!(detail.request_body_text(), Some("hello"));
        assert_eq!(detail.response_body_text(), Some("{\"ok\":true}"));
    }

    #[test]
    fn test_detail_body_text_empty() {
        let detail = HttpProfileEntryDetail {
            entry: make_entry(Some(200), 0, Some(1000)),
            request_headers: vec![],
            response_headers: vec![],
            request_body: vec![],
            response_body: vec![],
            events: vec![],
            connection_info: None,
        };
        assert!(detail.request_body_text().is_none());
        assert!(detail.response_body_text().is_none());
    }

    #[test]
    fn test_network_timing_from_events() {
        let entry = make_entry(Some(200), 1_000_000, Some(1_100_000));
        let events = vec![
            HttpProfileEvent {
                event: "connection established".to_string(),
                timestamp_us: 1_020_000,
            },
            HttpProfileEvent {
                event: "response started".to_string(),
                timestamp_us: 1_060_000,
            },
        ];
        let timing = NetworkTiming::from_events(&events, &entry);
        assert!((timing.total_ms - 100.0).abs() < f64::EPSILON);
        assert!((timing.connection_ms.unwrap() - 20.0).abs() < f64::EPSILON);
        assert!((timing.waiting_ms.unwrap() - 40.0).abs() < f64::EPSILON);
        assert!((timing.receiving_ms.unwrap() - 40.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_network_timing_no_events() {
        let entry = make_entry(Some(200), 0, Some(50_000));
        let timing = NetworkTiming::from_events(&[], &entry);
        assert!((timing.total_ms - 50.0).abs() < f64::EPSILON);
        assert!(timing.connection_ms.is_none());
        assert!(timing.waiting_ms.is_none());
        assert!(timing.receiving_ms.is_none());
    }

    // ── format_body_text tests ────────────────────────────────────────────────

    #[test]
    fn test_format_body_text_empty_returns_empty() {
        assert_eq!(format_body_text(""), "");
    }

    #[test]
    fn test_format_body_text_valid_json_pretty_prints() {
        let input = r#"{"name":"Alice","age":30}"#;
        let result = format_body_text(input);
        // Pretty-printed JSON should contain newlines and indentation.
        assert!(
            result.contains('\n'),
            "Pretty-printed JSON should contain newlines"
        );
        assert!(
            result.contains("name"),
            "Pretty-printed JSON should contain the key 'name'"
        );
        assert!(
            result.contains("Alice"),
            "Pretty-printed JSON should contain the value 'Alice'"
        );
    }

    #[test]
    fn test_format_body_text_invalid_json_returns_raw() {
        let raw = "{invalid json}";
        let result = format_body_text(raw);
        assert_eq!(
            result, raw,
            "Invalid JSON should be returned as-is (no panic, no modification)"
        );
    }

    #[test]
    fn test_format_body_text_non_json_returns_raw() {
        let raw = "plain text, not JSON";
        let result = format_body_text(raw);
        assert_eq!(result, raw, "Non-JSON text should be returned unchanged");
    }

    #[test]
    fn test_format_body_text_oversized_body_returns_placeholder() {
        // Build a string larger than MAX_PRETTY_PRINT_BYTES (4 MiB).
        let oversized = "x".repeat(5 * 1024 * 1024);
        let result = format_body_text(&oversized);
        assert!(
            result.contains("too large"),
            "Oversized body should produce a placeholder message"
        );
        assert!(
            !result.contains('x'),
            "Oversized body placeholder should not contain raw content"
        );
    }

    #[test]
    fn test_format_body_text_json_array() {
        let input = r#"[1,2,3]"#;
        let result = format_body_text(input);
        // JSON arrays should also be pretty-printed.
        assert!(result.contains('\n'), "JSON array should be pretty-printed");
        assert!(
            result.contains('1'),
            "Pretty result should contain element 1"
        );
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(1_048_576), "1.0 MB");
        assert_eq!(format_bytes(2_621_440), "2.5 MB");
    }

    #[test]
    fn test_format_duration_ms() {
        assert_eq!(format_duration_ms(0.5), "500us");
        assert_eq!(format_duration_ms(42.0), "42ms");
        assert_eq!(format_duration_ms(1500.0), "1.50s");
    }

    #[test]
    fn test_socket_entry_construction() {
        let socket = SocketEntry {
            id: "sock_1".to_string(),
            address: "93.184.216.34".to_string(),
            port: 443,
            socket_type: "tcp".to_string(),
            start_time_us: 1_000_000,
            end_time_us: Some(2_000_000),
            read_bytes: 4096,
            write_bytes: 512,
        };
        assert_eq!(socket.port, 443);
        assert_eq!(socket.read_bytes, 4096);
    }
}
