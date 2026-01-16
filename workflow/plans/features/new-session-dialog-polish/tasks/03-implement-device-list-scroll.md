# Task 03: Implement Device List Scrolling

## Objective

Modify device list rendering to use scroll offset, ensuring selected items are always visible, and add scroll indicators.

## Priority

**High** - Critical UX fix for lists with many devices

## Depends On

- Task 02: Add Scroll State

## Problem

Current rendering shows all items from index 0, clipping items that don't fit:
- User navigates to item 15 in a list of 20
- Only items 0-9 are visible (10 visible rows)
- Selected item 15 is off-screen - user can't see what they selected

## Solution

### Step 1: Update Navigation to Call adjust_scroll

**File:** `src/app/handler/new_session/target_selector.rs`

Update navigation handlers to adjust scroll after selection changes:

```rust
pub fn handle_target_selector_up(state: &mut AppState) -> UpdateResult {
    state.new_session_dialog_state.target_selector.select_previous();
    // Adjust scroll - use estimated visible height (will be refined by render)
    state.new_session_dialog_state.target_selector.adjust_scroll(10);
    UpdateResult::none()
}

pub fn handle_target_selector_down(state: &mut AppState) -> UpdateResult {
    state.new_session_dialog_state.target_selector.select_next();
    state.new_session_dialog_state.target_selector.adjust_scroll(10);
    UpdateResult::none()
}
```

### Step 2: Update ConnectedDeviceList Rendering

**File:** `src/tui/widgets/new_session_dialog/device_list.rs`

Modify `ConnectedDeviceList` to accept and use scroll offset:

```rust
pub struct ConnectedDeviceList<'a> {
    state: &'a TargetSelectorState,
    scroll_offset: usize,  // Add field
}

impl<'a> ConnectedDeviceList<'a> {
    pub fn new(state: &'a TargetSelectorState) -> Self {
        Self {
            state,
            scroll_offset: state.scroll_offset,
        }
    }
}

impl Widget for ConnectedDeviceList<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let items = self.state.get_flat_connected_list();

        // Calculate visible range
        let visible_height = area.height as usize;
        let start = self.scroll_offset.min(items.len().saturating_sub(1));
        let end = (start + visible_height).min(items.len());

        // Create list items only for visible range
        let list_items: Vec<ListItem> = items[start..end]
            .iter()
            .enumerate()
            .map(|(visible_idx, item)| {
                let actual_idx = start + visible_idx;
                self.render_item(item, actual_idx)
            })
            .collect();

        let list = List::new(list_items);
        list.render(area, buf);

        // Render scroll indicators
        self.render_scroll_indicators(area, buf, start, end, items.len());
    }

    fn render_scroll_indicators(
        &self,
        area: Rect,
        buf: &mut Buffer,
        start: usize,
        end: usize,
        total: usize,
    ) {
        // Show "↑ more" if scrolled down
        if start > 0 {
            let indicator = "↑ more";
            let x = area.right().saturating_sub(indicator.len() as u16 + 1);
            buf.set_string(x, area.top(), indicator, Style::default().fg(Color::DarkGray));
        }

        // Show "↓ more" if more items below
        if end < total {
            let indicator = "↓ more";
            let x = area.right().saturating_sub(indicator.len() as u16 + 1);
            let y = area.bottom().saturating_sub(1);
            buf.set_string(x, y, indicator, Style::default().fg(Color::DarkGray));
        }
    }
}
```

### Step 3: Update BootableDeviceList Rendering

Apply the same pattern to `BootableDeviceList`:

```rust
pub struct BootableDeviceList<'a> {
    state: &'a TargetSelectorState,
    scroll_offset: usize,
}

impl<'a> BootableDeviceList<'a> {
    pub fn new(state: &'a TargetSelectorState) -> Self {
        Self {
            state,
            scroll_offset: state.scroll_offset,
        }
    }
}

impl Widget for BootableDeviceList<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let items = self.state.get_flat_bootable_list();

        // Calculate visible range
        let visible_height = area.height as usize;
        let start = self.scroll_offset.min(items.len().saturating_sub(1));
        let end = (start + visible_height).min(items.len());

        // Create list items only for visible range
        let list_items: Vec<ListItem> = items[start..end]
            .iter()
            .enumerate()
            .map(|(visible_idx, item)| {
                let actual_idx = start + visible_idx;
                self.render_item(item, actual_idx)
            })
            .collect();

        let list = List::new(list_items);
        list.render(area, buf);

        // Render scroll indicators
        self.render_scroll_indicators(area, buf, start, end, items.len());
    }
}
```

### Step 4: Update Selection Highlight

Ensure selection highlight works with visible range:

```rust
fn render_item(&self, item: &DeviceListItem<impl AsRef<str>>, actual_index: usize) -> ListItem {
    let is_selected = actual_index == self.state.selected_index;

    let style = if is_selected {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    let prefix = if is_selected { "▶ " } else { "  " };

    // ... rest of item rendering
}
```

### Step 5: Refine adjust_scroll with Actual Height

**File:** `src/tui/widgets/new_session_dialog/target_selector.rs`

Add method to get device list area height during render:

```rust
impl TargetSelector<'_> {
    fn render_with_scroll_adjustment(&self, area: Rect, buf: &mut Buffer) {
        // Calculate actual visible height for device list
        // (area minus tabs, borders, etc.)
        let device_list_height = self.calculate_device_list_height(area);

        // This is called during render, so we need interior mutability
        // or pass height back to state update
    }
}
```

Alternative: Store `last_visible_height` in state and update during render:

```rust
// In TargetSelectorState
pub last_visible_height: usize,

// During render
self.state.last_visible_height = device_list_area.height as usize;

// In navigation handler
state.new_session_dialog_state.target_selector.adjust_scroll(
    state.new_session_dialog_state.target_selector.last_visible_height.max(5)
);
```

## Files to Modify

| File | Changes |
|------|---------|
| `src/tui/widgets/new_session_dialog/device_list.rs` | Add scroll offset to rendering, add scroll indicators |
| `src/app/handler/new_session/target_selector.rs` | Call adjust_scroll after navigation |
| `src/tui/widgets/new_session_dialog/target_selector.rs` | Track visible height during render |

## Acceptance Criteria

1. Device list shows only items in visible range
2. Navigating down past visible area scrolls the list
3. Navigating up past visible area scrolls the list
4. Selected item is always visible
5. "↑ more" indicator shows when scrolled down
6. "↓ more" indicator shows when more items below
7. Works for both Connected and Bootable tabs
8. `cargo check` passes

## Testing

```bash
cargo check
cargo test device_list
cargo test scroll
```

**Manual Testing:**
1. Connect or simulate 15+ devices
2. Open NewSessionDialog, navigate down through list
3. Verify selection stays visible as you scroll
4. Verify scroll indicators appear appropriately
5. Switch tabs, verify scroll resets

Add unit tests:

```rust
#[test]
fn test_visible_range_calculation() {
    let scroll_offset = 5;
    let visible_height = 10;
    let total_items = 20;

    let start = scroll_offset;
    let end = (start + visible_height).min(total_items);

    assert_eq!(start, 5);
    assert_eq!(end, 15);
}

#[test]
fn test_scroll_indicators_shown() {
    // Test that indicators render at correct positions
}
```

## Notes

- The `calculate_scroll_offset()` function from Task 02 handles the math
- Scroll indicators use Unicode arrows (↑↓) - works in most terminals
- Consider adding scrollbar visualization in future enhancement

---

## Completion Summary

**Status:** Not Started
