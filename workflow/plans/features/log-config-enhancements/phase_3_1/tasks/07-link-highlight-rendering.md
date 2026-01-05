## Task: 07-link-highlight-rendering

**Objective**: Implement the visual rendering of highlighted links in the log view when Link Highlight Mode is active, showing shortcut indicators next to each detected file reference.

**Depends on**: 06-key-and-update-handlers

### Background

When the user enters Link Highlight Mode, all detected file references need to be visually highlighted in the log view. Each link should show its shortcut key (1-9, a-z) so the user knows what to press to open that file.

### Scope

- `src/tui/widgets/log_view.rs`:
  - Modify rendering to check for link highlight mode
  - Insert shortcut indicators before file references
  - Apply highlight styling to link text

### Visual Design

When link mode is active, file references should appear like this:

**Before (normal mode):**
```
[ERROR] Exception at lib/main.dart:42:5
  #0  MyWidget.build (lib/widgets/my_widget.dart:15:10)
  #1  StatelessElement.build (package:flutter/src/widgets.dart:23)
```

**After (link mode active):**
```
[ERROR] Exception at [1]lib/main.dart:42:5
  #0  MyWidget.build ([2]lib/widgets/my_widget.dart:15:10)
  #1  StatelessElement.build ([3]package:flutter/src/widgets.dart:23)
```

### Styling

| Element | Style |
|---------|-------|
| Shortcut badge `[1]` | Cyan background, black text, bold |
| File reference text | Underline + cyan foreground |
| Non-link text | Normal (unchanged) |

### Implementation Approach

#### Option A: Modify Line Generation (Recommended)

Modify `format_entry()` and `format_stack_frame_line()` to accept link highlight state and insert shortcut badges when appropriate.

```rust
// In LogView impl

fn format_entry_with_links(
    &self,
    entry: &LogEntry,
    idx: usize,
    link_state: Option<&LinkHighlightState>,
) -> Line<'static> {
    // ... existing format logic ...
    
    // If link mode active, check if this entry has a link
    if let Some(state) = link_state {
        if state.is_active() {
            // Find link for this entry (no frame = from message)
            if let Some(link) = state.links.iter().find(|l| 
                l.entry_index == idx && l.frame_index.is_none()
            ) {
                // Insert shortcut badge before the file reference
                // This requires modifying the spans
                return self.insert_link_badge(formatted_line, link);
            }
        }
    }
    
    formatted_line
}
```

#### Option B: Post-process Lines (Alternative)

Generate lines normally, then scan and modify them if link mode is active.

### Required Changes to `LogView`

#### 1. Pass link state to LogView

Modify `LogView` struct to optionally hold link highlight state:

```rust
pub struct LogView<'a> {
    // ... existing fields ...
    
    /// Link highlight state for rendering badges (Phase 3.1)
    link_highlight_state: Option<&'a LinkHighlightState>,
}

impl<'a> LogView<'a> {
    // Add builder method
    pub fn link_highlight_state(mut self, state: &'a LinkHighlightState) -> Self {
        if state.is_active() {
            self.link_highlight_state = Some(state);
        }
        self
    }
}
```

#### 2. Create badge span helper

```rust
impl<'a> LogView<'a> {
    /// Create a styled shortcut badge like "[1]" or "[a]"
    fn link_badge(shortcut: char) -> Span<'static> {
        Span::styled(
            format!("[{}]", shortcut),
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
    }
    
    /// Style for highlighted file reference text
    fn link_text_style() -> Style {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::UNDERLINED)
    }
}
```

#### 3. Modify format_entry() to support link badges

```rust
fn format_entry(
    &self,
    entry: &LogEntry,
    idx: usize,
) -> Line<'static> {
    let mut spans = vec![/* existing spans */];
    
    // Check if we need to add a link badge for this entry's message
    if let Some(link_state) = self.link_highlight_state {
        if let Some(link) = link_state.links.iter().find(|l| 
            l.entry_index == idx && l.frame_index.is_none()
        ) {
            // We need to insert the badge before the file reference
            // This is complex because we need to find where in the message
            // the file ref appears and split the spans
            spans = self.insert_message_link_badge(spans, &entry.message, link);
        }
    }
    
    Line::from(spans)
}
```

#### 4. Modify format_stack_frame_line() to support link badges

```rust
fn format_stack_frame_line_with_links(
    frame: &StackFrame,
    entry_index: usize,
    frame_index: usize,
    link_state: Option<&LinkHighlightState>,
) -> Line<'static> {
    let mut spans = vec![/* existing frame formatting */];
    
    // Check if this frame has a link
    if let Some(state) = link_state {
        if let Some(link) = state.links.iter().find(|l| 
            l.entry_index == entry_index && l.frame_index == Some(frame_index)
        ) {
            // Insert badge before the location part
            spans = Self::insert_frame_link_badge(spans, link);
        }
    }
    
    Line::from(spans)
}
```

### Span Manipulation Helper

The tricky part is inserting the badge into the correct position within the spans. Here's a helper approach:

```rust
impl<'a> LogView<'a> {
    /// Insert a link badge into a line's spans before the file reference.
    ///
    /// This finds the span containing the file:line reference and splits it
    /// to insert the badge.
    fn insert_link_badge_into_spans(
        spans: Vec<Span<'static>>,
        file_ref: &str,
        shortcut: char,
    ) -> Vec<Span<'static>> {
        let mut result = Vec::new();
        let badge = Self::link_badge(shortcut);
        let mut badge_inserted = false;
        
        for span in spans {
            if !badge_inserted {
                if let Some(pos) = span.content.find(file_ref) {
                    // Found the file reference in this span
                    // Split: before | badge | file_ref (styled)
                    
                    let before = &span.content[..pos];
                    let file_part = &span.content[pos..pos + file_ref.len()];
                    let after = &span.content[pos + file_ref.len()..];
                    
                    if !before.is_empty() {
                        result.push(Span::styled(before.to_string(), span.style));
                    }
                    
                    result.push(badge.clone());
                    result.push(Span::styled(
                        file_part.to_string(),
                        Self::link_text_style(),
                    ));
                    
                    if !after.is_empty() {
                        result.push(Span::styled(after.to_string(), span.style));
                    }
                    
                    badge_inserted = true;
                    continue;
                }
            }
            result.push(span);
        }
        
        result
    }
}
```

### Update Render Loop

In `StatefulWidget::render()`, pass the link state when formatting:

```rust
fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
    // ... existing code ...
    
    for &idx in &filtered_indices {
        let entry = &self.logs[idx];
        
        // Format entry with link highlighting
        if self.link_highlight_state.is_some() {
            all_lines.push(self.format_entry_with_links(entry, idx));
        } else {
            all_lines.push(self.format_entry(entry, idx));
        }
        
        // Stack frames
        if let Some(trace) = &entry.stack_trace {
            for (frame_idx, frame) in trace.frames.iter().enumerate() {
                if self.link_highlight_state.is_some() {
                    all_lines.push(Self::format_stack_frame_line_with_links(
                        frame,
                        idx,
                        frame_idx,
                        self.link_highlight_state,
                    ));
                } else {
                    all_lines.push(Self::format_stack_frame_line(frame));
                }
            }
        }
    }
    
    // ... rest of render ...
}
```

### Update render.rs to Pass Link State

In `tui/render.rs`, pass the link highlight state to LogView:

```rust
// In view() function
if let Some(handle) = state.session_manager.selected_mut() {
    let mut log_view = widgets::LogView::new(&handle.session.logs)
        .filter_state(&handle.session.filter_state);

    // Add link highlight state if active
    if handle.session.link_highlight_state.is_active() {
        log_view = log_view.link_highlight_state(&handle.session.link_highlight_state);
    }

    // ... rest of log view setup ...
    
    frame.render_stateful_widget(log_view, areas.logs, &mut handle.session.log_view_state);
}
```

### Acceptance Criteria

1. Link badges `[1]`, `[2]`, etc. appear before file references in link mode
2. Badges have cyan background, black text, bold styling
3. File reference text is underlined and cyan when in link mode
4. Badges only appear when `LinkHighlightState.is_active()` is true
5. Badges disappear when link mode is exited
6. Correct badges appear for both log messages and stack frames
7. Performance: no noticeable rendering slowdown
8. Horizontal scroll still works correctly with badges
9. All existing tests pass
10. No visual artifacts or rendering glitches

### Testing

#### Manual Testing Checklist

1. **Enter link mode**: Press `L`, verify badges appear
2. **Badge positions**: Verify badges appear immediately before file references
3. **Badge styling**: Verify cyan background, black text, bold
4. **Link text styling**: Verify underline + cyan on file:line text
5. **Exit link mode**: Press `Esc`, verify badges disappear
6. **Scroll**: Scroll while in link mode, verify badges update correctly
7. **Stack frames**: Verify badges appear on stack frame file locations
8. **Mixed content**: Verify entries without links don't have spurious badges
9. **Horizontal scroll**: Scroll right, verify badges scroll with content
10. **Long file paths**: Test with long paths, verify layout doesn't break

#### Visual Test Cases

```
# Test case 1: Simple error message
Input:  [ERROR] Failed at lib/main.dart:42:5
Output: [ERROR] Failed at [1]lib/main.dart:42:5
                          ^^^^^^^^^^^^^^^^^^^
                          cyan underline

# Test case 2: Stack trace
Input:  #0  build (lib/widget.dart:10:3)
Output: #0  build ([2]lib/widget.dart:10:3)

# Test case 3: No file reference
Input:  [INFO] App started
Output: [INFO] App started (unchanged)
```

### Edge Cases

1. **Multiple file refs in one message**: Only first one gets badge (per scanning logic)
2. **No links detected**: No badges rendered, link mode may not even activate
3. **File ref at start of message**: Badge appears at very start
4. **File ref at end of message**: Badge appears before file ref
5. **Package URIs**: `package:flutter/widgets.dart:10` - badge before "package:"

### Performance Considerations

- Badge insertion only happens when link mode is active
- Span splitting is O(n) where n = span content length
- Pre-allocated vectors for result spans
- No regex matching in render loop (uses pre-computed link positions)

### Files Changed

| File | Change Type |
|------|-------------|
| `src/tui/widgets/log_view.rs` | Modified - add link badge rendering |
| `src/tui/render.rs` | Modified - pass link state to LogView |

### Estimated Time

3-4 hours

---

## Completion Summary

**Status:** ✅ Done

**Date Completed:** 2026-01-05

### Files Modified

| File | Changes |
|------|---------|
| `src/tui/widgets/log_view.rs` | Added `link_highlight_state` field, builder method, badge helpers (`link_badge`, `link_text_style`, `insert_link_badge_into_spans`), modified `format_entry()` for badges, added `format_stack_frame_line_with_links()`, updated render loop |
| `src/tui/render.rs` | Added link_highlight_state to LogView builder chain |

### Implementation Details

1. **LogView field and builder** (`log_view.rs:270-340`):
   - Added `link_highlight_state: Option<&'a LinkHighlightState>` field
   - Added builder method that only stores state if `is_active()`

2. **Badge helpers** (`log_view.rs:404-472`):
   - `link_badge(shortcut)`: Creates cyan background, black text, bold badge `[1]`
   - `link_text_style()`: Returns cyan underlined style for file references
   - `insert_link_badge_into_spans()`: Splits spans to insert badge before file reference

3. **Entry formatting** (`log_view.rs:509-519`):
   - After building spans, checks for link with `frame_index == None`
   - If found, calls `insert_link_badge_into_spans()` to add badge

4. **Stack frame formatting** (`log_view.rs:656-746`):
   - New method `format_stack_frame_line_with_links(&self, frame, entry_index, frame_index)`
   - Checks for link with matching entry/frame index
   - If link found, uses link styling and inserts badge before file path
   - Updated both expanded and collapsed render loops to use new method

5. **render.rs integration** (`render.rs:37-40`):
   - Added conditional check for `link_highlight_state.is_active()`
   - Passes state to LogView via builder pattern

### Testing Performed

- `cargo check` - Passed (no warnings)
- `cargo test` - 950 tests passed

### Visual Behavior

When link mode is active, file references appear like:
```
[ERROR] Exception at [1]lib/main.dart:42:5
  #0  build ([2]lib/widget.dart:10:3)
```

Where:
- `[1]`, `[2]` badges: cyan background, black text, bold
- File paths: cyan underlined text

### Notable Decisions/Tradeoffs

1. **Separate method for frame formatting**: Created `format_stack_frame_line_with_links` instead of modifying the existing static method, because it needs `&self` to access link_highlight_state.

2. **Link styling replaces default styling**: When a link exists for a frame, the entire file:line:column uses link styling (cyan underlined), overriding the project/package differentiation. This ensures visual consistency.

3. **Badge insertion for stack frames**: Badge is inserted directly before the file path span rather than using span-splitting, which is simpler for the structured frame format.

4. **Kept original methods for tests**: Added `#[allow(dead_code)]` to `format_stack_frame` and `format_stack_frame_line` since they're used in existing unit tests.

### Risks/Limitations

- Badge rendering may affect horizontal scroll calculations (badge adds 3 chars per link)
- Very long lines with many links could become visually cluttered