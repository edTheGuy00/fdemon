## Task: Error Count Status Bar

**Objective**: Display a real-time error count in the status bar, giving users immediate visibility into the number of errors in the current session without needing to scroll through logs.

**Depends on**: [04-integrate-stack-trace-parsing](04-integrate-stack-trace-parsing.md)

### Scope

- `src/tui/widgets/status_bar.rs`: Add error count display
- `src/app/session.rs`: Track error count per session
- `src/core/types.rs`: Ensure `LogEntry::is_error()` is available

### Current Status Bar State

The status bar currently displays:
- App phase (Running/Reloading/etc.)
- Device name
- Reload count
- Hot reload shortcut hints

### Target Status Bar State

Add error count to the status bar:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ ● Running on iPhone 15 Pro │ Reloads: 5 │ ✗ Errors: 3 │ r:reload R:restart │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Error Count Tracking

```rust
// In src/app/session.rs

impl Session {
    // ... existing fields ...
    
    /// Cached count of error-level log entries
    error_count: usize,
}

impl Session {
    /// Add a log entry and update error count
    pub fn add_log(&mut self, entry: LogEntry) {
        if entry.is_error() {
            self.error_count += 1;
        }
        self.logs.push(entry);
    }
    
    /// Get the current error count
    pub fn error_count(&self) -> usize {
        self.error_count
    }
    
    /// Recalculate error count (for consistency/debugging)
    pub fn recalculate_error_count(&mut self) {
        self.error_count = self.logs.iter().filter(|e| e.is_error()).count();
    }
    
    /// Clear logs and reset error count
    pub fn clear_logs(&mut self) {
        self.logs.clear();
        self.error_count = 0;
    }
}
```

### Status Bar Rendering

```rust
// In src/tui/widgets/status_bar.rs

use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

pub struct StatusBarData {
    pub phase: AppPhase,
    pub device_name: Option<String>,
    pub reload_count: u32,
    pub error_count: usize,  // NEW
}

impl StatusBar {
    fn render_error_count(&self, error_count: usize) -> Span<'static> {
        if error_count == 0 {
            // No errors - dim/green indicator
            Span::styled(
                "✓ No errors".to_string(),
                Style::default().fg(Color::DarkGray),
            )
        } else {
            // Has errors - red, attention-grabbing
            let text = if error_count == 1 {
                "✗ 1 error".to_string()
            } else {
                format!("✗ {} errors", error_count)
            };
            
            Span::styled(
                text,
                Style::default()
                    .fg(Color::Red)
                    .add_modifier(Modifier::BOLD),
            )
        }
    }
    
    fn build_status_line(&self, data: &StatusBarData) -> Line<'static> {
        let mut spans = vec![];
        
        // Phase indicator
        spans.push(self.render_phase(data.phase));
        spans.push(Span::raw(" │ "));
        
        // Device name
        if let Some(device) = &data.device_name {
            spans.push(Span::styled(
                device.clone(),
                Style::default().fg(Color::Cyan),
            ));
            spans.push(Span::raw(" │ "));
        }
        
        // Reload count
        spans.push(Span::styled(
            format!("Reloads: {}", data.reload_count),
            Style::default().fg(Color::DarkGray),
        ));
        spans.push(Span::raw(" │ "));
        
        // Error count (NEW)
        spans.push(self.render_error_count(data.error_count));
        
        // Spacer and shortcuts
        spans.push(Span::raw(" │ "));
        spans.push(Span::styled(
            "r:reload R:restart".to_string(),
            Style::default().fg(Color::DarkGray),
        ));
        
        Line::from(spans)
    }
}
```

### Error Count Styling

| State | Icon | Color | Style |
|-------|------|-------|-------|
| No errors | ✓ | DarkGray | Normal |
| 1+ errors | ✗ | Red | Bold |

Optional: Add blinking/pulsing effect for new errors (may be distracting):

```rust
// Only if you want attention-grabbing effect
if error_count > 0 && is_new_error {
    style = style.add_modifier(Modifier::SLOW_BLINK);
}
```

### Integration with AppState

```rust
// In src/app/state.rs or render logic

impl AppState {
    /// Get error count for the current session
    pub fn current_error_count(&self) -> usize {
        self.current_session()
            .map(|s| s.error_count())
            .unwrap_or(0)
    }
}

// In render
let status_data = StatusBarData {
    phase: state.phase,
    device_name: state.device_name.clone(),
    reload_count: state.reload_count,
    error_count: state.current_error_count(),  // NEW
};
```

### Multi-Session Considerations

For multi-session mode, consider showing:
- Error count for current session only
- Or aggregated error count across all sessions with indicator

```rust
// Option 1: Current session only (simpler)
let error_count = state.current_session().map(|s| s.error_count()).unwrap_or(0);

// Option 2: Show both (if multi-session is common)
// "✗ 3 errors (5 total)"
let current_errors = state.current_session().map(|s| s.error_count()).unwrap_or(0);
let total_errors: usize = state.session_manager.sessions()
    .map(|s| s.error_count())
    .sum();
```

### Acceptance Criteria

1. [ ] Error count displayed in status bar
2. [ ] Count updates in real-time as errors are logged
3. [ ] Zero errors shows "✓ No errors" in dim style
4. [ ] 1+ errors shows "✗ N error(s)" in bold red
5. [ ] Error count uses `is_error()` method on `LogEntry`
6. [ ] Count is cached for performance (not recalculated on every render)
7. [ ] Count resets when logs are cleared
8. [ ] Works correctly in multi-session mode (shows current session)
9. [ ] Status bar layout accommodates error count without overflow
10. [ ] Error count updates when navigating between sessions

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_error_count_increments() {
        let mut session = Session::new("test-id", device);
        
        session.add_log(LogEntry::info(LogSource::App, "Hello"));
        assert_eq!(session.error_count(), 0);
        
        session.add_log(LogEntry::error(LogSource::App, "Oops"));
        assert_eq!(session.error_count(), 1);
        
        session.add_log(LogEntry::error(LogSource::App, "Another"));
        assert_eq!(session.error_count(), 2);
        
        session.add_log(LogEntry::warn(LogSource::App, "Warning"));
        assert_eq!(session.error_count(), 2); // Warnings don't count
    }
    
    #[test]
    fn test_error_count_clear() {
        let mut session = Session::new("test-id", device);
        
        session.add_log(LogEntry::error(LogSource::App, "Error 1"));
        session.add_log(LogEntry::error(LogSource::App, "Error 2"));
        assert_eq!(session.error_count(), 2);
        
        session.clear_logs();
        assert_eq!(session.error_count(), 0);
    }
    
    #[test]
    fn test_render_error_count_zero() {
        let status_bar = StatusBar::new();
        let span = status_bar.render_error_count(0);
        
        // Should show "No errors" in dim style
        assert!(span.content.contains("No errors"));
    }
    
    #[test]
    fn test_render_error_count_singular() {
        let status_bar = StatusBar::new();
        let span = status_bar.render_error_count(1);
        
        // Should show "1 error" (singular)
        assert!(span.content.contains("1 error"));
        assert!(!span.content.contains("errors"));
    }
    
    #[test]
    fn test_render_error_count_plural() {
        let status_bar = StatusBar::new();
        let span = status_bar.render_error_count(5);
        
        // Should show "5 errors" (plural)
        assert!(span.content.contains("5 errors"));
    }
    
    #[test]
    fn test_recalculate_error_count() {
        let mut session = Session::new("test-id", device);
        
        // Manually add logs without going through add_log
        session.logs.push(LogEntry::error(LogSource::App, "Error"));
        session.logs.push(LogEntry::info(LogSource::App, "Info"));
        
        // Count is stale
        assert_eq!(session.error_count, 0);
        
        // Recalculate
        session.recalculate_error_count();
        assert_eq!(session.error_count(), 1);
    }
}
```

### Manual Testing Checklist

Using enhanced sample apps:

- [ ] Start Flutter Demon with sample app
- [ ] Verify "✓ No errors" shown initially (dim)
- [ ] Trigger an error via test button
- [ ] Verify "✗ 1 error" appears in bold red
- [ ] Trigger more errors
- [ ] Verify count updates correctly ("✗ 3 errors")
- [ ] Clear logs (if feature exists) or restart
- [ ] Verify count resets
- [ ] With multiple sessions, verify count shows current session
- [ ] Switch sessions and verify count updates

### Status Bar Visual Layout

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ ● Running │ iPhone 15 Pro │ Reloads: 5 │ ✗ 3 errors │ r:reload R:restart q │
└─────────────────────────────────────────────────────────────────────────────┘

Without errors:
┌─────────────────────────────────────────────────────────────────────────────┐
│ ● Running │ iPhone 15 Pro │ Reloads: 5 │ ✓ No errors │ r:reload R:restart  │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Files to Modify

| File | Action | Description |
|------|--------|-------------|
| `src/app/session.rs` | Modify | Add `error_count` field and tracking methods |
| `src/tui/widgets/status_bar.rs` | Modify | Add error count rendering |
| `src/tui/render.rs` | Modify | Pass error count to status bar data |

### Estimated Time

2-3 hours

### Notes

- Error count is incremented on `add_log()`, not recalculated every render
- Only `LogLevel::Error` counts as an error (not warnings)
- The `e`/`E` navigation (from Phase 1) complements this feature - users can quickly jump to errors after seeing the count
- Consider adding a "click" action (via mouse or Enter) on the error count to jump to first error
- Future enhancement: Show warning count as well ("3 errors, 5 warnings")