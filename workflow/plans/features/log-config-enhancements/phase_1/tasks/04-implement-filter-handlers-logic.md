## Task: Implement Filter Handlers and Logic

**Objective**: Implement keyboard handlers for filter cycling and the filter logic in the LogView widget, including visual filter indicators in the log panel header.

**Depends on**: 03-integrate-filter-search-state

**Estimated Time**: 5-6 hours

### Scope

- `src/app/handler/keys.rs`: Add filter keyboard handlers
- `src/app/handler/update.rs`: Handle filter messages
- `src/tui/widgets/log_view.rs`: Implement filter display logic and header indicator

### Details

#### 1. Update `src/app/handler/keys.rs`

Add filter keyboard shortcuts to `handle_key_normal()`:

```rust
fn handle_key_normal(state: &AppState, key: KeyEvent) -> Option<Message> {
    // ... existing code ...
    
    match (key.code, key.modifiers) {
        // ... existing handlers ...
        
        // ─────────────────────────────────────────────────────────
        // Log Filtering (Phase 1)
        // ─────────────────────────────────────────────────────────
        // 'f' - Cycle log level filter
        (KeyCode::Char('f'), KeyModifiers::NONE) => Some(Message::CycleLevelFilter),
        
        // 'F' - Cycle log source filter
        (KeyCode::Char('F'), KeyModifiers::NONE) => Some(Message::CycleSourceFilter),
        (KeyCode::Char('F'), m) if m.contains(KeyModifiers::SHIFT) => {
            Some(Message::CycleSourceFilter)
        }
        
        // Shift+f (different handling for some terminals) - Reset all filters
        // Note: Some terminals send 'F' for Shift+f, handled above
        // We'll use Ctrl+f for reset to avoid conflicts
        (KeyCode::Char('f'), m) if m.contains(KeyModifiers::CONTROL) => {
            Some(Message::ResetFilters)
        }
        
        // ... rest of handlers ...
    }
}
```

**Note on Shift+f**: Terminal key handling for Shift+letter can be inconsistent. Consider using:
- `f` for level filter cycle
- `F` (Shift+f) for source filter cycle  
- `Ctrl+f` for reset filters

Alternatively, document that pressing 'f' repeatedly cycles through all options and eventually resets.

#### 2. Update `src/app/handler/update.rs`

Add message handlers for filter operations:

```rust
pub fn update(state: &mut AppState, msg: Message) -> UpdateResult {
    match msg {
        // ... existing handlers ...
        
        // ─────────────────────────────────────────────────────────
        // Log Filter Messages (Phase 1)
        // ─────────────────────────────────────────────────────────
        Message::CycleLevelFilter => {
            if let Some(session) = state.session_manager.current_session_mut() {
                session.cycle_level_filter();
                // Invalidate any cached filtered view
                // (This will be handled by LogView re-render)
            }
            UpdateResult::none()
        }
        
        Message::CycleSourceFilter => {
            if let Some(session) = state.session_manager.current_session_mut() {
                session.cycle_source_filter();
            }
            UpdateResult::none()
        }
        
        Message::ResetFilters => {
            if let Some(session) = state.session_manager.current_session_mut() {
                session.reset_filters();
            }
            UpdateResult::none()
        }
        
        // ... rest of handlers ...
    }
}
```

#### 3. Update `src/tui/widgets/log_view.rs`

##### 3.1 Add filter state to LogView

```rust
use crate::core::{FilterState, LogEntry, LogLevel, LogSource};

pub struct LogView<'a> {
    logs: &'a [LogEntry],
    title: &'a str,
    show_timestamps: bool,
    show_source: bool,
    /// Filter state for displaying indicator and filtering logs
    filter_state: Option<&'a FilterState>,
}

impl<'a> LogView<'a> {
    pub fn new(logs: &'a [LogEntry]) -> Self {
        Self {
            logs,
            title: " Logs ",
            show_timestamps: true,
            show_source: true,
            filter_state: None,
        }
    }
    
    /// Set the filter state for filtering and indicator display
    pub fn filter_state(mut self, state: &'a FilterState) -> Self {
        self.filter_state = Some(state);
        self
    }
    
    // ... rest of builder methods ...
}
```

##### 3.2 Generate dynamic title with filter indicator

```rust
impl<'a> LogView<'a> {
    /// Generate the title string including filter indicators
    fn build_title(&self) -> String {
        let base = self.title.trim();
        
        let Some(filter) = self.filter_state else {
            return base.to_string();
        };
        
        if !filter.is_active() {
            return base.to_string();
        }
        
        let mut indicators = Vec::new();
        
        // Level filter indicator
        if filter.level_filter != LogLevelFilter::All {
            indicators.push(filter.level_filter.display_name());
        }
        
        // Source filter indicator
        if filter.source_filter != LogSourceFilter::All {
            indicators.push(filter.source_filter.display_name());
        }
        
        if indicators.is_empty() {
            base.to_string()
        } else {
            format!("{} [{}]", base, indicators.join(" | "))
        }
    }
}
```

##### 3.3 Apply filtering in render

Update the `StatefulWidget::render` implementation:

```rust
impl<'a> StatefulWidget for LogView<'a> {
    type State = LogViewState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // Handle empty state specially
        if self.logs.is_empty() {
            self.render_empty(area, buf);
            return;
        }

        // Apply filter to get visible log indices
        let filtered_indices: Vec<usize> = if let Some(filter) = self.filter_state {
            self.logs
                .iter()
                .enumerate()
                .filter(|(_, entry)| filter.matches(entry))
                .map(|(i, _)| i)
                .collect()
        } else {
            (0..self.logs.len()).collect()
        };
        
        // Handle empty filtered state
        if filtered_indices.is_empty() {
            self.render_no_matches(area, buf);
            return;
        }

        // Create bordered block with dynamic title
        let title = self.build_title();
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));

        let inner = block.inner(area);
        block.render(area, buf);

        // Update state with filtered content dimensions
        let visible_lines = inner.height as usize;
        state.update_content_size(filtered_indices.len(), visible_lines);

        // Get visible slice of filtered logs
        let start = state.offset;
        let end = (start + visible_lines).min(filtered_indices.len());

        // Format visible entries using original log indices
        let lines: Vec<Line> = filtered_indices[start..end]
            .iter()
            .map(|&idx| self.format_entry(&self.logs[idx]))
            .collect();

        // Render log content
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .render(inner, buf);

        // Render scrollbar if content exceeds visible area
        if filtered_indices.len() > visible_lines {
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("▲"))
                .end_symbol(Some("▼"))
                .track_symbol(Some("│"))
                .thumb_symbol("█");

            let mut scrollbar_state = ScrollbarState::new(filtered_indices.len())
                .position(state.offset);

            scrollbar.render(area, buf, &mut scrollbar_state);
        }
    }
}
```

##### 3.4 Add "no matches" rendering

```rust
impl<'a> LogView<'a> {
    /// Render empty filtered state
    fn render_no_matches(&self, area: Rect, buf: &mut Buffer) {
        let title = self.build_title();
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));

        let inner = block.inner(area);
        block.render(area, buf);

        let message = vec![
            Line::from(""),
            Line::from(Span::styled(
                "No logs match current filter",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::ITALIC),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Press Ctrl+f to reset filters",
                Style::default().fg(Color::DarkGray),
            )),
        ];

        Paragraph::new(message)
            .alignment(ratatui::layout::Alignment::Center)
            .render(inner, buf);
    }
}
```

#### 4. Update `src/tui/render.rs` (if needed)

Ensure the LogView is passed the filter state when rendering:

```rust
// When rendering log view for current session
if let Some(session) = state.session_manager.current_session() {
    let log_view = LogView::new(&session.logs)
        .title(" Logs ")
        .filter_state(&session.filter_state);
    
    log_view.render(log_area, buf, &mut session.log_view_state);
}
```

### Acceptance Criteria

1. Pressing `f` cycles through level filters: All → Errors → Warnings → Info → Debug → All
2. Pressing `F` (Shift+f) cycles through source filters: All → App → Daemon → Flutter → Watcher → All
3. Pressing `Ctrl+f` resets both filters to All
4. Log panel header shows filter indicator when filter is active:
   - `" Logs [Errors only]"` for level filter
   - `" Logs [App logs]"` for source filter
   - `" Logs [Errors only | App logs]"` for combined filters
5. Logs are correctly filtered on display (original log buffer unchanged)
6. "No logs match current filter" message appears when filter excludes all logs
7. Scrolling works correctly with filtered logs
8. Auto-scroll behavior works with filtered logs
9. Filter state persists when switching between sessions

### Testing

Add tests to `src/app/handler/tests.rs`:

```rust
#[test]
fn test_cycle_level_filter_message() {
    let mut state = create_test_state_with_session();
    let session_id = state.session_manager.current_session().unwrap().id;
    
    // Initial state
    assert_eq!(
        state.session_manager.current_session().unwrap()
            .filter_state.level_filter,
        LogLevelFilter::All
    );
    
    // Cycle to Errors
    update(&mut state, Message::CycleLevelFilter);
    assert_eq!(
        state.session_manager.current_session().unwrap()
            .filter_state.level_filter,
        LogLevelFilter::Errors
    );
}

#[test]
fn test_reset_filters_message() {
    let mut state = create_test_state_with_session();
    
    // Set some filters
    update(&mut state, Message::CycleLevelFilter);
    update(&mut state, Message::CycleSourceFilter);
    
    // Reset
    update(&mut state, Message::ResetFilters);
    
    let filter = &state.session_manager.current_session().unwrap().filter_state;
    assert_eq!(filter.level_filter, LogLevelFilter::All);
    assert_eq!(filter.source_filter, LogSourceFilter::All);
}
```

Add widget tests to `src/tui/widgets/log_view.rs`:

```rust
#[test]
fn test_build_title_no_filter() {
    let logs = vec![make_entry(LogLevel::Info, LogSource::App, "Test")];
    let view = LogView::new(&logs).title(" Logs ");
    assert_eq!(view.build_title(), " Logs ");
}

#[test]
fn test_build_title_with_level_filter() {
    let logs = vec![make_entry(LogLevel::Info, LogSource::App, "Test")];
    let filter = FilterState {
        level_filter: LogLevelFilter::Errors,
        source_filter: LogSourceFilter::All,
    };
    let view = LogView::new(&logs).title(" Logs ").filter_state(&filter);
    assert!(view.build_title().contains("Errors"));
}

#[test]
fn test_filtered_logs_count() {
    let logs = vec![
        make_entry(LogLevel::Info, LogSource::App, "info"),
        make_entry(LogLevel::Error, LogSource::App, "error"),
        make_entry(LogLevel::Warning, LogSource::Daemon, "warning"),
    ];
    let filter = FilterState {
        level_filter: LogLevelFilter::Errors,
        source_filter: LogSourceFilter::All,
    };
    
    let filtered: Vec<_> = logs.iter()
        .filter(|e| filter.matches(e))
        .collect();
    
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].level, LogLevel::Error);
}
```

### Notes

- The filter is applied during render, not when logs are added. This ensures the full log buffer is always preserved.
- When scrolling in a filtered view, the scroll offset refers to the filtered list, not the original log indices.
- Consider caching the filtered indices if performance becomes an issue with large log buffers (can be done in a follow-up optimization task).
- The filter indicator uses color coding: active filters could use a highlight color (e.g., yellow) to make them more visible.
- Ensure keyboard shortcuts don't conflict with existing bindings:
  - `f` was previously unused
  - `F` (Shift+f) was previously unused
  - `Ctrl+f` was previously unused (not a standard quit/control binding)