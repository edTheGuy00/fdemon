# Task 07: Adapt Widgets for Responsive Layout

## Objective

Refine child widgets (TargetSelector, LaunchContext, modals) to work well in both horizontal and vertical layouts.

## Priority

**Medium** - Polish and refinement for responsive behavior

## Depends On

- Task 06: Implement Vertical Layout

## Problem

After implementing vertical layout, some edge cases may not render well:
- Device list might be too short in vertical mode
- Fuzzy modal might not fit in narrow terminals
- Dart defines modal needs adaptation
- Text truncation for long device names

## Solution

### Step 1: Add Text Truncation Utilities

**File:** `src/tui/widgets/new_session_dialog/mod.rs` or new `utils.rs`

```rust
/// Truncate text to fit within max_width, adding ellipsis if needed
pub fn truncate_with_ellipsis(text: &str, max_width: usize) -> String {
    if text.len() <= max_width {
        text.to_string()
    } else if max_width <= 3 {
        ".".repeat(max_width)
    } else {
        format!("{}...", &text[..max_width - 3])
    }
}

/// Truncate text from the middle, preserving start and end
pub fn truncate_middle(text: &str, max_width: usize) -> String {
    if text.len() <= max_width {
        text.to_string()
    } else if max_width <= 5 {
        truncate_with_ellipsis(text, max_width)
    } else {
        let half = (max_width - 3) / 2;
        let start = &text[..half];
        let end = &text[text.len() - half..];
        format!("{}...{}", start, end)
    }
}
```

### Step 2: Update Device List Rendering with Truncation

**File:** `src/tui/widgets/new_session_dialog/device_list.rs`

```rust
impl ConnectedDeviceList<'_> {
    fn render_item(&self, item: &DeviceListItem<impl AsRef<str>>, index: usize) -> ListItem {
        match item {
            DeviceListItem::Device(device) => {
                // Truncate device name if needed
                let available_width = self.available_width.saturating_sub(4); // prefix + padding
                let name = truncate_with_ellipsis(&device.name, available_width);

                let is_selected = index == self.state.selected_index;
                let prefix = if is_selected { "▶ " } else { "  " };

                // ... rest of rendering
            }
            // ... other variants
        }
    }
}
```

### Step 3: Adapt Fuzzy Modal for Narrow Terminals

**File:** `src/tui/widgets/new_session_dialog/fuzzy_modal.rs`

```rust
impl FuzzyModal<'_> {
    fn calculate_modal_area(&self, parent_area: Rect) -> Rect {
        // In narrow terminals, use more of the width
        let width_percent = if parent_area.width < 60 { 95 } else { 80 };
        let height_percent = if parent_area.height < 30 { 70 } else { 50 };

        centered_rect(width_percent, height_percent, parent_area)
    }

    fn render_item(&self, item: &str, index: usize, area_width: u16) -> ListItem {
        // Truncate items to fit
        let max_width = area_width.saturating_sub(4) as usize;
        let display_text = truncate_with_ellipsis(item, max_width);

        // ... render with truncated text
    }
}
```

### Step 4: Adapt Dart Defines Modal

**File:** `src/tui/widgets/new_session_dialog/dart_defines_modal.rs`

```rust
impl DartDefinesModal<'_> {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        // In narrow terminals, use vertical layout for the modal too
        if area.width < 60 {
            self.render_vertical(area, buf);
        } else {
            self.render_horizontal(area, buf);
        }
    }

    fn render_vertical(&self, area: Rect, buf: &mut Buffer) {
        // Stack the list and edit form vertically
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(40),  // Define list
                Constraint::Min(6),          // Edit form
            ])
            .split(area);

        self.render_define_list(chunks[0], buf);
        self.render_edit_form(chunks[1], buf);
    }
}
```

### Step 5: Add Dynamic Constraints Based on Available Space

**File:** `src/tui/widgets/new_session_dialog/target_selector.rs`

```rust
impl TargetSelector<'_> {
    fn calculate_device_list_height(&self, total_height: u16) -> u16 {
        // Reserve space for tabs and borders
        let reserved = 3; // 1 tab bar + 2 borders
        total_height.saturating_sub(reserved)
    }

    fn should_show_loading_indicator(&self, area: Rect) -> bool {
        // Only show spinner if we have enough space
        area.height >= 5
    }

    fn should_show_platform_headers(&self, area: Rect) -> bool {
        // Show platform group headers only if we have enough height
        let item_count = self.state.current_device_count();
        let needs_headers = item_count > 3;
        let has_space = area.height > (item_count as u16 + 4);
        needs_headers && has_space
    }
}
```

### Step 6: Handle Minimum Content Requirements

```rust
impl NewSessionDialog<'_> {
    fn render_vertical(&self, area: Rect, buf: &mut Buffer) {
        // ... existing code ...

        // Ensure minimum heights for each section
        let target_min_height = 8;
        let context_min_height = 6;

        let available = area.height.saturating_sub(4); // borders, footer

        let (target_height, context_height) = if available < target_min_height + context_min_height {
            // Very tight - prioritize target selector
            (available.saturating_sub(context_min_height).max(4), context_min_height.min(available))
        } else {
            // Normal distribution
            let target = (available * 55) / 100;
            let context = available - target;
            (target, context)
        };

        // Use calculated heights in layout
    }
}
```

### Step 7: Add Responsive Scroll Indicators

**File:** `src/tui/widgets/new_session_dialog/device_list.rs`

```rust
fn render_scroll_indicators(&self, area: Rect, buf: &mut Buffer, start: usize, end: usize, total: usize) {
    // Use shorter indicators in narrow terminals
    let (up_indicator, down_indicator) = if area.width < 50 {
        ("↑", "↓")
    } else {
        ("↑ more", "↓ more")
    };

    if start > 0 {
        let x = area.right().saturating_sub(up_indicator.len() as u16 + 1);
        buf.set_string(x, area.top(), up_indicator, Style::default().fg(Color::DarkGray));
    }

    if end < total {
        let x = area.right().saturating_sub(down_indicator.len() as u16 + 1);
        let y = area.bottom().saturating_sub(1);
        buf.set_string(x, y, down_indicator, Style::default().fg(Color::DarkGray));
    }
}
```

## Files to Modify

| File | Changes |
|------|---------|
| `src/tui/widgets/new_session_dialog/mod.rs` | Add truncation utilities, refine vertical layout |
| `src/tui/widgets/new_session_dialog/device_list.rs` | Truncation, responsive scroll indicators |
| `src/tui/widgets/new_session_dialog/target_selector.rs` | Dynamic height calculations |
| `src/tui/widgets/new_session_dialog/launch_context.rs` | Adaptive layout for narrow widths |
| `src/tui/widgets/new_session_dialog/fuzzy_modal.rs` | Adaptive modal sizing |
| `src/tui/widgets/new_session_dialog/dart_defines_modal.rs` | Vertical layout option |

## Acceptance Criteria

1. Long device names truncate gracefully with ellipsis
2. Fuzzy modal adapts to narrow terminals
3. Dart defines modal usable in vertical layout
4. Scroll indicators adapt to available width
5. No text overflow or rendering glitches
6. All functionality preserved at all supported sizes
7. `cargo check` passes

## Testing

```bash
cargo check
cargo test truncate
cargo test responsive
```

**Manual Testing:**
1. Test with device names > 40 characters
2. Test fuzzy modal at various widths (40, 60, 100)
3. Test dart defines modal in narrow terminal
4. Verify no visual glitches at boundary sizes (40x20, 70x20)

## Notes

- Prioritize functionality over aesthetics in very narrow terminals
- Truncation should preserve enough context to identify items
- Modal overlays may need adjusted positioning in vertical mode

---

## Completion Summary

**Status:** Not Started
