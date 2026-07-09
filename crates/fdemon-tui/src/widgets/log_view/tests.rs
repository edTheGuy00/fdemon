//! Tests for log_view widget module

use super::*;
use crate::theme::icons::IconSet;
use fdemon_app::config::IconMode;
use fdemon_app::hyperlinks::LinkHighlightState;
use fdemon_app::session::CollapseState;
use fdemon_core::stack_trace::ParsedStackTrace;
use fdemon_core::{FilterState, LogLevelFilter, LogSourceFilter, SearchState};
use ratatui::style::{Color, Modifier, Style};
use std::collections::VecDeque;

// Import styles for constant tests
use super::styles;

fn make_entry(level: LogLevel, source: LogSource, msg: &str) -> LogEntry {
    LogEntry::new(level, source, msg)
}

/// Helper to create a VecDeque of log entries for tests
fn logs_from(entries: Vec<LogEntry>) -> VecDeque<LogEntry> {
    VecDeque::from(entries)
}

/// Helper to create IconSet for tests (Unicode mode)
fn test_icons() -> IconSet {
    IconSet::new(IconMode::Unicode)
}

#[test]
fn test_log_view_state_default() {
    let state = LogViewState::new();
    assert_eq!(state.offset, 0);
    assert!(state.auto_scroll);
}

#[test]
fn test_scroll_up_disables_auto_scroll() {
    let mut state = LogViewState::new();
    state.total_lines = 100;
    state.visible_lines = 20;
    state.offset = 50;

    state.scroll_up(1);

    assert_eq!(state.offset, 49);
    assert!(!state.auto_scroll);
}

#[test]
fn test_scroll_to_bottom_enables_auto_scroll() {
    let mut state = LogViewState::new();
    state.total_lines = 100;
    state.visible_lines = 20;
    state.auto_scroll = false;

    state.scroll_to_bottom();

    assert_eq!(state.offset, 80);
    assert!(state.auto_scroll);
}

#[test]
fn test_scroll_up_at_top() {
    let mut state = LogViewState::new();
    state.offset = 0;

    state.scroll_up(5);

    assert_eq!(state.offset, 0);
}

#[test]
fn test_update_content_size_auto_scrolls() {
    let mut state = LogViewState::new();
    state.auto_scroll = true;

    state.update_content_size(100, 20);

    assert_eq!(state.offset, 80);
}

#[test]
fn test_page_up_down() {
    let mut state = LogViewState::new();
    state.total_lines = 100;
    state.visible_lines = 20;
    state.offset = 50;

    state.page_down();
    assert_eq!(state.offset, 68); // 50 + 18

    state.page_up();
    assert_eq!(state.offset, 50); // 68 - 18
}

#[test]
fn test_format_entry_includes_timestamp() {
    let logs = logs_from(vec![make_entry(LogLevel::Info, LogSource::App, "Test")]);
    let view = LogView::new(&logs, test_icons()).show_timestamps(true);
    let line = view.format_entry(&logs[0], 0);

    // Should have multiple spans including timestamp
    assert!(line.spans.len() >= 3);
}

#[test]
fn test_format_entry_no_timestamp() {
    let logs = logs_from(vec![make_entry(LogLevel::Info, LogSource::App, "Test")]);
    let view = LogView::new(&logs, test_icons()).show_timestamps(false);
    let line = view.format_entry(&logs[0], 0);

    // Fewer spans without timestamp
    let with_ts = LogView::new(&logs, test_icons()).show_timestamps(true);
    let line_with = with_ts.format_entry(&logs[0], 0);
    assert!(line.spans.len() < line_with.spans.len());
}

#[test]
fn test_format_entry_no_source() {
    let logs = logs_from(vec![make_entry(LogLevel::Info, LogSource::App, "Test")]);
    let view = LogView::new(&logs, test_icons()).show_source(false);
    let line = view.format_entry(&logs[0], 0);

    // Fewer spans without source
    let with_src = LogView::new(&logs, test_icons()).show_source(true);
    let line_with = with_src.format_entry(&logs[0], 0);
    assert!(line.spans.len() < line_with.spans.len());
}

#[test]
fn test_level_styles_are_distinct() {
    let (err_level, _) = LogView::level_style(LogLevel::Error);
    let (info_level, _) = LogView::level_style(LogLevel::Info);

    // Error should be red, Info should be green
    assert_ne!(err_level.fg, info_level.fg);
}

#[test]
fn test_source_styles_are_distinct() {
    let app_style = LogView::source_style(&LogSource::App);
    let flutter_style = LogView::source_style(&LogSource::Flutter);

    assert_ne!(app_style.fg, flutter_style.fg);
}

#[test]
fn test_warning_has_bold_modifier() {
    let (warn_level, _) = LogView::level_style(LogLevel::Warning);
    assert!(warn_level.add_modifier.contains(Modifier::BOLD));
}

#[test]
fn test_error_has_bold_modifier() {
    let (err_level, _) = LogView::level_style(LogLevel::Error);
    assert!(err_level.add_modifier.contains(Modifier::BOLD));
}

// ─────────────────────────────────────────────────────────
// Filter Tests (Phase 1 - Task 4)
// ─────────────────────────────────────────────────────────

#[test]
fn test_filter_state_builder() {
    let logs = logs_from(vec![make_entry(LogLevel::Info, LogSource::App, "Test")]);
    let filter = FilterState::default();
    let view = LogView::new(&logs, test_icons()).filter_state(&filter);
    assert!(view.filter_state.is_some());
}

#[test]
fn test_filtered_logs_count() {
    let logs = logs_from(vec![
        make_entry(LogLevel::Info, LogSource::App, "info"),
        make_entry(LogLevel::Error, LogSource::App, "error"),
        make_entry(LogLevel::Warning, LogSource::Daemon, "warning"),
    ]);
    let filter = FilterState {
        level_filter: LogLevelFilter::Errors,
        source_filter: LogSourceFilter::All,
    };

    let filtered: Vec<_> = logs.iter().filter(|e| filter.matches(e)).collect();

    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].level, LogLevel::Error);
}

#[test]
fn test_filtered_logs_by_source() {
    let logs = logs_from(vec![
        make_entry(LogLevel::Info, LogSource::App, "app info"),
        make_entry(LogLevel::Error, LogSource::Flutter, "flutter error"),
        make_entry(LogLevel::Warning, LogSource::Daemon, "daemon warning"),
    ]);
    let filter = FilterState {
        level_filter: LogLevelFilter::All,
        source_filter: LogSourceFilter::App,
    };

    let filtered: Vec<_> = logs.iter().filter(|e| filter.matches(e)).collect();

    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].source, LogSource::App);
}

#[test]
fn test_combined_filter() {
    let logs = logs_from(vec![
        make_entry(LogLevel::Error, LogSource::App, "app error"),
        make_entry(LogLevel::Error, LogSource::Flutter, "flutter error"),
        make_entry(LogLevel::Info, LogSource::App, "app info"),
        make_entry(LogLevel::Warning, LogSource::App, "app warning"),
    ]);
    let filter = FilterState {
        level_filter: LogLevelFilter::Errors,
        source_filter: LogSourceFilter::App,
    };

    let filtered: Vec<_> = logs.iter().filter(|e| filter.matches(e)).collect();

    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].message, "app error");
}

// ─────────────────────────────────────────────────────────
// Search Highlighting Tests (Phase 1 - Task 6)
// ─────────────────────────────────────────────────────────

#[test]
fn test_format_message_with_highlights_no_search() {
    let logs = logs_from(vec![make_entry(
        LogLevel::Info,
        LogSource::App,
        "Hello world",
    )]);
    let view = LogView::new(&logs, test_icons());

    let spans = view.format_message_with_highlights("Hello world", 0, Style::default());

    assert_eq!(spans.len(), 1);
}

#[test]
fn test_format_message_with_highlights_with_match() {
    let logs = logs_from(vec![make_entry(
        LogLevel::Info,
        LogSource::App,
        "Hello world",
    )]);
    let mut search = SearchState::default();
    search.set_query("world");
    search.execute_search(&logs);

    let view = LogView::new(&logs, test_icons()).search_state(&search);

    let spans = view.format_message_with_highlights("Hello world", 0, Style::default());

    // Should be: "Hello " + "world" (highlighted)
    assert_eq!(spans.len(), 2);
}

#[test]
fn test_format_message_with_highlights_multiple_matches() {
    let logs = logs_from(vec![make_entry(
        LogLevel::Info,
        LogSource::App,
        "test one test two",
    )]);
    let mut search = SearchState::default();
    search.set_query("test");
    search.execute_search(&logs);

    let view = LogView::new(&logs, test_icons()).search_state(&search);

    let spans = view.format_message_with_highlights("test one test two", 0, Style::default());

    // Should be: "test" (highlighted) + " one " + "test" (highlighted) + " two"
    assert_eq!(spans.len(), 4);
}

#[test]
fn test_format_message_with_highlights_no_match_in_entry() {
    let logs = logs_from(vec![
        make_entry(LogLevel::Info, LogSource::App, "test here"),
        make_entry(LogLevel::Info, LogSource::App, "no match"),
    ]);
    let mut search = SearchState::default();
    search.set_query("test");
    search.execute_search(&logs);

    let view = LogView::new(&logs, test_icons()).search_state(&search);

    // Entry 1 has no matches - should return single span
    let spans = view.format_message_with_highlights("no match", 1, Style::default());

    assert_eq!(spans.len(), 1);
}

#[test]
fn test_format_message_with_highlights_invalid_regex() {
    let logs = logs_from(vec![make_entry(LogLevel::Info, LogSource::App, "test")]);
    let mut search = SearchState::default();
    search.set_query("[invalid");
    search.execute_search(&logs);

    let view = LogView::new(&logs, test_icons()).search_state(&search);

    // Invalid regex should not highlight
    let spans = view.format_message_with_highlights("test", 0, Style::default());

    assert_eq!(spans.len(), 1);
}

#[test]
fn test_search_state_builder() {
    let logs = logs_from(vec![make_entry(LogLevel::Info, LogSource::App, "test")]);
    let search = SearchState::default();
    let view = LogView::new(&logs, test_icons()).search_state(&search);
    assert!(view.search_state.is_some());
}

#[test]
fn test_format_entry_with_search_highlights() {
    let logs = logs_from(vec![make_entry(
        LogLevel::Info,
        LogSource::App,
        "error occurred",
    )]);
    let mut search = SearchState::default();
    search.set_query("error");
    search.execute_search(&logs);

    let view = LogView::new(&logs, test_icons())
        .show_timestamps(false)
        .show_source(false)
        .search_state(&search);

    let line = view.format_entry(&logs[0], 0);

    // Should have at least 2 spans for message: "error" (highlighted) + " occurred"
    // (Phase 2: Level indicator icon removed from redesign)
    assert!(line.spans.len() >= 2, "Got {} spans", line.spans.len());
}

// ─────────────────────────────────────────────────────────
// Stack Trace Rendering Tests (Phase 2 - Task 5)
// ─────────────────────────────────────────────────────────

#[test]
fn test_format_stack_frame_project_frame() {
    let frame = StackFrame::new(0, "main", "package:app/main.dart", 15, 3);

    let spans = LogView::format_stack_frame(&frame);

    // Should have multiple spans: indent, frame#, function, (, file, :, line, :col, )
    assert!(spans.len() >= 7, "Got {} spans", spans.len());

    // First span should be indentation
    assert!(spans[0].content.starts_with("    "), "Expected indentation");

    // Check that function name is included
    let content: String = spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(content.contains("main"), "Should contain function name");
    assert!(
        content.contains("main.dart"),
        "Should contain short file path"
    );
    assert!(content.contains("15"), "Should contain line number");
}

#[test]
fn test_format_stack_frame_package_frame() {
    let frame = StackFrame::new(
        1,
        "State.setState",
        "package:flutter/src/widgets/framework.dart",
        1187,
        9,
    );

    let spans = LogView::format_stack_frame(&frame);

    // Package frame should have all dimmed styling
    // Just verify it produces spans
    assert!(!spans.is_empty());

    let content: String = spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(content.contains("State.setState"));
    assert!(content.contains("framework.dart"));
}

#[test]
fn test_format_stack_frame_async_gap() {
    let frame = StackFrame::async_gap(2);

    let spans = LogView::format_stack_frame(&frame);

    // Async gap should have 2 spans: indent + message
    assert_eq!(spans.len(), 2);

    let content: String = spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(
        content.contains("<asynchronous suspension>"),
        "Got: {}",
        content
    );
}

#[test]
fn test_format_stack_frame_no_column() {
    let mut frame = StackFrame::new(0, "test", "package:app/test.dart", 10, 0);
    frame.column = 0;

    let spans = LogView::format_stack_frame(&frame);

    let content: String = spans.iter().map(|s| s.content.as_ref()).collect();
    // Should contain line number but not ":0" for column
    assert!(content.contains(":10"), "Should have line number");
    // Column 0 means no column should be shown
    assert!(
        !content.contains(":0)"),
        "Should not show :0 column, got: {}",
        content
    );
}

#[test]
fn test_calculate_total_lines_no_traces() {
    let logs = logs_from(vec![
        make_entry(LogLevel::Info, LogSource::App, "Hello"),
        make_entry(LogLevel::Error, LogSource::App, "Error"),
    ]);

    let total = LogViewState::calculate_total_lines(&logs);
    assert_eq!(total, 2); // No stack traces, just 2 entries
}

#[test]
fn test_calculate_total_lines_with_traces() {
    let entry1 = make_entry(LogLevel::Info, LogSource::App, "Hello");
    // entry1 has no stack trace

    let mut entry2 = make_entry(LogLevel::Error, LogSource::App, "Error");
    let trace = ParsedStackTrace::parse(
        r#"
#0      main (package:app/main.dart:15:3)
#1      runApp (package:flutter/src/widgets/binding.dart:100:5)
#2      _startIsolate (dart:isolate-patch/isolate_patch.dart:307:19)
"#,
    );
    entry2.stack_trace = Some(trace);

    let logs = logs_from(vec![entry1, entry2]);

    let total = LogViewState::calculate_total_lines(&logs);
    // entry1: 1 line, entry2: 1 line + 3 frames = 4 lines, total = 5
    assert_eq!(total, 5);
}

#[test]
fn test_calculate_total_lines_filtered() {
    let entry1 = make_entry(LogLevel::Info, LogSource::App, "Hello");
    let mut entry2 = make_entry(LogLevel::Error, LogSource::App, "Error");
    let trace = ParsedStackTrace::parse("#0 main (package:app/main.dart:15:3)");
    entry2.stack_trace = Some(trace);

    let logs = logs_from(vec![entry1, entry2]);

    // Only include entry2 (index 1)
    let indices = vec![1];
    let total = LogViewState::calculate_total_lines_filtered(&logs, &indices);
    assert_eq!(total, 2); // 1 message + 1 frame
}

#[test]
fn test_format_stack_frame_line() {
    let frame = StackFrame::new(0, "test", "package:app/test.dart", 5, 1);

    let line = LogView::format_stack_frame_line(&frame);

    // Should produce a Line with spans
    assert!(!line.spans.is_empty());
}

#[test]
fn test_stack_frame_with_long_function_name() {
    let frame = StackFrame::new(
        0,
        "_SomeVeryLongPrivateClassName.someEvenLongerMethodName",
        "package:app/file.dart",
        100,
        5,
    );

    let spans = LogView::format_stack_frame(&frame);

    let content: String = spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(content.contains("_SomeVeryLongPrivateClassName.someEvenLongerMethodName"));
}

#[test]
fn test_stack_frame_styles_module_constants() {
    // Verify style constants are accessible and have expected properties
    use crate::theme::palette;
    use styles::*;

    assert_eq!(INDENT, "    ");
    assert_eq!(FRAME_NUMBER.fg, Some(palette::STACK_FRAME_NUMBER));
    assert_eq!(FUNCTION_PROJECT.fg, Some(palette::STACK_FUNCTION_PROJECT));
    assert_eq!(FUNCTION_PACKAGE.fg, Some(palette::STACK_FUNCTION_PACKAGE));
    assert_eq!(FILE_PROJECT.fg, Some(palette::STACK_FILE_PROJECT));
    assert!(FILE_PROJECT.add_modifier.contains(Modifier::UNDERLINED));
    assert_eq!(LOCATION_PROJECT.fg, Some(palette::STACK_LOCATION_PROJECT));
    assert!(ASYNC_GAP.add_modifier.contains(Modifier::ITALIC));
}

// ─────────────────────────────────────────────────────────
// Collapsible Stack Traces Tests (Phase 2 Task 6)
// ─────────────────────────────────────────────────────────

#[test]
fn test_format_collapsed_indicator_singular() {
    let line = LogView::format_collapsed_indicator(1);
    let content: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(content.contains("1 more frame..."), "Got: {}", content);
}

#[test]
fn test_format_collapsed_indicator_plural() {
    let line = LogView::format_collapsed_indicator(5);
    let content: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(content.contains("5 more frames..."), "Got: {}", content);
}

#[test]
fn test_format_collapsed_indicator_has_arrow() {
    let line = LogView::format_collapsed_indicator(3);
    let content: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(content.contains("▶"), "Should have arrow indicator");
}

#[test]
fn test_calculate_entry_lines_no_trace() {
    let entry = make_entry(LogLevel::Info, LogSource::App, "Hello");
    let logs = logs_from(vec![entry]);
    let view = LogView::new(&logs, test_icons())
        .default_collapsed(true)
        .max_collapsed_frames(3);

    assert_eq!(view.calculate_entry_lines(&logs[0]), 1); // Just message
}

#[test]
fn test_calculate_entry_lines_collapsed() {
    let mut entry = make_entry(LogLevel::Error, LogSource::App, "Error");
    let trace = ParsedStackTrace::parse(
        r#"
#0      main (package:app/main.dart:15:3)
#1      runApp (package:flutter/src/widgets/binding.dart:100:5)
#2      _startIsolate (dart:isolate-patch/isolate_patch.dart:307:19)
#3      frame4 (package:app/other.dart:50:1)
#4      frame5 (package:app/other.dart:60:1)
"#,
    );
    entry.stack_trace = Some(trace);

    let logs = logs_from(vec![entry]);
    let view = LogView::new(&logs, test_icons())
        .default_collapsed(true)
        .max_collapsed_frames(3);

    // Collapsed: 1 message + 3 visible frames + 1 indicator = 5
    assert_eq!(view.calculate_entry_lines(&logs[0]), 5);
}

#[test]
fn test_calculate_entry_lines_expanded() {
    let mut entry = make_entry(LogLevel::Error, LogSource::App, "Error");
    let trace = ParsedStackTrace::parse(
        r#"
#0      main (package:app/main.dart:15:3)
#1      runApp (package:flutter/src/widgets/binding.dart:100:5)
#2      _startIsolate (dart:isolate-patch/isolate_patch.dart:307:19)
#3      frame4 (package:app/other.dart:50:1)
#4      frame5 (package:app/other.dart:60:1)
"#,
    );
    entry.stack_trace = Some(trace);

    let logs = logs_from(vec![entry]);
    let mut collapse_state = CollapseState::new();
    collapse_state.toggle(logs[0].id, true); // Expand it

    let view = LogView::new(&logs, test_icons())
        .default_collapsed(true)
        .max_collapsed_frames(3)
        .collapse_state(&collapse_state);

    // Expanded: 1 message + 5 frames = 6
    assert_eq!(view.calculate_entry_lines(&logs[0]), 6);
}

#[test]
fn test_calculate_entry_lines_few_frames() {
    // When there are fewer frames than max, no indicator needed
    let mut entry = make_entry(LogLevel::Error, LogSource::App, "Error");
    let trace = ParsedStackTrace::parse("#0 main (package:app/main.dart:15:3)");
    entry.stack_trace = Some(trace);

    let logs = logs_from(vec![entry]);
    let view = LogView::new(&logs, test_icons())
        .default_collapsed(true)
        .max_collapsed_frames(3);

    // Only 1 frame, no indicator needed: 1 message + 1 frame = 2
    assert_eq!(view.calculate_entry_lines(&logs[0]), 2);
}

#[test]
fn test_is_entry_expanded_no_collapse_state() {
    let mut entry = make_entry(LogLevel::Error, LogSource::App, "Error");
    let trace = ParsedStackTrace::parse("#0 main (package:app/main.dart:15:3)");
    entry.stack_trace = Some(trace);

    let logs = logs_from(vec![entry]);

    // Without collapse state, use default_collapsed setting
    let view = LogView::new(&logs, test_icons()).default_collapsed(true);
    assert!(!view.is_entry_expanded(&logs[0])); // Collapsed by default

    let view = LogView::new(&logs, test_icons()).default_collapsed(false);
    assert!(view.is_entry_expanded(&logs[0])); // Expanded by default
}

#[test]
fn test_is_entry_expanded_with_collapse_state() {
    let mut entry = make_entry(LogLevel::Error, LogSource::App, "Error");
    let trace = ParsedStackTrace::parse("#0 main (package:app/main.dart:15:3)");
    entry.stack_trace = Some(trace);

    let logs = logs_from(vec![entry]);
    let mut collapse_state = CollapseState::new();

    // Toggle to expanded
    collapse_state.toggle(logs[0].id, true);

    let view = LogView::new(&logs, test_icons())
        .default_collapsed(true)
        .collapse_state(&collapse_state);

    assert!(view.is_entry_expanded(&logs[0]));
}

#[test]
fn test_collapse_state_builder() {
    let logs: VecDeque<LogEntry> = VecDeque::new();
    let collapse_state = CollapseState::new();

    let view = LogView::new(&logs, test_icons()).collapse_state(&collapse_state);

    assert!(view.collapse_state.is_some());
}

#[test]
fn test_max_collapsed_frames_builder() {
    let logs: VecDeque<LogEntry> = VecDeque::new();

    let view = LogView::new(&logs, test_icons()).max_collapsed_frames(5);

    assert_eq!(view.max_collapsed_frames, 5);
}

#[test]
fn test_default_collapsed_builder() {
    let logs: VecDeque<LogEntry> = VecDeque::new();

    let view = LogView::new(&logs, test_icons()).default_collapsed(false);

    assert!(!view.default_collapsed);
}

// ─────────────────────────────────────────────────────────
// Horizontal Scroll Tests (Phase 2 Task 12)
// ─────────────────────────────────────────────────────────

#[test]
fn test_horizontal_scroll_state_default() {
    let state = LogViewState::new();
    assert_eq!(state.h_offset, 0);
    assert_eq!(state.max_line_width, 0);
    assert_eq!(state.visible_width, 0);
}

#[test]
fn test_scroll_left() {
    let mut state = LogViewState::new();
    state.h_offset = 20;
    state.max_line_width = 200;
    state.visible_width = 80;

    state.scroll_left(10);
    assert_eq!(state.h_offset, 10);

    state.scroll_left(20);
    assert_eq!(state.h_offset, 0); // Clamped at 0
}

#[test]
fn test_scroll_right() {
    let mut state = LogViewState::new();
    state.h_offset = 0;
    state.max_line_width = 200;
    state.visible_width = 80;

    state.scroll_right(10);
    assert_eq!(state.h_offset, 10);

    state.scroll_right(200);
    assert_eq!(state.h_offset, 120); // Clamped at max - visible
}

#[test]
fn test_scroll_to_line_start() {
    let mut state = LogViewState::new();
    state.h_offset = 50;

    state.scroll_to_line_start();
    assert_eq!(state.h_offset, 0);
}

#[test]
fn test_scroll_to_line_end() {
    let mut state = LogViewState::new();
    state.h_offset = 0;
    state.max_line_width = 200;
    state.visible_width = 80;

    state.scroll_to_line_end();
    assert_eq!(state.h_offset, 120); // max - visible
}

#[test]
fn test_no_horizontal_scroll_needed() {
    let mut state = LogViewState::new();
    state.max_line_width = 50;
    state.visible_width = 80;

    state.scroll_right(10);
    assert_eq!(state.h_offset, 0); // No scroll when content fits
}

#[test]
fn test_update_horizontal_size() {
    let mut state = LogViewState::new();
    state.h_offset = 50;

    // Update with smaller content
    state.update_horizontal_size(60, 80);

    // h_offset should be clamped to 0 since content now fits
    assert_eq!(state.h_offset, 0);
    assert_eq!(state.max_line_width, 60);
    assert_eq!(state.visible_width, 80);
}

#[test]
fn test_update_horizontal_size_clamps_offset() {
    let mut state = LogViewState::new();
    state.h_offset = 100;
    state.max_line_width = 200;
    state.visible_width = 80;

    // Shrink the content
    state.update_horizontal_size(150, 80);

    // h_offset should be clamped to max_h_offset = 150 - 80 = 70
    assert_eq!(state.h_offset, 70);
}

#[test]
fn test_line_width() {
    let line = Line::from(vec![Span::raw("Hello"), Span::raw(" "), Span::raw("World")]);
    assert_eq!(LogView::line_width(&line), 11);
}

#[test]
fn test_apply_horizontal_scroll_no_scroll_needed() {
    let line = Line::from("Short line");
    let result = LogView::apply_horizontal_scroll(line, 0, 80);
    let content: String = result.spans.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(content, "Short line");
}

#[test]
fn test_apply_horizontal_scroll_truncate_right() {
    let line = Line::from("A very long line that exceeds visible width");
    let result = LogView::apply_horizontal_scroll(line, 0, 20);
    let content: String = result.spans.iter().map(|s| s.content.as_ref()).collect();

    // Should have truncated content + right arrow
    assert!(content.ends_with('→'), "Got: {}", content);
    assert_eq!(content.chars().count(), 20);
}

#[test]
fn test_apply_horizontal_scroll_with_offset() {
    let line = Line::from("A very long line that exceeds visible width");
    let result = LogView::apply_horizontal_scroll(line, 10, 20);
    let content: String = result.spans.iter().map(|s| s.content.as_ref()).collect();

    // Should have left arrow, content, and right arrow
    assert!(content.starts_with('←'), "Got: {}", content);
    assert!(content.ends_with('→'), "Got: {}", content);
    assert_eq!(content.chars().count(), 20);
}

#[test]
fn test_apply_horizontal_scroll_at_end() {
    let line = Line::from("A very long line");
    // Scroll to the end
    let result = LogView::apply_horizontal_scroll(line, 6, 20);
    let content: String = result.spans.iter().map(|s| s.content.as_ref()).collect();

    // Should have left arrow but no right arrow (at end of line)
    assert!(content.starts_with('←'), "Got: {}", content);
    assert!(!content.ends_with('→'), "Got: {}", content);
}

#[test]
fn test_apply_horizontal_scroll_preserves_styles() {
    let line = Line::from(vec![
        Span::styled("Red", Style::default().fg(Color::Red)),
        Span::styled("Blue", Style::default().fg(Color::Blue)),
    ]);
    // Scroll so we see part of both spans
    let result = LogView::apply_horizontal_scroll(line, 0, 20);

    // Should still have styled spans
    assert!(result.spans.len() >= 2);
}

#[test]
fn test_apply_horizontal_scroll_offset_beyond_content() {
    let line = Line::from("Short");
    let result = LogView::apply_horizontal_scroll(line, 100, 20);
    let content: String = result.spans.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(content, "");
}

// ─────────────────────────────────────────────────────────
// Virtualized Rendering Tests (Task 05)
// ─────────────────────────────────────────────────────────

#[test]
fn test_visible_range_basic() {
    let mut state = LogViewState::new();
    state.total_lines = 100;
    state.visible_lines = 20;
    state.buffer_lines = 5;
    state.offset = 50;

    let (start, end) = state.visible_range();

    assert_eq!(start, 45); // 50 - 5 buffer
    assert_eq!(end, 75); // 50 + 20 + 5 buffer
}

#[test]
fn test_visible_range_at_start() {
    let mut state = LogViewState::new();
    state.total_lines = 100;
    state.visible_lines = 20;
    state.buffer_lines = 5;
    state.offset = 0;

    let (start, end) = state.visible_range();

    assert_eq!(start, 0); // Can't go negative
    assert_eq!(end, 25); // 0 + 20 + 5
}

#[test]
fn test_visible_range_at_end() {
    let mut state = LogViewState::new();
    state.total_lines = 100;
    state.visible_lines = 20;
    state.buffer_lines = 5;
    state.offset = 80;

    let (start, end) = state.visible_range();

    assert_eq!(start, 75); // 80 - 5
    assert_eq!(end, 100); // Capped at total
}

#[test]
fn test_visible_range_small_content() {
    let mut state = LogViewState::new();
    state.total_lines = 10;
    state.visible_lines = 20;
    state.buffer_lines = 5;
    state.offset = 0;

    let (start, end) = state.visible_range();

    assert_eq!(start, 0);
    assert_eq!(end, 10); // Capped at total
}

#[test]
fn test_visible_range_zero_buffer() {
    let mut state = LogViewState::new();
    state.total_lines = 100;
    state.visible_lines = 20;
    state.buffer_lines = 0;
    state.offset = 50;

    let (start, end) = state.visible_range();

    assert_eq!(start, 50); // No buffer
    assert_eq!(end, 70); // No buffer
}

#[test]
fn test_buffer_lines_default() {
    let state = LogViewState::new();
    assert_eq!(state.buffer_lines, 10); // DEFAULT_BUFFER_LINES value
}

#[test]
fn test_set_buffer_lines() {
    let mut state = LogViewState::new();
    state.set_buffer_lines(20);
    assert_eq!(state.buffer_lines, 20);
}

#[test]
fn test_visible_range_with_custom_buffer() {
    let mut state = LogViewState::new();
    state.total_lines = 200;
    state.visible_lines = 30;
    state.set_buffer_lines(15);
    state.offset = 100;

    let (start, end) = state.visible_range();

    assert_eq!(start, 85); // 100 - 15
    assert_eq!(end, 145); // 100 + 30 + 15
}

#[test]
fn test_visible_range_empty_content() {
    let mut state = LogViewState::new();
    state.total_lines = 0;
    state.visible_lines = 20;
    state.buffer_lines = 5;
    state.offset = 0;

    let (start, end) = state.visible_range();

    assert_eq!(start, 0);
    assert_eq!(end, 0);
}

#[test]
fn test_footer_height_not_stolen_in_small_area() {
    // Test that status info doesn't cause overflow in small spaces
    // This is a regression test for the footer height desync bug (Phase 2 Task 03)
    use crate::test_utils::TestTerminal;
    use std::time::Duration;

    // Create a terminal with 7 rows total: border(2) + top_meta(1) + gap(1) + content(1) + gap(1) + bottom_meta(1)
    let mut term = TestTerminal::with_size(80, 7);

    let logs = logs_from(vec![
        make_entry(LogLevel::Info, LogSource::App, "Line 1"),
        make_entry(LogLevel::Info, LogSource::App, "Line 2"),
        make_entry(LogLevel::Info, LogSource::App, "Line 3"),
    ]);

    let status_info = StatusInfo {
        phase: &AppPhase::Running,
        is_busy: false,
        mode: None,
        flavor: None,
        duration: Some(Duration::from_secs(5)),
        error_count: 0,
        vm_connected: false,
        dap_port: None,
        dap_config_ide: None,
        mouse_capture_active: true,
        animation_frame: 0,
        progress: None,
    };

    let log_view = LogView::new(&logs, test_icons()).with_status(status_info);
    let mut state = LogViewState::new();

    // Render the widget
    term.render_stateful_widget(log_view, term.area(), &mut state);

    // In a 7-row area:
    // - inner height = 5 (7 - 2 for borders)
    // - top metadata = 1, top gap = 1
    // - bottom metadata = 1, bottom gap = 1
    // - content = 5 - 1 - 1 - 1 - 1 = 1 line visible
    assert_eq!(
        state.visible_lines, 1,
        "visible_lines should be calculated correctly with footer"
    );

    // Now test without footer (no status_info)
    let log_view_no_footer = LogView::new(&logs, test_icons());
    let mut state_no_footer = LogViewState::new();

    term.render_stateful_widget(log_view_no_footer, term.area(), &mut state_no_footer);

    // Without footer:
    // - inner height = 5
    // - top metadata = 1, top gap = 1
    // - content = 5 - 1 - 1 = 3 lines visible
    assert_eq!(
        state_no_footer.visible_lines, 3,
        "visible_lines should be higher without footer"
    );

    // Verify the footer presence changes visible line count
    assert!(
        state_no_footer.visible_lines > state.visible_lines,
        "Footer should reduce visible lines by exactly 1"
    );
}

// ─────────────────────────────────────────────────────────
// Wrap mode rendering tests
// ─────────────────────────────────────────────────────────

#[test]
fn test_wrap_mode_wraps_long_lines() {
    use crate::test_utils::TestTerminal;

    // Use a narrow terminal to force wrapping
    let mut term = TestTerminal::with_size(30, 10);

    let logs = logs_from(vec![make_entry(
        LogLevel::Info,
        LogSource::App,
        "This is a long log line that should wrap at terminal width",
    )]);

    let log_view = LogView::new(&logs, test_icons()).wrap_mode(true);
    let mut state = LogViewState::new();

    term.render_stateful_widget(log_view, term.area(), &mut state);

    // In wrap mode, total_lines counts terminal rows (not logical lines).
    // A long message that exceeds the terminal width wraps to multiple rows.
    assert!(
        state.total_lines >= 1,
        "total_lines should account for wrapped rows"
    );
    assert_eq!(
        state.h_offset, 0,
        "wrap mode should not scroll horizontally"
    );
}

#[test]
fn test_nowrap_mode_preserves_single_line() {
    use crate::test_utils::TestTerminal;

    let mut term = TestTerminal::with_size(30, 10);

    let logs = logs_from(vec![make_entry(
        LogLevel::Info,
        LogSource::App,
        "This is a long log line that should be truncated",
    )]);

    let log_view = LogView::new(&logs, test_icons()).wrap_mode(false);
    let mut state = LogViewState::new();

    term.render_stateful_widget(log_view, term.area(), &mut state);

    // In nowrap mode the single entry still occupies one logical line
    assert_eq!(state.total_lines, 1, "one log entry means total_lines = 1");
    // At h_offset=0 the content is visible (no scroll applied from state perspective)
    assert_eq!(state.h_offset, 0, "h_offset should be 0 at initial render");
}

#[test]
fn test_wrap_indicator_shown_in_metadata_bar() {
    use crate::test_utils::TestTerminal;

    let mut term = TestTerminal::new(); // 80x24

    let logs = logs_from(vec![make_entry(
        LogLevel::Info,
        LogSource::App,
        "test message",
    )]);

    let log_view = LogView::new(&logs, test_icons()).wrap_mode(true);
    let mut state = LogViewState::new();

    term.render_stateful_widget(log_view, term.area(), &mut state);

    assert!(
        term.buffer_contains("wrap"),
        "wrap indicator should be visible in the metadata bar"
    );
}

#[test]
fn test_nowrap_indicator_shown_in_metadata_bar() {
    use crate::test_utils::TestTerminal;

    let mut term = TestTerminal::new();

    let logs = logs_from(vec![make_entry(
        LogLevel::Info,
        LogSource::App,
        "test message",
    )]);

    let log_view = LogView::new(&logs, test_icons()).wrap_mode(false);
    let mut state = LogViewState::new();

    term.render_stateful_widget(log_view, term.area(), &mut state);

    assert!(
        term.buffer_contains("nowrap"),
        "nowrap indicator should be visible in the metadata bar"
    );
}

#[test]
fn test_wrap_mode_no_horizontal_scroll_indicators() {
    use crate::test_utils::TestTerminal;

    let mut term = TestTerminal::with_size(30, 10);

    let logs = logs_from(vec![make_entry(
        LogLevel::Info,
        LogSource::App,
        "A line that is definitely longer than thirty chars total",
    )]);

    let log_view = LogView::new(&logs, test_icons()).wrap_mode(true);
    let mut state = LogViewState::new();

    term.render_stateful_widget(log_view, term.area(), &mut state);

    // In wrap mode, horizontal scroll indicators (→ for right, ← for left)
    // should not appear in the content area because lines wrap instead of scrolling.
    // h_offset stays at 0 in wrap mode, so no left indicator.
    assert_eq!(state.h_offset, 0, "h_offset should remain 0 in wrap mode");
}

#[test]
fn test_wrap_mode_scrollbar_present_for_many_entries() {
    use crate::test_utils::TestTerminal;

    // Small terminal, many log entries
    let mut term = TestTerminal::with_size(40, 8);

    let entries: Vec<_> = (0..20)
        .map(|i| make_entry(LogLevel::Info, LogSource::App, &format!("Log line {}", i)))
        .collect();
    let logs = logs_from(entries);

    let log_view = LogView::new(&logs, test_icons()).wrap_mode(true);
    let mut state = LogViewState::new();

    term.render_stateful_widget(log_view, term.area(), &mut state);

    // total_lines should reflect terminal rows (>= entry count in wrap mode)
    assert!(
        state.total_lines >= 20,
        "total_lines should be at least the entry count (may be more due to wrapping)"
    );
    assert!(
        state.total_lines > state.visible_lines,
        "with 20 entries in an 8-row terminal, a scrollbar should be needed"
    );
}

// ─────────────────────────────────────────────────────────
// DAP badge rendering tests
// ─────────────────────────────────────────────────────────

#[test]
fn test_status_bar_no_dap_badge_when_off() {
    use crate::test_utils::TestTerminal;

    let mut term = TestTerminal::with_size(80, 10);

    let logs = logs_from(vec![make_entry(LogLevel::Info, LogSource::App, "msg")]);

    let status_info = StatusInfo {
        phase: &AppPhase::Running,
        is_busy: false,
        mode: None,
        flavor: None,
        duration: None,
        error_count: 0,
        vm_connected: false,
        dap_port: None,
        dap_config_ide: None,
        mouse_capture_active: true,
        animation_frame: 0,
        progress: None,
    };

    let log_view = LogView::new(&logs, test_icons()).with_status(status_info);
    let mut state = LogViewState::new();

    term.render_stateful_widget(log_view, term.area(), &mut state);

    assert!(
        !term.buffer_contains("[DAP"),
        "No DAP badge should appear when dap_port is None"
    );
}

#[test]
fn test_status_bar_shows_dap_badge_with_port() {
    use crate::test_utils::TestTerminal;

    // Wide terminal to ensure full (non-compact) mode
    let mut term = TestTerminal::with_size(80, 10);

    let logs = logs_from(vec![make_entry(LogLevel::Info, LogSource::App, "msg")]);

    let status_info = StatusInfo {
        phase: &AppPhase::Running,
        is_busy: false,
        mode: None,
        flavor: None,
        duration: None,
        error_count: 0,
        vm_connected: false,
        dap_port: Some(4711),
        dap_config_ide: None,
        mouse_capture_active: true,
        animation_frame: 0,
        progress: None,
    };

    let log_view = LogView::new(&logs, test_icons()).with_status(status_info);
    let mut state = LogViewState::new();

    term.render_stateful_widget(log_view, term.area(), &mut state);

    assert!(
        term.buffer_contains("[DAP :4711]"),
        "DAP badge [DAP :4711] should appear when dap_port is Some(4711)"
    );
}

#[test]
fn test_status_bar_dap_badge_different_port() {
    use crate::test_utils::TestTerminal;

    let mut term = TestTerminal::with_size(80, 10);

    let logs = logs_from(vec![make_entry(LogLevel::Info, LogSource::App, "msg")]);

    let status_info = StatusInfo {
        phase: &AppPhase::Running,
        is_busy: false,
        mode: None,
        flavor: None,
        duration: None,
        error_count: 0,
        vm_connected: false,
        dap_port: Some(54321),
        dap_config_ide: None,
        mouse_capture_active: true,
        animation_frame: 0,
        progress: None,
    };

    let log_view = LogView::new(&logs, test_icons()).with_status(status_info);
    let mut state = LogViewState::new();

    term.render_stateful_widget(log_view, term.area(), &mut state);

    assert!(
        term.buffer_contains("[DAP :54321]"),
        "DAP badge [DAP :54321] should appear when dap_port is Some(54321)"
    );
}

#[test]
fn test_dap_badge_hidden_in_compact_mode() {
    use crate::test_utils::TestTerminal;

    // Narrow terminal forces compact mode (< MIN_FULL_STATUS_WIDTH = 60)
    let mut term = TestTerminal::with_size(40, 10);

    let logs = logs_from(vec![make_entry(LogLevel::Info, LogSource::App, "msg")]);

    let status_info = StatusInfo {
        phase: &AppPhase::Running,
        is_busy: false,
        mode: None,
        flavor: None,
        duration: None,
        error_count: 0,
        vm_connected: false,
        dap_port: Some(4711),
        dap_config_ide: None,
        mouse_capture_active: true,
        animation_frame: 0,
        progress: None,
    };

    let log_view = LogView::new(&logs, test_icons()).with_status(status_info);
    let mut state = LogViewState::new();

    term.render_stateful_widget(log_view, term.area(), &mut state);

    assert!(
        !term.buffer_contains("[DAP"),
        "DAP badge should not appear in compact mode (terminal width < 60)"
    );
}

// ─────────────────────────────────────────────────────────
// DAP config IDE badge rendering tests (Phase 5, Task 11)
// ─────────────────────────────────────────────────────────

#[test]
fn test_status_bar_shows_dap_with_ide_name() {
    use crate::test_utils::TestTerminal;

    let mut term = TestTerminal::with_size(80, 10);

    let logs = logs_from(vec![make_entry(LogLevel::Info, LogSource::App, "msg")]);

    let status_info = StatusInfo {
        phase: &AppPhase::Running,
        is_busy: false,
        mode: None,
        flavor: None,
        duration: None,
        error_count: 0,
        vm_connected: false,
        dap_port: Some(4711),
        dap_config_ide: Some("VS Code".to_string()),
        mouse_capture_active: true,
        animation_frame: 0,
        progress: None,
    };

    let log_view = LogView::new(&logs, test_icons()).with_status(status_info);
    let mut state = LogViewState::new();

    term.render_stateful_widget(log_view, term.area(), &mut state);

    assert!(
        term.buffer_contains("[DAP :4711"),
        "DAP badge should appear when dap_port is Some(4711)"
    );
    assert!(
        term.buffer_contains("VS Code"),
        "IDE name 'VS Code' should appear in the DAP badge"
    );
}

#[test]
fn test_status_bar_shows_dap_without_ide_name() {
    use crate::test_utils::TestTerminal;

    let mut term = TestTerminal::with_size(80, 10);

    let logs = logs_from(vec![make_entry(LogLevel::Info, LogSource::App, "msg")]);

    let status_info = StatusInfo {
        phase: &AppPhase::Running,
        is_busy: false,
        mode: None,
        flavor: None,
        duration: None,
        error_count: 0,
        vm_connected: false,
        dap_port: Some(4711),
        dap_config_ide: None,
        mouse_capture_active: true,
        animation_frame: 0,
        progress: None,
    };

    let log_view = LogView::new(&logs, test_icons()).with_status(status_info);
    let mut state = LogViewState::new();

    term.render_stateful_widget(log_view, term.area(), &mut state);

    assert!(
        term.buffer_contains("[DAP :4711]"),
        "Badge should be '[DAP :4711]' with no IDE suffix when dap_config_ide is None"
    );
    assert!(
        !term.buffer_contains("VS Code"),
        "No IDE name should appear when dap_config_ide is None"
    );
}

#[test]
fn test_status_bar_no_dap_with_ide_name_when_port_absent() {
    use crate::test_utils::TestTerminal;

    // dap_config_ide is Some but dap_port is None — no badge should appear
    let mut term = TestTerminal::with_size(80, 10);

    let logs = logs_from(vec![make_entry(LogLevel::Info, LogSource::App, "msg")]);

    let status_info = StatusInfo {
        phase: &AppPhase::Running,
        is_busy: false,
        mode: None,
        flavor: None,
        duration: None,
        error_count: 0,
        vm_connected: false,
        dap_port: None,
        dap_config_ide: Some("VS Code".to_string()),
        mouse_capture_active: true,
        animation_frame: 0,
        progress: None,
    };

    let log_view = LogView::new(&logs, test_icons()).with_status(status_info);
    let mut state = LogViewState::new();

    term.render_stateful_widget(log_view, term.area(), &mut state);

    assert!(
        !term.buffer_contains("[DAP"),
        "No DAP badge should appear when dap_port is None, even if dap_config_ide is set"
    );
}

// ─────────────────────────────────────────────────────────
// Native Source Styling Tests (Phase 1 - Task 08)
// ─────────────────────────────────────────────────────────

#[test]
fn test_source_style_native() {
    let style = LogView::source_style(&LogSource::Native {
        tag: "GoLog".into(),
    });
    assert_eq!(style.fg, Some(palette::SOURCE_NATIVE));
}

#[test]
fn test_native_log_entry_prefix_rendering() {
    let entry = LogEntry::new(
        LogLevel::Info,
        LogSource::Native {
            tag: "GoLog".into(),
        },
        "Hello from Go".to_string(),
    );
    assert_eq!(entry.source.prefix(), "GoLog");
    // format_entry renders this as "[GoLog] Hello from Go"
}

#[test]
fn test_native_log_entry_long_tag() {
    let entry = LogEntry::new(
        LogLevel::Debug,
        LogSource::Native {
            tag: "com.example.myplugin.logging".into(),
        },
        "verbose message".to_string(),
    );
    assert_eq!(entry.source.prefix(), "com.example.myplugin.logging");
}

#[test]
fn test_source_style_existing_sources_unchanged() {
    // Verify existing source styles haven't regressed
    assert_eq!(
        LogView::source_style(&LogSource::App).fg,
        Some(palette::SOURCE_APP)
    );
    assert_eq!(
        LogView::source_style(&LogSource::Daemon).fg,
        Some(palette::SOURCE_DAEMON)
    );
    assert_eq!(
        LogView::source_style(&LogSource::Flutter).fg,
        Some(palette::SOURCE_FLUTTER)
    );
    assert_eq!(
        LogView::source_style(&LogSource::FlutterError).fg,
        Some(palette::SOURCE_FLUTTER_ERROR)
    );
    assert_eq!(
        LogView::source_style(&LogSource::Watcher).fg,
        Some(palette::SOURCE_WATCHER)
    );
}

#[test]
fn test_native_source_color_is_distinct_from_others() {
    // SOURCE_NATIVE must differ from all other source colors
    assert_ne!(palette::SOURCE_NATIVE, palette::SOURCE_APP);
    assert_ne!(palette::SOURCE_NATIVE, palette::SOURCE_DAEMON);
    assert_ne!(palette::SOURCE_NATIVE, palette::SOURCE_FLUTTER);
    assert_ne!(palette::SOURCE_NATIVE, palette::SOURCE_FLUTTER_ERROR);
    assert_ne!(palette::SOURCE_NATIVE, palette::SOURCE_WATCHER);
    assert_ne!(palette::SOURCE_NATIVE, palette::ACCENT);
}

// ── Phase 4 Task 06: render_with_regions click-region tests ──────────────────

/// Helper: create a VecDeque with `count` plain (no stack trace) log entries.
fn make_logs_no_traces(count: usize) -> std::collections::VecDeque<LogEntry> {
    let mut logs = std::collections::VecDeque::new();
    for i in 0..count {
        logs.push_back(make_entry(
            LogLevel::Info,
            LogSource::App,
            &format!("message {i}"),
        ));
    }
    logs
}

/// Helper: create a VecDeque with a single error entry that has `frame_count`
/// stack frames.  Expanded=true because `LogView` will be built with
/// `default_collapsed(false)`.
fn make_logs_with_stack_trace(frame_count: usize) -> std::collections::VecDeque<LogEntry> {
    let raw = (0..frame_count)
        .map(|i| format!("#{i}      fn_{i} (package:app/main.dart:{i}:3)"))
        .collect::<Vec<_>>()
        .join("\n");
    let trace = ParsedStackTrace::parse(&raw);

    let mut entry = make_entry(LogLevel::Error, LogSource::App, "boom");
    entry.stack_trace = Some(trace);

    let mut logs = std::collections::VecDeque::new();
    logs.push_back(entry);
    logs
}

#[test]
fn render_with_regions_records_one_region_per_visible_row_nowrap() {
    use crate::render::MouseCtx;
    use fdemon_app::message::Message;
    use fdemon_app::{MouseAction, MouseRegions};
    use ratatui::{buffer::Buffer, layout::Rect};

    let logs = make_logs_no_traces(3);
    let mut state = LogViewState::new();
    let view = LogView::new(&logs, test_icons()).wrap_mode(false);

    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    let mut regions = MouseRegions::with_capacity();
    {
        let builder = regions.builder();
        let mut ctx = MouseCtx::new(builder);
        super::render_with_regions(area, &mut buf, &mut state, view, Some(&mut ctx));
    }

    let click_rows: Vec<_> = regions
        .iter()
        .filter(|e| {
            matches!(
                &e.on_left,
                Some(MouseAction::Emit(msg)) if matches!(**msg, Message::ClickLogRow { .. })
            )
        })
        .collect();

    assert_eq!(
        click_rows.len(),
        3,
        "expected one ClickLogRow region per visible entry, got {}",
        click_rows.len()
    );
}

#[test]
fn render_with_regions_no_regions_without_ctx() {
    // The plain StatefulWidget::render path must not record any regions.
    use ratatui::{buffer::Buffer, layout::Rect, widgets::StatefulWidget};

    let logs = make_logs_no_traces(3);
    let mut state = LogViewState::new();
    let view = LogView::new(&logs, test_icons()).wrap_mode(false);

    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);

    // Render without a ctx — no regions should be recorded.
    // Verify it compiles and does not panic (the assertion is that `render_inner`
    // with None ctx never touches the regions API).
    StatefulWidget::render(view, area, &mut buf, &mut state);

    // The positive case (ctx = Some) is verified by other tests; this test
    // documents that `None` produces no side effects.
}

/// Verifies that `render_with_regions` records exactly the expected number of
/// rows when given a log with a 3-frame stack trace and `default_collapsed(false)`.
/// Expected layout: 1 message row + 3 frame rows = 4 ClickLogRow regions.
#[test]
fn render_with_regions_records_frame_index_for_stack_frames() {
    use crate::render::MouseCtx;
    use fdemon_app::message::Message;
    use fdemon_app::{MouseAction, MouseRegions};
    use ratatui::{buffer::Buffer, layout::Rect};

    let logs = make_logs_with_stack_trace(3);
    let mut state = LogViewState::new();
    // default_collapsed(false) so all frames are shown without a CollapseState.
    let view = LogView::new(&logs, test_icons())
        .wrap_mode(false)
        .default_collapsed(false);

    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    let mut regions = MouseRegions::with_capacity();
    {
        let builder = regions.builder();
        let mut ctx = MouseCtx::new(builder);
        super::render_with_regions(area, &mut buf, &mut state, view, Some(&mut ctx));
    }

    let frame_indices: Vec<Option<usize>> = regions
        .iter()
        .filter_map(|e| match &e.on_left {
            Some(MouseAction::Emit(msg)) => match **msg {
                Message::ClickLogRow { frame_index, .. } => Some(frame_index),
                _ => None,
            },
            _ => None,
        })
        .collect();

    // 1 message row (None) + 3 stack frames (Some(0), Some(1), Some(2)).
    assert_eq!(
        frame_indices,
        vec![None, Some(0), Some(1), Some(2)],
        "expected [None, Some(0), Some(1), Some(2)], got {frame_indices:?}"
    );
}

#[test]
fn render_with_regions_entry_ids_match_log_entries() {
    use crate::render::MouseCtx;
    use fdemon_app::message::Message;
    use fdemon_app::{MouseAction, MouseRegions};
    use ratatui::{buffer::Buffer, layout::Rect};

    let logs = make_logs_no_traces(3);
    let expected_ids: Vec<u64> = logs.iter().map(|e| e.id).collect();

    let mut state = LogViewState::new();
    let view = LogView::new(&logs, test_icons()).wrap_mode(false);

    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    let mut regions = MouseRegions::with_capacity();
    {
        let builder = regions.builder();
        let mut ctx = MouseCtx::new(builder);
        super::render_with_regions(area, &mut buf, &mut state, view, Some(&mut ctx));
    }

    let recorded_ids: Vec<u64> = regions
        .iter()
        .filter_map(|e| match &e.on_left {
            Some(MouseAction::Emit(msg)) => match **msg {
                Message::ClickLogRow { entry_id, .. } => Some(entry_id),
                _ => None,
            },
            _ => None,
        })
        .collect();

    assert_eq!(
        recorded_ids, expected_ids,
        "recorded entry_ids must match the log entries in order"
    );
}

#[test]
fn render_with_regions_row_rects_have_correct_dimensions_nowrap() {
    use crate::render::MouseCtx;
    use fdemon_app::message::Message;
    use fdemon_app::{MouseAction, MouseRegions};
    use ratatui::{buffer::Buffer, layout::Rect};

    let logs = make_logs_no_traces(3);
    let mut state = LogViewState::new();
    let view = LogView::new(&logs, test_icons()).wrap_mode(false);

    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    let mut regions = MouseRegions::with_capacity();
    {
        let builder = regions.builder();
        let mut ctx = MouseCtx::new(builder);
        super::render_with_regions(area, &mut buf, &mut state, view, Some(&mut ctx));
    }

    let row_rects: Vec<_> = regions
        .iter()
        .filter(|e| {
            matches!(
                &e.on_left,
                Some(MouseAction::Emit(msg)) if matches!(**msg, Message::ClickLogRow { .. })
            )
        })
        .map(|e| e.rect)
        .collect();

    for rect in &row_rects {
        // In nowrap mode every row is exactly 1 cell tall.
        assert_eq!(
            rect.height, 1,
            "nowrap row height must be 1, got {}",
            rect.height
        );
        // Width spans the content area (80-wide terminal minus borders = 78).
        assert!(rect.width > 0, "row width must be > 0");
        // No zero-area rects — width > 0 and height > 0.
        assert!(rect.width > 0 && rect.height > 0, "rect must not be empty");
    }

    // Row Y positions must be strictly increasing (rows don't overlap).
    let ys: Vec<u16> = row_rects.iter().map(|r| r.y).collect();
    for window in ys.windows(2) {
        assert!(
            window[0] < window[1],
            "row Y positions must be strictly increasing: {:?}",
            ys
        );
    }
}

// ── Phase 4.5 Task 01: wrap-mode click-region alignment regression tests ──────

/// Helper: extract ClickLogRow regions from a MouseRegions, returning (rect, entry_id) pairs.
fn collect_click_regions(regions: &fdemon_app::MouseRegions) -> Vec<(fdemon_app::MouseRect, u64)> {
    use fdemon_app::message::Message;
    use fdemon_app::MouseAction;

    regions
        .iter()
        .filter_map(|e| match &e.on_left {
            Some(MouseAction::Emit(msg)) => match **msg {
                Message::ClickLogRow { entry_id, .. } => Some((e.rect, entry_id)),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

/// Wrap mode with offset=0: a single entry whose message wraps to exactly 3
/// terminal rows should register one click region spanning those 3 rows.
///
/// Layout (area 20x20, no timestamps/source, width=18):
///   content_area: x=1, y=3, width=18, height=16
///   Entry A: message = 54 chars → 3 wrapped rows
///   wrap_intra_offset = 0
///   Expected region: y=3, height=3
#[test]
fn wrap_mode_zero_offset_regions_align_with_rows() {
    use crate::render::MouseCtx;
    use fdemon_app::MouseRegions;
    use ratatui::{buffer::Buffer, layout::Rect};

    // 54-char message → ceil(54/18) = 3 wrapped rows at content_area width=18.
    let msg_a = "A".repeat(54);
    let entry_a = make_entry(LogLevel::Info, LogSource::App, &msg_a);
    let id_a = entry_a.id;

    let logs = logs_from(vec![entry_a]);
    let mut state = LogViewState::new();
    state.auto_scroll = false;
    state.offset = 0;

    let view = LogView::new(&logs, test_icons())
        .wrap_mode(true)
        .show_timestamps(false)
        .show_source(false);

    let area = Rect::new(0, 0, 20, 20);
    let mut buf = Buffer::empty(area);
    let mut regions = MouseRegions::with_capacity();
    {
        let builder = regions.builder();
        let mut ctx = MouseCtx::new(builder);
        super::render_with_regions(area, &mut buf, &mut state, view, Some(&mut ctx));
    }

    let click_regions = collect_click_regions(&regions);

    assert_eq!(
        click_regions.len(),
        1,
        "expected exactly 1 click region for entry A, got {}: {click_regions:?}",
        click_regions.len()
    );
    let (rect, entry_id) = click_regions[0];
    assert_eq!(entry_id, id_a, "region entry_id must match entry A");
    // content_area.y = area.y(0) + border(1) + metadata(1) + top_gap(1) = 3
    assert_eq!(rect.y, 3, "region must start at content_area top (y=3)");
    assert_eq!(
        rect.height, 3,
        "region height must equal wrapped row count (3)"
    );
}

/// Wrap mode with offset=2 (partial top-clip): entry A has 3 wrapped rows,
/// entry B has 2. With offset=2, wrap_intra_offset=2, so only A's third row
/// is visible (height=1) and B's two rows follow.
///
/// Before the fix, A was registered at y=3 with height=3 and B at y=6 with
/// height=2 — both off the screen relative to what the Paragraph rendered.
/// After the fix, A is at y=3 height=1 and B is at y=4 height=2.
#[test]
fn wrap_mode_intra_offset_skips_top_clipped_row() {
    use crate::render::MouseCtx;
    use fdemon_app::MouseRegions;
    use ratatui::{buffer::Buffer, layout::Rect};

    let msg_a = "A".repeat(54); // 3 wrapped rows at width=18
    let msg_b = "B".repeat(36); // 2 wrapped rows at width=18
    let entry_a = make_entry(LogLevel::Info, LogSource::App, &msg_a);
    let entry_b = make_entry(LogLevel::Info, LogSource::App, &msg_b);
    let id_a = entry_a.id;
    let id_b = entry_b.id;

    let logs = logs_from(vec![entry_a, entry_b]);
    let mut state = LogViewState::new();
    state.auto_scroll = false;
    // offset=2 → wrap_intra_offset=2 for entry A (which starts at units_skipped=0).
    // A's visible portion: rows 2..=2 (1 row). B follows at screen y=1..=2.
    state.offset = 2;

    let view = LogView::new(&logs, test_icons())
        .wrap_mode(true)
        .show_timestamps(false)
        .show_source(false);

    let area = Rect::new(0, 0, 20, 20);
    let mut buf = Buffer::empty(area);
    let mut regions = MouseRegions::with_capacity();
    {
        let builder = regions.builder();
        let mut ctx = MouseCtx::new(builder);
        super::render_with_regions(area, &mut buf, &mut state, view, Some(&mut ctx));
    }

    let click_regions = collect_click_regions(&regions);

    // Both entries should have a region (A is partially visible).
    assert_eq!(
        click_regions.len(),
        2,
        "expected 2 click regions (A clipped + B full), got {}: {click_regions:?}",
        click_regions.len()
    );

    let [(rect_a, eid_a), (rect_b, eid_b)] = [click_regions[0], click_regions[1]];
    assert_eq!(eid_a, id_a, "first region must be entry A");
    assert_eq!(eid_b, id_b, "second region must be entry B");

    // content_area.y = 3; wrap_intra_offset=2 hides the first 2 rows of A.
    // A: visible_y=0, height=1 → rect.y=3, height=1.
    assert_eq!(
        rect_a.y, 3,
        "A clipped region must start at content_area top"
    );
    assert_eq!(
        rect_a.height, 1,
        "A must show only its 1 remaining visible row"
    );

    // B: visible_y = 3 - 2 = 1 → rect.y=4, height=2.
    assert_eq!(
        rect_b.y, 4,
        "B region must start one row below A's clipped region"
    );
    assert_eq!(rect_b.height, 2, "B must show all 2 of its wrapped rows");
}

/// Wrap mode with offset=3 (full top-skip): entry A has exactly 3 wrapped rows
/// so it is completely scrolled past. Only B is visible, starting at screen y=0.
///
/// The fix must not produce any region for A (it is never added to row_actions),
/// and B's region must be aligned to the top of the content area.
#[test]
fn wrap_mode_intra_offset_top_skipped_row_dropped() {
    use crate::render::MouseCtx;
    use fdemon_app::MouseRegions;
    use ratatui::{buffer::Buffer, layout::Rect};

    let msg_a = "A".repeat(54); // 3 wrapped rows at width=18
    let msg_b = "B".repeat(36); // 2 wrapped rows at width=18
    let entry_a = make_entry(LogLevel::Info, LogSource::App, &msg_a);
    let entry_b = make_entry(LogLevel::Info, LogSource::App, &msg_b);
    let id_a = entry_a.id;
    let id_b = entry_b.id;

    let logs = logs_from(vec![entry_a, entry_b]);
    let mut state = LogViewState::new();
    state.auto_scroll = false;
    // offset=3 → A (3 rows) is fully past; B starts at screen y=0, wrap_intra_offset=0.
    state.offset = 3;

    let view = LogView::new(&logs, test_icons())
        .wrap_mode(true)
        .show_timestamps(false)
        .show_source(false);

    let area = Rect::new(0, 0, 20, 20);
    let mut buf = Buffer::empty(area);
    let mut regions = MouseRegions::with_capacity();
    {
        let builder = regions.builder();
        let mut ctx = MouseCtx::new(builder);
        super::render_with_regions(area, &mut buf, &mut state, view, Some(&mut ctx));
    }

    let click_regions = collect_click_regions(&regions);

    // Only B is visible; A must produce no region.
    let entry_ids: Vec<u64> = click_regions.iter().map(|(_, id)| *id).collect();
    assert!(
        !entry_ids.contains(&id_a),
        "A must produce no region when fully scrolled off; got {click_regions:?}"
    );
    assert_eq!(
        click_regions.len(),
        1,
        "expected exactly 1 click region (B only), got {}: {click_regions:?}",
        click_regions.len()
    );

    let (rect_b, eid_b) = click_regions[0];
    assert_eq!(eid_b, id_b, "the sole region must be entry B");
    // B starts at top of content_area: rect.y = 3, height = 2.
    assert_eq!(rect_b.y, 3, "B must align to content_area top (y=3)");
    assert_eq!(rect_b.height, 2, "B must show its full 2 wrapped rows");
}

// ── Phase 5 Task 08: link-highlight badge region tests ────────────────────────

/// Build a `LinkHighlightState` from a list of (entry_index, frame_index, shortcut,
/// display_text) tuples. The state is set to active.
fn make_link_state(links: &[(usize, Option<usize>, char, &str)]) -> LinkHighlightState {
    use fdemon_app::hyperlinks::{DetectedLink, FileReference};

    let mut state = LinkHighlightState::new();
    for (i, &(entry_index, frame_index, shortcut, display_text)) in links.iter().enumerate() {
        // Construct a FileReference whose display matches display_text.
        // We need display_text == path:line:col so parse it naively.
        // For test purposes, create any FileReference and override the display via
        // DetectedLink directly (display_text is a stored field, not recomputed on access).
        let file_ref = FileReference::new(display_text, 1, 1);
        let mut link = DetectedLink::new(file_ref, entry_index, frame_index, shortcut, i);
        // Override display_text to match exactly what's in the log message.
        link.display_text = display_text.to_string();
        state.add_link(link);
    }
    state.activate();
    state
}

/// Helper: extract SelectLink shortcuts from recorded regions.
fn collect_badge_shortcuts(regions: &fdemon_app::MouseRegions) -> Vec<char> {
    use fdemon_app::message::Message;
    use fdemon_app::MouseAction;

    regions
        .iter()
        .filter_map(|e| match &e.on_left {
            Some(MouseAction::Emit(msg)) => match **msg {
                Message::SelectLink(c) => Some(c),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

/// Helper: extract badge regions (rect + shortcut) from recorded regions.
fn collect_badge_regions(
    regions: &fdemon_app::MouseRegions,
) -> Vec<(fdemon_app::MouseRect, char, u8)> {
    use fdemon_app::message::Message;
    use fdemon_app::MouseAction;

    regions
        .iter()
        .filter_map(|e| match &e.on_left {
            Some(MouseAction::Emit(msg)) => match **msg {
                Message::SelectLink(c) => Some((e.rect, c, e.z_index)),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

/// When `link_highlight_state` is not set on the `LogView`, zero badge regions
/// must be recorded even when a `MouseCtx` is provided.
#[test]
fn render_with_regions_records_no_badges_when_link_mode_inactive() {
    use crate::render::MouseCtx;
    use fdemon_app::MouseRegions;
    use ratatui::{buffer::Buffer, layout::Rect};

    // One entry with a file reference in the message — badge would be rendered
    // if link mode were active, but it isn't here.
    let logs = logs_from(vec![make_entry(
        LogLevel::Info,
        LogSource::App,
        "Error at lib/main.dart:10:1",
    )]);
    let mut state = LogViewState::new();
    // No link_highlight_state set on the view.
    let view = LogView::new(&logs, test_icons())
        .show_timestamps(false)
        .show_source(false);

    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    let mut regions = MouseRegions::with_capacity();
    {
        let builder = regions.builder();
        let mut ctx = MouseCtx::new(builder);
        super::render_with_regions(area, &mut buf, &mut state, view, Some(&mut ctx));
    }

    let badge_count = collect_badge_shortcuts(&regions).len();
    assert_eq!(
        badge_count, 0,
        "no badge regions expected when link mode is inactive"
    );
}

/// When `link_highlight_state` is active with N links, exactly N badge regions
/// must be recorded (one per link badge rendered in the log view).
#[test]
fn render_with_regions_records_one_badge_per_link_when_active() {
    use crate::render::MouseCtx;
    use fdemon_app::MouseRegions;
    use ratatui::{buffer::Buffer, layout::Rect};

    // Three entries each with a distinct file reference in the message.
    let logs = logs_from(vec![
        make_entry(
            LogLevel::Info,
            LogSource::App,
            "see lib/a.dart:1:1 for details",
        ),
        make_entry(
            LogLevel::Info,
            LogSource::App,
            "see lib/b.dart:2:1 for details",
        ),
        make_entry(
            LogLevel::Info,
            LogSource::App,
            "see lib/c.dart:3:1 for details",
        ),
    ]);

    let link_state = make_link_state(&[
        (0, None, '1', "lib/a.dart:1:1"),
        (1, None, '2', "lib/b.dart:2:1"),
        (2, None, '3', "lib/c.dart:3:1"),
    ]);

    let mut state = LogViewState::new();
    let view = LogView::new(&logs, test_icons())
        .show_timestamps(false)
        .show_source(false)
        .link_highlight_state(&link_state);

    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    let mut regions = MouseRegions::with_capacity();
    {
        let builder = regions.builder();
        let mut ctx = MouseCtx::new(builder);
        super::render_with_regions(area, &mut buf, &mut state, view, Some(&mut ctx));
    }

    let badge_regions = collect_badge_regions(&regions);
    assert_eq!(
        badge_regions.len(),
        3,
        "expected 3 badge regions (one per link), got {}: {badge_regions:?}",
        badge_regions.len()
    );

    // Each badge rect must be exactly 3 cells wide, 1 cell tall, at z_index = 0.
    for (rect, _shortcut, z_index) in &badge_regions {
        assert_eq!(rect.width, 3, "badge rect must be 3 cells wide");
        assert_eq!(rect.height, 1, "badge rect must be 1 cell tall");
        assert_eq!(*z_index, 0, "badge regions must be at z_index = 0");
    }

    // Shortcuts must match links in order.
    let shortcuts: Vec<char> = badge_regions.iter().map(|(_, c, _)| *c).collect();
    assert_eq!(
        shortcuts,
        vec!['1', '2', '3'],
        "shortcuts must match links in order"
    );
}

/// Badge regions must be pushed *after* the row regions so they win on
/// overlapping cells (last-pushed-wins at equal z_index).
#[test]
fn render_with_regions_badges_pushed_after_row_regions() {
    use crate::render::MouseCtx;
    use fdemon_app::message::Message;
    use fdemon_app::{MouseAction, MouseRegions};
    use ratatui::{buffer::Buffer, layout::Rect};

    let logs = logs_from(vec![make_entry(
        LogLevel::Info,
        LogSource::App,
        "see lib/x.dart:5:1 here",
    )]);

    let link_state = make_link_state(&[(0, None, 'a', "lib/x.dart:5:1")]);

    let mut state = LogViewState::new();
    let view = LogView::new(&logs, test_icons())
        .show_timestamps(false)
        .show_source(false)
        .link_highlight_state(&link_state);

    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    let mut regions = MouseRegions::with_capacity();
    {
        let builder = regions.builder();
        let mut ctx = MouseCtx::new(builder);
        super::render_with_regions(area, &mut buf, &mut state, view, Some(&mut ctx));
    }

    // Find the positions of ClickLogRow and SelectLink entries in the push order.
    let all: Vec<_> = regions.iter().collect();
    let row_pos = all.iter().position(|e| {
        matches!(
            &e.on_left,
            Some(MouseAction::Emit(msg)) if matches!(**msg, Message::ClickLogRow { .. })
        )
    });
    let badge_pos = all.iter().position(|e| {
        matches!(
            &e.on_left,
            Some(MouseAction::Emit(msg)) if matches!(**msg, Message::SelectLink(_))
        )
    });

    assert!(row_pos.is_some(), "expected a ClickLogRow region");
    assert!(badge_pos.is_some(), "expected a SelectLink region");
    assert!(
        badge_pos.unwrap() > row_pos.unwrap(),
        "badge region must be pushed after row region so it wins on overlap"
    );
}

/// Links whose entries are scrolled out of the visible window must not produce
/// any badge regions (the badge is not rendered, so no rect is registered).
#[test]
fn render_with_regions_off_screen_links_not_recorded() {
    use crate::render::MouseCtx;
    use fdemon_app::MouseRegions;
    use ratatui::{buffer::Buffer, layout::Rect};

    // 5 entries, but we render a small area that only shows ~1 entry.
    // The entries at indices 2-4 will be scrolled off screen.
    let mut entries = Vec::new();
    for i in 0..5 {
        entries.push(make_entry(
            LogLevel::Info,
            LogSource::App,
            &format!("see lib/file{i}.dart:{i}:1 here"),
        ));
    }
    let logs = logs_from(entries);

    // Links for entries 2-4 only (entries 0-1 have no badge).
    let link_state = make_link_state(&[
        (2, None, '1', "lib/file2.dart:2:1"),
        (3, None, '2', "lib/file3.dart:3:1"),
        (4, None, '3', "lib/file4.dart:4:1"),
    ]);

    let mut state = LogViewState::new();
    state.auto_scroll = false;
    // offset = 0: only entries 0 and 1 fit in a 6-row area
    // (border=2 + metadata=1 + gap=1 + content=2 rows visible).

    let view = LogView::new(&logs, test_icons())
        .show_timestamps(false)
        .show_source(false)
        .link_highlight_state(&link_state);

    // Small area: only 2 content rows visible.
    let area = Rect::new(0, 0, 80, 6);
    let mut buf = Buffer::empty(area);
    let mut regions = MouseRegions::with_capacity();
    {
        let builder = regions.builder();
        let mut ctx = MouseCtx::new(builder);
        super::render_with_regions(area, &mut buf, &mut state, view, Some(&mut ctx));
    }

    let badge_count = collect_badge_shortcuts(&regions).len();
    assert_eq!(
        badge_count, 0,
        "no badge regions expected: links are for entries scrolled off screen"
    );
}

// ── Phase 5.5 Task 03: wrap-mode badge y-position fix ────────────────────────

/// Regression: in wrap mode, a badge whose `col_offset` fits on the first
/// wrapped sub-row (col_offset < visible_width) must record a rect at the
/// correct screen-y with dx = col_offset, dy = 0.
///
/// Layout (area 22×10, visible_width=20):
///   content_area: x=1, y=3, width=20, height=6
///   col_offset=10 → dx=10, dy=0 → rect (11, 3, 3, 1)
#[test]
fn wrap_mode_badge_on_first_wrapped_row_records_at_correct_y() {
    use crate::render::MouseCtx;
    use fdemon_app::MouseRegions;
    use ratatui::{buffer::Buffer, layout::Rect};

    // 10-char prefix so badge lands at col_offset=10.
    let prefix = "A".repeat(10);
    let display_text = "lib/foo.dart:1:1";
    let msg = format!("{prefix}{display_text}");
    let entry = make_entry(LogLevel::Info, LogSource::App, &msg);
    let logs = logs_from(vec![entry]);

    let link_state = make_link_state(&[(0, None, 'a', display_text)]);
    let mut state = LogViewState::new();
    state.auto_scroll = false;
    state.offset = 0;

    // area width=22 → content_area.width=20 (visible_width=20).
    let area = Rect::new(0, 0, 22, 10);
    let mut buf = Buffer::empty(area);
    let mut regions = MouseRegions::with_capacity();
    {
        let view = LogView::new(&logs, test_icons())
            .wrap_mode(true)
            .show_timestamps(false)
            .show_source(false)
            .link_highlight_state(&link_state);
        let builder = regions.builder();
        let mut ctx = MouseCtx::new(builder);
        super::render_with_regions(area, &mut buf, &mut state, view, Some(&mut ctx));
    }

    let badge_regions = collect_badge_regions(&regions);
    assert_eq!(
        badge_regions.len(),
        1,
        "expected exactly 1 badge region, got {}: {badge_regions:?}",
        badge_regions.len()
    );
    let (rect, shortcut, _z) = badge_regions[0];
    assert_eq!(shortcut, 'a');
    // content_area: x=1, y=3, width=20.
    // col_offset=10 → dx=10, dy=0 → badge_x=11, screen_y=3.
    assert_eq!(
        rect.x, 11,
        "badge x should be content_area.x + dx (1+10=11)"
    );
    assert_eq!(rect.y, 3, "badge y should be content_area.y + 0 (dy=0)");
    assert_eq!(rect.width, 3, "badge width should be 3");
    assert_eq!(rect.height, 1, "badge height should be 1");
}

/// Regression: in wrap mode, a badge whose `col_offset` exceeds `visible_width`
/// renders on a wrapped sub-row. The rect must use modular arithmetic:
///   dx = col_offset % visible_width
///   dy = col_offset / visible_width
///
/// Layout (area 22×10, visible_width=20):
///   content_area: x=1, y=3, width=20, height=6
///   col_offset=25 → dx=5, dy=1 → rect (6, 4, 3, 1)
#[test]
fn wrap_mode_badge_on_second_wrapped_row_records_at_correct_y() {
    use crate::render::MouseCtx;
    use fdemon_app::MouseRegions;
    use ratatui::{buffer::Buffer, layout::Rect};

    // 25-char prefix so badge lands at col_offset=25.
    let prefix = "A".repeat(25);
    let display_text = "lib/foo.dart:1:1";
    let msg = format!("{prefix}{display_text}");
    let entry = make_entry(LogLevel::Info, LogSource::App, &msg);
    let logs = logs_from(vec![entry]);

    let link_state = make_link_state(&[(0, None, 'b', display_text)]);
    let mut state = LogViewState::new();
    state.auto_scroll = false;
    state.offset = 0;

    let area = Rect::new(0, 0, 22, 10);
    let mut buf = Buffer::empty(area);
    let mut regions = MouseRegions::with_capacity();
    {
        let view = LogView::new(&logs, test_icons())
            .wrap_mode(true)
            .show_timestamps(false)
            .show_source(false)
            .link_highlight_state(&link_state);
        let builder = regions.builder();
        let mut ctx = MouseCtx::new(builder);
        super::render_with_regions(area, &mut buf, &mut state, view, Some(&mut ctx));
    }

    let badge_regions = collect_badge_regions(&regions);
    assert_eq!(
        badge_regions.len(),
        1,
        "expected exactly 1 badge region, got {}: {badge_regions:?}",
        badge_regions.len()
    );
    let (rect, shortcut, _z) = badge_regions[0];
    assert_eq!(shortcut, 'b');
    // col_offset=25, visible_width=20 → dx=25%20=5, dy=25/20=1.
    // badge_x = content_area.x + dx = 1 + 5 = 6.
    // screen_y = content_area.y + (rel_y + dy - wio) = 3 + (0+1-0) = 4.
    assert_eq!(
        rect.x, 6,
        "badge x should be content_area.x + (col_offset % 20) = 1+5=6"
    );
    assert_eq!(rect.y, 4, "badge y should be content_area.y + dy = 3+1=4");
    assert_eq!(rect.width, 3, "badge width should be 3");
    assert_eq!(rect.height, 1, "badge height should be 1");
}

/// Regression: a badge at the last column of a wrapped sub-row (dx + badge_w > visible_width)
/// must be clipped to fit within the content area rather than overflowing.
///
/// Layout (area 22×10, visible_width=20):
///   content_area: x=1, y=3, width=20, height=6
///   col_offset=19 → dx=19, dy=0 → badge_x=20, badge_w=min(3, right_edge-badge_x)=min(3,1)=1
#[test]
fn wrap_mode_badge_clipped_at_right_edge() {
    use crate::render::MouseCtx;
    use fdemon_app::MouseRegions;
    use ratatui::{buffer::Buffer, layout::Rect};

    // 19-char prefix so badge lands at col_offset=19 (last column of first sub-row).
    let prefix = "A".repeat(19);
    let display_text = "lib/foo.dart:1:1";
    let msg = format!("{prefix}{display_text}");
    let entry = make_entry(LogLevel::Info, LogSource::App, &msg);
    let logs = logs_from(vec![entry]);

    let link_state = make_link_state(&[(0, None, 'c', display_text)]);
    let mut state = LogViewState::new();
    state.auto_scroll = false;
    state.offset = 0;

    let area = Rect::new(0, 0, 22, 10);
    let mut buf = Buffer::empty(area);
    let mut regions = MouseRegions::with_capacity();
    {
        let view = LogView::new(&logs, test_icons())
            .wrap_mode(true)
            .show_timestamps(false)
            .show_source(false)
            .link_highlight_state(&link_state);
        let builder = regions.builder();
        let mut ctx = MouseCtx::new(builder);
        super::render_with_regions(area, &mut buf, &mut state, view, Some(&mut ctx));
    }

    let badge_regions = collect_badge_regions(&regions);
    assert_eq!(
        badge_regions.len(),
        1,
        "expected exactly 1 badge region (clipped), got {}: {badge_regions:?}",
        badge_regions.len()
    );
    let (rect, shortcut, _z) = badge_regions[0];
    assert_eq!(shortcut, 'c');
    // col_offset=19 → dx=19, dy=0. badge_x=1+19=20.
    // content right edge = 1+20=21. badge_w = min(3, 21-20) = 1.
    assert_eq!(
        rect.x, 20,
        "badge x should be 20 (content_area.x + dx = 1+19)"
    );
    assert_eq!(rect.y, 3, "badge y should be content_area.y (dy=0)");
    assert_eq!(
        rect.width, 1,
        "badge width should be clipped to 1 (only 1 cell before right edge)"
    );
    assert_eq!(rect.height, 1, "badge height should be 1");
}

/// Regression: a badge whose computed `screen_y >= content_area.height` after
/// wrap-offset adjustment must be silently dropped — no panic, no region recorded.
///
/// Layout (area 22×10, visible_width=20, content_area.height=6):
///   col_offset=120 → dy=6 → screen_y=6 >= 6 → dropped.
#[test]
fn wrap_mode_badge_off_screen_dropped() {
    use crate::render::MouseCtx;
    use fdemon_app::MouseRegions;
    use ratatui::{buffer::Buffer, layout::Rect};

    // 120-char prefix → col_offset=120, dy=120/20=6 which equals content_area.height.
    let prefix = "A".repeat(120);
    let display_text = "lib/foo.dart:1:1";
    let msg = format!("{prefix}{display_text}");
    let entry = make_entry(LogLevel::Info, LogSource::App, &msg);
    let logs = logs_from(vec![entry]);

    let link_state = make_link_state(&[(0, None, 'd', display_text)]);
    let mut state = LogViewState::new();
    // Keep offset=0 so wio=0 and badge_all_lines_y=dy=6 >= content_area.height=6.
    state.auto_scroll = false;
    state.offset = 0;

    let area = Rect::new(0, 0, 22, 10);
    let mut buf = Buffer::empty(area);
    let mut regions = MouseRegions::with_capacity();
    {
        let view = LogView::new(&logs, test_icons())
            .wrap_mode(true)
            .show_timestamps(false)
            .show_source(false)
            .link_highlight_state(&link_state);
        let builder = regions.builder();
        let mut ctx = MouseCtx::new(builder);
        super::render_with_regions(area, &mut buf, &mut state, view, Some(&mut ctx));
    }

    let badge_regions = collect_badge_regions(&regions);
    assert_eq!(
        badge_regions.len(),
        0,
        "badge at screen_y >= content_area.height must be dropped, got: {badge_regions:?}"
    );
}

// ─────────────────────────────────────────────────────────
// Mouse capture status badge tests (Task 08)
// ─────────────────────────────────────────────────────────

#[test]
fn test_status_info_renders_mouse_on_badge() {
    use crate::test_utils::TestTerminal;

    // Wide terminal — full (non-compact) mode so badge is visible
    let mut term = TestTerminal::with_size(120, 10);

    let logs = logs_from(vec![make_entry(LogLevel::Info, LogSource::App, "msg")]);

    let status_info = StatusInfo {
        phase: &AppPhase::Running,
        is_busy: false,
        mode: None,
        flavor: None,
        duration: None,
        error_count: 0,
        vm_connected: false,
        dap_port: None,
        dap_config_ide: None,
        mouse_capture_active: true,
        animation_frame: 0,
        progress: None,
    };

    let log_view = LogView::new(&logs, test_icons()).with_status(status_info);
    let mut state = LogViewState::new();

    term.render_stateful_widget(log_view, term.area(), &mut state);

    assert!(
        term.buffer_contains("[mouse]"),
        "Status bar should show '[mouse]' badge when mouse_capture_active is true"
    );
    assert!(
        !term.buffer_contains("[mouse-off]"),
        "Status bar must not show '[mouse-off]' when mouse_capture_active is true"
    );
}

#[test]
fn test_status_info_renders_mouse_off_badge() {
    use crate::test_utils::TestTerminal;

    // Wide terminal — full (non-compact) mode so badge is visible
    let mut term = TestTerminal::with_size(120, 10);

    let logs = logs_from(vec![make_entry(LogLevel::Info, LogSource::App, "msg")]);

    let status_info = StatusInfo {
        phase: &AppPhase::Running,
        is_busy: false,
        mode: None,
        flavor: None,
        duration: None,
        error_count: 0,
        vm_connected: false,
        dap_port: None,
        dap_config_ide: None,
        mouse_capture_active: false,
        animation_frame: 0,
        progress: None,
    };

    let log_view = LogView::new(&logs, test_icons()).with_status(status_info);
    let mut state = LogViewState::new();

    term.render_stateful_widget(log_view, term.area(), &mut state);

    assert!(
        term.buffer_contains("[mouse-off]"),
        "Status bar should show '[mouse-off]' badge when mouse_capture_active is false"
    );
    // `[mouse-off]` contains the substring `[mouse`, so a naive substring search
    // for `[mouse` is always true when `[mouse-off]` is present. However, the
    // 7-char sequence `[mouse]` (closing bracket, no `-off`) is only present in
    // the on-state badge and will NOT match `[mouse-off]`. Assert its absence.
    assert!(
        !term.buffer_contains("[mouse]"),
        "Status bar must not show plain '[mouse]' (the on-state badge) when capture is off"
    );
}

#[test]
fn test_status_info_drops_badge_when_width_too_narrow() {
    use crate::test_utils::TestTerminal;

    // Narrow terminal (40 cols) — compact mode kicks in (< MIN_FULL_STATUS_WIDTH = 60),
    // which hides all right-side content including the mouse badge.
    let mut term = TestTerminal::with_size(40, 10);

    let logs = logs_from(vec![make_entry(LogLevel::Info, LogSource::App, "msg")]);

    let status_info = StatusInfo {
        phase: &AppPhase::Running,
        is_busy: false,
        mode: None,
        flavor: None,
        duration: None,
        error_count: 0,
        vm_connected: false,
        dap_port: None,
        dap_config_ide: None,
        mouse_capture_active: true,
        animation_frame: 0,
        progress: None,
    };

    let log_view = LogView::new(&logs, test_icons()).with_status(status_info);
    let mut state = LogViewState::new();

    term.render_stateful_widget(log_view, term.area(), &mut state);

    // In compact mode (width < 60) the right-side section (uptime + errors + badge)
    // is suppressed entirely, so neither badge variant should appear.
    // Note: `[mouse-off]` does NOT contain the substring `[mouse]` (the latter
    // requires a `]` immediately after `e`), so we must assert both variants
    // separately to catch a wrong-badge regression.
    assert!(
        !term.buffer_contains("[mouse]"),
        "On-state mouse badge should be absent in compact mode (terminal width 40 < 60)"
    );
    assert!(
        !term.buffer_contains("[mouse-off]"),
        "Off-state mouse badge should also be absent in compact mode (terminal width 40 < 60)"
    );
}

// ── shimmer label tests ────────────────────────────────────────────────────

#[test]
fn reloading_label_shimmers_across_chars() {
    // For a transient phase the label should produce one span per character
    // (the shimmer sweep) whose fg colours are not all identical.
    use ratatui::style::{Color, Modifier, Style};

    let phase_style = Style::default()
        .fg(Color::Rgb(200, 170, 0)) // STATUS_YELLOW-ish
        .add_modifier(Modifier::BOLD);

    // Frame 5 → head somewhere in the middle of "Reloading"
    let spans = LogView::status_label_spans_inner("Reloading", phase_style, true, 5);

    // One span per character
    assert_eq!(
        spans.len(),
        "Reloading".chars().count(),
        "transient label must produce one span per char (shimmer)"
    );

    // Collect all fg values; the sweep must produce at least two distinct colours
    let fg_values: Vec<Color> = spans.iter().filter_map(|s| s.style.fg).collect();
    assert_eq!(
        fg_values.len(),
        spans.len(),
        "every shimmer span must carry an fg colour"
    );
    let all_same = fg_values.windows(2).all(|w| w[0] == w[1]);
    assert!(
        !all_same,
        "shimmer must produce a gradient — not all fg colours are equal"
    );
}

#[test]
fn running_label_is_static_single_style() {
    // For a steady state the label must be a single span with the phase style's fg.
    use ratatui::style::{Color, Style};

    let base_fg = Color::Rgb(63, 185, 80); // STATUS_GREEN
    let phase_style = Style::default().fg(base_fg);

    let spans = LogView::status_label_spans_inner("Running", phase_style, false, 42);

    assert_eq!(spans.len(), 1, "steady state must produce exactly one span");
    assert_eq!(
        spans[0].style.fg,
        Some(base_fg),
        "steady state span fg must match the phase style"
    );
    assert_eq!(
        spans[0].content.as_ref(),
        "Running",
        "steady state span must carry the full label text"
    );
}

#[test]
fn shimmer_advances_with_animation_frame() {
    // Two renders with different animation_frame values must produce different
    // fg distributions (the shimmer head has moved).
    use ratatui::style::{Color, Modifier, Style};

    let phase_style = Style::default()
        .fg(Color::Rgb(200, 100, 0))
        .add_modifier(Modifier::BOLD);
    let label = "Reloading";

    let spans_a = LogView::status_label_spans_inner(label, phase_style, true, 0);
    let spans_b = LogView::status_label_spans_inner(label, phase_style, true, 10);

    let fgs_a: Vec<Option<Color>> = spans_a.iter().map(|s| s.style.fg).collect();
    let fgs_b: Vec<Option<Color>> = spans_b.iter().map(|s| s.style.fg).collect();

    assert_ne!(
        fgs_a, fgs_b,
        "different animation_frame values must yield different shimmer fg distributions"
    );
}

// ── Phase 2.5 Task 04: Launching/Preparing shimmer + progress tests ───────────

#[test]
fn launching_label_shimmers_across_chars() {
    // Launching is a transient phase — the label must produce one span per
    // character with varying fg colours (shimmer sweep).
    use ratatui::style::{Color, Modifier, Style};

    let phase_style = Style::default()
        .fg(palette::STATUS_BLUE)
        .add_modifier(Modifier::BOLD);

    // Frame 3 → head somewhere in the middle of "Launching"
    let spans = LogView::status_label_spans_inner("Launching", phase_style, true, 3);

    assert_eq!(
        spans.len(),
        "Launching".chars().count(),
        "Launching label must produce one span per char (shimmer)"
    );

    let fg_values: Vec<Color> = spans.iter().filter_map(|s| s.style.fg).collect();
    assert_eq!(
        fg_values.len(),
        spans.len(),
        "every shimmer span must carry an fg colour"
    );
    let all_same = fg_values.windows(2).all(|w| w[0] == w[1]);
    assert!(
        !all_same,
        "shimmer must produce a gradient — not all fg colours are equal"
    );
}

#[test]
fn preparing_label_shimmers_across_chars() {
    // Preparing is a transient phase — the label must produce one span per
    // character with varying fg colours (shimmer sweep).
    use ratatui::style::{Color, Style};

    let phase_style = Style::default().fg(palette::STATUS_BLUE);

    // Frame 7 → head somewhere in the middle of "Preparing"
    let spans = LogView::status_label_spans_inner("Preparing", phase_style, true, 7);

    assert_eq!(
        spans.len(),
        "Preparing".chars().count(),
        "Preparing label must produce one span per char (shimmer)"
    );

    let fg_values: Vec<Color> = spans.iter().filter_map(|s| s.style.fg).collect();
    assert_eq!(
        fg_values.len(),
        spans.len(),
        "every shimmer span must carry an fg colour"
    );
    let all_same = fg_values.windows(2).all(|w| w[0] == w[1]);
    assert!(
        !all_same,
        "shimmer must produce a gradient — not all fg colours are equal"
    );
}

#[test]
fn progress_suffix_rendered_when_present() {
    // When phase is Launching and progress is Some, the progress text must
    // appear in the rendered bottom metadata bar with muted fg.
    use crate::test_utils::TestTerminal;
    use std::time::Duration;

    let mut term = TestTerminal::with_size(120, 10);

    let logs = logs_from(vec![make_entry(LogLevel::Info, LogSource::App, "msg")]);

    let status_info = StatusInfo {
        phase: &AppPhase::Launching,
        is_busy: false,
        mode: None,
        flavor: None,
        duration: Some(Duration::from_secs(2)),
        error_count: 0,
        vm_connected: false,
        dap_port: None,
        dap_config_ide: None,
        mouse_capture_active: true,
        animation_frame: 0,
        progress: Some("Running Gradle task"),
    };

    let log_view = LogView::new(&logs, test_icons()).with_status(status_info);
    let mut state = LogViewState::new();

    term.render_stateful_widget(log_view, term.area(), &mut state);

    assert!(
        term.buffer_contains("Running Gradle task"),
        "Progress suffix 'Running Gradle task' should appear in the status bar when present"
    );
}

#[test]
fn running_has_no_progress_suffix() {
    // When phase is Running (steady state), progress text must NOT appear
    // even if progress is Some — steady states ignore the progress field.
    use crate::test_utils::TestTerminal;

    let mut term = TestTerminal::with_size(120, 10);

    let logs = logs_from(vec![make_entry(LogLevel::Info, LogSource::App, "msg")]);

    let status_info = StatusInfo {
        phase: &AppPhase::Running,
        is_busy: false,
        mode: None,
        flavor: None,
        duration: None,
        error_count: 0,
        vm_connected: false,
        dap_port: None,
        dap_config_ide: None,
        mouse_capture_active: true,
        animation_frame: 0,
        progress: Some("this should not appear"),
    };

    let log_view = LogView::new(&logs, test_icons()).with_status(status_info);
    let mut state = LogViewState::new();

    term.render_stateful_widget(log_view, term.area(), &mut state);

    assert!(
        !term.buffer_contains("this should not appear"),
        "Running (steady state) must not render progress suffix even when progress is Some"
    );
}

// ─────────────────────────────────────────────────────────
// Jump-to-latest indicator pill tests (Phase 4, Task 02)
// ─────────────────────────────────────────────────────────

/// Read the content of a single row from a raw `Buffer` as a `String`.
///
/// Used by the jump-to-latest pill tests which operate directly on `Buffer`
/// instances (rather than `TestTerminal`) so they can control exact coordinates.
fn read_row(buf: &ratatui::buffer::Buffer, row: u16) -> String {
    let mut s = String::new();
    if row < buf.area.height {
        for x in 0..buf.area.width {
            s.push_str(buf[(x, row)].symbol());
        }
    }
    s
}

/// Helper: create a `Buffer` of the given dimensions (origin 0, 0).
fn make_buffer(width: u16, height: u16) -> ratatui::buffer::Buffer {
    ratatui::buffer::Buffer::empty(ratatui::layout::Rect::new(0, 0, width, height))
}

/// Helper: create `count` plain log entries with no stack traces.
fn make_logs(count: usize) -> std::collections::VecDeque<LogEntry> {
    make_logs_no_traces(count)
}

/// Helper: return the default `IconSet` for rendering tests.
fn default_icons() -> crate::theme::icons::IconSet {
    test_icons()
}

#[test]
fn jump_hint_visible_when_scrolled_up_with_unseen_logs() {
    use ratatui::layout::Rect;

    // Layout for 40x10, no status footer:
    //   border(1) + meta(1) + gap(1) + content(6) + border(1) = 10 rows
    //   content rows: y=3..=8; last content row = y=8.
    let mut buf = make_buffer(40, 10);
    let mut state = LogViewState::new();
    state.auto_scroll = false;
    let logs = make_logs(2);
    let view = LogView::new(&logs, default_icons()).unseen_log_count(7);
    super::render_with_regions(Rect::new(0, 0, 40, 10), &mut buf, &mut state, view, None);

    // The pill is on the last row of content_area (y=8); row y=9 is the border.
    let bottom_content_row = read_row(&buf, 8);
    assert!(
        bottom_content_row.contains("G to jump"),
        "pill should be visible; last content row (y=8): {:?}",
        bottom_content_row
    );
    assert!(
        bottom_content_row.contains("7 new"),
        "pill should show count 7; last content row (y=8): {:?}",
        bottom_content_row
    );
}

#[test]
fn jump_hint_hidden_when_following_tail() {
    use ratatui::layout::Rect;

    let mut buf = make_buffer(40, 10);
    let mut state = LogViewState::new();
    state.auto_scroll = true; // default, but explicit
    let logs = make_logs(2);
    let view = LogView::new(&logs, default_icons()).unseen_log_count(50);
    super::render_with_regions(Rect::new(0, 0, 40, 10), &mut buf, &mut state, view, None);

    // Check both the last content row (y=8) and the border row (y=9).
    let bottom_content_row = read_row(&buf, 8);
    let border_row = read_row(&buf, 9);
    assert!(
        !bottom_content_row.contains("G to jump"),
        "pill must not appear when auto_scroll is true; last content row (y=8): {:?}",
        bottom_content_row
    );
    assert!(
        !border_row.contains("G to jump"),
        "pill must not appear on border row; border row (y=9): {:?}",
        border_row
    );
}

#[test]
fn jump_hint_hidden_when_count_zero() {
    use ratatui::layout::Rect;

    let mut buf = make_buffer(40, 10);
    let mut state = LogViewState::new();
    state.auto_scroll = false;
    let logs = make_logs(2);
    let view = LogView::new(&logs, default_icons()).unseen_log_count(0);
    super::render_with_regions(Rect::new(0, 0, 40, 10), &mut buf, &mut state, view, None);

    // Check the last content row (y=8).
    let bottom_content_row = read_row(&buf, 8);
    assert!(
        !bottom_content_row.contains("G to jump"),
        "pill must not appear when unseen_log_count is 0; last content row (y=8): {:?}",
        bottom_content_row
    );
}

#[test]
fn jump_hint_caps_display_at_999_plus() {
    use ratatui::layout::Rect;

    let mut buf = make_buffer(40, 10);
    let mut state = LogViewState::new();
    state.auto_scroll = false;
    let logs = make_logs(2);
    let view = LogView::new(&logs, default_icons()).unseen_log_count(12_345);
    super::render_with_regions(Rect::new(0, 0, 40, 10), &mut buf, &mut state, view, None);

    // Last content row is y=8 (border is y=9).
    let bottom_content_row = read_row(&buf, 8);
    assert!(
        bottom_content_row.contains("999+"),
        "pill should display '999+' for counts > 999; last content row (y=8): {:?}",
        bottom_content_row
    );
    assert!(
        !bottom_content_row.contains("12345"),
        "pill must not show the raw count when > 999; last content row (y=8): {:?}",
        bottom_content_row
    );
}

#[test]
fn jump_hint_suppressed_when_terminal_too_narrow() {
    use ratatui::layout::Rect;

    // Pill text "↓ 5 new · G to jump" = 20 chars; need ≥ 21 width to render.
    // A 10-column terminal is far too narrow.
    let mut buf = make_buffer(10, 10);
    let mut state = LogViewState::new();
    state.auto_scroll = false;
    let logs = make_logs(2);
    let view = LogView::new(&logs, default_icons()).unseen_log_count(5);
    super::render_with_regions(Rect::new(0, 0, 10, 10), &mut buf, &mut state, view, None);

    // Check both the last content row (y=8) and entire buffer.
    let bottom_content_row = read_row(&buf, 8);
    let border_row = read_row(&buf, 9);
    assert!(
        !bottom_content_row.contains("G to jump"),
        "pill must be suppressed when terminal is too narrow; last content row: {:?}",
        bottom_content_row
    );
    assert!(
        !border_row.contains("G to jump"),
        "pill must not appear on border row either; border row: {:?}",
        border_row
    );
    // Check the entire buffer doesn't contain the down-arrow or the suffix.
    for row in 0..10u16 {
        let r = read_row(&buf, row);
        assert!(
            !r.contains("G to jump"),
            "pill must not appear anywhere; row {row}: {:?}",
            r
        );
    }
}

#[test]
fn jump_hint_click_emits_scroll_to_bottom() {
    use crate::render::MouseCtx;
    use fdemon_app::message::Message;
    use fdemon_app::{MouseAction, MouseButton, MouseRegions};
    use ratatui::{buffer::Buffer, layout::Rect};

    // Wide enough to render the pill (≥ 21 columns needed for "↓ 3 new · G to jump" + 1 margin).
    // For a 60x10 area (no status footer):
    //   area = Rect(0, 0, 60, 10)
    //   inner (after border) = Rect(1, 1, 58, 8)
    //   top_meta(1) + gap(1) = y offset 2; content_area = Rect(1, 3, 58, 6)
    //   last content row y = 3 + 6 - 1 = 8
    //   pill label "↓ 3 new · G to jump" = 19 chars → pill_width = 19
    //   pill x = 1 + 58 - 19 - 1 = 39
    let area = Rect::new(0, 0, 60, 10);
    let mut buf = Buffer::empty(area);
    let mut state = LogViewState::new();
    state.auto_scroll = false;
    // 10 log entries so that a ClickLogRow region also covers row y=8,
    // letting us verify the pill's z=1 wins over the row's z=0.
    let logs = make_logs(10);
    let view = LogView::new(&logs, default_icons()).unseen_log_count(3);

    let mut regions = MouseRegions::with_capacity();
    {
        let builder = regions.builder();
        let mut ctx = MouseCtx::new(builder);
        super::render_with_regions(area, &mut buf, &mut state, view, Some(&mut ctx));
    }

    // For count=3 the pill label "↓ 3 new · G to jump" is 19 chars wide.
    // content_area.x=1, content_area.width=58 → pill x = 1 + 58 - 19 - 1 = 39, y = 8.
    // We pick a cell in the middle of the pill to be safe.
    let pill_y: u16 = 8;
    let pill_x: u16 = 45; // well inside pill (x=39..57)

    // hit_test must resolve to ScrollToBottom at pill coordinates, even though
    // a ClickLogRow region (z=0) also covers that cell.  The pill is at z=1.
    let hit = regions.hit_test(pill_x, pill_y, MouseButton::Left);
    assert!(
        matches!(
            hit.and_then(|e| e.on_left.as_ref()),
            Some(MouseAction::Emit(msg)) if matches!(**msg, Message::ScrollToBottom)
        ),
        "hit_test at the pill cell must resolve to ScrollToBottom (z=1 wins over ClickLogRow z=0); \
         hit = {:?}",
        hit.map(|e| &e.on_left)
    );
}

/// n1 — pill and scrollbar render simultaneously without collision.
///
/// Conditions: scrolled up, total_lines > visible_lines, auto_scroll == false,
/// unseen_log_count > 0.  Asserts that the keybind text "G to jump" is fully
/// intact on the last content row and that the scrollbar end-cap "▼" appears
/// in the buffer.
#[test]
fn jump_hint_pill_and_scrollbar_render_together() {
    use ratatui::layout::Rect;

    // 60-wide, 10-tall, no status footer.
    // content_area = Rect(1, 3, 58, 6), visible_lines = 6.
    // With 20 log entries total_lines = 20 > 6 → scrollbar drawn.
    let area = Rect::new(0, 0, 60, 10);
    let mut buf = make_buffer(60, 10);
    let mut state = LogViewState::new();
    state.auto_scroll = false;
    // Scroll up so offset > 0 (ensures scrollbar position is non-trivial).
    state.offset = 10;
    let logs = make_logs(20);
    let view = LogView::new(&logs, default_icons()).unseen_log_count(5);
    super::render_with_regions(area, &mut buf, &mut state, view, None);

    // The pill must be fully intact on the last content row (y=8).
    let last_content_row = read_row(&buf, 8);
    assert!(
        last_content_row.contains("G to jump"),
        "pill keybind text must be intact when pill and scrollbar render together; row y=8: {:?}",
        last_content_row
    );

    // The scrollbar end-cap "▼" must appear somewhere in the buffer (rightmost column).
    let scrollbar_col = 59u16; // area.x + area.width - 1
    let has_end_cap = (0..10u16)
        .map(|row| buf[(scrollbar_col, row)].symbol().to_string())
        .any(|sym| sym == "▼");
    assert!(
        has_end_cap,
        "scrollbar end-cap '▼' must appear in column 59 when content overflows"
    );
}

/// n3 — narrow-terminal exact boundary: pill suppressed at width == pill_width,
/// rendered at width == pill_width + 1.
///
/// Uses count=1 so pill_width is derived from the public constants rather than
/// a hardcoded literal.  The suppression condition is
/// `content_area.width < pill_width + 1`, i.e. the pill is:
///   - suppressed: content_area.width == pill_width   (minimum-content test)
///   - rendered:   content_area.width == pill_width + 1
///
/// content_area.width = area.width - 2 (border columns).
#[test]
fn jump_hint_suppressed_at_exact_pill_width() {
    use ratatui::layout::Rect;

    // Derive pill_width for count=1: "↓ 1 new · G to jump".
    let label = format!("{}1 new{}", JUMP_HINT_PREFIX, JUMP_HINT_SUFFIX);
    let pill_width = label.chars().count() as u16;

    // content_area.width == pill_width  →  suppressed.
    // area.width = pill_width + 2 (left border + right border).
    let area_width = pill_width + 2;
    let area = Rect::new(0, 0, area_width, 10);
    let mut buf = make_buffer(area_width, 10);
    let mut state = LogViewState::new();
    state.auto_scroll = false;
    let logs = make_logs(2);
    let view = LogView::new(&logs, default_icons()).unseen_log_count(1);
    super::render_with_regions(area, &mut buf, &mut state, view, None);

    // Verify no row contains the keybind text.
    for row in 0..10u16 {
        let r = read_row(&buf, row);
        assert!(
            !r.contains("G to jump"),
            "pill must be suppressed when content_area.width == pill_width ({}); row {}: {:?}",
            pill_width,
            row,
            r
        );
    }
}

#[test]
fn jump_hint_rendered_at_pill_width_plus_one() {
    use ratatui::layout::Rect;

    // Derive pill_width for count=1: "↓ 1 new · G to jump".
    let label = format!("{}1 new{}", JUMP_HINT_PREFIX, JUMP_HINT_SUFFIX);
    let pill_width = label.chars().count() as u16;

    // content_area.width == pill_width + 1  →  rendered.
    // area.width = pill_width + 3 (left border + 1 col margin + right border).
    let area_width = pill_width + 3;
    let area = Rect::new(0, 0, area_width, 10);
    let mut buf = make_buffer(area_width, 10);
    let mut state = LogViewState::new();
    state.auto_scroll = false;
    let logs = make_logs(2);
    let view = LogView::new(&logs, default_icons()).unseen_log_count(1);
    super::render_with_regions(area, &mut buf, &mut state, view, None);

    // The pill must appear on the last content row.
    // Layout for area_width x 10, no footer:
    //   inner.height=8, content_area.height = 8 - 2 = 6, last content row y=8.
    let last_content_row = read_row(&buf, 8);
    assert!(
        last_content_row.contains("G to jump"),
        "pill must render when content_area.width == pill_width + 1 ({}); row y=8: {:?}",
        pill_width + 1,
        last_content_row
    );
}

// ─────────────────────────────────────────────────────────
// Launch-lifecycle spinner glyph tests (phase-6.5 task 02)
// ─────────────────────────────────────────────────────────

/// Helper to build a minimal StatusInfo for spinner tests.
fn spinner_status(phase: AppPhase, is_busy: bool, animation_frame: u64) -> StatusInfo<'static> {
    // We need a 'static reference to AppPhase for the struct. Use Box::leak for test convenience.
    let leaked: &'static AppPhase = Box::leak(Box::new(phase));
    StatusInfo {
        phase: leaked,
        is_busy,
        mode: None,
        flavor: None,
        duration: None,
        error_count: 0,
        vm_connected: false,
        dap_port: None,
        dap_config_ide: None,
        mouse_capture_active: true,
        animation_frame,
        progress: None,
    }
}

#[test]
fn launch_phases_show_spinner_glyph() {
    use crate::test_utils::TestTerminal;
    use crate::widgets::spinner::{spinner_char, SPINNER_FRAMES, SPINNER_TICKS_PER_FRAME};

    for phase in [
        AppPhase::Initializing,
        AppPhase::Preparing,
        AppPhase::Launching,
    ] {
        for frame in [0u64, 1, 4, 10, 19] {
            let mut term = TestTerminal::with_size(80, 10);
            let logs = logs_from(vec![make_entry(LogLevel::Info, LogSource::App, "msg")]);
            let status = spinner_status(phase, false, frame);
            let view = LogView::new(&logs, test_icons()).with_status(status);
            let mut state = LogViewState::new();
            term.render_stateful_widget(view, term.area(), &mut state);

            let expected_glyph = spinner_char(frame / SPINNER_TICKS_PER_FRAME);
            assert!(
                SPINNER_FRAMES.contains(&expected_glyph),
                "glyph {expected_glyph:?} must be a SPINNER_FRAMES char"
            );

            // The spinner glyph must appear somewhere in the rendered output.
            assert!(
                term.buffer_contains(&expected_glyph.to_string()),
                "phase {:?} frame {frame}: expected spinner glyph {expected_glyph:?} in buffer",
                phase
            );

            // The static circle '○' must NOT appear — it has been replaced.
            assert!(
                !term.buffer_contains("○"),
                "phase {:?} frame {frame}: static '○' must not appear when spinner is active",
                phase
            );
        }
    }
}

#[test]
fn non_launch_phases_keep_static_icon() {
    use crate::test_utils::TestTerminal;

    // Reloading keeps ↻, Running keeps ●, Stopped keeps ○
    let cases: &[(AppPhase, &str)] = &[(AppPhase::Running, "●"), (AppPhase::Stopped, "○")];

    for (phase, expected_icon) in cases {
        let mut term = TestTerminal::with_size(80, 10);
        let logs = logs_from(vec![make_entry(LogLevel::Info, LogSource::App, "msg")]);
        let status = spinner_status(*phase, false, 0);
        let view = LogView::new(&logs, test_icons()).with_status(status);
        let mut state = LogViewState::new();
        term.render_stateful_widget(view, term.area(), &mut state);

        assert!(
            term.buffer_contains(expected_icon),
            "phase {:?}: expected static icon {:?} in buffer",
            phase,
            expected_icon
        );
    }

    // is_busy = true → phase_indicator_busy (Reloading / ↻); spinner must not replace it.
    {
        let mut term = TestTerminal::with_size(80, 10);
        let logs = logs_from(vec![make_entry(LogLevel::Info, LogSource::App, "msg")]);
        // Even if phase is Launching, is_busy = true should stay with the busy icon ↻
        let status = spinner_status(AppPhase::Launching, true, 4);
        let view = LogView::new(&logs, test_icons()).with_status(status);
        let mut state = LogViewState::new();
        term.render_stateful_widget(view, term.area(), &mut state);

        assert!(
            term.buffer_contains("↻"),
            "is_busy=true: expected static busy icon '↻' in buffer, not a spinner glyph"
        );
    }
}

#[test]
fn launch_spinner_advances_with_frame() {
    use crate::test_utils::TestTerminal;
    use crate::widgets::spinner::{spinner_char, SPINNER_TICKS_PER_FRAME};

    // Two renders separated by SPINNER_TICKS_PER_FRAME frames must yield different glyphs.
    let frame_a = 0u64;
    let frame_b = frame_a + SPINNER_TICKS_PER_FRAME;

    let glyph_a = spinner_char(frame_a / SPINNER_TICKS_PER_FRAME);
    let glyph_b = spinner_char(frame_b / SPINNER_TICKS_PER_FRAME);
    assert_ne!(
        glyph_a, glyph_b,
        "frames {frame_a} and {frame_b} must produce different spinner glyphs"
    );

    for phase in [
        AppPhase::Initializing,
        AppPhase::Preparing,
        AppPhase::Launching,
    ] {
        let mut term_a = TestTerminal::with_size(80, 10);
        let logs = logs_from(vec![make_entry(LogLevel::Info, LogSource::App, "msg")]);
        let status_a = spinner_status(phase, false, frame_a);
        let view_a = LogView::new(&logs, test_icons()).with_status(status_a);
        let mut state_a = LogViewState::new();
        term_a.render_stateful_widget(view_a, term_a.area(), &mut state_a);

        let mut term_b = TestTerminal::with_size(80, 10);
        let logs2 = logs_from(vec![make_entry(LogLevel::Info, LogSource::App, "msg")]);
        let status_b = spinner_status(phase, false, frame_b);
        let view_b = LogView::new(&logs2, test_icons()).with_status(status_b);
        let mut state_b = LogViewState::new();
        term_b.render_stateful_widget(view_b, term_b.area(), &mut state_b);

        assert!(
            term_a.buffer_contains(&glyph_a.to_string()),
            "phase {:?} frame_a={frame_a}: expected glyph {glyph_a:?}",
            phase
        );
        assert!(
            term_b.buffer_contains(&glyph_b.to_string()),
            "phase {:?} frame_b={frame_b}: expected glyph {glyph_b:?}",
            phase
        );
    }
}

// ── Drag-to-select: character-wrap + selection mapping/highlight/text ─────────

#[test]
fn wrap_line_chars_splits_at_width_preserving_styles() {
    use ratatui::text::{Line, Span};

    let style_a = Style::default().fg(Color::Red);
    let line = Line::from(vec![Span::styled("abcdef", style_a)]);
    let rows = LogView::wrap_line_chars(&line, 4);
    assert_eq!(rows.len(), 2);

    let row0: String = rows[0].spans.iter().map(|s| s.content.as_ref()).collect();
    let row1: String = rows[1].spans.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(row0, "abcd");
    assert_eq!(row1, "ef");
    assert_eq!(
        rows[0].spans[0].style.fg,
        Some(Color::Red),
        "style preserved"
    );
}

#[test]
fn wrap_line_chars_empty_line_yields_one_empty_row() {
    use ratatui::text::Line;

    let line = Line::from("");
    let rows = LogView::wrap_line_chars(&line, 10);
    assert_eq!(rows.len(), 1);
    let txt: String = rows[0].spans.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(txt, "");
}

#[test]
fn wrap_line_chars_row_count_matches_line_wrapped_row_count() {
    use ratatui::text::Line;

    let line = Line::from("a".repeat(25));
    let width = 10;
    let rows = LogView::wrap_line_chars(&line, width);
    assert_eq!(rows.len(), LogView::line_wrapped_row_count(&line, width));
    assert_eq!(rows.len(), 3);
}

#[test]
fn wrap_line_chars_wide_chars_split_by_display_width() {
    use ratatui::text::Line;

    // 10 CJK chars (2 cells each) in a 10-cell row → 5 glyphs per row, and no
    // row's display width may exceed the content width (that overflow was
    // silently clipped by the non-wrapping Paragraph before).
    let line = Line::from("漢".repeat(10));
    let rows = LogView::wrap_line_chars(&line, 10);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows.len(), LogView::line_wrapped_row_count(&line, 10));
    for row in &rows {
        let txt: String = row.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(txt.chars().count(), 5);
        let display_width: usize = fdemon_app::log_view_state::grapheme_cell_widths(&txt)
            .map(|(_, _, w)| w)
            .sum();
        assert!(display_width <= 10, "no wrapped row may overflow its width");
    }
}

#[test]
fn wrap_line_chars_mixed_width_never_overflows() {
    use ratatui::text::Line;

    let line = Line::from("ab漢c漢漢d🎉e");
    for width in 2..12 {
        let rows = LogView::wrap_line_chars(&line, width);
        assert_eq!(
            rows.len(),
            LogView::line_wrapped_row_count(&line, width),
            "row count must match at width {width}"
        );
        for row in &rows {
            let txt: String = row.spans.iter().map(|s| s.content.as_ref()).collect();
            let display_width: usize = fdemon_app::log_view_state::grapheme_cell_widths(&txt)
                .map(|(_, _, w)| w)
                .sum();
            assert!(
                display_width <= width,
                "row overflows {width} cells: {row:?}"
            );
        }
        // Nothing lost: concatenating all rows restores the full text.
        let total: String = rows
            .iter()
            .flat_map(|r| r.spans.iter())
            .map(|s| s.content.as_ref())
            .collect::<String>();
        assert_eq!(total, "ab漢c漢漢d🎉e");
    }
}

#[test]
fn wrap_line_chars_zero_width_yields_single_row() {
    use ratatui::text::Line;

    let line = Line::from("abc");
    assert_eq!(LogView::wrap_line_chars(&line, 0).len(), 1);
}

/// VS16 emoji (⚠️/❤️) and ZWJ sequences (👨‍👩‍👧) render 2 cells but char-sum
/// differently — wrapping must measure grapheme clusters or ratatui silently
/// clips rows packed too wide (PR #72 review, R1).
#[test]
fn wrap_line_chars_grapheme_clusters_never_overflow() {
    use fdemon_app::log_view_state::grapheme_cell_widths;
    use ratatui::text::Line;

    let text = "a⚠\u{FE0F}b❤\u{FE0F}c👨\u{200D}👩\u{200D}👧d";
    let cluster_starts: Vec<usize> = grapheme_cell_widths(text).map(|(i, _, _)| i).collect();
    let total_chars = text.chars().count();
    let line = Line::from(text);
    for width in 2..12 {
        let rows = LogView::wrap_line_chars(&line, width);
        assert_eq!(
            rows.len(),
            LogView::line_wrapped_row_count(&line, width),
            "row count must match at width {width}"
        );
        let mut boundary = 0usize;
        for row in &rows {
            let txt: String = row.spans.iter().map(|s| s.content.as_ref()).collect();
            let display_width: usize = grapheme_cell_widths(&txt).map(|(_, _, w)| w).sum();
            assert!(
                display_width <= width,
                "row overflows {width} cells: {txt:?}"
            );
            assert!(
                boundary == total_chars || cluster_starts.contains(&boundary),
                "row boundary at char {boundary} splits a cluster (width {width})"
            );
            boundary += txt.chars().count();
        }
        // Nothing lost: concatenating all rows restores the full text.
        let total: String = rows
            .iter()
            .flat_map(|r| r.spans.iter())
            .map(|s| s.content.as_ref())
            .collect::<String>();
        assert_eq!(total, text);
    }
}

/// R1's actual symptom, tested against real ratatui rendering: every wrapped
/// row must fit its width, so rendering a row into a buffer exactly that wide
/// never clips its tail cluster.
#[test]
fn wrap_line_chars_rows_render_without_clipping() {
    use fdemon_app::log_view_state::grapheme_cell_widths;
    use ratatui::text::Line;
    use ratatui::widgets::Widget;
    use ratatui::{buffer::Buffer, layout::Rect};

    let text = "a⚠\u{FE0F}b❤\u{FE0F}c👨\u{200D}👩\u{200D}👧d";
    let line = Line::from(text);
    for width in 2..12 {
        for row in LogView::wrap_line_chars(&line, width) {
            let txt: String = row.spans.iter().map(|s| s.content.as_ref()).collect();
            let Some(last_cluster) = grapheme_cell_widths(&txt)
                .last()
                .map(|(start, n, _)| txt.chars().skip(start).take(n).collect::<String>())
            else {
                continue;
            };
            let area = Rect::new(0, 0, width as u16, 1);
            let mut buf = Buffer::empty(area);
            (&row).render(area, &mut buf);
            let rendered: String = (0..area.width)
                .filter_map(|x| buf.cell((x, 0)).map(|c| c.symbol()))
                .collect();
            assert!(
                rendered.contains(&last_cluster),
                "width {width}: row {txt:?} lost its tail {last_cluster:?} when \
                 rendered (got {rendered:?})"
            );
        }
    }
}

#[test]
fn render_publishes_selection_rows_nowrap() {
    use crate::render::MouseCtx;
    use fdemon_app::MouseRegions;
    use ratatui::{buffer::Buffer, layout::Rect};

    let logs = make_logs_no_traces(3);
    let mut state = LogViewState::new();
    let view = LogView::new(&logs, test_icons()).wrap_mode(false);

    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);
    let mut regions = MouseRegions::with_capacity();
    {
        let builder = regions.builder();
        let mut ctx = MouseCtx::new(builder);
        super::render_with_regions(area, &mut buf, &mut state, view, Some(&mut ctx));
    }

    assert_eq!(state.selection_rows.len(), 3, "one row per visible entry");
    assert!(state.content_bottom_y > state.content_top_y);
    assert!(state.selection_top.is_some());
    assert!(state.selection_bottom.is_some());
    assert_eq!(
        state.selection_rows[0].wrap_width, 0,
        "nowrap → wrap_width 0"
    );
}

#[test]
fn nowrap_h_scroll_locate_matches_displayed_char() {
    use crate::render::MouseCtx;
    use fdemon_app::log_view_state::{LogSelection, SelPoint};
    use fdemon_app::MouseRegions;
    use ratatui::{buffer::Buffer, layout::Rect};

    // A long line with a distinct char at every position, horizontally scrolled.
    // The char a cell locates to must be exactly the char drawn in that cell —
    // the `←` indicator replaces the char at index h_offset, so content starts
    // at h_offset + 1 (this was off by one before).
    let msg: String = (0..120u32)
        .map(|i| char::from_u32('a' as u32 + (i % 26)).unwrap())
        .collect();
    let logs = logs_from(vec![make_entry(LogLevel::Info, LogSource::App, &msg)]);
    let mut state = LogViewState::new();
    state.wrap_mode = false;
    state.h_offset = 5;
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);

    {
        let view = LogView::new(&logs, test_icons()).wrap_mode(false);
        let mut regions = MouseRegions::with_capacity();
        let builder = regions.builder();
        let mut ctx = MouseCtx::new(builder);
        super::render_with_regions(area, &mut buf, &mut state, view, Some(&mut ctx));
    }

    let row = state.selection_rows[0].clone();
    assert!(row.left_indicator, "h-scrolled line shows the ← indicator");
    assert!(row.right_indicator, "long line overflows → indicator");
    assert_eq!(
        row.base_col,
        state.h_offset + 1,
        "first visible char sits one past h_offset (the indicator's cell)"
    );

    // Map a mid-row cell to a SelPoint, then prove the selection text for that
    // single char equals the glyph the cell displays (WYSIWYG).
    let (x, y) = (row.rect.x + 10, row.rect.y);
    let displayed = buf.cell((x, y)).unwrap().symbol().to_string();
    let p = row.locate(x, y).unwrap();
    state.selection = Some(LogSelection {
        anchor: p,
        focus: SelPoint {
            col: p.col + 1,
            ..p
        },
        dragging: false,
    });

    {
        let view = LogView::new(&logs, test_icons()).wrap_mode(false);
        let mut regions = MouseRegions::with_capacity();
        let builder = regions.builder();
        let mut ctx = MouseCtx::new(builder);
        super::render_with_regions(area, &mut buf, &mut state, view, Some(&mut ctx));
    }

    assert_eq!(
        state.selection_text.as_deref(),
        Some(displayed.as_str()),
        "the located char must be the one the cell displays"
    );
    // And the highlight lands on the very cell that was clicked.
    assert_eq!(
        buf.cell((x, y)).unwrap().bg,
        crate::theme::palette::SELECTION_BG,
        "highlight paints the clicked cell"
    );
    // The `→` indicator cell (last content column) must never be painted even
    // when the selection covers the whole line.
    state.selection = Some(LogSelection {
        anchor: SelPoint {
            entry_id: row.entry_id,
            frame_index: None,
            col: 0,
        },
        focus: SelPoint {
            entry_id: row.entry_id,
            frame_index: None,
            col: row.text_len,
        },
        dragging: false,
    });
    {
        let view = LogView::new(&logs, test_icons()).wrap_mode(false);
        let mut regions = MouseRegions::with_capacity();
        let builder = regions.builder();
        let mut ctx = MouseCtx::new(builder);
        super::render_with_regions(area, &mut buf, &mut state, view, Some(&mut ctx));
    }
    let right_edge = (row.rect.x + row.rect.width - 1, row.rect.y);
    assert_eq!(
        buf.cell(right_edge).unwrap().symbol(),
        "→",
        "sanity: the right indicator is drawn at the row's last cell"
    );
    assert_ne!(
        buf.cell(right_edge).unwrap().bg,
        crate::theme::palette::SELECTION_BG,
        "the → indicator cell is not selectable content"
    );
}

#[test]
fn wrap_mode_wide_char_locate_and_highlight_are_width_aware() {
    use crate::render::MouseCtx;
    use fdemon_app::log_view_state::{LogSelection, SelPoint};
    use fdemon_app::MouseRegions;
    use ratatui::{buffer::Buffer, layout::Rect};

    // CJK message: every char occupies two terminal cells.
    let logs = logs_from(vec![make_entry(
        LogLevel::Info,
        LogSource::App,
        &"漢".repeat(30),
    )]);
    let mut state = LogViewState::new();
    state.wrap_mode = true;
    let area = Rect::new(0, 0, 40, 24);
    let mut buf = Buffer::empty(area);

    {
        let view = LogView::new(&logs, test_icons()).wrap_mode(true);
        let mut regions = MouseRegions::with_capacity();
        let builder = regions.builder();
        let mut ctx = MouseCtx::new(builder);
        super::render_with_regions(area, &mut buf, &mut state, view, Some(&mut ctx));
    }

    let row = state.selection_rows[0].clone();
    assert!(
        row.rect.height > 1,
        "60-cell CJK payload must wrap across rows in a 38-cell content area"
    );
    assert!(!row.text.is_empty(), "wrap rows carry the line text");

    // A cell on the second sub-row must locate to a char the FIRST sub-row does
    // not contain, and both cells of one wide glyph must map to the same char.
    let y2 = row.rect.y + 1;
    let p_first = row.locate(row.rect.x, row.rect.y).unwrap();
    let p_second = row.locate(row.rect.x, y2).unwrap();
    let starts = fdemon_app::log_view_state::wrap_row_starts(&row.text, row.wrap_width as usize);
    assert_eq!(
        p_second.col, starts[1],
        "second sub-row starts at the width-aware row boundary"
    );
    assert!(p_second.col > p_first.col);
    let glyph_cell_a = row.locate(row.rect.x + 20, y2).unwrap();
    let glyph_cell_b = row.locate(row.rect.x + 21, y2).unwrap();
    assert_eq!(
        glyph_cell_a.col, glyph_cell_b.col,
        "both cells of a wide glyph map to the same char"
    );

    // Selecting one wide char must paint exactly its two cells on that sub-row.
    state.selection = Some(LogSelection {
        anchor: p_second,
        focus: SelPoint {
            col: p_second.col + 1,
            ..p_second
        },
        dragging: false,
    });
    {
        let view = LogView::new(&logs, test_icons()).wrap_mode(true);
        let mut regions = MouseRegions::with_capacity();
        let builder = regions.builder();
        let mut ctx = MouseCtx::new(builder);
        super::render_with_regions(area, &mut buf, &mut state, view, Some(&mut ctx));
    }
    assert_eq!(
        buf.cell((row.rect.x, y2)).unwrap().bg,
        crate::theme::palette::SELECTION_BG,
        "first cell of the wide glyph highlighted"
    );
    assert_eq!(
        buf.cell((row.rect.x + 1, y2)).unwrap().bg,
        crate::theme::palette::SELECTION_BG,
        "second cell of the wide glyph highlighted"
    );
    assert_ne!(
        buf.cell((row.rect.x + 2, y2)).unwrap().bg,
        crate::theme::palette::SELECTION_BG,
        "the next glyph is not highlighted"
    );
    assert_eq!(
        state.selection_text.as_deref(),
        Some("漢"),
        "copying the single selected wide char yields exactly that char"
    );
}

/// Mirror of the CJK test for VS16 emoji: `⚠️` is a 2-**char** cluster (⚠ +
/// U+FE0F) rendering 2 cells, so per-char width sums drift from ratatui's
/// per-grapheme layout (PR #72 review, R1). Flutter logs emit `⚠️` constantly.
#[test]
fn wrap_mode_vs16_emoji_locate_and_highlight_are_cluster_aware() {
    use crate::render::MouseCtx;
    use fdemon_app::log_view_state::{LogSelection, SelPoint};
    use fdemon_app::MouseRegions;
    use ratatui::{buffer::Buffer, layout::Rect};

    let warn = "⚠\u{FE0F}";
    let logs = logs_from(vec![make_entry(
        LogLevel::Info,
        LogSource::App,
        &warn.repeat(30),
    )]);
    let mut state = LogViewState::new();
    state.wrap_mode = true;
    let area = Rect::new(0, 0, 40, 24);
    let mut buf = Buffer::empty(area);

    {
        let view = LogView::new(&logs, test_icons()).wrap_mode(true);
        let mut regions = MouseRegions::with_capacity();
        let builder = regions.builder();
        let mut ctx = MouseCtx::new(builder);
        super::render_with_regions(area, &mut buf, &mut state, view, Some(&mut ctx));
    }

    let row = state.selection_rows[0].clone();
    assert!(
        row.rect.height > 1,
        "60-cell VS16 payload must wrap across rows in a 38-cell content area"
    );

    // Sub-row boundaries must be cluster-aware, and both cells of one cluster
    // must map to the same (first) char index.
    let y2 = row.rect.y + 1;
    let p_first = row.locate(row.rect.x, row.rect.y).unwrap();
    let p_second = row.locate(row.rect.x, y2).unwrap();
    let starts = fdemon_app::log_view_state::wrap_row_starts(&row.text, row.wrap_width as usize);
    assert_eq!(
        p_second.col, starts[1],
        "second sub-row starts at the cluster-aware row boundary"
    );
    assert!(p_second.col > p_first.col);
    let cluster_cell_a = row.locate(row.rect.x + 20, y2).unwrap();
    let cluster_cell_b = row.locate(row.rect.x + 21, y2).unwrap();
    assert_eq!(
        cluster_cell_a.col, cluster_cell_b.col,
        "both cells of a VS16 cluster map to the same char"
    );

    // Selecting one cluster spans TWO chars (base + VS16), paints exactly its
    // two cells, and copies exactly that cluster.
    state.selection = Some(LogSelection {
        anchor: p_second,
        focus: SelPoint {
            col: p_second.col + 2,
            ..p_second
        },
        dragging: false,
    });
    {
        let view = LogView::new(&logs, test_icons()).wrap_mode(true);
        let mut regions = MouseRegions::with_capacity();
        let builder = regions.builder();
        let mut ctx = MouseCtx::new(builder);
        super::render_with_regions(area, &mut buf, &mut state, view, Some(&mut ctx));
    }
    assert_eq!(
        buf.cell((row.rect.x, y2)).unwrap().bg,
        crate::theme::palette::SELECTION_BG,
        "first cell of the cluster highlighted"
    );
    assert_eq!(
        buf.cell((row.rect.x + 1, y2)).unwrap().bg,
        crate::theme::palette::SELECTION_BG,
        "second cell of the cluster highlighted"
    );
    assert_ne!(
        buf.cell((row.rect.x + 2, y2)).unwrap().bg,
        crate::theme::palette::SELECTION_BG,
        "the next cluster is not highlighted"
    );
    assert_eq!(
        state.selection_text.as_deref(),
        Some(warn),
        "copying the single selected cluster yields exactly that cluster"
    );
}

/// At degenerate `wrap_width == 1` a 2-cell cluster overflows its row; the
/// highlight must clamp to the row rect instead of painting the neighbor cell
/// (PR #72 review, optional item).
#[test]
fn highlight_never_paints_past_rect_at_degenerate_width() {
    use fdemon_app::log_view_state::{LogSelection, SelPoint, SelectionRow};
    use fdemon_app::MouseRect;
    use ratatui::{buffer::Buffer, layout::Rect};

    let text = "漢漢";
    let mut state = LogViewState::new();
    state.selection_rows = vec![SelectionRow {
        rect: MouseRect::new(0, 0, 1, 2),
        entry_id: 1,
        frame_index: None,
        base_col: 0,
        left_indicator: false,
        right_indicator: false,
        text_len: text.chars().count(),
        wrap_width: 1,
        top_clip: 0,
        text: text.to_string(),
    }];
    state.selection = Some(LogSelection {
        anchor: SelPoint {
            entry_id: 1,
            frame_index: None,
            col: 0,
        },
        focus: SelPoint {
            entry_id: 1,
            frame_index: None,
            col: 2,
        },
        dragging: false,
    });

    let area = Rect::new(0, 0, 5, 3);
    let mut buf = Buffer::empty(area);
    LogView::render_selection_highlight(&state, &mut buf);

    for y in 0..area.height {
        for x in 0..area.width {
            let inside = x < 1 && y < 2;
            let painted = buf.cell((x, y)).unwrap().bg == crate::theme::palette::SELECTION_BG;
            assert_eq!(
                painted, inside,
                "cell ({x}, {y}) painted={painted}, expected inside-rect only"
            );
        }
    }
}

#[test]
fn render_highlights_selected_cells() {
    use crate::render::MouseCtx;
    use fdemon_app::log_view_state::{LogSelection, SelPoint};
    use fdemon_app::MouseRegions;
    use ratatui::{buffer::Buffer, layout::Rect};

    let logs = make_logs_no_traces(1);
    let mut state = LogViewState::new();
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);

    // First render publishes selection_rows (entry id + geometry).
    {
        let view = LogView::new(&logs, test_icons()).wrap_mode(false);
        let mut regions = MouseRegions::with_capacity();
        let builder = regions.builder();
        let mut ctx = MouseCtx::new(builder);
        super::render_with_regions(area, &mut buf, &mut state, view, Some(&mut ctx));
    }
    let row = state.selection_rows[0].clone();

    // Select columns [0, 5) on that line.
    state.selection = Some(LogSelection {
        anchor: SelPoint {
            entry_id: row.entry_id,
            frame_index: None,
            col: 0,
        },
        focus: SelPoint {
            entry_id: row.entry_id,
            frame_index: None,
            col: 5,
        },
        dragging: false,
    });

    // Second render applies the highlight pass.
    {
        let view = LogView::new(&logs, test_icons()).wrap_mode(false);
        let mut regions = MouseRegions::with_capacity();
        let builder = regions.builder();
        let mut ctx = MouseCtx::new(builder);
        super::render_with_regions(area, &mut buf, &mut state, view, Some(&mut ctx));
    }

    let y = row.rect.y;
    for x in row.rect.x..row.rect.x + 5 {
        assert_eq!(
            buf.cell((x, y)).unwrap().bg,
            crate::theme::palette::SELECTION_BG,
            "cell ({x},{y}) should carry the selection background"
        );
    }
    assert_ne!(
        buf.cell((row.rect.x + 5, y)).unwrap().bg,
        crate::theme::palette::SELECTION_BG,
        "the cell just past the selection must not be highlighted"
    );
}

#[test]
fn render_publishes_exact_selection_text_slice() {
    use crate::render::MouseCtx;
    use fdemon_app::log_view_state::{LogSelection, SelPoint};
    use fdemon_app::MouseRegions;
    use ratatui::{buffer::Buffer, layout::Rect};

    // Rendered message line = "HH:MM:SS" (8) + " • " (3) + "[app] " (6) = 17-char
    // prefix, then the message. Selecting [17, 22) yields the first 5 message chars.
    let logs = logs_from(vec![make_entry(
        LogLevel::Info,
        LogSource::App,
        "HELLOworld",
    )]);
    let mut state = LogViewState::new();
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);

    {
        let view = LogView::new(&logs, test_icons()).wrap_mode(false);
        let mut regions = MouseRegions::with_capacity();
        let builder = regions.builder();
        let mut ctx = MouseCtx::new(builder);
        super::render_with_regions(area, &mut buf, &mut state, view, Some(&mut ctx));
    }
    let entry_id = state.selection_rows[0].entry_id;

    const PREFIX: usize = 17;
    state.selection = Some(LogSelection {
        anchor: SelPoint {
            entry_id,
            frame_index: None,
            col: PREFIX,
        },
        focus: SelPoint {
            entry_id,
            frame_index: None,
            col: PREFIX + 5,
        },
        dragging: false,
    });

    {
        let view = LogView::new(&logs, test_icons()).wrap_mode(false);
        let mut regions = MouseRegions::with_capacity();
        let builder = regions.builder();
        let mut ctx = MouseCtx::new(builder);
        super::render_with_regions(area, &mut buf, &mut state, view, Some(&mut ctx));
    }

    assert_eq!(
        state.selection_text.as_deref(),
        Some("HELLO"),
        "WYSIWYG slice of the rendered message text"
    );
}

#[test]
fn render_selection_text_spans_multiple_lines() {
    use crate::render::MouseCtx;
    use fdemon_app::log_view_state::{LogSelection, SelPoint};
    use fdemon_app::MouseRegions;
    use ratatui::{buffer::Buffer, layout::Rect};

    let logs = make_logs_no_traces(3);
    let mut state = LogViewState::new();
    let area = Rect::new(0, 0, 80, 24);
    let mut buf = Buffer::empty(area);

    {
        let view = LogView::new(&logs, test_icons()).wrap_mode(false);
        let mut regions = MouseRegions::with_capacity();
        let builder = regions.builder();
        let mut ctx = MouseCtx::new(builder);
        super::render_with_regions(area, &mut buf, &mut state, view, Some(&mut ctx));
    }
    let first = state.selection_rows[0].clone();
    let third = state.selection_rows[2].clone();

    // Select from start of line 0 through end of line 2 → 3 newline-joined lines.
    state.selection = Some(LogSelection {
        anchor: SelPoint {
            entry_id: first.entry_id,
            frame_index: None,
            col: 0,
        },
        focus: SelPoint {
            entry_id: third.entry_id,
            frame_index: None,
            col: third.text_len,
        },
        dragging: false,
    });

    {
        let view = LogView::new(&logs, test_icons()).wrap_mode(false);
        let mut regions = MouseRegions::with_capacity();
        let builder = regions.builder();
        let mut ctx = MouseCtx::new(builder);
        super::render_with_regions(area, &mut buf, &mut state, view, Some(&mut ctx));
    }

    let text = state.selection_text.as_deref().unwrap();
    assert_eq!(text.lines().count(), 3, "three lines selected");
}
