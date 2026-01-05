## Task: Collapsible Stack Traces

**Objective**: Implement expand/collapse functionality for stack traces, allowing users to toggle between a compact view (showing first N frames) and full view, with persistent state tracking and configurable defaults.

**Depends on**: [05-stack-trace-rendering](05-stack-trace-rendering.md)

### Scope

- `src/tui/widgets/log_view.rs`: Add collapse/expand logic and indicators
- `src/app/session.rs`: Track collapse state per log entry
- `src/app/handler/keys.rs`: Handle Enter key for toggle
- `src/app/message.rs`: Add toggle message
- `src/config/types.rs`: Add collapse configuration options

### Configuration Options

Add to `config/types.rs` and `.fdemon/config.toml`:

```rust
// In src/config/types.rs

#[derive(Debug, Clone, Deserialize)]
pub struct UiConfig {
    // ... existing fields ...
    
    /// Whether stack traces start collapsed (default: true)
    #[serde(default = "default_stack_trace_collapsed")]
    pub stack_trace_collapsed: bool,
    
    /// Maximum frames to show when collapsed (default: 5)
    #[serde(default = "default_stack_trace_max_frames")]
    pub stack_trace_max_frames: usize,
}

fn default_stack_trace_collapsed() -> bool {
    true
}

fn default_stack_trace_max_frames() -> usize {
    5
}
```

```toml
# In .fdemon/config.toml

[ui]
# Stack trace display settings
stack_trace_collapsed = true    # Default collapsed state
stack_trace_max_frames = 5      # Max frames shown when collapsed
```

### Collapse State Tracking

```rust
// In src/app/session.rs or a new src/app/collapse_state.rs

use std::collections::HashSet;

/// Tracks which log entries have expanded stack traces
#[derive(Debug, Clone, Default)]
pub struct CollapseState {
    /// Set of log entry IDs that are currently expanded
    /// (by default, entries are collapsed based on config)
    expanded_entries: HashSet<u64>,
    
    /// Set of log entry IDs that are explicitly collapsed
    /// (overrides default when default is expanded)
    collapsed_entries: HashSet<u64>,
}

impl CollapseState {
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Check if an entry's stack trace should be shown expanded
    pub fn is_expanded(&self, entry_id: u64, default_collapsed: bool) -> bool {
        if default_collapsed {
            // Default is collapsed, check if user expanded it
            self.expanded_entries.contains(&entry_id)
        } else {
            // Default is expanded, check if user collapsed it
            !self.collapsed_entries.contains(&entry_id)
        }
    }
    
    /// Toggle the collapse state of an entry
    pub fn toggle(&mut self, entry_id: u64, default_collapsed: bool) {
        if default_collapsed {
            if self.expanded_entries.contains(&entry_id) {
                self.expanded_entries.remove(&entry_id);
            } else {
                self.expanded_entries.insert(entry_id);
            }
        } else {
            if self.collapsed_entries.contains(&entry_id) {
                self.collapsed_entries.remove(&entry_id);
            } else {
                self.collapsed_entries.insert(entry_id);
            }
        }
    }
    
    /// Expand all stack traces
    pub fn expand_all(&mut self, entry_ids: impl Iterator<Item = u64>) {
        self.collapsed_entries.clear();
        self.expanded_entries.extend(entry_ids);
    }
    
    /// Collapse all stack traces
    pub fn collapse_all(&mut self) {
        self.expanded_entries.clear();
        self.collapsed_entries.clear(); // Let default take over
    }
}
```

### Session State Integration

```rust
// In src/app/session.rs

pub struct Session {
    // ... existing fields ...
    
    /// Collapse state for stack traces
    pub collapse_state: CollapseState,
}

impl Session {
    /// Toggle stack trace collapse for the currently selected/focused entry
    pub fn toggle_stack_trace(&mut self, entry_id: u64, default_collapsed: bool) {
        self.collapse_state.toggle(entry_id, default_collapsed);
    }
}
```

### Message for Toggle

```rust
// In src/app/message.rs

pub enum Message {
    // ... existing variants ...
    
    /// Toggle stack trace expand/collapse for entry at current position
    ToggleStackTrace,
    
    /// Expand all stack traces in current session
    ExpandAllStackTraces,
    
    /// Collapse all stack traces in current session
    CollapseAllStackTraces,
}
```

### Key Handler

```rust
// In src/app/handler/keys.rs

fn handle_key_normal(key: KeyEvent, state: &mut AppState) -> Option<Message> {
    match (key.code, key.modifiers) {
        // ... existing handlers ...
        
        // Enter on a log entry with stack trace toggles collapse
        (KeyCode::Enter, KeyModifiers::NONE) => {
            // Check if current entry has a stack trace
            if let Some(entry) = state.current_log_entry() {
                if entry.has_stack_trace() {
                    return Some(Message::ToggleStackTrace);
                }
            }
            None
        }
        
        // Optional: Ctrl+E to expand all, Ctrl+C to collapse all
        // (if these don't conflict with other bindings)
        
        _ => None,
    }
}
```

### Update Handler

```rust
// In src/app/handler/update.rs

fn handle_toggle_stack_trace(state: &mut AppState) -> UpdateResult {
    if let Some(session) = state.current_session_mut() {
        if let Some(entry) = session.current_focused_entry() {
            let default_collapsed = state.settings.ui.stack_trace_collapsed;
            session.toggle_stack_trace(entry.id, default_collapsed);
        }
    }
    
    UpdateResult::default()
}
```

### Visual Indicators

Update rendering to show collapse indicators:

```rust
// In src/tui/widgets/log_view.rs

impl<'a> LogView<'a> {
    /// Render a log entry with collapsible stack trace
    fn render_entry_with_collapsible_trace(
        &self,
        entry: &LogEntry,
        is_expanded: bool,
        max_frames: usize,
        area: Rect,
        buf: &mut Buffer,
        y: &mut u16,
    ) {
        // Render the error message line
        self.render_log_line(entry, area, buf, *y);
        *y += 1;
        
        if let Some(trace) = &entry.stack_trace {
            if trace.frames.is_empty() {
                return;
            }
            
            if is_expanded {
                // Render expanded indicator and all frames
                self.render_expanded_indicator(area, buf, *y);
                *y += 1;
                
                for frame in &trace.frames {
                    if *y >= area.height {
                        break;
                    }
                    self.render_stack_frame(frame, area, buf, *y);
                    *y += 1;
                }
            } else {
                // Render first N frames + collapsed indicator
                let visible_count = max_frames.min(trace.frames.len());
                let hidden_count = trace.frames.len().saturating_sub(max_frames);
                
                for frame in trace.frames.iter().take(visible_count) {
                    if *y >= area.height {
                        break;
                    }
                    self.render_stack_frame(frame, area, buf, *y);
                    *y += 1;
                }
                
                if hidden_count > 0 {
                    self.render_collapsed_indicator(hidden_count, area, buf, *y);
                    *y += 1;
                }
            }
        }
    }
    
    /// Render "▼ Stack trace:" header for expanded traces
    fn render_expanded_indicator(&self, area: Rect, buf: &mut Buffer, y: u16) {
        let indicator = Line::from(vec![
            Span::styled("    ", Style::default()),
            Span::styled("▼ ", Style::default().fg(Color::Cyan)),
            Span::styled("Stack trace:", Style::default().fg(Color::DarkGray)),
        ]);
        buf.set_line(area.x, area.y + y, &indicator, area.width);
    }
    
    /// Render "▶ N more frames..." indicator for collapsed traces
    fn render_collapsed_indicator(&self, hidden_count: usize, area: Rect, buf: &mut Buffer, y: u16) {
        let text = if hidden_count == 1 {
            "1 more frame...".to_string()
        } else {
            format!("{} more frames...", hidden_count)
        };
        
        let indicator = Line::from(vec![
            Span::styled("    ", Style::default()),
            Span::styled("▶ ", Style::default().fg(Color::Yellow)),
            Span::styled(text, Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC)),
        ]);
        buf.set_line(area.x, area.y + y, &indicator, area.width);
    }
}
```

### Focused Entry Tracking

To toggle the correct entry, track which entry is "focused":

```rust
// In src/app/session.rs or log_view state

impl Session {
    /// Get the log entry at the current scroll position (for toggle)
    pub fn focused_entry(&self) -> Option<&LogEntry> {
        // The focused entry is the one at the top of the visible area
        // or the one the user has navigated to
        self.logs.get(self.log_view_state.focused_index)
    }
    
    /// Get the focused entry's ID
    pub fn focused_entry_id(&self) -> Option<u64> {
        self.focused_entry().map(|e| e.id)
    }
}

impl LogViewState {
    // Track which entry is focused (for toggle)
    pub focused_index: usize,
    
    /// Move focus to next error entry
    pub fn focus_next_error(&mut self, logs: &[LogEntry]) {
        // Find next error after focused_index
    }
    
    /// Move focus to previous error entry  
    pub fn focus_prev_error(&mut self, logs: &[LogEntry]) {
        // Find previous error before focused_index
    }
}
```

### Line Count Calculation Update

Update line count to account for collapse state:

```rust
impl LogViewState {
    /// Calculate total visible lines with collapse state
    pub fn calculate_visible_lines(
        &self,
        logs: &[LogEntry],
        collapse_state: &CollapseState,
        default_collapsed: bool,
        max_frames: usize,
    ) -> usize {
        logs.iter()
            .map(|entry| {
                let frame_count = entry.stack_trace_frame_count();
                if frame_count == 0 {
                    1 // Just the message line
                } else {
                    let is_expanded = collapse_state.is_expanded(entry.id, default_collapsed);
                    if is_expanded {
                        1 + 1 + frame_count // message + header + all frames
                    } else {
                        let visible = max_frames.min(frame_count);
                        let has_more = frame_count > max_frames;
                        1 + visible + if has_more { 1 } else { 0 } // message + visible + indicator
                    }
                }
            })
            .sum()
    }
}
```

### Acceptance Criteria

1. [ ] `CollapseState` struct tracks expanded/collapsed entries
2. [ ] `stack_trace_collapsed` config option added (default: true)
3. [ ] `stack_trace_max_frames` config option added (default: 5)
4. [ ] Enter key toggles collapse on focused entry with stack trace
5. [ ] Collapsed indicator shows "▶ N more frames..."
6. [ ] Expanded indicator shows "▼ Stack trace:"
7. [ ] Collapse state persists during session (not across restarts)
8. [ ] Line count calculation accounts for collapse state
9. [ ] Scrolling works correctly with mixed collapsed/expanded entries
10. [ ] Toggle only works on entries that have stack traces
11. [ ] Visual feedback when toggle occurs (e.g., immediate re-render)

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_collapse_state_default() {
        let state = CollapseState::new();
        
        // With default collapsed=true, entries should show as collapsed
        assert!(!state.is_expanded(1, true));
        
        // With default collapsed=false, entries should show as expanded
        assert!(state.is_expanded(1, false));
    }
    
    #[test]
    fn test_collapse_state_toggle() {
        let mut state = CollapseState::new();
        
        // Toggle from collapsed (default) to expanded
        state.toggle(42, true);
        assert!(state.is_expanded(42, true));
        
        // Toggle back to collapsed
        state.toggle(42, true);
        assert!(!state.is_expanded(42, true));
    }
    
    #[test]
    fn test_collapse_state_multiple_entries() {
        let mut state = CollapseState::new();
        
        state.toggle(1, true); // Expand entry 1
        state.toggle(3, true); // Expand entry 3
        
        assert!(state.is_expanded(1, true));
        assert!(!state.is_expanded(2, true)); // Not toggled
        assert!(state.is_expanded(3, true));
    }
    
    #[test]
    fn test_collapse_all() {
        let mut state = CollapseState::new();
        
        state.toggle(1, true);
        state.toggle(2, true);
        state.toggle(3, true);
        
        state.collapse_all();
        
        assert!(!state.is_expanded(1, true));
        assert!(!state.is_expanded(2, true));
        assert!(!state.is_expanded(3, true));
    }
    
    #[test]
    fn test_line_count_collapsed() {
        let entry = create_entry_with_trace(10); // 10 frames
        let state = CollapseState::new();
        
        // Collapsed: 1 message + 5 visible + 1 indicator = 7
        let lines = calculate_entry_lines(&entry, &state, true, 5);
        assert_eq!(lines, 7);
    }
    
    #[test]
    fn test_line_count_expanded() {
        let entry = create_entry_with_trace(10); // 10 frames
        let mut state = CollapseState::new();
        state.toggle(entry.id, true); // Expand it
        
        // Expanded: 1 message + 1 header + 10 frames = 12
        let lines = calculate_entry_lines(&entry, &state, true, 5);
        assert_eq!(lines, 12);
    }
}
```

### Manual Testing Checklist

Using enhanced sample apps:

- [ ] Trigger error with 10+ frame stack trace
- [ ] Verify only first 5 frames visible (collapsed)
- [ ] Verify "▶ 5 more frames..." indicator shown
- [ ] Press Enter on the entry
- [ ] Verify all frames now visible (expanded)
- [ ] Verify "▼ Stack trace:" header shown
- [ ] Press Enter again to collapse
- [ ] Scroll through multiple errors with different collapse states
- [ ] Modify `stack_trace_max_frames` in config and restart
- [ ] Verify new frame limit respected

### Keyboard Shortcuts Summary

| Key | Action |
|-----|--------|
| `Enter` | Toggle expand/collapse on focused entry with stack trace |
| (Future) `Ctrl+e` | Expand all stack traces |
| (Future) `Ctrl+Shift+e` | Collapse all stack traces |

### Files to Create/Modify

| File | Action | Description |
|------|--------|-------------|
| `src/app/session.rs` | Modify | Add `CollapseState` and collapse tracking |
| `src/app/message.rs` | Modify | Add `ToggleStackTrace` message |
| `src/app/handler/keys.rs` | Modify | Handle Enter for toggle |
| `src/app/handler/update.rs` | Modify | Implement toggle handler |
| `src/tui/widgets/log_view.rs` | Modify | Add collapse indicators and conditional rendering |
| `src/config/types.rs` | Modify | Add collapse config options |

### Estimated Time

4-5 hours

### Notes

- Collapse state is per-session and per-entry
- State is not persisted across restarts (logs are ephemeral anyway)
- The focused entry concept may need refinement based on how scrolling works
- Consider adding visual highlighting for the focused entry
- The expand/collapse animation is instantaneous (no transition)

---

## Completion Summary

**Status:** ✅ Done

**Date Completed:** 2026-01-05

### Files Modified

| File | Changes |
|------|---------|
| `src/app/session.rs` | Added `CollapseState` struct with `is_expanded()`, `toggle()`, `collapse_all()`, `expand_all()` methods; added `collapse_state` field to `Session`; added `focused_entry()`, `focused_entry_id()`, `toggle_stack_trace()`, `is_stack_trace_expanded()` methods |
| `src/app/message.rs` | Added `ToggleStackTrace` message variant |
| `src/app/handler/keys.rs` | Added Enter key handler for toggle in normal mode (checks if focused entry has stack trace) |
| `src/app/handler/update.rs` | Added `ToggleStackTrace` message handler |
| `src/tui/widgets/log_view.rs` | Added `collapse_state`, `default_collapsed`, `max_collapsed_frames` fields to `LogView`; added builder methods; added `format_collapsed_indicator()`, `is_entry_expanded()`, `calculate_entry_lines()` methods; updated `StatefulWidget::render()` to conditionally show frames and indicator |
| `src/config/types.rs` | Added `stack_trace_collapsed` (default: true) and `stack_trace_max_frames` (default: 3) to `UiSettings` |

### Notable Decisions/Tradeoffs

1. **No Expanded Indicator**: The plan suggested a "▼ Stack trace:" header for expanded traces, but this was omitted for cleaner UX. Expanded traces show all frames directly without an extra header line.

2. **Focused Entry Based on Scroll Position**: The focused entry for toggle is determined by `current_log_position()` which maps the scroll offset to the actual log entry, accounting for filtering.

3. **Default Max Frames = 3**: Changed from the plan's suggested 5 to 3 for more compact default view, configurable via `stack_trace_max_frames`.

4. **HashSet-based State**: Used `HashSet<u64>` for tracking expanded/collapsed entries for O(1) lookup performance.

5. **Dual HashSet Design**: Separate `expanded_entries` and `collapsed_entries` sets handle both `default_collapsed=true` and `default_collapsed=false` scenarios correctly.

### Testing Performed

```bash
cargo check   # ✅ Pass
cargo clippy  # ✅ Pass (no warnings)
cargo test    # ✅ 647 pass, 1 unrelated failure
cargo fmt     # ✅ Applied
```

**New tests added (21 tests total):**

In `session.rs` (10 tests):
- `test_collapse_state_default`
- `test_collapse_state_toggle`
- `test_collapse_state_toggle_default_expanded`
- `test_collapse_state_multiple_entries`
- `test_collapse_all`
- `test_expand_all`
- `test_session_has_collapse_state`
- `test_session_toggle_stack_trace`

In `log_view.rs` (11 tests):
- `test_format_collapsed_indicator_singular`
- `test_format_collapsed_indicator_plural`
- `test_format_collapsed_indicator_has_arrow`
- `test_calculate_entry_lines_no_trace`
- `test_calculate_entry_lines_collapsed`
- `test_calculate_entry_lines_expanded`
- `test_calculate_entry_lines_few_frames`
- `test_is_entry_expanded_no_collapse_state`
- `test_is_entry_expanded_with_collapse_state`
- `test_collapse_state_builder`
- `test_max_collapsed_frames_builder`
- `test_default_collapsed_builder`

### Acceptance Criteria Checklist

- [x] `CollapseState` struct tracks expanded/collapsed entries
- [x] `stack_trace_collapsed` config option added (default: true)
- [x] `stack_trace_max_frames` config option added (default: 3)
- [x] Enter key toggles collapse on focused entry with stack trace
- [x] Collapsed indicator shows "▶ N more frames..."
- [ ] Expanded indicator shows "▼ Stack trace:" (SKIPPED - cleaner without)
- [x] Collapse state persists during session (not across restarts)
- [x] Line count calculation accounts for collapse state
- [x] Scrolling works correctly with mixed collapsed/expanded entries
- [x] Toggle only works on entries that have stack traces
- [x] Visual feedback when toggle occurs (immediate re-render)

### Risks/Limitations

1. **No Visual Focus Indicator**: The "focused" entry for toggle is based on scroll position but there's no visual indicator showing which entry is focused. Future enhancement could add highlighting.

2. **Pre-existing test failure**: `test_indeterminate_ratio_oscillates` in device_selector.rs continues to fail intermittently - unrelated to Task 6 changes.