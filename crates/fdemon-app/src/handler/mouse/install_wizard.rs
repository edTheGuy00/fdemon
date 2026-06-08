//! Scroll routing for `UiMode::InstallWizard`.
//!
//! Mirrors `handle_key_install_wizard` (keys.rs): wheel up/down maps to
//! `InstallWizardUp`/`InstallWizardDown`. No click hit-testing is required
//! in Phase 1.

use crate::input_mouse::{KeyModSet, ScrollDir};
use crate::message::Message;
use crate::state::AppState;

pub(super) fn handle_scroll(
    _state: &AppState,
    dir: ScrollDir,
    _mods: KeyModSet,
) -> Option<Message> {
    // Modifiers ignored: InstallWizard has no page-step analogue in the
    // keyboard handler (keys.rs binds only j/k and Up/Down).
    match dir {
        ScrollDir::Up => Some(Message::InstallWizardUp),
        ScrollDir::Down => Some(Message::InstallWizardDown),
        ScrollDir::Left | ScrollDir::Right => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input_mouse::KeyModSet;
    use crate::state::AppState;

    #[test]
    fn wheel_up_moves_wizard_up() {
        let s = AppState::new();
        assert!(matches!(
            handle_scroll(&s, ScrollDir::Up, KeyModSet::NONE),
            Some(Message::InstallWizardUp)
        ));
    }

    #[test]
    fn wheel_down_moves_wizard_down() {
        let s = AppState::new();
        assert!(matches!(
            handle_scroll(&s, ScrollDir::Down, KeyModSet::NONE),
            Some(Message::InstallWizardDown)
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
                Some(Message::InstallWizardUp)
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
