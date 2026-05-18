//! Performance panel handlers.
//!
//! Split into:
//! - [`frame`] — frame selection, frame-chart scroll/page/jump, section focus.
//! - [`details`] — Phase 2 details-pane tab cycling/focus.
//!
//! Memory and allocation profile handlers live in [`super::memory`].

mod details;
mod frame;

pub(crate) use details::{handle_perf_cycle_details_tab, handle_perf_focus_details_tab};
pub(crate) use frame::{
    handle_perf_focus_section, handle_perf_jump_to_end, handle_perf_jump_to_start,
    handle_perf_page, handle_perf_scroll, handle_select_performance_frame,
};
