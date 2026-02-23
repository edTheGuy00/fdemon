## Task: Add Wrap Mode Tests

**Objective**: Add comprehensive unit tests covering wrap mode state management, horizontal scroll guards, rendering with wrap enabled, and the status indicator.

**Depends on**: 02-wrap-rendering

### Scope

- `crates/fdemon-app/src/log_view_state.rs`: Tests in inline `#[cfg(test)] mod tests`
- `crates/fdemon-app/src/handler/scroll.rs`: Tests for scroll guards (or `handler/tests.rs`)
- `crates/fdemon-tui/src/widgets/log_view/tests.rs`: Rendering tests with `TestTerminal`

### Details

#### 1. LogViewState unit tests

**File:** `crates/fdemon-app/src/log_view_state.rs` — add to existing `#[cfg(test)] mod tests` block

```rust
// --- Wrap mode tests ---

#[test]
fn test_wrap_mode_defaults_to_true() {
    let state = LogViewState::new();
    assert!(state.wrap_mode);
}

#[test]
fn test_toggle_wrap_mode_disables() {
    let mut state = LogViewState::new();
    assert!(state.wrap_mode);  // default true
    state.toggle_wrap_mode();
    assert!(!state.wrap_mode);
}

#[test]
fn test_toggle_wrap_mode_enables_and_resets_h_offset() {
    let mut state = LogViewState::new();
    state.wrap_mode = false;
    state.h_offset = 42;  // simulate horizontal scroll position
    state.toggle_wrap_mode();
    assert!(state.wrap_mode);
    assert_eq!(state.h_offset, 0, "h_offset should reset to 0 when wrap enabled");
}

#[test]
fn test_toggle_wrap_mode_does_not_reset_h_offset_when_disabling() {
    let mut state = LogViewState::new();
    // wrap is on by default, h_offset should be 0
    state.toggle_wrap_mode(); // disable wrap
    assert!(!state.wrap_mode);
    // h_offset stays at whatever it was (0 in this case, but point is no reset)
    assert_eq!(state.h_offset, 0);
}

#[test]
fn test_toggle_wrap_mode_roundtrip() {
    let mut state = LogViewState::new();
    assert!(state.wrap_mode);   // start: wrap on
    state.toggle_wrap_mode();   // wrap off
    assert!(!state.wrap_mode);
    state.toggle_wrap_mode();   // wrap on again
    assert!(state.wrap_mode);
    assert_eq!(state.h_offset, 0);
}
```

#### 2. Horizontal scroll guard tests

**File:** `crates/fdemon-app/src/handler/scroll.rs` — add to existing tests, or add inline test module

These tests verify that horizontal scroll functions are no-ops when wrap mode is enabled. Follow the existing test patterns in `fdemon-app` for handler tests.

```rust
#[test]
fn test_scroll_left_noop_when_wrap_enabled() {
    let mut state = create_test_state_with_session();
    // wrap_mode defaults to true
    let handle = state.session_manager.selected_mut().unwrap();
    handle.session.log_view_state.h_offset = 10;
    handle.session.log_view_state.wrap_mode = true;

    handle_scroll_left(&mut state, 5);

    let handle = state.session_manager.selected().unwrap();
    assert_eq!(handle.session.log_view_state.h_offset, 10, "scroll_left should be no-op in wrap mode");
}

#[test]
fn test_scroll_right_noop_when_wrap_enabled() {
    // Similar pattern
}

#[test]
fn test_scroll_to_line_start_noop_when_wrap_enabled() {
    // Similar pattern
}

#[test]
fn test_scroll_to_line_end_noop_when_wrap_enabled() {
    // Similar pattern
}

#[test]
fn test_scroll_left_works_when_wrap_disabled() {
    let mut state = create_test_state_with_session();
    let handle = state.session_manager.selected_mut().unwrap();
    handle.session.log_view_state.wrap_mode = false;
    handle.session.log_view_state.h_offset = 10;

    handle_scroll_left(&mut state, 5);

    let handle = state.session_manager.selected().unwrap();
    assert_eq!(handle.session.log_view_state.h_offset, 5);
}
```

Note: Look at how existing handler tests create test state with sessions. Reuse the existing `create_test_state_with_session()` or equivalent helper if one exists, or construct `AppState` manually following existing test patterns.

#### 3. Log view rendering tests

**File:** `crates/fdemon-tui/src/widgets/log_view/tests.rs` — add after existing tests

Use `TestTerminal` for full-render tests. Follow the template from `test_footer_height_not_stolen_in_small_area` (line 966 of tests.rs).

```rust
#[test]
fn test_wrap_mode_wraps_long_lines() {
    use crate::test_utils::TestTerminal;

    // Use a narrow terminal to force wrapping
    let mut term = TestTerminal::with_size(30, 10);

    let logs = logs_from(vec![
        make_entry(LogLevel::Info, LogSource::App, "This is a long log line that should wrap at terminal width"),
    ]);

    let log_view = LogView::new(&logs, test_icons()).wrap_mode(true);
    let mut state = LogViewState::new();

    term.render_stateful_widget(log_view, term.area(), &mut state);

    // The long line should be visible across multiple rows
    // (exact assertions depend on rendering details)
}

#[test]
fn test_nowrap_mode_truncates_long_lines() {
    use crate::test_utils::TestTerminal;

    let mut term = TestTerminal::with_size(30, 10);

    let logs = logs_from(vec![
        make_entry(LogLevel::Info, LogSource::App, "This is a long log line that should be truncated"),
    ]);

    let log_view = LogView::new(&logs, test_icons()).wrap_mode(false);
    let mut state = LogViewState::new();

    term.render_stateful_widget(log_view, term.area(), &mut state);

    // In nowrap mode with h_offset=0, no left indicator should appear
    // Right indicator (→) should appear if line exceeds width
}

#[test]
fn test_wrap_indicator_shown_in_metadata_bar() {
    use crate::test_utils::TestTerminal;

    let mut term = TestTerminal::new(); // 80x24

    let logs = logs_from(vec![
        make_entry(LogLevel::Info, LogSource::App, "test message"),
    ]);

    let log_view = LogView::new(&logs, test_icons()).wrap_mode(true);
    let mut state = LogViewState::new();

    term.render_stateful_widget(log_view, term.area(), &mut state);

    assert!(term.buffer_contains("wrap"), "wrap indicator should be visible");
}

#[test]
fn test_nowrap_indicator_shown_in_metadata_bar() {
    use crate::test_utils::TestTerminal;

    let mut term = TestTerminal::new();

    let logs = logs_from(vec![
        make_entry(LogLevel::Info, LogSource::App, "test message"),
    ]);

    let log_view = LogView::new(&logs, test_icons()).wrap_mode(false);
    let mut state = LogViewState::new();

    term.render_stateful_widget(log_view, term.area(), &mut state);

    assert!(term.buffer_contains("nowrap"), "nowrap indicator should be visible");
}

#[test]
fn test_wrap_mode_no_horizontal_scroll_indicators() {
    use crate::test_utils::TestTerminal;

    let mut term = TestTerminal::with_size(30, 10);

    let logs = logs_from(vec![
        make_entry(LogLevel::Info, LogSource::App, "A line that is definitely longer than thirty chars"),
    ]);

    let log_view = LogView::new(&logs, test_icons()).wrap_mode(true);
    let mut state = LogViewState::new();

    term.render_stateful_widget(log_view, term.area(), &mut state);

    // In wrap mode, horizontal scroll indicators should NOT appear
    // No ← or → characters in the content area
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

    // total_lines should reflect entry count
    assert!(state.total_lines > state.visible_lines, "scrollbar should be needed");
}
```

### Acceptance Criteria

1. At least 5 `LogViewState` unit tests covering: default value, toggle on/off, h_offset reset, roundtrip
2. At least 4 scroll handler tests covering: all 4 horizontal scroll functions are no-ops in wrap mode, and work normally in nowrap mode
3. At least 4 rendering tests covering: wrap mode wraps lines, nowrap mode preserves truncation, metadata bar shows correct indicator, no horizontal indicators in wrap mode
4. All new tests pass: `cargo test -p fdemon-app` and `cargo test -p fdemon-tui`
5. All existing tests still pass: `cargo test --workspace`
6. `cargo clippy --workspace -- -D warnings` passes

### Testing

Run the full quality gate:

```bash
cargo fmt --all && cargo check --workspace && cargo test --workspace && cargo clippy --workspace -- -D warnings
```

### Notes

- Use existing test helpers: `make_entry()`, `logs_from()`, `test_icons()`, `TestTerminal` — do not create new helpers unless absolutely necessary
- Follow existing test naming convention: `test_<scenario_description>`
- Keep tests focused — each test should verify one behavior
- The scroll handler tests may need to construct an `AppState` with a session. Check existing handler tests for the pattern (there are 1,039 tests in `fdemon-app` — patterns are well-established)
- For rendering tests, exact buffer content assertions can be brittle. Prefer `buffer_contains()` and `line_contains()` over exact cell-by-cell matching
