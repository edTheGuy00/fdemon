## Task: Normal-mode scroll routing (incl. tag-filter overlay)

**Objective**: Implement `crates/fdemon-app/src/handler/mouse/normal.rs::handle_scroll` so the wheel scrolls the log view in `UiMode::Normal`, with Shift+wheel page-scrolling. When the tag-filter overlay is open (`state.tag_filter_visible == true`), the wheel instead navigates the tag list — mirroring the keyboard handler at `keys.rs:105-126`.

**Depends on**: 01-mouse-handler-restructure

**Estimated Time**: 1h

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/mouse/normal.rs` — Replace the stub `handle_scroll` body with the routing logic described below; add a `#[cfg(test)] mod tests` block.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/state.rs` — `AppState::tag_filter_visible` (line 974).
- `crates/fdemon-app/src/message.rs` — `Message::ScrollUp`, `ScrollDown`, `PageUp`, `PageDown`, `TagFilterMoveUp`, `TagFilterMoveDown`.
- `crates/fdemon-app/src/input_mouse.rs` — `KeyModSet::is_shift_only` (added in Task 01).
- `crates/fdemon-app/src/handler/keys.rs` — Reference: `handle_key_normal` lines 100-272 (tag-filter overlay block at 105-126; scroll bindings at 263-272).

### Details

```rust
//! Scroll routing for `UiMode::Normal`.
//!
//! Mirrors the keyboard handler at [`crate::handler::keys::handle_key_normal`]:
//! when the tag-filter overlay is open it intercepts up/down navigation,
//! otherwise vertical scrolling drives the log view directly.

use crate::input_mouse::{KeyModSet, ScrollDir};
use crate::message::Message;
use crate::state::AppState;

pub(super) fn handle_scroll(
    state: &AppState,
    dir: ScrollDir,
    mods: KeyModSet,
) -> Option<Message> {
    // Tag-filter overlay intercepts wheel up/down (mirrors keys.rs:112-114).
    if state.tag_filter_visible {
        return match dir {
            ScrollDir::Up => Some(Message::TagFilterMoveUp),
            ScrollDir::Down => Some(Message::TagFilterMoveDown),
            ScrollDir::Left | ScrollDir::Right => None,
        };
    }

    // Shift+wheel → page scroll (mirrors keys.rs:269-270).
    if mods.is_shift_only() {
        return match dir {
            ScrollDir::Up => Some(Message::PageUp),
            ScrollDir::Down => Some(Message::PageDown),
            ScrollDir::Left | ScrollDir::Right => None,
        };
    }

    // Plain wheel → line scroll (mirrors keys.rs:265-266).
    match dir {
        ScrollDir::Up => Some(Message::ScrollUp),
        ScrollDir::Down => Some(Message::ScrollDown),
        ScrollDir::Left | ScrollDir::Right => None,
    }
}
```

**Modifier handling beyond Shift.** Ctrl+wheel and Alt+wheel return `None` rather than fall through to plain scroll. Rationale: terminals commonly bind Ctrl+wheel to font-size zoom, and Alt+wheel is reserved for future horizontal-scroll mappings; consuming them silently would surprise users. If the user holds modifiers other than Shift-only, the event is a no-op.

**Touchpad horizontal scroll** (`ScrollDir::Left` / `Right`): no consumer in v1 per PLAN.md "Out of scope" — return `None` for every state.

### Acceptance Criteria

1. With `tag_filter_visible == false` and `mods == KeyModSet::NONE`, `handle_scroll(state, ScrollDir::Up, mods)` returns `Some(Message::ScrollUp)` and `(ScrollDir::Down, mods)` returns `Some(Message::ScrollDown)`.
2. With `tag_filter_visible == false` and `mods.is_shift_only()`, `handle_scroll(state, ScrollDir::Up, mods)` returns `Some(Message::PageUp)` and `Down` returns `Some(Message::PageDown)`.
3. With `tag_filter_visible == true`, `handle_scroll(state, ScrollDir::Up, KeyModSet::NONE)` returns `Some(Message::TagFilterMoveUp)` and `Down` returns `Some(Message::TagFilterMoveDown)` regardless of Shift state (tag-filter overlay does not page-scroll).
4. `ScrollDir::Left` and `ScrollDir::Right` return `None` in every state.
5. `mods` with Ctrl-only or Alt-only (no Shift) returns `None` for vertical scroll (does not fall through to `ScrollUp`/`ScrollDown`).
6. `mods` with Ctrl+Shift or Alt+Shift returns `None` (because `is_shift_only()` is false).
7. `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets -- -D warnings` pass.

### Testing

Add to the new `#[cfg(test)] mod tests` block in `normal.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::input_mouse::KeyModSet;
    use crate::state::AppState;

    fn state_with_tag_filter(visible: bool) -> AppState {
        let mut s = AppState::new();
        s.tag_filter_visible = visible;
        s
    }

    #[test]
    fn plain_wheel_up_scrolls_up() {
        let s = state_with_tag_filter(false);
        let msg = handle_scroll(&s, ScrollDir::Up, KeyModSet::NONE);
        assert!(matches!(msg, Some(Message::ScrollUp)));
    }

    #[test]
    fn plain_wheel_down_scrolls_down() {
        let s = state_with_tag_filter(false);
        let msg = handle_scroll(&s, ScrollDir::Down, KeyModSet::NONE);
        assert!(matches!(msg, Some(Message::ScrollDown)));
    }

    #[test]
    fn shift_wheel_up_pages_up() {
        let s = state_with_tag_filter(false);
        let msg = handle_scroll(&s, ScrollDir::Up, KeyModSet::new(true, false, false));
        assert!(matches!(msg, Some(Message::PageUp)));
    }

    #[test]
    fn shift_wheel_down_pages_down() {
        let s = state_with_tag_filter(false);
        let msg = handle_scroll(&s, ScrollDir::Down, KeyModSet::new(true, false, false));
        assert!(matches!(msg, Some(Message::PageDown)));
    }

    #[test]
    fn ctrl_wheel_is_a_no_op() {
        let s = state_with_tag_filter(false);
        let msg = handle_scroll(&s, ScrollDir::Up, KeyModSet::new(false, true, false));
        assert!(msg.is_none());
    }

    #[test]
    fn alt_wheel_is_a_no_op() {
        let s = state_with_tag_filter(false);
        let msg = handle_scroll(&s, ScrollDir::Down, KeyModSet::new(false, false, true));
        assert!(msg.is_none());
    }

    #[test]
    fn ctrl_shift_wheel_is_a_no_op() {
        let s = state_with_tag_filter(false);
        let msg = handle_scroll(&s, ScrollDir::Up, KeyModSet::new(true, true, false));
        assert!(msg.is_none());
    }

    #[test]
    fn tag_filter_visible_routes_to_tag_filter_nav() {
        let s = state_with_tag_filter(true);
        assert!(matches!(
            handle_scroll(&s, ScrollDir::Up, KeyModSet::NONE),
            Some(Message::TagFilterMoveUp)
        ));
        assert!(matches!(
            handle_scroll(&s, ScrollDir::Down, KeyModSet::NONE),
            Some(Message::TagFilterMoveDown)
        ));
    }

    #[test]
    fn tag_filter_visible_ignores_shift_modifier() {
        let s = state_with_tag_filter(true);
        let mods = KeyModSet::new(true, false, false);
        // Tag-filter overlay does not page-scroll; Shift is dropped.
        assert!(matches!(
            handle_scroll(&s, ScrollDir::Up, mods),
            Some(Message::TagFilterMoveUp)
        ));
    }

    #[test]
    fn horizontal_wheel_is_no_op_in_both_states() {
        let off = state_with_tag_filter(false);
        let on = state_with_tag_filter(true);
        for s in [&off, &on] {
            assert!(handle_scroll(s, ScrollDir::Left, KeyModSet::NONE).is_none());
            assert!(handle_scroll(s, ScrollDir::Right, KeyModSet::NONE).is_none());
        }
    }
}
```

### Notes

- **No `is_busy` gate.** `handle_key_normal` comments at line 263 that scroll is "always allowed". Phase 2 follows that — wheel works during reload/restart.
- **Tag-filter overlay precedence.** Mirrors keyboard handler exactly: the overlay intercepts up/down even though the user is technically in `UiMode::Normal`. The wheel matches by reading `state.tag_filter_visible` rather than introducing a new sub-mode.
- **Why no page-scroll for tag filter.** The keyboard handler at `keys.rs:107-126` does not bind PageUp/PageDown to any tag-filter action, so Shift+wheel falls back to single-step move (rather than `None`). Choosing single-step is a small UX improvement: a user who happens to hold Shift while scrolling still navigates the tag list.
- **`ScrollLeft`/`ScrollRight`.** PLAN.md "Out of scope" defers horizontal-scroll consumers to a future phase. Returning `None` here is the documented v1 behavior.

---

## Completion Summary

**Status:** Done
**Branch:** feat/mouse-support

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/mouse/normal.rs` | Replaced stub `handle_scroll` with full routing logic; added 10-test `#[cfg(test)] mod tests` block covering all acceptance criteria |
| `crates/fdemon-app/src/handler/mouse/mod.rs` | Updated `test_scroll_no_op_in_every_mode` to remove `UiMode::Normal` (now produces real messages); added `test_scroll_normal_mode_returns_scroll_up` positive assertion |

### Notable Decisions/Tradeoffs

1. **Ctrl+wheel guard**: The task spec says Ctrl/Alt wheel return `None` rather than falling through to plain scroll. The implementation adds an explicit `if mods.ctrl || mods.alt { return None; }` guard after the shift-only check. This matches the spec's "Modifier handling beyond Shift" note exactly.
2. **mod.rs placeholder test update**: The `test_scroll_no_op_in_every_mode` test in `mod.rs` was written as a Phase 2 placeholder (all stubs returned `None`). Updating it was necessary to keep the suite green; the new comment documents that the list shrinks as each mode is wired.

### Testing Performed

- `cargo test -p fdemon-app handler::mouse::normal` — Passed (10/10 new tests)
- `cargo test -p fdemon-app --lib` — Passed (1939 tests, 0 failed)
- `cargo fmt --all -- --check` — Passed
- `cargo clippy --workspace --all-targets -- -D warnings` — Passed

### Risks/Limitations

1. **Other modes still stub**: `DevTools`, `Settings`, `NewSessionDialog`, `LinkHighlight`, `FlutterVersion` scroll handlers remain stubs returning `None`. Their Phase 2 tasks (03-06) will populate them and update `mod.rs` similarly.
