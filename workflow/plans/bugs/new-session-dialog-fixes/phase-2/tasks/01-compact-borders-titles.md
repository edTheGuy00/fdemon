## Task: Add Borders and Titles to Compact Mode

**Objective**: Add section titles and borders to the portrait/compact layout rendering so it matches the visual styling of horizontal layout.

**Depends on**: None

**Bug Reference**: Bug 3 - Portrait Layout Missing Section Titles and Borders

### Scope

- `src/tui/widgets/new_session_dialog/target_selector.rs`: Add border and title to `render_compact()`
- `src/tui/widgets/new_session_dialog/launch_context.rs`: Add border and title to `render_compact()`

### Details

**Current State:**

In horizontal layout, sections are wrapped in bordered blocks with titles:
```rust
// render_full() in target_selector.rs:372-378
let block = Block::default()
    .title(" Target Selector ")
    .borders(Borders::ALL)
    .border_type(BorderType::Rounded)
    .border_style(border_style);
```

In portrait/compact layout, sections render WITHOUT any borders or titles:
```rust
// render_compact() in target_selector.rs:426-467
fn render_compact(&self, area: Rect, buf: &mut Buffer) {
    let chunks = Layout::vertical([
        Constraint::Length(1), // Compact tab bar
        Constraint::Min(3),    // Device list
    ])
    .split(area);
    // No Block wrapper - content renders directly
}
```

**Implementation:**

**Step 1:** Update `TargetSelector::render_compact()` (`target_selector.rs`):

```rust
fn render_compact(&self, area: Rect, buf: &mut Buffer) {
    let border_style = if self.is_focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    // Add block with title and border
    let block = Block::default()
        .title(" Target Selector ")
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)  // Use Plain instead of Rounded to save visual weight
        .border_style(border_style);

    let inner = block.inner(area);
    block.render(area, buf);

    // Render content in inner area
    let chunks = Layout::vertical([
        Constraint::Length(1), // Compact tab bar
        Constraint::Min(1),    // Device list (reduced from 3)
    ])
    .split(inner);

    // ... rest of rendering using inner area
}
```

**Step 2:** Update `LaunchContextWithDevice::render_compact()` (`launch_context.rs`):

```rust
fn render_compact(&self, area: Rect, buf: &mut Buffer) {
    let border_style = if self.is_focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    // Add block with title and border
    let block = Block::default()
        .title(" Launch Context ")
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(border_style);

    let inner = block.inner(area);
    block.render(area, buf);

    // Render content in inner area
    let chunks = Layout::vertical([
        Constraint::Length(1), // Config field
        Constraint::Length(1), // Mode inline
        // ... etc
    ])
    .split(inner);

    // ... rest of rendering using inner area
}
```

**Key Files to Reference:**
- `src/tui/widgets/new_session_dialog/target_selector.rs:363-424` - `render_full()` for reference
- `src/tui/widgets/new_session_dialog/target_selector.rs:426-467` - `render_compact()` to modify
- `src/tui/widgets/new_session_dialog/launch_context.rs:755-769` - `render_full()` for reference
- `src/tui/widgets/new_session_dialog/launch_context.rs:771-804` - `render_compact()` to modify
- `src/tui/widgets/new_session_dialog/mod.rs:339-406` - `render_vertical()` orchestrates compact rendering

### Design Considerations

**Border Style Options:**

| Style | Pros | Cons |
|-------|------|------|
| `BorderType::Plain` | Minimal, saves visual weight | Less distinct |
| `BorderType::Rounded` | Matches horizontal layout | More prominent |
| Top border only (`Borders::TOP`) | Minimal vertical space | Less enclosed feel |

**Recommendation:** Use `BorderType::Plain` with `Borders::ALL` for consistency with horizontal layout while being less visually heavy.

**Vertical Space Impact:**
- Adding full borders costs 2 lines (top + bottom)
- Portrait layout has `MIN_VERTICAL_HEIGHT: 20`, so 2 lines is acceptable
- Consider reducing internal padding/spacing to compensate

### Acceptance Criteria

1. Portrait layout shows "Target Selector" section with visible border and title
2. Portrait layout shows "Launch Context" section with visible border and title
3. Focused section has cyan border, unfocused has dark gray border
4. Borders don't consume excessive vertical space (2 lines max per section)
5. Content remains readable and properly spaced within borders
6. No visual regression in horizontal layout
7. Tab switching between sections updates border highlight correctly

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    #[test]
    fn test_target_selector_compact_has_border() {
        let state = TargetSelectorState::default();
        let widget = TargetSelector::new(&state, true).compact(true);

        let area = Rect::new(0, 0, 50, 10);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        // Check that title is rendered
        let content = buffer_to_string(&buf);
        assert!(content.contains("Target Selector"));

        // Check for border characters (Plain style uses │ and ─)
        assert!(content.contains("│") || content.contains("─"));
    }

    #[test]
    fn test_launch_context_compact_has_border() {
        let state = LaunchContextState::default();
        let widget = LaunchContextWithDevice::new(&state, None, true).compact(true);

        let area = Rect::new(0, 0, 50, 10);
        let mut buf = Buffer::empty(area);
        widget.render(area, &mut buf);

        let content = buffer_to_string(&buf);
        assert!(content.contains("Launch Context"));
    }

    fn buffer_to_string(buf: &Buffer) -> String {
        let mut s = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                s.push(buf.get(x, y).symbol().chars().next().unwrap_or(' '));
            }
            s.push('\n');
        }
        s
    }
}
```

### Notes

- The `compact(true)` builder method already exists - we're just changing what `render_compact()` does
- Need to ensure `block.inner(area)` is used for content so it doesn't overlap borders
- Watch out for minimum height requirements - content needs enough space after borders
- Consider adding a small visual test to verify rendering looks correct

---

## Completion Summary

**Status:** Not Started

**Files Modified:**
- (to be filled after implementation)

**Implementation Details:**

(to be filled after implementation)

**Testing Performed:**
- `cargo fmt` -
- `cargo check` -
- `cargo clippy` -
- `cargo test` -

**Notable Decisions:**
- (to be filled after implementation)

**Risks/Limitations:**
- (to be filled after implementation)
