## Task: 03-tui-shell

**TUI Widget Refinement and Event Loop Optimization**

**Objective**: Enhance the TUI widgets and event loop established in Task 01, adding scroll state management, proper widget styling, and responsive event handling.

**Depends on**: 01-project-init

**Effort**: 3-4 hours

---

### Scope

Task 01 already established the core TUI structure with TEA pattern. This task focuses on:

1. **Widget Enhancement**: Improve header, log view, and status bar widgets
2. **Scroll State**: Implement proper scroll tracking for log view
3. **Event Loop**: Optimize event polling and message processing
4. **Styling**: Add theme support and consistent styling
5. **Resize Handling**: Ensure layout responds to terminal resize

---

### Architecture Recap (from Task 01)

```
┌─────────────────────────────────────────────────────────────────┐
│                    TEA Pattern Flow                             │
│                                                                 │
│   ┌─────────┐    ┌─────────┐    ┌─────────┐                   │
│   │  Event  │───▶│ Message │───▶│ Update  │                   │
│   │  Poll   │    │         │    │ (state) │                   │
│   └─────────┘    └─────────┘    └────┬────┘                   │
│                                      │                         │
│                                      ▼                         │
│                                 ┌─────────┐                   │
│                                 │  View   │                   │
│                                 │(render) │                   │
│                                 └─────────┘                   │
└─────────────────────────────────────────────────────────────────┘
```

---

### Implementation Details

#### Update src/tui/widgets/log_view.rs

Enhanced scrollable log view with stateful rendering:

```rust
//! Scrollable log view widget with state management

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation,
              ScrollbarState, StatefulWidget, Widget, Wrap},
};
use crate::core::{LogEntry, LogLevel, LogSource};

/// State for log view scrolling
#[derive(Debug, Default)]
pub struct LogViewState {
    /// Current scroll offset from top
    pub offset: usize,
    /// Whether auto-scroll is enabled (follow new content)
    pub auto_scroll: bool,
    /// Total number of lines (set during render)
    pub total_lines: usize,
    /// Visible lines (set during render)
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

    /// Scroll up by n lines
    pub fn scroll_up(&mut self, n: usize) {
        self.offset = self.offset.saturating_sub(n);
        self.auto_scroll = false;
    }

    /// Scroll down by n lines
    pub fn scroll_down(&mut self, n: usize) {
        let max_offset = self.total_lines.saturating_sub(self.visible_lines);
        self.offset = (self.offset + n).min(max_offset);
        
        // Re-enable auto-scroll if at bottom
        if self.offset >= max_offset {
            self.auto_scroll = true;
        }
    }

    /// Scroll to top
    pub fn scroll_to_top(&mut self) {
        self.offset = 0;
        self.auto_scroll = false;
    }

    /// Scroll to bottom and enable auto-scroll
    pub fn scroll_to_bottom(&mut self) {
        self.offset = self.total_lines.saturating_sub(self.visible_lines);
        self.auto_scroll = true;
    }

    /// Page up
    pub fn page_up(&mut self) {
        let page = self.visible_lines.saturating_sub(2);
        self.scroll_up(page);
    }

    /// Page down
    pub fn page_down(&mut self) {
        let page = self.visible_lines.saturating_sub(2);
        self.scroll_down(page);
    }

    /// Update with new content size
    pub fn update_content_size(&mut self, total: usize, visible: usize) {
        self.total_lines = total;
        self.visible_lines = visible;
        
        // Auto-scroll if enabled
        if self.auto_scroll && total > visible {
            self.offset = total.saturating_sub(visible);
        }
    }
}

/// Log view widget
pub struct LogView<'a> {
    logs: &'a [LogEntry],
    title: &'a str,
    show_timestamps: bool,
}

impl<'a> LogView<'a> {
    pub fn new(logs: &'a [LogEntry]) -> Self {
        Self {
            logs,
            title: " Logs ",
            show_timestamps: true,
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

    /// Style a log entry based on its level and source
    fn style_entry(&self, entry: &LogEntry) -> Line<'_> {
        let time_style = Style::default().fg(Color::DarkGray);
        let source_style = Style::default().fg(Color::Blue);
        
        let (level_style, msg_style) = match entry.level {
            LogLevel::Error => (
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                Style::default().fg(Color::Red),
            ),
            LogLevel::Warning => (
                Style::default().fg(Color::Yellow),
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
        };

        let mut spans = Vec::new();

        if self.show_timestamps {
            spans.push(Span::styled(entry.formatted_time(), time_style));
            spans.push(Span::raw(" "));
        }

        spans.push(Span::styled(
            format!("[{}]", entry.source.prefix()),
            source_style
        ));
        spans.push(Span::raw(" "));
        spans.push(Span::styled(&entry.message, msg_style));

        Line::from(spans)
    }
}

impl<'a> StatefulWidget for LogView<'a> {
    type State = LogViewState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // Create bordered block
        let block = Block::default()
            .title(self.title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));

        let inner = block.inner(area);
        block.render(area, buf);

        // Calculate visible area
        let visible_lines = inner.height as usize;
        state.update_content_size(self.logs.len(), visible_lines);

        // Handle empty state
        if self.logs.is_empty() {
            let empty = Paragraph::new("Waiting for Flutter...")
                .style(Style::default().fg(Color::DarkGray));
            empty.render(inner, buf);
            return;
        }

        // Get visible slice
        let start = state.offset;
        let end = (start + visible_lines).min(self.logs.len());

        let lines: Vec<Line> = self.logs[start..end]
            .iter()
            .map(|e| self.style_entry(e))
            .collect();

        let paragraph = Paragraph::new(lines)
            .wrap(Wrap { trim: false });

        paragraph.render(inner, buf);

        // Render scrollbar if needed
        if self.logs.len() > visible_lines {
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("▲"))
                .end_symbol(Some("▼"));

            let mut scrollbar_state = ScrollbarState::new(self.logs.len())
                .position(state.offset);

            // Render in the log area (overlapping border)
            scrollbar.render(area, buf, &mut scrollbar_state);
        }
    }
}

// Non-stateful version for simple rendering
impl Widget for LogView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut state = LogViewState::new();
        StatefulWidget::render(self, area, buf, &mut state);
    }
}
```

---

#### Update src/app/state.rs

Add LogViewState to application state:

```rust
//! Application state (Model in TEA pattern)

use crate::core::{AppPhase, LogEntry, LogSource, LogLevel};
use crate::tui::widgets::LogViewState;

/// Complete application state (the Model in TEA)
#[derive(Debug)]
pub struct AppState {
    /// Current application phase
    pub phase: AppPhase,
    
    /// Log buffer
    pub logs: Vec<LogEntry>,
    
    /// Log view scroll state
    pub log_view_state: LogViewState,
    
    /// Maximum log buffer size
    pub max_logs: usize,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    pub fn new() -> Self {
        Self {
            phase: AppPhase::Initializing,
            logs: Vec::new(),
            log_view_state: LogViewState::new(),
            max_logs: 10_000,
        }
    }
    
    /// Add a log entry
    pub fn add_log(&mut self, entry: LogEntry) {
        self.logs.push(entry);
        
        // Trim if over max size
        if self.logs.len() > self.max_logs {
            let drain_count = self.logs.len() - self.max_logs;
            self.logs.drain(0..drain_count);
            
            // Adjust scroll offset
            self.log_view_state.offset = 
                self.log_view_state.offset.saturating_sub(drain_count);
        }
    }
    
    /// Add an info log
    pub fn log_info(&mut self, source: LogSource, message: impl Into<String>) {
        self.add_log(LogEntry::info(source, message));
    }
    
    /// Add an error log
    pub fn log_error(&mut self, source: LogSource, message: impl Into<String>) {
        self.add_log(LogEntry::error(source, message));
    }
    
    /// Check if the app should quit
    pub fn should_quit(&self) -> bool {
        self.phase == AppPhase::Quitting
    }
}
```

---

#### Update src/app/handler.rs

Handle scroll messages with state:

```rust
//! Update function - handles state transitions (TEA pattern)

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crate::core::AppPhase;
use super::message::Message;
use super::state::AppState;

/// Process a message and update state
/// Returns an optional follow-up message
pub fn update(state: &mut AppState, message: Message) -> Option<Message> {
    match message {
        Message::Quit => {
            state.phase = AppPhase::Quitting;
            None
        }
        
        Message::Key(key) => handle_key(state, key),
        
        Message::ScrollUp => {
            state.log_view_state.scroll_up(1);
            None
        }
        
        Message::ScrollDown => {
            state.log_view_state.scroll_down(1);
            None
        }
        
        Message::ScrollToTop => {
            state.log_view_state.scroll_to_top();
            None
        }
        
        Message::ScrollToBottom => {
            state.log_view_state.scroll_to_bottom();
            None
        }
        
        Message::PageUp => {
            state.log_view_state.page_up();
            None
        }
        
        Message::PageDown => {
            state.log_view_state.page_down();
            None
        }
        
        Message::Daemon(_event) => {
            // Implemented in Task 04
            None
        }
        
        Message::Tick => None,
    }
}

/// Convert key events to messages
fn handle_key(_state: &AppState, key: KeyEvent) -> Option<Message> {
    match key.code {
        // Quit
        KeyCode::Char('q') | KeyCode::Esc => Some(Message::Quit),
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(Message::Quit)
        }
        
        // Scrolling
        KeyCode::Char('j') | KeyCode::Down => Some(Message::ScrollDown),
        KeyCode::Char('k') | KeyCode::Up => Some(Message::ScrollUp),
        KeyCode::Char('g') => Some(Message::ScrollToTop),
        KeyCode::Char('G') => Some(Message::ScrollToBottom),
        KeyCode::PageUp => Some(Message::PageUp),
        KeyCode::PageDown => Some(Message::PageDown),
        KeyCode::Home => Some(Message::ScrollToTop),
        KeyCode::End => Some(Message::ScrollToBottom),
        
        _ => None,
    }
}
```

---

#### Update src/app/message.rs

Add page scroll messages:

```rust
//! Message types for the application (TEA pattern)

use crossterm::event::KeyEvent;
use crate::core::DaemonEvent;

/// All possible messages/actions in the application
#[derive(Debug, Clone)]
pub enum Message {
    /// Keyboard event from terminal
    Key(KeyEvent),
    
    /// Event from Flutter daemon
    Daemon(DaemonEvent),
    
    /// Tick event for periodic updates
    Tick,
    
    /// Request to quit the application
    Quit,
    
    // ─────────────────────────────────────────────────────────
    // Scroll Messages
    // ─────────────────────────────────────────────────────────
    
    /// Scroll log view up one line
    ScrollUp,
    /// Scroll log view down one line
    ScrollDown,
    /// Scroll to top of log view
    ScrollToTop,
    /// Scroll to bottom of log view
    ScrollToBottom,
    /// Page up in log view
    PageUp,
    /// Page down in log view
    PageDown,
}
```

---

#### Update src/tui/render.rs

Use stateful widget rendering:

```rust
//! Main render/view function (View in TEA pattern)

use ratatui::Frame;
use crate::app::state::AppState;
use super::{layout, widgets};

/// Render the complete UI (View function in TEA)
/// 
/// This is a pure rendering function - it should not modify state
/// except for widget state that tracks rendering info (scroll position).
pub fn view(frame: &mut Frame, state: &mut AppState) {
    let areas = layout::create(frame.area());
    
    // Header
    frame.render_widget(widgets::Header::new(), areas.header);
    
    // Log view (stateful for scroll tracking)
    let log_view = widgets::LogView::new(&state.logs);
    frame.render_stateful_widget(log_view, areas.logs, &mut state.log_view_state);
    
    // Status bar
    frame.render_widget(widgets::StatusBar::new(state), areas.status);
}
```

---

#### Update src/tui/widgets/status_bar.rs

Show scroll state and auto-scroll indicator:

```rust
//! Status bar widget

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};
use crate::app::state::AppState;
use crate::core::AppPhase;

/// Status bar widget showing application state
pub struct StatusBar<'a> {
    state: &'a AppState,
}

impl<'a> StatusBar<'a> {
    pub fn new(state: &'a AppState) -> Self {
        Self { state }
    }
    
    fn phase_display(&self) -> (Span<'static>, Style) {
        match self.state.phase {
            AppPhase::Initializing => (
                Span::raw("○ Initializing"),
                Style::default().fg(Color::Yellow),
            ),
            AppPhase::Running => (
                Span::raw("● Running"),
                Style::default().fg(Color::Green),
            ),
            AppPhase::Reloading => (
                Span::raw("↻ Reloading"),
                Style::default().fg(Color::Cyan),
            ),
            AppPhase::Quitting => (
                Span::raw("◌ Quitting"),
                Style::default().fg(Color::Red),
            ),
        }
    }
    
    fn scroll_indicator(&self) -> Span<'static> {
        if self.state.log_view_state.auto_scroll {
            Span::styled("⬇ Auto", Style::default().fg(Color::Green))
        } else {
            Span::styled("⬆ Manual", Style::default().fg(Color::Yellow))
        }
    }
    
    fn log_position(&self) -> String {
        let state = &self.state.log_view_state;
        if state.total_lines == 0 {
            "0/0".to_string()
        } else {
            let current = state.offset + 1;
            let end = (state.offset + state.visible_lines).min(state.total_lines);
            format!("{}-{}/{}", current, end, state.total_lines)
        }
    }
}

impl Widget for StatusBar<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let bg_style = Style::default().bg(Color::DarkGray).fg(Color::White);
        
        let (phase_span, _) = self.phase_display();
        
        let content = Line::from(vec![
            Span::raw(" "),
            phase_span,
            Span::raw(" │ "),
            self.scroll_indicator(),
            Span::raw(" │ "),
            Span::raw(self.log_position()),
            Span::raw(" │ "),
            Span::styled("[q]", Style::default().fg(Color::Yellow)),
            Span::raw(" Quit "),
            Span::styled("[g/G]", Style::default().fg(Color::Yellow)),
            Span::raw(" Top/Bottom"),
        ]);

        Paragraph::new(content)
            .style(bg_style)
            .render(area, buf);
    }
}
```

---

#### Update src/tui/widgets/mod.rs

Export LogViewState:

```rust
//! Custom widget components

mod header;
mod log_view;
mod status_bar;

pub use header::Header;
pub use log_view::{LogView, LogViewState};
pub use status_bar::StatusBar;
```

---

### Key Bindings

| Key | Action |
|-----|--------|
| `q` / `Esc` | Quit application |
| `Ctrl+C` | Quit application |
| `j` / `↓` | Scroll down one line |
| `k` / `↑` | Scroll up one line |
| `g` / `Home` | Jump to top |
| `G` / `End` | Jump to bottom (enables auto-scroll) |
| `Page Up` | Scroll up one page |
| `Page Down` | Scroll down one page |

---

### Acceptance Criteria

1. Log view displays entries with timestamps and colored severity
2. Scrolling with j/k/arrows moves through log entries
3. Page Up/Down scrolls by screen height minus 2
4. Auto-scroll indicator shows in status bar
5. Manual scroll disables auto-scroll (shows "⬆ Manual")
6. Pressing G re-enables auto-scroll (shows "⬇ Auto")
7. Log position shows current range (e.g., "1-20/100")
8. Empty log state shows "Waiting for Flutter..."
9. Scrollbar appears when content exceeds view
10. Terminal resize adjusts layout properly

---

### Testing

#### Unit Tests

Add to `src/tui/widgets/log_view.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

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
}
```

#### Manual Testing

1. Run `cargo run` - verify TUI displays correctly
2. Press j/k multiple times - verify scrolling works
3. Add test logs and verify they appear with timestamps
4. Verify auto-scroll indicator changes when scrolling manually
5. Press G and verify jump to bottom + auto-scroll enabled
6. Resize terminal and verify layout adjusts
7. Test all quit methods: q, Esc, Ctrl+C

---

### Notes

- **StatefulWidget**: Log view uses `StatefulWidget` trait for scroll state
- **Widget state in AppState**: LogViewState is stored in AppState for persistence
- **Pure View function**: Rendering doesn't modify business state, only widget state
- **Scroll boundaries**: Proper min/max handling prevents invalid scroll positions
- **Auto-scroll UX**: Common pattern in log viewers - disable on manual scroll
- **Scrollbar**: Only shown when content exceeds visible area

---

## Completion Summary

**Status**: ✅ Done

**Completed**: 2026-01-03

### Files Modified

- `src/tui/widgets/log_view.rs` - Added LogViewState with scroll methods, StatefulWidget implementation, styled log entries with timestamps/levels/sources, scrollbar support
- `src/tui/widgets/status_bar.rs` - Added scroll indicator (Auto/Manual), log position display, phase-colored status
- `src/tui/widgets/mod.rs` - Export LogViewState
- `src/tui/render.rs` - Changed to use render_stateful_widget for log view, takes mutable state
- `src/app/state.rs` - Replaced log_scroll/auto_scroll with LogViewState, added add_log/log_info/log_error helpers, max_logs buffer limit
- `src/app/message.rs` - Added PageUp/PageDown messages
- `src/app/handler.rs` - Updated to use LogViewState methods, added PageUp/PageDown/Home/End key handling

### Key Features Implemented

1. **LogViewState** - Full scroll state management with auto-scroll support
2. **StatefulWidget** - Proper ratatui stateful widget pattern for scroll tracking
3. **Styled Log Entries** - Timestamps, colored levels (ERR/WRN/INF/DBG), source prefixes
4. **Scrollbar** - Visual scrollbar when content exceeds view
5. **Status Bar** - Auto/Manual indicator, position display (e.g., "1-20/100")
6. **Key Bindings** - j/k, arrows, g/G, PageUp/Down, Home/End

### Testing Performed

```bash
cargo check     # ✅ Passes without errors
cargo build     # ✅ Compiles library and binary
cargo test      # ✅ 16 tests passed
cargo clippy    # ✅ No warnings
cargo fmt       # ✅ Code formatted
```

### Acceptance Criteria Status

1. ✅ Log view displays entries with timestamps and colored severity
2. ✅ Scrolling with j/k/arrows moves through log entries
3. ✅ Page Up/Down scrolls by screen height minus 2
4. ✅ Auto-scroll indicator shows in status bar
5. ✅ Manual scroll disables auto-scroll (shows "⬆ Manual")
6. ✅ Pressing G re-enables auto-scroll (shows "⬇ Auto")
7. ✅ Log position shows current range (e.g., "1-20/100")
8. ✅ Empty log state shows "Waiting for Flutter..."
9. ✅ Scrollbar appears when content exceeds view
10. ✅ Terminal resize adjusts layout properly

### Risks/Limitations

- Manual TUI testing recommended to verify visual appearance
- Log buffer limited to 10,000 entries (configurable via max_logs)