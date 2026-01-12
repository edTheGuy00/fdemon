## Task: Implement Dart Defines List Widget

**Objective**: Create the left pane widget showing the list of dart defines with "[+] Add New" option.

**Depends on**: Task 01 (Dart Defines State)

**Estimated Time**: 25 minutes

### Background

The left pane shows all defined variables in a scrollable list, with a special "[+] Add New" option at the bottom. The selected item is highlighted, and the pane border indicates focus state.

### Scope

- `src/tui/widgets/new_session_dialog/dart_defines_modal.rs`: Create file, add list widget

### Changes Required

**Create `src/tui/widgets/new_session_dialog/dart_defines_modal.rs`:**

```rust
//! Dart defines master-detail modal widget

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Widget},
};

use super::state::{DartDefinesModalState, DartDefinesPane};

/// Style constants for dart defines modal
mod styles {
    use super::*;

    pub const MODAL_BG: Color = Color::Rgb(30, 30, 40);
    pub const BORDER_FOCUSED: Color = Color::Cyan;
    pub const BORDER_UNFOCUSED: Color = Color::DarkGray;
    pub const HEADER_FG: Color = Color::Cyan;
    pub const ITEM_FG: Color = Color::White;
    pub const SELECTED_FG: Color = Color::Black;
    pub const SELECTED_BG: Color = Color::Cyan;
    pub const ADD_NEW_FG: Color = Color::Green;
    pub const HINT_FG: Color = Color::DarkGray;
}

/// Widget for the left pane (list of defines)
pub struct DartDefinesListPane<'a> {
    state: &'a DartDefinesModalState,
}

impl<'a> DartDefinesListPane<'a> {
    pub fn new(state: &'a DartDefinesModalState) -> Self {
        Self { state }
    }

    fn is_focused(&self) -> bool {
        self.state.active_pane == DartDefinesPane::List
    }
}

impl Widget for DartDefinesListPane<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let border_color = if self.is_focused() {
            styles::BORDER_FOCUSED
        } else {
            styles::BORDER_UNFOCUSED
        };

        let block = Block::default()
            .title(" Active Variables ")
            .title_style(Style::default().fg(styles::HEADER_FG).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_set(symbols::border::ROUNDED)
            .border_style(Style::default().fg(border_color))
            .style(Style::default().bg(styles::MODAL_BG));

        let inner = block.inner(area);
        block.render(area, buf);

        // Calculate visible range
        let visible_height = inner.height as usize;
        let total_items = self.state.list_item_count();
        let start = self.state.scroll_offset;
        let end = (start + visible_height).min(total_items);

        // Build list items
        let items: Vec<ListItem> = (start..end)
            .map(|idx| {
                let is_selected = idx == self.state.selected_index;
                let is_add_new = idx >= self.state.defines.len();

                let (text, base_style) = if is_add_new {
                    (
                        "[+] Add New".to_string(),
                        Style::default().fg(styles::ADD_NEW_FG),
                    )
                } else {
                    let define = &self.state.defines[idx];
                    (
                        define.key.clone(),
                        Style::default().fg(styles::ITEM_FG),
                    )
                };

                let indicator = if is_selected { "> " } else { "  " };
                let display_text = format!("{}{}", indicator, text);

                let style = if is_selected {
                    Style::default()
                        .fg(styles::SELECTED_FG)
                        .bg(styles::SELECTED_BG)
                        .add_modifier(Modifier::BOLD)
                } else {
                    base_style
                };

                ListItem::new(display_text).style(style)
            })
            .collect();

        let list = List::new(items);
        list.render(inner, buf);

        // Show scroll indicator if needed
        if total_items > visible_height {
            let scroll_info = format!(" {}/{} ", self.state.selected_index + 1, total_items);
            let info_width = scroll_info.len() as u16;

            if inner.width > info_width + 2 {
                let x = inner.x + inner.width - info_width;
                let y = area.y;  // Top border

                buf.set_string(
                    x,
                    y,
                    &scroll_info,
                    Style::default().fg(styles::HINT_FG),
                );
            }
        }
    }
}

#[cfg(test)]
mod list_tests {
    use super::*;
    use crate::tui::widgets::new_session_dialog::state::DartDefine;

    fn create_test_buffer(width: u16, height: u16) -> (Buffer, Rect) {
        let rect = Rect::new(0, 0, width, height);
        let buf = Buffer::empty(rect);
        (buf, rect)
    }

    fn buffer_contains(buf: &Buffer, text: &str) -> bool {
        let content: String = buf.content.iter().map(|c| c.symbol()).collect();
        content.contains(text)
    }

    #[test]
    fn test_list_renders_defines() {
        let defines = vec![
            DartDefine::new("API_KEY", "secret"),
            DartDefine::new("DEBUG", "true"),
        ];
        let state = DartDefinesModalState::new(defines);

        let (mut buf, rect) = create_test_buffer(40, 10);
        let widget = DartDefinesListPane::new(&state);
        widget.render(rect, &mut buf);

        assert!(buffer_contains(&buf, "API_KEY"));
        assert!(buffer_contains(&buf, "DEBUG"));
        assert!(buffer_contains(&buf, "Add New"));
    }

    #[test]
    fn test_list_shows_selection() {
        let defines = vec![DartDefine::new("KEY", "value")];
        let state = DartDefinesModalState::new(defines);

        let (mut buf, rect) = create_test_buffer(40, 10);
        let widget = DartDefinesListPane::new(&state);
        widget.render(rect, &mut buf);

        // First item should have selection indicator
        assert!(buffer_contains(&buf, "> KEY"));
    }

    #[test]
    fn test_empty_list_shows_add_new() {
        let state = DartDefinesModalState::new(vec![]);

        let (mut buf, rect) = create_test_buffer(40, 10);
        let widget = DartDefinesListPane::new(&state);
        widget.render(rect, &mut buf);

        assert!(buffer_contains(&buf, "Add New"));
    }
}
```

**Update mod.rs:**

```rust
mod dart_defines_modal;
pub use dart_defines_modal::*;
```

### Acceptance Criteria

1. `DartDefinesListPane` widget struct with `new(state)` constructor
2. Renders all define keys in a scrollable list
3. "[+] Add New" option rendered at bottom in green
4. Current selection highlighted with `> ` indicator and cyan background
5. Border color indicates focus state (cyan when focused, gray when not)
6. Scroll indicator shows position when list exceeds visible height
7. Block title shows "Active Variables"
8. `cargo check` passes
9. `cargo clippy -- -D warnings` passes

### Testing

Widget rendering tests using buffer inspection:

```rust
#[test]
fn test_list_focused_border() {
    let mut state = DartDefinesModalState::new(vec![]);
    state.active_pane = DartDefinesPane::List;

    let (mut buf, rect) = create_test_buffer(40, 10);
    let widget = DartDefinesListPane::new(&state);
    widget.render(rect, &mut buf);

    // Check that focused border style is applied
    // (Implementation detail: check border cells have cyan color)
}

#[test]
fn test_list_scroll_indicator() {
    let defines: Vec<DartDefine> = (0..20)
        .map(|i| DartDefine::new(format!("KEY_{}", i), "value"))
        .collect();
    let state = DartDefinesModalState::new(defines);

    let (mut buf, rect) = create_test_buffer(40, 10);
    let widget = DartDefinesListPane::new(&state);
    widget.render(rect, &mut buf);

    // Should show scroll position
    assert!(buffer_contains(&buf, "1/21"));
}
```

### Notes

- List pane width should be about 40% of modal width
- Selection indicator uses `>` character
- Empty state shows only "[+] Add New"
- Consider truncating long keys with ellipsis

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/tui/widgets/new_session_dialog/dart_defines_modal.rs` | Created new file with `DartDefinesListPane` widget, style constants, and comprehensive tests |
| `src/tui/widgets/new_session_dialog/mod.rs` | Added `dart_defines_modal` module and public exports |

### Notable Decisions/Tradeoffs

1. **Style Constants Module**: Created a nested `styles` module to encapsulate all color constants for better organization and maintainability
2. **Focus State Detection**: Implemented `is_focused()` helper method to check active pane and apply appropriate border colors
3. **Scroll Indicator Positioning**: Placed scroll indicator on top border (right side) to avoid conflicts with list content
4. **Test Coverage**: Implemented 8 widget rendering tests covering all acceptance criteria including focused/unfocused states, selection, scrolling, and empty state

### Testing Performed

- `cargo fmt` - Passed (code formatted)
- `cargo check` - Passed (no compilation errors)
- `cargo test --lib` - Passed (1405 tests passed, including 8 new tests for DartDefinesListPane)
- `cargo clippy -- -D warnings` - Passed (no warnings)
- `cargo test dart_defines_modal --lib` - Passed (18 tests total including state tests)

### Risks/Limitations

1. **Long Key Names**: Current implementation does not truncate long define keys - they may overflow the visible area. Future enhancement could add ellipsis truncation
2. **Fixed Visible Height**: The scroll adjustment uses a hardcoded VISIBLE_ITEMS constant in state.rs. The widget should dynamically adjust based on actual rendered height, but relies on state's scroll logic
3. **Buffer Cell Access**: Tests use direct buffer cell access which assumes border cells exist at specific coordinates - may be fragile if border rendering changes
