## Task: Global animation frame counter on AppState

**Objective**: Add an always-incrementing `animation_frame: u64` to `AppState`, advanced on every `Message::Tick` regardless of `UiMode`, to serve as the shared time source for all time-based animations.

**Depends on**: None

**Estimated Time**: 0.5–1h

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/state.rs`: add the `animation_frame: u64` field to the `AppState` struct (near other UI/animation fields), initialize to `0` in `with_settings()`, and optionally add a `pub fn animation_frame(&self) -> u64` accessor.
- `crates/fdemon-app/src/handler/update.rs`: in the `Message::Tick` arm, increment the counter unconditionally.

**Files Read (Dependencies):**
- None.

### Details

The 50 ms event-loop tick (`crates/fdemon-tui/src/event.rs`) already emits `Message::Tick` on idle. Today only the loading screen consumes it (`state.tick_loading_animation_with_cycling(true)` when `ui_mode == Loading`). Add a global counter that advances on every tick so widgets outside the loading screen can drive animations.

In `state.rs`, `AppState` struct:

```rust
pub struct AppState {
    // ... existing fields ...

    /// Monotonic animation tick, advanced once per `Message::Tick` (≈50 ms)
    /// regardless of `UiMode`. Shared time source for shimmer/spinner/flash
    /// animations. Wraps via `wrapping_add`; consumers use modulo arithmetic.
    pub animation_frame: u64,
}
```

Initialize in `with_settings()` (the single real constructor; `new()` delegates to it):

```rust
Self {
    // ...
    animation_frame: 0,
}
```

In `handler/update.rs`, `Message::Tick` arm:

```rust
Message::Tick => {
    // Global animation clock — drives shimmer/spinner/flash everywhere,
    // independent of the loading screen's own frame counter.
    state.animation_frame = state.animation_frame.wrapping_add(1);

    // Existing: loading screen animation with message cycling
    if state.ui_mode == UiMode::Loading && state.loading_state.is_some() {
        state.tick_loading_animation_with_cycling(true);
    }

    state.expire_toasts();
    UpdateResult::none()
}
```

### Acceptance Criteria

1. `AppState::animation_frame` exists, defaults to `0`.
2. Dispatching `Message::Tick` increments `animation_frame` by 1 in **any** `UiMode` (verified in a non-Loading mode).
3. The loading-screen animation path is unchanged (its own `LoadingState::animation_frame` still advances when in `Loading` mode).
4. Repeated ticks near `u64::MAX` wrap without panicking.

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_advances_global_animation_frame_in_normal_mode() {
        let mut state = AppState::new();
        state.ui_mode = UiMode::Normal;
        let before = state.animation_frame;
        let _ = update(&mut state, Message::Tick);
        assert_eq!(state.animation_frame, before + 1);
    }

    #[test]
    fn animation_frame_wraps_without_panic() {
        let mut state = AppState::new();
        state.animation_frame = u64::MAX;
        let _ = update(&mut state, Message::Tick);
        assert_eq!(state.animation_frame, 0);
    }
}
```

(Place the test alongside the existing `Message::Tick` / handler tests in `update.rs`, matching how the loading-tick behavior is currently tested.)

### Notes

- Do not remove or merge `LoadingState::animation_frame`; the two counters serve different layers.
- A public field matches existing `AppState` conventions; an accessor is optional sugar for the render layer.
- No keybinding, config, or doc changes required.

---

## Completion Summary

**Status:** Done
**Branch:** feat/ux-polish-and-multilaunch

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/state.rs` | Added `pub animation_frame: u64` field to `AppState` struct (with doc comment); initialized to `0` in `with_settings()` |
| `crates/fdemon-app/src/handler/update.rs` | Added `state.animation_frame = state.animation_frame.wrapping_add(1);` at the top of the `Message::Tick` arm; added two tests in `mod tests` |

### Notable Decisions/Tradeoffs

1. **Field placement**: `animation_frame` was placed just before `pending_runner_actions` in the struct, keeping it near other late-added UI/animation fields and avoiding disruption to the surrounding field ordering.
2. **No accessor added**: The task notes it as optional sugar; the public field matches existing `AppState` convention and is sufficient.
3. **LoadingState::animation_frame preserved**: Not merged or removed — both counters serve different layers as specified.

### Testing Performed

- `cargo check -p fdemon-app` - Passed
- `cargo test -p fdemon-app -- tick_advances_global_animation_frame_in_normal_mode animation_frame_wraps_without_panic` - Passed (2 tests)
- `cargo test -p fdemon-app` - Passed (2548 + 2 = 2550 unit tests, 0 failed)
- `cargo clippy -p fdemon-app` - Passed (no warnings or errors)

### Risks/Limitations

1. **None identified**: This is a purely additive change with no behavioral impact on existing code paths.
