//! Performance panel handlers.
//!
//! Split into:
//! - [`frame`] — frame selection, frame-chart scroll/page/jump, section focus.
//! - [`details`] — Phase 2 details-pane tab cycling/focus.
//! - [`rebuild_stats`] — Phase 3 rebuild stats event accumulation and toggle.
//! - [`timeline`] — Phase 3 timeline event batch handling and filter cycling.
//!
//! Memory and allocation profile handlers live in [`super::memory`].

mod details;
pub(crate) mod frame;
pub(crate) mod rebuild_stats;
pub(crate) mod timeline;

pub(crate) use details::{handle_perf_cycle_details_tab, handle_perf_focus_details_tab};
pub(crate) use frame::{
    handle_apply_frame_anchor, handle_perf_focus_section, handle_perf_jump_to_end,
    handle_perf_jump_to_start, handle_perf_page, handle_perf_scroll,
    handle_select_performance_frame,
};
pub(crate) use timeline::{
    handle_clear_selection, handle_close_popup, handle_follow_latest, handle_move_selection,
    handle_next_match, handle_open_popup, handle_pan_left, handle_pan_right, handle_prev_match,
    handle_search_input_backspace, handle_search_input_cancel, handle_search_input_char,
    handle_search_input_commit, handle_search_open, handle_select_at, handle_select_first_visible,
    handle_zoom_in, handle_zoom_out,
};
