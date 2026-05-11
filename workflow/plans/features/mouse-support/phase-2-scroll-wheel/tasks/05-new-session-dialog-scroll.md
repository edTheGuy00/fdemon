## Task: NewSessionDialog/Startup scroll routing

**Objective**: Implement `crates/fdemon-app/src/handler/mouse/new_session.rs::handle_scroll` for the `Startup` and `NewSessionDialog` modes (which share a single keyboard handler — see `keys.rs:11`). The wheel must respect modal precedence (fuzzy modal → dart-defines modal → main dialog) and, in the main dialog, dispatch by `focused_pane` (TargetSelector vs LaunchContext) to navigate device list or launch-context fields.

**Depends on**: 01-mouse-handler-restructure

**Estimated Time**: 1h

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/mouse/new_session.rs` — Replace stub `handle_scroll`; add `#[cfg(test)] mod tests`.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/new_session_dialog.rs` — `NewSessionDialogState`, `DialogPane::{TargetSelector, LaunchContext}`, `is_fuzzy_modal_open()`, `is_dart_defines_modal_open()`, `dart_defines_modal.active_pane`.
- `crates/fdemon-app/src/state.rs` — `AppState::new_session_dialog_state`.
- `crates/fdemon-app/src/message.rs` — `Message::NewSessionDialogFuzzyUp`, `NewSessionDialogFuzzyDown`, `NewSessionDialogDartDefinesUp`, `NewSessionDialogDartDefinesDown`, `NewSessionDialogDeviceUp`, `NewSessionDialogDeviceDown`, `NewSessionDialogFieldPrev`, `NewSessionDialogFieldNext`.
- `crates/fdemon-app/src/handler/keys.rs` — Reference: `handle_key_new_session_dialog` lines 793-825 (modal precedence at 798-804, pane dispatch at 819-823); `handle_fuzzy_modal_key` 827-837; `handle_dart_defines_modal_key` 839-866; `handle_target_selector_key` 868-876; `handle_launch_context_key` 878-896.

### Details

```rust
//! Scroll routing for `UiMode::Startup` and `UiMode::NewSessionDialog`.
//!
//! Both modes share a single keyboard handler in `handler::keys` and share
//! this mouse handler too. Modal precedence mirrors `handle_key_new_session_dialog`:
//! fuzzy modal → dart-defines modal → main dialog (dispatched by focused pane).

use crate::input_mouse::{KeyModSet, ScrollDir};
use crate::message::Message;
use crate::new_session_dialog::DialogPane;
use crate::state::AppState;

pub(super) fn handle_scroll(
    state: &AppState,
    dir: ScrollDir,
    _mods: KeyModSet,
) -> Option<Message> {
    let dialog = &state.new_session_dialog_state;

    // Modal precedence — matches keys.rs:799-804.
    if dialog.is_fuzzy_modal_open() {
        return match dir {
            ScrollDir::Up => Some(Message::NewSessionDialogFuzzyUp),
            ScrollDir::Down => Some(Message::NewSessionDialogFuzzyDown),
            _ => None,
        };
    }

    if dialog.is_dart_defines_modal_open() {
        // The dart-defines modal handler at keys.rs:851-855 routes Up/Down
        // unconditionally (regardless of List vs Edit pane) — so do we.
        return match dir {
            ScrollDir::Up => Some(Message::NewSessionDialogDartDefinesUp),
            ScrollDir::Down => Some(Message::NewSessionDialogDartDefinesDown),
            _ => None,
        };
    }

    // Main dialog — dispatch by focused pane.
    match dialog.focused_pane {
        DialogPane::TargetSelector => match dir {
            ScrollDir::Up => Some(Message::NewSessionDialogDeviceUp),
            ScrollDir::Down => Some(Message::NewSessionDialogDeviceDown),
            _ => None,
        },
        DialogPane::LaunchContext => match dir {
            ScrollDir::Up => Some(Message::NewSessionDialogFieldPrev),
            ScrollDir::Down => Some(Message::NewSessionDialogFieldNext),
            _ => None,
        },
    }
}
```

**Modifier handling.** The NewSessionDialog keyboard handler binds no PageUp/PageDown, no Shift+anything to navigation. As with Settings, the mouse handler ignores modifiers and treats any wheel as single-step. `_mods` is unused.

**Why dart-defines modal does not split List vs Edit.** Looking at the keyboard handler at `keys.rs:851-855`, the dart-defines modal in NewSessionDialog routes `InputKey::Up`/`Down` unconditionally — both in List and Edit panes — to the same `Up`/`Down` messages. The handler at `keys.rs:856-861` differentiates panes only for `Esc` (to switch panes vs cancel modal). So the mouse handler also routes wheel unconditionally; if a user is in the Edit pane the message is still `Up`/`Down` and the update handler decides what that means in context.

**Note**: This differs from the Settings dart-defines mouse handler in Task 04, which DOES gate Edit pane to no-op. The asymmetry comes from the keyboard handlers themselves — `handle_key_settings_dart_defines` (`keys.rs:733-770`) routes Up/Down only in List pane, while `handle_dart_defines_modal_key` (`keys.rs:839-866`) routes Up/Down in both panes. Mouse must mirror keyboard exactly to avoid surprising users.

### Acceptance Criteria

1. Fuzzy modal open: `Up` → `NewSessionDialogFuzzyUp`, `Down` → `NewSessionDialogFuzzyDown`. Takes precedence over everything else.
2. Dart-defines modal open (any pane): `Up` → `NewSessionDialogDartDefinesUp`, `Down` → `NewSessionDialogDartDefinesDown`. Takes precedence over main dialog.
3. Main dialog, `focused_pane == TargetSelector`: `Up` → `NewSessionDialogDeviceUp`, `Down` → `NewSessionDialogDeviceDown`.
4. Main dialog, `focused_pane == LaunchContext`: `Up` → `NewSessionDialogFieldPrev`, `Down` → `NewSessionDialogFieldNext`.
5. `ScrollDir::Left` / `ScrollDir::Right` → `None` in every state.
6. Modifiers do not change behavior (no Shift+wheel page-step in this mode).
7. Both `UiMode::Startup` and `UiMode::NewSessionDialog` route through this handler (verified via the `mod.rs` dispatcher, not in the submodule itself — but the submodule must operate on `state.new_session_dialog_state` consistently for both).
8. No new `Message` variants added.

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::input_mouse::KeyModSet;
    use crate::new_session_dialog::DialogPane;
    use crate::state::AppState;

    fn fresh_state() -> AppState {
        AppState::new()
    }

    #[test]
    fn main_dialog_target_selector_scroll_moves_device_selection() {
        let mut s = fresh_state();
        s.new_session_dialog_state.focused_pane = DialogPane::TargetSelector;
        assert!(matches!(
            handle_scroll(&s, ScrollDir::Up, KeyModSet::NONE),
            Some(Message::NewSessionDialogDeviceUp)
        ));
        assert!(matches!(
            handle_scroll(&s, ScrollDir::Down, KeyModSet::NONE),
            Some(Message::NewSessionDialogDeviceDown)
        ));
    }

    #[test]
    fn main_dialog_launch_context_scroll_moves_field_focus() {
        let mut s = fresh_state();
        s.new_session_dialog_state.focused_pane = DialogPane::LaunchContext;
        assert!(matches!(
            handle_scroll(&s, ScrollDir::Up, KeyModSet::NONE),
            Some(Message::NewSessionDialogFieldPrev)
        ));
        assert!(matches!(
            handle_scroll(&s, ScrollDir::Down, KeyModSet::NONE),
            Some(Message::NewSessionDialogFieldNext)
        ));
    }

    #[test]
    fn fuzzy_modal_takes_precedence_over_main_dialog() {
        // Implementor: open fuzzy modal via existing test helper. Verify Up/Down
        // route to NewSessionDialogFuzzyUp/Down even with focused_pane set to
        // TargetSelector (which would otherwise route to DeviceUp/Down).
    }

    #[test]
    fn dart_defines_modal_takes_precedence_over_main_dialog() {
        // Implementor: open dart-defines modal via existing test helper.
        // Verify Up/Down route to NewSessionDialogDartDefinesUp/Down regardless
        // of underlying focused_pane.
    }

    #[test]
    fn dart_defines_modal_routes_in_both_panes() {
        // Open dart-defines modal in List pane: Up → DartDefinesUp.
        // Switch to Edit pane: Up → still DartDefinesUp (unlike Settings dart-defines).
    }

    #[test]
    fn modifier_keys_do_not_change_behavior() {
        let mut s = fresh_state();
        s.new_session_dialog_state.focused_pane = DialogPane::TargetSelector;
        for mods in [
            KeyModSet::new(true, false, false),
            KeyModSet::new(false, true, false),
            KeyModSet::new(true, true, false),
        ] {
            assert!(matches!(
                handle_scroll(&s, ScrollDir::Up, mods),
                Some(Message::NewSessionDialogDeviceUp)
            ));
        }
    }

    #[test]
    fn horizontal_wheel_no_op_in_every_pane_and_modal() {
        let mut s = fresh_state();
        for pane in [DialogPane::TargetSelector, DialogPane::LaunchContext] {
            s.new_session_dialog_state.focused_pane = pane;
            assert!(handle_scroll(&s, ScrollDir::Left, KeyModSet::NONE).is_none());
            assert!(handle_scroll(&s, ScrollDir::Right, KeyModSet::NONE).is_none());
        }
    }
}
```

The fuzzy-modal and dart-defines-modal tests are sketched as TODOs because constructing those modal states requires the existing test helpers in `crates/fdemon-app/src/handler/new_session.rs` (or the relevant test module). Implementor should grep for `is_fuzzy_modal_open()` and `is_dart_defines_modal_open()` in test code and reuse the helpers.

### Notes

- **Why route both `Startup` and `NewSessionDialog` here.** `handle_key` dispatches both to `handle_key_new_session_dialog` (`keys.rs:11`). The Phase 2 mouse dispatcher in `handler/mouse/mod.rs` does the same. The submodule operates on `state.new_session_dialog_state`, which is populated identically in both modes.
- **Asymmetry with Settings dart-defines pane handling.** Documented inline above. The asymmetry exists today in the keyboard handler and is intentional UX; the mouse must follow.
- **No PageUp/PageDown.** The keyboard handlers in `keys.rs:828-836` (fuzzy) and `851-855` (dart-defines) bind only `Up` and `Down`. Inventing Shift+wheel page-step here would diverge from the keyboard.
- **Refresh devices (`r` key) is not a scroll target.** PLAN.md Phase 5 covers click handling for the device row → confirm flow; Phase 2 stays scroll-only.
- **Settings access from NewSessionDialog (`,` key)** is not a scroll target either.

---

## Completion Summary

**Status:** Done
**Branch:** feat/mouse-support

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/mouse/new_session.rs` | Replaced stub with full `handle_scroll` implementation; added 9 unit tests covering all acceptance criteria |
| `crates/fdemon-app/src/handler/mouse/mod.rs` | Removed `UiMode::Startup` and `UiMode::NewSessionDialog` from `test_scroll_no_op_in_every_mode` — these modes now produce real messages |

### Notable Decisions/Tradeoffs

1. **Modal state directly accessed via public fields**: `FuzzyModalState` and `DartDefinesModalState` are set directly on `new_session_dialog_state` in tests (same pattern used in `keys.rs` tests), rather than going through message dispatch. This keeps tests simple and focused on the scroll logic itself.
2. **No test helpers needed**: The task sketch suggested using existing test helpers from `handler/new_session.rs`, but direct field assignment (`s.new_session_dialog_state.fuzzy_modal = Some(...)`) is cleaner and doesn't create unnecessary coupling.
3. **`test_scroll_no_op_in_every_mode` update**: The stub-era test in `mod.rs` expected all modes to be no-ops. Removing `Startup` and `NewSessionDialog` from the no-op list (with a comment explaining the intent) is the correct approach as each task gets implemented.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo test --workspace` - Passed (1937 fdemon-app tests, 9 new tests in this module)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed

### Risks/Limitations

1. **Other scroll handlers still stubbed**: Tasks 02 (Normal), 03 (DevTools), 04 (Settings), 06 (simple modes) are still stubs. The `test_scroll_no_op_in_every_mode` test in `mod.rs` will need further updates as each is implemented.
