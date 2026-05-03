//! Scroll routing for `UiMode::LinkHighlight`.
//!
//! Phase 2 task 06-link-highlight-scroll populates the body. The stub returns
//! `None` so the dispatcher compiles and tests stay green between waves.

use crate::input_mouse::{KeyModSet, ScrollDir};
use crate::message::Message;
use crate::state::AppState;

pub(super) fn handle_scroll(
    _state: &AppState,
    _dir: ScrollDir,
    _mods: KeyModSet,
) -> Option<Message> {
    None
}
