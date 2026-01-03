## Task: 05-output-display

**LogEntry Display and Formatting**

**Objective**: Enhance the log display to properly format LogEntry objects with timestamps, colored severity levels, and source indicators. Build upon the scroll system implemented in Task 03 and daemon integration from Task 04.

**Depends on**: 04-flutter-spawn

**Effort**: 3-4 hours

---

### Scope

Tasks 03 and 04 have already established:
- ✅ LogViewState with scroll tracking
- ✅ StatefulWidget for LogView
- ✅ DaemonEvent to LogEntry conversion
- ✅ AppState.add_log() with buffer management

This task focuses on:
1. **Enhanced LogEntry formatting** with proper timestamp display
2. **Color-coded log levels** (Error=red, Warning=yellow, Info=gray, Debug=cyan)
3. **Source prefixes** for Flutter vs App messages
4. **Long line handling** with proper wrapping
5. **Visual improvements** for better readability

---

### Architecture Context

```
┌─────────────────────────────────────────────────────────────────┐
│                    Log Display Flow                             │
│                                                                 │
│  DaemonEvent ──▶ handler.rs ──▶ LogEntry ──▶ AppState.logs     │
│                                                                 │
│                         ▼                                       │
│                                                                 │
│  render.rs ──▶ LogView::new(&state.logs) ──▶ styled Lines      │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

---

### Implementation Details

#### Update src/tui/widgets/log_view.rs

Enhance the log entry styling with comprehensive formatting:

```rust
//! Scrollable log view widget with rich formatting

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation,
              ScrollbarState, StatefulWidget, Widget, Wrap},
};
use crate::core::{LogEntry, LogLevel, LogSource};

/// State for log view scrolling (established in Task 03)
#[derive(Debug, Default)]
pub struct LogViewState {
    pub offset: usize,
    pub auto_scroll: bool,
    pub total_lines: usize,
    pub visible_lines: usize,
}

impl LogViewState {
    pub fn new() -> Self {
        Self {
            offset: 0,
            auto_scroll: true,
            total_lines: 0,
            visible_lines: 0,
        }
    }

    pub fn scroll_up(&mut self, n: usize) {
        self.offset = self.offset.saturating_sub(n);
        self.auto_scroll = false;
    }

    pub fn scroll_down(&mut self, n: usize) {
        let max_offset = self.total_lines.saturating_sub(self.visible_lines);
        self.offset = (self.offset + n).min(max_offset);
        if self.offset >= max_offset {
            self.auto_scroll = true;
        }
    }

    pub fn scroll_to_top(&mut self) {
        self.offset = 0;
        self.auto_scroll = false;
    }

    pub fn scroll_to_bottom(&mut self) {
        self.offset = self.total_lines.saturating_sub(self.visible_lines);
        self.auto_scroll = true;
    }

    pub fn page_up(&mut self) {
        let page = self.visible_lines.saturating_sub(2);
        self.scroll_up(page);
    }

    pub fn page_down(&mut self) {
        let page = self.visible_lines.saturating_sub(2);
        self.scroll_down(page);
    }

    pub fn update_content_size(&mut self, total: usize, visible: usize) {
        self.total_lines = total;
        self.visible_lines = visible;
        if self.auto_scroll && total > visible {
            self.offset = total.saturating_sub(visible);
        }
    }
}

/// Log view widget with rich formatting
pub struct LogView<'a> {
    logs: &'a [LogEntry],
    title: &'a str,
    show_timestamps: bool,
    show_source: bool,
}

impl<'a> LogView<'a> {
    pub fn new(logs: &'a [LogEntry]) -> Self {
        Self {
            logs,
            title: " Logs ",
            show_timestamps: true,
            show_source: true,
        }
    }

    pub fn title(mut self, title: &'a str) -> Self {
        self.title = title;
        self
    }

    pub fn show_timestamps(mut self, show: bool) -> Self {
        self.show_timestamps = show;
        self
    }

    pub fn show_source(mut self, show: bool) -> Self {
        self.show_source = show;
        self
    }

    /// Get style for log level
    fn level_style(level: LogLevel) -> (Style, Style) {
        match level {
            LogLevel::Error => (
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                Style::default().fg(Color::LightRed),
            ),
            LogLevel::Warning => (
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                Style::default().fg(Color::Yellow),
            ),
            LogLevel::Info => (
                Style::default().fg(Color::Green),
                Style::default().fg(Color::Gray),
            ),
            LogLevel::Debug => (
                Style::default().fg(Color::Cyan),
                Style::default().fg(Color::DarkGray),
            ),
        }
    }

    /// Get style for log source
    fn source_style(source: LogSource) -> Style {
        match source {
            LogSource::App => Style::default().fg(Color::Magenta),
            LogSource::Flutter => Style::default().fg(Color::Blue),
            LogSource::FlutterError => Style::default().fg(Color::Red),
            LogSource::Watcher => Style::default().fg(Color::Cyan),
        }
    }

    /// Format a single log entry as a styled Line
    fn format_entry(&self, entry: &LogEntry) -> Line<'_> {
        let (level_style, msg_style) = Self::level_style(entry.level);
        let source_style = Self::source_style(entry.source);
        
        let mut spans = Vec::with_capacity(6);

        // Timestamp: "12:34:56 "
        if self.show_timestamps {
            spans.push(Span::styled(
                entry.formatted_time(),
                Style::default().fg(Color::DarkGray),
            ));
            spans.push(Span::raw(" "));
        }

        // Level indicator: "ERR " or "INF " etc.
        spans.push(Span::styled(
            format!("{} ", entry.level.prefix()),
            level_style,
        ));

        // Source: "[flutter] " or "[app] "
        if self.show_source {
            spans.push(Span::styled(
                format!("[{}] ", entry.source.prefix()),
                source_style,
            ));
        }

        // Message content
        spans.push(Span::styled(entry.message.as_str(), msg_style));

        Line::from(spans)
    }

    /// Render empty state
    fn render_empty(&self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .title(self.title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));

        let inner = block.inner(area);
        block.render(area, buf);

        // Center the waiting message
        let waiting_text = vec![
            Line::from(""),
            Line::from(Span::styled(
                "Waiting for Flutter...",
                Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Make sure you're in a Flutter project directory",
                Style::default().fg(Color::DarkGray),
            )),
        ];

        Paragraph::new(waiting_text)
            .alignment(ratatui::layout::Alignment::Center)
            .render(inner, buf);
    }
}

impl<'a> StatefulWidget for LogView<'a> {
    type State = LogViewState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // Handle empty state specially
        if self.logs.is_empty() {
            self.render_empty(area, buf);
            return;
        }

        // Create bordered block
        let block = Block::default()
            .title(self.title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));

        let inner = block.inner(area);
        block.render(area, buf);

        // Update state with current dimensions
        let visible_lines = inner.height as usize;
        state.update_content_size(self.logs.len(), visible_lines);

        // Get visible slice of logs
        let start = state.offset;
        let end = (start + visible_lines).min(self.logs.len());

        // Format visible entries
        let lines: Vec<Line> = self.logs[start..end]
            .iter()
            .map(|entry| self.format_entry(entry))
            .collect();

        // Render log content
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .render(inner, buf);

        // Render scrollbar if content exceeds visible area
        if self.logs.len() > visible_lines {
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("▲"))
                .end_symbol(Some("▼"))
                .track_symbol(Some("│"))
                .thumb_symbol("█");

            let mut scrollbar_state = ScrollbarState::new(self.logs.len())
                .position(state.offset);

            scrollbar.render(area, buf, &mut scrollbar_state);
        }
    }
}

// Fallback non-stateful implementation
impl Widget for LogView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut state = LogViewState::new();
        StatefulWidget::render(self, area, buf, &mut state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Local;

    fn make_entry(level: LogLevel, source: LogSource, msg: &str) -> LogEntry {
        LogEntry {
            timestamp: Local::now(),
            level,
            source,
            message: msg.to_string(),
        }
    }

    #[test]
    fn test_format_entry_includes_timestamp() {
        let logs = vec![make_entry(LogLevel::Info, LogSource::App, "Test")];
        let view = LogView::new(&logs).show_timestamps(true);
        let line = view.format_entry(&logs[0]);
        
        // Should have multiple spans including timestamp
        assert!(line.spans.len() >= 3);
    }

    #[test]
    fn test_format_entry_no_timestamp() {
        let logs = vec![make_entry(LogLevel::Info, LogSource::App, "Test")];
        let view = LogView::new(&logs).show_timestamps(false);
        let line = view.format_entry(&logs[0]);
        
        // Fewer spans without timestamp
        let with_ts = LogView::new(&logs).show_timestamps(true);
        let line_with = with_ts.format_entry(&logs[0]);
        assert!(line.spans.len() < line_with.spans.len());
    }

    #[test]
    fn test_level_styles_are_distinct() {
        let (err_level, _) = LogView::level_style(LogLevel::Error);
        let (info_level, _) = LogView::level_style(LogLevel::Info);
        
        // Error should be red, Info should be green
        assert_ne!(err_level.fg, info_level.fg);
    }
}
```

---

#### Update src/core/types.rs

Ensure LogEntry has all needed display methods:

```rust
impl LogEntry {
    // ... existing methods from Task 02 ...

    /// Format for single-line display (without wrapping)
    pub fn display_line(&self) -> String {
        format!(
            "{} {} [{}] {}",
            self.formatted_time(),
            self.level.prefix(),
            self.source.prefix(),
            self.message
        )
    }

    /// Check if this is an error-level entry
    pub fn is_error(&self) -> bool {
        self.level == LogLevel::Error
    }

    /// Check if this is from Flutter
    pub fn is_flutter(&self) -> bool {
        matches!(self.source, LogSource::Flutter | LogSource::FlutterError)
    }
}
```

---

### Display Format

Each log entry is displayed with the following format:

```
HH:MM:SS LVL [source] message content here
│        │   │        └── Message text (colored by level)
│        │   └── Source prefix: app, flutter, watch
│        └── Level: ERR, WRN, INF, DBG
└── Timestamp in local time
```

**Example Output:**

```
12:34:56 INF [app] Flutter Demon starting...
12:34:57 INF [flutter] Event: daemon.connected
12:34:58 INF [flutter] Launching lib/main.dart on iPhone 15 Pro...
12:35:01 INF [flutter] Event: app.started
12:35:15 WRN [flutter] Warning: Some deprecation notice
12:35:22 ERR [flutter] Error: Could not find asset
```

---

### Color Scheme

| Level | Level Color | Message Color |
|-------|-------------|---------------|
| Error | Red Bold | Light Red |
| Warning | Yellow Bold | Yellow |
| Info | Green | Gray |
| Debug | Cyan | Dark Gray |

| Source | Color |
|--------|-------|
| App | Magenta |
| Flutter | Blue |
| FlutterError | Red |
| Watcher | Cyan |

---

### Acceptance Criteria

1. Log entries display with HH:MM:SS timestamps
2. Level prefixes (ERR/WRN/INF/DBG) are color-coded
3. Source prefixes show in brackets with distinct colors
4. Error messages appear in red/light-red
5. Warning messages appear in yellow
6. Info messages appear in gray
7. Debug messages appear in cyan/dark-gray
8. Empty state shows centered "Waiting for Flutter..."
9. Long messages wrap properly
10. Scrollbar has visible track and thumb symbols

---

### Testing

#### Unit Tests

Tests are included in the log_view.rs module above.

#### Visual Testing

1. Run with a Flutter project and observe:
   - Timestamps align properly
   - Colors are visible and distinct
   - Level indicators are readable

2. Generate different log types:
   ```rust
   // In a test mode, add sample logs:
   state.add_log(LogEntry::info(LogSource::App, "Info message"));
   state.add_log(LogEntry::warn(LogSource::Flutter, "Warning message"));
   state.add_log(LogEntry::error(LogSource::FlutterError, "Error message"));
   ```

3. Test scrolling:
   - Add 50+ log entries
   - Verify scrollbar appears
   - Verify scroll position tracking

4. Test terminal themes:
   - Light terminal background
   - Dark terminal background
   - Verify colors remain readable

---

### Integration with Task 04

The log entries are created in `src/app/handler.rs` when processing `DaemonEvent`:

```rust
// From Task 04 - handler.rs
DaemonEvent::Stdout(line) => {
    if let Some(json) = protocol::strip_brackets(&line) {
        if let Some(msg) = protocol::RawMessage::parse(json) {
            state.add_log(LogEntry::new(
                LogLevel::Info,
                LogSource::Flutter,
                msg.summary(),  // e.g., "Event: app.started"
            ));
        }
    }
}

DaemonEvent::Stderr(line) => {
    state.add_log(LogEntry::new(
        LogLevel::Error,
        LogSource::FlutterError,
        line,
    ));
}
```

---

### Notes

- **Stateful rendering**: LogViewState tracks scroll, updated during render
- **Builder pattern**: LogView uses builder pattern for configuration
- **Color fallback**: Colors chosen to work on both light and dark terminals
- **Timestamp format**: Using `chrono::Local` for user's timezone
- **Performance**: Only visible entries are formatted and rendered
- **Wrap behavior**: `Wrap { trim: false }` preserves indentation
- **Empty state UX**: Clear message when no logs yet

---

## Completion Summary

**Status**: ✅ Done

### Files Modified

- `src/tui/widgets/log_view.rs` - Enhanced LogView widget with:
  - Added `show_source` builder method
  - Added `level_style()` helper returning distinct (level, message) style tuples
  - Added `source_style()` helper with source-specific colors
  - Changed format order to: `HH:MM:SS LVL [source] message`
  - Added `render_empty()` for centered "Waiting for Flutter..." message
  - Added scrollbar track (`│`) and thumb (`█`) symbols
  - Updated tests for new formatting

- `src/core/types.rs` - Added LogEntry helper methods:
  - `display_line()` - formats entry as single-line string
  - `is_error()` - checks if entry is error-level
  - `is_flutter()` - checks if source is Flutter or FlutterError
  - Added corresponding unit tests

### Notable Decisions/Tradeoffs

1. **Owned data in format_entry**: Changed return type to `Line<'static>` and cloned the message string to avoid lifetime complexity. Minimal performance impact since only visible entries are formatted.

2. **Color scheme alignment**: Implemented colors per spec:
   - Error: Red (bold) / LightRed
   - Warning: Yellow (bold) / Yellow
   - Info: Green / Gray
   - Debug: Cyan / DarkGray
   - Sources: App=Magenta, Flutter=Blue, FlutterError=Red, Watcher=Cyan

3. **Format order change**: Changed from `[source] [level] message` to `LVL [source] message` to match task spec and improve readability.

### Testing Performed

```bash
cargo fmt    # ✅ Pass
cargo check  # ✅ Pass
cargo test   # ✅ 34 tests passed
cargo clippy # ✅ No warnings
```

### Acceptance Criteria Met

1. ✅ Log entries display with HH:MM:SS timestamps
2. ✅ Level prefixes (ERR/WRN/INF/DBG) are color-coded
3. ✅ Source prefixes show in brackets with distinct colors
4. ✅ Error messages appear in red/light-red
5. ✅ Warning messages appear in yellow (with bold level)
6. ✅ Info messages appear in gray
7. ✅ Debug messages appear in cyan/dark-gray
8. ✅ Empty state shows centered "Waiting for Flutter..."
9. ✅ Long messages wrap properly (Wrap { trim: false })
10. ✅ Scrollbar has visible track (│) and thumb (█) symbols

### Risks/Limitations

- Colors may vary based on terminal theme/capabilities
- LightRed color for error messages may not be available in all terminals (falls back gracefully)
- Empty state message assumes vertical centering works correctly in ratatui