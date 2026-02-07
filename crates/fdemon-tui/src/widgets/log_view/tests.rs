//! Tests for log_view widget module

use super::*;
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
    let view = LogView::new(&logs).show_timestamps(true);
    let line = view.format_entry(&logs[0], 0);

    // Should have multiple spans including timestamp
    assert!(line.spans.len() >= 3);
}

#[test]
fn test_format_entry_no_timestamp() {
    let logs = logs_from(vec![make_entry(LogLevel::Info, LogSource::App, "Test")]);
    let view = LogView::new(&logs).show_timestamps(false);
    let line = view.format_entry(&logs[0], 0);

    // Fewer spans without timestamp
    let with_ts = LogView::new(&logs).show_timestamps(true);
    let line_with = with_ts.format_entry(&logs[0], 0);
    assert!(line.spans.len() < line_with.spans.len());
}

#[test]
fn test_format_entry_no_source() {
    let logs = logs_from(vec![make_entry(LogLevel::Info, LogSource::App, "Test")]);
    let view = LogView::new(&logs).show_source(false);
    let line = view.format_entry(&logs[0], 0);

    // Fewer spans without source
    let with_src = LogView::new(&logs).show_source(true);
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
    let app_style = LogView::source_style(LogSource::App);
    let flutter_style = LogView::source_style(LogSource::Flutter);

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
fn test_build_title_no_filter() {
    let logs = logs_from(vec![make_entry(LogLevel::Info, LogSource::App, "Test")]);
    let view = LogView::new(&logs).title("Logs");
    assert_eq!(view.build_title(), " Logs ");
}

#[test]
fn test_build_title_with_default_filter() {
    let logs = logs_from(vec![make_entry(LogLevel::Info, LogSource::App, "Test")]);
    let filter = FilterState::default();
    let view = LogView::new(&logs).title("Logs").filter_state(&filter);
    // Default filter (All/All) should not show indicator
    assert_eq!(view.build_title(), " Logs ");
}

#[test]
fn test_build_title_with_level_filter() {
    let logs = logs_from(vec![make_entry(LogLevel::Info, LogSource::App, "Test")]);
    let filter = FilterState {
        level_filter: LogLevelFilter::Errors,
        source_filter: LogSourceFilter::All,
    };
    let view = LogView::new(&logs).title("Logs").filter_state(&filter);
    let title = view.build_title();
    assert!(title.contains("Errors only"), "Title was: {}", title);
}

#[test]
fn test_build_title_with_source_filter() {
    let logs = logs_from(vec![make_entry(LogLevel::Info, LogSource::App, "Test")]);
    let filter = FilterState {
        level_filter: LogLevelFilter::All,
        source_filter: LogSourceFilter::App,
    };
    let view = LogView::new(&logs).title("Logs").filter_state(&filter);
    let title = view.build_title();
    assert!(title.contains("App logs"), "Title was: {}", title);
}

#[test]
fn test_build_title_with_combined_filter() {
    let logs = logs_from(vec![make_entry(LogLevel::Info, LogSource::App, "Test")]);
    let filter = FilterState {
        level_filter: LogLevelFilter::Errors,
        source_filter: LogSourceFilter::Flutter,
    };
    let view = LogView::new(&logs).title("Logs").filter_state(&filter);
    let title = view.build_title();
    assert!(title.contains("Errors only"), "Title was: {}", title);
    assert!(title.contains("Flutter logs"), "Title was: {}", title);
    assert!(title.contains(" | "), "Title was: {}", title);
}

#[test]
fn test_filter_state_builder() {
    let logs = logs_from(vec![make_entry(LogLevel::Info, LogSource::App, "Test")]);
    let filter = FilterState::default();
    let view = LogView::new(&logs).filter_state(&filter);
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
    let view = LogView::new(&logs);

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

    let view = LogView::new(&logs).search_state(&search);

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

    let view = LogView::new(&logs).search_state(&search);

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

    let view = LogView::new(&logs).search_state(&search);

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

    let view = LogView::new(&logs).search_state(&search);

    // Invalid regex should not highlight
    let spans = view.format_message_with_highlights("test", 0, Style::default());

    assert_eq!(spans.len(), 1);
}

#[test]
fn test_build_title_with_search_status() {
    let logs = logs_from(vec![
        make_entry(LogLevel::Info, LogSource::App, "test message"),
        make_entry(LogLevel::Info, LogSource::App, "another test"),
    ]);
    let mut search = SearchState::default();
    search.set_query("test");
    search.execute_search(&logs);

    let view = LogView::new(&logs).title("Logs").search_state(&search);

    let title = view.build_title();
    assert!(title.contains("["), "Title was: {}", title);
    assert!(title.contains("2"), "Title was: {}", title);
    assert!(title.contains("matches"), "Title was: {}", title);
}

#[test]
fn test_build_title_with_filter_and_search() {
    let logs = logs_from(vec![make_entry(LogLevel::Info, LogSource::App, "test")]);
    let filter = FilterState {
        level_filter: LogLevelFilter::Errors,
        source_filter: LogSourceFilter::All,
    };
    let mut search = SearchState::default();
    search.set_query("test");
    search.execute_search(&logs);

    let view = LogView::new(&logs)
        .title("Logs")
        .filter_state(&filter)
        .search_state(&search);

    let title = view.build_title();
    // Should contain both filter and search indicators
    assert!(title.contains("Errors"), "Title was: {}", title);
    assert!(title.contains("•"), "Title was: {}", title); // separator
}

#[test]
fn test_search_state_builder() {
    let logs = logs_from(vec![make_entry(LogLevel::Info, LogSource::App, "test")]);
    let search = SearchState::default();
    let view = LogView::new(&logs).search_state(&search);
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

    let view = LogView::new(&logs)
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
    use styles::*;

    assert_eq!(INDENT, "    ");
    assert_eq!(FRAME_NUMBER.fg, Some(Color::DarkGray));
    assert_eq!(FUNCTION_PROJECT.fg, Some(Color::White));
    assert_eq!(FUNCTION_PACKAGE.fg, Some(Color::DarkGray));
    assert_eq!(FILE_PROJECT.fg, Some(Color::Blue));
    assert!(FILE_PROJECT.add_modifier.contains(Modifier::UNDERLINED));
    assert_eq!(LOCATION_PROJECT.fg, Some(Color::Cyan));
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
    let view = LogView::new(&logs)
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
    let view = LogView::new(&logs)
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

    let view = LogView::new(&logs)
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
    let view = LogView::new(&logs)
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
    let view = LogView::new(&logs).default_collapsed(true);
    assert!(!view.is_entry_expanded(&logs[0])); // Collapsed by default

    let view = LogView::new(&logs).default_collapsed(false);
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

    let view = LogView::new(&logs)
        .default_collapsed(true)
        .collapse_state(&collapse_state);

    assert!(view.is_entry_expanded(&logs[0]));
}

#[test]
fn test_collapse_state_builder() {
    let logs: VecDeque<LogEntry> = VecDeque::new();
    let collapse_state = CollapseState::new();

    let view = LogView::new(&logs).collapse_state(&collapse_state);

    assert!(view.collapse_state.is_some());
}

#[test]
fn test_max_collapsed_frames_builder() {
    let logs: VecDeque<LogEntry> = VecDeque::new();

    let view = LogView::new(&logs).max_collapsed_frames(5);

    assert_eq!(view.max_collapsed_frames, 5);
}

#[test]
fn test_default_collapsed_builder() {
    let logs: VecDeque<LogEntry> = VecDeque::new();

    let view = LogView::new(&logs).default_collapsed(false);

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
