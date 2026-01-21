# Task 06: Implement Vertical Layout

## Objective

Implement the vertical (stacked) layout for narrow terminals, with Target Selector above Launch Context.

## Priority

**Medium** - Enables dialog usage in narrow terminals

## Depends On

- Task 05: Add Layout Mode Detection

## Problem

Users in narrow terminals (split panes, IDE embedded terminals, mobile SSH) cannot use the dialog at all. Need a stacked layout that works in 40-69 column terminals.

## Target Layout

```
┌─────────────────────────────────┐
│  ┌── 🎯 Target Selector ─────┐  │
│  │                           │  │
│  │  ╭───────────╮ ╭────────╮ │  │
│  │  │ Connected │ │Bootable│ │  │
│  │  ╰───────────╯ ╰────────╯ │  │
│  │                           │  │
│  │  iOS Devices              │  │
│  │  ▶ iPhone 15 Pro          │  │
│  │    Pixel 8                │  │
│  │                           │  │
│  │  [↑↓] Navigate [Enter]    │  │
│  └───────────────────────────┘  │
│                                 │
│  ┌── ⚙️ Launch Context ──────┐  │
│  │                           │  │
│  │  Config: [Development  ▼] │  │
│  │  Mode:   (●) Debug        │  │
│  │  Flavor: [dev_________▼]  │  │
│  │                           │  │
│  │  [     🚀 LAUNCH     ]    │  │
│  └───────────────────────────┘  │
│                                 │
│  [Tab] Switch  [Esc] Close      │
└─────────────────────────────────┘
```

## Solution

### Step 1: Implement render_vertical Method

**File:** `src/tui/widgets/new_session_dialog/mod.rs`

```rust
impl NewSessionDialog<'_> {
    /// Render vertical (stacked) layout for narrow terminals
    fn render_vertical(&self, area: Rect, buf: &mut Buffer) {
        use ratatui::layout::{Constraint, Direction, Layout};

        // Clear background
        Clear.render(area, buf);

        // Use more of the available space in vertical mode (90% width, 85% height)
        let dialog_area = centered_rect(90, 85, area);

        // Draw outer border
        let outer_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title(" Launch Session ")
            .title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
        let inner_area = outer_block.inner(dialog_area);
        outer_block.render(dialog_area, buf);

        // Vertical split: Target Selector (60%) | Launch Context (40%)
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(55),  // Target Selector
                Constraint::Length(1),       // Separator
                Constraint::Min(10),         // Launch Context
                Constraint::Length(1),       // Footer
            ])
            .split(inner_area);

        // Render Target Selector (top)
        self.render_target_selector_compact(chunks[0], buf);

        // Render separator line
        let separator = "─".repeat(chunks[1].width as usize);
        buf.set_string(
            chunks[1].x,
            chunks[1].y,
            &separator,
            Style::default().fg(Color::DarkGray),
        );

        // Render Launch Context (bottom)
        self.render_launch_context_compact(chunks[2], buf);

        // Render footer
        self.render_footer_compact(chunks[3], buf);
    }
}
```

### Step 2: Add Compact Target Selector Rendering

```rust
impl NewSessionDialog<'_> {
    /// Render target selector in compact mode (for vertical layout)
    fn render_target_selector_compact(&self, area: Rect, buf: &mut Buffer) {
        let target_selector = TargetSelector::new(&self.state.target_selector)
            .compact(true);  // Enable compact mode
        target_selector.render(area, buf);
    }
}
```

### Step 3: Add Compact Launch Context Rendering

```rust
impl NewSessionDialog<'_> {
    /// Render launch context in compact mode (for vertical layout)
    fn render_launch_context_compact(&self, area: Rect, buf: &mut Buffer) {
        let launch_context = LaunchContext::new(
            &self.state.launch_context,
            &self.state.target_selector,
            self.state.focused_pane == DialogPane::LaunchContext,
        ).compact(true);  // Enable compact mode
        launch_context.render(area, buf);
    }
}
```

### Step 4: Add Compact Footer

```rust
impl NewSessionDialog<'_> {
    /// Render footer with abbreviated keybindings (for vertical layout)
    fn render_footer_compact(&self, area: Rect, buf: &mut Buffer) {
        // Shorter keybinding hints for narrow terminals
        let hints = "[Tab]Pane [↑↓]Nav [Enter]Select [Esc]Close";

        let paragraph = Paragraph::new(hints)
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        paragraph.render(area, buf);
    }
}
```

### Step 5: Update TargetSelector for Compact Mode

**File:** `src/tui/widgets/new_session_dialog/target_selector.rs`

Add compact mode support:

```rust
pub struct TargetSelector<'a> {
    state: &'a TargetSelectorState,
    focused: bool,
    compact: bool,  // Add field
}

impl<'a> TargetSelector<'a> {
    pub fn new(state: &'a TargetSelectorState) -> Self {
        Self {
            state,
            focused: true,
            compact: false,
        }
    }

    pub fn compact(mut self, compact: bool) -> Self {
        self.compact = compact;
        self
    }
}

impl Widget for TargetSelector<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if self.compact {
            self.render_compact(area, buf);
        } else {
            self.render_full(area, buf);
        }
    }
}

impl TargetSelector<'_> {
    fn render_compact(&self, area: Rect, buf: &mut Buffer) {
        // Smaller tabs, tighter spacing
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),  // Compact tab bar
                Constraint::Min(3),     // Device list
            ])
            .split(area);

        // Render compact tab bar (shorter labels)
        self.render_tabs_compact(chunks[0], buf);

        // Render device list (same as full mode)
        self.render_device_list(chunks[1], buf);
    }

    fn render_tabs_compact(&self, area: Rect, buf: &mut Buffer) {
        let tabs = vec![
            Span::raw(if self.state.active_tab == TargetTab::Connected { "[1]Con" } else { " 1 Con" }),
            Span::raw(" "),
            Span::raw(if self.state.active_tab == TargetTab::Bootable { "[2]Boot" } else { " 2 Boot" }),
        ];

        let paragraph = Paragraph::new(Line::from(tabs));
        paragraph.render(area, buf);
    }
}
```

### Step 6: Update LaunchContext for Compact Mode

**File:** `src/tui/widgets/new_session_dialog/launch_context.rs`

Add compact mode support:

```rust
pub struct LaunchContext<'a> {
    // ... existing fields ...
    compact: bool,
}

impl<'a> LaunchContext<'a> {
    pub fn compact(mut self, compact: bool) -> Self {
        self.compact = compact;
        self
    }
}

impl Widget for LaunchContext<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if self.compact {
            self.render_compact(area, buf);
        } else {
            self.render_full(area, buf);
        }
    }
}

impl LaunchContext<'_> {
    fn render_compact(&self, area: Rect, buf: &mut Buffer) {
        // Tighter layout for narrow terminals
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),  // Config
                Constraint::Length(1),  // Mode (inline)
                Constraint::Length(1),  // Flavor
                Constraint::Length(1),  // Spacing
                Constraint::Length(1),  // Launch button
            ])
            .split(area);

        // Single-line fields with abbreviated labels
        self.render_config_compact(chunks[0], buf);
        self.render_mode_inline(chunks[1], buf);
        self.render_flavor_compact(chunks[2], buf);
        self.render_launch_button(chunks[4], buf);
    }

    fn render_mode_inline(&self, area: Rect, buf: &mut Buffer) {
        // Mode as inline radio: "Mode: (●)Dbg (○)Prof (○)Rel"
        let mode_str = format!(
            "Mode: {} {} {}",
            if self.state.mode == FlutterMode::Debug { "(●)Dbg" } else { "(○)Dbg" },
            if self.state.mode == FlutterMode::Profile { "(●)Prof" } else { "(○)Prof" },
            if self.state.mode == FlutterMode::Release { "(●)Rel" } else { "(○)Rel" },
        );
        buf.set_string(area.x, area.y, &mode_str, Style::default());
    }
}
```

## Files to Modify

| File | Changes |
|------|---------|
| `src/tui/widgets/new_session_dialog/mod.rs` | Add `render_vertical`, compact rendering methods |
| `src/tui/widgets/new_session_dialog/target_selector.rs` | Add `compact()` builder, compact rendering |
| `src/tui/widgets/new_session_dialog/launch_context.rs` | Add `compact()` builder, compact rendering |

## Acceptance Criteria

1. Vertical layout renders at 50x30 terminal
2. Target Selector appears above Launch Context
3. All functionality works (tab switching, navigation, launch)
4. Compact mode uses abbreviated labels
5. Layout is readable and usable
6. `cargo check` passes

## Testing

```bash
cargo check
cargo test vertical
cargo test compact
```

**Manual Testing:**
1. Resize terminal to 50x30
2. Open NewSessionDialog → Should show vertical layout
3. Navigate devices, switch tabs, launch → All should work
4. Resize to 100x40 → Should switch to horizontal layout

## Notes

- Vertical layout uses 90% width vs 80% for horizontal (more space efficiency)
- Tab labels abbreviated: "Connected" → "Con", "Bootable" → "Boot"
- Mode selector inline instead of vertical
- Footer keybindings abbreviated

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `src/tui/widgets/new_session_dialog/mod.rs` | Added `render_vertical()` method with stacked layout, `centered_rect_custom()` helper, and `render_footer_compact()` for abbreviated keybindings |
| `src/tui/widgets/new_session_dialog/target_selector.rs` | Added `compact` field and `compact()` builder method, split rendering into `render_full()` and `render_compact()`, added `render_tabs_compact()` for single-line tab bar |
| `src/tui/widgets/new_session_dialog/launch_context.rs` | Added `compact` field and `compact()` builder method to `LaunchContextWithDevice`, split rendering into `render_full()` and `render_compact()`, added `render_mode_inline()` for inline mode selector with abbreviated labels |

### Notable Decisions/Tradeoffs

1. **Vertical layout uses 90% width vs 80% for horizontal**: More space efficiency in narrow terminals
2. **Compact mode removes borders from sub-widgets**: Target Selector and Launch Context render without individual borders in vertical mode to save vertical space
3. **Tab bar abbreviated to single line**: "Connected" and "Bootable" rendered inline with number indicators instead of boxed tabs
4. **Mode selector inline with abbreviated labels**: "(●)Dbg (○)Prof (○)Rel" instead of vertical radio buttons to save vertical space
5. **Footer keybindings abbreviated**: Shorter hints to fit narrow terminal width

### Testing Performed

- `cargo fmt` - Passed
- `cargo check` - Passed
- `cargo test --lib` - Passed (1402 tests)
- `cargo clippy -- -D warnings` - Passed

### Risks/Limitations

1. **Manual testing required**: Vertical layout should be tested manually at 50x30 terminal size to verify visual appearance and usability
2. **Layout transition**: Users resizing terminals will experience layout switches between horizontal and vertical modes at 70-column boundary
