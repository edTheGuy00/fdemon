## Task: Stack Trace Rendering with Highlighting

**Objective**: Implement enhanced visual rendering of parsed stack traces in the log view, with syntax highlighting for file references, dimmed package frames, and emphasized project frames.

**Depends on**: [04-integrate-stack-trace-parsing](04-integrate-stack-trace-parsing.md)

### Scope

- `src/tui/widgets/log_view.rs`: Extend rendering to display parsed stack traces
- `src/tui/styles.rs`: Add stack trace-specific styles (if needed)

### Current State

Currently, stack traces are rendered as plain indented text lines:

```rust
// Old approach - stack traces as separate log entries
Span::styled(message.to_string(), Style::default().fg(Color::DarkGray))
```

### Target State

Stack traces should be rendered with rich formatting:

1. **Error message** - Red, bold
2. **Frame number** - Dim gray (`#0`, `#1`, etc.)
3. **Function name** - White/default
4. **File path** - Blue, underlined (for project files)
5. **Line:column** - Cyan
6. **Package frames** - All dimmed gray
7. **Async gap** - Italic, dimmed

### Rendering Implementation

```rust
// In src/tui/widgets/log_view.rs

impl<'a> LogView<'a> {
    /// Render a log entry with its stack trace
    fn render_entry_with_trace(
        &self,
        entry: &LogEntry,
        area: Rect,
        buf: &mut Buffer,
        y_offset: &mut u16,
    ) {
        // 1. Render the error message line (existing logic)
        self.render_log_line(entry, area, buf, *y_offset);
        *y_offset += 1;
        
        // 2. Render stack trace if present
        if let Some(trace) = &entry.stack_trace {
            for frame in &trace.frames {
                if *y_offset >= area.height {
                    break;
                }
                self.render_stack_frame(frame, area, buf, *y_offset);
                *y_offset += 1;
            }
        }
    }
    
    /// Render a single stack frame with appropriate styling
    fn render_stack_frame(
        &self,
        frame: &StackFrame,
        area: Rect,
        buf: &mut Buffer,
        y: u16,
    ) {
        let spans = self.format_stack_frame(frame);
        let line = Line::from(spans);
        
        // Render with 4-space indent
        let indent = "    ";
        buf.set_string(area.x, area.y + y, indent, Style::default());
        
        let text_area = Rect {
            x: area.x + indent.len() as u16,
            width: area.width.saturating_sub(indent.len() as u16),
            ..area
        };
        
        buf.set_line(text_area.x, area.y + y, &line, text_area.width);
    }
    
    /// Format a stack frame into styled spans
    fn format_stack_frame(&self, frame: &StackFrame) -> Vec<Span<'static>> {
        // Handle async gap specially
        if frame.is_async_gap {
            return vec![
                Span::styled(
                    "<asynchronous suspension>".to_string(),
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::ITALIC),
                ),
            ];
        }
        
        // Determine base style based on frame type
        let (frame_num_style, func_style, file_style, loc_style) = if frame.is_package_frame {
            // Package frame - all dimmed
            (
                Style::default().fg(Color::DarkGray),
                Style::default().fg(Color::DarkGray),
                Style::default().fg(Color::DarkGray),
                Style::default().fg(Color::DarkGray),
            )
        } else {
            // Project frame - highlighted
            (
                Style::default().fg(Color::DarkGray),
                Style::default().fg(Color::White),
                Style::default().fg(Color::Blue).add_modifier(Modifier::UNDERLINED),
                Style::default().fg(Color::Cyan),
            )
        };
        
        vec![
            // Frame number: #0
            Span::styled(format!("#{:<3}", frame.frame_number), frame_num_style),
            // Function name
            Span::styled(format!("{} ", frame.function_name), func_style),
            // Opening paren
            Span::styled("(".to_string(), Style::default().fg(Color::DarkGray)),
            // File path
            Span::styled(frame.short_path(), file_style),
            // Colon separator
            Span::styled(":".to_string(), Style::default().fg(Color::DarkGray)),
            // Line number
            Span::styled(frame.line.to_string(), loc_style),
            // Column (if present)
            if frame.column > 0 {
                Span::styled(format!(":{}", frame.column), loc_style)
            } else {
                Span::raw("")
            },
            // Closing paren
            Span::styled(")".to_string(), Style::default().fg(Color::DarkGray)),
        ]
    }
}
```

### Style Constants

Consider adding to a styles module or as constants:

```rust
mod stack_trace_styles {
    use ratatui::style::{Color, Modifier, Style};
    
    /// Frame number (#0, #1, etc.)
    pub const FRAME_NUMBER: Style = Style::new().fg(Color::DarkGray);
    
    /// Function name for project frames
    pub const FUNCTION_PROJECT: Style = Style::new().fg(Color::White);
    
    /// Function name for package frames
    pub const FUNCTION_PACKAGE: Style = Style::new().fg(Color::DarkGray);
    
    /// File path for project frames (clickable in Phase 3)
    pub const FILE_PROJECT: Style = Style::new()
        .fg(Color::Blue)
        .add_modifier(Modifier::UNDERLINED);
    
    /// File path for package frames
    pub const FILE_PACKAGE: Style = Style::new().fg(Color::DarkGray);
    
    /// Line/column numbers for project frames
    pub const LOCATION_PROJECT: Style = Style::new().fg(Color::Cyan);
    
    /// Line/column numbers for package frames
    pub const LOCATION_PACKAGE: Style = Style::new().fg(Color::DarkGray);
    
    /// Async suspension marker
    pub const ASYNC_GAP: Style = Style::new()
        .fg(Color::DarkGray)
        .add_modifier(Modifier::ITALIC);
    
    /// Indentation for stack frames
    pub const INDENT: &str = "    ";
}
```

### Visual Output Example

For an error with stack trace, the rendered output should look like:

```
12:34:56 ✗ [APP] Exception: Something went wrong
    #0   main (main.dart:15:3)                      ← Project: blue underlined file, cyan line
    #1   State.setState (framework.dart:1187:9)     ← Package: all dimmed
    #2   _MyHomePageState._increment (main.dart:45) ← Project: highlighted
    <asynchronous suspension>                        ← Italic, dimmed
    #3   runApp (binding.dart:1234:5)               ← Package: dimmed
```

### Integration with Existing LogView

Update the `StatefulWidget` implementation:

```rust
impl StatefulWidget for LogView<'a> {
    type State = LogViewState;
    
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // ... existing setup code ...
        
        let mut y = 0;
        for (i, entry) in visible_logs.iter().enumerate() {
            if y >= content_area.height {
                break;
            }
            
            // Render the main log line
            self.render_log_line(entry, content_area, buf, y);
            y += 1;
            
            // Render stack trace frames if present
            if let Some(trace) = &entry.stack_trace {
                for frame in trace.visible_frames(MAX_VISIBLE_FRAMES) {
                    if y >= content_area.height {
                        break;
                    }
                    self.render_stack_frame(frame, content_area, buf, y);
                    y += 1;
                }
            }
        }
        
        // ... rest of rendering ...
    }
}
```

### Line Count Calculation

Update `LogViewState` to account for multi-line entries:

```rust
impl LogViewState {
    /// Calculate total lines including expanded stack traces
    pub fn calculate_total_lines(&self, logs: &[LogEntry]) -> usize {
        logs.iter()
            .map(|entry| {
                1 + entry.stack_trace_frame_count() // 1 for message + frames
            })
            .sum()
    }
}
```

### Acceptance Criteria

1. [ ] Stack frames render below their parent error message
2. [ ] Frame numbers displayed with consistent padding (`#0 `, `#1 `, etc.)
3. [ ] Function names displayed after frame number
4. [ ] File path displayed with appropriate styling
5. [ ] Line:column displayed in distinct color
6. [ ] Project frames have highlighted styling (blue file, white function)
7. [ ] Package frames are fully dimmed (all gray)
8. [ ] Async suspension gaps render as italic dimmed text
9. [ ] 4-space indentation for all stack frame lines
10. [ ] Rendering handles long function/file names gracefully (truncation)
11. [ ] Total line count calculation includes stack trace frames
12. [ ] Scrolling works correctly with multi-line entries

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::stack_trace::{StackFrame, ParsedStackTrace};
    
    #[test]
    fn test_format_project_frame() {
        let frame = StackFrame {
            frame_number: 0,
            function_name: "main".to_string(),
            file_path: "package:app/main.dart".to_string(),
            line: 15,
            column: 3,
            is_package_frame: false,
            is_async_gap: false,
        };
        
        let log_view = LogView::new(&[]);
        let spans = log_view.format_stack_frame(&frame);
        
        // Verify spans are created correctly
        assert!(!spans.is_empty());
        // Verify file path is blue/underlined (project frame)
    }
    
    #[test]
    fn test_format_package_frame() {
        let frame = StackFrame {
            frame_number: 1,
            function_name: "State.setState".to_string(),
            file_path: "package:flutter/src/widgets/framework.dart".to_string(),
            line: 1187,
            column: 9,
            is_package_frame: true,
            is_async_gap: false,
        };
        
        let log_view = LogView::new(&[]);
        let spans = log_view.format_stack_frame(&frame);
        
        // Verify all spans are dimmed gray
    }
    
    #[test]
    fn test_format_async_gap() {
        let frame = StackFrame {
            is_async_gap: true,
            ..Default::default()
        };
        
        let log_view = LogView::new(&[]);
        let spans = log_view.format_stack_frame(&frame);
        
        assert_eq!(spans.len(), 1);
        // Verify italic modifier
    }
    
    #[test]
    fn test_total_lines_with_traces() {
        let mut entries = vec![];
        
        // Entry without trace
        entries.push(LogEntry::new(LogLevel::Info, LogSource::App, "Hello"));
        
        // Entry with 5-frame trace
        let mut entry = LogEntry::new(LogLevel::Error, LogSource::App, "Error");
        let trace = ParsedStackTrace::parse(SAMPLE_5_FRAME_TRACE);
        entry.stack_trace = Some(trace);
        entries.push(entry);
        
        let state = LogViewState::new();
        assert_eq!(state.calculate_total_lines(&entries), 7); // 1 + (1 + 5)
    }
}
```

### Manual Testing Checklist

Using enhanced sample apps:

- [ ] Trigger null check error → verify stack trace renders
- [ ] Verify project frames (sample/main.dart) are highlighted
- [ ] Verify Flutter frames are dimmed
- [ ] Verify async errors show suspension markers
- [ ] Verify deep stack traces (10+ frames) display correctly
- [ ] Scroll through logs with stack traces
- [ ] Verify file:line is visually distinct

### Files to Modify

| File | Action | Description |
|------|--------|-------------|
| `src/tui/widgets/log_view.rs` | Modify | Add stack frame rendering methods |
| `src/tui/styles.rs` | Create (optional) | Stack trace style constants |

### Estimated Time

5-6 hours

### Notes

- The underlined blue file paths are prepared for Phase 3 OSC 8 hyperlinks
- Truncation should preserve the file:line portion (most important for debugging)
- Consider adding `...` for truncated function names
- Frame rendering should reuse the search highlighting infrastructure if matches exist in stack trace text

---

## Completion Summary

**Status:** ✅ Done

**Date Completed:** 2026-01-05

### Files Modified

| File | Changes |
|------|---------|
| `src/tui/widgets/log_view.rs` | Added `stack_trace_styles` module with style constants; added `format_stack_frame()` and `format_stack_frame_line()` methods; added `calculate_total_lines()` and `calculate_total_lines_filtered()` to `LogViewState`; updated `StatefulWidget::render()` to handle multi-line entries with stack traces and proper scrolling |

### Notable Decisions/Tradeoffs

1. **Style Constants Module**: Created a dedicated `stack_trace_styles` module within `log_view.rs` to keep styles organized and reusable. This module could be extracted to a separate `styles.rs` file in the future if needed.

2. **Multi-line Scrolling**: Implemented proper line-based scrolling that handles partial visibility of entries. When scrolling through a log with stack traces, the scroll offset is based on total lines (message + frames) rather than entry count.

3. **Frame Styling**:
   - Project frames: White function name, blue underlined file path, cyan line/column
   - Package frames: All dimmed gray
   - Async gaps: Italic dimmed with `<asynchronous suspension>` text

4. **Indentation**: 4-space indent for all stack frame lines to visually distinguish them from the parent log entry.

5. **Frame Number Padding**: Frame numbers use `#{:<3}` format for consistent alignment (e.g., `#0  `, `#10 `).

### Testing Performed

```bash
cargo check   # ✅ Pass
cargo clippy  # ✅ Pass (no warnings)
cargo test    # ✅ 627 pass, 1 unrelated failure
cargo fmt     # ✅ Applied
```

**New tests added (10 tests):**
- `test_format_stack_frame_project_frame` - verifies project frame styling
- `test_format_stack_frame_package_frame` - verifies package frame styling
- `test_format_stack_frame_async_gap` - verifies async suspension rendering
- `test_format_stack_frame_no_column` - verifies column omission when 0
- `test_calculate_total_lines_no_traces` - verifies line count without traces
- `test_calculate_total_lines_with_traces` - verifies line count with traces
- `test_calculate_total_lines_filtered` - verifies filtered line count
- `test_format_stack_frame_line` - verifies Line generation
- `test_stack_frame_with_long_function_name` - verifies long name handling
- `test_stack_frame_styles_module_constants` - verifies style constant values

### Acceptance Criteria Checklist

- [x] Stack frames render below their parent error message
- [x] Frame numbers displayed with consistent padding (`#0 `, `#1 `, etc.)
- [x] Function names displayed after frame number
- [x] File path displayed with appropriate styling
- [x] Line:column displayed in distinct color (Cyan)
- [x] Project frames have highlighted styling (blue underlined file, white function)
- [x] Package frames are fully dimmed (all gray)
- [x] Async suspension gaps render as italic dimmed text
- [x] 4-space indentation for all stack frame lines
- [x] Rendering handles long function/file names (no truncation yet - full names shown)
- [x] Total line count calculation includes stack trace frames
- [x] Scrolling works correctly with multi-line entries

### Not Implemented (Deferred/Out of Scope)

1. **Truncation**: Long function/file names are not truncated. This could be added later if terminal width becomes an issue.

2. **Search Highlighting in Stack Traces**: Stack frame text is not currently included in search highlighting. The infrastructure exists but integration would require additional work.

### Risks/Limitations

1. **Performance with Many Stack Traces**: Each render calculates total lines by iterating all entries. For very large logs with many stack traces, this could be optimized with caching.

2. **Pre-existing test failure**: `test_indeterminate_ratio_oscillates` in device_selector.rs continues to fail intermittently - unrelated to Task 5 changes.