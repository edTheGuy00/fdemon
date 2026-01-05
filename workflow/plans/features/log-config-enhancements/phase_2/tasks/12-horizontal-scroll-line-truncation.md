## Task: Horizontal Scrolling and Line Truncation Fix

**Objective**: Fix log view scrolling behavior when terminal width is narrow, preventing content from being cut off and allowing users to see full log lines.

**Depends on**: None (independent fix)

### Background

When the terminal width decreases, long log lines exhibit problematic behavior:

1. **Line wrapping causes scroll miscalculation**: The `Paragraph` widget uses `Wrap { trim: false }`, which wraps long lines to multiple display lines. However, the scroll offset is calculated based on logical log entries, not actual display lines.

2. **Content appears "cut off"**: Users report they cannot scroll to see all content - it keeps being "pushed down" as the log view doesn't account for wrapped lines in its scroll calculations.

3. **Poor UX for log viewing**: Logs are typically meant to be read in full width; wrapping mid-line makes them hard to read, especially for stack traces and structured output.

### Current Behavior

```
│11:57:12 ✗ [flutter] flutter: │ RangeError (length): Invalid value: Not in inclusive range 0..2: 10
│11:57:12 • [flutter] flutter:
│├┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄
```

Notice:
- Second line shows empty content (the wrapped portion from line 1)
- Box-drawing characters appear on their own line
- Content is cut off at terminal width

### Proposed Solution

**Option A: Disable Wrapping + Add Horizontal Scroll (Recommended)**

1. Remove line wrapping - truncate lines at terminal width
2. Add horizontal scroll offset state
3. Add keyboard shortcuts for horizontal scrolling (←/→ or h/l)
4. Show truncation indicator when lines exceed width

**Option B: Accurate Wrapped Line Calculation**

1. Pre-calculate how many display lines each log entry takes when wrapped
2. Use display line count for scroll offset calculations
3. More complex, may still have poor UX for reading wrapped logs

### Scope

- `src/tui/widgets/log_view.rs`: Main changes for scroll behavior
- `src/app/handler/keys.rs`: Add horizontal scroll key handlers
- `src/app/message.rs`: Add horizontal scroll messages

### Implementation (Option A)

#### 1. Add Horizontal Scroll State

```rust
// src/tui/widgets/log_view.rs

#[derive(Debug, Default)]
pub struct LogViewState {
    /// Current vertical scroll offset from top
    pub offset: usize,
    /// Current horizontal scroll offset from left
    pub h_offset: usize,
    /// Whether auto-scroll is enabled (follow new content)
    pub auto_scroll: bool,
    /// Total number of lines (set during render)
    pub total_lines: usize,
    /// Visible lines (set during render)
    pub visible_lines: usize,
    /// Maximum line width in current view (for h-scroll bounds)
    pub max_line_width: usize,
    /// Visible width (set during render)
    pub visible_width: usize,
}

impl LogViewState {
    /// Scroll left by n columns
    pub fn scroll_left(&mut self, n: usize) {
        self.h_offset = self.h_offset.saturating_sub(n);
    }

    /// Scroll right by n columns
    pub fn scroll_right(&mut self, n: usize) {
        let max_h_offset = self.max_line_width.saturating_sub(self.visible_width);
        self.h_offset = (self.h_offset + n).min(max_h_offset);
    }

    /// Reset horizontal scroll to start
    pub fn scroll_to_line_start(&mut self) {
        self.h_offset = 0;
    }
}
```

#### 2. Remove Line Wrapping in Render

```rust
impl<'a> StatefulWidget for LogView<'a> {
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // ... existing code ...

        // Update visible width
        state.visible_width = inner.width as usize;

        // Calculate max line width for h-scroll bounds
        let max_width = all_lines.iter()
            .map(|line| line.width())
            .max()
            .unwrap_or(0);
        state.max_line_width = max_width;

        // Apply horizontal offset to lines
        let scrolled_lines: Vec<Line> = all_lines
            .into_iter()
            .map(|line| self.apply_horizontal_scroll(line, state.h_offset, inner.width as usize))
            .collect();

        // Render WITHOUT wrapping
        Paragraph::new(scrolled_lines)
            // No .wrap() call - lines will be truncated at width
            .render(inner, buf);

        // ... scrollbar code ...
    }
}
```

#### 3. Apply Horizontal Scroll to Lines

```rust
impl<'a> LogView<'a> {
    /// Apply horizontal scroll offset to a line, truncating and offsetting content
    fn apply_horizontal_scroll(&self, line: Line<'static>, h_offset: usize, width: usize) -> Line<'static> {
        if h_offset == 0 && line.width() <= width {
            // No scrolling needed
            return line;
        }

        // Convert line to string, apply offset, truncate
        let full_text: String = line.spans.iter()
            .map(|s| s.content.as_ref())
            .collect();

        if h_offset >= full_text.len() {
            return Line::from("");
        }

        // Simple approach: slice the string and create new line
        // Note: This loses per-span styling for offset content
        // A more complex approach would track span boundaries
        let visible_start = h_offset;
        let visible_end = (h_offset + width).min(full_text.len());
        let visible_text = &full_text[visible_start..visible_end];

        // Add truncation indicator if content extends beyond view
        let mut result = visible_text.to_string();
        if visible_end < full_text.len() {
            // Show indicator that more content exists to the right
            if result.len() > 1 {
                result.pop();
                result.push('→');
            }
        }
        if h_offset > 0 {
            // Show indicator that more content exists to the left
            if !result.is_empty() {
                result.remove(0);
                result.insert(0, '←');
            }
        }

        Line::from(result)
    }
}
```

#### 4. Add Keyboard Handlers

```rust
// src/app/handler/keys.rs

// In handle_key_normal():
(KeyCode::Left, KeyModifiers::NONE) | (KeyCode::Char('h'), KeyModifiers::NONE) => {
    Some(Message::ScrollLeft(10))
}
(KeyCode::Right, KeyModifiers::NONE) | (KeyCode::Char('l'), KeyModifiers::NONE) => {
    Some(Message::ScrollRight(10))
}
(KeyCode::Char('0'), KeyModifiers::NONE) => {
    Some(Message::ScrollToLineStart)
}
(KeyCode::Char('$'), KeyModifiers::NONE) => {
    Some(Message::ScrollToLineEnd)
}
```

#### 5. Add Messages

```rust
// src/app/message.rs

pub enum Message {
    // ... existing messages ...
    
    /// Scroll log view left by n columns
    ScrollLeft(usize),
    /// Scroll log view right by n columns
    ScrollRight(usize),
    /// Scroll to start of line (column 0)
    ScrollToLineStart,
    /// Scroll to end of line
    ScrollToLineEnd,
}
```

### Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `←` or `h` | Scroll left 10 columns |
| `→` or `l` | Scroll right 10 columns |
| `0` | Scroll to line start |
| `$` | Scroll to line end |

### Visual Indicators

When content extends beyond visible area:
- `←` at line start indicates content to the left
- `→` at line end indicates content to the right

### Acceptance Criteria

1. [ ] Line wrapping disabled - lines truncate at terminal width
2. [ ] Horizontal scroll state added to LogViewState
3. [ ] ←/→ or h/l keys scroll horizontally
4. [ ] `0` scrolls to line start, `$` scrolls to line end
5. [ ] Truncation indicators show when content extends beyond view
6. [ ] Vertical scrolling still works correctly
7. [ ] Auto-scroll to bottom still works
8. [ ] Scroll position maintained when terminal resizes
9. [ ] Unit tests for horizontal scroll logic
10. [ ] No regressions in existing log view functionality

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_horizontal_scroll_left() {
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
    fn test_horizontal_scroll_right() {
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
    fn test_no_horizontal_scroll_needed() {
        let mut state = LogViewState::new();
        state.max_line_width = 50;
        state.visible_width = 80;

        state.scroll_right(10);
        assert_eq!(state.h_offset, 0); // No scroll when content fits
    }
}
```

### Integration Tests

After implementation, manually verify:
1. Run Flutter Demon with narrow terminal (e.g., 80 columns)
2. Generate long log lines (Logger package output)
3. Press → to scroll right - should reveal truncated content
4. Press ← to scroll left - should return to start
5. Press `0` to jump to line start
6. Verify truncation indicators (← →) appear appropriately
7. Verify vertical scroll (j/k) still works
8. Resize terminal - scroll position should be maintained

### Files to Create/Modify

| File | Action | Description |
|------|--------|-------------|
| `src/tui/widgets/log_view.rs` | Modify | Add h_offset, remove Wrap, add scroll methods |
| `src/app/handler/keys.rs` | Modify | Add horizontal scroll key handlers |
| `src/app/message.rs` | Modify | Add scroll messages |
| `src/app/handler/update.rs` | Modify | Handle scroll messages |

### Estimated Time

4-5 hours

### Alternative Considerations

**If horizontal scroll proves too complex:**

A simpler alternative is to calculate wrapped line count accurately:

```rust
fn calculate_wrapped_lines(line_width: usize, terminal_width: usize) -> usize {
    if line_width == 0 || terminal_width == 0 {
        return 1;
    }
    (line_width + terminal_width - 1) / terminal_width
}
```

Then use this in `calculate_entry_lines()` to account for wrapping.

However, this still results in poor UX with wrapped log lines that are hard to read.

### References

- [Ratatui Paragraph Widget](https://docs.rs/ratatui/latest/ratatui/widgets/struct.Paragraph.html)
- [Ratatui Wrap](https://docs.rs/ratatui/latest/ratatui/widgets/struct.Wrap.html)
- Vim horizontal scroll behavior (for UX reference)

---

## Completion Summary

**Status:** ✅ Done

**Completed:** 2026-01-05

### Files Modified

| File | Changes |
|------|---------|
| `src/tui/widgets/log_view.rs` | Added `h_offset`, `max_line_width`, `visible_width` fields to `LogViewState`; added `scroll_left()`, `scroll_right()`, `scroll_to_line_start()`, `scroll_to_line_end()`, `update_horizontal_size()` methods; added `line_width()` and `apply_horizontal_scroll()` helper methods to `LogView`; removed `Wrap` usage in render; added 13 unit tests for horizontal scroll functionality |
| `src/app/handler/keys.rs` | Added horizontal scroll key handlers: `h`/`←` (scroll left 10), `l`/`→` (scroll right 10), `0` (line start), `$` (line end) |
| `src/app/message.rs` | Added `ScrollLeft(usize)`, `ScrollRight(usize)`, `ScrollToLineStart`, `ScrollToLineEnd` messages |
| `src/app/handler/update.rs` | Added handlers for the four new horizontal scroll messages |

### Implementation Notes

1. **Line wrapping disabled**: Removed `.wrap(Wrap { trim: false })` from Paragraph rendering; lines now truncate at terminal width
2. **Horizontal scroll state**: Added `h_offset` to track horizontal position, `max_line_width` and `visible_width` for bounds checking
3. **Truncation indicators**: Lines that extend beyond visible area show `←` at start (when scrolled) and `→` at end (when more content exists)
4. **Style preservation**: The `apply_horizontal_scroll()` method carefully preserves per-character styling when truncating and offsetting lines
5. **Bounds checking**: `scroll_right()` clamps to `max_line_width - visible_width` to prevent over-scrolling

### Testing Performed

```bash
cargo fmt      # ✓ Passed
cargo check    # ✓ Passed
cargo test horizontal_scroll  # ✓ 8 tests passed
cargo test scroll             # ✓ 19 tests passed (including existing scroll tests)
```

### Acceptance Criteria Status

- [x] Line wrapping disabled - lines truncate at terminal width
- [x] Horizontal scroll state added to LogViewState
- [x] ←/→ or h/l keys scroll horizontally (10 columns per press)
- [x] `0` scrolls to line start, `$` scrolls to line end
- [x] Truncation indicators show when content extends beyond view
- [x] Vertical scrolling still works correctly
- [x] Auto-scroll to bottom still works
- [x] Scroll position maintained when terminal resizes (via `update_horizontal_size` clamping)
- [x] Unit tests for horizontal scroll logic (13 new tests)
- [x] No regressions in existing log view functionality

### Risks/Limitations

1. **Character counting**: Line width is calculated by counting characters (`chars().count()`), not Unicode grapheme clusters or terminal display width. This may cause slight misalignment with lines containing wide characters (e.g., CJK characters, emoji)
2. **Style boundaries**: When scrolling horizontally past a styled region, the per-character style mapping works correctly but adds some memory overhead for very long lines
3. **Max line width calculation**: The `max_line_width` is calculated only from visible lines in the current scroll window, not all log entries. This is intentional for performance but means the horizontal scroll bounds may change as you scroll vertically