//! Tests for the [`NetworkMonitor`] widget.

use super::*;
use fdemon_app::session::NetworkState;
use fdemon_app::state::VmConnectionStatus;
use fdemon_core::network::HttpProfileEntry;
use ratatui::{buffer::Buffer, layout::Rect, style::Color};

// ── Test helpers ──────────────────────────────────────────────────────────────

fn make_entry(id: &str, i: usize) -> HttpProfileEntry {
    HttpProfileEntry {
        id: id.to_string(),
        method: if i % 2 == 0 {
            "GET".to_string()
        } else {
            "POST".to_string()
        },
        uri: format!("https://api.example.com/resource/{}", i),
        status_code: Some(200),
        content_type: Some("application/json".to_string()),
        start_time_us: 1_000_000 + (i as i64 * 50_000),
        end_time_us: Some(1_050_000 + (i as i64 * 50_000)),
        request_content_length: None,
        response_content_length: Some(1024),
        error: None,
    }
}

fn make_network_state() -> NetworkState {
    NetworkState::default()
}

fn make_network_state_with_entries(n: usize) -> NetworkState {
    let mut state = NetworkState::default();
    for i in 0..n {
        state.merge_entries(vec![make_entry(&format!("req_{}", i), i)]);
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
    for y in 0..h {
        for x in 0..w {
            if let Some(c) = buf.cell((x, y)) {
                full.push_str(c.symbol());
            }
        }
    }
    full.contains(text)
}

// ── No-panic / basic render tests ─────────────────────────────────────────────

#[test]
fn test_renders_without_panic() {
    render_monitor(&make_network_state(), true, 80, 24);
}

#[test]
fn test_tiny_terminal_no_panic() {
    render_monitor(&make_network_state(), true, 10, 3);
}

#[test]
fn test_zero_height_no_panic() {
    render_monitor(&make_network_state(), true, 80, 0);
}

#[test]
fn test_zero_width_no_panic() {
    render_monitor(&make_network_state(), true, 0, 24);
}

#[test]
fn test_large_terminal_no_panic() {
    render_monitor(&make_network_state(), true, 200, 60);
}

// ── Disconnected state tests ───────────────────────────────────────────────────

#[test]
fn test_disconnected_state_shows_waiting_message() {
    let buf = render_monitor(&make_network_state(), false, 80, 24);
    assert!(
        buf_contains(&buf, 80, 24, "Waiting for VM Service"),
        "Disconnected state should show waiting message"
    );
}

#[test]
fn test_reconnecting_shows_attempt_and_max() {
    let state = make_network_state();
    let conn_status = VmConnectionStatus::Reconnecting {
        attempt: 3,
        max_attempts: 10,
    };
    let widget = NetworkMonitor::new(&state, false, &conn_status);
    let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
    widget.render(Rect::new(0, 0, 80, 24), &mut buf);
    assert!(
        buf_contains(&buf, 80, 24, "3"),
        "Should show attempt number 3"
    );
    assert!(
        buf_contains(&buf, 80, 24, "10"),
        "Should show max attempts 10"
    );
}

#[test]
fn test_timed_out_shows_timed_out_message() {
    let state = make_network_state();
    let conn_status = VmConnectionStatus::TimedOut;
    let widget = NetworkMonitor::new(&state, false, &conn_status);
    let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
    widget.render(Rect::new(0, 0, 80, 24), &mut buf);
    assert!(
        buf_contains(&buf, 80, 24, "timed out"),
        "TimedOut state should show timed out message"
    );
}

// ── Extensions unavailable state ──────────────────────────────────────────────

#[test]
fn test_extensions_unavailable_shows_message() {
    let mut state = make_network_state();
    state.extensions_available = Some(false);
    let buf = render_monitor(&state, true, 80, 24);
    assert!(
        buf_contains(&buf, 80, 24, "not available"),
        "Extensions unavailable should show 'not available' message"
    );
}

#[test]
fn test_extensions_unavailable_shows_release_mode_hint() {
    let mut state = make_network_state();
    state.extensions_available = Some(false);
    let buf = render_monitor(&state, true, 80, 24);
    assert!(
        buf_contains(&buf, 80, 24, "release mode"),
        "Should mention release mode when extensions unavailable"
    );
}

#[test]
fn test_extensions_none_does_not_show_unavailable() {
    // extensions_available = None means we haven't checked yet, should NOT show the message
    let state = make_network_state();
    assert_eq!(state.extensions_available, None);
    let buf = render_monitor(&state, true, 80, 24);
    assert!(
        !buf_contains(&buf, 80, 24, "not available"),
        "extensions_available=None should not show unavailable message"
    );
}

// ── Empty state / recording indicator ─────────────────────────────────────────

#[test]
fn test_empty_state_connected_shows_recording_indicator() {
    let buf = render_monitor(&make_network_state(), true, 80, 24);
    assert!(
        buf_contains(&buf, 80, 24, "REC"),
        "Empty state with VM connected should show REC indicator"
    );
}

#[test]
fn test_empty_state_connected_shows_zero_requests() {
    let buf = render_monitor(&make_network_state(), true, 80, 24);
    assert!(
        buf_contains(&buf, 80, 24, "0 requests"),
        "Empty state should show 0 requests"
    );
}

// ── Table display with entries ─────────────────────────────────────────────────

#[test]
fn test_with_entries_shows_table_with_method() {
    let state = make_network_state_with_entries(5);
    let buf = render_monitor(&state, true, 100, 24);
    assert!(
        buf_contains(&buf, 100, 24, "GET"),
        "Should show GET method in table"
    );
}

#[test]
fn test_with_entries_shows_request_count() {
    let state = make_network_state_with_entries(5);
    let buf = render_monitor(&state, true, 100, 24);
    assert!(
        buf_contains(&buf, 100, 24, "5 requests"),
        "Should show '5 requests' count"
    );
}

// ── Wide terminal layout ───────────────────────────────────────────────────────

#[test]
fn test_wide_terminal_with_selection_shows_split() {
    let mut state = make_network_state_with_entries(5);
    state.selected_index = Some(0);
    let buf = render_monitor(&state, true, 120, 24);
    // Should show both table content and detail panel
    assert!(
        buf_contains(&buf, 120, 24, "GET"),
        "Wide layout should show table with GET"
    );
    assert!(
        buf_contains(&buf, 120, 24, "General"),
        "Wide layout should show detail panel tab bar"
    );
}

#[test]
fn test_wide_terminal_exact_threshold_shows_split() {
    // Exactly at WIDE_THRESHOLD (100) should use wide layout
    let mut state = make_network_state_with_entries(3);
    state.selected_index = Some(0);
    let buf = render_monitor(&state, true, 100, 24);
    assert!(
        buf_contains(&buf, 100, 24, "GET"),
        "At WIDE_THRESHOLD should show table"
    );
    assert!(
        buf_contains(&buf, 100, 24, "General"),
        "At WIDE_THRESHOLD should show detail"
    );
}

#[test]
fn test_wide_terminal_no_selection_shows_full_table() {
    // Wide terminal but no selection: full-width table
    let state = make_network_state_with_entries(5);
    let buf = render_monitor(&state, true, 120, 24);
    assert!(
        buf_contains(&buf, 120, 24, "GET"),
        "Wide without selection should show table"
    );
    // No detail panel tab bar expected
    assert!(
        !buf_contains(&buf, 120, 24, "General"),
        "Wide without selection should not show detail panel"
    );
}

// ── Narrow terminal layout ─────────────────────────────────────────────────────

#[test]
fn test_narrow_terminal_no_selection_shows_table() {
    let state = make_network_state_with_entries(5);
    let buf = render_monitor(&state, true, 60, 24);
    assert!(
        buf_contains(&buf, 60, 24, "GET"),
        "Narrow without selection should show table"
    );
}

#[test]
fn test_narrow_terminal_with_selection_shows_vertical_split() {
    let mut state = make_network_state_with_entries(3);
    state.selected_index = Some(0);
    let buf = render_monitor(&state, true, 60, 24);
    // Narrow vertical split: both table content (top half) and detail (bottom half) visible
    assert!(
        buf_contains(&buf, 60, 24, "GET"),
        "Narrow vertical split should show table (top half) with GET method"
    );
    assert!(
        buf_contains(&buf, 60, 24, "General"),
        "Narrow vertical split should show detail tab bar (bottom half)"
    );
}

#[test]
fn test_narrow_terminal_with_selection_shows_both_panels() {
    // Additional verification: narrow split shows request count (table header)
    // AND detail tab bar simultaneously
    let mut state = make_network_state_with_entries(3);
    state.selected_index = Some(0);
    let buf = render_monitor(&state, true, 60, 24);
    assert!(
        buf_contains(&buf, 60, 24, "requests"),
        "Narrow vertical split should show 'requests' count from table header"
    );
    assert!(
        buf_contains(&buf, 60, 24, "General"),
        "Narrow vertical split should show 'General' detail tab"
    );
}

#[test]
fn test_narrow_terminal_just_below_threshold_uses_vertical_split() {
    // Width 99 is just below WIDE_THRESHOLD (100) — must use vertical split
    let mut state = make_network_state_with_entries(3);
    state.selected_index = Some(0);
    let buf = render_monitor(&state, true, 99, 24);
    assert!(
        buf_contains(&buf, 99, 24, "GET"),
        "Width 99 (< WIDE_THRESHOLD) should use vertical split showing table"
    );
    assert!(
        buf_contains(&buf, 99, 24, "General"),
        "Width 99 (< WIDE_THRESHOLD) should use vertical split showing details"
    );
}

// ── Footer row reservation ─────────────────────────────────────────────────────

#[test]
fn test_footer_row_reserved_no_content_on_last_row() {
    let state = make_network_state_with_entries(20);
    let buf = render_monitor(&state, true, 80, 10);
    // Last row (y=9) should be blank — reserved for parent footer
    let last_row_y = 9u16;
    let mut last_row_blank = true;
    for x in 0..80 {
        if let Some(c) = buf.cell((x, last_row_y)) {
            if c.symbol() != " " {
                last_row_blank = false;
                break;
            }
        }
    }
    assert!(
        last_row_blank,
        "Last row should be reserved for parent footer (blank)"
    );
}

// ── http_method_color unit tests ──────────────────────────────────────────────

#[test]
fn test_http_method_color_get_is_green() {
    assert_eq!(
        http_method_color("GET"),
        Color::Green,
        "GET should be green"
    );
}

#[test]
fn test_http_method_color_post_is_blue() {
    assert_eq!(
        http_method_color("POST"),
        Color::Blue,
        "POST should be blue"
    );
}

#[test]
fn test_http_method_color_put_is_yellow() {
    assert_eq!(
        http_method_color("PUT"),
        Color::Yellow,
        "PUT should be yellow"
    );
}

#[test]
fn test_http_method_color_patch_is_yellow() {
    assert_eq!(
        http_method_color("PATCH"),
        Color::Yellow,
        "PATCH should be yellow (same as PUT)"
    );
}

#[test]
fn test_http_method_color_delete_is_red() {
    assert_eq!(
        http_method_color("DELETE"),
        Color::Red,
        "DELETE should be red"
    );
}

#[test]
fn test_http_method_color_head_is_cyan() {
    assert_eq!(
        http_method_color("HEAD"),
        Color::Cyan,
        "HEAD should be cyan"
    );
}

#[test]
fn test_http_method_color_options_is_magenta() {
    assert_eq!(
        http_method_color("OPTIONS"),
        Color::Magenta,
        "OPTIONS should be magenta"
    );
}

#[test]
fn test_http_method_color_unknown_is_white() {
    assert_eq!(
        http_method_color("CONNECT"),
        Color::White,
        "Unknown methods should be white"
    );
    assert_eq!(
        http_method_color(""),
        Color::White,
        "Empty method should be white"
    );
}

// ── Paused recording ──────────────────────────────────────────────────────────

#[test]
fn test_paused_recording_shows_paused_indicator() {
    let mut state = make_network_state();
    state.recording = false;
    let buf = render_monitor(&state, true, 80, 24);
    assert!(
        buf_contains(&buf, 80, 24, "PAUSED"),
        "Paused recording should show PAUSED indicator"
    );
}

// ── Filter display ────────────────────────────────────────────────────────────

#[test]
fn test_active_filter_shown_in_header() {
    let mut state = make_network_state_with_entries(5);
    state.filter = "api".to_string();
    let buf = render_monitor(&state, true, 80, 24);
    assert!(
        buf_contains(&buf, 80, 24, "filter: api"),
        "Active filter should be shown in header"
    );
}
