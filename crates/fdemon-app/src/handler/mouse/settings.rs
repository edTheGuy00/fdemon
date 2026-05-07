//! Mouse event handlers for `UiMode::Settings`.
//!
//! **Scroll** mirrors the modal-routing precedence of
//! [`crate::handler::keys::handle_key_settings`]:
//! dart-defines modal first → extra-args modal next → editing inline → main list.
//!
//! **Press** hit-tests against the per-frame region registry with an editing
//! gate: when the user is typing into an inline setting field (`editing == true`),
//! clicks are suppressed so selection cannot move while text is being entered.

use crate::input_mouse::{KeyModSet, MouseButton, ScrollDir};
use crate::message::Message;
use crate::new_session_dialog::DartDefinesPane;
use crate::state::AppState;

/// Hit-test a left/middle click in `UiMode::Settings` against the per-frame
/// region registry. Returns the matched region's resolved [`Message`].
///
/// **Editing gate.** When `state.settings_view_state.editing` is `true` (the
/// user is typing into an inline setting field), clicks are suppressed so
/// selection cannot move mid-edit. This mirrors [`handle_scroll`]'s editing gate.
///
/// **Right-click reserved.** Right-click returns `None` for future context-menu
/// support, matching the convention established in [`normal::handle_press`].
///
/// [`normal::handle_press`]: super::normal::handle_press
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

    // Sub-modal gate — when a dart-defines or extra-args modal is open, suppress
    // all clicks on the underlying settings panel.  The sub-modals do not change
    // `ui_mode` (they are rendered on top of the Settings panel), so the
    // renderer-level modal gate in `render::view` cannot cover them.  This early
    // return ensures that clicks outside the sub-modal's click regions (which are
    // not yet wired in v1) do not accidentally route to the underlying tab or row.
    if state.settings_view_state.has_modal_open() {
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

pub(super) fn handle_scroll(state: &AppState, dir: ScrollDir, _mods: KeyModSet) -> Option<Message> {
    // Dart-defines modal takes top priority (matches keys.rs:597-599).
    if let Some(modal) = state.settings_view_state.dart_defines_modal.as_ref() {
        return match modal.active_pane {
            DartDefinesPane::List => match dir {
                ScrollDir::Up => Some(Message::SettingsDartDefinesUp),
                ScrollDir::Down => Some(Message::SettingsDartDefinesDown),
                ScrollDir::Left | ScrollDir::Right => None,
            },
            // Edit pane is text input — wheel must not move the list underneath.
            //
            // Asymmetry note: NewSessionDialog's dart-defines modal routes Up/Down in
            // BOTH panes (see `new_session.rs::handle_scroll`). The two surfaces look
            // identical to a user but behave differently. The asymmetry mirrors the
            // underlying keyboard handlers:
            //   - Settings dart-defines (keys.rs:733-770) only binds Up/Down in List pane.
            //   - NewSessionDialog dart-defines (keys.rs:839-866) binds Up/Down in both.
            // Reconciling the two surfaces requires changing the keyboard handler at
            // keys.rs:851-855 — a real product decision, not a polish fix. If pursued,
            // see `workflow/plans/bugs/dart-defines-edit-scroll-asymmetry/` (TBD).
            DartDefinesPane::Edit => None,
        };
    }

    // Extra-args fuzzy modal (matches keys.rs:602-604).
    if state.settings_view_state.extra_args_modal.is_some() {
        return match dir {
            ScrollDir::Up => Some(Message::SettingsExtraArgsUp),
            ScrollDir::Down => Some(Message::SettingsExtraArgsDown),
            ScrollDir::Left | ScrollDir::Right => None,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input_mouse::KeyModSet;
    use crate::new_session_dialog::{
        DartDefine, DartDefinesModalState, DartDefinesPane, FuzzyModalState,
    };
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
        let mut s = fresh_state();
        s.settings_view_state.dart_defines_modal =
            Some(DartDefinesModalState::new(vec![DartDefine::new("K", "V")]));
        // Default active_pane is List.
        assert!(matches!(
            handle_scroll(&s, ScrollDir::Up, KeyModSet::NONE),
            Some(Message::SettingsDartDefinesUp)
        ));
        assert!(matches!(
            handle_scroll(&s, ScrollDir::Down, KeyModSet::NONE),
            Some(Message::SettingsDartDefinesDown)
        ));
    }

    #[test]
    fn dart_defines_edit_pane_swallows_scroll() {
        let mut s = fresh_state();
        let mut modal = DartDefinesModalState::new(vec![]);
        modal.active_pane = DartDefinesPane::Edit;
        s.settings_view_state.dart_defines_modal = Some(modal);
        assert!(handle_scroll(&s, ScrollDir::Up, KeyModSet::NONE).is_none());
        assert!(handle_scroll(&s, ScrollDir::Down, KeyModSet::NONE).is_none());
    }

    #[test]
    fn extra_args_modal_routes_to_extra_args_nav() {
        use crate::new_session_dialog::FuzzyModalType;
        let mut s = fresh_state();
        s.settings_view_state.extra_args_modal =
            Some(FuzzyModalState::new(FuzzyModalType::ExtraArgs, vec![]));
        assert!(matches!(
            handle_scroll(&s, ScrollDir::Up, KeyModSet::NONE),
            Some(Message::SettingsExtraArgsUp)
        ));
        assert!(matches!(
            handle_scroll(&s, ScrollDir::Down, KeyModSet::NONE),
            Some(Message::SettingsExtraArgsDown)
        ));
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

    #[test]
    fn dart_defines_modal_takes_precedence_over_editing() {
        // Even with `editing == true`, the dart-defines modal wins if it is open.
        let mut s = fresh_state();
        s.settings_view_state.editing = true;
        s.settings_view_state.dart_defines_modal =
            Some(DartDefinesModalState::new(vec![DartDefine::new("K", "V")]));
        assert!(matches!(
            handle_scroll(&s, ScrollDir::Up, KeyModSet::NONE),
            Some(Message::SettingsDartDefinesUp)
        ));
    }

    #[test]
    fn extra_args_modal_takes_precedence_over_editing() {
        // Even with `editing == true`, the extra-args modal wins if it is open.
        use crate::new_session_dialog::FuzzyModalType;
        let mut s = fresh_state();
        s.settings_view_state.editing = true;
        s.settings_view_state.extra_args_modal =
            Some(FuzzyModalState::new(FuzzyModalType::ExtraArgs, vec![]));
        assert!(matches!(
            handle_scroll(&s, ScrollDir::Up, KeyModSet::NONE),
            Some(Message::SettingsExtraArgsUp)
        ));
    }

    #[test]
    fn horizontal_wheel_no_op_in_dart_defines_list_pane() {
        let mut s = fresh_state();
        s.settings_view_state.dart_defines_modal =
            Some(DartDefinesModalState::new(vec![DartDefine::new("K", "V")]));
        assert!(handle_scroll(&s, ScrollDir::Left, KeyModSet::NONE).is_none());
        assert!(handle_scroll(&s, ScrollDir::Right, KeyModSet::NONE).is_none());
    }

    #[test]
    fn horizontal_wheel_no_op_in_extra_args_modal() {
        use crate::new_session_dialog::FuzzyModalType;
        let mut s = fresh_state();
        s.settings_view_state.extra_args_modal =
            Some(FuzzyModalState::new(FuzzyModalType::ExtraArgs, vec![]));
        assert!(handle_scroll(&s, ScrollDir::Left, KeyModSet::NONE).is_none());
        assert!(handle_scroll(&s, ScrollDir::Right, KeyModSet::NONE).is_none());
    }

    // ── handle_press tests ───────────────────────────────────────────────

    #[test]
    fn press_no_region_returns_none() {
        let mut s = fresh_state();
        assert!(handle_press(&mut s, 0, 0, MouseButton::Left, KeyModSet::NONE).is_none());
    }

    #[test]
    fn press_registered_region_returns_message() {
        use crate::mouse_regions::{MouseAction, MouseRect};
        let mut s = fresh_state();
        let mut regions = s.mouse_regions.take();
        regions.builder().click(
            MouseRect::new(0, 0, 10, 1),
            MouseAction::emit(Message::SettingsClickRow { index: 0 }),
        );
        s.mouse_regions.set(regions);
        let r = handle_press(&mut s, 0, 0, MouseButton::Left, KeyModSet::NONE);
        assert!(matches!(r, Some(Message::SettingsClickRow { index: 0 })));
    }

    #[test]
    fn press_right_click_is_no_op() {
        use crate::mouse_regions::{MouseAction, MouseRect};
        let mut s = fresh_state();
        let mut regions = s.mouse_regions.take();
        regions.builder().click(
            MouseRect::new(0, 0, 10, 1),
            MouseAction::emit(Message::SettingsClickRow { index: 0 }),
        );
        s.mouse_regions.set(regions);
        assert!(handle_press(&mut s, 0, 0, MouseButton::Right, KeyModSet::NONE).is_none());
    }

    #[test]
    fn click_while_editing_returns_none() {
        use crate::mouse_regions::{MouseAction, MouseRect};
        let mut s = fresh_state();
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
}
