## Task: Use Actual Visible Height in Handler

**Objective**: Update `handle_device_up()` and `handle_device_down()` to read the actual visible height from `TargetSelectorState.last_known_visible_height` instead of using the hardcoded `DEFAULT_ESTIMATED_VISIBLE_HEIGHT` constant. Keep the constant as a fallback for the first frame before any render has occurred.

**Depends on**: 01-add-visible-height-field

**Estimated Time**: 1-2 hours

### Scope

- `crates/fdemon-app/src/handler/new_session/target_selector.rs`: Modify `handle_device_up()` (line 17-28) and `handle_device_down()` (line 31-38)

### Details

**Current code (line 17-28):**
```rust
pub fn handle_device_up(state: &mut AppState) -> UpdateResult {
    state
        .new_session_dialog_state
        .target_selector
        .select_previous();
    // Adjust scroll - use estimated visible height (will be refined by render)
    state
        .new_session_dialog_state
        .target_selector
        .adjust_scroll(DEFAULT_ESTIMATED_VISIBLE_HEIGHT);
    UpdateResult::none()
}
```

**New code:**
```rust
pub fn handle_device_up(state: &mut AppState) -> UpdateResult {
    state
        .new_session_dialog_state
        .target_selector
        .select_previous();
    // Use actual visible height from last render, fall back to estimate on first frame
    let visible_height = state
        .new_session_dialog_state
        .target_selector
        .last_known_visible_height
        .get();
    let height = if visible_height > 0 {
        visible_height
    } else {
        DEFAULT_ESTIMATED_VISIBLE_HEIGHT
    };
    state
        .new_session_dialog_state
        .target_selector
        .adjust_scroll(height);
    UpdateResult::none()
}
```

Apply the identical change to `handle_device_down()` (line 31-38).

**Extract helper to avoid duplication:**

Since both handlers use the same logic, extract a small helper:

```rust
/// Get the effective visible height for scroll calculations.
///
/// Returns the actual visible height from the last render frame,
/// or falls back to `DEFAULT_ESTIMATED_VISIBLE_HEIGHT` if no render
/// has occurred yet (first frame).
fn effective_visible_height(state: &AppState) -> usize {
    let height = state
        .new_session_dialog_state
        .target_selector
        .last_known_visible_height
        .get();
    if height > 0 {
        height
    } else {
        DEFAULT_ESTIMATED_VISIBLE_HEIGHT
    }
}
```

Then both handlers simplify to:
```rust
pub fn handle_device_up(state: &mut AppState) -> UpdateResult {
    state.new_session_dialog_state.target_selector.select_previous();
    state.new_session_dialog_state.target_selector
        .adjust_scroll(effective_visible_height(state));
    UpdateResult::none()
}
```

**Update the `DEFAULT_ESTIMATED_VISIBLE_HEIGHT` doc comment:**

```rust
/// Default estimated visible height for scroll calculations.
/// Used as a fallback on the first frame before the renderer has
/// written the actual visible height to `last_known_visible_height`.
const DEFAULT_ESTIMATED_VISIBLE_HEIGHT: usize = 10;
```

### Acceptance Criteria

1. `handle_device_up()` reads `last_known_visible_height.get()` and uses it when > 0
2. `handle_device_down()` reads `last_known_visible_height.get()` and uses it when > 0
3. When `last_known_visible_height` is 0 (first frame), falls back to `DEFAULT_ESTIMATED_VISIBLE_HEIGHT`
4. `DEFAULT_ESTIMATED_VISIBLE_HEIGHT` constant is kept (not removed) — it's the first-frame fallback
5. Helper function `effective_visible_height` avoids duplicating the fallback logic
6. `cargo check -p fdemon-app` passes
7. `cargo test -p fdemon-app` passes — all existing tests pass

### Testing

Existing tests in `handler/new_session/target_selector.rs` (lines 178-462) test `handle_device_select`, `handle_refresh_devices`, etc. — these don't exercise `handle_device_up/down` directly with scroll behavior. No existing tests should break because:
- On a freshly created `TargetSelectorState`, `last_known_visible_height` is `Cell::new(0)`, so the fallback to `DEFAULT_ESTIMATED_VISIBLE_HEIGHT = 10` kicks in
- This produces identical behavior to the current code

New tests in Task 04 will verify the actual height feedback path.

### Notes

- The `state` parameter in `handle_device_up/down` is `&mut AppState`. We read `last_known_visible_height.get()` through the mutable reference, which is fine — `Cell::get()` works on both `&Cell` and `&mut Cell` (through auto-deref).
- The helper function takes `&AppState` (not `&mut`) since it only reads.
- We do NOT change the scroll behavior for other handlers (e.g., `handle_connected_devices_received` which resets `scroll_offset = 0`). Those don't use `adjust_scroll` and don't need visible height.
