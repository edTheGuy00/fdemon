## Task: Per-Tag Filter UI

**Objective**: Add a tag filter popup/overlay that shows all discovered native log tags and lets users toggle individual tags on/off. Activated via the `T` keybinding in the log view.

**Depends on**: 07-per-tag-state, 08-per-tag-config

### Scope

- `crates/fdemon-tui/src/widgets/tag_filter.rs`: **NEW** — Tag filter overlay widget
- `crates/fdemon-app/src/handler/key.rs` (or equivalent key handler): Add `T` key binding
- `crates/fdemon-app/src/handler/update.rs`: Handle `ShowTagFilter`/`HideTagFilter` messages
- `crates/fdemon-app/src/state.rs`: Add `tag_filter_visible: bool` to app state
- `crates/fdemon-tui/src/render.rs` (or layout module): Render overlay when `tag_filter_visible`

### Details

#### 1. Tag Filter Overlay Design

The overlay appears centered over the log view when the user presses `T`. It shows:

```
┌─── Native Tag Filter ──────────────┐
│                                     │
│  [x] GoLog          (42 entries)    │
│  [x] MyPlugin       (15 entries)    │
│  [ ] OkHttp         (203 entries)   │
│  [x] com.example.mp (7 entries)     │
│                                     │
│  ─────────────────────────────────  │
│  [a] Show All  [n] Hide All        │
│  [Esc/T] Close                      │
└─────────────────────────────────────┘
```

- **Tag list**: All discovered tags sorted alphabetically with checkbox state
- **Entry count**: Number of log entries per tag (from `NativeTagState.discovered_tags`)
- **Navigation**: Arrow keys move selection, Space/Enter toggle selected tag
- **Bulk actions**: `a` to show all, `n` to hide all (none)
- **Close**: `Esc` or `T` (toggle) closes the overlay

#### 2. Tag Filter Widget

```rust
//! # Tag Filter Widget
//!
//! Overlay widget for per-tag native log filtering.
//! Shows all discovered native tags with toggle checkboxes.

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph};

/// Minimum width for the tag filter overlay.
const TAG_FILTER_MIN_WIDTH: u16 = 38;

/// Maximum height for the tag filter overlay (excluding border).
const TAG_FILTER_MAX_VISIBLE_TAGS: u16 = 15;

/// State for the tag filter overlay UI.
#[derive(Debug, Clone, Default)]
pub struct TagFilterUiState {
    /// Currently selected index in the tag list.
    pub selected_index: usize,
    /// Scroll offset for the tag list.
    pub scroll_offset: usize,
}

impl TagFilterUiState {
    pub fn move_up(&mut self) {
        self.selected_index = self.selected_index.saturating_sub(1);
    }

    pub fn move_down(&mut self, max_index: usize) {
        if self.selected_index < max_index {
            self.selected_index += 1;
        }
    }

    /// Reset selection when the overlay is opened.
    pub fn reset(&mut self) {
        self.selected_index = 0;
        self.scroll_offset = 0;
    }
}
```

#### 3. Render the tag filter overlay

```rust
pub fn render_tag_filter(
    frame: &mut Frame,
    area: Rect,
    tag_state: &NativeTagState,
    ui_state: &TagFilterUiState,
) {
    // Calculate overlay dimensions
    let tag_count = tag_state.tag_count();
    let visible_tags = tag_count.min(TAG_FILTER_MAX_VISIBLE_TAGS as usize);
    let overlay_height = (visible_tags as u16 + 5).min(area.height - 2); // +5 for borders, separator, footer
    let overlay_width = TAG_FILTER_MIN_WIDTH.max(area.width / 3).min(area.width - 4);

    // Center the overlay
    let x = (area.width.saturating_sub(overlay_width)) / 2 + area.x;
    let y = (area.height.saturating_sub(overlay_height)) / 2 + area.y;
    let overlay_area = Rect::new(x, y, overlay_width, overlay_height);

    // Clear the background
    frame.render_widget(Clear, overlay_area);

    // Render the block
    let block = Block::default()
        .title(" Native Tag Filter ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(overlay_area);
    frame.render_widget(block, overlay_area);

    if tag_count == 0 {
        let msg = Paragraph::new("No native tags discovered yet.")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(msg, inner);
        return;
    }

    // Build tag list items
    let tags: Vec<(&String, &usize)> = tag_state.sorted_tags();
    let items: Vec<ListItem> = tags
        .iter()
        .enumerate()
        .map(|(i, (tag, count))| {
            let visible = tag_state.is_tag_visible(tag);
            let checkbox = if visible { "[x]" } else { "[ ]" };
            let line = format!("{} {:<20} ({} entries)", checkbox, truncate_tag(tag, 20), count);
            let style = if i == ui_state.selected_index {
                Style::default().fg(Color::Black).bg(Color::White)
            } else if !visible {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default()
            };
            ListItem::new(line).style(style)
        })
        .collect();

    // Split inner area for list + footer
    let chunks = Layout::vertical([
        Constraint::Min(1),     // tag list
        Constraint::Length(1),  // separator
        Constraint::Length(1),  // footer
    ])
    .split(inner);

    let list = List::new(items);
    frame.render_widget(list, chunks[0]);

    // Footer with keybindings
    let footer = Paragraph::new("[a] All  [n] None  [Space] Toggle  [Esc] Close")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(footer, chunks[2]);
}

fn truncate_tag(tag: &str, max_len: usize) -> String {
    if tag.len() <= max_len {
        tag.to_string()
    } else {
        format!("{}...", &tag[..max_len - 3])
    }
}
```

#### 4. Add `T` keybinding

In the key handler (likely `handler/key.rs` or the key match in `handler/update.rs`), add:

```rust
// When in normal log view mode:
KeyCode::Char('T') | KeyCode::Char('t') => {
    if state.tag_filter_visible {
        return UpdateResult::message(Message::HideTagFilter);
    } else {
        return UpdateResult::message(Message::ShowTagFilter);
    }
}

// When tag filter overlay is visible, intercept keys:
if state.tag_filter_visible {
    match key.code {
        KeyCode::Esc | KeyCode::Char('T') | KeyCode::Char('t') => {
            return UpdateResult::message(Message::HideTagFilter);
        }
        KeyCode::Up | KeyCode::Char('k') => {
            state.tag_filter_ui.move_up();
            return UpdateResult::none();
        }
        KeyCode::Down | KeyCode::Char('j') => {
            let max = active_tag_count.saturating_sub(1);
            state.tag_filter_ui.move_down(max);
            return UpdateResult::none();
        }
        KeyCode::Char(' ') | KeyCode::Enter => {
            // Toggle selected tag
            if let Some(tag) = get_selected_tag_name(state) {
                return UpdateResult::message(Message::ToggleNativeTag { tag });
            }
        }
        KeyCode::Char('a') => {
            return UpdateResult::message(Message::ShowAllNativeTags);
        }
        KeyCode::Char('n') => {
            return UpdateResult::message(Message::HideAllNativeTags);
        }
        _ => {}
    }
    return UpdateResult::none(); // Consume all other keys while overlay is open
}
```

#### 5. State additions

```rust
// In state.rs or the appropriate state struct:
pub tag_filter_visible: bool,
pub tag_filter_ui: TagFilterUiState,
```

Handle `ShowTagFilter`:
```rust
Message::ShowTagFilter => {
    state.tag_filter_visible = true;
    state.tag_filter_ui.reset();
    UpdateResult::none()
}

Message::HideTagFilter => {
    state.tag_filter_visible = false;
    UpdateResult::none()
}
```

#### 6. Render integration

In the main render function, check if the tag filter overlay should be drawn:

```rust
// After rendering the main log view:
if state.tag_filter_visible {
    if let Some(handle) = state.session_manager.active_session() {
        render_tag_filter(frame, area, &handle.native_tag_state, &state.tag_filter_ui);
    }
}
```

### Acceptance Criteria

1. Pressing `T` in log view opens the tag filter overlay
2. Pressing `T` or `Esc` closes the overlay
3. All discovered native tags are listed alphabetically with entry counts
4. Arrow keys / `j`/`k` navigate the tag list
5. Space/Enter toggles the selected tag's visibility
6. `a` shows all tags, `n` hides all tags
7. Hidden tags display with `[ ]` checkbox and dimmed style
8. Visible tags display with `[x]` checkbox
9. The overlay is centered over the log view
10. No tags discovered shows "No native tags discovered yet." message
11. Tag names longer than 20 chars are truncated with `...`
12. All other keys are consumed while the overlay is open (no pass-through)
13. `cargo check --workspace` compiles
14. Widget rendering tests pass

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tag_filter_ui_state_default() {
        let state = TagFilterUiState::default();
        assert_eq!(state.selected_index, 0);
        assert_eq!(state.scroll_offset, 0);
    }

    #[test]
    fn test_tag_filter_ui_state_move_up() {
        let mut state = TagFilterUiState { selected_index: 3, scroll_offset: 0 };
        state.move_up();
        assert_eq!(state.selected_index, 2);
    }

    #[test]
    fn test_tag_filter_ui_state_move_up_at_zero() {
        let mut state = TagFilterUiState::default();
        state.move_up();
        assert_eq!(state.selected_index, 0); // saturating_sub
    }

    #[test]
    fn test_tag_filter_ui_state_move_down() {
        let mut state = TagFilterUiState::default();
        state.move_down(5);
        assert_eq!(state.selected_index, 1);
    }

    #[test]
    fn test_tag_filter_ui_state_move_down_at_max() {
        let mut state = TagFilterUiState { selected_index: 5, scroll_offset: 0 };
        state.move_down(5);
        assert_eq!(state.selected_index, 5); // stays at max
    }

    #[test]
    fn test_truncate_tag_short() {
        assert_eq!(truncate_tag("GoLog", 20), "GoLog");
    }

    #[test]
    fn test_truncate_tag_long() {
        assert_eq!(
            truncate_tag("com.example.very.long.subsystem.name", 20),
            "com.example.very...."
        );
    }

    #[test]
    fn test_truncate_tag_exact_length() {
        let tag = "a".repeat(20);
        assert_eq!(truncate_tag(&tag, 20), tag);
    }

    // Snapshot tests for the overlay rendering
    // (follow existing widget test patterns using ratatui's test buffer)
}
```

### Notes

- **Overlay pattern**: The tag filter is an overlay drawn on top of the existing log view, similar to how device selectors and other dialogs work in fdemon. The `Clear` widget erases the background before drawing the overlay.
- **Key interception**: When the tag filter overlay is visible, ALL keys should be intercepted by the overlay handler first. This prevents accidental scrolling, reloading, or quitting while the overlay is open.
- **Scroll handling for many tags**: If there are more tags than fit in the visible area, the tag list should scroll. The `scroll_offset` field in `TagFilterUiState` handles this (similar to existing list scroll patterns in the codebase).
- **The `T` key was chosen** because it's mnemonic for "Tag filter" and doesn't conflict with existing keybindings. Check `docs/KEYBINDINGS.md` to confirm.
- **Empty state**: When no native tags have been discovered (no native log events received yet), the overlay shows an informative message rather than an empty list.
- **Per-tag entry counts** come from `NativeTagState.discovered_tags` (BTreeMap<String, usize>) — these are updated in real-time as events arrive (task 07).
- **Future enhancement**: Could add a search/filter bar within the overlay for projects with many tags. Deferred for now — the sorted list with scroll should be sufficient for typical tag counts (5-50 tags).
