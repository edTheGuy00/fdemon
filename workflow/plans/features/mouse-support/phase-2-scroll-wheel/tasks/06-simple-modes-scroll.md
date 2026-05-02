## Task: Simple modes scroll routing (LinkHighlight + FlutterVersion)

**Objective**: Implement `handler/mouse/link_highlight.rs::handle_scroll` and `handler/mouse/flutter_version.rs::handle_scroll`. LinkHighlight mode mirrors Normal mode's scroll behavior (line scroll + Shift page scroll). FlutterVersion mode does single-step list navigation only.

**Depends on**: 01-mouse-handler-restructure

**Estimated Time**: 0.5h

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/mouse/link_highlight.rs` — Replace stub `handle_scroll`; add `#[cfg(test)] mod tests`.
- `crates/fdemon-app/src/handler/mouse/flutter_version.rs` — Replace stub `handle_scroll`; add `#[cfg(test)] mod tests`.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/message.rs` — `Message::ScrollUp`, `ScrollDown`, `PageUp`, `PageDown`, `FlutterVersionUp`, `FlutterVersionDown`.
- `crates/fdemon-app/src/input_mouse.rs` — `KeyModSet::is_shift_only` (added in Task 01).
- `crates/fdemon-app/src/handler/keys.rs` — Reference: `handle_key_link_highlight` lines 361-383 (especially scroll bindings at 370-373); `handle_key_flutter_version` lines 332-355 (especially nav at 344-345).

### Details

**`link_highlight.rs`** — LinkHighlight reuses the same scroll messages as Normal mode (`Message::ScrollUp`/`ScrollDown` for line, `PageUp`/`PageDown` for page). The keyboard handler at `keys.rs:370-373` confirms the binding parity.

```rust
//! Scroll routing for `UiMode::LinkHighlight`.
//!
//! Mirrors `handle_key_link_highlight` (keys.rs:361-383): plain wheel scrolls
//! the log view; Shift+wheel does page scroll. Same messages as Normal mode.

use crate::input_mouse::{KeyModSet, ScrollDir};
use crate::message::Message;
use crate::state::AppState;

pub(super) fn handle_scroll(
    _state: &AppState,
    dir: ScrollDir,
    mods: KeyModSet,
) -> Option<Message> {
    if mods.is_shift_only() {
        return match dir {
            ScrollDir::Up => Some(Message::PageUp),
            ScrollDir::Down => Some(Message::PageDown),
            ScrollDir::Left | ScrollDir::Right => None,
        };
    }
    if mods.ctrl || mods.alt {
        return None;
    }
    match dir {
        ScrollDir::Up => Some(Message::ScrollUp),
        ScrollDir::Down => Some(Message::ScrollDown),
        ScrollDir::Left | ScrollDir::Right => None,
    }
}
```

**`flutter_version.rs`** — Single-step navigation only. The keyboard handler at `keys.rs:344-345` binds `k`/Up to `FlutterVersionUp` and `j`/Down to `FlutterVersionDown` with no page-step variant.

```rust
//! Scroll routing for `UiMode::FlutterVersion`.
//!
//! Mirrors `handle_key_flutter_version` (keys.rs:332-355): wheel up/down
//! moves the version list selection; no page-step (no keyboard analogue).

use crate::input_mouse::{KeyModSet, ScrollDir};
use crate::message::Message;
use crate::state::AppState;

pub(super) fn handle_scroll(
    _state: &AppState,
    dir: ScrollDir,
    _mods: KeyModSet,
) -> Option<Message> {
    match dir {
        ScrollDir::Up => Some(Message::FlutterVersionUp),
        ScrollDir::Down => Some(Message::FlutterVersionDown),
        ScrollDir::Left | ScrollDir::Right => None,
    }
}
```

### Acceptance Criteria

**LinkHighlight:**

1. `Up` no modifiers → `Some(Message::ScrollUp)`.
2. `Down` no modifiers → `Some(Message::ScrollDown)`.
3. `Up` Shift-only → `Some(Message::PageUp)`.
4. `Down` Shift-only → `Some(Message::PageDown)`.
5. Ctrl-only / Alt-only / Ctrl+Shift / Alt+Shift → `None`.
6. `Left` / `Right` → `None`.

**FlutterVersion:**

7. `Up` (any modifier) → `Some(Message::FlutterVersionUp)`.
8. `Down` (any modifier) → `Some(Message::FlutterVersionDown)`.
9. `Left` / `Right` → `None`.
10. No new `Message` variants added.

### Testing

```rust
// link_highlight.rs
#[cfg(test)]
mod tests {
    use super::*;
    use crate::input_mouse::KeyModSet;
    use crate::state::AppState;

    #[test]
    fn plain_wheel_scrolls() {
        let s = AppState::new();
        assert!(matches!(
            handle_scroll(&s, ScrollDir::Up, KeyModSet::NONE),
            Some(Message::ScrollUp)
        ));
        assert!(matches!(
            handle_scroll(&s, ScrollDir::Down, KeyModSet::NONE),
            Some(Message::ScrollDown)
        ));
    }

    #[test]
    fn shift_wheel_pages() {
        let s = AppState::new();
        let mods = KeyModSet::new(true, false, false);
        assert!(matches!(
            handle_scroll(&s, ScrollDir::Up, mods),
            Some(Message::PageUp)
        ));
        assert!(matches!(
            handle_scroll(&s, ScrollDir::Down, mods),
            Some(Message::PageDown)
        ));
    }

    #[test]
    fn ctrl_or_alt_only_no_op() {
        let s = AppState::new();
        assert!(handle_scroll(&s, ScrollDir::Up, KeyModSet::new(false, true, false)).is_none());
        assert!(handle_scroll(&s, ScrollDir::Down, KeyModSet::new(false, false, true)).is_none());
    }

    #[test]
    fn ctrl_shift_no_op() {
        let s = AppState::new();
        let msg = handle_scroll(&s, ScrollDir::Up, KeyModSet::new(true, true, false));
        assert!(msg.is_none());
    }

    #[test]
    fn horizontal_wheel_no_op() {
        let s = AppState::new();
        assert!(handle_scroll(&s, ScrollDir::Left, KeyModSet::NONE).is_none());
        assert!(handle_scroll(&s, ScrollDir::Right, KeyModSet::NONE).is_none());
    }
}
```

```rust
// flutter_version.rs
#[cfg(test)]
mod tests {
    use super::*;
    use crate::input_mouse::KeyModSet;
    use crate::state::AppState;

    #[test]
    fn wheel_up_moves_version_selection_up() {
        let s = AppState::new();
        assert!(matches!(
            handle_scroll(&s, ScrollDir::Up, KeyModSet::NONE),
            Some(Message::FlutterVersionUp)
        ));
    }

    #[test]
    fn wheel_down_moves_version_selection_down() {
        let s = AppState::new();
        assert!(matches!(
            handle_scroll(&s, ScrollDir::Down, KeyModSet::NONE),
            Some(Message::FlutterVersionDown)
        ));
    }

    #[test]
    fn modifiers_do_not_change_behavior() {
        let s = AppState::new();
        for mods in [
            KeyModSet::new(true, false, false),
            KeyModSet::new(false, true, false),
            KeyModSet::new(false, false, true),
            KeyModSet::new(true, true, true),
        ] {
            assert!(matches!(
                handle_scroll(&s, ScrollDir::Up, mods),
                Some(Message::FlutterVersionUp)
            ));
        }
    }

    #[test]
    fn horizontal_wheel_no_op() {
        let s = AppState::new();
        assert!(handle_scroll(&s, ScrollDir::Left, KeyModSet::NONE).is_none());
        assert!(handle_scroll(&s, ScrollDir::Right, KeyModSet::NONE).is_none());
    }
}
```

### Notes

- **Why these two modes share a task.** Both are simple list-style modes with minimal sub-state, share the same testing pattern, and together total ~30 LOC of production code. Splitting into two tasks would over-fragment the orchestrator wave for negligible gain.
- **LinkHighlight reuses log-view messages.** `Message::ScrollUp`/`PageUp` are the same messages Normal mode emits — the underlying `scroll::handle_scroll_up` dispatcher decides what to do based on `state.ui_mode`. No code duplication; the mouse handler simply emits the existing message.
- **FlutterVersion ignores modifiers fully.** Even Shift+wheel single-steps. The keyboard handler binds nothing to Shift+anything in this mode (`keys.rs:332-355`); the mouse mirrors.
- **The "simple no-op modes"** (`SearchInput`, `ConfirmDialog`, `EmulatorSelector`, `Loading`) are already inlined as `None` in `handler/mouse/mod.rs` by Task 01. This task does not touch them.
