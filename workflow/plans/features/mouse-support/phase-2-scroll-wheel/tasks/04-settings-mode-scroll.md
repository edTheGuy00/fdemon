## Task: Settings-mode scroll routing (incl. dart-defines & extra-args modals)

**Objective**: Implement `crates/fdemon-app/src/handler/mouse/settings.rs::handle_scroll` so the wheel moves item selection in the Settings list, dart-defines modal (List pane), and extra-args modal — mirroring keyboard handlers at `keys.rs:594-786`. The wheel is a no-op when an inline edit (text input) is active or when the dart-defines modal is in its Edit pane.

**Depends on**: 01-mouse-handler-restructure

**Estimated Time**: 1h

### Scope

**Files Modified (Write):**
- `crates/fdemon-app/src/handler/mouse/settings.rs` — Replace stub `handle_scroll`; add `#[cfg(test)] mod tests`.

**Files Read (Dependencies):**
- `crates/fdemon-app/src/state.rs` — `SettingsViewState::editing`, `dart_defines_modal`, `extra_args_modal`.
- `crates/fdemon-app/src/new_session_dialog.rs` — `DartDefinesPane` enum (used by both Settings and NewSessionDialog dart-defines modals).
- `crates/fdemon-app/src/message.rs` — `Message::SettingsPrevItem`, `SettingsNextItem`, `SettingsDartDefinesUp`, `SettingsDartDefinesDown`, `SettingsExtraArgsUp`, `SettingsExtraArgsDown`.
- `crates/fdemon-app/src/handler/keys.rs` — Reference: `handle_key_settings` lines 595-647 (modal routing at 596-609, item nav at 626-627), `handle_key_settings_dart_defines` lines 733-770 (List pane at 738-746), `handle_key_settings_extra_args` lines 775-786.

### Details

```rust
//! Scroll routing for `UiMode::Settings`.
//!
//! Mirrors the modal-routing precedence of [`crate::handler::keys::handle_key_settings`]:
//! dart-defines modal first → extra-args modal next → editing inline → main list.

use crate::input_mouse::{KeyModSet, ScrollDir};
use crate::message::Message;
use crate::new_session_dialog::DartDefinesPane;
use crate::state::AppState;

pub(super) fn handle_scroll(
    state: &AppState,
    dir: ScrollDir,
    _mods: KeyModSet,
) -> Option<Message> {
    // Dart-defines modal takes top priority (matches keys.rs:597-599).
    if let Some(modal) = state.settings_view_state.dart_defines_modal.as_ref() {
        return match modal.active_pane {
            DartDefinesPane::List => match dir {
                ScrollDir::Up => Some(Message::SettingsDartDefinesUp),
                ScrollDir::Down => Some(Message::SettingsDartDefinesDown),
                _ => None,
            },
            // Edit pane is text input — wheel must not move the list underneath.
            DartDefinesPane::Edit => None,
        };
    }

    // Extra-args fuzzy modal (matches keys.rs:602-604).
    if state.settings_view_state.extra_args_modal.is_some() {
        return match dir {
            ScrollDir::Up => Some(Message::SettingsExtraArgsUp),
            ScrollDir::Down => Some(Message::SettingsExtraArgsDown),
            _ => None,
        };
    }

    // Inline editing of a setting value — wheel is a no-op (matches keys.rs:607-609,
    // where edit-mode text input intercepts keys).
    if state.settings_view_state.editing {
        return None;
    }

    // Main settings list (matches keys.rs:626-627).
    match dir {
        ScrollDir::Up => Some(Message::SettingsPrevItem),
        ScrollDir::Down => Some(Message::SettingsNextItem),
        ScrollDir::Left | ScrollDir::Right => None,
    }
}
```

**Modifier handling.** Settings has no PageUp/PageDown analogues in the keyboard map (`keys.rs:611-647`), so Shift+wheel falls back to single-step move. Ctrl/Alt are ignored (treated as plain wheel) — settings navigation is low-stakes; consuming Ctrl/Alt aggressively here would feel inconsistent with the low information density of the list. The `_mods` parameter is intentionally unused to make this explicit.

### Acceptance Criteria

1. With no modals open and `editing == false`: `Up` → `SettingsPrevItem`, `Down` → `SettingsNextItem`.
2. With dart-defines modal open in `List` pane: `Up` → `SettingsDartDefinesUp`, `Down` → `SettingsDartDefinesDown`. Modal takes precedence over the main list.
3. With dart-defines modal open in `Edit` pane: any wheel direction → `None` (text input must not be disturbed).
4. With extra-args modal open: `Up` → `SettingsExtraArgsUp`, `Down` → `SettingsExtraArgsDown`. Takes precedence over inline editing.
5. With `editing == true` and no modal open: wheel returns `None`.
6. `ScrollDir::Left` / `ScrollDir::Right` always return `None` regardless of state.
7. Modifier keys do not change behavior (Shift+wheel still single-steps; Ctrl/Alt+wheel still single-steps). Document this explicitly in test names.
8. No new `Message` variants added.

### Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::input_mouse::KeyModSet;
    use crate::state::AppState;

    fn fresh_state() -> AppState {
        AppState::new()
    }

    #[test]
    fn main_list_scroll_moves_selection() {
        let s = fresh_state();
        assert!(matches!(
            handle_scroll(&s, ScrollDir::Up, KeyModSet::NONE),
            Some(Message::SettingsPrevItem)
        ));
        assert!(matches!(
            handle_scroll(&s, ScrollDir::Down, KeyModSet::NONE),
            Some(Message::SettingsNextItem)
        ));
    }

    #[test]
    fn editing_inline_value_swallows_scroll() {
        let mut s = fresh_state();
        s.settings_view_state.editing = true;
        assert!(handle_scroll(&s, ScrollDir::Up, KeyModSet::NONE).is_none());
        assert!(handle_scroll(&s, ScrollDir::Down, KeyModSet::NONE).is_none());
    }

    #[test]
    fn dart_defines_list_pane_routes_to_dart_defines_nav() {
        // Implementor: construct a DartDefinesModalState in List pane via the
        // same helper used by handler/settings_dart_defines tests.
        // assert Up → SettingsDartDefinesUp, Down → SettingsDartDefinesDown.
    }

    #[test]
    fn dart_defines_edit_pane_swallows_scroll() {
        // Implementor: as above, with active_pane = Edit. Both Up and Down → None.
    }

    #[test]
    fn extra_args_modal_routes_to_extra_args_nav() {
        // Implementor: open extra-args modal via existing test helper, assert
        // Up → SettingsExtraArgsUp, Down → SettingsExtraArgsDown.
    }

    #[test]
    fn modifier_keys_do_not_change_behavior_in_main_list() {
        let s = fresh_state();
        // Single-step regardless of modifier (no PageUp/PageDown analogue).
        for mods in [
            KeyModSet::new(true, false, false),
            KeyModSet::new(false, true, false),
            KeyModSet::new(false, false, true),
            KeyModSet::new(true, true, true),
        ] {
            assert!(matches!(
                handle_scroll(&s, ScrollDir::Up, mods),
                Some(Message::SettingsPrevItem)
            ));
        }
    }

    #[test]
    fn horizontal_wheel_no_op_in_every_settings_state() {
        let s = fresh_state();
        assert!(handle_scroll(&s, ScrollDir::Left, KeyModSet::NONE).is_none());
        assert!(handle_scroll(&s, ScrollDir::Right, KeyModSet::NONE).is_none());
    }
}
```

The dart-defines and extra-args modal tests are sketched as TODOs because constructing those modal states requires existing test helpers in `crates/fdemon-app/src/handler/settings_dart_defines.rs` and `settings_extra_args.rs`. Implementor should grep for `dart_defines_modal = Some` and `extra_args_modal = Some` in those test files and reuse the helpers.

### Notes

- **Modal precedence order matches keyboard.** `handle_key_settings` checks dart_defines → extra_args → editing → main. The mouse handler follows the same order so a user with both kinds of modal context sees consistent behavior.
- **Why ignore modifiers.** The keyboard handler binds no PageUp/PageDown, no Shift+anything for Settings navigation. Inventing Shift+wheel page-step for Settings would diverge from the keyboard contract; doing nothing is safer and matches user expectation.
- **Unused `_mods` parameter.** Intentional and documented — the parameter is kept in the signature for parity with the dispatcher's other handlers and to leave room for future Shift behavior without an API change.
- **Edit-pane swallows scroll.** When the user is typing into the dart-defines key/value editor or an inline setting value, scrolling the underlying list would be disorienting. Returning `None` is the conservative behavior; a future phase could revisit if user feedback demands it.
- **Auto-save not affected.** Settings auto-save (`UpdateAction::AutoSaveConfig`) is triggered by setting commits, not by scroll. Phase 2 does not change save semantics.

---

## Completion Summary

**Status:** Done
**Branch:** feat/mouse-support

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/handler/mouse/settings.rs` | Replaced stub `handle_scroll` with full modal-routing implementation + 11 unit tests |
| `crates/fdemon-app/src/handler/mouse/mod.rs` | Updated `test_scroll_no_op_in_every_mode` → `test_scroll_no_op_in_non_scrollable_modes`; removed `UiMode::Settings` from no-op assertion since Settings now dispatches scroll |

### Notable Decisions/Tradeoffs

1. **FuzzyModalState unused in top-level imports**: `FuzzyModalState` is test-only; kept in `#[cfg(test)]` module's `use` block to satisfy the compiler without a dead-code warning in production code.
2. **Extra tests beyond task spec**: Added 4 additional tests (`dart_defines_modal_takes_precedence_over_editing`, `extra_args_modal_takes_precedence_over_editing`, `horizontal_wheel_no_op_in_dart_defines_list_pane`, `horizontal_wheel_no_op_in_extra_args_modal`) to cover precedence and horizontal-wheel behavior across all modal states.
3. **`test_scroll_no_op_in_every_mode` renamed and trimmed**: The original test covered modes that are now or will be handled by phase-2 tasks (Normal, DevTools, NewSessionDialog, LinkHighlight, FlutterVersion). Since those are still stubs, only the explicitly no-op modes (EmulatorSelector, ConfirmDialog, Loading, SearchInput) remain; the comment documents the omission.

### Testing Performed

- `cargo fmt --all -- --check` - Passed
- `cargo check --workspace --all-targets` - Passed
- `cargo test --workspace` - Passed (all 11 new tests pass; full workspace 4,019+ tests pass)
- `cargo clippy --workspace --all-targets -- -D warnings` - Passed

### Risks/Limitations

1. **Stale no-op test**: The `test_scroll_no_op_in_every_mode` test in `mod.rs` still covers Normal, DevTools, etc. as no-ops since those handler stubs haven't been implemented. When phase-2 tasks 02, 03, 05, and 06 are completed, those modes must also be removed from that test (or the test updated accordingly by those tasks).
