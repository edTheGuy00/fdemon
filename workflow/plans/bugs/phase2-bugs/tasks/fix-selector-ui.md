## Task: Fix Selector UI Layout

**Objective**: Refactor the project selector to use Ratatui's layout system instead of raw crossterm output, providing a visually appealing centered modal with proper styling.

**Depends on**: None (can be done independently)

### Scope

- `src/tui/selector.rs`: Complete rewrite using Ratatui widgets and layout

### Background

The current selector uses raw crossterm `Print()` commands with manual newlines:
```rust
stdout.queue(Print("\n  "))?;
stdout.queue(SetForegroundColor(Color::Cyan))?;
stdout.queue(Print("Flutter Demon".bold()))?;
```

This bypasses Ratatui's layout system, resulting in:
- Text appearing on plain new lines without centering
- No visual border or box structure
- Inconsistent styling compared to the main TUI

### Implementation Details

1. **Initialize temporary Ratatui terminal**
   ```rust
   let mut terminal = ratatui::init();
   // ... render selector ...
   ratatui::restore();
   ```

2. **Create centered modal layout**
   - Calculate modal size based on content (e.g., 60% width, fit height)
   - Use `Layout` with `Constraint::Percentage` for centering
   - Create inner area with `Block` widget and border

3. **Use List widget for projects**
   ```rust
   let items: Vec<ListItem> = projects
       .iter()
       .enumerate()
       .map(|(i, p)| {
           ListItem::new(format!("[{}] {}", i + 1, format_relative_path(p, base)))
       })
       .collect();
   
   let list = List::new(items)
       .block(Block::default().borders(Borders::ALL).title("Select Project"))
       .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
   ```

4. **Add arrow key navigation (enhancement)**
   - Track `selected_index` state
   - Handle Up/Down arrow keys to change selection
   - Handle Enter to confirm selection
   - Keep number key selection as alternative

5. **Layout structure**
   ```
   ┌─────────────────────────────────────────┐
   │           Flutter Demon                 │
   │                                         │
   │  Multiple Flutter projects found in:    │
   │  /path/to/workspace                     │
   │                                         │
   │  ┌─ Select a project ─────────────────┐ │
   │  │ [1] app_one                        │ │
   │  │ [2] app_two                        │ │
   │  │ [3] my_plugin/example              │ │
   │  └────────────────────────────────────┘ │
   │                                         │
   │  ↑/↓ Navigate  Enter Select  q Quit    │
   └─────────────────────────────────────────┘
   ```

### Acceptance Criteria

1. Selector renders as a centered modal with border
2. Project list uses Ratatui `List` widget
3. Number keys (1-9) still work for selection
4. Arrow keys navigate the list (optional enhancement)
5. Enter key confirms selection (optional enhancement)
6. 'q' and Escape still cancel
7. Terminal is properly restored after selection
8. Works correctly with varying terminal sizes

### Testing

- **Manual test**: Run `fdemon` from a directory with multiple Flutter projects
- **Visual verification**: Selector should appear centered with proper borders
- **Keyboard test**: Verify all key bindings work
- **Edge cases**:
  - Very long project paths (should truncate or wrap)
  - Small terminal size (should gracefully handle)
  - Single project (should auto-select, not show selector)

### Files to Reference

- `src/tui/layout.rs` - Existing layout patterns
- `src/tui/render.rs` - How main TUI uses widgets
- `src/tui/widgets/` - Existing widget implementations

### Estimated Effort

2-3 hours

---

## Completion Summary

**Status:** ✅ Done

**Files Modified:**
- `src/tui/selector.rs` - Complete rewrite using Ratatui widgets

**Implementation Details:**
- Replaced raw crossterm `Print()` commands with Ratatui terminal and widgets
- Created centered modal using `Layout::vertical/horizontal` with `Flex::Center`
- Used `List` widget with `ListState` for project selection
- Added `SelectorState` struct to track selection and list state
- Implemented arrow key navigation (Up/Down, j/k) in addition to number keys
- Added Enter key to confirm current selection
- Used `Block`, `Paragraph`, and styled `Span`s for consistent UI
- Added path truncation for long base paths
- Added 3 new unit tests for truncation and navigation

**UI Structure:**
```
┌─────── Flutter Demon ───────┐
│  Multiple Flutter projects  │
│  found in: /path/to/base    │
│                             │
│ ┌─ Select a project ──────┐ │
│ │> [1] app_one            │ │
│ │  [2] app_two            │ │
│ │  [3] plugin/example     │ │
│ └─────────────────────────┘ │
│                             │
│ ↑/↓ Navigate  Enter Select  │
│ 1-9 Quick select  q Quit    │
└─────────────────────────────┘
```

**Key Bindings:**
- `↑/k` - Move selection up
- `↓/j` - Move selection down
- `Enter` - Confirm current selection
- `1-9` - Quick select by number
- `q/Esc/Ctrl+C` - Cancel

**Testing Performed:**
- `cargo check` - PASS
- `cargo test` - PASS (223 tests, +3 new)
- `cargo clippy` - PASS (no warnings)

**New Tests Added:**
- `test_truncate_path_short` - verify short paths unchanged
- `test_truncate_path_long` - verify long paths truncated with `...`
- `test_selector_state_navigation` - verify up/down/index navigation