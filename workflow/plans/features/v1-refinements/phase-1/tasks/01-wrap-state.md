## Task: Add Wrap Mode State, Message, Keybinding, and Scroll Guards

**Objective**: Add the `wrap_mode` boolean to `LogViewState`, define the `ToggleWrapMode` message, wire the `w` keybinding, implement the handler, and guard horizontal scroll functions so they become no-ops when wrap is enabled.

**Depends on**: None

### Scope

- `crates/fdemon-app/src/log_view_state.rs`: Add `wrap_mode: bool` field and `toggle_wrap_mode()` method
- `crates/fdemon-app/src/message.rs`: Add `ToggleWrapMode` variant
- `crates/fdemon-app/src/handler/keys.rs`: Add `w` key binding in `handle_key_normal()`
- `crates/fdemon-app/src/handler/update.rs`: Add `ToggleWrapMode` match arm
- `crates/fdemon-app/src/handler/scroll.rs`: Add wrap mode guards to all 4 horizontal scroll handlers

### Details

#### 1. Add `wrap_mode` to `LogViewState`

**File:** `crates/fdemon-app/src/log_view_state.rs`

Add `wrap_mode: bool` field to the `LogViewState` struct (lines 44-64). Default to `true` (wrap on by default).

```rust
pub struct LogViewState {
    pub offset: usize,
    pub h_offset: usize,
    pub auto_scroll: bool,
    pub total_lines: usize,
    pub visible_lines: usize,
    pub max_line_width: usize,
    pub visible_width: usize,
    pub buffer_lines: usize,
    pub focus_info: FocusInfo,
    pub wrap_mode: bool,         // NEW — default true
}
```

In `new()` (lines 66-85), set `wrap_mode: true`.

Add a `toggle_wrap_mode()` method:

```rust
/// Toggle line wrap mode. When enabling wrap, resets horizontal offset to 0.
pub fn toggle_wrap_mode(&mut self) {
    self.wrap_mode = !self.wrap_mode;
    if self.wrap_mode {
        self.h_offset = 0;
    }
}
```

#### 2. Add `ToggleWrapMode` message

**File:** `crates/fdemon-app/src/message.rs`

Add `ToggleWrapMode` variant near the horizontal scroll messages (around lines 256-265). Follow the `ToggleStackTrace` pattern (line 253):

```rust
/// Toggle line wrap mode on/off
ToggleWrapMode,
```

#### 3. Add `w` key binding

**File:** `crates/fdemon-app/src/handler/keys.rs`

In `handle_key_normal()` (lines 100-264), add a new arm near the horizontal scroll bindings (after line 248):

```rust
InputKey::Char('w') => Some(Message::ToggleWrapMode),
```

No session guard needed — wrap mode applies to the current session's `LogViewState`, and if no session exists, the handler will safely no-op via the `selected_mut()` guard.

#### 4. Add handler in update.rs

**File:** `crates/fdemon-app/src/handler/update.rs`

Add a match arm after `ToggleStackTrace` (around line 594). Follow the same pattern:

```rust
Message::ToggleWrapMode => {
    if let Some(handle) = state.session_manager.selected_mut() {
        handle.session.log_view_state.toggle_wrap_mode();
    }
    UpdateResult::none()
}
```

#### 5. Guard horizontal scroll handlers

**File:** `crates/fdemon-app/src/handler/scroll.rs`

All 4 horizontal scroll handlers (lines 64-93) need a wrap mode guard. Extract the `wrap_mode` flag before the mutable borrow to avoid double-borrow issues:

```rust
pub fn handle_scroll_left(state: &mut AppState, n: usize) -> UpdateResult {
    if let Some(handle) = state.session_manager.selected_mut() {
        if !handle.session.log_view_state.wrap_mode {
            handle.session.log_view_state.scroll_left(n);
        }
    }
    UpdateResult::none()
}
```

Apply the same guard to:
- `handle_scroll_right` (line 72)
- `handle_scroll_to_line_start` (line 80)
- `handle_scroll_to_line_end` (line 88)

### Acceptance Criteria

1. `LogViewState::new()` creates state with `wrap_mode: true`
2. `toggle_wrap_mode()` flips `wrap_mode` and resets `h_offset` to 0 when enabling wrap
3. Pressing `w` in normal mode dispatches `Message::ToggleWrapMode`
4. The `ToggleWrapMode` handler calls `toggle_wrap_mode()` on the active session
5. `ScrollLeft`, `ScrollRight`, `ScrollToLineStart`, `ScrollToLineEnd` are no-ops when `wrap_mode` is `true`
6. `cargo check -p fdemon-app` passes
7. `cargo clippy -p fdemon-app -- -D warnings` passes
8. All existing `fdemon-app` tests pass (`cargo test -p fdemon-app`)

### Notes

- `wrap_mode` is per-session (on `LogViewState`) rather than global (on `UiSettings`). This allows different sessions to have independent wrap preferences. Each session starts with wrap enabled.
- The `h_offset` reset on wrap enable is critical — otherwise stale horizontal offsets would cause confusion when toggling back to nowrap mode.
- No need to guard the `update_horizontal_size()` call — it still updates `max_line_width` and `visible_width` even when wrap is on, so the values are correct when toggling back to nowrap.
