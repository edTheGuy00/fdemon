## Task: Mouse Press Dispatchers — Multi-Mode

**Objective**: Wire press hit-testing into every UI mode that Phase 5 makes clickable. Add `handle_press` to existing `handler/mouse/{settings,new_session,link_highlight}.rs`. Create new `handler/mouse/confirm_dialog.rs` and `handler/mouse/tag_filter.rs`. Update `handler/mouse/mod.rs::handle_press` to (a) lift the `tag_filter_visible` short-circuit into a route to `tag_filter::handle_press` and (b) add per-mode arms for `Settings`, `Startup | NewSessionDialog`, `ConfirmDialog`, and `LinkHighlight`. Modal precedence (z-index) is respected automatically by the registry's `hit_test`; this task does not change the registry.

**Depends on**: 01 (Phase-5 messages must exist for the dispatchers to reference)

**Estimated Time**: 1.75 hours

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/mouse/mod.rs`: Replace the `tag_filter_visible` short-circuit in `handle_press` with a route to `tag_filter::handle_press`. Add arms for `UiMode::Settings`, `UiMode::Startup | UiMode::NewSessionDialog`, `UiMode::ConfirmDialog`, `UiMode::LinkHighlight`. Update the doc-comment.
- `crates/fdemon-app/src/handler/mouse/settings.rs`: Add a sister `handle_press` function. Existing `handle_scroll` stays untouched.
- `crates/fdemon-app/src/handler/mouse/new_session.rs`: Add a sister `handle_press` function with modal precedence (fuzzy modal → dart-defines modal → main dialog). The fuzzy/dart-defines modals' clickable rows are wired in Task 09; the dispatcher already routes hit-tests via the registry, so modal precedence is handled by `z_index = 2` on sub-modal regions and `z_index = 1` on main dialog regions.
- `crates/fdemon-app/src/handler/mouse/link_highlight.rs`: Add a sister `handle_press` function.
- `crates/fdemon-app/src/handler/mouse/confirm_dialog.rs` (NEW): A new submodule with a single `handle_press` function. Add `mod confirm_dialog;` to `handler/mouse/mod.rs`.
- `crates/fdemon-app/src/handler/mouse/tag_filter.rs` (NEW): A new submodule with a single `handle_press` function. Add `mod tag_filter;` to `handler/mouse/mod.rs`.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/handler/mouse/normal.rs::handle_press` (template — shows the take-guard + hit-test pattern with right-click reservation).
- `crates/fdemon-app/src/handler/mouse/devtools.rs::handle_press` (template — shows `&mut AppState` usage when the press handler must mutate session state).
- `crates/fdemon-app/src/mouse_regions.rs::MouseRegionGuard` and `MouseRegions::hit_test`.

### Details

#### Common dispatcher pattern

Every new `handle_press` follows the Phase-3/4 template:

```rust
pub(super) fn handle_press(
    state: &mut AppState,
    x: u16,
    y: u16,
    button: MouseButton,
    _mods: KeyModSet,
) -> Option<Message> {
    // Right-click reserved for future context menus.
    if button == MouseButton::Right {
        return None;
    }

    // EXCEPTION: TEA render-hint write-back via Cell — see docs/CODE_STANDARDS.md Principle 3
    let regions = state.mouse_regions.take_guard();
    let action_opt = regions.hit_test(x, y, button).and_then(|entry| {
        let action = match button {
            MouseButton::Left => entry.on_left.as_ref(),
            MouseButton::Middle => entry.on_middle.as_ref(),
            MouseButton::Right => None,
        };
        action.map(|a| a.resolve(x, y))
    });
    drop(regions);

    action_opt
}
```

For `link_highlight::handle_press`, this template suffices unchanged — link badges just emit `Message::SelectLink(c)` and there are no busy/filter gates.

For `tag_filter::handle_press`, this template suffices — tag rows emit `Message::TagFilterClickRow { index }` and `[a]`/`[n]` action labels emit `Message::ShowAllNativeTags` / `Message::HideAllNativeTags`.

For `confirm_dialog::handle_press`, this template suffices — Yes/No buttons emit whatever message is stored in `confirm_dialog_state.actions[i].1`.

For `new_session::handle_press` and `settings::handle_press`, modal precedence is handled by the registry's `z_index` (sub-modals at z=2, main dialog at z=1, base UI at z=0). The dispatcher does not need to know which modal is open — it just hit-tests, and the highest z wins.

For `settings::handle_press`, a small additional check: when `editing == true`, return `None` (mirrors `handle_scroll`'s editing gate). The user is typing into a field; clicks must not move selection.

#### Updated `handler/mouse/mod.rs::handle_press`

```rust
mod confirm_dialog; // NEW (Phase 5 task 05)
mod devtools;
mod flutter_version;
mod link_highlight;
mod new_session;
mod normal;
mod settings;
mod tag_filter; // NEW (Phase 5 task 05)

// ...

/// Route a button press to the appropriate per-mode handler.
///
/// The tag-filter overlay routes to its own per-mode handler when visible —
/// see [`tag_filter::handle_press`]. (Earlier phases short-circuited press
/// to `None` here; Phase 5 task 05 lifted that gate so the overlay's tag
/// rows become clickable.) The keyboard handler at `handler/keys.rs:105-126`
/// continues to intercept ALL keys when the overlay is visible — only the
/// mouse path is reworked.
fn handle_press(
    state: &mut AppState,
    x: u16,
    y: u16,
    button: MouseButton,
    mods: KeyModSet,
) -> Option<Message> {
    // Tag-filter overlay routes to its own handler regardless of underlying ui_mode.
    if state.tag_filter_visible {
        return tag_filter::handle_press(state, x, y, button, mods);
    }

    match state.ui_mode {
        UiMode::Normal => normal::handle_press(state, x, y, button, mods),
        UiMode::DevTools => devtools::handle_press(state, x, y, button, mods),
        UiMode::ConfirmDialog => confirm_dialog::handle_press(state, x, y, button, mods),
        UiMode::Settings => settings::handle_press(state, x, y, button, mods),
        UiMode::Startup | UiMode::NewSessionDialog => {
            new_session::handle_press(state, x, y, button, mods)
        }
        UiMode::LinkHighlight => link_highlight::handle_press(state, x, y, button, mods),
        // No clickable surface in v1.
        UiMode::EmulatorSelector | UiMode::Loading | UiMode::SearchInput | UiMode::FlutterVersion => None,
    }
}
```

#### `confirm_dialog::handle_press`

```rust
//! Mouse press handling for `UiMode::ConfirmDialog`.
//!
//! Yes/No buttons (and any other action buttons stored on
//! `state.confirm_dialog_state.actions`) become clickable. The button's
//! action message is stored on the state; the dispatcher just resolves
//! whatever the registry returns.

use crate::input_mouse::{KeyModSet, MouseButton};
use crate::message::Message;
use crate::state::AppState;

pub(super) fn handle_press(
    state: &mut AppState,
    x: u16,
    y: u16,
    button: MouseButton,
    _mods: KeyModSet,
) -> Option<Message> {
    if button == MouseButton::Right {
        return None;
    }
    let regions = state.mouse_regions.take_guard();
    let action_opt = regions.hit_test(x, y, button).and_then(|entry| {
        let action = match button {
            MouseButton::Left => entry.on_left.as_ref(),
            MouseButton::Middle => entry.on_middle.as_ref(),
            MouseButton::Right => None,
        };
        action.map(|a| a.resolve(x, y))
    });
    drop(regions);
    action_opt
}
```

#### `tag_filter::handle_press`

```rust
//! Mouse press handling when the tag-filter overlay is visible.
//!
//! Routed to from `handler/mouse/mod.rs::handle_press` whenever
//! `state.tag_filter_visible == true`, regardless of the underlying
//! `ui_mode`. Mirrors how the keyboard handler intercepts all keys at
//! `handler/keys.rs:105-126`.
//!
//! Tag-row regions emit `Message::TagFilterClickRow { index }`; footer
//! action labels emit `Message::ShowAllNativeTags` / `Message::HideAllNativeTags`.

use crate::input_mouse::{KeyModSet, MouseButton};
use crate::message::Message;
use crate::state::AppState;

pub(super) fn handle_press(
    state: &mut AppState,
    x: u16,
    y: u16,
    button: MouseButton,
    _mods: KeyModSet,
) -> Option<Message> {
    if button == MouseButton::Right {
        return None;
    }
    let regions = state.mouse_regions.take_guard();
    let action_opt = regions.hit_test(x, y, button).and_then(|entry| {
        let action = match button {
            MouseButton::Left => entry.on_left.as_ref(),
            MouseButton::Middle => entry.on_middle.as_ref(),
            MouseButton::Right => None,
        };
        action.map(|a| a.resolve(x, y))
    });
    drop(regions);
    action_opt
}
```

#### `settings::handle_press`

```rust
pub(super) fn handle_press(
    state: &mut AppState,
    x: u16,
    y: u16,
    button: MouseButton,
    _mods: KeyModSet,
) -> Option<Message> {
    if button == MouseButton::Right {
        return None;
    }

    // Edit-mode gate — mirrors `handle_scroll`'s `state.settings_view_state.editing`
    // gate. Click while editing must not move selection.
    //
    // Exception: clicks on dart-defines/extra-args sub-modal buttons (when
    // they are wired in Phase 6) need to land. For v1 we keep the gate
    // strict — sub-modals are deferred — and revisit when sub-modals get
    // their click regions.
    if state.settings_view_state.editing {
        return None;
    }

    let regions = state.mouse_regions.take_guard();
    let action_opt = regions.hit_test(x, y, button).and_then(|entry| {
        let action = match button {
            MouseButton::Left => entry.on_left.as_ref(),
            MouseButton::Middle => entry.on_middle.as_ref(),
            MouseButton::Right => None,
        };
        action.map(|a| a.resolve(x, y))
    });
    drop(regions);
    action_opt
}
```

#### `new_session::handle_press`

```rust
pub(super) fn handle_press(
    state: &mut AppState,
    x: u16,
    y: u16,
    button: MouseButton,
    _mods: KeyModSet,
) -> Option<Message> {
    if button == MouseButton::Right {
        return None;
    }
    // Modal precedence is handled by `z_index`:
    //   - main dialog regions: z = 1
    //   - fuzzy modal / dart-defines modal regions: z = 2
    // The registry's hit_test returns the highest-z match — so a click
    // inside an open fuzzy modal lands on the modal's row, not the
    // device-list row underneath.
    //
    // No edit-mode gate — the dialog has no inline-edit state. Field
    // activation goes through the keyboard-Enter chain (Message::FieldActivate).

    let regions = state.mouse_regions.take_guard();
    let action_opt = regions.hit_test(x, y, button).and_then(|entry| {
        let action = match button {
            MouseButton::Left => entry.on_left.as_ref(),
            MouseButton::Middle => entry.on_middle.as_ref(),
            MouseButton::Right => None,
        };
        action.map(|a| a.resolve(x, y))
    });
    drop(regions);
    action_opt
}
```

#### `link_highlight::handle_press`

```rust
pub(super) fn handle_press(
    state: &mut AppState,
    x: u16,
    y: u16,
    button: MouseButton,
    _mods: KeyModSet,
) -> Option<Message> {
    if button == MouseButton::Right {
        return None;
    }
    let regions = state.mouse_regions.take_guard();
    let action_opt = regions.hit_test(x, y, button).and_then(|entry| {
        let action = match button {
            MouseButton::Left => entry.on_left.as_ref(),
            MouseButton::Middle => entry.on_middle.as_ref(),
            MouseButton::Right => None,
        };
        action.map(|a| a.resolve(x, y))
    });
    drop(regions);
    action_opt
}
```

### Acceptance Criteria

1. `cargo check --workspace --all-targets` passes.
2. `cargo test --workspace` passes — the new tests below are added and pass; the existing `dispatcher_press_tag_filter_visible_is_no_op` test (currently in `handler/mouse/mod.rs`) is **deleted** and replaced with a positive-routing test (see Notes).
3. `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets -- -D warnings` pass.
4. Each of `handler/mouse/{settings,new_session,link_highlight,confirm_dialog,tag_filter}.rs` exports a `pub(super) fn handle_press(state: &mut AppState, x: u16, y: u16, button: MouseButton, mods: KeyModSet) -> Option<Message>`.
5. `handler/mouse/mod.rs::handle_press` no longer returns `None` unconditionally for `tag_filter_visible`; it routes to `tag_filter::handle_press`.
6. Right-click and middle-click in the new modes return `None` for v1 (except where a region explicitly registers an `on_middle` binding — none do in Phase 5).
7. `Settings::handle_press` returns `None` when `editing == true`, even if a region is registered at the click coordinate.

### Testing

#### Replace dispatcher tag-filter test

The existing `dispatcher_press_tag_filter_visible_is_no_op` test in `handler/mouse/mod.rs` asserts that `tag_filter_visible` short-circuits press to `None`. This is the gate Phase 5 explicitly removes. Delete it and add the new positive-routing test:

```rust
#[test]
fn dispatcher_press_tag_filter_visible_routes_to_tag_filter_handler() {
    use crate::mouse_regions::{MouseAction, MouseRect};

    for mode in [
        UiMode::Normal,
        UiMode::DevTools,
        UiMode::Settings,
        UiMode::NewSessionDialog,
    ] {
        let mut state = state_in_mode(mode);
        state.tag_filter_visible = true;

        // Register a tag-row click region that the tag_filter handler should hit.
        let mut regions = state.mouse_regions.take();
        regions.builder().click(
            MouseRect::new(0, 0, 10, 1),
            MouseAction::emit(Message::TagFilterClickRow { index: 0 }),
        );
        state.mouse_regions.set(regions);

        let result = handle_mouse(
            &mut state,
            MouseInput::Press {
                x: 0,
                y: 0,
                button: MouseButton::Left,
                modifiers: KeyModSet::NONE,
            },
        );
        assert!(
            matches!(result, Some(Message::TagFilterClickRow { index: 0 })),
            "tag_filter_visible should route press to tag_filter handler in {:?} mode, got {:?}",
            mode,
            result
        );
    }
}
```

#### Per-submodule tests

Each new dispatcher submodule gets a 2–3 test smoke suite (no-region returns None, registered-region returns the message, right-click returns None). Mirror the pattern from `handler/mouse/normal.rs::tests`.

Example for `confirm_dialog::tests`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::input_mouse::KeyModSet;
    use crate::mouse_regions::{MouseAction, MouseRect};
    use crate::state::{AppState, UiMode};

    fn state_in_confirm_dialog() -> AppState {
        let mut s = AppState::new();
        s.ui_mode = UiMode::ConfirmDialog;
        s
    }

    #[test]
    fn no_region_returns_none() {
        let mut s = state_in_confirm_dialog();
        assert!(handle_press(&mut s, 0, 0, MouseButton::Left, KeyModSet::NONE).is_none());
    }

    #[test]
    fn click_on_yes_button_returns_confirm_quit() {
        let mut s = state_in_confirm_dialog();
        let mut regions = s.mouse_regions.take();
        regions.builder().click_at_z(
            MouseRect::new(0, 0, 5, 1),
            MouseAction::emit(Message::ConfirmQuit),
            1,
        );
        s.mouse_regions.set(regions);
        let r = handle_press(&mut s, 0, 0, MouseButton::Left, KeyModSet::NONE);
        assert!(matches!(r, Some(Message::ConfirmQuit)));
    }

    #[test]
    fn right_click_is_no_op() {
        let mut s = state_in_confirm_dialog();
        let mut regions = s.mouse_regions.take();
        regions.builder().click(
            MouseRect::new(0, 0, 5, 1),
            MouseAction::emit(Message::ConfirmQuit),
        );
        s.mouse_regions.set(regions);
        assert!(handle_press(&mut s, 0, 0, MouseButton::Right, KeyModSet::NONE).is_none());
    }
}
```

Repeat the same shape for `tag_filter::tests`, `settings::tests` (additional editing-gate test), `new_session::tests`, `link_highlight::tests`. Each suite is ~3–4 tests. Total: ~15–20 new tests.

#### Settings editing-gate test

```rust
#[test]
fn click_while_editing_returns_none() {
    let mut s = AppState::new();
    s.ui_mode = UiMode::Settings;
    s.settings_view_state.editing = true;
    let mut regions = s.mouse_regions.take();
    regions.builder().click(
        MouseRect::new(0, 0, 10, 1),
        MouseAction::emit(Message::SettingsClickRow { index: 0 }),
    );
    s.mouse_regions.set(regions);
    let r = handle_press(&mut s, 0, 0, MouseButton::Left, KeyModSet::NONE);
    assert!(r.is_none(), "click while editing must be a no-op");
}
```

### Notes

- **Why we delete `dispatcher_press_tag_filter_visible_is_no_op` instead of refactoring it.** That test was a *negative* lock-in: "press is a no-op when tag_filter_visible." Phase 5 changes the contract to "press routes to tag_filter handler when tag_filter_visible." A refactored test would still assert "no-op when no region matches," which is just the empty-registry case — the new test above asserts the positive routing instead, which is what we actually want to lock in.
- **Why right-click is reserved.** Phase 3 deferred right-click context menus; Phase 5 maintains that. A future right-click context menu task can selectively enable per-mode dispatch.
- **Why `confirm_dialog::handle_press` doesn't read `confirm_dialog_state`.** The registry stores `MouseAction::emit(Message::ConfirmQuit)` (or whatever action is in `state.confirm_dialog_state.actions[i].1`) directly. The widget recording (Task 06) reads `confirm_dialog_state.actions` at render time; the dispatcher just hit-tests. The state-read is one-way.
- **Why `new_session::handle_press` doesn't special-case fuzzy/dart-defines modals.** Same reason — z-index handles modal precedence at the registry level. The widget recording (Task 09) puts the main dialog regions at z=1 and the modal-overlay regions at z=2 (when those modals are open). The dispatcher doesn't need to know.
- **Why `Settings::handle_press` has an editing gate but `new_session::handle_press` doesn't.** Settings has inline edit mode (text-buffer typing on a row). NewSessionDialog has no inline editing — the dart-defines modal is a full-screen overlay that handles its own input. So the gate is unnecessary there.
- **Why no busy-gate in any Phase-5 handler.** None of the Phase-5 click messages are gated by `any_session_busy()`. Hot-reload-while-busy is the only such gate (Phase 3 task 06), and that's only on header `[r]`/`[R]`/`[x]` clicks in `Normal` mode.
- **`tag_filter_visible` interaction with the underlying mode.** The keyboard handler intercepts all keys when `tag_filter_visible` (gating at `handler/keys.rs:105-126`); the mouse handler now intercepts presses. Scroll routing in `handle_scroll` is unchanged — it already routes to `TagFilterMoveUp/Down` when visible.

---

## Completion Summary

**Status:** Done
**Branch:** feat/mouse-support

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/mouse/mod.rs` | Added `mod confirm_dialog` and `mod tag_filter`; replaced `tag_filter_visible` short-circuit with positive route to `tag_filter::handle_press`; added arms for `Settings`, `Startup|NewSessionDialog`, `ConfirmDialog`, `LinkHighlight`; replaced `dispatcher_press_tag_filter_visible_is_no_op` test with `dispatcher_press_tag_filter_visible_routes_to_tag_filter_handler` |
| `crates/fdemon-app/src/handler/mouse/confirm_dialog.rs` | NEW: `handle_press` with right-click guard + hit-test pattern; 4 tests |
| `crates/fdemon-app/src/handler/mouse/tag_filter.rs` | NEW: `handle_press` with right-click guard + hit-test pattern; 4 tests |
| `crates/fdemon-app/src/handler/mouse/settings.rs` | Added `handle_press` with editing gate; updated imports; 4 new tests |
| `crates/fdemon-app/src/handler/mouse/new_session.rs` | Added `handle_press` with z-index modal precedence via registry; updated imports; 3 new tests |
| `crates/fdemon-app/src/handler/mouse/link_highlight.rs` | Added `handle_press` with right-click guard + hit-test pattern; updated imports; 3 new tests |

### Notable Decisions/Tradeoffs

1. **Test replacement over refactoring**: Deleted `dispatcher_press_tag_filter_visible_is_no_op` as specified — the old test locked in a negative contract that Phase 5 explicitly removes. Replaced with `dispatcher_press_tag_filter_visible_routes_to_tag_filter_handler` which asserts positive routing across multiple modes.
2. **Settings editing gate position**: Gate is checked before hit-test (early return), matching the task spec and mirroring `handle_scroll`'s gate position. Sub-modal interactions deferred to Phase 6.
3. **Module-level imports**: Added `MouseButton` to the imports of `settings.rs`, `new_session.rs`, and `link_highlight.rs` — `ScrollDir` stays to avoid removing unused-import warnings.

### Testing Performed

- `cargo check --workspace --all-targets` - Passed
- `cargo test --workspace` - Passed (all tests pass, 0 failed)
- `cargo test -p fdemon-app handler::mouse` - Passed (93 tests)
- `cargo fmt --all -- --check` - Passed (after formatting fix for long arm)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed

### Risks/Limitations

1. **Sub-modal click regions not yet registered**: `settings::handle_press` and `new_session::handle_press` will route hits, but the corresponding widget recording tasks (06-10) haven't been completed yet — clicks will return `None` until those tasks populate the registry.
