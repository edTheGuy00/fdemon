//! Shared chart-scroll helpers for DevTools panel handlers.
//!
//! Both the Performance panel (`handler/devtools/performance.rs`) and the
//! Memory panel (`handler/devtools/memory.rs`) need identical scroll-direction
//! semantics and a clamped-offset computation. Keeping a single canonical copy
//! here prevents the two implementations from drifting silently — for example
//! if signed-overflow handling or the `max_back` formula ever needs to change.
//!
//! # Visibility
//!
//! All items are `pub(super)`, meaning they are visible to sibling modules
//! under `handler/devtools/` (i.e., `performance.rs`, `memory.rs`, etc.).
//! `mod.rs` additionally re-exports [`ScrollDir`] at `pub(crate)` level so
//! that `handler/update.rs` can name `devtools::ScrollDir` without reaching
//! into a specific panel submodule.

/// Direction enum for chart scroll operations within a DevTools panel.
///
/// `Up` means "scroll back in time" (higher offset — older data), while `Down`
/// means "scroll toward the live edge" (lower offset — newest data).
///
/// This is **not** the same type as [`crate::input_mouse::ScrollDir`], which
/// covers four physical directions (up/down/left/right) for terminal mouse
/// events. This enum is intentionally minimal — only the two directions
/// relevant to time-series chart scrolling.
///
/// Exposed as `pub(crate)` so that `handler/update.rs` can reference
/// `devtools::ScrollDir` directly (via the re-export in `devtools/mod.rs`)
/// without reaching into a specific panel submodule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScrollDir {
    Up,
    Down,
}

/// Clamp a chart scroll offset against the maximum back-scroll.
///
/// ## Parameters
///
/// - `buffer_len` — total number of data points in the ring buffer.
/// - `visible_width` — number of data points visible in the chart at once
///   (the render hint; pass `0` if the hint has not yet been recorded and the
///   caller will substitute a sensible default before calling this function).
/// - `current` — the current scroll offset (0 = live edge; higher = older data
///   visible).
/// - `delta` — signed change to apply: positive scrolls back (toward older
///   data), negative scrolls toward the live edge.
///
/// ## Returns
///
/// The new offset clamped to `[0, buffer_len.saturating_sub(visible_width.max(1))]`.
///
/// ## Underflow guarantee
///
/// `current` and `delta` are combined as `i64` so that a negative `delta`
/// cannot underflow a `usize` subtraction. The `clamp` call on the `i64`
/// result ensures the final value is non-negative before the cast back to
/// `usize`.
pub(super) fn clamp_chart_scroll(
    buffer_len: usize,
    visible_width: usize,
    current: usize,
    delta: i64,
) -> usize {
    let max_back = buffer_len.saturating_sub(visible_width.max(1));
    let new = current as i64 + delta;
    new.clamp(0, max_back as i64) as usize
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── clamp_chart_scroll ────────────────────────────────────────────────────

    #[test]
    fn clamp_scroll_forward_within_bounds() {
        // buffer_len=100, visible=10 → max_back=90; current=0, delta=+5 → 5
        assert_eq!(clamp_chart_scroll(100, 10, 0, 5), 5);
    }

    #[test]
    fn clamp_scroll_back_within_bounds() {
        // current=10, delta=-3 → 7
        assert_eq!(clamp_chart_scroll(100, 10, 10, -3), 7);
    }

    #[test]
    fn clamp_scroll_clamped_at_max_back() {
        // max_back = 100 - 10 = 90; delta would push past it
        assert_eq!(clamp_chart_scroll(100, 10, 85, 10), 90);
    }

    #[test]
    fn clamp_scroll_clamped_at_zero_live_edge() {
        // current=2, delta=-5 would underflow; clamped to 0
        assert_eq!(clamp_chart_scroll(100, 10, 2, -5), 0);
    }

    #[test]
    fn clamp_scroll_visible_width_zero_treated_as_one() {
        // visible_width=0 → max(1) → max_back = 100 - 1 = 99
        assert_eq!(clamp_chart_scroll(100, 0, 0, 5), 5);
        assert_eq!(clamp_chart_scroll(100, 0, 95, 10), 99);
    }

    #[test]
    fn clamp_scroll_empty_buffer_always_zero() {
        // buffer_len=0 → max_back=0 regardless of other params
        assert_eq!(clamp_chart_scroll(0, 10, 0, 1), 0);
        assert_eq!(clamp_chart_scroll(0, 0, 0, 100), 0);
    }
}
