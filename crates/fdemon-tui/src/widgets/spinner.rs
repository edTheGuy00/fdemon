//! # Spinner Helper
//!
//! Pure frame-selection math for a braille throbber. Callers supply a
//! monotonically advancing `u64` frame (e.g. `AppState::animation_frame`) and
//! receive the glyph to draw this frame. Intentionally has no `AppState`,
//! rendering, or I/O dependency so it is trivially testable and reusable.

/// Braille throbber frames, in sweep order. Lifted verbatim from the inline
/// `SPINNER` constant previously in `render_loading_screen` so the startup
/// screen has zero visual change after task 02 adopts this helper.
pub const SPINNER_FRAMES: &[char] =
    &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// Ticks per spinner frame for callers driving the spinner off the global
/// `AppState::animation_frame` (which advances every ~50 ms / 20 fps tick).
/// At 2 ticks/frame the throbber advances ~every 100 ms (~10 fps) — lively but
/// calm. The loading screen does NOT use this divisor (see `spinner_char`).
pub const SPINNER_TICKS_PER_FRAME: u64 = 2;

/// The throbber glyph for the given frame index, via direct modulo over
/// `SPINNER_FRAMES`. `frame` is the already-cadence-adjusted index: the caller
/// decides how fast to advance (pass the raw frame for one-glyph-per-tick, or
/// `frame / SPINNER_TICKS_PER_FRAME` for the calmer dialog cadence). Wraps
/// cleanly via `%`, so `u64` wrap in the source frame is harmless.
pub fn spinner_char(frame: u64) -> char {
    SPINNER_FRAMES[(frame % SPINNER_FRAMES.len() as u64) as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spinner_char_advances_deterministically() {
        let len = SPINNER_FRAMES.len() as u64;
        for i in 0u64..25 {
            assert_eq!(
                spinner_char(i),
                SPINNER_FRAMES[(i % len) as usize],
                "spinner_char({i}) should equal SPINNER_FRAMES[{} % {len}]",
                i
            );
        }
    }

    #[test]
    fn spinner_char_wraps_over_frame_set() {
        let len = SPINNER_FRAMES.len() as u64;
        assert_eq!(
            spinner_char(len),
            spinner_char(0),
            "spinner_char(len) should equal spinner_char(0)"
        );
    }

    #[test]
    fn spinner_char_no_panic_near_u64_max() {
        let _ = spinner_char(u64::MAX);
        let _ = spinner_char(u64::MAX - 1);
    }

    #[test]
    fn spinner_frames_match_legacy_constant() {
        // Must exactly match the 10-glyph sequence from render_loading_screen
        let expected = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
        assert_eq!(
            SPINNER_FRAMES.len(),
            10,
            "SPINNER_FRAMES must have exactly 10 glyphs"
        );
        assert_eq!(
            SPINNER_FRAMES,
            expected,
            "SPINNER_FRAMES must match legacy SPINNER constant order"
        );
    }
}
