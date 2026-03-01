## Task: Write Visible Height and Apply Scroll Correction in Renderer

**Objective**: Update `TargetSelector::render_full()` and `render_compact()` to (a) write the actual device list area height back to `TargetSelectorState.last_known_visible_height` each frame, and (b) compute a corrected scroll offset at render time to guarantee the selected item is always visible in the viewport.

**Depends on**: 01-add-visible-height-field

**Estimated Time**: 2-3 hours

### Scope

- `crates/fdemon-tui/src/widgets/new_session_dialog/target_selector.rs`: Modify `render_full()` and `render_compact()` to write visible height and apply scroll correction

### Details

#### Part A: Write visible height

In `render_full()` (line 65-111), after the layout split, write the device list area height:

```rust
fn render_full(&self, area: Rect, buf: &mut Buffer) {
    let chunks = Layout::vertical([
        Constraint::Length(3), // Tab bar
        Constraint::Min(5),    // Content (device list)
        Constraint::Length(1), // Footer hints
    ])
    .split(area);

    // Write actual visible height back to state for scroll calculations
    let visible_height = chunks[1].height as usize;
    self.state.last_known_visible_height.set(visible_height);

    // ... rest of render
}
```

In `render_compact()` (line 114-170), after the inner layout split:

```rust
fn render_compact(&self, area: Rect, buf: &mut Buffer) {
    // ... border setup ...
    let inner = block.inner(area);
    block.render(area, buf);

    let chunks = Layout::vertical([
        Constraint::Length(1), // Compact tab bar
        Constraint::Min(1),    // Device list
    ])
    .split(inner);

    // Write actual visible height back to state for scroll calculations
    let visible_height = chunks[1].height as usize;
    self.state.last_known_visible_height.set(visible_height);

    // ... rest of render
}
```

#### Part B: Render-time scroll correction

Before constructing the device list widgets, compute a corrected scroll offset using the actual visible height. This ensures the selected item is always visible even if the handler's `adjust_scroll()` used a stale or estimated height.

Use the existing `calculate_scroll_offset` function from `device_list.rs` (it's already `pub`):

```rust
use super::device_list::calculate_scroll_offset;
```

In `render_full()`, after writing visible height and before the device list construction:

```rust
// Render-time scroll correction: ensure selected item is visible
// even if handler used stale visible_height estimate
let corrected_scroll = calculate_scroll_offset(
    self.state.selected_index,
    visible_height,
    self.state.scroll_offset,
);
```

Then pass `corrected_scroll` instead of `self.state.scroll_offset` to the device list constructors:

```rust
// Before (line 87-92):
let list = ConnectedDeviceList::new(
    &self.state.connected_devices,
    self.state.selected_index,
    self.is_focused,
    self.state.scroll_offset,  // ← old
);

// After:
let list = ConnectedDeviceList::new(
    &self.state.connected_devices,
    self.state.selected_index,
    self.is_focused,
    corrected_scroll,  // ← corrected
);
```

Apply the same change to:
- `ConnectedDeviceList::new()` call in `render_full()` (line 87-92)
- `BootableDeviceList::new()` call in `render_full()` (line 96-103)
- `ConnectedDeviceList::new()` call in `render_compact()` (line 149-154)
- `BootableDeviceList::new()` call in `render_compact()` (line 157-165)

#### Why not persist corrected_scroll back to state?

The corrected scroll offset is only used for rendering — it's not written back to `state.scroll_offset`. This is intentional:
- `scroll_offset` is a `usize` (not `Cell<usize>`), so it can't be written through `&TargetSelectorState`
- The handler will compute a fresh scroll offset on the next key press using the now-accurate `last_known_visible_height`
- The render-time correction is a **safety net** for the case where terminal resize occurs between handler and render

### Acceptance Criteria

1. `render_full()` writes `chunks[1].height as usize` to `self.state.last_known_visible_height`
2. `render_compact()` writes `chunks[1].height as usize` to `self.state.last_known_visible_height`
3. Both methods compute a corrected scroll offset using `calculate_scroll_offset()` from `device_list.rs`
4. All four device list constructor calls use `corrected_scroll` instead of `self.state.scroll_offset`
5. Import of `calculate_scroll_offset` from `device_list` module added
6. `cargo check -p fdemon-tui` passes
7. `cargo test -p fdemon-tui` passes — all existing tests pass (corrected scroll should not change behavior when scroll is already correct)

### Testing

No new tests in this task — Task 04 covers the full feedback loop. Verify with:
- `cargo check -p fdemon-tui` — compilation
- `cargo test -p fdemon-tui` — no regressions

Existing tests like `test_target_selector_renders` use small device lists where `scroll_offset = 0` is already correct, so the correction will be a no-op.

### Notes

- The `calculate_scroll_offset` function in `device_list.rs` (line 453-474) is `pub` and has unit tests. It's the same logic as the private version in `target_selector_state.rs`.
- The correction only fires when `scroll_offset` is wrong for the actual visible height. For the common case (handler already computed correct offset), it's a no-op — the function returns `current_offset` unchanged.
- Loading and error states skip the device list render entirely, so `visible_height` is still written but `corrected_scroll` is unused. This is fine — the visible height is still accurate for when devices load.
- The `self.state` reference is `&'a TargetSelectorState`. Writing to `last_known_visible_height` (a `Cell<usize>`) is safe through a shared reference — this is the fundamental purpose of `Cell`.
