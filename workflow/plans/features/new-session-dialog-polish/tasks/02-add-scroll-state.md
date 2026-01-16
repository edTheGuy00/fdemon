# Task 02: Add Scroll State to TargetSelectorState

## Objective

Add `scroll_offset` field to `TargetSelectorState` and implement `adjust_scroll()` method to keep the selected item visible.

## Priority

**High** - Prerequisite for device list scrolling

## Problem

`TargetSelectorState` tracks `selected_index` but has no `scroll_offset` field. Without scroll state, the rendering code cannot know which items to display in the visible area.

## Current State Structure

**File:** `src/tui/widgets/new_session_dialog/target_selector.rs` (lines 21-50)

```rust
pub struct TargetSelectorState {
    pub active_tab: TargetTab,
    pub connected_devices: Vec<Device>,
    pub ios_simulators: Vec<IosSimulator>,
    pub android_avds: Vec<AndroidAvd>,
    pub selected_index: usize,
    pub loading: bool,
    pub bootable_loading: bool,
    pub error: Option<String>,
    cached_flat_list: Option<Vec<DeviceListItem<String>>>,
    // Missing: scroll_offset
}
```

## Solution

### Step 1: Add scroll_offset Field

**File:** `src/tui/widgets/new_session_dialog/target_selector.rs`

Add field to struct:

```rust
pub struct TargetSelectorState {
    // ... existing fields ...

    /// Scroll offset for device list (number of items scrolled past)
    pub scroll_offset: usize,
}
```

Update `Default` impl:

```rust
impl Default for TargetSelectorState {
    fn default() -> Self {
        Self {
            // ... existing defaults ...
            scroll_offset: 0,
        }
    }
}
```

### Step 2: Add adjust_scroll Method

Add method to `TargetSelectorState` impl block:

```rust
impl TargetSelectorState {
    /// Adjust scroll offset to keep selected item visible
    ///
    /// # Arguments
    /// * `visible_height` - Number of items that can be displayed
    pub fn adjust_scroll(&mut self, visible_height: usize) {
        if visible_height == 0 {
            return;
        }

        self.scroll_offset = crate::tui::widgets::new_session_dialog::device_list::calculate_scroll_offset(
            self.selected_index,
            visible_height,
            self.scroll_offset,
        );
    }

    /// Reset scroll offset (called when switching tabs or updating device list)
    pub fn reset_scroll(&mut self) {
        self.scroll_offset = 0;
    }
}
```

### Step 3: Reset Scroll on Tab Switch

Update `set_tab()` method:

```rust
pub fn set_tab(&mut self, tab: TargetTab) {
    if self.active_tab != tab {
        self.active_tab = tab;
        self.selected_index = self.first_selectable_index();
        self.scroll_offset = 0;  // Reset scroll when switching tabs
        self.invalidate_cache();
    }
}
```

### Step 4: Reset Scroll on Device List Update

Update `set_connected_devices()`:

```rust
pub fn set_connected_devices(&mut self, devices: Vec<Device>) {
    self.connected_devices = devices;
    self.loading = false;
    self.error = None;
    self.invalidate_cache();
    self.selected_index = self.first_selectable_index();
    self.scroll_offset = 0;  // Reset scroll when devices change
}
```

Update `set_bootable_devices()`:

```rust
pub fn set_bootable_devices(&mut self, simulators: Vec<IosSimulator>, avds: Vec<AndroidAvd>) {
    self.ios_simulators = simulators;
    self.android_avds = avds;
    self.bootable_loading = false;
    self.error = None;
    self.invalidate_cache();
    self.selected_index = self.first_selectable_index();
    self.scroll_offset = 0;  // Reset scroll when devices change
}
```

### Step 5: Verify calculate_scroll_offset is Public

**File:** `src/tui/widgets/new_session_dialog/device_list.rs`

Ensure the function is public (it should already be):

```rust
/// Calculate scroll offset to keep selected item visible
pub fn calculate_scroll_offset(
    selected_index: usize,
    visible_height: usize,
    current_offset: usize,
) -> usize {
    // ... existing implementation ...
}
```

## Files to Modify

| File | Changes |
|------|---------|
| `src/tui/widgets/new_session_dialog/target_selector.rs` | Add `scroll_offset` field, `adjust_scroll()`, `reset_scroll()` methods |
| `src/tui/widgets/new_session_dialog/device_list.rs` | Verify `calculate_scroll_offset` is public |

## Acceptance Criteria

1. `TargetSelectorState` has `scroll_offset: usize` field
2. `adjust_scroll(visible_height)` method exists and works correctly
3. Scroll resets to 0 when switching tabs
4. Scroll resets to 0 when device list updates
5. `cargo check` passes
6. Existing tests pass

## Testing

```bash
cargo check
cargo test target_selector
cargo test scroll
```

Add unit tests:

```rust
#[test]
fn test_scroll_offset_default() {
    let state = TargetSelectorState::default();
    assert_eq!(state.scroll_offset, 0);
}

#[test]
fn test_adjust_scroll_keeps_selection_visible() {
    let mut state = TargetSelectorState::default();
    // Add 20 devices
    state.connected_devices = (0..20).map(|i| test_device(i)).collect();
    state.selected_index = 15;
    state.scroll_offset = 0;

    state.adjust_scroll(10); // 10 visible items

    // Selection at 15 should require scroll offset of at least 6
    assert!(state.scroll_offset >= 6);
    assert!(state.scroll_offset <= 15);
}

#[test]
fn test_scroll_resets_on_tab_switch() {
    let mut state = TargetSelectorState::default();
    state.scroll_offset = 5;
    state.set_tab(TargetTab::Bootable);
    assert_eq!(state.scroll_offset, 0);
}
```

## Notes

- This task only adds the state - Task 03 will use it in rendering
- The `calculate_scroll_offset()` function already exists and is tested
- Navigation methods (`select_next`, `select_previous`) will call `adjust_scroll` in Task 03

---

## Completion Summary

**Status:** Not Started
