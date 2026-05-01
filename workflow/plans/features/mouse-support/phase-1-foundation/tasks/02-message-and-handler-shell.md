## Task: Add `Message::Mouse` variant + no-op `handle_mouse` shell

**Objective**: Wire the new `MouseInput` type onto the existing TEA message bus by adding a `Message::Mouse(MouseInput)` variant, then create a `handler::mouse::handle_mouse` dispatcher that returns `None` for every `UiMode`. Phases 2+ rewrite the dispatcher; Phase 1 just establishes the routing so mouse events reach the engine and are silently consumed without crashing or mutating state.

**Depends on**: 01-input-mouse-type

**Estimated Time**: 1–2 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/message.rs` — add `Mouse(MouseInput)` variant; add `use crate::input_mouse::MouseInput;` to the imports
- `crates/fdemon-app/src/handler/mod.rs` — register the new `pub(crate) mod mouse;` submodule
- `crates/fdemon-app/src/handler/mouse.rs` — **NEW**: per-`UiMode` dispatcher that returns `None`, plus tests
- `crates/fdemon-app/src/handler/update.rs` — match `Message::Mouse(input)` and dispatch to `handle_mouse`

**Files Read (Dependencies):**
- `crates/fdemon-app/src/input_mouse.rs` — Task 01's type definitions
- `crates/fdemon-app/src/handler/keys.rs` — pattern reference for the per-`UiMode` match
- `crates/fdemon-app/src/state.rs` — `UiMode` variants (must be exhaustively matched)
- `crates/fdemon-app/src/handler/update.rs` — existing `Message::Key` dispatch case (lines 52–58) as the model for the `Message::Mouse` case

### Details

#### Step 1: Add the `Message::Mouse` variant

In `crates/fdemon-app/src/message.rs`, near the existing `Key(InputKey)` variant (around line 70), insert:

```rust
/// Mouse event from terminal (click, release, drag, scroll).
///
/// Routed to [`crate::handler::mouse::handle_mouse`] which dispatches
/// per `UiMode` to a concrete `Message`. Mouse events are no-ops in
/// Phase 1; later phases populate the dispatcher.
Mouse(MouseInput),
```

And add the import at the top of the file alongside the existing `use crate::input_key::InputKey;`:

```rust
use crate::input_mouse::MouseInput;
```

#### Step 2: Create the no-op dispatcher

Create `crates/fdemon-app/src/handler/mouse.rs`:

```rust
//! Mouse event handlers for different UI modes.
//!
//! Mirrors [`crate::handler::keys`] — converts a [`MouseInput`] into a
//! concrete [`Message`] based on the current [`UiMode`]. Phase 1 of the
//! mouse-support feature implements this as a no-op shell so events flow
//! into the engine without behavior changes; later phases populate per-mode
//! dispatch (scroll wheel, region hit-testing, dialog clicks).

use crate::input_mouse::MouseInput;
use crate::message::Message;
use crate::state::{AppState, UiMode};

/// Convert a mouse event to a follow-up message based on the current UI mode.
///
/// Returns `None` in Phase 1 — every variant is intentionally unhandled.
/// Phase 2 introduces scroll-wheel routing, Phase 3+ adds click hit-testing.
pub fn handle_mouse(state: &AppState, _input: MouseInput) -> Option<Message> {
    match state.ui_mode {
        UiMode::Startup
        | UiMode::Normal
        | UiMode::NewSessionDialog
        | UiMode::EmulatorSelector
        | UiMode::ConfirmDialog
        | UiMode::Loading
        | UiMode::SearchInput
        | UiMode::LinkHighlight
        | UiMode::Settings
        | UiMode::FlutterVersion
        | UiMode::DevTools => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input_mouse::{KeyModSet, MouseButton, ScrollDir};

    fn make_click() -> MouseInput {
        MouseInput::Click {
            x: 0,
            y: 0,
            button: MouseButton::Left,
            modifiers: KeyModSet::NONE,
        }
    }

    fn make_scroll_up() -> MouseInput {
        MouseInput::Scroll {
            x: 0,
            y: 0,
            direction: ScrollDir::Up,
            modifiers: KeyModSet::NONE,
        }
    }

    fn state_in_mode(mode: UiMode) -> AppState {
        let mut state = AppState::new(std::path::PathBuf::from("."));
        state.ui_mode = mode;
        state
    }

    /// Helper to assert handle_mouse returns None for a given (mode, input).
    fn assert_noop(mode: UiMode, input: MouseInput) {
        let state = state_in_mode(mode);
        assert_eq!(
            handle_mouse(&state, input),
            None,
            "expected no-op for {:?} + {:?}",
            mode,
            input
        );
    }

    #[test]
    fn test_click_no_op_in_every_mode() {
        for mode in [
            UiMode::Startup,
            UiMode::Normal,
            UiMode::NewSessionDialog,
            UiMode::EmulatorSelector,
            UiMode::ConfirmDialog,
            UiMode::Loading,
            UiMode::SearchInput,
            UiMode::LinkHighlight,
            UiMode::Settings,
            UiMode::FlutterVersion,
            UiMode::DevTools,
        ] {
            assert_noop(mode, make_click());
        }
    }

    #[test]
    fn test_scroll_no_op_in_every_mode() {
        for mode in [
            UiMode::Startup,
            UiMode::Normal,
            UiMode::NewSessionDialog,
            UiMode::EmulatorSelector,
            UiMode::ConfirmDialog,
            UiMode::Loading,
            UiMode::SearchInput,
            UiMode::LinkHighlight,
            UiMode::Settings,
            UiMode::FlutterVersion,
            UiMode::DevTools,
        ] {
            assert_noop(mode, make_scroll_up());
        }
    }
}
```

> **Note on `AppState::new`** — verify this constructor exists and takes a `PathBuf`. If the actual signature differs, adjust `state_in_mode` accordingly. Reference: `crates/fdemon-app/src/state.rs`.

#### Step 3: Register the submodule

In `crates/fdemon-app/src/handler/mod.rs`, add to the existing module list (alphabetically next to `keys`):

```rust
pub(crate) mod mouse;
```

#### Step 4: Wire `Message::Mouse` into `update`

In `crates/fdemon-app/src/handler/update.rs`, find the existing `Message::Key(key) => { ... }` arm (around line 52). Add the `Message::Mouse` arm immediately after it:

```rust
Message::Mouse(input) => {
    if let Some(msg) = super::mouse::handle_mouse(state, input) {
        UpdateResult::message(msg)
    } else {
        UpdateResult::none()
    }
}
```

(Use the same `super::mouse::handle_mouse(...)` invocation pattern that the file already uses for other handlers — check the import block at the top of `update.rs` and add `mouse` if needed; the existing imports follow the form `use super::{... keys::handle_key, ...};` so adding `mouse::handle_mouse` to that list is the natural extension.)

### Acceptance Criteria

1. `Message::Mouse(MouseInput)` exists in `message.rs`, derives `Debug` + `Clone` (inherited from the enum-level derive).
2. `handler::mouse::handle_mouse(state, input)` exists and returns `None` for every `UiMode` variant — *exhaustively matched*, no `_` catch-all.
3. `update(state, Message::Mouse(...))` dispatches through `handle_mouse` and returns `UpdateResult::none()` (since `handle_mouse` returns `None` in Phase 1).
4. `update` does not mutate `state` for any `Message::Mouse(...)` input.
5. The 22 unit tests above (11 `UiMode` × 2 inputs) all pass.
6. `cargo check -p fdemon-app --all-targets` passes.
7. `cargo test -p fdemon-app handler::mouse` passes.
8. `cargo clippy -p fdemon-app --all-targets -- -D warnings` passes — no `unused_variables` warnings on the underscore-prefixed `_input` parameter.
9. The `match state.ui_mode` block in `handle_mouse` does NOT use a `_` arm. New `UiMode` variants must force a compile error so we know to think about mouse routing for them.

### Testing

The unit tests above cover every `UiMode` × {click, scroll} pair (22 tests total). One end-to-end test asserts the `update` integration:

```rust
// In crates/fdemon-app/src/handler/tests.rs (or a new module if cleaner):
#[test]
fn test_update_mouse_message_is_no_op() {
    let mut state = AppState::new(std::path::PathBuf::from("."));
    let original_phase = state.phase.clone();
    let mouse = MouseInput::Click {
        x: 0, y: 0,
        button: MouseButton::Left,
        modifiers: KeyModSet::NONE,
    };
    let result = update(&mut state, Message::Mouse(mouse));
    assert!(result.message.is_none());
    assert!(result.action.is_none());
    assert_eq!(state.phase, original_phase);
}
```

Place this test wherever the existing `Message::Key` integration tests live — search for an existing test that constructs `AppState::new(...)` and `update(...)` together to find the canonical home.

### Notes

- **Exhaustive match is intentional.** Using `_` as a catch-all in `handle_mouse` would silently swallow new `UiMode` variants in the future. The exhaustive match guarantees we get a compile error when adding `UiMode::FooDialog`, forcing us to consciously decide its mouse behavior.
- **`_input` parameter naming.** Phase 1 leaves this `_`-prefixed so clippy doesn't warn. Phase 2 will rename it to `input` when scroll-wheel dispatch starts using it.
- **No new `Message` follow-up actions.** Phase 1 does not add any messages downstream of `handle_mouse`. The complete set of messages is unchanged from the current code; only the new `Mouse` variant is added.
