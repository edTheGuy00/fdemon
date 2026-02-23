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

// ── Filter input bar tests ────────────────────────────────────────────────────

#[test]
fn test_filter_input_bar_shows_prompt_when_active() {
    let mut state = make_network_state();
    state.filter_input_active = true;
    state.filter_input_buffer = "api".to_string();
    let buf = render_monitor(&state, true, 80, 24);
    assert!(
        buf_contains(&buf, 80, 24, "Filter:"),
        "Filter input bar should show 'Filter:' prompt when active"
    );
}

#[test]
fn test_filter_input_bar_shows_buffer_text() {
    let mut state = make_network_state();
    state.filter_input_active = true;
    state.filter_input_buffer = "api/users".to_string();
    let buf = render_monitor(&state, true, 80, 24);
    assert!(
        buf_contains(&buf, 80, 24, "api/users"),
        "Filter input bar should show the buffer text"
    );
}

#[test]
fn test_filter_input_bar_shows_hint() {
    let mut state = make_network_state();
    state.filter_input_active = true;
    state.filter_input_buffer = String::new();
    let buf = render_monitor(&state, true, 80, 24);
    assert!(
        buf_contains(&buf, 80, 24, "Enter"),
        "Filter input bar should show 'Enter' in key hint"
    );
    assert!(
        buf_contains(&buf, 80, 24, "Esc"),
        "Filter input bar should show 'Esc' in key hint"
    );
}

#[test]
fn test_filter_input_bar_inactive_does_not_show_prompt() {
    let mut state = make_network_state_with_entries(3);
    state.filter_input_active = false;
    let buf = render_monitor(&state, true, 80, 24);
    assert!(
        !buf_contains(&buf, 80, 24, "Filter:"),
        "Filter prompt should not appear when filter input is inactive"
    );
}

#[test]
fn test_filter_input_bar_no_panic_on_tiny_terminal() {
    let mut state = make_network_state();
    state.filter_input_active = true;
    state.filter_input_buffer = "api".to_string();
    // Should not panic even at tiny sizes
    render_monitor(&state, true, 10, 5);
    render_monitor(&state, true, 80, 4);
}

#[test]
fn test_filter_input_bar_active_still_shows_requests() {
    let mut state = make_network_state_with_entries(3);
    state.filter_input_active = true;
    state.filter_input_buffer = "GET".to_string();
    let buf = render_monitor(&state, true, 80, 24);
    // Filter bar takes one row but the table should still be visible below.
    assert!(
        buf_contains(&buf, 80, 24, "Filter:"),
        "Filter bar should be shown"
    );
    // Table is still rendered (may show 'requests' count from table header).
    // Note: the filter bar takes row 0, so the table starts from row 1.
    assert!(
        buf_contains(&buf, 80, 24, "requests"),
        "Table with request count should still be visible below the filter bar"
    );
}

// ── Cursor position: display width vs byte length ─────────────────────────────

/// Helper: returns the x column of the cursor cell (rendered with REVERSED
/// modifier) in the filter bar row (y == 0 of the rendered buffer).
fn find_cursor_x(buf: &Buffer, w: u16) -> Option<u16> {
    use ratatui::style::Modifier;
    for x in 0..w {
        if let Some(cell) = buf.cell((x, 0u16)) {
            if cell.style().add_modifier.contains(Modifier::REVERSED) {
                return Some(x);
            }
        }
    }
    None
}

#[test]
fn test_filter_input_bar_cursor_position_with_multibyte() {
    // "日本" is 6 bytes but 4 display columns (each CJK char is 2 columns wide).
    // prompt "Filter: " is 8 bytes and 8 display columns (ASCII only).
    // Expected cursor x = 0 (area.x) + 8 (prompt) + 4 (display width of "日本") = 12.
    // Before the fix, .len() would give 8 + 6 = 14 — two columns too far right.
    let mut state = make_network_state();
    state.filter_input_active = true;
    state.filter_input_buffer = "日本".to_string();

    let buf = render_monitor(&state, true, 80, 24);

    let cursor_x = find_cursor_x(&buf, 80)
        .expect("cursor cell (REVERSED style) should be present in the filter bar row");

    // prompt width = 8, "日本" display width = 4  ->  expected x = 12
    assert_eq!(
        cursor_x, 12,
        "cursor should be at column 12 (prompt 8 + display width 4 of '日本'), \
         not column 14 (which byte-length would give)"
    );
}

#[test]
fn test_filter_input_bar_cursor_position_ascii_input() {
    // ASCII only: byte length == display width, so both approaches give the same result.
    // "api" is 3 bytes and 3 display columns.
    // Expected cursor x = 0 + 8 (prompt) + 3 (display of "api") = 11.
    let mut state = make_network_state();
    state.filter_input_active = true;
    state.filter_input_buffer = "api".to_string();

    let buf = render_monitor(&state, true, 80, 24);

    let cursor_x = find_cursor_x(&buf, 80)
        .expect("cursor cell (REVERSED style) should be present in the filter bar row");

    assert_eq!(
        cursor_x, 11,
        "cursor should be at column 11 (prompt 8 + display width 3 of 'api')"
    );
}

#[test]
fn test_filter_input_bar_cursor_position_empty_buffer() {
    // Empty buffer: cursor should be immediately after the prompt (column 8).
    let mut state = make_network_state();
    state.filter_input_active = true;
    state.filter_input_buffer = String::new();

    let buf = render_monitor(&state, true, 80, 24);

    let cursor_x = find_cursor_x(&buf, 80)
        .expect("cursor cell (REVERSED style) should be present in the filter bar row");

    assert_eq!(
        cursor_x, 8,
        "cursor should be immediately after the prompt (column 8) for an empty buffer"
    );
}

// ── Small terminal / "too small" message tests ────────────────────────────────

#[test]
fn test_network_monitor_very_small_terminal_shows_too_small_message() {
    // Height < MIN_USABLE_HEIGHT (3) — should show "too small" message, not crash
    let state = make_network_state_with_entries(3);
    let buf = render_monitor(&state, true, 20, 3);
    // The usable area after subtracting the footer row is height-1 = 2, which
    // is < MIN_USABLE_HEIGHT (3), so the "too small" message should appear.
    assert!(
        buf_contains(&buf, 20, 3, "small") || buf_contains(&buf, 20, 3, "Terminal"),
        "Very small terminal should show 'too small' message"
    );
}

#[test]
fn test_network_monitor_narrow_width_shows_too_small_message() {
    // Width < MIN_USABLE_WIDTH (20) — should show "too small" message
    let state = make_network_state_with_entries(3);
    // Use width 15, height 10 — height is fine but width is too small
    let buf = render_monitor(&state, true, 15, 10);
    assert!(
        buf_contains(&buf, 15, 10, "small") || buf_contains(&buf, 15, 10, "Terminal"),
        "Narrow width terminal should show 'too small' message"
    );
}

#[test]
fn test_network_monitor_20x5_no_panic() {
    // 20x5 — one of the extreme terminal sizes from the acceptance criteria
    let state = make_network_state_with_entries(3);
    render_monitor(&state, true, 20, 5);
    // Should not panic
}

#[test]
fn test_network_monitor_40x10_no_panic() {
    // 40x10 — acceptance criteria terminal size
    let state = make_network_state_with_entries(5);
    render_monitor(&state, true, 40, 10);
    // Should not panic
}

#[test]
fn test_network_monitor_60x15_no_panic() {
    // 60x15 — acceptance criteria terminal size
    let state = make_network_state_with_entries(5);
    render_monitor(&state, true, 60, 15);
    // Should not panic
}

#[test]
fn test_network_monitor_200x50_no_panic() {
    // 200x50 — large terminal (acceptance criteria)
    let state = make_network_state_with_entries(10);
    render_monitor(&state, true, 200, 50);
    // Should not panic
}

#[test]
fn test_network_monitor_height_2_shows_too_small() {
    // height=2, usable=1 after footer reservation — below MIN_USABLE_HEIGHT
    let state = make_network_state_with_entries(3);
    // render at 60 wide so width is fine, only height is the constraint
    let buf = render_monitor(&state, true, 60, 2);
    assert!(
        buf_contains(&buf, 60, 2, "small") || buf_contains(&buf, 60, 2, "Terminal"),
        "Height-2 terminal should show 'too small' message"
    );
}

#[test]
fn test_network_monitor_state_preserved_across_sizes() {
    // Verify selected_index state is not mutated when rendering at small sizes.
    // This is a read-only render — state should be unaffected.
    let mut state = make_network_state_with_entries(5);
    state.selected_index = Some(2);
    // Render at tiny size
    render_monitor(&state, true, 15, 4);
    // State should be unchanged (render takes &NetworkState, so it cannot mutate it)
    assert_eq!(
        state.selected_index,
        Some(2),
        "selected_index should be preserved after rendering at small terminal size"
    );
}
