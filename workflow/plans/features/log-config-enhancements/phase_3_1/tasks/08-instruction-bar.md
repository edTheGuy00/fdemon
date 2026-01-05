## Task: 08-instruction-bar

**Objective**: Implement an instruction bar that appears at the bottom of the log view when Link Highlight Mode is active, showing the user how to select links or exit the mode.

**Depends on**: 07-link-highlight-rendering

### Background

When the user enters Link Highlight Mode, they need guidance on how to interact with the detected links. An instruction bar at the bottom of the log area provides clear, contextual help without requiring the user to memorize keyboard shortcuts.

### Scope

- `src/tui/render.rs`:
  - Add rendering of instruction bar when `UiMode::LinkHighlight`
  - Position bar at bottom of log area (similar to search input)

- `src/tui/widgets/mod.rs` (optional):
  - Create a new `LinkModeBar` widget if needed

### Visual Design

The instruction bar appears at the bottom of the log view area, overlaying the last line:

```
┌─ Logs ──────────────────────────────────────────────────────────┐
│ [ERROR] Exception at [1]lib/main.dart:42:5                      │
│   #0  MyWidget.build ([2]lib/widgets/my_widget.dart:15:10)      │
│   #1  StatelessElement.build ([3]package:flutter/src/...dart:23)│
│ [DEBUG] Loading config from [4]lib/config/app_config.dart:8     │
│                                                                 │
├─────────────────────────────────────────────────────────────────┤
│ Links: 4 │ Press 1-4 to open │ Esc to cancel │ ↑↓ scroll        │
└─────────────────────────────────────────────────────────────────┘
```

### Styling

| Element | Style |
|---------|-------|
| Bar background | Dark gray (`Color::DarkGray` or `Color::Rgb(40, 40, 40)`) |
| Link count | Cyan, bold (`Links: 4`) |
| Instructions | White/default text |
| Separators `│` | Dark gray |
| Key hints | Yellow or cyan (`1-4`, `Esc`, `↑↓`) |

### Implementation

#### Option A: Inline in render.rs (Simpler)

Add the instruction bar rendering directly in the `view()` function:

```rust
// In view() function, after log view rendering
match state.ui_mode {
    UiMode::LinkHighlight => {
        if let Some(handle) = state.session_manager.selected() {
            let link_count = handle.session.link_highlight_state.link_count();
            
            // Calculate position for instruction bar (bottom of log area, inside border)
            let bar_area = Rect::new(
                areas.logs.x + 1,
                areas.logs.y + areas.logs.height.saturating_sub(2),
                areas.logs.width.saturating_sub(2),
                1,
            );
            
            // Clear the line
            frame.render_widget(Clear, bar_area);
            
            // Build instruction text
            let max_shortcut = if link_count <= 9 {
                format!("1-{}", link_count)
            } else if link_count <= 35 {
                let last_letter = (b'a' + (link_count - 10) as u8) as char;
                format!("1-9,a-{}", last_letter)
            } else {
                "1-9,a-z".to_string()
            };
            
            let instruction = Line::from(vec![
                Span::styled("Links: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    link_count.to_string(),
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                ),
                Span::styled(" │ Press ", Style::default().fg(Color::DarkGray)),
                Span::styled(max_shortcut, Style::default().fg(Color::Yellow)),
                Span::styled(" to open │ ", Style::default().fg(Color::DarkGray)),
                Span::styled("Esc", Style::default().fg(Color::Yellow)),
                Span::styled(" cancel │ ", Style::default().fg(Color::DarkGray)),
                Span::styled("↑↓", Style::default().fg(Color::Yellow)),
                Span::styled(" scroll", Style::default().fg(Color::DarkGray)),
            ]);
            
            let bar = Paragraph::new(instruction)
                .style(Style::default().bg(Color::Rgb(30, 30, 30)));
            
            frame.render_widget(bar, bar_area);
        }
    }
    // ... other modes ...
}
```

#### Option B: Dedicated Widget (More Reusable)

Create a `LinkModeBar` widget:

```rust
// In src/tui/widgets/link_mode_bar.rs

use ratatui::{
    prelude::*,
    widgets::Widget,
};

pub struct LinkModeBar {
    link_count: usize,
}

impl LinkModeBar {
    pub fn new(link_count: usize) -> Self {
        Self { link_count }
    }
    
    fn shortcut_range(&self) -> String {
        match self.link_count {
            0 => "none".to_string(),
            1..=9 => format!("1-{}", self.link_count),
            10..=35 => {
                let last = (b'a' + (self.link_count - 10) as u8) as char;
                format!("1-9,a-{}", last)
            }
            _ => "1-9,a-z".to_string(),
        }
    }
}

impl Widget for LinkModeBar {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Clear background
        buf.set_style(area, Style::default().bg(Color::Rgb(30, 30, 30)));
        
        let spans = vec![
            Span::styled(" Links: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                self.link_count.to_string(),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
            Span::styled(self.shortcut_range(), Style::default().fg(Color::Yellow)),
            Span::styled(" to open │ ", Style::default().fg(Color::DarkGray)),
            Span::styled("Esc", Style::default().fg(Color::Yellow)),
            Span::styled(" cancel │ ", Style::default().fg(Color::DarkGray)),
            Span::styled("↑↓", Style::default().fg(Color::Yellow)),
            Span::styled(" scroll ", Style::default().fg(Color::DarkGray)),
        ];
        
        let line = Line::from(spans);
        buf.set_line(area.x, area.y, &line, area.width);
    }
}
```

### Positioning

The instruction bar should:
1. Appear at the bottom of the log area
2. Be inside the border (1 cell padding)
3. Clear the underlying log content
4. Be exactly 1 row tall

```rust
let bar_area = Rect::new(
    areas.logs.x + 1,                           // Inside left border
    areas.logs.y + areas.logs.height - 2,       // 1 above bottom border
    areas.logs.width.saturating_sub(2),         // Inside both borders
    1,                                          // Single row
);
```

### Empty State

If there are no links detected but the user somehow entered link mode, show:

```
│ No links found in viewport │ Esc to exit │
```

This shouldn't normally happen (EnterLinkMode checks for links), but it's good defensive design.

### Acceptance Criteria

1. Instruction bar appears when `UiMode::LinkHighlight`
2. Bar shows correct link count
3. Bar shows correct shortcut range based on link count
4. Bar styled with dark background
5. Key hints (shortcuts, Esc, arrows) highlighted in yellow/cyan
6. Bar disappears when exiting link mode
7. Bar positioned at bottom of log area, inside border
8. Underlying content cleared before bar renders
9. Empty state handled gracefully
10. No visual artifacts when entering/exiting link mode

### Testing

#### Manual Testing Checklist

1. **Enter link mode with 3 links**: Bar shows "Press 1-3 to open"
2. **Enter link mode with 12 links**: Bar shows "Press 1-9,a-c to open"
3. **Enter link mode with 35 links**: Bar shows "Press 1-9,a-z to open"
4. **Exit link mode**: Bar disappears
5. **Scroll in link mode**: Bar remains visible
6. **Small terminal**: Bar truncates gracefully
7. **No links**: Shows appropriate message (if reachable)

#### Visual Test Cases

```
# 5 links detected
Links: 5 │ Press 1-5 to open │ Esc cancel │ ↑↓ scroll

# 15 links detected  
Links: 15 │ Press 1-9,a-f to open │ Esc cancel │ ↑↓ scroll

# 35 links detected
Links: 35 │ Press 1-9,a-z to open │ Esc cancel │ ↑↓ scroll
```

### Integration with Existing UI

The instruction bar follows the same pattern as the search input overlay:
- Uses `Clear` widget to erase underlying content
- Positioned relative to log area bounds
- Renders on top of log content
- Single row height

### Notes

- The instruction bar is similar to the search input bar in design
- Consider extracting common styling/positioning logic if patterns emerge
- Keep text concise to fit narrow terminals
- Unicode arrows (↑↓) should work in most terminals

### Files Changed

| File | Change Type |
|------|-------------|
| `src/tui/render.rs` | Modified - add instruction bar for LinkHighlight mode |
| `src/tui/widgets/link_mode_bar.rs` | New (optional) - dedicated widget |
| `src/tui/widgets/mod.rs` | Modified (optional) - export new widget |

### Estimated Time

1-2 hours

---

## Completion Summary

**Status:** ✅ Done

**Date Completed:** 2026-01-05

### Files Modified

| File | Changes |
|------|---------|
| `src/tui/render.rs` | Added imports for styling; implemented instruction bar in `UiMode::LinkHighlight` handler |

### Implementation Details

1. **Added imports** (`render.rs:10-12`):
   - `Color`, `Modifier`, `Style` from `ratatui::style`
   - `Line`, `Span` from `ratatui::text`
   - `Paragraph` from `ratatui::widgets`

2. **Instruction bar rendering** (`render.rs:122-186`):
   - Positioned at bottom of log area inside border
   - Clears underlying content with `Clear` widget
   - Displays link count with cyan bold styling
   - Shows shortcut range based on link count:
     - 1 link: "1"
     - 2-9 links: "1-N"
     - 10-35 links: "1-9,a-X"
     - 35+ links: "1-9,a-z"
   - Key hints (shortcuts, Esc, arrows) in yellow
   - Dark gray background (RGB 30,30,30)
   - Empty state handling when no links found

### Visual Examples

```
# 5 links detected
 Links: 5 │ Press 1-5 to open │ Esc cancel │ ↑↓ scroll

# 15 links detected
 Links: 15 │ Press 1-9,a-f to open │ Esc cancel │ ↑↓ scroll

# No links (edge case)
 No links found in viewport │ Esc to exit
```

### Testing Performed

- `cargo check` - Passed
- `cargo test` - 950 tests passed

### Notable Decisions

1. **Option A chosen (inline)**: Implemented directly in render.rs rather than creating a separate widget, as the logic is simple and specific to link mode.

2. **Consistent positioning**: Used same positioning pattern as SearchInput (bottom of log area, inside border).

3. **Shortcut range calculation**: Handles single link, 2-9, 10-35, and 35+ cases correctly.